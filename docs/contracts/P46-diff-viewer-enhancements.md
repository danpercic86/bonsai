# P46 — Diff viewer enhancements: split view, copy, auto-advance

Status: contract. Three independent frontend-only workstreams on the shared diff renderer.
No Rust / IPC / contract-shape changes — `FileDiff → Hunk[] → DiffLine[]`
([src/ipc/types.ts](../../src/ipc/types.ts)) already carries `kind` (`add|del|context`),
nullable `oldNo`/`newNo`, and `content`, which is all three features need.

Sequence (shared gutter-drag mechanism): **#3 auto-advance → #2 copy/selection → #1 split**.

---

## Workstream 3 — Auto-advance to the next file after staging

**Where:** `handleStage(paths)` in
[src/components/RepoWorkspace.tsx](../../src/components/RepoWorkspace.tsx) (~1063). Today: stage then
`await refetchStatus()`; the generic slot-remap (~590-614) collapses the open slot because the file
left its section.

**Behavior:**
1. Before staging, read current `status` (in scope) and `diffSlotRef.current`.
2. Build the visible changes order exactly as `StatusPanel` does:
   `[...status.unstaged, ...status.untracked]`, tagging each entry with its origin section
   ([StatusPanel.tsx:579-582](../../src/components/StatusPanel.tsx)).
3. Auto-advance **only** when the open slot is a workdir diff (`unstaged:`/`untracked:`) whose path is
   in `paths`. Capture the next entry in the merged list whose path is NOT in `paths` as
   `nextTarget = { section, path, origPath }`.
4. `await ipc.stage(...)`, `await refetchStatus()` (collapses the staged slot), then:
   - if `nextTarget` exists in the fresh snapshot's `unstaged`/`untracked`, open it via
     `fetchDiffSlot(\`${section}:${path}\`, () => ipc.getWorkdirFileDiff(repoId, path, origPath, false,
     diffViewModeRef.current === 'file'))` (reuse the pattern at ~604-611 and ~2634-2649);
   - else leave collapsed (diff closes).

**Constraints:** all changes stay inside `handleStage`; the shared `refetchStatus` (used by
discard/commit/refresh/watcher) is untouched. "Stage all" (many paths) finds no surviving next file →
collapses, which is correct. `handleUnstage` mirroring is OUT of scope.

**Mock:** `src/ipc/mock.ts` `stage` must move a file from `unstaged`/`untracked` into `staged` so the
harness can exercise auto-advance; verify/adjust.

**Acceptance:**
- Open file A (of ≥2 unstaged), stage A → overlay shows file B's diff.
- Stage the last remaining file → overlay closes.
- Stage a file via its row `+` while a *different* file is open → open diff is unchanged (path not in
  the open slot).
- Pure `nextFileAfter(changes, stagedPaths)` helper is unit-testable: middle → next; last → none;
  all-paths (stage-all) → none.

---

## Workstream 2 — Copy via native text selection

**Problem:** the interactive (working-dir) renderer calls `e.preventDefault()` on a **row-level**
pointerdown to own the drag for range-staging
([DiffView.tsx:166-173](../../src/components/DiffView.tsx)), which suppresses native text selection.
Read-only surfaces already support select + `Ctrl+C` (gutters/markers are `user-select: none` at
[styles.css:2545,2550](../../src/styles.css)).

**Change:** move the range-staging drag handle from the whole row onto the **line-number gutter
cells** (`.diff-lineno`):
- Remove the row-level `onPointerDown`/`preventDefault` so content pointerdown does nothing custom →
  native selection over `.diff-content`.
- Attach `onPointerDown` (anchor + `preventDefault`) to the `.diff-lineno` spans (already
  `user-select: none`).
- Keep `onPointerEnter` on the row to extend the range, still gated on `draggingRef` (now only armed
  by a gutter pointerdown; a pure content drag never arms it).
- Same gutter-as-handle pattern in the split renderer (WS1).

**CSS:** `.diff-lineno` gets a `cursor: row-resize` (drag-handle affordance); confirm `.diff-content`
has no `user-select` restriction. Optional: a `keydown` `Ctrl+C` handler copying the active staging
range's line contents when the browser has no text selection — only if cheap (reuse `pushToast` for a
"Copied" confirmation).

