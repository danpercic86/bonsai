# P3a — Diff Overlay in the Center Pane: Implementation Contract

Status: authoritative for P3a. Scope: **frontend only** — move the per-file diff from the inline
accordion (`<li className="diff-expansion">` under the row) to a full-panel overlay covering the
center graph pane. Builds on `M4-diff.md` (DiffSlot machinery, mode A/B keys), `P1-polish.md`
(stale-content refetch §4.1, Esc/shortcut guard order §6), `P2-followups.md` (pane structure).

Invariants (unchanged): Rust owns Git logic + layout math; no IPC changes in this milestone;
`DiffView` (unified renderer) is untouched; `src/ipc/mock.ts` needs **no change** (the IPC surface
is identical — the harness keeps demoing both diff kinds through the same `getWorkdirFileDiff` /
`getCommitFileDiff` mock paths). Plain `useState`/props, no new dependencies.

---

## 1. Scope split (sub-increments)

| # | Increment | Content |
|---|---|---|
| 1 | **P3a-1** | New `DiffOverlay` component; App renders it inside `.graph-pane`; header-meta derivation; Esc handling. Inline expansions still render (transiently duplicated — build stays green, harness stays demoable). |
| 2 | **P3a-2** | Remove inline `diff-expansion` rendering from `StatusPanel.tsx` + `CommitPanel.tsx` (rows keep chevron/expanded styling); CSS cleanup (`.diff-scroll` cap removal, `.diff-expansion` rule removal). |

Each is a self-contained senior-dev pass; read §2 (shared design) plus its own section.

---

## 2. Shared design

### 2.1 What stays exactly as-is (do NOT touch)

- `App.tsx` diff-slot machinery: `diffSlot` state, `diffSlotRef`, `fileDiffReqId`,
  `fetchDiffSlot`, `collapseDiffSlot`, `handleToggleWorkdirDiff(section, entry)`,
  `handleToggleCommitDiff(file)`, the key conventions (`${section}:${path}`, `commit:${path}`),
  the same-key toggle-off behavior, the refetch-on-status-snapshot logic in `refetchStatus`
  (M4 §4.4), and the selection-change effect that resets the slot. **Only WHERE the slot renders
  changes.**
- `DiffView` and `DiffSlotView` in `src/components/DiffView.tsx` — reused unchanged (unified
  renderer; loading-skeleton / error-banner / stale-dimmed states all carry over to the overlay
  body for free).
- Lifecycle consequences (verified against current code, no new code needed — state them in a
  comment where the overlay renders):
  - Status refresh removes the expanded file → `refetchStatus` already calls
    `collapseDiffSlot()` → overlay disappears. File still present → same-key refetch → overlay
    shows stale content dimmed (`diff-stale`), then the new diff.
  - Selecting a different commit / deselecting → the `selectedIndex` effect already resets the
    slot → overlay closes. Arrow/PageUp/PageDown/Home/End navigation therefore also closes an
    open commit-diff overlay (accepted; see §7.4).
  - Clicking a different file row while the overlay is open → `fetchDiffSlot` with a new key →
    overlay content switches in place (no close/reopen flicker; the slot state transition is
    `loading` with `diff: null` since the key changed — skeleton, then content).

### 2.2 New component `src/components/DiffOverlay.tsx`

```ts
import type { FileStatus } from '../ipc';
import type { DiffSlot } from './DiffView';

/** Display metadata for the overlay header, derived by App (§2.3) from the
 * slot key + the current snapshot/commitDiff. Never stored — recomputed each
 * render so it can't go stale relative to the data that produced the slot. */
export interface DiffOverlayMeta {
  path: string;
  origPath: string | null;              // rename: header shows "orig → path"
  status: FileStatus | null;            // null = lookup failed (§2.3 fallback): no badge
  kind: 'staged' | 'unstaged' | 'untracked' | 'commit';  // drives the header context label
}

export interface DiffOverlayProps {
  slot: DiffSlot;                       // non-null by construction — App only mounts when open
  meta: DiffOverlayMeta;
  onClose(): void;                      // × button AND error-banner dismiss both call this
}
export function DiffOverlay(props: DiffOverlayProps): JSX.Element;
```

