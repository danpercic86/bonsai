# P7 — GitKraken-style graph layout

Restructure the commit-graph row into three zones and enrich the commit node:

1. **Three-zone rows** — `[ LEFT ref column | CENTER graph lanes + node | RIGHT summary + relative time ]`.
   The old single left→right run (`dot → ref pills → summary → author → date`) is replaced.
2. **Author-initials avatar** replaces the plain lane dot; background hue hashed deterministically
   from the author NAME (no network, no email); hover → tooltip with the full name.
3. **Multi-ref overflow** — keep the `+N` chip; hover → tooltip listing the hidden entries stacked.
4. **Collapse local + remote on the SAME commit** — one combined label with a laptop glyph (has
   local) + a cloud glyph (has remote), short name shown once. Diverged refs (different commits)
   stay separate.

House style: `M2-graph.md` (geometry/draw-order/virtualization — **P7 supersedes its row-content
layout only**; lane/edge/virtualization/perf invariants HOLD), `ui-reference.md` (tokens),
`P5-graph-context-menus.md` + `P6-unified-context-menus.md` (context-menu system + `GraphContextTarget`
+ `branchMenuItems` — **unchanged and reused verbatim**).

---

## §0 Scope, invariants, and the load-bearing decisions

### 0.1 Rust / wire: UNCHANGED
`src-tauri/src/graph.rs` and every Rust wire type (`GraphNode`, `RefLabel`, `GraphLayout`, …) are
**UNCHANGED in P7**. Justification:
- The author NAME is already on `GraphNode.author`; all refs are already on `GraphNode.refs`
  (pre-sorted: `localBranch` head-first, then `remoteBranch`, then `tag`; the `head` kind carries a
  detached-HEAD label). No new field is required.
- The local+remote **collapse** is a per-node DISPLAY grouping over `refs[]` — pure presentation
  math, not lane/edge/row math. It lives in the frontend renderer, same category as the existing WIP
  compositing and the P5 pill hit-test (see `P5 §1.4`).
- The avatar hue is derived in the renderer from the string already on the wire.

Because the wire shape is unchanged, **`src/ipc/types.ts` and the Rust↔TS mirror do not change**, and
**graph mock fixtures need no shape change** (P7d only adds fixture ROWS/refs that exercise P7 — §9).

### 0.2 Invariants enforced
- Rust owns all Git + layout math; React only renders. (§0.1 keeps this true.)
- Canvas stays virtualized to visible rows; avatars/icons/labels draw **per visible row only**
  (≈30–60 rows). Text measurement stays cached via the existing `measure`/`truncateToWidth`. The 20k
  perf gate is preserved.
- All colors flow through the `Theme` object / CSS custom properties. The avatar hue-hash produces a
  computed HSL color with FIXED saturation/lightness (§3) so it reads on both themes; no per-frame
  `getComputedStyle`.