**Acceptance:**
- Working-dir diff: drag over code → native selection; `Ctrl+C` yields code text with no line numbers
  or `+/-` markers.
- Working-dir diff: drag on the line-number gutter → still selects lines for staging (float appears).
- Per-line `+`/`−`/`×` gutter buttons and per-hunk buttons still work.
- Read-only diffs (commit/compare/`DiffBrowser`) unchanged.

---

## Workstream 1 — Interactive side-by-side (split) view

**Toggle:** widen the view-mode union `'diff' | 'file'` → `'diff' | 'file' | 'split'` and add a
**"Split"** button to the toggle group ([DiffOverlay.tsx:227-244](../../src/components/DiffOverlay.tsx)).
Thread the widened type through `DiffView`/`DiffSlotView` (18,351), `DiffOverlayProps` (166-167),
`WorkspaceGraphPane` (48-49), and `diffViewMode` state
([RepoWorkspace.tsx:343](../../src/components/RepoWorkspace.tsx)). The fetch gates on
`viewMode === 'file'` (610,2646) so `'split'` fetches 3-context hunks like `'diff'` — no fetch change.
Leave `DiffBrowser` on its narrower `'diff' | 'file'` (scope split to the overlay).

**Layout — new `viewMode === 'split'` branch.** Pure helper `pairSplitRows(hunk)`:
- context line → one paired row (old cell + new cell, same content, both line numbers);
- collect consecutive `del` into a left buffer and consecutive `add` into a right buffer, then emit
  `max(dels, adds)` rows pairing `del[i]`↔`add[i]`; surplus dels → left-only (empty right filler),
  surplus adds → right-only (empty left filler).

Each split row is a two-side grid — left `oldNo | marker(−) | old content`, right
`newNo | marker(+) | new content` — `.diff-line-del` tint on the left change cell, `.diff-line-add` on
the right. Keep the per-hunk `@@` header with its Stage/Discard-hunk buttons.

**Interactive staging (reuse):** keep the flat `rows` list (unified order) as the selection domain and
the existing `range`/`changedInRange`/`commitRange`/float machinery
([DiffView.tsx:89-192,269-292](../../src/components/DiffView.tsx)) unchanged. Each split cell carries
`data-g` = the global index of its `DiffLine`; gutter-drag anchors/extends on those indices. Because
unified order groups dels-then-adds within a run, a `[lo..hi]` range captures a contiguous change block
spanning both columns → `onStageLines`/`onStageHunk`/`onDiscardLines`/`onDiscardHunk` and the per-cell
`+`/`−`/`×` buttons reuse the same `toSelection(line)` calls (212-249).

**File-size discipline (CLAUDE.md ~500-line soft limit):** extract the split renderer + `pairSplitRows`
into their own module (e.g. `src/components/DiffViewSplit.tsx` + a helper file) rather than growing
`DiffView.tsx`. Keep `pairSplitRows` a pure export for unit tests.

**CSS** ([src/styles.css](../../src/styles.css) near the `.diff-*` block ~2468-2706): `.diff-view-split`
(two-side grid + center divider), `.diff-split-row`, left/right cell classes, per-cell add/del tints,
filler-cell styling, `.diff-line-selected` adapted to cells.

**Acceptance:**
- Split shows two aligned columns (old left / new right); changed lines paired row-for-row with correct
  del-left / add-right tinting; unequal runs get filler cells.
- Per-cell `+` stages one line; gutter-drag across cells shows "Stage N lines" and stages the range.
- Toggling File / Diff / Split preserves the open file; `Ctrl+C` selection still works in split.
- `pairSplitRows` unit-tested: context passthrough; even del/add pairing; surplus-del and surplus-add
  filler placement; empty hunk.

---

## Workstream 1 — module & prop boundary (design)

Concrete implementation plan for the split view. The unified renderer keeps ownership of ALL selection
state; the split file is pure presentation. Two new files, targeted edits to five existing files.

### New files

**1. `src/utils/splitRows.ts`** — pure, unit-testable pairing. No React, no CSS.

