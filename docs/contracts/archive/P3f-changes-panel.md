# P3f — Changes-panel refinements (user feedback 2026-07-29)

Two small **frontend-only** tweaks to the working-directory changes panel. No Rust, no IPC, no wire
changes. Files: `src/components/Tree.tsx`, `src/components/StatusPanel.tsx` (and CSS if needed).

## §1 Double-click a directory row → apply the section action to all files beneath it
In tree view, single-click on a directory toggles expand/collapse (unchanged). Add: **double-click**
on a directory row applies that section's row action to EVERY file under it (recursively). In the
"Changes" section that stages all descendants; in the "Staged" section it unstages all descendants
(the section already owns the correct `onAction` = `onStage`/`onUnstage`).

- `Tree` is generic and display-only; it must not know about "stage". Add an optional prop
  `onActivateDir?(leaves: TreeLeaf<T>[]): void` and an optional `dirActionHint?: string` (used as the
  dir row's `title`, for discoverability, e.g. "Double-click to stage all").
- On the dir toggle button add `onDoubleClick`; when `onActivateDir` is set, collect all descendant
  leaves of that dir node (recursive helper over `node.children`) and call `onActivateDir(leaves)`.
  Keep the existing single-click `toggle` — a double-click fires two clicks (expand toggles cancel
  out, net-zero) plus the dblclick; that is acceptable, do not add timers.
- In `StatusPanel`'s `Section`, pass to `Tree`:
  `onActivateDir={(leaves) => onAction(leaves.flatMap((l) => entryPaths(l.item)))}` and
  `dirActionHint={rowAction === 'unstage' ? 'Double-click to unstage all' : 'Double-click to stage all'}`.
  (`entryPaths` already sends both sides of a rename.) Only the tree branch needs this; flat lists
  have no dir rows.

## §2 File leaves must not show the expand chevron
The diff opens in the center-pane overlay (not inline), and the selected row already highlights via
`file-row-expanded`, so the leading `›` chevron on file rows is misleading. Remove the
`<span className="file-chevron">…</span>` from the expandable `FileRow` (the `expandable` branch in
`StatusPanel.tsx`). Keep the row a `<button>` with the same `onClick`/`aria-expanded` so clicking
still toggles the diff; only the chevron glyph goes. Directory rows keep their chevron (they truly
expand/collapse). Scope: `FileRow` only — leave `ConflictRow` (different 3-button interaction) as is.
Do not add a spacer; leaves align without the arrow.

## Gate
`pnpm build` clean (tsc + vite). Harness (orchestrator): tree view shows no chevron on file rows,
dir rows still toggle on single click, and double-clicking a dir stages/unstages all files under it.
