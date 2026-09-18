# Author Coloring & Parent-Highlight on Hover

**Status:** done (AI gate green 2026-08-27; USER CHECKPOINT verified 2026-08-27 by the user in `3a71951`)
**Created:** 2026-08-26

## Problem

The graph colors lanes by branch structure, which answers "what branched where" but not "who did
what" — a common lens on team repos. Separately, tracing a commit's lineage (which parents feed
it) requires visually following edges through a tangle; a hover emphasis would make lineage
readable at a glance.

## Goals

- A persisted **"color by author"** toggle: lane/edge colors derive from each commit's author
  identity (matching the existing author-colored avatars) instead of the branch-lane palette.
- **Parent highlight:** hovering or selecting a commit emphasizes its direct parent edges and
  parent nodes (optionally de-emphasizing the rest), and releases cleanly when the pointer moves
  away.
- Both work in light and dark themes and in both graph styles (standard and Bonsai), meeting the
  app's existing contrast standards.
- No idle cost: highlight repaints only when the hovered/selected row changes.

## Non-goals

- No full-ancestry highlighting (transitive parents) — direct parents only in v1.
- No changes to avatar rendering or author identity resolution.
- No backend/Rust changes.
- No new palette configuration UI (author hues are derived deterministically, as avatars already
  are).

## User-facing behavior

- Appearance settings gain a graph coloring mode choice (branch-lane vs by-author, per
  ui-designer, consistent with `docs/contracts/ui-reference.md`). Switching recolors the graph
  immediately; the choice persists across restarts.
- In author mode, each commit's edges/lane segments take the author's hue — the same identity
  color the avatar already uses — so one person's work reads as a consistent color thread.
- Hovering a commit row (or selecting it) makes its parent edges and parent commit dots visibly
  emphasized; other content may subtly recede. Moving the pointer off restores normal rendering.
  Selection keeps its existing appearance; the parent emphasis composes with it.
- No behavior change when the toggle is off and nothing is hovered.

## Acceptance criteria

1. Given "color by author" is enabled, then lanes/edges are colored by author identity
   consistently with avatar colors; toggling back restores branch-lane coloring exactly; the
   setting persists across restart.
2. Given a commit is hovered or selected, then its direct parent edges and parent nodes are
   visibly emphasized; moving away restores normal paint.
3. Given no pointer movement over the graph, then no repaints occur (idle CPU unchanged).
4. Both coloring modes and the highlight are legible in light + dark themes and in standard +
   Bonsai graph styles (contrast per existing standards).
5. Works composed with spec-003 filters and spec-004 folding (highlight targets whatever parents
   are visible; a parent hidden by the current view is simply not highlighted).

## Edge cases & error states

- **Root commits (no parents):** hover shows no parent emphasis; no errors.
- **Merge commits:** all direct parents (2+) are emphasized.
- **Parent off-screen:** the visible portion of the parent edge is emphasized; no scrolling
  side-effects on hover.
- **Same author everywhere (solo repos):** author mode renders a mostly single-hue graph — 
  acceptable and expected; structure remains readable via existing node/edge geometry.
- **Unknown/empty author names:** fall back to the same deterministic color the avatar fallback
  uses.
- **Touch/keyboard:** selection-driven highlight provides the keyboard path; hover is
  pointer-only.

## Open questions

None.
