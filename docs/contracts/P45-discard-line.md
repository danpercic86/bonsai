# P45 — Per-Line Discard: Implementation Contract

Status: authoritative for P45. Implementer: senior-dev (single fresh-context pass — frontend +
CSS only). Builds on `P28-discard-hunk.md` (the `discard_partial` core/command/IPC/mock and the
"Discard hunk?" ConfirmDialog pattern) and `P17-partial-staging.md` (`LineSelection`, the
per-line gutter `+` button, the drag-range float, `toSelection`).

**Feature:** a per-line **discard** affordance in the diff overlay that mirrors the existing
per-line **stage**, in BOTH forms the user chose:
1. A floating **"Discard N lines"** button beside the existing "Stage N lines" float (drag-select).
2. A per-line **gutter discard control** beside the existing `+` stage gutter button.

Discard is destructive → per the project guardrail every discard first arms a `ConfirmDialog`,
exactly like "Discard hunk".

## 0. NO backend / IPC / mock changes

The Rust command `discard_partial` (`commands.rs:841` → core `discard_partial.rs:34`), the IPC
wrapper `ipc.discardPartial(repoId, path, origPath, selection)` (`tauri.ts:195`), and the mock
`discardPartial` (`mock.ts:2048`, `src/main.rs`-modeled, already accepts single-line selections)
ALREADY support arbitrary per-line `LineSelection[]` discard. **This increment touches ONLY
frontend `.tsx` + `styles.css`. No `.rs`, no `ipc/*.ts`, no `mock.ts` edits.** Any diff outside
`src/components/*.tsx` and `src/styles.css` is out of scope for P45.

Architecture invariants (unchanged): Rust owns all Git logic + the byte-exact worktree edit;
React only collects the selection, confirms, and calls back. Commands = request/response; no new
events/channels. The backend's stale-selection guard is the source of truth — the UI never
re-validates coordinates.

---

## 1. Changed files (all frontend)

```
src/components/DiffView.tsx          # onDiscardLines prop; gutter + float discard controls; discardable modifier
src/components/DiffOverlay.tsx       # forward onDiscardLines (DiffSlotView branch only)
src/components/WorkspaceGraphPane.tsx# thread + gate onDiscardLines
src/components/RepoWorkspace.tsx     # pendingLineDiscard state + 2 handlers + render/dialog wiring
src/components/WorkspaceDialogs.tsx  # pendingLineDiscard prop trio + "Discard line(s)?" ConfirmDialog
src/styles.css                       # .diff-view--discardable widen; .diff-gutter-discard-btn; .diff-float-discard
```

---

## 2. New prop: `onDiscardLines`

One prop, threaded down the SAME chain as `onDiscardHunk`, gated by the SAME expression. Both
affordances call this ONE prop.

`DiffView.tsx` — `DiffViewProps` (after `onDiscardHunk`, ~line 29):
```ts
/** P45: discard exactly these changed lines (context already dropped) in the
 *  WORKTREE. Rendered only when provided AND stageable === 'stage' (unstaged
 *  tracked diffs). Both the gutter control and the drag-range float call this. */
onDiscardLines?(selection: LineSelection[]): void;
```

`DiffView.tsx` — `DiffSlotViewProps` (~line 309): add `onDiscardLines?(selection: LineSelection[]): void;`
and forward it into `<DiffView ... onDiscardLines={onDiscardLines} />` (~line 355).

`DiffOverlay.tsx` — `DiffOverlayProps` (after `onDiscardHunk`, ~line 174):
```ts
/** P45: set ONLY for unstaged tracked diffs (same gate as onDiscardHunk);
 *  forwarded to the DiffSlotView branch only. */
onDiscardLines?(selection: LineSelection[]): void;
```
Destructure (~line 189) and pass into the `DiffSlotView` branch only (~line 271), next to `onDiscardHunk`.

`WorkspaceGraphPane.tsx` — `WorkspaceGraphPaneProps` (after `onDiscardHunk`, line 52):
```ts
onDiscardLines: DiffOverlayProps['onDiscardLines'];
```
Destructure (~line 128) and, at the `<DiffOverlay>` render (~line 212), gate it IDENTICALLY to
`onDiscardHunk`:
```tsx
onDiscardLines={
  overlayMeta.kind === 'unstaged' && stageable === 'stage' ? onDiscardLines : undefined
}
```
(`kind` is `'staged' | 'unstaged' | 'untracked' | 'commit' | 'conflict' | 'compare' | 'aiProposal'`
— so untracked, staged, commit, compare, conflict, and any `stageable === null` diff never
receives the prop.)

