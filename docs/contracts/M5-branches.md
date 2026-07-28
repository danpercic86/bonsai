# M5 — Branches: Implementation Contract

Status: authoritative for M5. Implementer: senior-dev. Builds on `M0-scaffold.md` (error shape,
IPC conventions), `M1-status.md` (watcher/refetch patterns), `M2-graph.md` (RefLabel pills —
unchanged by M5), `M3-commit.md` (mutation command pattern, imperative refetch, stateful mock),
`M4-diff.md` (right-panel modes — unchanged), `ui-reference.md` §1 (sidebar geometry: fixed
240px, sections "Branches" / "Remotes" / "Tags", 11px uppercase headers).

Scope (locked): list local branches + remote-tracking branches + tags in the left sidebar;
create branch from current HEAD; checkout a local branch; delete a local branch. Show current
branch / detached HEAD. **No** rename, no remote-branch deletion, no stash, no force-delete.

Guardrail: **branch delete is destructive → explicit UI confirmation dialog is mandatory**
(§5.5). Checkout is safe-only (git2 default safe checkout; never force — §2.5).

---

## 1. New / changed files

```
src-tauri/
  src/error.rs                 # + BranchExists, InvalidName, CheckoutConflict,
                               #   UnmergedBranch, BranchNotFound variants
  src/git/repo.rs              # read_head_info: fn -> pub(crate) fn (reused by branches.rs)
  src/git/branches.rs          # NEW: list_refs / create_branch / checkout_branch /
                               #      delete_branch + CLI-oracle tests   (git/mod.rs: pub mod branches;)
  src/commands.rs              # + list_branches, create_branch, checkout_branch, delete_branch
  src/lib.rs                   # register the four commands
src/
  ipc/types.ts                 # + BranchInfo, RemoteBranchInfo, BranchesSnapshot; IpcApi +4;
                               #   AppError kind union +5
  ipc/tauri.ts                 # + 4 wrappers
  ipc/mock.ts                  # stateful branch mock (create/checkout/delete mutate state)
  ipc/fixtures/branches.ts     # NEW: INITIAL_BRANCHES fixture
  components/Sidebar.tsx       # NEW: sections, rows, actions, create-input, delete dialog
  components/ConfirmDialog.tsx # NEW: small reusable modal (danger-styled confirm)
  App.tsx                      # branches state + refetch wiring + op handlers
  styles.css                   # sidebar rows/badges, dialog overlay, danger button
```

## 2. Rust backend — `src-tauri/src/git/branches.rs`

All functions blocking, repo opened with `NO_SEARCH` (same as every git/ module); bare/missing
repo → `AppError::Git`/`Io` as usual.

### 2.1 Wire types

```rust
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BranchInfo {
    /// Shorthand, e.g. "main", "feature/sidebar".
    pub name: String,
    /// True for the branch HEAD points at (always false when detached/unborn).
    pub is_head: bool,
    /// Upstream shorthand, e.g. "origin/main"; None when no upstream configured
    /// or the upstream ref is gone.
    pub upstream: Option<String>,
    /// Commits ahead of / behind upstream. None whenever `upstream` is None.
    pub ahead: Option<u32>,
    pub behind: Option<u32>,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteBranchInfo {
    /// Shorthand incl. remote, e.g. "origin/main".
    pub name: String,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BranchesSnapshot {
    /// Sorted case-insensitively by name.
    pub local: Vec<BranchInfo>,
    /// Sorted case-insensitively; symbolic "<remote>/HEAD" entries EXCLUDED.
    pub remote: Vec<RemoteBranchInfo>,
    /// Tag names (lightweight + annotated), sorted case-insensitively.
    pub tags: Vec<String>,
    /// Same shape the header already uses (git/repo.rs HeadInfo) — one source
    /// of truth for attached/detached/unborn in the sidebar.
    pub head: crate::git::repo::HeadInfo,
}
```

