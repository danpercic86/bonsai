# Graph Overview Rail — Search Match Ticks & On-Demand Minimap

**Status:** implemented (AI gate green 2026-08-27; USER CHECKPOINT pending)
**Created:** 2026-08-26

## Problem

In large histories, users lose their bearings: commit search shows matches one at a time with no
sense of where in history they cluster, and there is no compressed overview of the whole graph to
orient or jump by. Scrolling blindly through 20k commits to find "that area with the release
tags" is slow and frustrating.

## Goals

- A thin vertical **overview rail** along the graph's scroll edge with two cooperating layers:
  1. **Match distribution** (search open): a tick per search match at its proportional position
     in the full history; the current match visually distinct; clicking a tick jumps to and
     selects that match.
  2. **Minimap** (on demand): a downsampled rendering of the whole history — branch/lane density,
     small markers for refs (branch/tag) and HEAD, and a viewport thumb showing the currently
     visible window. Clicking or dragging jumps/scrolls.
- **On-demand policy:** hidden by default; appears while search is open and on hover near the
  graph's right edge; a persisted "always show" setting.
- Zero cost when hidden: no rendering or computation while the rail is not shown.
- Smooth on 20k+ histories.

## Non-goals

- No new search capabilities (fields, syntax, scope) — only visualization of existing matches.
- No Rust/backend changes — the overview uses data the graph already has.
- No editing/interaction beyond jump/scroll (no drag-to-select ranges, no context menus).
- Not a replacement for the native scrollbar — it complements it and must not overlap/steal it.

## User-facing behavior

- With commit search open and matches present, a slim rail appears along the graph's right edge
  showing ticks where matches live in the whole history (browser-find style). Dense clusters read
  as denser marks, not thousands of overlapping ticks. The current match's tick is highlighted
  and updates on next/previous. Clicking a tick scrolls there and selects that match. Closing
  search hides the ticks (and the rail, unless otherwise shown).
- Hovering near the graph's right edge (or enabling "always show" in Appearance/Commit-graph
  settings per ui-designer) reveals the minimap: a compressed strip of the entire history with
  visible branch density, ref/HEAD pips, and a thumb marking the current viewport. The thumb
  moves as the user scrolls; dragging the thumb or clicking elsewhere on the strip jumps the main
  graph there. Moving the pointer away hides it again (unless search is open or always-show is
  set).
- Both layers combine when active (match ticks overlay the minimap).
- The rail respects the existing graph inset/scrollbar geometry and both app themes and graph
  styles.

## Acceptance criteria

1. Given search is open with N matches, then the rail shows their distribution at proportional
   positions; clicking a tick scrolls to and selects that match.
2. Given next/previous navigation, then the current-match tick is visually distinct and updates.
3. Given search closes, then ticks disappear; with always-show off and no hover, the rail is
   fully hidden and contributes no idle cost.
4. Given the minimap is visible, then it shows compressed lane/branch density, ref and HEAD pips,
   and a viewport thumb that tracks scrolling; click/drag jumps the graph accordingly.
5. Given "always show" is enabled, then the minimap is persistent and the setting survives
   restart.
6. Given a 20k+ commit repo, revealing/using the rail causes no jank (USER CHECKPOINT for feel).
7. The rail never overlaps or blocks the native scrollbar or existing graph overlays.

## Edge cases & error states

- **Very short histories** (fewer rows than rail pixels): ticks/pips still land at sensible
  positions; the thumb may span most of the rail.
- **Huge match counts** (thousands): ticks coalesce by pixel density — no per-match DOM/draw cost.
- **Graph reloads (filter change, refresh) while the rail is visible:** the rail rebuilds from
  the new layout; stale marks never linger.
- **Folded rows (spec-004) / filtered views (spec-003):** positions map to the *current* view's
  rows, so ticks/pips always align with what clicking would reach.
- **Empty search (0 matches):** rail shows no ticks; existing "no matches" search UI is
  unchanged.

## Open questions

None — on-demand default and always-show setting are locked per the feature brief.