`RepoWorkspace.tsx` — pass `onDiscardLines={handleDiscardLines}` at the WorkspaceGraphPane render
site (~line 3042, beside `onDiscardHunk={handleDiscardHunk}`).

---

## 3. State + handlers (`RepoWorkspace.tsx`)

**State** (beside `pendingHunkDiscard`, ~line 217) — STORE THE SELECTION. Unlike a hunk (re-derivable
from `diff.hunks[hunkIndex]`), an arbitrary line selection cannot be recomputed later, so it is
captured verbatim at arm time:
```ts
const [pendingLineDiscard, setPendingLineDiscard] = useState<{
  path: string;
  origPath: string | null;
  selection: LineSelection[];
} | null>(null);
```
Add `pendingLineDiscard !== null ||` to the "any dialog open" guard (~line 306, mirrors `pendingHunkDiscard`).

**`handleDiscardLines`** (mirror `handleDiscardHunk`, 1309 — but store the selection). Just arms the
dialog:
```ts
const handleDiscardLines = useCallback((selection: LineSelection[]) => {
  if (selection.length === 0) return;              // empty -> skip
  const meta = overlayMetaRef.current;
  if (meta === null) return;
  setPendingLineDiscard({ path: meta.path, origPath: meta.origPath, selection });
}, []);
```

**`handleConfirmLineDiscard`** (mirror `handleConfirmHunkDiscard`, 1318 — but use the stored
selection instead of re-deriving from a hunk index):
```ts
const handleConfirmLineDiscard = useCallback(
  async (pending: { path: string; origPath: string | null; selection: LineSelection[] }) => {
    if (mutatingRef.current) return;
    if (overlayMetaRef.current?.path !== pending.path) return;   // slot moved on
    if (pending.selection.length === 0) return;
    setMutating(true);
    try {
      await ipc.discardPartial(repoId, pending.path, pending.origPath, pending.selection);
      await refetchStatus();
    } catch (e) {
      reportStatusError(errorMessage(e));           // surfaces backend stale() error
    } finally {
      setMutating(false);
    }
  },
  [repoId, refetchStatus, reportStatusError],
);
```

Wire into `<WorkspaceDialogs>` (~line 3177, beside the hunk trio):
`pendingLineDiscard={pendingLineDiscard}`, `setPendingLineDiscard={setPendingLineDiscard}`,
`handleConfirmLineDiscard={(pending) => void handleConfirmLineDiscard(pending)}`.

---

## 4. ConfirmDialog (`WorkspaceDialogs.tsx`)

Props (after the `pendingHunkDiscard` trio, ~line 105):
```ts
pendingLineDiscard: { path: string; origPath: string | null; selection: LineSelection[] } | null;
setPendingLineDiscard: (v: { path: string; origPath: string | null; selection: LineSelection[] } | null) => void;
handleConfirmLineDiscard(pending: { path: string; origPath: string | null; selection: LineSelection[] }): void;
```
(Import `LineSelection` from `../ipc` if not already imported.)

Dialog (mirror "Discard hunk?", 532; title/label pluralize on `selection.length`):
```tsx
<ConfirmDialog
  open={pendingLineDiscard !== null}
  title={pendingLineDiscard !== null && pendingLineDiscard.selection.length === 1 ? 'Discard line?' : 'Discard lines?'}
  confirmLabel={pendingLineDiscard !== null && pendingLineDiscard.selection.length === 1 ? 'Discard line' : 'Discard lines'}
  busy={mutating}
  onConfirm={() => {
    const pending = pendingLineDiscard;
    setPendingLineDiscard(null);
    if (pending !== null) void handleConfirmLineDiscard(pending);
  }}
  onCancel={() => setPendingLineDiscard(null)}
>
  <div>
    Discard {pendingLineDiscard?.selection.length ?? 0} selected{' '}
    line{(pendingLineDiscard?.selection.length ?? 0) === 1 ? '' : 's'} in{' '}
    <span className="mono">{pendingLineDiscard?.path ?? ''}</span>?
  </div>
  <div className="dialog-body-note">
    The change{(pendingLineDiscard?.selection.length ?? 0) === 1 ? '' : 's'} are permanently
    reverted in your working tree and cannot be undone. Staged changes are not affected.
  </div>
</ConfirmDialog>
```