**Decision — ahead/behind is IN for v1.** It is one `repo.graph_ahead_behind(local, upstream)`
per branch-with-upstream (merge-base walk on already-loaded odb); repos have tens of branches,
not thousands. It makes the sidebar immediately useful for M6 (fetch/pull). If a lookup errors
(e.g. shallow oddities) set `ahead`/`behind` to `None` — never fail the whole snapshot for it.

### 2.2 `list_refs`

```rust
/// Blocking. One snapshot of local branches, remote-tracking branches, tags, HEAD.
/// - local:  repo.branches(Some(BranchType::Local)); name = shorthand (skip non-UTF-8 with
///           eprintln, never error); is_head = branch.is_head(); upstream = branch.upstream()
///           .ok() shorthand; ahead/behind per §2.1.
/// - remote: repo.branches(Some(BranchType::Remote)); SKIP entries whose reference is
///           symbolic (branch.get().symbolic_target().is_some()) — that is "<remote>/HEAD".
/// - tags:   repo.tag_names(None), skip non-UTF-8 entries.
/// - head:   crate::git::repo::read_head_info(&repo)  (make it pub(crate)).
/// Unborn repo: local/remote/tags empty (or whatever exists), head.unborn = true — Ok, not Err.
pub fn list_refs(workdir: &Path) -> Result<BranchesSnapshot, AppError>;
```

### 2.3 Branch-name validation (used by create; delete/checkout don't need it)

```rust
/// git2::Branch::name_is_valid(name)? == true, plus our stricter pre-checks:
/// trimmed input; empty after trim -> InvalidName. On invalid ->
/// AppError::InvalidName(format!("invalid branch name: '{name}'")).
fn validate_branch_name(name: &str) -> Result<(), AppError>;
```

Validation lives in the **backend only** (authoritative; mirrors `git check-ref-format
--branch`). The frontend does convenience-only gating: create button disabled while the input
is empty/whitespace — everything else round-trips and shows the backend error. Rationale:
duplicating ref-format rules in TS invites drift; the error path is cheap and rare.

### 2.4 `create_branch`

```rust
/// Blocking. Creates local branch `name` at the current HEAD commit. Does NOT check out.
/// 1. validate_branch_name(name)
/// 2. head commit = repo.head()?.peel_to_commit(); UnbornBranch/NotFound ->
///    AppError::Git("cannot create a branch: the repository has no commits yet")
/// 3. repo.branch(name, &head_commit, /*force=*/ false)
///    Err code Exists -> AppError::BranchExists(format!("branch '{name}' already exists"))
pub fn create_branch(workdir: &Path, name: &str) -> Result<(), AppError>;
```

**Decision — create does NOT auto-checkout.** Orthogonal ops keep the confirmation/refetch
story simple, and the row's hover Checkout action is one click away. (GitKraken prompts;
we skip the prompt in v1. If disagreed, checkout-after-create is a frontend-only change:
call `checkout_branch` after `create_branch` succeeds.)

### 2.5 `checkout_branch`

```rust
/// Blocking. Checks out LOCAL branch `name` (v1: local branch names only — no tags, no oids,
/// no remote-tracking checkout; see §9).
/// 1. branch = repo.find_branch(name, BranchType::Local)
///    Err NotFound -> AppError::BranchNotFound(format!("branch '{name}' not found"))
/// 2. if branch.is_head() -> Ok(()) (no-op; UI hides the action but guard the race)
/// 3. obj = branch tip peeled to commit's tree-ish: repo.find_object(target_oid, None)
/// 4. repo.checkout_tree(&obj, Some(CheckoutBuilder::new().safe()))
///    -- DEFAULT SAFE MODE. NEVER .force(). --
///    Err code Conflict -> AppError::CheckoutConflict(format!(
///        "cannot switch to '{name}': local changes would be overwritten. \
///         Commit or discard them first."))
/// 5. repo.set_head(&format!("refs/heads/{name}"))   // only after checkout_tree succeeded
pub fn checkout_branch(workdir: &Path, name: &str) -> Result<(), AppError>;
```

