# P6 — Unified branch/remote context menus

One right-click context menu is the **single** place that acts on a branch or a
remote-tracking branch, available **identically** from BOTH the commit-graph ref pills
AND the left sidebar rows. The sidebar's inline per-row action buttons are removed so ref
names take all available horizontal space.

House style follows `M5-branches.md`, `P3c-merge-conflicts.md`, `P3d-rebase.md`
(merge/rebase wording + gating) and `P5-graph-context-menus.md` (the `ContextMenu` component,
`GraphContextTarget`, `buildContextItems`, `handleGraphContextMenu`, and Compare mode — all
reused verbatim here).

---

## §1 Scope + the exact per-kind menus

### 1.1 The one shared builder
A single `branchMenuItems(name, kind)` in `RepoWorkspace.tsx` produces the menu for BOTH
surfaces. It is given only a ref **name** and **kind**; it resolves everything else (`tip`,
`isHead`) from the current `branches` snapshot so the graph and the sidebar can never diverge.
`GraphContextTarget` is **unchanged**.

### 1.2 Menu item lists (exact order, labels, gating)

Let `cur = headBranch?.name ?? null` (current branch, null when detached/unborn),
`gate = mutating || opActive`, `headUnborn = head === null || head.unborn`.

**A. Local branch, non-current** (`kind: 'localBranch'`, snapshot `isHead === false`) — items in order:

| # | Label (exact) | Present when | Disabled when | onSelect |
|---|---|---|---|---|
| 1 | `Checkout` | always | `gate` | `void handleCheckoutBranch(name)` |
| 2 | `Merge ${name} into ${cur}` | `cur !== null` | `gate` | `void handleMergeBranch(name)` |
| 3 | `Rebase ${cur} onto ${name}` | `cur !== null` | `gate` | `void handleRebaseBranch(name)` |
| 4 | `Compare with HEAD` | `!headUnborn` | never (read-only) | `handleCompareWithHead(tip)` |
| 5 | `Delete` | always | `gate` | open the local-delete confirm (§4.5) |