```ts
import type { DiffLine, Hunk } from '../ipc';

/** One row of the side-by-side view. `left` = OLD-side cell, `right` = NEW-side cell.
 *  `null` = filler (empty cell). Both-null never occurs. For a context line the SAME
 *  DiffLine object is placed in both cells (one global index; identity preserved). */
export interface SplitRow {
  left: DiffLine | null;
  right: DiffLine | null;
}

/**
 * Pair a hunk's unified lines into side-by-side rows.
 * Rules:
 *  - `context`  → flush the pending change block, then push `{ left: line, right: line }`
 *    (same object reference in both cells).
 *  - `del`      → buffer into `dels`.
 *  - `add`      → buffer into `adds`.
 *  - At each context boundary AND at end-of-hunk, flush: emit `max(dels,adds)` rows,
 *    `{ left: dels[i] ?? null, right: adds[i] ?? null }`. Surplus dels → `{left, right:null}`,
 *    surplus adds → `{left:null, right}`.
 *  - Empty hunk (`hunk.lines.length === 0`) → returns `[]`.
 * The returned cells reference the EXACT `hunk.lines[*]` objects (no copies), so
 * `globalIndexByLine.get(cell)` resolves via object identity.
 */
export function pairSplitRows(hunk: Hunk): SplitRow[];
```

Reference body shape (buffer + flush at every context boundary and once at the end):

```
out = []; dels = []; adds = [];
flush(): n = max(dels.length, adds.length);
         for i in 0..n: out.push({ left: dels[i] ?? null, right: adds[i] ?? null });
         dels = []; adds = [];
for line in hunk.lines:
    if line.kind === 'del' → dels.push(line)
    else if line.kind === 'add' → adds.push(line)
    else (context) → flush(); out.push({ left: line, right: line })
flush(); return out;
```

**2. `src/components/DiffViewSplit.tsx`** — presentational split renderer. Owns NO state; receives the
selection domain + callbacks from `DiffView`.

```ts
import type React from 'react';
import type { DiffLine, Hunk, LineSelection } from '../ipc';

export interface DiffViewSplitProps {
  /** The diff's hunks (same objects DiffView's flat `rows` were built from). */
  hunks: Hunk[];
  /** DiffLine → global index in DiffView's flat unified `rows`, by object identity.
   *  Used for BOTH the `data-g` attribute and the per-cell selection test. */
  globalIndexByLine: Map<DiffLine, number>;
  /** Current unified-order selection bounds [lo..hi] inclusive, or null. */
  selectedBounds: { lo: number; hi: number } | null;
  /** stageable !== null (App gates working-dir diffs). Read-only when false. */
  interactive: boolean;
  /** Direction of a granular action; drives marker glyphs + button labels. */
  stageable: null | 'stage' | 'unstage';
  /** stageable === 'stage' && onDiscardLines wired (widens the marker column, adds ×). */
  discardable: boolean;
  /** highlight.js per-line renderer (HTML string) or null; DiffView's `highlight`. */
  highlight: ((text: string) => string | null) | null;
  /** Gutter (`.diff-lineno`) pointerdown → anchor the drag at global index g.
   *  DiffView passes its existing `onRowPointerDown` unchanged. */
  onGutterPointerDown(e: React.PointerEvent<HTMLElement>, g: number): void;
  /** Cell pointerenter (while dragging) → extend the range to global index g.
   *  DiffView passes its existing `onRowPointerEnter` unchanged. */
  onRowPointerEnter(e: React.PointerEvent<HTMLElement>, g: number): void;
  /** Per-cell `+`/`−` stages/unstages exactly one line (DiffView.onStageLines). */
  onStageLines(selection: LineSelection[]): void;
  /** Per-cell `×` discards one line (DiffView.onDiscardLines); only when discardable. */
  onDiscardLines?(selection: LineSelection[]): void;
  /** Per-hunk header Stage/Unstage button. */
  onStageHunk(hunkIndex: number): void;
  /** Per-hunk header Discard button (unstaged tracked only). */
  onDiscardHunk?(hunkIndex: number): void;
}

export function DiffViewSplit(props: DiffViewSplitProps): JSX.Element;
```

`DiffViewSplit` imports the shared `toSelection(line: DiffLine): LineSelection` — promote DiffView's
current module-private `toSelection` (DiffView.tsx:58) to an **exported** function so both files share
one source of truth (per-cell buttons build `[toSelection(line)]`). Do NOT duplicate it.

