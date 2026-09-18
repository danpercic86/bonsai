# Bonsai Graph Theme (organic commit-graph reskin)

**Status:** done (implemented; USER CHECKPOINT verified 2026-08-27 by the user in `3a71951`)
**Created:** 2026-08-26

## Problem
Bonsai is named and branded around a bonsai tree, but its centerpiece — the commit graph — looks
like every other GitKraken-style lane graph: colored lanes, round dots, bezier edges. There's no
moment where the product's identity shows up in the thing users stare at most. A selectable
"Bonsai" theme would make the graph *look* like a living tree — bark-colored branches, leaf/blossom
commit nodes, an earthy palette — turning the app's signature name into a signature visual, at no
cost to the graph's speed or correctness.

## Goals
- Add a **Bonsai** graph theme the user can turn on and off, alongside the existing look.
- Make the graph read as organic/tree-like: branch edges that taper (thicker toward the trunk,
  thinner toward the tips), commit nodes styled as leaves/buds/blossoms, an earthy bark-and-foliage
  palette, and a soft paper/pot backdrop behind the canvas.
- Preserve **every** existing graph behavior unchanged: topological-then-date ordering, stable
  per-lane colors while scrolling, ref pills (branches/remotes/tags/HEAD), selection, and smooth
  virtualized scrolling over 20k+ commits without new jank.
- Keep lane/branch topology **identical** to the current graph — this is a reskin, not a re-layout.
  Whatever the standard theme shows for a given repo, the Bonsai theme shows the same structure,
  just dressed differently.
- Respect light and dark app modes so the theme is legible in both — Bonsai is a **third theme
  with its own light and dark palettes**, not a single fixed look.
- Include organic **flourishes**: a blossom/accent treatment on HEAD (and/or selected) nodes,
  optional seasonal palette variants, and subtle motion (e.g. a gentle sway) — all subject to the
  app's reduced-motion / a11y conventions.

## Non-goals
- **Version B — a genuine tree-growth layout** (trunk-at-root, canopy spreading outward, a new
  Rust layout algorithm). Explicitly out of scope here; noted as possible future, lower-probability
  work. It would change topology/ordering and collide with locked v1 layout decisions, so it must
  go through the full milestone loop, not this spec.
- No change to what data crosses from backend to frontend (no new topology/position data).
- No change to the non-graph panels (sidebar, status/diff panel) beyond whatever backdrop framing
  the graph area needs.
- No new commit-graph interactions (hover tooltips, animations on commit, etc.) beyond styling.
- No performance *improvement* work; the bar is "no regression."

## User-facing behavior
- The user can select the **Bonsai** graph theme from the app's **settings / appearance surface**,
  alongside the existing theme controls. When selected, the commit graph re-renders in the organic
  style; when deselected, it returns to the standard look. The choice persists across app restarts
  like other appearance preferences.
- Bonsai has its own **light and dark variants**; following the app's light/dark mode keeps it
  legible in both.
- Flourishes are part of this pass: a blossom/accent node treatment for HEAD (and/or the selected
  commit), one or more optional **seasonal palette** variants, and a subtle organic **sway** — all
  disabled/toned down under reduced-motion, and none of which alter topology or scroll performance.
- In the Bonsai theme the graph shows: edges drawn as tapering bark-colored branches, commit nodes
  as leaves/buds (with a distinct blossom/accent treatment for HEAD and/or the selected commit),
  an earthy foliage-and-bark lane palette that still gives adjacent lanes distinguishable colors,
  and a soft paper/pot backdrop behind the graph canvas.
- Ref pills, commit summaries, the selected-row highlight, and all other graph affordances keep
  their existing placement and meaning, restyled only enough to sit legibly on the new backdrop.
- The theme looks intentional in both light and dark app modes.
- Everything the graph does today — click to select a commit, scroll a large history, see HEAD and
  refs, stable colors while scrolling — works identically.
- Exact visual language (leaf shapes, blossom treatment, taper curve, palette hues, backdrop
  texture, and where the selector lives) is owned by `ui-designer` and will be pinned in a UI
  contract before implementation; this spec fixes intent and acceptance, not pixels.

## Acceptance criteria
1. Given any repo open, when the user selects the Bonsai theme, then the commit graph re-renders in
   the organic style with the same nodes, lanes, edges, and ref pills as the standard theme (same
   topology, same ordering, same ref labels) — only the styling differs.
2. Given the Bonsai theme is active, when the user deselects it, then the graph returns to the exact
   standard appearance with no residual styling.
3. Given the Bonsai theme was selected, when the app is restarted, then the Bonsai theme is still
   active.
4. Given the Bonsai theme is active, when the user scrolls a large history (target: 20k+ commits),
   then scrolling is smooth with no measurable frame-rate regression versus the standard theme, and
   lane colors stay stable (no re-coloring while scrolling).
5. Given the Bonsai theme is active, when adjacent branch lanes are drawn, then their colors are
   visually distinguishable from each other in both light and dark app modes.
6. Given the Bonsai theme is active, when a commit is HEAD and/or selected, then its node is
   visually distinct (e.g. blossom/accent treatment) from ordinary commit nodes.
7. Given the Bonsai theme is active, when the graph and its ref pills / summaries / selection
   highlight are shown together, then all text and affordances meet the same legibility/contrast bar
   the standard theme is held to in `docs/contracts/ui-reference.md`.
8. Given either theme, when the user selects commits, expands refs, or otherwise interacts with the
   graph, then behavior is identical between themes (theme is styling only).

## Edge cases & error states
- **Empty / unborn-HEAD repo:** Bonsai theme shows the same empty graph state as the standard
  theme, styled consistently (e.g. empty pot / bare backdrop) — no error.
- **Very wide history (many concurrent lanes):** with only ~10 palette hues, distant lanes reuse
  colors exactly as they do today; the Bonsai palette must degrade the same way without becoming
  indistinguishable mush. No new failure mode beyond the standard theme's.
- **Detached HEAD:** HEAD styling still applies to the checked-out commit's node, same as the
  standard theme's HEAD pill behavior.
- **Theme + app light/dark interaction:** switching app light/dark while Bonsai is active must not
  produce an illegible or broken palette (see Open questions on whether Bonsai has its own
  light/dark variants).
- **Reduced-motion / accessibility:** any subtle organic touches (e.g. edge jitter) must not
  violate the app's motion and a11y conventions in `docs/contracts/ui-reference.md`.

## Resolved decisions
- **Own light/dark variants.** Bonsai is a third theme with its own light and dark palettes.
- **Selector lives in settings.** It hangs on the app's existing settings / appearance surface
  (confirm during `/plan` that such a surface exists; if not, adding a minimal appearance control is
  in scope).
- **Include flourishes.** Blossom/accent HEAD (and/or selected) nodes, optional seasonal palette
  variants, and subtle sway are all in scope for this pass — gated by reduced-motion/a11y and never
  affecting topology or scroll performance. **Sway is settle-on-scroll only** (a brief bounded
  settle after scrolling/selection, then fully idle) — NOT perpetual motion, to preserve the
  graph's on-demand repaint model and idle-CPU behavior.

## Open questions
- None blocking. Remaining specifics (leaf/blossom shapes, taper curve, exact hues, seasonal set,
  sway amplitude, selector placement within settings) are `ui-designer`'s to pin in the UI contract
  during `/plan`.