---

## 5. DiffView controls

Compute once in the component body: `const discardable = stageable === 'stage' && onDiscardLines !== undefined;`

**5.1 Gutter control** — inside the `.diff-marker` span, when `interactive && isChanged`, render the
EXISTING stage `+` button, then when `discardable` render a SECOND danger button (mirror the stage
gutter button's `onPointerDown` stop-drag + `onClick` stop-propagation):
```tsx
{discardable && (
  <button
    type="button"
    className="diff-gutter-discard-btn"
    title="Discard this line"
    aria-label="Discard this line"
    onPointerDown={(e) => e.stopPropagation()}
    onClick={(e) => { e.stopPropagation(); onDiscardLines?.([toSelection(line)]); }}
  >
    {'×'}
  </button>
)}
```
(Glyph `×` is cosmetic — implementer may substitute a revert glyph; keep it single monospace cell.)

**5.2 Range float** — a `commitDiscardRange` beside `commitRange` (172):
```ts
const commitDiscardRange = () => {
  if (changedInRange.length === 0) return;
  onDiscardLines?.(changedInRange.map(toSelection));
  setRange(null);           // App has captured the selection into pendingLineDiscard
};
```
In the `floatButton` JSX (237), after the existing Stage button and inside the same
`.diff-stage-float` container, render a second button when `discardable`:
```tsx
{discardable && (
  <button
    type="button"
    className="diff-float-discard"
    onPointerDown={(e) => e.stopPropagation()}
    onClick={commitDiscardRange}
  >
    {`Discard ${changedInRange.length} line${changedInRange.length === 1 ? '' : 's'}`}
  </button>
)}
```

**5.3 Discardable modifier on the container.** Add `diff-view--discardable` to the root class of BOTH
render branches when `discardable` (default branch `className="diff-view"` @264 and file branch
`"diff-view diff-view-file"` @256), e.g. ``className={`diff-view${discardable ? ' diff-view--discardable' : ''}`}``.

---

## 6. The marker-gutter layout decision — CHOSEN: (a) container-scoped modifier that widens the marker column

The `.diff-marker` cell is a single 16px grid column (`.diff-line { grid-template-columns: 40px 40px
16px auto }`, styles.css:2504). Two controls cannot both fit at 16px. **Chosen: a container-scoped
modifier class `.diff-view--discardable`** (set only when discard is enabled, i.e. unstaged tracked)
that widens the marker column to hold two side-by-side gutter buttons and turns `.diff-marker` into a
tiny flex row. Rejected alternative (b) hover-pinned absolute control: absolute positioning inside a
`white-space: pre` monospace grid row is fragile (overlaps line numbers on horizontal scroll) and the
existing gutter buttons are already hover-revealed via `.diff-line:hover .diff-gutter-btn`, so keeping
both controls in-flow is more consistent and deterministic. The modifier scopes the change so read-only
commit/compare/conflict diffs and STAGED diffs (no modifier) keep the untouched 16px column.

CSS (styles.css, near `.diff-gutter-btn` @2594 and `.diff-stage-float` @2621):
```css
/* P45: unstaged tracked diffs carry a second gutter control -> widen the marker
   column and lay its two buttons out side by side. Read-only/staged diffs (no
   modifier) keep the base 16px single-control gutter. */
.diff-view--discardable .diff-line { grid-template-columns: 40px 40px 34px auto; }
.diff-view--discardable .diff-marker { display: flex; align-items: center; justify-content: center; gap: 2px; }

/* Danger sibling of .diff-gutter-btn (same hover-reveal aesthetic). */
.diff-gutter-discard-btn {
  flex: 1; padding: 0; border: none; background: none; font: inherit;
  color: var(--text-3); cursor: pointer; user-select: none;
}
.diff-line:hover .diff-gutter-discard-btn,
.diff-gutter-discard-btn:hover,
.diff-gutter-discard-btn:focus-visible { color: var(--danger); }
/* keep the existing .diff-gutter-btn sizing valid inside the flex marker */
.diff-view--discardable .diff-gutter-btn { flex: 1; width: auto; }

/* Danger float button beside "Stage N lines" (shares the .diff-stage-float box). */
.diff-float-discard {
  font-family: var(--font-ui); font-size: 12px; line-height: 1; padding: 6px 12px;
  border: none; border-radius: 6px; background: var(--danger); color: var(--bg-0);
  cursor: pointer; box-shadow: 0 2px 8px rgba(0, 0, 0, 0.28); margin-left: 8px;
}
.diff-float-discard:hover { filter: brightness(1.08); }
```

---

## 7. Edge cases (normative)

- **Single vs range:** gutter → `onDiscardLines([toSelection(line)])` (one line); float →
  `onDiscardLines(changedInRange.map(toSelection))` (the drag range's add/del lines only).
- **Empty / no-op selection:** `commitDiscardRange` returns on `changedInRange.length === 0`;
  `handleDiscardLines` and `handleConfirmLineDiscard` return on `selection.length === 0`. Never
  call `ipc.discardPartial` with `[]`.
- **Stale selection:** if the worktree changed after arming, `ipc.discardPartial` rejects with the
  backend `stale()` error (`{ kind:'other', message:'selection is stale; refresh the diff' }`);
  caught → `reportStatusError(errorMessage(e))`. The UI does not pre-validate.
- **Confirm required:** every discard (gutter click or float click) arms `pendingLineDiscard` and
  reverts NOTHING until the user confirms; Cancel changes nothing.
- **Controls ABSENT** on: staged diffs (`stageable === 'unstage'` → `discardable` false), untracked
  (`kind === 'untracked'` fails the gate), and read-only commit/compare/conflict diffs
  (`stageable === null`). In all these the marker column stays 16px (no modifier) and neither the
  gutter discard button nor the float discard button renders — identical exclusion set to P28's
  "Discard hunk".
- **Guard on confirm:** `mutatingRef` blocks concurrent mutations; `overlayMetaRef.current?.path`
  must still equal the armed path or the confirm no-ops.
- After a successful discard, `refetchStatus()` re-fetches the open slot; a fully-reverted file drops
  from the unstaged section (mock: `workdir` re-equals `index`).

---

## 8. Acceptance criteria

**AI gate (orchestrator, browser harness `pnpm dev` + `VITE_MOCK_IPC=1`):**
- `pnpm build` + `tsc` green.
- Open `src/main.rs` UNSTAGED diff: each changed line shows the `+` stage button AND a danger `×`
  discard button in a widened gutter; drag-select shows BOTH "Stage N lines" and "Discard N lines"
  floats (screenshot).
- Click a gutter `×` → "Discard line?" ConfirmDialog; Cancel changes nothing; Confirm reverts that
  one line (mock `workdir` mutates, row updates). Drag-select + "Discard N lines" → "Discard lines?"
  dialog, plural body naming the file and count; Confirm reverts exactly those lines.
- STAGED diff, commit/compare overlays, and untracked rows show NO discard gutter button and NO
  discard float — marker column stays 16px there (screenshot of a staged diff).
- Console clean; no `@tauri-apps/*` module executed; no backend/IPC/mock diff in the changeset.

**USER CHECKPOINT (native `pnpm tauri dev` — never self-declared):** on a scratch repo, in a
multi-line unstaged modification: discard a single line via the gutter `×` + dialog, then drag-select
several lines and use "Discard N lines" + dialog; verify in a terminal that `git diff` shows only the
non-discarded changes remaining and `git diff --cached` is unchanged.

---

## 9. Ambiguities resolved here (flag if disagreed)

1. **Layout:** container-scoped `.diff-view--discardable` widen (§6) over a hover-pinned absolute
   control — more robust in the monospace grid and consistent with the existing hover-reveal gutter.
2. **Store the selection** in `pendingLineDiscard` (not a hunk index) — arbitrary lines can't be
   re-derived after arming (unlike P28's hunk index).
3. **One prop for both affordances** (`onDiscardLines`) — gutter passes a 1-element selection, float
   passes the range; App arms the same dialog either way.
4. **Glyph `×`** for the gutter discard control is cosmetic; a revert glyph is acceptable if the
   reviewer prefers, provided it stays a single monospace cell with the "Discard this line" title.
