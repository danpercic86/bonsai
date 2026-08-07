# P51 — USER CHECKPOINT checklist (native `pnpm tauri dev`)

The AI gate proves each element **draws**, each toggle **hides/shows and reclaims space**, compact
**densifies**, the date tooltip shows **absolute** times, and ahead/behind renders on diverged tips
(harness screenshots + settings round-trips + unit tests). What the harness **cannot** judge is
visual density / legibility / hover feel on a real display. Run the app against a real repo (ideally
one with several branches, an upstream that is ahead/behind, and a rebased/amended commit so
author-time != committer-time) and confirm:

## Defaults (declutter principle)
- [ ] At default settings the rows look **clean, not busy**: avatar, ref pills, summary, short SHA,
      relative date. Author-name column is OFF; ahead/behind shows only on diverged branch tips.
- [ ] The short SHA is readable and clearly a hash (monospace); it does not crowd the date.
- [ ] The verified-badge slot left of the SHA is **unobtrusive** (a faint placeholder, not noise) and
      does not look broken/clickable.

## Toggles
- [ ] Toggling **Short SHA**, **Author name**, **Date** each on/off adds/removes the column instantly;
      when a column is hidden the summary widens to reclaim the space (no gap, no overlap, no jank).
- [ ] **Date basis: Author vs Committer** changes the shown date on rebased/amended commits and matches
      what `git log --format=%ad` vs `%cd` reports.
- [ ] **Ahead/behind on branches** on/off toggles the `↑a ↓b` chip; with it on, the checked-out branch's
      numbers match the sidebar's ahead/behind for that branch.
- [ ] On a branch-tip row carrying **several refs**, the `↑a ↓b` chip sits clear of the neighbouring
      pill and, when the band is full, folds into the `+n` chip with no overlap or floating gap.
- [ ] **Compact rows** produces visibly denser rows that are still legible on your display/scaling
      (text not clipped or cramped); switching back to comfortable restores the slider-driven sizes.

## Hover
- [ ] Hovering the date column shows a tooltip with the full **absolute** Authored + Committed
      timestamps; it appears at a comfortable delay/position and dismisses on leave.

## Performance / large history
- [ ] On a large repo (thousands of commits) scrolling stays smooth with SHA + date + ahead/behind on,
      and while flipping toggles — no stutter, no smearing, lane colors stable.

## Regression
- [ ] Edges, avatars, HEAD ring, selection ring, search match-ring, stash nodes, ref pills, and the WIP
      row all still render correctly (P51 must not regress the existing passes).
