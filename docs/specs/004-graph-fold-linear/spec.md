# Fold Linear Runs — Collapse Uneventful History into Expandable Rows

**Status:** draft
**Created:** 2026-08-26

## Problem

Even a decluttered graph (spec-003) can contain long uneventful stretches: dozens or hundreds of
consecutive commits with no branching, no merging, and no refs. Users scrolling large histories
must wade through these runs to reach the structurally interesting points (forks, merges, tagged
releases). Other clients offer "commit folding" for exactly this reason.

## Goals

- An opt-in **fold mode**: maximal linear runs — consecutive commits where nothing branches in or
  out and no ref points at any commit in the run — collapse into a single placeholder row reading
  as "⋯ N commits".
- Folded rows are **expandable**: activating a placeholder reveals that run's commits in place;
  a revealed run can be re-collapsed.
- Structure is always preserved: merge commits, fork points, ref-carrying commits, HEAD, and
  stash entries are never folded away.
- Fold mode composes with spec-003's filters (first-parent, solo/hide) — folding applies to
  whatever history the current filter view shows.
- The toggle persists across restarts, alongside the spec-003 declutter settings.
- Performance on 20k+ histories is equal or better with folding on (fewer rows to render).

## Non-goals

- Folding a merge's entire second-parent subtree behind the merge commit (a different, heavier
  interaction — future work).
- Any automatic/heuristic folding thresholds tuning UI (a single sensible minimum run length is
  chosen by design, not user-configurable in v1).
- Changing search behavior: search is out of scope; matches inside folded runs may simply expand
  or be unreachable per design's choice in `/plan` — but no new search features.
- Any change to the default (fold off) graph output.

## User-facing behavior

- A "Fold linear runs" toggle joins the spec-003 declutter controls (settings + the graph's
  inline filter control, per the spec-003 UI conventions), and participates in the same
  "filtered" indication so users know the graph is condensed.
- With fold on, a run of uneventful commits shows as one placeholder row ("⋯ 37 commits") drawn
  in the run's lane, visually distinct from a commit row (no avatar, no message, not selectable
  as a commit).
- Clicking/activating a placeholder expands that run in place; the surrounding graph stays put as
  much as possible (the expanded commits appear where the placeholder was). An expanded run shows
  an affordance to re-collapse it.
- Selecting a commit, then enabling fold: if the selected commit would be inside a folded run,
  its run stays expanded (or the selection is preserved by keeping that commit visible).
- Turning fold off restores the exact unfolded view of the current filter state.

## Acceptance criteria

1. Given fold mode on and a history containing a linear run of ≥ the minimum length with no
   refs/branch/merge points, when the graph renders, then the run appears as a single "⋯ N
   commits" placeholder and N equals the hidden commit count.
2. Given a placeholder row, when the user activates it, then the N commits appear in place and a
   re-collapse affordance is available; re-collapsing restores the placeholder.
3. Given fold mode on, merge commits, fork points, commits with any ref pill, HEAD, and stash
   entries are always visible (never inside a folded run).
4. Given fold mode off, the graph is identical to the pre-fold view of the same filter state.
5. Given fold mode on together with first-parent and/or solo/hide, folding applies to the
   filtered history and all spec-003 guarantees still hold.
6. Given a selected commit inside a would-be-folded run, when fold is enabled, then the commit
   remains visible and selected.
7. The fold toggle persists across restart and the filtered/condensed indication reflects it.
8. A 20k+ commit repo with fold on scrolls smoothly (USER CHECKPOINT for feel).

## Edge cases & error states

- **Run at the graph's top or bottom / adjacent to the truncation cap:** folds normally; the
  truncated indicator still shows when the 100k cap is hit.
- **Overlapping lanes:** a run only folds if it is linear within its own lane and no other lane's
  edge passes through in a way that would be misrendered — design/plan decides the conservative
  rule; when in doubt, don't fold.
- **Very short runs:** runs below the minimum length (design-chosen) never fold — a "⋯ 2 commits"
  pill is worse than the two rows.
- **Search/reveal targeting a hidden commit** (e.g. jump from blame): the run auto-expands so the
  reveal lands on a real row.
- **Expansion state on reload/filter change:** expansion state is transient (per-session,
  per-view); changing filters or reloading returns runs to folded.

## Open questions

None blocking — minimum run length and the conservative lane rule are design decisions for
`/plan`; expansion transience decided above.