**Render structure (per hunk):** the `@@` header (`.diff-hunk-header`, reused verbatim with
Stage-hunk/Discard-hunk buttons), then `pairSplitRows(hunk).map(splitRow)`. Each `splitRow` is a
`.diff-split-row` containing exactly two cells rendered by a shared `cell(side, line)` helper:

- `line === null` → `<div className="diff-split-cell diff-split-filler" />` (no gutter/marker/content,
  no handlers).
- otherwise, `g = globalIndexByLine.get(line)`; `selected = selectedBounds !== null && g !== undefined
  && g >= selectedBounds.lo && g <= selectedBounds.hi`; classes:
  `diff-split-cell diff-split-cell-{side}` + (`diff-line-del` if `left`+del / `diff-line-add` if
  `right`+add) + (`diff-line-selected` if `selected`). Attributes/children:
  - `data-g={g}`.
  - `onPointerEnter={interactive ? (e) => onRowPointerEnter(e, g) : undefined}` (per-cell, so the drag
    extends to the column under the cursor).
  - `<span className="diff-lineno" onPointerDown={interactive ? (e) => onGutterPointerDown(e, g) :
    undefined}>{side==='left' ? (line.oldNo ?? '') : (line.newNo ?? '')}</span>` — the gutter is the
    drag handle (WS2), reusing `.diff-lineno` (`user-select:none`, `cursor:row-resize`).
  - `<span className="diff-marker">…</span>` — same button logic as `lineRow` (DiffView.tsx:229-268):
    an interactive changed cell shows the `+`/`−` `.diff-gutter-btn` (`onClick` →
    `onStageLines([toSelection(line)])`, `onPointerDown` stopPropagation) and, when `discardable`, the
    `×` `.diff-gutter-discard-btn` (→ `onDiscardLines?.([toSelection(line)])`); a non-interactive
    changed cell shows the static `+`/`−` glyph; context/filler show blank.
  - content span: `highlight` HTML via `dangerouslySetInnerHTML` else plain `{line.content}`
    (`.diff-split-content`). When `line.noNewline === true`, append a `.diff-split-nonewline` note
    (`\ No newline at end of file`).

### DiffView changes (state stays here)

1. **Export `toSelection`** (change `function toSelection` → `export function toSelection`).
2. **New derived value** beside `globalIndexOf` (DiffView.tsx:94-98):

   ```ts
   const globalIndexByLine = useMemo(() => {
     const m = new Map<DiffLine, number>();
     rows.forEach((r, g) => m.set(r.line, g));
     return m;
   }, [rows]);
   ```

   (Object-identity keyed; context lines appear once in `rows` → once here.)
3. **Generalize the two float-anchor `closest()` selectors** in `onRowPointerDown` and
   `onRowPointerEnter` (DiffView.tsx:176,182) from `closest('.diff-line')` to
   `closest('.diff-line, .diff-split-row')` so `floatTop` resolves in split too (`.diff-split-row` is a
   child of the `position:relative` container → correct `offsetTop`). No other handler change; rename
   `onRowPointerDown` → `onGutterPointerDown` is OPTIONAL (kept-as-is is fine; it is passed as the
   `onGutterPointerDown` prop).
4. **New `viewMode === 'split'` branch** (before the final `'diff'` return), sibling-composing the float
   button so it reuses DiffView's `floatButton`/`floatTop`/`commitRange` unchanged:

   ```tsx
   if (viewMode === 'split') {
     return (
       <div
         className={`diff-view diff-view-split${discardable ? ' diff-view--discardable' : ''}`}
         ref={containerRef}
       >
         <DiffViewSplit
           hunks={diff.hunks}
           globalIndexByLine={globalIndexByLine}
           selectedBounds={selectedBounds}
           interactive={interactive}
           stageable={stageable}
           discardable={discardable}
           highlight={highlight}
           onGutterPointerDown={onRowPointerDown}
           onRowPointerEnter={onRowPointerEnter}
           onStageLines={(s) => onStageLines?.(s)}
           onDiscardLines={onDiscardLines}
           onStageHunk={(i) => onStageHunk?.(i)}
           onDiscardHunk={onDiscardHunk}
         />
         {floatButton}
       </div>
     );
   }
   ```

   `range`/`changedInRange`/`selectedBounds`/`floatButton`/`commitRange`/`commitDiscardRange` and the
   reset effect (`[diff, viewMode, interactive]`) are UNCHANGED — split reuses them all. Because the
   drag lands on a `.diff-lineno` inside a `.diff-split-cell` whose `data-g` is the DiffLine's global
   index, the existing `[lo..hi]` path drives per-cell `.diff-line-selected` and staging with zero
   changes to `onStageLines`/`onStageHunk`/`onDiscardLines`/`onDiscardHunk`.