Render shape (presentational only; all state lives in App):

```tsx
<div className="diff-overlay" role="region" aria-label={`Diff: ${meta.path}`}>
  <div className="diff-overlay-header">
    {meta.status !== null && <span className="file-badge mono">{BADGES[meta.status]}</span>}
    {meta.origPath !== null
      ? <span className="diff-overlay-path mono file-rename" title={`${meta.origPath} → ${meta.path}`}>
          {meta.origPath} {'→'} {meta.path}
        </span>
      : <span className="diff-overlay-path mono" title={meta.path}>{meta.path}</span>}
    <span className="diff-overlay-kind">{KIND_LABEL[meta.kind]}</span>
    <button type="button" className="btn-icon diff-overlay-close"
            aria-label="Close diff" title="Close (Esc)" onClick={onClose}>{'×'}</button>
  </div>
  <div className="diff-overlay-body">
    <DiffSlotView slot={slot} onDismissError={onClose} />
  </div>
</div>
```

- `BADGES`: the same `Record<FileStatus, string>` map already duplicated in StatusPanel and
  CommitPanel — copy it here too (third copy; extracting a shared module is a NIT, allowed but
  not required).
- `KIND_LABEL: Record<DiffOverlayMeta['kind'], string>` =
  `{ staged: 'Staged', unstaged: 'Unstaged', untracked: 'Untracked', commit: 'Commit' }`.
- The header renders in **every** slot state — including `error` and first-load `loading` — so
  close is always reachable (`DiffSlotView` renders the skeleton/error/diff below it unchanged).
- No keyboard handling inside the component — Esc is App's job (§2.4), keeping the existing
  single-listener guard-order architecture.

### 2.3 App: mount point + meta derivation (`App.tsx`)

Render inside `<main className="graph-pane">` (already `position: relative; overflow: hidden`),
as the **last child** so it paints above the canvas, error banner, and truncated banner:

```tsx
<main className="graph-pane">
  {/* ...existing banners + GraphCanvas / empty state, unchanged... */}
  {diffSlot !== null && (
    <DiffOverlay slot={diffSlot} meta={overlayMeta} onClose={collapseDiffSlot} />
  )}
</main>
```

Meta derivation — `useMemo` on `[diffSlot, status, commitDiff]`:

```
overlayMeta(diffSlot, status, commitDiff) -> DiffOverlayMeta:
  key = diffSlot.key
  if key starts with 'commit:':
    path = key after 'commit:'
    file = commitDiff?.files.find(f => f.path === path)
    return { path, origPath: file?.origPath ?? null, status: file?.status ?? null, kind: 'commit' }
  else:
    section = key up to first ':'          // 'staged' | 'unstaged' | 'untracked' (WorkdirSection)
    path    = key after first ':'
    entry   = status?.[section].find(e => e.path === path)
    return { path, origPath: entry?.origPath ?? null, status: entry?.status ?? null, kind: section }
```

Lookup-miss fallback (entry gone from a newer snapshot in the brief window before
`refetchStatus` collapses the slot, or commitDiff cleared mid-flight): path from the key, no
badge, `origPath: null`. Never throw, never hide the close button.

### 2.4 Esc handling (`App.tsx`, existing Esc effect)

Extend the **existing** Esc `useEffect` (the one handling switcher → shortcut overlay →
typing guard → deselect). New precedence order, top wins:

1. `switcherOpen` → return (RepoSwitcher consumes Esc itself — unchanged).
2. `overlayOpen` (shortcut `?` overlay) → close it, return (unchanged).
3. Typing guard (`TEXTAREA`/`INPUT` target) → return (unchanged).
4. **NEW:** `diffSlotRef.current !== null` → `collapseDiffSlot()`, return.
   (Diff overlay closes BEFORE commit deselection — one Esc = one layer, GitKraken-style.)
5. `setSelectedIndex(null)` if selected (unchanged).

