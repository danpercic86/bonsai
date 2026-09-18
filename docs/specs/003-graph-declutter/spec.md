# Graph Declutter Modes — First-Parent Toggle & Branch Solo/Hide

**Status:** done (AI gate green 2026-08-26; USER CHECKPOINT verified 2026-08-27 by the user in `3a71951`)
**Created:** 2026-08-26

## Problem

In repositories with heavy merge traffic or many branches, Bonsai's commit graph becomes a tangle
of lanes that buries the history the user actually cares about. This is the single most-requested
commit-graph capability across competing Git clients. Users need ways to temporarily simplify the
graph — collapse merged-in side branches, or focus on just the branches they choose — without
losing the ability to snap back to the full picture.

## Goals

- A **first-parent view** toggle: the graph follows only each commit's first parent, so side
  branches that were merged in collapse out of the mainline, and linear history renders as a
  single lane.
- **Branch solo / hide**: the user can restrict the graph to the ancestry of a chosen subset of
  refs (solo one branch, or hide selected branches/remotes/tags) instead of the default
  "everything" view.
- Both filters are **interactive and reversible**: turning a filter off restores the exact full
  graph the user had before.
- The user can always tell the graph is filtered (a visible indicator), so missing commits are
  never mistaken for missing data.
- Filter choices persist across app restarts.
- Filtering must not degrade performance; a filtered view walks less history and should feel equal
  or faster than the full view.

## Non-goals

- Folding linear runs or collapsing a merge's subtree into an expandable placeholder (a separate,
  later spec).
- Any change to the default, unfiltered graph — with no filter active, output and behavior remain
  exactly as today.
- Filtering the working-directory status, diffs, search scope, or any panel other than the commit
  graph.
- Animating the transition between filtered and unfiltered layouts (the graph simply re-renders).

## User-facing behavior

- The Commit-graph settings category gains persisted controls for the declutter filters, and a
  quick inline control near the graph itself offers the same toggles for interactive use (exact
  placement/appearance per ui-designer, consistent with `docs/contracts/ui-reference.md`).
- **First-parent on:** merge commits remain, but the side histories they merged in disappear; the
  mainline reads as a straight lane where history is linear. Ref pills on hidden side branches are
  not shown (their tips are no longer in the graph).
- **Solo a branch:** the graph shows only that branch's ancestry, plus the current HEAD's ancestry
  (the checked-out state is never hidden). Hiding refs works the same way in reverse: hidden refs'
  tips and exclusive history disappear; ref pills for hidden refs are gone.
- While any filter is active, a subtle "filtered" indicator is visible near the graph.
- Changing a filter re-loads the graph. The previously selected commit stays selected and is
  scrolled into view if it still exists in the filtered graph; otherwise selection clears and the
  view rests at the top.
- Turning all filters off restores the identical full graph (same lanes, same colors, same order).

## Acceptance criteria

1. Given a repo with merged branches, when the user enables first-parent view, then the merged-in
   side branches disappear, linear stretches render as a single lane, and merge commits themselves
   remain visible.
2. Given first-parent view is on, when the user turns it off, then the graph is identical to the
   pre-filter full graph (same nodes, lanes, ordering, colors).
3. Given the user solos branch X while branch Y is checked out, when the graph reloads, then it
   contains exactly X's ancestry plus HEAD's ancestry, and ref pills for other branches/tags are
   absent.
4. Given no filter is active, then graph output is unchanged versus today (regression-tested
   equality with the pre-feature behavior).
5. Given any filter is active, then a visible "filtered" indicator is present; with no filter, it
   is absent.
6. Given the user sets filters and restarts Bonsai, when the repo reopens, then the same filters
   are applied and indicated.
7. Given a commit is selected, when the user changes a filter and that commit exists in the new
   view, then it remains selected and visible; when it does not exist, selection clears.
8. Given a 20k+ commit repo, filtered views scroll as smoothly as the full view (native scroll
   feel is a USER CHECKPOINT).

## Edge cases & error states

- **HEAD's branch hidden / not in the solo set:** HEAD's ancestry is always included; the HEAD pill
  always resolves. The graph is never empty while a repo with commits is open.
- **All refs hidden:** treated as above — HEAD's ancestry still shows.
- **Detached HEAD:** the HEAD pill and its ancestry behave the same as a checked-out branch.
- **Solo'd/hidden ref deleted (e.g. branch deleted externally):** stale refs in the persisted
  filter are ignored; the graph shows the remaining valid selection, falling back to the full
  graph if none remain valid.
- **Empty/unborn-HEAD repo:** filters have no visible effect; the existing empty-graph state shows.
- **Truncated graph (100k cap):** the truncated indicator continues to work in filtered views.
- **Stashes:** first-parent view must not break stash display; stash entries follow the same
  first-parent rule as other commits.

## Open questions

None — decisions already locked with the requester: solo/hide always keeps HEAD's ancestry;
persisted topology toggles live in the Commit-graph settings category; no transition animation.
