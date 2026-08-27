# UI Contract — Spec-005: Graph Overview Rail (match ticks + on-demand minimap)

**Inputs:** `docs/specs/005-graph-rail-minimap/spec.md` + `plan.md` (mechanics authoritative:
one `OverviewRail` in GraphCanvas, `drawRail.ts` painter, `RAIL_BUCKETS=1024`, 300 ms linger,
min-24px thumb, `graphMinimapAlwaysShow`, rings-channel parity confirmed).
This contract owns visuals, states, copy, motion, a11y. No new tokens.

## 1. Placement & geometry

```
┌ graph pane ──────────────────────────────────────────────┐
│                                    [filter fab][search ▣]│ ← top:10, right:52 / right:14
│  ref band │ lanes/avatars │ summary …          ║R║ ▐sb▌  │
│                                                ║a║       │   R = rail, 14px, full height,
│                                                ║i║       │       absolute, right: rightInset
│                                                ║l║       │  sb = native scrollbar (never covered)
└──────────────────────────────────────────────────────────┘
```

- Rail: `position:absolute; top:0; bottom:0; right:<rightInset>px; width:14px` inside
  `.graph-canvas-host`. `rightInset` = live `offsetWidth − clientWidth` GraphCanvas already
  computes — the rail hugs the scrollbar's inner edge and never overlaps it (spec AC7).
- One HiDPI `<canvas class="graph-rail">` (backing store × devicePixelRatio; all px below are
  CSS px). Full pane height in **both** densities — rail geometry is density-invariant; all
  row↔y mapping uses live scroller metrics, so cozy/compact need no separate numbers.
- **Right-edge overlay stack (canon defined here; to be folded into ui-reference as §4.2 —
  see report flag):** native scrollbar (never covered) → rail `z-index: 4` → fabs/chip
  `z-index: 5`. The search fab (right 14–44 px) and filter fab/chip (right 52–82 px) sit at
  top 10–40 px; the fab band partially overlaps the rail's top ~40 px and **wins pointer
  events** there. Accepted occlusion: the rail's top maps to the newest rows, which are almost
  always already on screen. Do NOT shorten the rail — full height keeps the mapping linear.
- `.graph-truncated-banner` is a flex block *above* the scroller (margin row, not an overlay);
  the rail lives inside the scroller host below it — no interaction, unaffected.

## 2. Component decomposition (mirrors plan; no changes)

- `src/graph/rail/OverviewRail.tsx` — mount/lifecycle, pointer, linger timer. `aria-hidden`.
- `src/graph/rail/drawRail.ts` — painter (this contract §3 is its spec).
- `src/graph/rail/railMath.ts` — pure math (plan-owned).
- `src/components/settings/SettingsGraphOverviewSection.tsx` — NEW small section file (§6).
  `SettingsGraphDeclutterSection` is semantically wrong for this row and files are never
  appended-to; nothing existing fits a right-edge minimap, hence the new component set.
- `src/styles/graph-rail.css` — NEW style file (positioning, fade, cursor classes); do not grow
  `graph-filter.css` or `styles.css`.

## 3. Rail anatomy (paint order = z-order, bottom→top)

All colors are existing tokens read via the resolved theme (same `resolveTheme` path as
`draw.ts`); nothing hardcoded except canon constants already fixed app-wide (`#d4a72c` TAG_COLOR).

1. **Track.** Fill `color-mix(in srgb, var(--bg-1) 92%, transparent)` over the graph bg, plus a
   1px left border `var(--border)`. Reads as a quiet gutter in both themes; graph edges remain
   faintly visible through it.
2. **Density strip** (minimap layer). Per rail pixel → nearest bucket:
   - Horizontal bar anchored at the track's **left** edge, width `4 + 2·laneMax` px, clamped to
     12 px (2 px right margin keeps the strip from touching the scrollbar edge).
   - Fill `var(--text-3)` at alpha ramped by density: floor **0.22** for any non-empty bucket,
     linear to **0.55** at the layout's max bucket density. The floor guarantees sparse history
     is still visible; the neutral hue keeps the strip legible under **both graph styles**
     (classic and Bonsai reskin) and both themes — lane colors are deliberately not used here.
   - Empty buckets paint nothing (short histories read as a sparse strip, correct).
3. **Ref/HEAD pips** (minimap layer). 3px-diameter dots per flagged bucket:
   - Branch/remote: `var(--text-2)` dot, **left column** (center x = 4 px).
   - Tag: `#d4a72c` dot (TAG_COLOR canon, ui-reference §6), **right column** (center x = 10 px).
   - HEAD: full-width 2 px bar in `var(--text-1)` — same hue as the §4 HEAD ring on the node.
   - The left/right/full-width **position split is the non-color carrier** (house rule: color
     never sole meaning) — kind stays distinguishable at 3 px even for color-blind users.
4. **Viewport thumb** (minimap layer). Rounded rect (radius 4), full rail width minus 1 px each
   side, min-height 24 px: fill `color-mix(in srgb, var(--accent) 14%, transparent)`, 1px border
   `var(--accent)` (accent 1px border = 4.4:1 dark / 4.1:1 light per ui-reference §3 — passes the
   3:1 graphics bar). While dragging (`.graph-rail--dragging`): fill alpha 22%, border 1.5 px.
5. **Match ticks** (rings layer; drawn only when a rings channel is live). Full-width 14×2 px
   bars in `var(--match-ring)` — one per occupied pixel (coalesced; density beyond presence is
   NOT encoded, browser-find precedent). Contrast vs graph bg: `#ff4dd2` on `#16181d` ≈ **6.2:1**
   dark; `#c026a3` on `#ffffff` ≈ **5.2:1** light — both clear 3:1 with margin, incl. through
   the translucent track.