Use `diffSlotRef` (already exists) so the effect's dep array doesn't grow `diffSlot`; add
`collapseDiffSlot` to deps (stable `useCallback`, no re-subscribe churn). The Sidebar
ConfirmDialog handles its own Esc and does not appear in this effect today — do not add a
`dialogOpen` guard here (out of scope; current behavior preserved).

### 2.5 CSS (`src/styles.css`)

New rules (dark/light both via existing tokens — no new tokens):

```css
.diff-overlay {
  position: absolute;
  inset: 0;
  z-index: 5;                    /* above canvas + graph banners; toasts/ShortcutOverlay are
                                    outside .graph-pane and unaffected */
  background: var(--bg-0);
  display: flex;
  flex-direction: column;
}
.diff-overlay-header {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px 12px;
  border-bottom: 1px solid var(--border);
  flex: none;
  min-width: 0;
}
.diff-overlay-path { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.diff-overlay-kind { color: var(--text-3); font-size: 12px; flex: none; }
.diff-overlay-body { flex: 1; min-height: 0; overflow: hidden; display: flex; flex-direction: column; padding: 8px 12px; }
.diff-overlay-body .diff-scroll { flex: 1; min-height: 0; }
```

`.diff-scroll` base rule: **remove** `max-height: calc(45vh - 18px)` (P3a-2). Verified: after
inline expansion removal, `DiffSlotView` (the only `.diff-scroll` producer) renders only inside
the overlay, where height comes from the flex column above. Keep `overflow-y: auto` and the
`.diff-stale` / scrollbar rules unchanged.

---

## 3. P3a-1 — Overlay component + App wiring

Files: `src/components/DiffOverlay.tsx` (new), `src/App.tsx`, `src/styles.css` (add §2.5 new
rules only; do NOT remove the `.diff-scroll` cap yet — the inline expansion still renders this
increment and must not blow past 45vh in the right panel).

1. Implement `DiffOverlay` per §2.2.
2. App: add the `overlayMeta` memo (§2.3), mount the overlay in `.graph-pane` (§2.3), extend the
   Esc effect (§2.4).
3. No changes to StatusPanel/CommitPanel/mock this pass.

Gate: `pnpm build` green. Harness (`pnpm dev:mock`): expanding a workdir file shows the diff in
BOTH the inline accordion and the overlay (known transitional duplication — acceptable for one
increment); overlay header shows badge + path + kind + ×; Esc / × / same-row-click all close it;
clicking a different row switches content; selecting a commit closes a workdir overlay.

## 4. P3a-2 — Inline expansion removal + CSS cleanup

Files: `src/components/StatusPanel.tsx`, `src/components/CommitPanel.tsx`, `src/styles.css`.

1. `StatusPanel.tsx` `Section`: delete the `{expanded && diffSlot !== null && (<li
   className="diff-expansion">…)}` block and the surrounding `Fragment` wrapper (the `FileRow`
   becomes the direct mapped child; move `key` onto it). **Keep** everything else: `diffSlot`
   prop (still needed to compute `expanded`), `expandable`, chevron, `aria-expanded`,
   `file-row-expanded` styling, `onToggleDiff`. Drop the now-unused `DiffSlotView` import; the
   `export type { DiffSlot }` re-export stays (App imports it from here).
2. `CommitPanel.tsx`: same removal in the `data.files.map` block; keep `FileHeaderRow`
   chevron/expanded handling and the `diffSlot` prop; drop the `DiffSlotView` import.
3. `styles.css`: delete `.diff-expansion` rule; remove the `max-height` line from `.diff-scroll`
   (§2.5); leave `.diff-slot-loading`/`.diff-slot-error` (used by the overlay body).
4. Semantics note: with the inline branch gone, the row's expanded state means "this file's diff
   is open in the overlay" — update the `StatusPanelProps.onToggleDiff` / `CommitPanelProps`
   doc comments accordingly (one-line comment change, no signature change).