### Float button placement (design decision)

**Keep the float button owned by `DiffView`** (recommended). It depends on DiffView-only state
(`changedInRange`, `floatTop`, `stageable`, `commitRange`, `commitDiscardRange`); rendering it as a
sibling of `<DiffViewSplit>` inside the shared `position:relative` `.diff-view` container needs zero new
plumbing. `floatTop` is set by the same `onGutterPointerDown`/`onRowPointerEnter` handlers via the
widened `closest('.diff-line, .diff-split-row')` lookup. Do NOT render a second float inside
`DiffViewSplit`.

### CSS — `src/styles.css` (near the `.diff-*` block ~2468-2706)

New rules (append after the existing split-relevant block):

- `.diff-view-split` — the container reuses `.diff-view` (already `position:relative; overflow-x:auto`);
  this modifier only sets `display:block` semantics for the stacked rows/headers.
- `.diff-split-row` — `display:grid; grid-template-columns:1fr 1fr; width:max-content; min-width:100%`.
- `.diff-split-cell` — inner grid `grid-template-columns:40px 16px auto` (lineno | marker | content),
  `white-space:pre`. `.diff-split-cell-left` gets the **center divider**
  (`border-right:1px solid var(--border)`).
- `.diff-split-content` — reuse `.diff-content` colour rules; add `overflow-x:auto` per cell (long-line
  handling — see RISK below).
- Per-cell tints: reuse `.diff-line-add` / `.diff-line-del` (already background-color) by applying those
  classes to the CELL; no new colour rules needed. `.diff-line-selected` (background-IMAGE gradient)
  composes over the tint on the cell exactly as it does on `.diff-line`.
- `.diff-split-filler` — an empty cell with a faint hatched/`--bg-2` background to read as "no
  corresponding line".
- `.diff-split-nonewline` — italic `--text-3` note (mirror `.diff-nonewline .diff-content`).
- Discardable marker widening (mirror the unified `--discardable` rule at 2646-2654):
  `.diff-view-split.diff-view--discardable .diff-split-cell { grid-template-columns:40px 34px auto; }`
  and flex the marker (`display:flex; align-items:center; justify-content:center; gap:2px`).
- Reused verbatim inside cells (no new rules): `.diff-lineno` (drag handle, `user-select:none`,
  `cursor:row-resize`), `.diff-marker` (`user-select:none`), `.diff-gutter-btn`,
  `.diff-gutter-discard-btn`, `.diff-hunk-header`, `.diff-hunk-stage-btn`, `.diff-hunk-discard-btn`.

Gutters and markers stay `user-select:none`; only `.diff-split-content` is selectable → `Ctrl+C` yields
clean code (WS2 invariant holds per column).

### Type-threading checklist (`'diff' | 'file'` → `'diff' | 'file' | 'split'`)

- [ ] `DiffViewProps.viewMode` — DiffView.tsx:18 (default `= 'diff'` param at :68 unchanged).
- [ ] `DiffSlotViewProps.viewMode` — DiffView.tsx:369 (forwarded to `<DiffView>` at :419).
- [ ] `DiffOverlayProps.viewMode` and `onSetViewMode(m: …)` — DiffOverlay.tsx:166-167.
- [ ] DiffOverlay toggle group (DiffOverlay.tsx:227-244) — add a third **"Split"** `<button>`:
      `className={viewMode === 'split' ? 'active' : ''}`, `aria-pressed={viewMode === 'split'}`,
      `onClick={() => onSetViewMode('split')}`.
- [ ] `WorkspaceGraphPane.diffViewMode` — WorkspaceGraphPane.tsx:48 (`onSetViewMode` at :49 references
      `DiffOverlayProps['onSetViewMode']` → auto-threads).