**B. Local branch, current / HEAD** (`kind: 'localBranch'`, snapshot `isHead === true`) — **empty**.
`branchMenuItems` returns `[]`; the menu does **not** open (matches today's own-branch-pill
behavior on the graph and the current sidebar's "no buttons on the head row").

**C. Remote-tracking branch** (`kind: 'remoteBranch'`) — items in order (the FULL set):

| # | Label (exact) | Present when | Disabled when | onSelect |
|---|---|---|---|---|
| 1 | `Checkout` | always | `gate` | `void handleCheckoutRemote(name)` (create+switch to a local tracking branch, GitKraken-style) |
| 2 | `Merge ${name} into ${cur}` | `cur !== null` | `gate` | `void handleMergeBranch(name)` |
| 3 | `Rebase ${cur} onto ${name}` | `cur !== null` | `gate` | `void handleRebaseBranch(name)` |
| 4 | `Compare with HEAD` | `!headUnborn` | never (read-only) | `handleCompareWithHead(tip)` |
| 5 | `Delete` | always | `gate` | open the remote-delete confirm (§4.5) — deletes the LOCAL remote-tracking ref only |

**D. Tag** (`kind: 'tag'`) and **the `head` pill** — **empty**, no menu (unchanged).

**E. Commit row** (graph only, `GraphContextTarget.kind === 'commit'`) — unchanged from P5:
one `Compare with HEAD` item (disabled: `false`), omitted when `headUnborn`. Uses the commit's
own `target.oid`.

### 1.3 Rationale / invariants preserved
- Merge/Rebase gating mirrors today EXACTLY: present only when `cur !== null`, disabled on `gate`.
- Checkout/Delete disabled on `gate`.
- Compare with HEAD is read-only → NOT gated on `gate`; only unavailable when HEAD is unborn.
- Because item 4 resolves the ref's **tip** by name from the snapshot and reuses the EXISTING
  `compareWithHead(repoId, tip)` command, the graph never has to pass a tip and
  `GraphContextTarget` stays `{kind:'ref';ref:RefLabel} | {kind:'commit';index;oid}`.

### 1.4 FLAG — fixture/name alignment (must-do in P6b, see §3.5)
`branchMenuItems` returns `[]` for a ref **name not present** in the snapshot. The current
graph fixture (`src/ipc/fixtures/graph.ts`) draws pills named `feat`, `exp`, `gh-pages` that are
**absent** from `INITIAL_BRANCHES`. Without alignment the P5 harness demo (right-click the `feat`
pill → Merge/Rebase) regresses to "no menu". P6b MUST add these as local branches (with `tip`s)
to the snapshot so every graph pill resolves. This is the least-disruptive fix (the layout
fixture is shared with the 20k perf path; renaming its pills is riskier than extending the
snapshot).

---

## §2 Backend (Rust)

### 2.1 `tip` field additions — `src-tauri/src/git/branches.rs`
Add a full-40-char-hex tip oid to both structs:

```rust
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BranchInfo {
    pub name: String,
    pub is_head: bool,
    pub upstream: Option<String>,
    pub ahead: Option<u32>,
    pub behind: Option<u32>,
    /// Full 40-char hex oid of the branch tip.
    pub tip: String,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteBranchInfo {
    pub name: String,
    /// Full 40-char hex oid of the remote-tracking branch tip.
    pub tip: String,
}
```

`list_refs` changes:
- **Local loop:** the loop already reads `branch.get().target()` for ahead/behind. Compute the
  tip once up front:
  ```rust
  let tip = match branch.get().target() {
      Some(oid) => oid.to_string(),
      None => { eprintln!("bonsai: skipping symbolic/targetless local branch"); continue; }
  };
  ```
  (Direct local branches always have a target; the `continue` is a defensive skip, consistent
  with the existing non-UTF-8 skip. Reuse this `Oid` for the existing ahead/behind
  `local_oid` computation to avoid a second `target()` call.) Add `tip` to the pushed `BranchInfo`.
- **Remote loop:** after the existing symbolic skip + `name` extraction:
  ```rust
  let tip = match branch.get().target() {
      Some(oid) => oid.to_string(),
      None => { eprintln!("bonsai: skipping targetless remote branch"); continue; }
  };
  ```
  Add `tip` to the pushed `RemoteBranchInfo`.

No change to sort order, symbolic-skip, or ahead/behind logic.

### 2.2 `checkout_remote` — new pure fn in `branches.rs`

```rust
/// Blocking. GitKraken-style remote checkout: create (or reuse) a LOCAL tracking
/// branch for the remote-tracking ref `remote_shorthand` ("<remote>/<branch>")
/// and safe-checkout it. SAFE checkout only — never force.
pub fn checkout_remote(workdir: &Path, remote_shorthand: &str) -> Result<(), AppError>;
```

Behavior (normative, no bodies):
1. Open repo (`open_repo_at`).
2. **Split** `remote_shorthand` on the FIRST `'/'` (remote names contain no `/`): `remote` =
   before, `local_name` = after. If there is no `'/'`, or either side is empty →
   `AppError::InvalidName(format!("invalid remote branch name: '{remote_shorthand}'"))`.
3. **Find the remote ref:** `repo.find_branch(remote_shorthand, BranchType::Remote)`; `NotFound`
   → `AppError::BranchNotFound(format!("remote-tracking branch '{remote_shorthand}' not found"))`.
   Get its tip: `let remote_tip = remote_branch.get().target().ok_or_else(|| AppError::Git(...))?;`.
4. **Decide the checkout target + whether we create** (compute BEFORE touching the worktree so a
   conflict leaves everything untouched and creates nothing):
   - If `repo.find_branch(local_name, BranchType::Local)` is `Ok(existing)` → **name collision =
     just switch to the existing local branch** (simplest safe behavior; do NOT repoint it).
     `checkout_oid = existing.get().target().ok_or(Git)?`; `created = false`.
   - If `NotFound` → `checkout_oid = remote_tip`; `created = true`.
   - Other error → propagate as `Git`.
5. **Safe checkout FIRST** (matches `checkout_branch`, so a conflict leaves HEAD + worktree
   untouched AND nothing was created yet): `let obj = repo.find_object(checkout_oid, None)?;`
   `opts.safe();` `repo.checkout_tree(&obj, Some(&mut opts))`; on `ErrorCode::Conflict` →
   `AppError::CheckoutConflict(format!("cannot switch to '{local_name}': local changes would be \
   overwritten. Commit or discard them first."))` (SAME message shape as `checkout_branch`).
6. **On checkout success, only now mutate refs:**
   - If `created`: `repo.branch(local_name, &remote_commit, false)`; then
     `new_branch.set_upstream(Some(remote_shorthand))?` (best-effort — an upstream-set failure is
     still a successful checkout: log via `eprintln!` and continue, do NOT roll back). If
     `repo.branch` returns `ErrorCode::Exists` (race), proceed to `set_head`.
   - `repo.set_head(&format!("refs/heads/{local_name}"))?`.
7. Return `Ok(())`.

Error taxonomy: `InvalidName` (malformed shorthand) | `BranchNotFound` (remote ref missing) |
`CheckoutConflict` (dirty worktree) | `Git` (targetless ref, unexpected libgit2 errors) |
(command layer adds `NoRepo`).

### 2.3 `delete_remote_tracking` — new pure fn in `branches.rs`

```rust
/// Blocking. Deletes the LOCAL remote-tracking ref `name` ("origin/feature").
/// Local-only: does NOT contact the server. No merged-check (a local-branch
/// concept only).
pub fn delete_remote_tracking(workdir: &Path, name: &str) -> Result<(), AppError>;
```

Behavior:
1. Open repo.
2. `let mut branch = repo.find_branch(name, BranchType::Remote)`; `NotFound` →
   `AppError::BranchNotFound(format!("remote-tracking branch '{name}' not found"))`; other → `Git`.
3. `branch.delete()?;` (libgit2 removes only the local `refs/remotes/...` ref).
4. `Ok(())`.

No head/merged gates. Error taxonomy: `BranchNotFound` | `Git` | (command layer adds `NoRepo`).

### 2.4 New commands — `src-tauri/src/commands.rs`
Follow the `checkout_branch` / `delete_branch` pattern exactly: thin `#[tauri::command]` + a
runtime-free `_inner` that resolves `repo_path` then runs the pure fn under `spawn_blocking`.

```rust
/// GitKraken-style remote checkout: create/reuse a local tracking branch for
/// `name` ("<remote>/<branch>") and safe-checkout it (P6 §2.2).
/// Errors: invalidName | branchNotFound | checkoutConflict | git | noRepo.
#[tauri::command]
pub async fn checkout_remote(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    name: String,
) -> Result<(), AppError>;

/// Deletes the LOCAL remote-tracking ref `name` — does NOT touch the server
/// (P6 §2.3). Errors: branchNotFound | git | noRepo.
#[tauri::command]
pub async fn delete_remote_tracking(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    name: String,
) -> Result<(), AppError>;
```

`_inner` bodies mirror `checkout_branch_inner` / `delete_branch_inner` (resolve `repo_path`,
`spawn_blocking(move || branches::checkout_remote(&path, &name))`, map join error to `Other`).
Add both to `use crate::git::branches::{...}` if the fns are referenced by path (they are called
as `branches::checkout_remote` / `branches::delete_remote_tracking`, so no `use` change needed).

### 2.5 Registration — `src-tauri/src/lib.rs`
Add to `tauri::generate_handler![ … ]`, next to the existing branch commands:
```
            commands::checkout_remote,
            commands::delete_remote_tracking,
```

### 2.6 No new `AppError` variants
All failures map to EXISTING variants: `InvalidName`, `BranchNotFound`, `CheckoutConflict`,
`Git`, `NoRepo`. Do not add variants.

---

## §3 IPC surface (TypeScript)

### 3.1 Type deltas — `src/ipc/types.ts`
Add `tip` to both interfaces (place after the existing fields):
```ts
export interface BranchInfo {
  name: string;
  isHead: boolean;
  upstream: string | null;
  ahead: number | null;
  behind: number | null;
  /** Full 40-char hex oid of the branch tip. */
  tip: string;
}

export interface RemoteBranchInfo {
  /** Shorthand incl. remote, e.g. "origin/main". */
  name: string;
  /** Full 40-char hex oid of the remote-tracking branch tip. */
  tip: string;
}
```

### 3.2 New `IpcApi` methods — `src/ipc/types.ts`
Add after `deleteBranch`:
```ts
  /** GitKraken-style remote checkout: create/reuse a local tracking branch for
   *  `name` ("<remote>/<branch>") and switch to it. Rejects
   *  invalidName | branchNotFound | checkoutConflict | git | noRepo. */
  checkoutRemoteBranch(repoId: string, name: string): Promise<void>;
  /** Delete the LOCAL remote-tracking ref `name` (does NOT touch the server).
   *  Rejects branchNotFound | git | noRepo. */
  deleteRemoteBranch(repoId: string, name: string): Promise<void>;
```

### 3.3 Real impl — `src/ipc/tauri.ts`
Add two wrappers after `deleteBranch` (method-shorthand style, matching the file; Tauri v2 maps
camelCase JS keys to the snake_case Rust params):
```ts
  checkoutRemoteBranch(repoId: string, name: string): Promise<void> {
    return invoke<void>('checkout_remote', { repoId, name });
  },

  deleteRemoteBranch(repoId: string, name: string): Promise<void> {
    return invoke<void>('delete_remote_tracking', { repoId, name });
  },
```

### 3.4 Barrel — `src/ipc/index.ts`
No new type export needed (`BranchInfo`/`RemoteBranchInfo` are already re-exported; the two new
methods return `void`). Adding `tip` is an in-place field change.

### 3.5 Mock + fixtures — `src/ipc/mock.ts`, `src/ipc/fixtures/branches.ts`

**Fixture deltas (`fixtures/branches.ts`)** — add `tip` to every branch AND align names with the
graph fixture so all pills resolve (§1.4). Use distinct 40-hex strings; keep `MOCK_OID` as the
HEAD/`main` tip so "Compare with HEAD" on a same-tip branch shows the "No differences" state:

```ts
export const INITIAL_BRANCHES: BranchesSnapshot = {
  local: [
    { name: 'main', isHead: true, upstream: 'origin/main', ahead: 0, behind: 0, tip: MOCK_OID },
    { name: 'feature/sidebar', isHead: false, upstream: 'origin/feature/sidebar', ahead: 2, behind: 1,
      tip: 'a'.repeat(40) },
    { name: 'fix/watcher-debounce', isHead: false, upstream: null, ahead: null, behind: null,
      tip: 'b'.repeat(40) },
    { name: 'experiment-unmerged', isHead: false, upstream: null, ahead: null, behind: null,
      tip: 'c'.repeat(40) },
    // Graph-fixture pill names (§1.4) so their right-click menus resolve:
    { name: 'feat', isHead: false, upstream: null, ahead: null, behind: null, tip: 'd'.repeat(40) },
    { name: 'exp', isHead: false, upstream: null, ahead: null, behind: null, tip: 'e'.repeat(40) },
    { name: 'gh-pages', isHead: false, upstream: null, ahead: null, behind: null, tip: 'f'.repeat(40) },
  ],
  remote: [
    { name: 'origin/main', tip: MOCK_OID },
    { name: 'origin/feature/sidebar', tip: 'a'.repeat(40) },
    // A remote with NO matching local, so the harness exercises the create-and-switch path:
    { name: 'origin/release', tip: '1'.repeat(40) },
  ],
  tags: ['v0.1.0', 'v0.2.0'],
  head: { branchName: 'main', oid: MOCK_OID, detached: false, unborn: false },
};
```
(Any distinct hex works; the important constraints are: `main.tip === MOCK_OID`; every graph-pill
name has a local entry; at least one remote has no matching local. `createBranch`/`push` mock
paths that build new `BranchInfo`/`RemoteBranchInfo` objects MUST also set `tip` — use
`randomOid()` for created locals and the local's `tip` for a newly-tracked remote.)

**Mock methods (`mock.ts`)** — add after `deleteBranch`, internally consistent with the existing
per-repo `MockRepoState`:

```ts
async checkoutRemoteBranch(repoId: string, name: string): Promise<void> {
  await delay(150);
  const state = requireRepo(repoId);
  const remote = state.branches.remote.find((r) => r.name === name);
  if (remote === undefined) {
    const err: AppError = { kind: 'branchNotFound', message: `remote-tracking branch '${name}' not found` };
    throw err;
  }
  const slash = name.indexOf('/');
  const localName = slash === -1 ? name : name.slice(slash + 1);
  let local = state.branches.local.find((b) => b.name === localName);
  if (local === undefined) {
    // Create-and-track path.
    local = { name: localName, isHead: false, upstream: name, ahead: 0, behind: 0, tip: remote.tip };
    state.branches.local.push(local);
    state.branches.local.sort((a, b) => a.name.toLowerCase().localeCompare(b.name.toLowerCase()));
  }
  // Switch HEAD (reuse the checkoutBranch state transition).
  for (const b of state.branches.local) b.isHead = false;
  local.isHead = true;
  state.headBranch = local.name;
  state.headOid = local.tip;
  state.branches.head = { branchName: local.name, oid: state.headOid, detached: false, unborn: false };
},

async deleteRemoteBranch(repoId: string, name: string): Promise<void> {
  await delay(150);
  const state = requireRepo(repoId);
  const remote = state.branches.remote.find((r) => r.name === name);
  if (remote === undefined) {
    const err: AppError = { kind: 'branchNotFound', message: `remote-tracking branch '${name}' not found` };
    throw err;
  }
  state.branches.remote = state.branches.remote.filter((r) => r.name !== name);
},
```
(Add `BranchInfo`/`RemoteBranchInfo` to the `mock.ts` type imports only if referenced; the object
literals above don't require them.)

---

## §4 Frontend (React)

### 4.1 `branchMenuItems` builder — `src/components/RepoWorkspace.tsx`
A single function defined inside `RepoWorkspace` (closes over `branches`, `headBranch`, `head`,
`mutating`, `opActive`, and the handlers). Signature:
```ts
function branchMenuItems(name: string, kind: 'localBranch' | 'remoteBranch'): ContextMenuItem[];
```
Construction:
1. `const snapshot = branches; if (snapshot === null) return [];`
2. `const cur = headBranch?.name ?? null;` `const gate = mutating || opActive;`
   `const headUnborn = head === null || head.unborn;`
3. Resolve the entry: `localBranch` → `snapshot.local.find(b => b.name === name)`; `remoteBranch`
   → `snapshot.remote.find(r => r.name === name)`. If `undefined` → **return `[]`** (missing entry).
4. `const isHead = kind === 'localBranch' ? (entry as BranchInfo).isHead : false;`
   if `isHead` → **return `[]`** (current branch's own pill — no actions).
5. `const tip = entry.tip;` Build the array in §1.2 order:
   - `{ label: 'Checkout', disabled: gate, onSelect: () => void (kind === 'remoteBranch' ? handleCheckoutRemote(name) : handleCheckoutBranch(name)) }`
   - if `cur !== null`: `{ label: `Merge ${name} into ${cur}`, disabled: gate, onSelect: () => void handleMergeBranch(name) }`
   - if `cur !== null`: `{ label: `Rebase ${cur} onto ${name}`, disabled: gate, onSelect: () => void handleRebaseBranch(name) }`
   - if `!headUnborn`: `{ label: 'Compare with HEAD', disabled: false, onSelect: () => handleCompareWithHead(tip) }`
   - `{ label: 'Delete', disabled: gate, onSelect: () => (kind === 'remoteBranch' ? setPendingDeleteRemote(name) : setPendingDeleteBranch(name)) }`

Empty-return cases (menu does not open): `branches === null`; entry not found; local branch is
the current HEAD.

### 4.2 Graph wiring — rewrite the `kind: 'ref'` case of `buildContextItems`
Replace today's inline merge/rebase-only block (RepoWorkspace ~967-997) with a delegation to the
shared builder; keep the tag/head short-circuit and the `kind: 'commit'` case unchanged:
```ts
function buildContextItems(target: GraphContextTarget): ContextMenuItem[] {
  if (target.kind === 'ref') {
    const r = target.ref;
    if (r.kind === 'tag' || r.kind === 'head') return [];
    return branchMenuItems(r.name, r.kind === 'remoteBranch' ? 'remoteBranch' : 'localBranch');
  }
  if (head === null || head.unborn) return [];
  return [
    { label: 'Compare with HEAD', disabled: false, onSelect: () => handleCompareWithHead(target.oid) },
  ];
}
```
`handleGraphContextMenu` and `closeMenu` are unchanged. The commit-row Compare still passes the
commit's own oid (`target.oid`); the ref Compare uses the tip resolved inside `branchMenuItems`.

### 4.3 Sidebar context-menu wiring — new handler in RepoWorkspace
```ts
function handleSidebarContextMenu(
  name: string,
  kind: 'localBranch' | 'remoteBranch',
  clientX: number,
  clientY: number,
) {
  const items = branchMenuItems(name, kind);
  if (items.length === 0) return; // e.g. the current branch → no menu
  setMenu({ x: clientX, y: clientY, items });
}
```
Passed to `<Sidebar onContextMenu={handleSidebarContextMenu} … />`. The same `ContextMenu`
render at the end of the tree (RepoWorkspace ~1327) serves both surfaces — no change.

### 4.4 New RepoWorkspace handlers (mirror the existing branch handlers)
```ts
async function handleCheckoutRemote(name: string) {
  setBranchesError(null);
  setMutating(true);
  try {
    await ipc.checkoutRemoteBranch(repoId, name);
    await refreshAll();
  } catch (e) {
    setBranchesError(errorMessage(e));
  } finally {
    setMutating(false);
  }
}

async function handleDeleteRemoteTracking(name: string) {
  setBranchesError(null);
  setMutating(true);
  try {
    await ipc.deleteRemoteBranch(repoId, name);
    await Promise.all([refetchBranches(), refetchGraph()]);
  } catch (e) {
    setBranchesError(errorMessage(e));
  } finally {
    setMutating(false);
  }
}
```
`handleCheckoutRemote` uses `refreshAll` (HEAD moves, like `handleCheckoutBranch`);
`handleDeleteRemoteTracking` refetches branches+graph (like `handleDeleteBranch`). The existing
`handleMergeBranch` / `handleRebaseBranch` / `handleCheckoutBranch` / `handleDeleteBranch` /
`handleCompareWithHead` are reused as-is.

### 4.5 Moved + new confirm dialogs — RepoWorkspace
Replace the `const [dialogOpen, setDialogOpen] = useState(false)` state with two pending-delete
states, and make `dialogOpen` **derived** so the existing shortcut effect (RepoWorkspace ~1056,
`if (dialogOpen || abortConfirmOpen) return;`) keeps suppressing shortcuts while either delete
dialog is up:
```ts
const [pendingDeleteBranch, setPendingDeleteBranch] = useState<string | null>(null);
const [pendingDeleteRemote, setPendingDeleteRemote] = useState<string | null>(null);
const dialogOpen = pendingDeleteBranch !== null || pendingDeleteRemote !== null;
```
Add both dialogs near the existing `abortConfirmOpen` `ConfirmDialog` (~1298). Copy verbatim:

**Local branch delete** (moved from Sidebar, unchanged wording):
```tsx
<ConfirmDialog
  open={pendingDeleteBranch !== null}
  title="Delete branch"
  confirmLabel="Delete branch"
  busy={mutating}
  onConfirm={() => {
    const name = pendingDeleteBranch;
    setPendingDeleteBranch(null);
    if (name !== null) void handleDeleteBranch(name);
  }}
  onCancel={() => setPendingDeleteBranch(null)}
>
  <div>Delete branch "<span className="mono">{pendingDeleteBranch ?? ''}</span>"?</div>
  <div className="dialog-body-note">
    The branch is fully merged, but this cannot be undone from Bonsai.
  </div>
</ConfirmDialog>
```

**Remote-tracking delete** (new, distinct wording making the local-only scope clear):
```tsx
<ConfirmDialog
  open={pendingDeleteRemote !== null}
  title="Delete remote-tracking reference"
  confirmLabel="Delete reference"
  busy={mutating}
  onConfirm={() => {
    const name = pendingDeleteRemote;
    setPendingDeleteRemote(null);
    if (name !== null) void handleDeleteRemoteTracking(name);
  }}
  onCancel={() => setPendingDeleteRemote(null)}
>
  <div>Delete the remote-tracking reference "<span className="mono">{pendingDeleteRemote ?? ''}</span>"?</div>
  <div className="dialog-body-note">
    This removes only Bonsai's local copy of the remote branch. It does NOT delete the branch on
    the server — a future fetch may recreate it.
  </div>
</ConfirmDialog>
```
`busy={mutating}` matches `ConfirmDialogProps` (which has no `opActive`); the menu item itself is
already gated on `gate` before the dialog can open.

### 4.6 Sidebar prop/API changes — `src/components/Sidebar.tsx`

`SidebarProps` **keeps:** `data`, `loading`, `error`, `onDismissError`, `busy`, `opActive`,
`currentBranch`, `onCheckout` (double-click only), `onCreateBranch`, `width`, `listView`.

`SidebarProps` **removes:** `onMergeBranch`, `onRebaseBranch`, `onDelete`, `onDialogOpenChange`.

`SidebarProps` **adds:**
```ts
  /** Right-click a branch/remote row → open the shared context menu at the cursor. */
  onContextMenu(name: string, kind: 'localBranch' | 'remoteBranch', clientX: number, clientY: number): void;
```

Component-internal removals:
- Delete `pendingDelete` state, the `useEffect(() => onDialogOpenChange?.(...))`, and the trailing
  `<ConfirmDialog>` (all moved to RepoWorkspace §4.5).
- Delete `TrashIcon`.
- `BranchRow`: remove the `onMerge`/`onRebase`/`onAskDelete` props and the entire
  `{!branch.isHead && (<> …buttons… </>)}` block (checkout/merge/rebase/trash). Keep the glyph,
  `.branch-name` (now the only flex child besides the badge), `AheadBehindBadge`, and the
  double-click checkout (`onDoubleClick` → `if (!branch.isHead && !busy) onCheckout(branch.name)`).
  Add right-click: on the `<li>`, `onContextMenu={(e) => { e.preventDefault(); onContextMenu(branch.name, 'localBranch', e.clientX, e.clientY); }}`.
- `RemoteRow`: remove the `onMerge`/`onRebase` props and both buttons. Add right-click on the
  `<li>`: `onContextMenu={(e) => { e.preventDefault(); onContextMenu(name, 'remoteBranch', e.clientX, e.clientY); }}`. No double-click on remote rows (checkout would silently create a branch — right-click only). FLAG: minor UX choice; can add later.
- `TagRow`: unchanged (no menu, no right-click handler).

Wiring: `BranchRow`/`RemoteRow` receive `onContextMenu` in BOTH flat mode and tree mode
(`renderLeaf`). Thread the single `onContextMenu` prop straight through; the row components add
`e.preventDefault()`. The detached-HEAD synthetic row (`branch-row-detached`) gets no menu.

Sidebar render call in RepoWorkspace (~1177-1194): drop `onMergeBranch`, `onRebaseBranch`,
`onDelete`, `onDialogOpenChange`; add `onContextMenu={handleSidebarContextMenu}`. Keep
`onCheckout={(name) => void handleCheckoutBranch(name)}`.

### 4.7 CSS — `src/styles.css`
- The inline buttons are gone, so `.branch-name` (already `flex: 1; min-width: 0; ellipsis`)
  naturally fills the freed horizontal space — no rule change needed for full-width names.
- **Remove** the now-dead branch-scoped row-action rules: `.branch-row:hover .row-action`,
  `.branch-row .row-action:focus-visible`, and `.branch-row .row-action { … }` (~lines 1761-1771).
  Keep the base `.row-action` rules — `.row-action.conflict-action` still uses them.
- Optional affordance (recommended): add `.branch-row { user-select: none; }` so a right-click
  drag doesn't select the label text. The `ContextMenu` component + its `.context-menu*` classes
  are reused unchanged.

---

## §5 Acceptance criteria

### AI gate (orchestrator-verifiable)
1. `cargo check` + `cargo clippy` clean; `pnpm build` + `tsc` clean.
2. **Rust tests** (git2-init + identity, like the existing `branches.rs` tests; scratch repos
   under `D:\Temp\bonsai-scratch`, `TMP/TEMP=D:\Temp`), each asserting against the git CLI oracle:
   - `list_refs`: `BranchInfo.tip` / `RemoteBranchInfo.tip` equal `git rev-parse <ref>` for local
     and remote-tracking refs.
   - `checkout_remote` create path: a repo with a remote-tracking `origin/topic` and NO local
     `topic` → after `checkout_remote(dir, "origin/topic")`, `git symbolic-ref HEAD` is
     `refs/heads/topic`, `git rev-parse topic` == the remote tip, and `git config
     branch.topic.remote/merge` (upstream) is set to `origin`/`refs/heads/topic`.
   - `checkout_remote` existing-local path: local `topic` already exists at a DIFFERENT oid →
     switches to it WITHOUT repointing (`git rev-parse topic` unchanged); returns `Ok`.
   - `checkout_remote` conflict: dirty worktree that a safe checkout would overwrite →
     `AppError::CheckoutConflict`, HEAD + worktree unchanged, and **no** new local branch created
     (assert `git branch --list topic` empty).
   - `checkout_remote` errors: shorthand with no `/` → `InvalidName`; unknown remote ref →
     `BranchNotFound`.
   - `delete_remote_tracking`: after deletion `git branch -r --list origin/topic` is empty AND the
     remote's own refs are untouched (delete a `file://` bare-remote-tracking ref, then
     `git --git-dir=<bare> show-ref` still lists the server branch); unknown ref → `BranchNotFound`.
   - `commands.rs`: `checkout_remote_inner` / `delete_remote_tracking_inner` return `NoRepo` for an
     unknown id (extend the existing `branch_commands_require_an_open_repo` test).
   - Run `cargo test --lib` (+ the `branches_cli` integration test) — `pnpm tauri dev` may hold the
     full test-bin link; do NOT use the tauri `test` feature (STATUS_ENTRYPOINT_NOT_FOUND); inners
     are runtime-free. Do not run `cargo test` concurrently with `clippy`.
3. **Browser harness** (`pnpm dev`, `VITE_MOCK_IPC=1`), BOTH dark + light themes, no console errors:
   - Sidebar rows show glyph + full-width name (+ badge) with **no** inline action buttons; long
     names ellipsize using the full freed width.
   - Right-click a local non-current row AND its graph pill → identical 5-item menu
     (Checkout / Merge … / Rebase … / Compare with HEAD / Delete), same order + wording.
   - Right-click the current/HEAD branch (row + pill) → **no menu opens**.
   - Right-click a tag (row + pill) → **no menu**.
   - Right-click a remote-tracking row AND its `origin/main` pill → 5-item remote menu; `Checkout`
     on `origin/release` (no matching local) creates+switches (header branch updates to `release`);
     `Checkout` on `origin/main` switches to existing local `main`.
   - `Compare with HEAD` from a branch menu opens ComparePanel; same-tip branch (`main`/`origin/main`)
     shows the "No differences" state.
   - `Delete` on a local branch opens the "Delete branch" confirm; `Delete` on a remote row opens
     the distinct "Delete remote-tracking reference" confirm with the local-only wording; confirming
     the remote delete removes the row.
   - Merge/Rebase items are disabled while a paused op is active (`?op=merge` / `?op=rebase` tab);
     Compare stays enabled.
   - Double-click a local branch row still checks it out; global Esc/refresh shortcuts are inert
     while either delete confirm is open.

### USER CHECKPOINT (native `pnpm tauri dev`)
- Right-click a remote-tracking pill/row → `Checkout` creates a local tracking branch from the
  remote ref and switches to it (upstream configured); the graph/sidebar/header update.
- Right-click a remote-tracking pill/row → `Delete` removes ONLY the local remote-tracking ref
  (confirmed via the CLI that the server branch is untouched); a fetch can bring it back.
- The unified menu behaves identically invoked from the graph pill and the sidebar row for
  checkout / merge / rebase / compare / delete on both local and remote refs.

---

## §6 Decomposition (implement → review → commit)

- **P6a — Backend.** `branches.rs`: `tip` on `BranchInfo`/`RemoteBranchInfo` + `list_refs` wiring;
  new `checkout_remote` + `delete_remote_tracking`; `commands.rs` two commands + runtime-free
  inners + extend the `NoRepo` test; `lib.rs` handler registration; Rust tests (§5.2).
  *Files:* `src-tauri/src/git/branches.rs`, `src-tauri/src/commands.rs`, `src-tauri/src/lib.rs`,
  `src-tauri/tests/branches_cli.rs` (or the existing branch test file).
  *Gate:* `cargo test --lib` + integration tests green; clippy clean.
- **P6b — IPC + mock + fixtures.** `types.ts` (`tip` on both interfaces; two `IpcApi` methods);
  `tauri.ts` wrappers; `mock.ts` two methods; `fixtures/branches.ts` (`tip`s + `feat`/`exp`/
  `gh-pages` locals + `origin/release` remote); set `tip` in the `createBranch`/`push` mock paths.
  *Files:* `src/ipc/types.ts`, `src/ipc/tauri.ts`, `src/ipc/mock.ts`, `src/ipc/fixtures/branches.ts`.
  *Gate:* `tsc`/build green; harness `listBranches` returns `tip`s; the two new methods callable.
- **P6c — Shared builder + confirms + graph wiring.** `RepoWorkspace.tsx`: `branchMenuItems`;
  rewrite `buildContextItems` ref-case; `handleSidebarContextMenu`; `handleCheckoutRemote` +
  `handleDeleteRemoteTracking`; moved local-delete confirm + new remote-delete confirm; derived
  `dialogOpen`. (Graph pills exercise the full new menu already at this step.)
  *Files:* `src/components/RepoWorkspace.tsx`.
  *Gate:* harness — every graph-pill menu item (local + remote), Compare/No-differences, both
  confirm dialogs, shortcut suppression; `tsc` green.
- **P6d — Sidebar strip + right-click + CSS.** `Sidebar.tsx` prop changes (add `onContextMenu`;
  remove `onMergeBranch`/`onRebaseBranch`/`onDelete`/`onDialogOpenChange`); strip `BranchRow`/
  `RemoteRow` buttons + `TrashIcon` + internal `ConfirmDialog`/`pendingDelete`; wire right-click in
  flat + tree modes; update the Sidebar render in `RepoWorkspace.tsx`; CSS cleanup + full-width names.
  *Files:* `src/components/Sidebar.tsx`, `src/components/RepoWorkspace.tsx`, `src/styles.css`.
  *Gate:* harness — sidebar rows full-width, no inline buttons, right-click parity with the graph,
  double-click checkout intact, both themes, no console errors.

---

## §7 Open items to FLAG for the orchestrator
1. **Fixture/name alignment (§1.4)** is mandatory in P6b, otherwise the P5 `feat`/`exp` graph-pill
   demo regresses. Chosen fix: extend `INITIAL_BRANCHES` (not rename graph pills).
2. **Remote-row double-click (§4.6):** disabled (right-click only) because a double-click would
   silently create+switch a branch. Trivial to enable later if the user wants GitKraken parity.
3. **`checkout_remote` name collision (§2.2 step 4):** simplest safe behavior chosen — switch to
   the existing local branch without repointing it. Alternative (error on divergent local) is more
   conservative but noisier; recommend keeping the switch-and-go behavior.