6. **Current-match tick** — painted last: 14×3 px `var(--match-ring)` bar with a 1px
   `var(--bg-0)` halo above/below (halo is the "current" carrier in addition to size, so it
   survives sitting inside a dense cluster). **historySearch channel has no current tick**
   (`currentMatchRow` is null there — implementer trap; do not synthesize one).

Layers combine freely: ticks overlay thumb overlays pips overlays density.

## 4. Visibility, motion, states

`visible = ringsLive || alwaysShow || hoverRevealed || dragging`; when false the component is
**unmounted** (plan's zero-idle-cost guarantee) — there is no CSS "hidden" state.

- **Reveal:** on mount, fade `opacity 0 → 1`, **120 ms ease-out, opacity only**. Nothing else
  animates (no slide — a transform near the scrollbar edge reads as layout shift).
- **Hide:** after the 300 ms hover linger, **instant unmount — no fade-out.** A fade-out would
  couple a transition to unmount and break "hidden ⇒ nothing mounted"; forbidden.
- **Reduced motion:** add `.graph-rail { transition: none; }` to the existing
  `src/styles/reduced-motion.css` block (the app has no other transition-kill mechanism — the
  reduced-motion file is the only lock; no theme/density no-transition class exists to respect).
- Thumb tracking scroll and tick updates repaint per rAF with no easing — position is truth,
  never animated.

State table:

| State | Behavior |
|---|---|
| Hidden (default) | Not mounted. No DOM, no listeners. |
| Hover-revealed | Pointer within 20 px of the scroller's outer right edge → mount + fade-in; leaves zone → 300 ms linger → unmount (never mid-drag). |
| Search-forced | Commit search open with ≥1 visible match ⇒ rail stays mounted regardless of hover; ticks layer on. 0 matches ⇒ no ticks; if search open with 0 matches and no hover/always-show, rail is hidden (ringsLive false). |
| historySearch (Ask) | Same as search-forced, ticks mirror the live rings channel; **no current tick**; tick click resolves via reveal-by-oid. |
| Always-show | Mounted permanently; no fade on app load (initial mount renders at opacity 1 — fade is for pointer-triggered reveals only). |
| Dragging thumb | `cursor: grabbing`, thumb active style (§3.4); rail pinned visible even if pointer exits zone. |
| Streaming | Previous generation's buckets keep painting; ticks/thumb live. No spinner, no shimmer — the rail is an orientation aid, silent while loading. First load before any `done`: track + thumb + ticks only (no density/pips yet). |
| Short history | Thumb spans most/all of the rail (falls out of the min-height clamp); ticks/pips land proportionally. No special casing. |
| Huge match counts | Coalesced to ≤ railHeight ticks; a saturated rail reads as a solid `--match-ring` band — acceptable and informative. |
| Empty repo / no layout | Rail never mounts (`displayRowCount` 0 guard). |

## 5. Pointer, cursors, a11y

- Cursors: `grab` over the thumb, `grabbing` while dragging, `pointer` within ±3 px of a tick,
  `default` elsewhere on the rail.
- Click tick → jump + select that match; click elsewhere → center viewport there; drag thumb →
  scroll (pointer capture, grab offset preserved); wheel over rail → forwarded to scroller
  (no dead zone). All per plan.
- **Hit-target deviation (explicit):** ticks are 14×2 px with ±3 px slop — under the house
  ≥24 px minimum. Sanctioned here because the rail is an `aria-hidden` **redundant pointer
  affordance** (browser-find-scrollbar precedent): every jump has a first-class keyboard path —
  Enter/F3 next match, Shift+F3 previous, the focusable results list, and grid navigation
  (arrows/PageUp/PageDown/Home/End) for viewport movement. Do not generalize this deviation.
- The canvas is `aria-hidden="true"`, `tabIndex` unset (not focusable). No roles, no live
  region, no new keyboard surface, no command-palette entry for the rail itself. The settings
  toggle (§6) is the standard keyboard-accessible switch row and IS palette-reachable via the
  existing Settings search.
- No tooltip on the rail (a hover tooltip would fight the hover-reveal lifecycle); the thumb and
  ticks are self-describing by position.

## 6. Settings row — Commit graph category

Placement: **Commit graph** category (not Appearance — keeps all graph toggles in one place),
new group **"Overview"** rendered after the Declutter group, in the new
`SettingsGraphOverviewSection.tsx`, standard §12.2 switch-row recipe (label `--text-1` 13px,
help `--text-3` 12px, `.settings-switch` track, focus per §12.2).

- Label: `Always show overview rail`
- Help: `Keep the minimap of the whole history visible along the graph's right edge. When off,
  it appears on hover or while search is open.`
- Default off; bound to `graphMinimapAlwaysShow`.

## 7. Harness / fixture states (VITE_MOCK_IPC=1)

1. 20k-commit fixture — density strip shape, thumb proportion, hover-reveal at the right edge.
2. Short-history fixture (< 40 commits) — thumb spans rail, sparse strip.
3. Search with a common term (hundreds–thousands of matches) — coalesced ticks, current-tick
   emphasis on F3 stepping.
4. historySearch (Ask) rings channel — ticks present, no current tick.
5. Always-show toggle round-trip across reload (mock persistence).
6. Both themes × both graph styles (classic/Bonsai) — neutral strip must read on both.
- **USER CHECKPOINT:** reveal/scroll *feel* and drag smoothness on 20k+ (headless harness pauses
  rAF; AC6).

## 8. Explicit non-goals / guards

- No new tokens; no lane colors in the rail; no density-count labels; no context menu; no
  animation of thumb/tick positions; no fade-out; no rail in compact-only or cozy-only variants.