- [ ] `RepoWorkspace.diffViewMode` state + `diffViewModeRef` — RepoWorkspace.tsx:344-345.
- [ ] `handleSetViewMode` param `m: 'diff' | 'file'` — RepoWorkspace.tsx:1304 (widen only).
- [ ] Fetch gates are EXPRESSIONS that stay correct with no logic change (split ≠ 'file' → `fullContext
      = false` → 3-context hunks, same as 'diff'): `diffViewModeRef.current === 'file'`
      (RepoWorkspace.tsx:616, 1114), `m === 'file'` (:1312), `diffViewMode === 'file'` (:2697). Confirm
      no `=== 'diff'` / `!== 'file'` gate elsewhere excludes split.
- [ ] `DiffBrowser` stays on the narrower `'diff' | 'file'` (DiffBrowser.tsx:350,408) — OUT of scope;
      it never offers Split.
- [ ] `src/ipc/mock.ts` — no shape change (split reuses the same 3-context `getWorkdirFileDiff` result);
      verify no mock branch hard-asserts `viewMode` is only `'diff' | 'file'`.

### Acceptance criteria (extends the Workstream 1 list above)

- `pairSplitRows` is a pure export with the unit cases below all green.
- Split branch renders `.diff-split-row`s with the left divider; del cells tint left, add cells tint
  right; unequal runs show `.diff-split-filler` on the short side.
- Per-cell `+`/`−` stages/unstages one line; per-cell `×` discards one line (unstaged tracked only);
  per-hunk header buttons act on the whole hunk — all via the unchanged DiffView callbacks.
- Gutter-drag down a column (and across to the other) shows the reused "Stage N lines"/"Discard N
  lines" float at the dragged row and stages/discards the contiguous change block.
- Native selection over `.diff-split-content` → `Ctrl+C` copies code only (no line numbers/markers).
- Toggling File / Diff / Split keeps the same file open (same slot key; only `fullContext` differs, and
  split == diff there → no visible reload). Escape clears an active range before closing the overlay.
- `tsc`/build clean; DiffView.tsx stays under the ~500-line soft limit (split renderer lives in its own
  file).

### Unit tests — `src/utils/splitRows.test.ts` (`pairSplitRows`)

Build `Hunk`s inline from `DiffLine` literals; assert length, per-cell `kind`, and **object identity**.

1. **Empty hunk** — `lines: []` → `[]`.
2. **Context passthrough** — 3 context lines → 3 rows, each `left === right ===` the source line
   (identity), content on both sides.
3. **Even del/add run** — 2 del + 2 add → 2 rows: `{left:del0,right:add0}`, `{left:del1,right:add1}`
   (identity per cell).
4. **Surplus dels** — 3 del + 1 add → `{del0,add0}`, `{del1,null}`, `{del2,null}`.
5. **Surplus adds** — 1 del + 3 add → `{del0,add0}`, `{null,add1}`, `{null,add2}`.
6. **Pure deletions** — N del, 0 add → N rows `{del[i], null}`.
7. **Pure additions** — 0 del, N add → N rows `{null, add[i]}`.
8. **Flush at context boundary** — `[ctx, del, add, ctx]` → `[{ctx,ctx},{del,add},{ctx,ctx}]`; the
   trailing/leading contexts do NOT merge into the change run.
9. **Two separated runs** — `[del, add, ctx, del, add]` → 3 rows; the two runs are flushed independently
   (no cross-context pairing).
10. **Identity for global lookup** — every non-null cell `===` the exact `hunk.lines[*]` object (so a
    `globalIndexByLine` built from the same objects resolves each cell).

### RISK / flag for orchestrator

- **Long-line horizontal scroll in two columns.** Unified view uses one `overflow-x:auto` container.
  Split has two independent code columns; per-cell `overflow-x:auto` yields up-to-two scrollbars and
  columns can scroll out of sync. Recommendation for this increment: per-cell `overflow-x:auto`
  (simplest, correct clipping) and defer synchronized horizontal scroll to a follow-up. Flagging in
  case the orchestrator wants synced scroll now.
- **Context rows are non-actionable but still selectable/highlightable.** Since a context line has one
  global index shared by both cells, dragging across a context row highlights both cells and the row is
  inside `[lo..hi]`; `changedInRange` already drops context, so staging is unaffected — behavior matches
  the unified renderer. No action needed; noted for reviewer awareness.