**Decision — dirty-worktree behavior:** git2's safe checkout succeeds when local changes don't
collide with the target tree (same as `git checkout` carrying changes over) and fails with
`Conflict` when they would be overwritten. We surface `checkoutConflict` with the message above
and change **nothing** (checkout_tree is applied before set_head, so a conflict leaves both
worktree and HEAD untouched). No force, no stash in v1.

### 2.6 `delete_branch`

```rust
/// Blocking. Deletes LOCAL branch `name`. Safety gates IN ORDER:
/// 1. branch = repo.find_branch(name, BranchType::Local)
///    Err NotFound -> AppError::BranchNotFound(...)
/// 2. branch.is_head() -> AppError::Git(format!(
///        "cannot delete '{name}': it is the currently checked-out branch"))
///    (pre-check ours; libgit2 would also refuse, but our message is clearer.
///     Race-only backstop — the UI never offers delete on the current branch.)
/// 3. MERGED CHECK (libgit2 deletes unconditionally — `git branch -D` semantics — so WE
///    implement the `-d` safety): let tip = branch tip oid; let head = HEAD commit oid
///    (detached HEAD: the detached commit; unborn HEAD -> treat as unmerged).
///    merged = tip == head || repo.graph_descendant_of(head, tip)?
///    !merged -> AppError::UnmergedBranch(format!(
///        "branch '{name}' is not fully merged into HEAD (tip {short_tip}). \
///         Bonsai v1 does not force-delete; use `git branch -D {name}` if you are sure."))
/// 4. branch.delete()
pub fn delete_branch(workdir: &Path, name: &str) -> Result<(), AppError>;
```

**Decision — unmerged deletion is BLOCKED, no force-delete path in v1.** Rationale: a
force-delete needs a second, visually distinct "scarier" dialog plus reflog-recovery messaging
to be honest — real UX surface for a rare op. Blocking with an actionable CLI hint is safe,
simple, and reviewable. (Rejected alternative: `-D` behind a red double-confirm — deferred to
Polish if users ask.) Merged-against-HEAD matches `git branch -d` default (upstream leniency
NOT replicated — stricter is fine for v1; note it in the error copy? No — message stays as is).

### 2.7 Error variants (`error.rs`)

```rust
#[error("{0}")] BranchExists(String),     // kind() -> "branchExists"
#[error("{0}")] InvalidName(String),      // kind() -> "invalidName"
#[error("{0}")] CheckoutConflict(String), // kind() -> "checkoutConflict"
#[error("{0}")] UnmergedBranch(String),   // kind() -> "unmergedBranch"
#[error("{0}")] BranchNotFound(String),   // kind() -> "branchNotFound"
```