Gate: `pnpm build` green; `rg "diff-expansion"` in `src/` returns nothing. Harness: no inline
accordion anywhere; expanded row keeps its highlight + open chevron while the overlay is up;
long diff scrolls inside the overlay at full pane height (no 45vh cap); both modes (workdir file
and commit file) work; error fixture (if present in mock) shows the error banner inside the
overlay with a working header ×.

---

## 5. No interface changes elsewhere

- `StatusPanelProps`, `CommitPanelProps`, `DiffSlot`, `DiffSlotView`, `IpcApi`: **unchanged**.
- `src/ipc/mock.ts`: **unchanged** (no IPC surface delta; harness coverage of both diff kinds is
  already exercised through the existing toggle handlers).
- `GraphCanvas`: unchanged — the overlay sits on top; the canvas keeps painting underneath
  (cheap; no pause/suspend mechanism added).

---

## 6. Acceptance

AI gate (orchestrator, harness `VITE_MOCK_IPC=1`):
- `pnpm build` (tsc + vite) green after EACH sub-increment.
- Screenshots: (a) workdir file expanded → full-pane overlay with badge/path/kind/×, row
  highlighted in the right panel; (b) commit selected + file expanded → same for `commit` kind;
  (c) rename fixture → header shows `orig → path`; (d) long diff scrolls to the bottom inside
  the overlay (taller than 45vh).
- Interaction checks: Esc closes overlay only (selection survives); second Esc deselects the
  commit; Esc with the shortcut `?` overlay open closes the `?` overlay, not the diff; same-row
  click toggles off; different-row click switches content; deselecting/switching commits closes
  a commit-kind overlay; `rg "diff-expansion" src/` empty after P3a-2.

USER CHECKPOINT (native `pnpm tauri dev` — never self-declared):
1. On a real repo: click a changed file → diff overlays the graph; × / Esc / re-click close it;
   the graph is intact underneath afterwards.
2. Select a commit, open a file diff from its file list → overlay shows it; picking another
   commit closes it.
3. While a workdir diff overlay is open, modify that file on disk → overlay refreshes in place
   (brief dimmed stale content); delete/revert the file → overlay closes with the row.

---

## 7. Ambiguities resolved here (flag to orchestrator if disagreed)

1. **Header metadata is derived, not stored** (§2.3): a `useMemo` over `diffSlot.key` +
   `status`/`commitDiff`, with a graceful no-badge fallback on lookup miss. Alternative — storing
   a meta snapshot at toggle time — rejected: `refetchStatus` refetches the slot without a toggle,
   so stored meta could go stale (wrong badge after a file's status changes M→R).
2. **Esc precedence** (§2.4): switcher → shortcut overlay → typing guard → **diff overlay** →
   deselect. One layer per keypress. The diff overlay closes before deselection because it is
   visually the topmost layer the user opened last.
3. **Rename header**: `origPath → path` in mono, matching the existing row rendering — no
   truncation middle-ellipsis cleverness; plain `text-overflow: ellipsis` with full text in
   `title`.
4. **Graph keyboard nav closes a commit-diff overlay** (existing selection-effect behavior,
   §2.1): ArrowUp/Down etc. change `selectedIndex`, which already resets the slot. Accepted as
   correct (the overlay's content belonged to the old commit); not suppressing nav while the
   overlay is open.
5. **Transitional duplication in P3a-1** (diff visible inline AND in overlay for one increment):
   chosen over a single big pass so each pass stays small and `pnpm build`/harness stay green at
   every commit point. The `.diff-scroll` 45vh cap is therefore removed only in P3a-2.
6. **`role="region"` not `role="dialog"`**: the overlay is non-modal — the right panel and
   sidebar stay interactive (clicking other rows switches content by design), so dialog
   semantics/focus-trapping would be wrong.
7. **No mock.ts change**: confirmed — P3a alters only render location; the mock's existing
   workdir/commit diff fixtures exercise every overlay state (including the stale-refetch dim via
   the repo-changed simulation, if the mock has one; otherwise loading/ready/error suffice).
8. **Third copy of the `BADGES` map** in DiffOverlay.tsx: extraction to a shared module is
   allowed as a NIT but not required — keeping the pass minimal beats deduplicating a 7-line
   constant.
