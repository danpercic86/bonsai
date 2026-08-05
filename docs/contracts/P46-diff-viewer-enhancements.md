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