All five carry their full display message (extend `message()`'s `m` arm). Deleting the current
branch deliberately reuses kind `git` (§2.6 step 2) — the UI never branches on it.

### 2.8 Commands (`commands.rs`) + registration

Exact M3 pattern: `_inner` core using `current_repo_path(state)`, then `spawn_blocking`, join
error → `AppError::Other`. None of them emit `repo-changed` (frontend refetches imperatively,
M3 §2.7; the watcher also fires — `.git/HEAD`, `refs/**`, `packed-refs` all pass
`watcher.rs::is_relevant`, verified — and is absorbed by request-id guards).

```rust
#[tauri::command]
pub async fn list_branches(state: tauri::State<'_, AppState>) -> Result<BranchesSnapshot, AppError>;
#[tauri::command]
pub async fn create_branch(state: tauri::State<'_, AppState>, name: String) -> Result<(), AppError>;
#[tauri::command]
pub async fn checkout_branch(state: tauri::State<'_, AppState>, name: String) -> Result<(), AppError>;
#[tauri::command]
pub async fn delete_branch(state: tauri::State<'_, AppState>, name: String) -> Result<(), AppError>;
```

(Naming note: the CLAUDE.md reference sketch says `checkout(name)`; `checkout_branch` is
deliberate — it documents the v1 restriction to local branches at the call site.)

Command surface after M5: `open_repo`, `get_status`, `get_graph`, `stage`, `unstage`, `commit`,
`get_workdir_file_diff`, `get_commit_diff`, `get_commit_file_diff`, `list_branches`,
`create_branch`, `checkout_branch`, `delete_branch`. Events: `repo-changed` (unchanged).
Channels: none.

## 3. IPC layer (TypeScript)

`src/ipc/types.ts`:

```ts
export interface BranchInfo {
  name: string;
  isHead: boolean;
  upstream: string | null;
  ahead: number | null;
  behind: number | null;
}

export interface RemoteBranchInfo {
  name: string;
}

export interface BranchesSnapshot {
  local: BranchInfo[];
  remote: RemoteBranchInfo[];
  tags: string[];
  head: HeadInfo;
}

// AppError kind union becomes:
//   'git' | 'io' | 'other' | 'noRepo' | 'emptyMessage' | 'configMissing' | 'nothingToCommit'
//   | 'branchExists' | 'invalidName' | 'checkoutConflict' | 'unmergedBranch' | 'branchNotFound'

export interface IpcApi {
  // ...existing members unchanged...
  /** Local branches + remotes + tags + HEAD in one snapshot. Rejects noRepo | git. */
  listBranches(): Promise<BranchesSnapshot>;
  /** Create branch at current HEAD (no checkout). Rejects
   *  invalidName | branchExists | git | noRepo. */
  createBranch(name: string): Promise<void>;
  /** Safe checkout of a LOCAL branch. Rejects
   *  branchNotFound | checkoutConflict | git | noRepo. */
  checkoutBranch(name: string): Promise<void>;
  /** Delete a LOCAL, fully merged, non-current branch. Rejects
   *  branchNotFound | unmergedBranch | git | noRepo. */
  deleteBranch(name: string): Promise<void>;
}
```

`src/ipc/tauri.ts`:

```ts
listBranches:   ()     => invoke<BranchesSnapshot>('list_branches'),
createBranch:   (name) => invoke<void>('create_branch', { name }),
checkoutBranch: (name) => invoke<void>('checkout_branch', { name }),
deleteBranch:   (name) => invoke<void>('delete_branch', { name }),
```

No capability changes.

## 4. Frontend

### 4.1 App wiring (`App.tsx`)

- New state: `branches: BranchesSnapshot | null`, `branchesError: string | null`,
  `branchesLoading: boolean`, request-id ref `branchesReqId` (same last-wins guard pattern),
  and `refetchBranches()` / `clearBranches()` mirroring status/graph.
- `refetchBranches` is added everywhere status+graph are already refetched together:
  `handleOpenRepository` success, `handleRefresh`, the `repo-changed` subscription, the
  window-focus subscription, and post-commit refresh in `handleCommit` (commit moves the
  branch tip → ahead counts change).
- Branch operation handlers (all set the existing `mutating` flag — one global busy flag stays
  the rule):

```ts
async function handleCreateBranch(name: string): Promise<void>;  // rethrows (inline input error)
async function handleCheckoutBranch(name: string): Promise<void>;
async function handleDeleteBranch(name: string): Promise<void>;
```

  - `handleCreateBranch`: `await ipc.createBranch(name)` → `await refetchBranches();
    void refetchGraph();` (new pill appears). Errors **rethrow** so the Sidebar's create input
    shows them inline (CommitBox pattern).
  - `handleCheckoutBranch`: `await ipc.checkoutBranch(name)` → then the full refresh: if
    `repoPath !== null` `setRepo(await ipc.openRepo(repoPath))` (header HEAD + watcher
    self-heal, same as post-commit), then
    `await Promise.all([refetchBranches(), refetchStatus(), refetchGraph()])`. Errors →
    `setBranchesError(errorMessage(e))` (banner in the sidebar, §4.2).
  - `handleDeleteBranch`: `await ipc.deleteBranch(name)` →
    `await Promise.all([refetchBranches(), refetchGraph()])`. Errors → `setBranchesError`.
  - All three: `setBranchesError(null)` on entry; `finally { setMutating(false) }`.
- Replace the placeholder `<aside className="sidebar">…</aside>` with `<Sidebar …/>`.

### 4.2 `src/components/Sidebar.tsx` (presentational — no ipc imports)

```ts
export interface SidebarProps {
  data: BranchesSnapshot | null;
  loading: boolean;
  /** Sidebar-level op/list error; rendered as a dismissible banner at the top. */
  error: string | null;
  onDismissError(): void;
  busy: boolean; // global mutating flag — disables every action
  onCheckout(name: string): void;
  /** Called ONLY after the confirmation dialog is confirmed (§5.5). */
  onDelete(name: string): void;
  /** Resolves on success (input clears+closes); rejects with AppError (shown inline). */
  onCreateBranch(name: string): Promise<void>;
}
```

Layout (ui-reference §1: 240px, `--bg-1`, right border):

- **Error banner** (top, when `error !== null`): existing `.error-banner` styling, ✕ dismiss
  button → `onDismissError()`.
- **Section "Branches"** (header: `BRANCHES` 11px uppercase text-3, right-aligned `+` icon
  button 20×20, `aria-label="Create branch"`, hidden while `data.head.unborn`).
  - **Detached HEAD row** (only when `data.head.detached`): pinned first, non-interactive:
    `◎ HEAD detached @ <short oid>` in `--warning`-tinted text, `title` = full oid.
  - One row per `local` branch: branch glyph `⎇` (12px, text-3), name (13px, truncated with
    `title`), and when `isHead`: name in 600 weight + `--accent` color and a small `●` dot
    replacing the glyph. Right-aligned when `upstream !== null` and (`ahead>0 || behind>0`):
    badge `↑{ahead} ↓{behind}` (11px mono, text-3; omit whichever half is 0).
  - **Hover-revealed row actions** (M3 §4.2 pattern: `opacity:0` → 1 on `:hover` and
    `:focus-visible`, real `<button>`s with `aria-label`): checkout button `⇄`
    (`"Checkout <name>"`) and delete button `🗑` rendered as an SVG/`✕`-style 20×20 icon
    (`"Delete <name>"`). Both hidden on the `isHead` row (nothing to do / never deletable).
    **Double-click on a non-head row also triggers checkout** (GitKraken muscle memory).
  - Delete button → opens the confirmation dialog (§5.5); only its Confirm calls `onDelete`.
- **Create-branch input**: clicking `+` inserts an inline row at the top of the Branches list:
  `<input>` (12px, `--bg-2`, placeholder `"new-branch-name"`, autofocus). Enter → if trimmed
  non-empty, `await onCreateBranch(trimmed)`; success closes+clears; rejection shows the
  AppError `message` under the input in `--danger` 11px (kinds `invalidName`/`branchExists`
  arrive pre-worded from the backend). Esc or blur-with-empty closes. Input + Enter disabled
  while `busy`.
- **Section "Remotes"**: rows `origin/main` etc. (glyph `☁` or plain, text-2). **Read-only in
  M5** — no actions, no checkout (see §9). Empty → muted `"No remotes"` line.
- **Section "Tags"**: rows with `#` glyph, read-only. Empty → muted `"No tags"`.
- Sections collapsible (chevron in header, local `useState`, default all expanded). Lists
  scroll (`overflow-y: auto` on the sidebar body).
- `data === null && loading` → three skeleton/muted placeholder rows; `data === null && !loading`
  → nothing.

### 4.3 `src/components/ConfirmDialog.tsx`

```ts
export interface ConfirmDialogProps {
  open: boolean;
  title: string;
  /** Body content; branch names rendered in <span className="mono">. */
  children: React.ReactNode;
  confirmLabel: string;   // rendered as the danger-styled button
  busy: boolean;
  onConfirm(): void;
  onCancel(): void;
}
```

Fixed overlay (`rgba(0,0,0,.45)`), centered card (`--bg-1`, 1px `--border`, radius 8, width
360px, padding 16). Buttons right-aligned: `Cancel` (secondary style, **initial focus**) then
the confirm button (`.btn-danger`: `--danger` background, white text). Esc and overlay-click →
`onCancel`. Enter only activates the focused button (no global Enter-confirms — focus starts
on Cancel precisely so a stray Enter is safe). Sidebar owns the `pendingDelete: string | null`
state and renders:

- title: `Delete branch`
- body: `Delete branch "<name>"?` line 1; line 2 (text-3, 12px):
  `The branch is fully merged, but this cannot be undone from Bonsai.`
- confirmLabel: `Delete branch`

On confirm: close dialog, call `onDelete(name)`. Unmerged branches still reach the backend gate
(§2.6) — the resulting `unmergedBranch` error lands in the sidebar banner.

## 5. Mock IPC (`src/ipc/mock.ts`) — stateful

`src/ipc/fixtures/branches.ts` exports:

```ts
export const INITIAL_BRANCHES: BranchesSnapshot = {
  local: [
    { name: 'main', isHead: true, upstream: 'origin/main', ahead: 0, behind: 0 },
    { name: 'feature/sidebar', isHead: false, upstream: 'origin/feature/sidebar', ahead: 2, behind: 1 },
    { name: 'fix/watcher-debounce', isHead: false, upstream: null, ahead: null, behind: null },
    { name: 'experiment-unmerged', isHead: false, upstream: null, ahead: null, behind: null },
  ],
  remote: [{ name: 'origin/main' }, { name: 'origin/feature/sidebar' }],
  tags: ['v0.1.0', 'v0.2.0'],
  head: { branchName: 'main', oid: MOCK_OID, detached: false, unborn: false },
};
```

Mock behavior (module state `let mockBranches = structuredClone(INITIAL_BRANCHES)`, reset in
`openRepo` alongside `mockStatus` when `path !== openedPath`):

- `listBranches()`: `delay(150)`; `structuredClone(mockBranches)` with
  `head.oid = mockHeadOid` and, for the `?fixture=detached` URL, `head` overridden to
  `{ branchName: null, detached: true, unborn: false }` and all `isHead: false`.
- `createBranch(name)`: trimmed empty, containing whitespace, `..`, `~^:?*[\`, `@{`, leading
  `-`/`/`, trailing `/` or `.lock` → throw `{ kind: 'invalidName', message: "invalid branch
  name: '<name>'" }` (documented simplification of the git rules — backend is authoritative).
  Duplicate → `{ kind: 'branchExists', ... }`. Success: push
  `{ name, isHead: false, upstream: null, ahead: null, behind: null }`, resort
  case-insensitively.
- `checkoutBranch(name)`: unknown → `branchNotFound`. `name === 'fix/watcher-debounce'` →
  throw `{ kind: 'checkoutConflict', message: "cannot switch to 'fix/watcher-debounce':
  local changes would be overwritten. Commit or discard them first." }` (the harness's
  designated dirty-checkout branch). Otherwise: clear all `isHead`, set it on `name`, set
  module `mockHeadBranch = name` — and `openRepo` returns
  `head: { branchName: mockHeadBranch, oid: mockHeadOid, ... }` so the App's post-checkout
  `openRepo` visibly updates the header.
- `deleteBranch(name)`: unknown → `branchNotFound`; `isHead` → `{ kind: 'git', message:
  "cannot delete '<name>': it is the currently checked-out branch" }`;
  `name === 'experiment-unmerged'` → `{ kind: 'unmergedBranch', message: "branch
  'experiment-unmerged' is not fully merged into HEAD (tip 1a2b3c4). Bonsai v1 does not
  force-delete; use `git branch -D experiment-unmerged` if you are sure." }`. Otherwise remove
  from `local`.
- **Decision — the mock graph fixture does NOT move HEAD/branch pills** on checkout/create/
  delete (same rationale as M3's no-graph-row-on-commit: fixtures are generator functions;
  coupling them to mutable branch state buys little). Honest harness proof: sidebar list
  mutates, `isHead` dot moves, header branch name changes after checkout, delete row vanishes.
  Leave `// TODO(polish)` where the graph would be touched.

## 6. Testing (contract for tester)

All scratch repos via the M3 `scratch_dir()` helper (**D:\Temp\bonsai-scratch — hard rule**);
run with `TMP`/`TEMP=D:\Temp`. CLI-oracle pattern as in M3 §6.1 (build fixture with CLI +
repo-local identity; apply our git2 op; compare against git CLI output / twin-repo CLI op).

### 6.1 `list_refs` oracle tests

1. Repo with 3 local branches (one with upstream to a second local "fake remote"? no — add a
   real `file://` remote is M6; instead set upstream via
   `git branch --set-upstream-to` against a fetched local bare remote created with
   `git init --bare` + `git push`) → `local` names+order match
   `git for-each-ref refs/heads --format='%(refname:short)' | sort -f`; `is_head` matches
   `git branch --show-current`; `upstream` matches `%(upstream:short)`; ahead/behind match
   `git rev-list --left-right --count upstream...branch`.
2. Remote-tracking list matches `git for-each-ref refs/remotes` **minus** `origin/HEAD`.
3. Tags: one lightweight + one annotated → both listed, sorted; matches `git tag --list`.
4. Detached HEAD → every `is_head == false`, `head.detached == true`.
5. Unborn repo → empty lists, `head.unborn == true`, `Ok` not `Err`.

### 6.2 `create_branch` tests

1. Create at HEAD → `git rev-parse refs/heads/<name>` equals `git rev-parse HEAD`; HEAD itself
   unchanged (`git branch --show-current` unchanged) — i.e. no checkout.
2. Duplicate name → `Err(BranchExists)`, ref list unchanged.
3. Invalid names (oracle: each must also fail `git check-ref-format --branch <name>`):
   `""`, `" "`, `"a b"`, `"a..b"`, `"a.lock"`, `"/a"`, `"a/"`, `"a~1"`, `"a^"`, `"a:b"`,
   `"a?"`, `"a[b"`, `"@{u}"`, `"-x"` → all `Err(InvalidName)`, no ref created.
4. Unborn repo → `Err(AppError::Git)` with the §2.4 message.

### 6.3 `checkout_branch` tests

1. Clean checkout: two branches with differing file content; ours vs twin `git checkout` →
   `.git/HEAD` symbolic target identical, worktree file contents identical,
   `git status --porcelain` empty in both.
2. Checkout carrying compatible changes: modify a file untouched between branches → succeeds;
   modification survives; porcelain identical to twin `git checkout`.
3. **Dirty conflict**: modify a file that DIFFERS between branches → `Err(CheckoutConflict)`;
   assert `.git/HEAD` unchanged, file content unchanged, porcelain unchanged (nothing moved).
   Twin oracle: `git checkout` exits non-zero with "would be overwritten".
4. Checkout current branch → `Ok(())`, no-op.
5. Nonexistent branch → `Err(BranchNotFound)`.

### 6.4 `delete_branch` tests

1. Merged branch → `Ok`; ref gone (`git rev-parse --verify` fails); twin `git branch -d`
   agrees (also succeeds).
2. Unmerged branch (commit on it, back to main) → `Err(UnmergedBranch)`, ref still present;
   twin oracle: `git branch -d` also fails.
3. Current branch → `Err(AppError::Git)` with the §2.6 message; ref still present.
4. Nonexistent → `Err(BranchNotFound)`.
5. Delete while detached on the merged tip commit → `Ok` (merged relative to detached HEAD).

### 6.5 Command-level tests (`commands.rs`)

All four `_inner` fns with no repo open → `AppError::NoRepo` (extend the existing test).

### 6.6 Frontend smoke (browser harness, `VITE_MOCK_IPC=1 pnpm dev`)

1. Sidebar shows BRANCHES (4 rows, `main` highlighted with dot + accent), REMOTES (2), TAGS (2);
   `feature/sidebar` shows `↑2 ↓1` badge.
2. Hover `feature/sidebar` → ⇄ and delete buttons appear; click ⇄ → head dot moves to
   `feature/sidebar`, header branch name updates, buttons disabled during flight.
3. Checkout `fix/watcher-debounce` → sidebar error banner with the checkoutConflict message;
   nothing else changed; banner dismissible.
4. `+` → input appears; Enter on `bad..name` → inline invalidName error; Enter on `main` →
   branchExists error; Enter on `topic/new` → input closes, row appears sorted.
5. Delete a non-current branch → dialog with exact §4.3 copy, focus on Cancel, Esc closes with
   no change; reopen → confirm (danger button) → row disappears.
6. Delete `experiment-unmerged` → confirm → sidebar banner with the unmergedBranch message,
   row still present.
7. Current branch row has NO hover action buttons.
8. `?fixture=detached` → pinned `HEAD detached @ …` row, no branch highlighted.
9. No `@tauri-apps/*` module executed; no console errors.

## 7. Sub-increment split for senior-dev

- **M5a — Rust backend + CLI-oracle tests.** `error.rs` variants, `repo.rs` visibility change,
  `git/branches.rs` (+ `git/mod.rs`), commands + registration, tests §6.1–§6.5.
  Gate: `cargo test` green, `cargo clippy -- -D warnings` clean, scratch dirs on D:.
- **M5b — Frontend + IPC/mock.** `types.ts`/`tauri.ts`, `fixtures/branches.ts`, stateful
  `mock.ts`, `Sidebar.tsx`, `ConfirmDialog.tsx`, `App.tsx` wiring, styles.
  Gate: `pnpm build` green; §6.6 smoke passes in the harness.

## 8. Acceptance criteria

AI gate:
- §6.1–§6.5 Rust tests pass (git CLI as oracle); `cargo check`/`clippy`/`test`, `pnpm build` green.
- Browser harness passes §6.6 (screenshots: sidebar with badges, delete dialog, conflict
  banner, detached fixture).
- Reviewer confirms: delete flows ONLY through the confirmation dialog; `checkout_tree` uses
  safe mode with no `.force()` anywhere; no force-delete path exists.

USER CHECKPOINT (never self-declared): in the native app on a scratch repo — create a branch,
check it out (header + graph pill follow), make a commit on it, switch back to main, attempt
delete of the unmerged branch (blocked with the clear message), merge it via CLI
(`git merge`), refresh, delete it through the dialog; also verify a dirty checkout is refused
with the conflict message and no files change.

## 9. Ambiguities resolved here (flag to orchestrator if disagreed)

- **Ahead/behind included in v1** (§2.1) — one graph walk per upstream branch, degrades to
  `null` on error; feeds directly into M6.
- **Single `list_branches` snapshot command** instead of three (branches/remotes/tags) — one
  round-trip, one loading state, matches the compact-IPC invariant.
- **Create does not auto-checkout** (§2.4) — orthogonal ops; checkout is one hover-click away.
- **Checkout accepts local branch names ONLY** — no detached checkout of tags/oids, and no
  "checkout remote-tracking → create local tracking branch" in M5. Justification: the latter
  needs upstream wiring that belongs with M6 (fetch/pull show its value); tag/oid checkout adds
  a detached-HEAD entry path with little v1 payoff. Remote and tag rows are read-only lists.
- **Unmerged delete blocked, no force-delete** (§2.6) — safest reviewable v1; CLI hint in the
  error message; merged check implemented by us because libgit2 deletes unconditionally.
- **Merged = reachable from HEAD** (strict `git branch -d`-style check against HEAD only; no
  upstream-based leniency).
- **Delete-current-branch maps to kind `git`** with a bespoke message, not a sixth new kind —
  it is a race-only backstop; the UI hides delete on the head row.
- **Backend-only name validation** via `Branch::name_is_valid`; frontend only gates empty input.
- **Dialog focus starts on Cancel; Enter never global-confirms** — destructive-op safety.
- **Errors surface in a sidebar banner** (checkout/delete) or inline under the create input
  (create), not in the status panel — errors live next to their trigger.
- **Mock graph pills don't move on checkout** (§5) — same fixture-coupling tradeoff as M3;
  header + sidebar state are the harness proof.
- **`repo-changed` covers external branch changes** — verified `watcher.rs::is_relevant`
  already passes `HEAD`, `refs/**`, `packed-refs`; no watcher changes in M5.