- Tooltips are DOM overlays inside `.graph-canvas-host` (canvas can't do native tooltips), so they
  are inspectable in the a11y tree.

### 0.3 Locked product decisions (do not reopen)
- Avatars: initials-only colored circles; color hashed from author name; hover → full name.
- Collapse ONLY when local & remote point at the SAME commit; diverged → render separately.
- LEFT column holds ALL refs (local, remote [collapsed per above], tags, detached HEAD). RIGHT side is
  summary + timestamp ONLY. There are NO right-side pills anymore. Author text is REMOVED from the row.

---

## §1 New row geometry / column model  (supersedes M2 §3.2 row content + pass 5)

All coordinates CSS px; the ctx is DPR-transformed (unchanged). Row height stays **28**.

### 1.1 Zones (x-ranges as functions of viewport width `W` and `laneCount`)

```
LEFT  ref column : [ REF_COL_PAD_LEFT , REF_COL_WIDTH - REF_COL_PAD_RIGHT ]   (fixed band)
CENTER graph     : lanes begin at REF_COL_WIDTH + gutter ; node avatar at laneX(node.lane)
RIGHT summary    : [ summaryStartX(laneCount) , dateLeft - colGap ]           (flex)
RIGHT timestamp  : right-aligned at dateRight, width dateColWidth
```

The LEFT ref column is a **fixed reserved band** on the left; the graph is shifted right by
`REF_COL_WIDTH`. This is the GitKraken arrangement (refs left, graph center, message right) and keeps
lanes vertically aligned while scrolling (a dynamic per-row width would jitter the graph horizontally
— rejected). Ref-less rows leave the band empty (the accepted GitKraken tradeoff; see §12 FLAG-1).

### 1.2 Geometry helpers (replace `laneX`, `textColumnX`, `pillArea` in `draw.ts`)

```ts
// laneX gains the fixed ref-band offset; the +8 lane inset is preserved.
export function laneX(lane: number): number {
  return METRICS.refColWidth + METRICS.gutter
       + Math.min(lane, METRICS.maxRenderLanes - 1) * METRICS.laneWidth + 8;
}

// Right edge of the graph area (clamped lane band), independent of the +8 inset.
function graphAreaRight(laneCount: number): number {
  return METRICS.refColWidth + METRICS.gutter
       + Math.min(laneCount, METRICS.maxRenderLanes) * METRICS.laneWidth;
}

// Summary column origin (replaces the old textColumnX; no pills live here now).
export function summaryStartX(laneCount: number): number {
  return graphAreaRight(laneCount) + METRICS.textGap;
}

// Fixed LEFT ref-column layout window (analog of the old pillArea; NOT a function
// of W or laneCount — the band is fixed).
export function refColArea(): { startX: number; budget: number } {
  return {
    startX: METRICS.refColPadLeft,
    budget: Math.max(0, METRICS.refColWidth - METRICS.refColPadLeft - METRICS.refColPadRight),
  };
}
```

`rowY`, `rowAtPoint`, `relativeDate`, `measure`, `truncateToWidth`, the edge renderer, and
`segmentTo` are **UNCHANGED** (edges use `laneX`/`rowY`, so the global right-shift flows through
automatically). `HALF_ROW`, `EDGE_CLAMP_MARGIN` unchanged.

`METRICS.authorColWidth` becomes unused by the graph (author removed). Leave the constant in place
(no churn); mark it `/** @deprecated P7: author removed from graph rows. */`.

---

## §2 Avatar (replaces pass-4 dot)

### 2.1 Geometry & composition (drawn per visible row, at `x = laneX(node.lane)`, `y = rowY(row)`)
Draw order at each node (specified inner→outer):
1. **bg ring**: `arc(x, y, avatarRadius + avatarBgRingExtra)` filled `theme.bg0` — edges passing
   under read cleanly (same purpose as today's dot bg ring).
2. **avatar disc**: `arc(x, y, avatarRadius)` filled with the hashed name color (§3 `avatarColor().bg`).
3. **lane ring**: stroke `arc(x, y, avatarRadius)`, `lineWidth = avatarRingWidth`,
   `strokeStyle = theme.laneColors[node.lane % 10]` — ties the avatar to its lane color.
4. **initials**: `font = "${METRICS.avatarFont} ${FONT_UI}"`, `textAlign='center'`,
   `textBaseline='middle'`, `fillStyle = avatarColor().text`, draw `initials(node.author)` at (x, y).
   (Restore `textAlign='left'` afterward, as the text pass expects.)
5. **HEAD ring** (if `layout.headIndex === row`): stroke `arc(x, y, avatarHeadRingRadius)`,
   `lineWidth 1.5`, `strokeStyle = theme.text1`.
6. **selection ring** (if `selectedIndex === row`): stroke `arc(x, y, avatarSelRingRadius)`,
   `lineWidth 1.5`, `strokeStyle = theme.accent`.

All rings fit inside row height 28: max radius `avatarSelRingRadius = 11.5` → dia 23, 2.5px margin.

### 2.2 Initials extraction — pure, exported from `draw.ts`
```ts
/** 1–2 uppercased chars from an author display name. Surrogate-safe (Array.from). */
export function initials(name: string): string;
```
Rules:
- `tokens = name.trim().split(/\s+/).filter(t => t.length > 0)`.
- `tokens.length === 0` → return `'?'` (fallback glyph).
- `tokens.length === 1` → `chars = Array.from(tokens[0])`; return
  `(chars[0] + (chars[1] ?? '')).toUpperCase()` (single-char token → 1 char).
- `tokens.length >= 2` → `(Array.from(tokens[0])[0] + Array.from(tokens[1])[0]).toUpperCase()`.

Examples: `"Dan Percic"→"DP"`, `"torvalds"→"TO"`, `"x"→"X"`, `""→"?"`, `"  Grace  Hopper "→"GH"`.

### 2.3 Deterministic name→color — pure, exported from `draw.ts`
```ts
export interface AvatarColor { bg: string; text: string; }
/** Deterministic; theme-invariant; legible on both canvases. */
export function avatarColor(name: string): AvatarColor;
```
Algorithm:
```
hashString(s):                       // FNV-1a 32-bit, Math.imul for 32-bit overflow
  h = 0x811c9dc5
  for cp of Array.from(s): h = Math.imul(h ^ cp.codePointAt(0), 0x01000193)
  return h >>> 0
hue = hashString(name.trim()) % 360
bg  = `hsl(${hue}, ${AVATAR.sat}%, ${AVATAR.light}%)`     // AVATAR.sat=52, AVATAR.light=42
text = '#ffffff'
```
Fixed S=52% / L=42% keeps the disc mid-dark for every hue → white initials clear ≥3:1 on BOTH the
dark (`#16181d`) and light (`#ffffff`) canvas; the disc itself is theme-invariant (like lane colors),
so no `getComputedStyle` per paint. Determinism: same name ⇒ same hue, always. (`AVATAR` const in
`metrics.ts`, §8.) No new CSS var needed.

### 2.4 Avatar hit-test — pure, exported (shared by tooltip hover)
```ts
export function avatarHit(px: number, py: number, cx: number, cy: number): boolean;
// return (px-cx)^2 + (py-cy)^2 <= (avatarRadius + avatarBgRingExtra)^2
```
Uses the bg-ring radius so the whole visible disc is hoverable.

---

## §3 Ref-grouping / collapse transform — pure, exported from `draw.ts`

### 3.1 Display-entity type
```ts
export type RefEntity =
  | {
      kind: 'branch';
      name: string;          // SHORT name shown once ("main"), key of the group
      hasLocal: boolean;     // laptop glyph when true
      remotes: string[];     // full remote shorthands, e.g. ["origin/main"]; cloud glyph when non-empty
      isHead: boolean;       // attached HEAD local branch
      refs: RefLabel[];      // the underlying wire refs (for hit-test targeting)
    }
  | { kind: 'tag';  name: string; ref: RefLabel }
  | { kind: 'head'; name: string; ref: RefLabel };   // detached-HEAD label
```

### 3.2 `groupRefs`
```ts
export function groupRefs(refs: readonly RefLabel[] | undefined): RefEntity[];
```
Algorithm (insertion order preserved; input is already sorted local-head-first / remotes / tags):
```
branches = new Map<string, branchEntity>()   // insertion-ordered
tags: RefEntity[] = []; heads: RefEntity[] = []
for ref of (refs ?? []):
  switch ref.kind:
    'localBranch':
      key = ref.name
      e = branches.get(key) ?? { kind:'branch', name:key, hasLocal:false, remotes:[], isHead:false, refs:[] }
      branches.set(key, e)
      e.hasLocal = true; e.isHead = e.isHead || ref.isHead; e.refs.push(ref)
    'remoteBranch':
      short = ref.name.slice(ref.name.lastIndexOf('/') + 1)   // "origin/main" -> "main"
      e = branches.get(short) ?? { kind:'branch', name:short, hasLocal:false, remotes:[], isHead:false, refs:[] }
      branches.set(short, e)
      e.remotes.push(ref.name); e.refs.push(ref)
    'tag':  tags.push({ kind:'tag',  name: ref.name, ref })
    'head': heads.push({ kind:'head', name: ref.name, ref })
return [...heads, ...branches.values(), ...tags]
```
- **Same-commit collapse:** local `main` + `origin/main` land on one node → one `branch` entity
  `{name:"main", hasLocal:true, remotes:["origin/main"]}` → laptop + cloud, name once.
- **Diverged:** local `feat` on row A, `origin/feat` on row B → each node holds only its own ref →
  two separate entities on two rows (row A: local-only; row B: remote-only). No cross-node grouping.
- **Remote-only:** `origin/release` with no local → `{hasLocal:false, remotes:["origin/release"]}` →
  cloud only. Display label = SHORT name `release` (see §12 FLAG-2).
- **Ordering:** detached `head` first, then branch entities (local-first by construction), then tags.

### 3.3 Entity style — pure, exported (reuses the existing `PillStyle`)
```ts
export function entityStyle(e: RefEntity, node: GraphNode, theme: Theme): PillStyle;
```
| entity | fill | text | border | label | icons |
|---|---|---|---|---|---|
| `branch` isHead (attached HEAD) | `laneColor` | `accentText` | null | `name` | laptop(+cloud if remotes) |
| `branch` hasLocal, !isHead | `laneAlpha` | `laneColor` | `laneColor` | `name` | laptop(+cloud if remotes) |
| `branch` remote-only | `bg2` | `text2` | `border` | `name` (short) | cloud |
| `tag` | `TAG_BG` | `TAG_COLOR` | `TAG_COLOR` | `# ${name}` | none |
| `head` (detached) | `danger` | `#ffffff` | null | `name` | none |

`laneColor = theme.laneColors[node.lane%10]`, `laneAlpha = theme.laneColorsAlpha[node.lane%10]`.
Note: the old `⌂ ` HEAD prefix is DROPPED (the laptop icon + solid fill now convey local + head).
Icon color = `PillStyle.text`.

### 3.4 Icons — canvas vector recipes (RECOMMENDED over emoji; see §12 FLAG-3)
Drawn monochrome with `strokeStyle = style.text`, `lineWidth = 1.2`, `lineJoin/lineCap = 'round'`,
inside an `ICON_SIZE × ICON_SIZE` box at `(bx, by)` (by = row-center − ICON_SIZE/2). Fractions of `S = ICON_SIZE`:

```
drawLaptopIcon(ctx, bx, by, S):
  // screen (rounded rect): x S*0.15, y S*0.10, w S*0.70, h S*0.52, r S*0.08 -> stroke
  // base: line (bx+S*0.05, by+S*0.82) -> (bx+S*0.95, by+S*0.82) -> stroke
  // sides: (bx+S*0.15, by+S*0.62)->(bx+S*0.05, by+S*0.82) ; (bx+S*0.85, by+S*0.62)->(bx+S*0.95, by+S*0.82)

drawCloudIcon(ctx, bx, by, S):
  b = by + S*0.74                                   // baseline
  beginPath()
  arc(bx+S*0.34, b - S*0.16, S*0.20, Math.PI*0.5, Math.PI*1.5)   // left lobe
  arc(bx+S*0.52, b - S*0.30, S*0.22, Math.PI*1.05, Math.PI*1.95) // top lobe
  arc(bx+S*0.68, b - S*0.14, S*0.18, Math.PI*1.5, Math.PI*0.5)   // right lobe
  lineTo(bx+S*0.30, b) ; closePath() ; stroke()
```
Exact arc angles may be tuned by the implementer to produce a recognizable laptop/cloud; **positions,
box size, stroke width, and color are normative.** The reviewer eyeballs the glyphs in the harness.

---

## §4 LEFT-column layout + overflow helper (the load-bearing shared pattern)

Analog of the old `layoutRowPills`, relocated to the LEFT band and operating on `RefEntity`. Single
source of truth for BOTH the draw pass and hit-testing — they can never diverge.

```ts
export interface LaidRefLabel {
  entity: RefEntity | null;   // null == the "+n" overflow chip
  style: PillStyle;
  x: number;                  // left edge, in canvas CSS-px (ref-column space)
  w: number;                  // full pill width incl. padding + icons
  icons: { laptop: boolean; cloud: boolean };   // false/false for chip, tag, head
}

/** PURE (no drawing). Sets ctx.font internally (pillFont). Lays entities L→R in
 *  the fixed band [startX, startX+budget]; breaks before an entity that would
 *  exceed the budget (except the first); appends a "+n" chip counting HIDDEN
 *  ENTITIES. Mirrors the old layoutRowPills overflow rule exactly. */
export function layoutRefLabels(
  ctx: CanvasRenderingContext2D,
  entities: readonly RefEntity[],
  node: GraphNode,
  theme: Theme,
  startX: number,
  budget: number,
): LaidRefLabel[];
```
Per-entity width:
```
iconsW(icons) = (icons.laptop?ICON_SIZE:0) + (icons.cloud?ICON_SIZE:0) + (icons.laptop&&icons.cloud? iconGap : 0)
anyIcon       = icons.laptop || icons.cloud
labelMaxPx    = pillMaxWidth - 2*pillPadX - iconsW - (anyIcon ? iconGap : 0)
labelText     = truncateToWidth(ctx, style.label, labelMaxPx)          // reuse cached truncation
w             = 2*pillPadX + iconsW + (anyIcon ? iconGap : 0) + ceil(measure(ctx, labelText))
```
Loop (identical break rule to today):
```
x = startX; shown = 0
for e in entities:
  style = entityStyle(e, node, theme); icons = iconsFor(e)   // {laptop:e.hasLocal, cloud:e.remotes.length>0} for branch; else {false,false}
  w = pillWidth(...)
  if shown > 0 and x + w > startX + budget: break
  push { entity:e, style, x, w, icons }; x += w + pillGap; shown++
hidden = entities.length - shown
if hidden > 0:
  chipStyle = { fill: bg2, text: text2, border, label: `+${hidden}` }
  push { entity:null, style:chipStyle, x, w: pillWidth(chipStyle,noIcons), icons:{false,false} }
```
The chip's hidden-entity labels are needed by the tooltip; recover them as
`entities.slice(shown).map(e => entityStyle(e,node,theme).label)` at hover time (§6), OR carry them on
the chip — simplest is recompute at hover (cheap, one row). Normative: **tooltip lists
`entityStyle(hiddenEntity).label` for each hidden entity, in entity order.**

### 4.1 Draw pass rewrite (`drawGraph`)
- **Pass 4** now draws the AVATAR (§2) instead of the dot.
- **Pass 5** per visible row:
  - **5a (LEFT):** `const {startX,budget}=refColArea(); const laid=layoutRefLabels(ctx, groupRefs(node.refs), node, theme, startX, budget);` draw each `LaidRefLabel` via `drawRefLabelAt` (pill bg/border, then icons at `x+pillPadX`, then label after icons). Clip: nothing draws past `REF_COL_WIDTH - REF_COL_PAD_RIGHT` (guaranteed by the budget).
  - **5b (summary):** `const sx = summaryStartX(layout.laneCount);` `summaryMax = dateLeft - colGap - sx;` draw `truncateToWidth(node.summary, summaryMax)` at `sx`. (`dateLeft = W - dateColWidth - colGap`, `dateRight = W - colGap`.)
  - **5c:** REMOVED (author).
  - **5d (date):** unchanged — `relativeDate(node.ts, now)` right-aligned at `dateRight`, width `dateColWidth`.
- `drawRefLabelAt(ctx, laid, cy)`: reuse today's `drawPillAt` rounded-rect body; add the icon block
  before the label; re-truncate label to the same `labelMaxPx` used in `layoutRefLabels` so drawn and
  measured widths stay pixel-identical (same discipline as today's `pillWidth`/`drawPillAt`).

---

## §5 Hit-testing + context-menu parity

`GraphContextTarget` and `branchMenuItems(name, kind)` (RepoWorkspace) are **UNCHANGED**. Only
`GraphCanvas.handleContextMenu` changes: resolve a click in the LEFT ref column via the new helpers.

```
handleContextMenu(e):
  e.preventDefault()
  y = e.clientY - rect.top ; x = e.clientX - rect.left
  hit = hitTest(y, scrollTop, wipOffset, nodes.length)
  if hit is null or 'wip': return
  node = nodes[hit]
  if ctx && theme && x < METRICS.refColWidth and node.refs?.length:
    { startX, budget } = refColArea()
    laid = layoutRefLabels(ctx, groupRefs(node.refs), node, theme, startX, budget)
    hitLabel = laid.find(l => l.entity !== null && x >= l.x && x <= l.x + l.w)
    if hitLabel:
      ref = targetRefOf(hitLabel.entity)     // see below
      if ref: onContextMenu({ kind:'ref', ref }, e.clientX, e.clientY); return
      // (tag/head entity resolve to a ref whose branchMenuItems is [] -> menu won't open)
  // empty band OR the "+n" chip OR non-ref entity -> fall through to the commit target
  onContextMenu({ kind:'commit', index: hit, oid: node.id }, e.clientX, e.clientY)
```
`targetRefOf(entity)` (collapsed-label targeting rule — LOCKED recommendation):
- `branch`, `hasLocal` (local-only OR collapsed local+remote) → the **local** `RefLabel`
  (`entity.refs.find(r => r.kind==='localBranch')`). Its P6 menu already has Checkout/Merge/Rebase/
  Compare/Delete — so a collapsed `main` targets the local `main`.
- `branch`, remote-only → the remote `RefLabel` (`entity.refs.find(r => r.kind==='remoteBranch')`;
  if multiple remotes, `remotes[0]`'s ref — §12 FLAG-2).
- `tag`/`head` entity → its `ref` (menu resolves to `[]` → no menu opens; matches today).

The `+N` chip is `entity === null` → not a ref target → falls through to the commit `Compare with HEAD`
target (identical to today's chip behavior). **Left-click stays row-select** (unchanged `handleClick`);
the avatar and `+N` chip are HOVER (tooltip) targets only, never click/context targets.

Update `GraphCanvas` imports: drop `pillArea`, `layoutRowPills`; add `refColArea`, `layoutRefLabels`,
`groupRefs`. Remove the now-dead `pillArea`/`layoutRowPills`/`pillStyle` exports from `draw.ts` **only
after** confirming no other importer (grep) — `pillStyle` is subsumed by `entityStyle`; keep `PillStyle`.

---

## §6 Tooltip overlay (DOM, inside `.graph-canvas-host`)

`.graph-canvas-host` is `position: relative` — render an absolutely-positioned tooltip div inside it.

### 6.1 State + triggers
Add React state `const [tooltip, setTooltip] = useState<TooltipState | null>(null)` where
```ts
type TooltipState =
  | { kind: 'avatar'; text: string;  anchor: Rect }
  | { kind: 'overflow'; lines: string[]; anchor: Rect };
type Rect = { left: number; top: number; width: number; height: number };  // host CSS coords
```
It changes only when the hover TARGET changes (not per frame), so re-renders are rare.

Extend `handleMouseMove` (and re-run inside `handleScroll`, like `hoverRow` already is) to compute the
hover target from `(mouseX, mouseY)`:
```
row = hitTestAtMouseY(y, scrollTop)         // existing; -1 => WIP, null => none
if row is a real layout row:
  cy = (row + wipOffset)*rowHeight + rowHeight/2 - scrollTop     // row center in host CSS coords
  cx = laneX(node.lane)
  if avatarHit(x, y, cx, cy):
      target = { kind:'avatar', text: node.author, anchor: avatarRect(cx, cy) }
  else if node.refs?.length && x < refColWidth:
      laid = layoutRefLabels(ctx, groupRefs(node.refs), node, theme, refColArea())
      chip = laid.find(l => l.entity === null && x>=l.x && x<=l.x+l.w)
      if chip:
          shown = laid.filter(l => l.entity !== null).length
          hidden = groupRefs(node.refs).slice(shown)
          lines = hidden.map(e => entityStyle(e,node,theme).label)
          target = { kind:'overflow', lines, anchor: pillRect(chip.x, chip.w, cy) }
      else target = null
  else target = null
else target = null
setTooltip(prev => sameTarget(prev,target) ? prev : target)
```
- `avatarRect(cx, cy)` = `{ left: cx - R, top: cy - R, width: 2R, height: 2R }`,
  `R = avatarRadius + avatarBgRingExtra`.
- `pillRect(x, w, cy)` = `{ left: x, top: cy - pillHeight/2, width: w, height: pillHeight }`.
- `x = e.clientX - scroller.getBoundingClientRect().left` (add to the existing mousemove which today
  only reads `y`; store `mouseXRef` alongside `mouseYRef` so scroll re-runs can recompute).

### 6.2 Positioning + dismissal
- Default place BELOW the anchor: `left = anchor.left`, `top = anchor.top + anchor.height + 4`.
- Clamp to host: if `left + tipW > hostW` → `left = hostW - tipW - 4`; `left = max(4, left)`.
  If `top + tipH > hostH` → place ABOVE: `top = anchor.top - tipH - 4`.
- Measure `tipW/tipH` in a `useLayoutEffect` that reads the tip element's `getBoundingClientRect`,
  then applies the clamp via a second style update (initial render can start at the un-clamped point).
- Dismiss: `handleMouseLeave` → `setTooltip(null)` (in addition to clearing `hoverRow`). Scroll keeps
  it in sync (recompute); if the target clears, `setTooltip(null)`.
- No timer needed; OPTIONAL 120 ms show-delay for polish (leave out for v1 so the a11y tree shows it
  immediately for orchestrator inspection).

### 6.3 DOM + a11y
```tsx
{tooltip && (
  <div className="graph-tooltip" role="tooltip" style={{ left, top }}>
    {tooltip.kind === 'avatar'
      ? tooltip.text
      : tooltip.lines.map((l, i) => <div key={i}>{l}</div>)}
  </div>
)}
```
Rendered as a sibling of `<canvas>`/`.graph-scroll` inside `.graph-canvas-host`. `pointer-events:none`
so it never eats the overlay's input. Plain text content → present in the a11y tree.

CSS (`src/styles.css`), reusing tokens:
```
.graph-tooltip {
  position: absolute; z-index: 5; pointer-events: none;
  max-width: 260px; padding: 4px 8px; border-radius: 6px;
  background: var(--bg-3); color: var(--text-1); border: 1px solid var(--border);
  font: 400 12px/1.4 var(--font-ui, "Segoe UI Variable","Segoe UI",system-ui,sans-serif);
  box-shadow: 0 2px 8px rgba(0,0,0,0.35); white-space: nowrap;
}
.graph-tooltip > div { white-space: nowrap; }   /* one hidden ref per line */
```

---

## §7 WIP row (`drawWipRow`) under the new model
- The dashed warning circle stays a **dot** (radius 4, `theme.warning` ring, `bg0` fill) at
  `laneX(headLane)` — the WIP node is NOT a commit, so it gets NO avatar/initials.
- Move the "Uncommitted changes" label + `(N files)` count from `textColumnX(...)` to
  **`summaryStartX(layout.laneCount)`** (the summary zone). The LEFT ref column stays empty for the
  WIP row. Everything else in `drawWipRow` (dashed connector to HEAD, hover bg) is unchanged; the
  global right-shift flows through `laneX`. Delete the now-dead local `textColumnX` helper.

---

## §8 Metrics additions (`src/graph/metrics.ts`)

Add to `METRICS` (values in CSS px unless noted):
```ts
  /** P7 §1: fixed LEFT ref-column band width. Holds ~1 medium label + icons or
   *  two short labels before "+n"; bounded so ref-less rows don't shove the graph
   *  too far right (see P7 §12 FLAG-1). */
  refColWidth: 180,
  refColPadLeft: 12,     // gutter before the first ref label (matches graph gutter feel)
  refColPadRight: 8,     // gap between the ref band and the graph gutter
  /** P7 §2: commit avatar. dia 16 fits row 28 with rings (max ring dia 23). */
  avatarRadius: 8,
  avatarBgRingExtra: 2,      // bg0 halo behind the avatar (edge readability)
  avatarRingWidth: 1.5,      // lane-color ring
  avatarHeadRingRadius: 10.5,
  avatarSelRingRadius: 11.5,
  avatarFont: '600 9px',     // 2 initials inside a dia-16 disc
  /** P7 §3.4: ref-label glyph box + gap (icon-icon and icon-label). */
  iconSize: 11,
  iconGap: 3,
```
Add a sibling const:
```ts
/** P7 §2.3: avatar hue-hash HSL constants (theme-invariant, legible both themes). */
export const AVATAR = { sat: 52, light: 42 } as const;
```
Keep `authorColWidth` (deprecated, §1.2). `rowHeight/laneWidth/gutter/dotRadius/edgeWidth/textGap/
colGap/dateColWidth/pill*/maxRenderLanes` unchanged.

---

## §9 Mock IPC + fixtures (P7d)

Wire shape is unchanged (§0.1) → `src/ipc/mock.ts` needs **no** method-signature change. Only the
graph FIXTURES gain P7-exercising cases, and — mirroring `P6 §1.4` — every new ref name MUST also
exist in `INITIAL_BRANCHES` so its right-click menu resolves.

`src/ipc/fixtures/graph.ts` (`buildMockGraph`):
- **Row 0 (collapse + overflow):** it already carries `main`(local head) + `origin/main`(remote) +
  `v1.0`(tag). ADD enough refs to (a) show the collapsed `main` (laptop+cloud) and (b) overflow the
  180px band → `+N`. Suggested row-0 refs:
  `main`(local,head), `origin/main`(remote), `dev`(local), `origin/dev`(remote),
  `origin/release`(remote-only), `v1.0`(tag), `v0.9`(tag). → entities:
  `main[L+R,head]`, `dev[L+R]`, `release[R]`, `#v1.0`, `#v0.9` (5) → band shows `main` + maybe `dev`
  then `+N`.
- **Diverged pair:** keep `feat`(local) on row 1; ADD `origin/feat`(remote) on a DIFFERENT row (e.g.
  row 4 "feat: start"). Harness must show them as SEPARATE labels (row1 laptop-only `feat`, row4
  cloud-only `feat`), proving diverged refs don't collapse.
- **Single-word author:** set one row's `author` to a single token (e.g. `"torvalds"`) so `initials`
  yields `"TO"` (multi-word rows already yield e.g. `"AL"`, `"GH"`).
- Detached variant (`buildMockGraphDetached`) unchanged — row 5 `head` label still renders (red, no
  icons).

`src/ipc/fixtures/branches.ts` (`INITIAL_BRANCHES`): add locals `dev` and (if absent) `feat`;
add remotes `origin/dev`, `origin/release`, `origin/feat` (each with a distinct 40-hex `tip`). This
keeps every new pill's `branchMenuItems` non-empty (Checkout/Merge/Rebase/Compare/Delete) so the
harness can verify context-menu parity from the LEFT column.

---

## §10 Acceptance criteria

### AI gate (orchestrator-verifiable)
1. `pnpm build` (`tsc && vite build`) clean; no console errors in the harness (both themes).
2. **Pure-fn checks.** The repo has NO TS test runner (see §12 FLAG-4). Expose the pure helpers in
   mock mode on `window.__bonsai.p7 = { initials, avatarColor, groupRefs, layoutRefLabels, refColArea,
   avatarHit, relativeDate }` (dev/mock only, like the existing `scrollSweep`), plus a
   `p7SelfTest(): {pass:number; fail:number; failures:string[]}` that asserts and returns results the
   orchestrator reads via `window.__bonsai.p7SelfTest()`:
   - `initials`: `"Dan Percic"→"DP"`, `"torvalds"→"TO"`, `"x"→"X"`, `""→"?"`, `"  a  b "→"AB"`.
   - `avatarColor`: deterministic (same name → same `bg` across calls); `bg` matches
     `/^hsl\(\d{1,3}, 52%, 42%\)$/`; `text === '#ffffff'`; two different names generally differ.
   - `groupRefs` same-commit: `[main(local,head), origin/main(remote), v1.0(tag)]` →
     `[branch{name:'main',hasLocal:true,remotes:['origin/main'],isHead:true}, tag{v1.0}]`.
   - `groupRefs` diverged: local `feat` node and remote `origin/feat` node → two entities, each on its
     own node: `{name:'feat',hasLocal:true,remotes:[]}` and `{name:'feat',hasLocal:false,remotes:['origin/feat']}`.
   - `layoutRefLabels` overflow: given many entities + a small `budget`, first entity always laid,
     total laid ≤ what fits, trailing `entity:null` chip with `label === "+" + hiddenCount`.
   - `refColArea`: `startX === refColPadLeft`, `budget === refColWidth - padLeft - padRight`.
   - `relativeDate` outputs unchanged from today (regression guard).
3. **Browser harness** (`pnpm dev`, `VITE_MOCK_IPC=1`), BOTH dark + light:
   - Three-zone layout: LEFT ref labels, CENTER graph + avatars, RIGHT summary + relative time; NO
     right-side pills; NO author text on rows.
   - Each commit node is a colored disc with 1–2 initials + a lane-color ring; HEAD node has the
     text-1 ring; selected node has the accent ring.
   - Row 0 shows ONE collapsed `main` label bearing BOTH a laptop and a cloud glyph; the row overflows
     to a `+N` chip.
   - The diverged `feat` / `origin/feat` render as two SEPARATE labels on two rows.
   - Hover the avatar → `role="tooltip"` div with the full author name (present in the a11y tree).
   - Hover the `+N` chip → tooltip listing the hidden entity labels, one per line.
   - Right-click a collapsed `main` in the LEFT column → same 5-item local menu as its sidebar row
     (targets the LOCAL branch); right-click a remote-only label → the remote menu; tag/HEAD → no menu;
     `+N` chip / empty band → the commit `Compare with HEAD` item. (Parity with P6, from the new column.)
   - `window.__bonsai.scrollSweep()` over the fixture logs frame timing with no sustained >33 ms
     frames (perf not regressed vs pre-P7). Avatars/labels draw for visible rows only.
4. On the 20k perf fixture path, layout math is untouched (§0.1); the scroll sweep still meets the M2
   perf gate.

### USER CHECKPOINT (native `pnpm tauri dev`)
- Real repo: rows read as three clean zones; refs sit left, graph center, message + relative time right.
- Avatars show correct initials for real authors; hovering shows the full name; same author → same color.
- A branch whose local & remote are level shows ONE label with laptop+cloud; after diverging (commit
  locally without pushing) they split into two labels — verified live.
- `+N` hover lists the hidden refs; right-clicking refs in the left column drives the identical
  P6 menu (checkout/merge/rebase/compare/delete) as the sidebar. Scrolling stays smooth.

---

## §11 Decomposition (implement → review → commit)

- **P7a — Pure helpers + metrics + self-test.** `metrics.ts` (§8 additions + `AVATAR`); `draw.ts`
  pure exports: `initials`, `avatarColor`, `avatarHit`, `RefEntity`, `groupRefs`, `entityStyle`,
  `refColArea`, `laneX`/`summaryStartX`/`graphAreaRight` updates, `LaidRefLabel`, `layoutRefLabels`,
  icon recipes `drawLaptopIcon`/`drawCloudIcon`; wire `window.__bonsai.p7` + `p7SelfTest` in
  `GraphCanvas.tsx` (mock-only). *Files:* `src/graph/metrics.ts`, `src/graph/draw.ts`,
  `src/graph/GraphCanvas.tsx`. *Gate:* `tsc` clean; `p7SelfTest()` all-pass in the harness console.
- **P7b — Renderer restructure (zones + avatars + icons).** `drawGraph` pass 4 (avatar) + pass 5
  rewrite (LEFT ref column via `layoutRefLabels` + `drawRefLabelAt`; summary at `summaryStartX`;
  date unchanged; author removed); `drawWipRow` label move (§7); remove dead `pillStyle`/
  `layoutRowPills`/`pillArea`/`textColumnX`. *Files:* `src/graph/draw.ts`. *Gate:* harness — three
  zones, avatars, collapsed `main` w/ both glyphs, diverged split, `+N` chip; both themes; scroll
  sweep clean.
- **P7c — Hit-test parity + tooltips.** `GraphCanvas.handleContextMenu` LEFT-column resolution
  (§5, `targetRefOf`); `mouseXRef` + hover-target computation in `handleMouseMove`/`handleScroll`;
  tooltip state + overlay DOM + `useLayoutEffect` clamp; CSS `.graph-tooltip`. *Files:*
  `src/graph/GraphCanvas.tsx`, `src/styles.css`. *Gate:* harness — avatar/`+N` tooltips in the a11y
  tree; right-click parity from the left column for local/remote/collapsed/tag/HEAD/chip.
- **P7d — Fixtures + wiring.** `fixtures/graph.ts` (§9 row-0 collapse+overflow, diverged `feat`,
  single-word author), `fixtures/branches.ts` (`dev`,`origin/dev`,`origin/release`,`origin/feat` with
  `tip`s). *Files:* `src/ipc/fixtures/graph.ts`, `src/ipc/fixtures/branches.ts`. *Gate:* harness shows
  all P7 cases; every left-column ref menu resolves (non-empty where expected).

No feature flags. No backend build. Ship P7a→P7d; each is one fresh-context senior-dev pass.

---

## §12 Open items to FLAG for the orchestrator
1. **Fixed ref-band width (§1.1).** `refColWidth = 180` reserves left space even on ref-less rows
   (the GitKraken tradeoff). A dynamic width jitters the graph horizontally while scrolling → rejected.
   If 180 feels too wide/narrow in the harness, it's a single metric to tune — recommend confirming at
   P7b review.
2. **Remote-only labels now show the SHORT name (§3.2/§3.3).** `origin/release` renders as `release` +
   cloud (was full `origin/release`). This unifies the icon system ("short name once") but loses the
   remote prefix on screen; the right-click target still uses the full remote name. Multi-remote same
   short name is a rare edge → targets `remotes[0]`. Recommendation: ship short-name + cloud; fallback
   is to keep the full `origin/…` label for remote-only entities (trivial toggle in `entityStyle`).
3. **Icon art is canvas-vector, not emoji (§3.4).** Emoji on canvas render inconsistently across the
   harness browser and the native WebView2 → vector recipes chosen. Exact arc tuning for the cloud is
   at implementer discretion; reviewer eyeballs both glyphs in the harness.
4. **No TS test runner in the repo.** Pure-fn "unit tests" are exposed as `window.__bonsai.p7SelfTest()`
   (mock-only) so the orchestrator can verify determinism/grouping/overflow without new tooling.
   Alternative: add `vitest` (dev-dep + `test` script) for real unit tests — recommend the self-test
   for P7 to avoid tooling scope creep; revisit if TS unit tests become broadly needed.
5. **Collapsed-label right-click targets the LOCAL branch (§5).** Chosen because the local menu is the
   superset (Checkout/Merge/Rebase/Compare/Delete). If the user wants remote actions on a collapsed
   label, a submenu is a later enhancement.

---

## §13 P7e — post-checkpoint layout refinement (user feedback 2026-07-29)

User screenshots surfaced two overlap defects plus a sizing request. All three are **frontend-only**
(Rust and the wire types are untouched); they refine P7's render/geometry.

### §13.1 Ref-column `+n` chip must stay inside the band (fixes screenshot-1 overlap)
`layoutRefLabels` (draw.ts §4) currently appends the `+n` overflow chip **after** the greedy loop with
no budget check, so the chip spills past `startX + budget` and butts against the CENTER avatar.
Fix — the laid-out set (chip included) must satisfy `lastLaid.x + lastLaid.w ≤ startX + budget`:
- Lay pills greedily as today. If **all** entities fit (`hidden === 0`) → no chip, return unchanged.
- If `hidden > 0`, the chip is mandatory. Reserve room for it: while the trailing shown pill's right
  edge + `pillGap` + chip width would exceed `startX + budget`, **pop** the trailing shown pill
  (increment `hidden`, recompute the chip width for the new `+n` label). Place the chip at the freed
  cursor. Guarantee the chip fits even if it means showing zero pills (chip alone at `startX`).
- `layoutRefLabels` stays PURE and the single source of truth — draw pass + both hit-tests
  (`computeHoverTarget`, `handleContextMenu`) consume it, so parity is automatic.
- Self-test: extend `p7SelfTest` with an assertion that for an overflowing ref set the **last** laid
  label's right edge (`x + w`) is `≤ startX + budget`.

### §13.2 Reserve the vertical-scrollbar width on the right (fixes screenshot-2 overlap)
`.graph-scroll` is an `inset:0` overlay whose native vertical scrollbar (~17px on Windows) paints over
the full-width canvas's right edge, so the right-aligned relative-time (and potentially the summary)
render **under** the scrollbar. Fix:
- Add optional `rightInset?: number` (CSS px) to `Viewport`; default `0`.
- drawGraph pass 5 uses an effective right edge `vp.width - (vp.rightInset ?? 0)`:
  `dateRight = width - rightInset - colGap`, `dateLeft = dateRight - dateColWidth`,
  `summaryMax = dateLeft - colGap - sx`. (WIP row unaffected.)
- `GraphCanvas.paintNow` computes `rightInset = scroller.offsetWidth - scroller.clientWidth`
  (0 when no scrollbar present) and passes it in the viewport. Dynamic — no scrollbar ⇒ inset 0.

### §13.3 Slightly larger commit avatars ("a little bigger")
Grow the avatar and its rings, and grow `rowHeight` to preserve ring margins (all values in metrics.ts;
every consumer derives from `METRICS.rowHeight`, so the change is parametric):
`rowHeight 28→32`, `avatarRadius 8→10`, `avatarHeadRingRadius 10.5→12.5`, `avatarSelRingRadius 11.5→13.5`,
`avatarFont '600 9px'→'600 11px'` (`avatarBgRingExtra` stays 2 → halo r=12, just inside the head ring).
Outermost (selection) ring dia 27 in a 32px row ⇒ ~2.5px top/bottom margin (matches the pre-P7e feel).

*Gate:* `pnpm build` clean; `p7SelfTest` all-pass incl. the new chip-fit assertion; harness shows the
`+n` chip fully left of the avatar, relative-time clear of the scrollbar, and visibly larger avatars.
