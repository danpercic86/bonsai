# P19 — Submodule support (read + common ops)

User request: submodule support at **read + common ops** — list submodules with status; init;
update; sync; and "open submodule in a new tab". **Out of scope** (explicit, future roadmap):
`add` / `deinit` / `set-branch` / `set-url`.

New Rust module `crates/bonsai-core/src/git/submodule.rs`, four commands, a `SubmoduleInfo` +
`SubmoduleStatus` wire type, the IPC triple, and a **Submodules** sidebar section mirroring
**Stashes**. "Open in new tab" reuses the existing multi-repo tab flow — **no new command**.

Reference contracts (patterns reused verbatim): `docs/contracts/P9-stash-management.md` (module +
command + IPC-triple + sidebar-section + stateful-mock template — this is the structural spine),
`docs/contracts/M6-remotes.md` (credential chain reused by `update_submodule`),
`docs/contracts/P3e-*` (multi-repo tabs — reused by open-in-tab).

Source files to mirror (exact patterns):
- `crates/bonsai-core/src/git/stash.rs` — canonical core module: pure git2, blocking, inline serde
  wire types, `AppError` returns, `open_workdir_repo` open, `#[cfg(test)]` matrix. **Structural template.**
- `crates/bonsai-core/src/git/mod.rs` — module declaration + `relax_odb_hash_verification`.
- `crates/bonsai-core/src/git/status.rs:89` — `.exclude_submodules(true)` **stays AS-IS** (§7).
- `crates/bonsai-core/src/git/remote.rs:95-154,159-189` — the credential chain
  (`next_cred_method` / `acquire_cred` / `map_remote_err` / `CredAttempts`) that `update_submodule`
  reuses (§2.5).
- `src-tauri/src/commands.rs` — stash `#[tauri::command] async fn` + `_inner` +
  `repo_path(state, repo_id)?` + `spawn_blocking` template (`commands.rs:505-539`, stash inners).
- `src-tauri/src/lib.rs:15-72` — `generate_handler!` registration list.
- `src/ipc/{types.ts,tauri.ts,mock.ts}` — the IPC triple; mirror `StashEntry` / `listStashes` /
  stateful `MockRepoState.stashes` shapes (`types.ts:284`, `tauri.ts:267`, `mock.ts:197/394/1806`).
- `src/components/Sidebar.tsx` Stashes section + `src/components/RepoWorkspace.tsx` P9c handlers +
  `src/App.tsx` `openTab` (`App.tsx:190`) + `<RepoWorkspace>` props (`App.tsx:634`).

---

## OPEN DECISIONS (recommended defaults chosen; implementation is NOT blocked)

1. **Absolute vs repo-relative path for open-in-tab.** → **Recommend: backend returns BOTH `path`
   (repo-relative) AND `absPath` (absolute).** The frontend needs the absolute workdir path to feed
   `openTab`; computing it in Rust (`repo.workdir().join(sm.path())`) keeps ALL path logic on the
   Rust side (invariant), avoids TS separator/canonicalization drift on Windows, and costs nothing
   (`open_repo` re-canonicalizes anyway, so the exact form is not load-bearing). `path` is still
   carried for display. *(Alternative: frontend joins `repoId + "/" + path` — rejected: duplicates
   canonicalization the backend already owns.)*
2. **`SubmoduleIgnore` level for status.** → **Recommend `SubmoduleIgnore::None`** (report the full
   picture incl. workdir dirtiness) so `modifiedWorkdir` is detectable. Porcelain `git submodule
   status` uses the config default and never surfaces internal dirtiness with a distinct sigil — so
   the CLI oracle cross-checks `uninitialized`/`outOfSync`/`upToDate` against `git submodule status`
   sigils, and `modifiedWorkdir` against `git -C <sub> status --porcelain` (§8). *(Alternative:
   `Untracked` to skip untracked scanning on huge submodules — flag if perf ever bites.)*
3. **Submodule-not-found error variant.** → **Recommend NO new `AppError` variant** (mirrors P9's
   "no new variant"): `find_submodule` NotFound → `AppError::Git("submodule '<name>' not found")`.
   Names always come from a fresh `list_submodules`, so this is an edge case. A defensively
   empty/blank name → `AppError::InvalidName`. *(Alternative: add `SubmoduleNotFound` — deferred; not
   worth the surface for a list-sourced key.)*
4. **`update` on an uninitialized submodule.** → **Recommend init-then-update in one call**:
   `Submodule::update(init: true, …)`. Semantically "Update" on an uninitialized row does the right
   thing (registers config, fetches, checks out), so the menu need not special-case it (§6.4).
5. **Return type of init/update/sync commands.** → **Recommend `()` + frontend refetches
   `list_submodules`** (matches `stage`/`create_branch` discipline and keeps the mock trivial), NOT
   returning the mutated `SubmoduleInfo`. The list is small; a refetch is cheap and race-free.

All defaults are read-only-safe or standard-git-equivalent; none touches the superproject worktree
except `update` (which checks out the pinned submodule commit, exactly like `git submodule update`).

---

## 1. Overview & invariants held

- **Rust owns all Git logic.** `submodule.rs` wraps every git2 submodule call and the status
  classification; React only renders `SubmoduleInfo` and dispatches the four commands.
- **IPC carries compact precomputed data.** `list_submodules` returns a small `Vec<SubmoduleInfo>`
  with the status already classified into a single enum — no raw libgit2 objects, no per-submodule
  round-trips. Commands = request/response; **no new events or channels**.
- **git2 is blocking → `spawn_blocking`.** Every command wraps its blocking core exactly like the
  stash inners (`commands.rs:514-519`).
- **Runtime-free core.** `submodule.rs` functions take `&Path` / `&str`, no Tauri types →
  unit/CLI-testable without the Tauri "test" feature (same rule as stash/remote).
- **The perf-gated graph walk is untouched.** Submodules never seed the walk and add no `RefLabel`.
- **Credential reuse.** `update_submodule`'s fetch reuses the M6 chain (helper → SSH agent →
  default); never prompts, never stores passwords (§2.5).
- **`status.rs` unchanged.** Submodule dirtiness surfaces ONLY in the Submodules section, never mixed
  into the file-status lists (§7).
- **Mock-implementable.** Every command has a `mock.ts` implementation seeded with 2–3 fixture
  submodules covering all badge states, so `VITE_MOCK_IPC=1` runs the whole feature in a plain browser.
- **`update` may modify the submodule worktree** (checks out the pinned commit). This is standard
  `git submodule update` behavior, is not destructive to the superproject, and never force-checks-out
  — so it needs **no** confirm dialog (unlike Drop/Delete). Init/Sync are non-destructive.

---

## 2. New Rust module `crates/bonsai-core/src/git/submodule.rs`

Register in `crates/bonsai-core/src/git/mod.rs`: add `pub mod submodule;` **after** `pub mod status;`
(alphabetical: `stash` < `status` < `submodule`).

Open the repo with the existing helper `open_workdir_repo(&Path)` (`git/stage.rs:14`) — it rejects
bare repos with a clear `AppError::Git`, exactly as stash does. `list_submodules` needs only `&repo`;
the mutating ops resolve one submodule via `repo.find_submodule(name)`.

### 2.1 Wire types

```rust
/// Consolidated state of one submodule. Wire: a camelCase string enum (no data),
/// e.g. "uninitialized" | "upToDate" | "outOfSync" | "modifiedWorkdir".
/// Derived from git2's `Repository::submodule_status` bitflags (§2.4), evaluated
/// in PRIORITY order (first match wins): Uninitialized > OutOfSync > ModifiedWorkdir > UpToDate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum SubmoduleStatus {
    /// Registered in .gitmodules/index but not checked out (WD_UNINITIALIZED).
    /// Maps to `git submodule status` leading `-`.
    Uninitialized,
    /// Checked out and matching the recorded commit, clean workdir.
    /// Maps to `git submodule status` leading ` ` (space).
    UpToDate,
    /// The checked-out commit differs from the commit recorded in the
    /// superproject (index or HEAD). Maps to `git submodule status` leading `+`.
    OutOfSync,
    /// Checked-out commit matches, but the submodule's OWN worktree/index is
    /// dirty (staged, unstaged, or untracked changes inside it).
    ModifiedWorkdir,
}

/// One submodule row. Wire: camelCase. All oids are full 40-hex or null.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SubmoduleInfo {
    /// Submodule NAME (stable key for init/update/sync). `Submodule::name()`.
    pub name: String,
    /// Repo-relative path, forward slashes on the wire. `Submodule::path()`.
    pub path: String,
    /// ABSOLUTE workdir path for open-in-tab (§OPEN-1): superproject workdir
    /// joined with `path`. Fed verbatim to the existing open-repo/tab flow.
    pub abs_path: String,
    /// Configured URL from .gitmodules/.git config. `Submodule::url()`. None if unreadable.
    pub url: Option<String>,
    /// Commit recorded in the superproject HEAD tree. `Submodule::head_id()`.
    pub head_oid: Option<String>,
    /// Commit recorded in the superproject index. `Submodule::index_id()`.
    pub index_oid: Option<String>,
    /// Commit currently checked out in the submodule worktree. `Submodule::workdir_id()`.
    /// None when uninitialized.
    pub wt_oid: Option<String>,
    pub status: SubmoduleStatus,
}
```

### 2.2 Function signatures

```rust
/// Blocking. List every submodule with its classified status. Empty repo (no
/// submodules) → Ok(vec![]). Order: `Repository::submodules()` order (stable).
pub fn list_submodules(workdir: &Path) -> Result<Vec<SubmoduleInfo>, AppError>;

/// Blocking. Register submodule `name` into .git/config (copies .gitmodules
/// url/config). git2: `Submodule::init(false)` (no overwrite). No worktree change.
pub fn init_submodule(workdir: &Path, name: &str) -> Result<(), AppError>;

/// Blocking. Init-if-needed + fetch (shared M6 credential chain) + checkout the
/// pinned commit. git2: `Submodule::update(true, Some(&mut opts))` with the fetch
/// callbacks wired to the credential chain (§2.5). MODIFIES the submodule worktree.
pub fn update_submodule(workdir: &Path, name: &str) -> Result<(), AppError>;

/// Blocking. Copy the URL from .gitmodules into .git/config and the submodule's
/// remote. git2: `Submodule::sync()`. No worktree change.
pub fn sync_submodule(workdir: &Path, name: &str) -> Result<(), AppError>;
```

### 2.3 `list_submodules` internals

```rust
let repo = open_workdir_repo(workdir)?;                 // rejects bare (stash-consistent)
let sm_workdir = repo.workdir()
    .ok_or_else(|| AppError::Git("repository has no working directory".to_string()))?;
let mut out = Vec::new();
for sm in repo.submodules()? {
    // Skip non-UTF-8 names (cannot key status; log + skip, like fetch_all does).
    let name = match sm.name() { Some(n) => n.to_string(), None => {
        eprintln!("bonsai: skipping submodule with non-UTF-8 name"); continue; } };
    let rel = sm.path().to_string_lossy().replace('\\', "/");    // forward slashes on the wire
    let abs = sm_workdir.join(sm.path()).to_string_lossy().into_owned();
    let flags = repo.submodule_status(&name, git2::SubmoduleIgnore::None)?;   // §OPEN-2
    out.push(SubmoduleInfo {
        name,
        path: rel,
        abs_path: abs,
        url: sm.url().map(str::to_string),
        head_oid: sm.head_id().map(|o| o.to_string()),
        index_oid: sm.index_id().map(|o| o.to_string()),
        wt_oid: sm.workdir_id().map(|o| o.to_string()),
        status: classify_status(flags),
    });
}
Ok(out)
```

### 2.4 `classify_status` — bitflag → enum (PRIORITY order, first match wins)

```rust
fn classify_status(f: git2::SubmoduleStatus) -> SubmoduleStatus {
    use git2::SubmoduleStatus as S;
    // 1. Not checked out at all.
    if f.contains(S::WD_UNINITIALIZED) {
        return SubmoduleStatus::Uninitialized;
    }
    // 2. Recorded-commit mismatch: superproject index/HEAD pointer changed, OR the
    //    checked-out commit differs from the index pointer.
    if f.intersects(S::INDEX_ADDED | S::INDEX_DELETED | S::INDEX_MODIFIED | S::WD_MODIFIED) {
        return SubmoduleStatus::OutOfSync;
    }
    // 3. Submodule's own index/worktree is dirty (but the pinned commit matches).
    if f.intersects(S::WD_INDEX_MODIFIED | S::WD_WD_MODIFIED | S::WD_UNTRACKED) {
        return SubmoduleStatus::ModifiedWorkdir;
    }
    // 4. Checked out, clean, matching.
    SubmoduleStatus::UpToDate
}
```

Bitflag reference (git2 `SubmoduleStatus`): `WD_UNINITIALIZED` = registered but no checkout;
`WD_MODIFIED` = submodule HEAD ≠ the SHA stored in the superproject index (this IS "outOfSync");
`INDEX_ADDED/DELETED/MODIFIED` = the superproject index staged a different pointer than HEAD;
`WD_INDEX_MODIFIED` / `WD_WD_MODIFIED` / `WD_UNTRACKED` = staged / unstaged / untracked changes
INSIDE the submodule. A submodule that is simultaneously out-of-sync AND dirty classifies as
`OutOfSync` (higher priority) — documented so the UI badge is deterministic.

### 2.5 `update_submodule` internals (credential reuse)

`update_submodule` performs a fetch, so it MUST reuse the M6 credential chain rather than inventing
its own. In `remote.rs` change `fn acquire_cred` (`remote.rs:118`) to **`pub(crate) fn acquire_cred`**
(and keep `CredAttempts`, `next_cred_method`, `CRED_EXHAUSTED_MSG`, `map_remote_err` as the existing
`pub(crate)`). Then in `submodule.rs`, build the fetch callbacks INLINE exactly mirroring
`fetch_remote` (`remote.rs:216-247`):

```rust
let repo = open_workdir_repo(workdir)?;
if name.trim().is_empty() {
    return Err(AppError::InvalidName("submodule name is empty".to_string()));
}
let mut sm = repo.find_submodule(name).map_err(|e| match e.code() {
    git2::ErrorCode::NotFound => AppError::Git(format!("submodule '{name}' not found")),  // §OPEN-3
    _ => e.into(),
})?;

let config = repo.config()?;
let attempts = std::cell::RefCell::new(crate::git::remote::CredAttempts::default());
let mut callbacks = git2::RemoteCallbacks::new();
callbacks.credentials(|url, username_from_url, allowed| {
    crate::git::remote::acquire_cred(&config, &attempts, url, username_from_url, allowed)
});

let mut fo = git2::FetchOptions::new();
fo.remote_callbacks(callbacks);
let mut opts = git2::SubmoduleUpdateOptions::new();
opts.fetch(fo);

// init=true → init-then-update in one call (§OPEN-4). SAFE checkout default
// (SubmoduleUpdateOptions uses a safe checkout builder; never force).
sm.update(true, Some(&mut opts))
    .map_err(|e| crate::git::remote::map_remote_err(e, name))?;
Ok(())
```

`init_submodule` / `sync_submodule` are the same open + `find_submodule` prologue, then
`sm.init(false)?;` / `sm.sync()?;` respectively (no fetch, no credentials, no worktree change).

### 2.6 Error mapping (→ `AppError`, `error.rs`)

| Situation | AppError |
|---|---|
| repo not usable / bare | `Git` (via `open_workdir_repo`) |
| empty/blank `name` arg | `InvalidName` |
| `find_submodule` NotFound | `Git` ("submodule '<name>' not found") — §OPEN-3 |
| update fetch auth exhausted / auth code | `AuthFailed` (via `map_remote_err`) |
| update fetch transport (Net/Http/Ssh) | `NetworkError` (via `map_remote_err`) |
| any other libgit2 | `Git` |

**No new `AppError` variant** (reuses `git` | `invalidName` | `authFailed` | `networkError`).

---

## 3. Commands (`src-tauri/src/commands.rs`) + registration

Add `use bonsai_core::git::submodule::{self, SubmoduleInfo};` to the import block (`commands.rs:1-33`).
Follow the stash `#[tauri::command] pub async fn` → runtime-free `_inner` → `repo_path(state, repo_id)?`
→ `spawn_blocking` template exactly (`commands.rs:505-539`). None emit `repo-changed` — the frontend
refetches imperatively (identical to stash/merge/branches).

```rust
// list_submodules(repoId) -> Vec<SubmoduleInfo>       errors: noRepo | git
#[tauri::command] pub async fn list_submodules(state, repo_id: String) -> Result<Vec<SubmoduleInfo>, AppError>;
async fn list_submodules_inner(state, repo_id) { spawn_blocking(|| submodule::list_submodules(&path)) }

// init_submodule(repoId, name) -> ()                  errors: noRepo | invalidName | git
#[tauri::command] pub async fn init_submodule(state, repo_id: String, name: String) -> Result<(), AppError>;
async fn init_submodule_inner(...) { spawn_blocking(move || submodule::init_submodule(&path, &name)) }

// update_submodule(repoId, name) -> ()   errors: noRepo | invalidName | authFailed | networkError | git
#[tauri::command] pub async fn update_submodule(state, repo_id: String, name: String) -> Result<(), AppError>;
async fn update_submodule_inner(...) { spawn_blocking(move || submodule::update_submodule(&path, &name)) }

// sync_submodule(repoId, name) -> ()                  errors: noRepo | invalidName | git
#[tauri::command] pub async fn sync_submodule(state, repo_id: String, name: String) -> Result<(), AppError>;
async fn sync_submodule_inner(...) { spawn_blocking(move || submodule::sync_submodule(&path, &name)) }
```

`spawn_blocking` join errors map with `AppError::Other(format!("task join error: {e}"))` verbatim.
Register all four in `src-tauri/src/lib.rs` `generate_handler!` (`lib.rs:15-72`), appended after
`commands::set_mcp_allow_write` (add a trailing comma to that line):

```rust
        commands::set_mcp_allow_write,
        commands::list_submodules,
        commands::init_submodule,
        commands::update_submodule,
        commands::sync_submodule
```

---

## 4. Wire types (TS mirror — `src/ipc/types.ts`)

New types (place near `StashEntry`, `types.ts:284`):

```ts
export type SubmoduleStatus = 'uninitialized' | 'upToDate' | 'outOfSync' | 'modifiedWorkdir';

export interface SubmoduleInfo {
  name: string;              // stable key for init/update/sync
  path: string;              // repo-relative, forward slashes
  absPath: string;           // absolute workdir path — feed to open-in-tab
  url: string | null;
  headOid: string | null;    // commit in superproject HEAD
  indexOid: string | null;   // commit in superproject index
  wtOid: string | null;      // commit checked out in the submodule (null if uninitialized)
  status: SubmoduleStatus;
}
```

`IpcApi` additions (near `listStashes`, `types.ts:643`; mirror the JSDoc style):

```ts
/** All submodules with classified status. Rejects noRepo | git. */
listSubmodules(repoId: string): Promise<SubmoduleInfo[]>;
/** Register `name` in .git/config (no worktree change). Rejects noRepo | invalidName | git. */
initSubmodule(repoId: string, name: string): Promise<void>;
/** Init-if-needed + fetch + checkout the pinned commit. Rejects
 *  noRepo | invalidName | authFailed | networkError | git. */
updateSubmodule(repoId: string, name: string): Promise<void>;
/** Copy the .gitmodules URL into config + the submodule remote. Rejects noRepo | invalidName | git. */
syncSubmodule(repoId: string, name: string): Promise<void>;
```

`src/ipc/tauri.ts` (add beside the stash wrappers, `tauri.ts:267-289`), snake_case command +
camelCase arg keys (Tauri auto-converts):

```ts
listSubmodules(repoId: string): Promise<SubmoduleInfo[]> {
  return invoke<SubmoduleInfo[]>('list_submodules', { repoId });
},
initSubmodule(repoId: string, name: string): Promise<void> {
  return invoke<void>('init_submodule', { repoId, name });
},
updateSubmodule(repoId: string, name: string): Promise<void> {
  return invoke<void>('update_submodule', { repoId, name });
},
syncSubmodule(repoId: string, name: string): Promise<void> {
  return invoke<void>('sync_submodule', { repoId, name });
},
```

---

## 5. Stateful mock (`src/ipc/mock.ts`)

- Import `SubmoduleInfo` (with `StashEntry`, `mock.ts:61`).
- Add `submodules: SubmoduleInfo[]` to `MockRepoState` (`mock.ts:197-223`, near `stashes`).
- Seed via a `seedSubmodules(kind, graphFixture): SubmoduleInfo[]` helper (mirror `seedStashes`,
  `mock.ts:394`) wired into `createRepoState` (`mock.ts:441`). Only the **default** repo seeds
  submodules (detached/unborn/20k → `[]`), covering ALL badge states so the harness shows every one:

```ts
function seedSubmodules(kind: RepoKind, graphFixture: GraphFixture): SubmoduleInfo[] {
  if (kind !== 'default' || graphFixture !== 'default') return [];
  return [
    { name: 'vendor/libcore', path: 'vendor/libcore',
      absPath: '/mock/repo/vendor/libcore', url: 'https://example.com/libcore.git',
      headOid: fixtureOid(1), indexOid: fixtureOid(1), wtOid: null,
      status: 'uninitialized' },
    { name: 'vendor/theme', path: 'vendor/theme',
      absPath: '/mock/repo/vendor/theme', url: 'https://example.com/theme.git',
      headOid: fixtureOid(2), indexOid: fixtureOid(2), wtOid: fixtureOid(2),
      status: 'upToDate' },
    { name: 'docs/spec', path: 'docs/spec',
      absPath: '/mock/repo/docs/spec', url: 'https://example.com/spec.git',
      headOid: fixtureOid(4), indexOid: fixtureOid(4), wtOid: randomOid(),
      status: 'outOfSync' },
    { name: 'tools/ci', path: 'tools/ci',
      absPath: '/mock/repo/tools/ci', url: 'https://example.com/ci.git',
      headOid: fixtureOid(5), indexOid: fixtureOid(5), wtOid: fixtureOid(5),
      status: 'modifiedWorkdir' },
  ];
}
```

- Command methods (add beside the stash methods, `mock.ts:1806-1877`):
  - `listSubmodules(repoId)` → `structuredClone(state.submodules)`.
  - `initSubmodule(repoId, name)` → find by name; if `uninitialized`, flip to `upToDate` and set
    `wtOid = indexOid`; resolve `void`. (Unknown name → resolve `void`; the real backend errors, but
    the mock list is authoritative so this path is unreachable from the UI.)
  - `updateSubmodule(repoId, name)` → find by name; set `wtOid = indexOid` and `status = 'upToDate'`
    (init-then-update semantics — clears `uninitialized`/`outOfSync`). `void`.
  - `syncSubmodule(repoId, name)` → find by name; no observable list change (URL already reflected);
    `void`. (Keeps the mock honest: sync mutates config, not the listed fields.)
- No new events/channels; `onRepoChanged` etc. unchanged.

---

## 6. Frontend

### 6.1 Sidebar "Submodules" section (`src/components/Sidebar.tsx`)

Add after the Stashes section, matching its styling (`SectionHeader`, `branch-list`, `branch-row`,
`branch-muted`) and its collapse pattern (`stashesCollapsed` → `submodulesCollapsed` local state).
New props on `SidebarProps`:

```ts
submodules: SubmoduleInfo[];
onSubmoduleContextMenu(name: string, clientX: number, clientY: number): void;
```

- **Header**: `SectionHeader label="Submodules"` — no header action button (add/deinit are out of
  scope). Hide/collapse when the list is empty (see empty state).
- **Empty state**: `<p className="branch-muted">No submodules</p>`.
- **Row** (`SubmoduleRow`, sibling of `StashRow`): a `branch-row` showing the submodule `name` (or
  `path` — prefer `name`) plus a right-aligned **status badge** (§6.2). `title={path}` for the full
  path. `onContextMenu` → `onSubmoduleContextMenu(name, e.clientX, e.clientY)` with
  `e.preventDefault()`, exactly like `StashRow`.

### 6.2 Status badge

A small pill keyed by `SubmoduleStatus`, theme-aware, reusing the existing badge/pill CSS
conventions (no new metric). Label + intent:

| status | label | intent (color) |
|---|---|---|
| `uninitialized` | "not initialized" | muted / neutral |
| `upToDate` | "up to date" | success / green |
| `outOfSync` | "out of sync" | warning / amber |
| `modifiedWorkdir` | "modified" | warning / amber |

Badge is display-only (no click behavior); all actions are via the context menu.

### 6.3 RepoWorkspace state + handlers (`src/components/RepoWorkspace.tsx`)

Mirror the P9c stash wiring. Add `submodules: SubmoduleInfo[]` state and `refetchSubmodules()`
(`ipc.listSubmodules(repoId)`), included in `refreshAll` and the `repo-changed` / window-focus
refresh batch. Pass `submodules` + `onSubmoduleContextMenu` into `<Sidebar>`.

New prop on `RepoWorkspace` (threaded for open-in-tab, §6.5):

```ts
onOpenRepoPath(path: string): void;   // opens `path` in a new/focused tab (App.openTab)
```

Handlers (mirror `handleApplyStash` — `setMutating(true)` / `try` / toast / refresh / `finally`):

```ts
async function handleInitSubmodule(name: string) {
  setMutating(true);
  try {
    await ipc.initSubmodule(repoId, name);
    pushToast('success', `Initialized ${name}`);
    await refetchSubmodules();
  } catch (e) { pushToast('error', errorMessage(e)); }
  finally { setMutating(false); }
}

async function handleUpdateSubmodule(name: string) {
  setMutating(true);
  try {
    await ipc.updateSubmodule(repoId, name);
    pushToast('success', `Updated ${name}`);
    await refetchSubmodules();
  } catch (e) { pushToast('error', errorMessage(e)); }
  finally { setMutating(false); }
}

async function handleSyncSubmodule(name: string) {
  setMutating(true);
  try {
    await ipc.syncSubmodule(repoId, name);
    pushToast('success', `Synced URL for ${name}`);
    await refetchSubmodules();
  } catch (e) { pushToast('error', errorMessage(e)); }
  finally { setMutating(false); }
}
```

`refetchSubmodules` suffices (submodule ops do not change the superproject status/graph in v1); if a
future `update` should surface as a superproject change, promote to `refreshAll` then. Init/update/
sync are non-destructive-to-superproject → no confirm dialog.

### 6.4 Submodule context menu (shared `ContextMenu`)

Add `submoduleMenuItems(sub: SubmoduleInfo)` beside `stashMenuItems`, returning
`ContextMenuItem[]`:

```ts
function submoduleMenuItems(sub: SubmoduleInfo): ContextMenuItem[] {
  const gate = mutating || opActive;
  return [
    // "Update" on an uninitialized row init-then-updates (backend §OPEN-4), so it is
    // always enabled; "Init" is a no-op once initialized → disable when not uninitialized.
    { label: 'Init',   disabled: gate || sub.status !== 'uninitialized',
      onSelect: () => void handleInitSubmodule(sub.name) },
    { label: 'Update', disabled: gate, onSelect: () => void handleUpdateSubmodule(sub.name) },
    { label: 'Sync',   disabled: gate, onSelect: () => void handleSyncSubmodule(sub.name) },
    { label: 'Open in new tab', disabled: sub.status === 'uninitialized',
      onSelect: () => onOpenRepoPath(sub.absPath) },
  ];
}
```

`onSubmoduleContextMenu(name, x, y)` looks up the `SubmoduleInfo` by name from state and
`setMenu({ x, y, items: submoduleMenuItems(sub) })` (reuses the single `<ContextMenu>`). "Open in new
tab" is disabled for uninitialized submodules (no workdir to open yet).

### 6.5 Open-in-tab wiring (`src/App.tsx`) — reuse the existing tab flow, NO new command

`App` already owns `openTab(path)` (`App.tsx:190`), which calls `ipc.openRepo(path)` and adds/focuses
a tab. Thread it into the workspace: on the `<RepoWorkspace>` element (`App.tsx:634-651`) add

```tsx
onOpenRepoPath={(path) => void openTab(path)}
```

`RepoWorkspace` passes `onOpenRepoPath` down to `submoduleMenuItems`. "Open in new tab" therefore
feeds the submodule's absolute workdir path to the **existing** open-repo/tab machinery — the submodule
is opened as an ordinary repo tab (it is a real Git repo once initialized). No P19-specific command,
event, or tab type is introduced. In the mock, `openRepo(absPath)` seeds a normal default repo state
(§P3e mock), so the harness can open a submodule tab end-to-end.

---

## 7. `status.rs` stays AS-IS (`.exclude_submodules(true)`)

`read_status` (`status.rs:82-90`) keeps `.exclude_submodules(true) // v1: no submodule support`
**unchanged**. Rationale:
- **Separation of concerns.** Submodule dirtiness/out-of-sync is richer than a file status
  (uninitialized vs out-of-sync vs internally-modified) and belongs in the dedicated Submodules
  section (§6), which classifies it into `SubmoduleStatus`. Folding a submodule into the working-dir
  file lists would produce a confusing "changes" row that cannot be staged like a file.
- **M1 porcelain parity preserved.** The status tests cross-check `git status --porcelain` with the
  default submodule handling; flipping `exclude_submodules` off would change that surface and break
  M1/P17 assumptions.
- **P17 partial-staging safety.** The three-way / blob-reconstruction staging model assumes file
  blobs; a submodule (a gitlink entry) has no blob and must never enter the stage/unstage/partial
  paths.

So submodule state is surfaced ONLY through `list_submodules`, never mixed into
`StatusSnapshot.{staged,unstaged,untracked,conflicted}`.

---

## 8. Testing (AI gate) — `crates/bonsai-core/tests/submodule_cli.rs`

CLI-oracle suite mirroring `tests/remote_cli.rs` (local `file://` only — no network, autonomous;
`require_git!` skip when `git` is absent). **Env (tester):** `TMP`/`TEMP` → `D:\Temp`; run `cargo
test` and `clippy` **sequentially**; scratch repos under `D:\Temp\bonsai-scratch` via
`common::scratch_dir()`; forward slashes in Bash-tool paths.

### 8.1 Fixture (built with the `git` CLI, like remote_cli)

1. Bare submodule origin: `git init --bare sub-origin.git`; a seed clone commits **A** then **B**
   (two files/commits) and pushes `main` → the submodule remote has two commits.
2. Superproject `super` (own repo): `git submodule add file:///…/sub-origin.git sub` (pins the
   submodule at tip **B**), then `git commit` the `.gitmodules` + gitlink.
3. Derive the states under test from `super` (and a fresh clone of `super` for the uninitialized case):
   - **uninitialized** — a fresh `git clone file:///…/super work` (submodule NOT recursed) → `sub` is
     registered but empty.
   - **upToDate** — after `init_submodule` + `update_submodule` (the system under test) on `work`.
   - **outOfSync** — inside `work/sub`, `git checkout <A>` so the checked-out commit ≠ the pinned **B**.
   - **modifiedWorkdir** — inside an up-to-date `work/sub`, edit a tracked file (no commit).

### 8.2 Assertions

1. **`list_submodules` status parity** across uninitialized / upToDate / outOfSync: the
   `SubmoduleStatus` maps to the `git submodule status` leading sigil — `-` ↔ `uninitialized`,
   ` ` ↔ `upToDate`, `+` ↔ `outOfSync` (parse `git -C <super> submodule status`).
2. **`modifiedWorkdir`** cross-checked against `git -C <super>/sub status --porcelain` being non-empty
   while `git submodule status` still shows ` `/no `+` (dirty but pinned-commit matches).
3. **`init_submodule` + `update_submodule`** bring the submodule to the pinned commit: after the two
   calls, `list_submodules` reports `status == upToDate` and `wt_oid == index_oid`; cross-check that a
   subsequent `git -C <super> submodule update` is a no-op (already at the pinned commit) and that
   `git -C <super>/sub rev-parse HEAD` equals the recorded index oid.
4. **`update_submodule` fetches** the missing commit over `file://` with the shared credential path
   (local transport → no callback invoked; covers the plumbing, not real auth — same honest-coverage
   note as remote_cli).
5. **`sync_submodule`** propagates a changed URL: rewrite `submodule.<name>.url` in `.gitmodules`
   (e.g. to a second bare path), call `sync_submodule`, assert `git -C <super> config
   submodule.<name>.url` now equals the new URL.
6. **Wire shapes** (unit test in `submodule.rs` `#[cfg(test)]`): `serde_json` asserts
   `SubmoduleStatus` → `"uninitialized"|"upToDate"|"outOfSync"|"modifiedWorkdir"` and `SubmoduleInfo`
   → camelCase keys `{name,path,absPath,url,headOid,indexOid,wtOid,status}`.
7. **`classify_status` unit table** (pure, no repo): each bitflag combination → expected variant,
   including the priority tie-break (WD_UNINITIALIZED wins; WD_MODIFIED+WD_WD_MODIFIED → `outOfSync`).

### 8.3 Browser-harness (orchestrator-verifiable)

- `pnpm build` + `tsc` clean.
- Sidebar shows a **Submodules** section listing the four seeded submodules with all four badge
  states (screenshot evidence: not initialized / up to date / out of sync / modified).
- Right-click a row → Init / Update / Sync / Open in new tab menu; Init is disabled on the
  already-initialized rows; Open in new tab is disabled on the uninitialized row.
- "Update" on the uninitialized/out-of-sync mock rows flips the badge to "up to date" and toasts.
- "Open in new tab" on an initialized submodule opens a second tab (mock `openRepo(absPath)`).

### 8.4 USER CHECKPOINT (native `pnpm tauri dev`, real repo with a real submodule)

The Submodules section lists real submodules with correct badges; Init/Update/Sync behave like the
`git` CLI equivalents (verify with `git submodule status`); Update over a real remote fetches +
checks out with the credential helper/agent (no password prompt); "Open in new tab" opens the
submodule as its own repo tab.

---

## 9. Sub-increments (each a single fresh-context senior-dev pass)

### P19a — Rust: `submodule.rs` + credential exposure + commands + registration + tests
- New `crates/bonsai-core/src/git/submodule.rs` (§2): `SubmoduleStatus`, `SubmoduleInfo`,
  `classify_status`, the four functions with exact git2 calls. `git/mod.rs` `pub mod submodule;`.
- `remote.rs`: `fn acquire_cred` → `pub(crate) fn acquire_cred` (§2.5) — the only change to remote.rs.
- `commands.rs`: four `#[tauri::command]` + `_inner` pairs (§3); `lib.rs` registration.
- Unit tests in `submodule.rs` `#[cfg(test)]`: wire shapes + `classify_status` table (§8.2 #6–#7).
- **Acceptance**: `cargo check`/`clippy` clean; unit tests pass; no frontend change needed to compile.

### P19b — CLI-oracle test suite
- `crates/bonsai-core/tests/submodule_cli.rs` (+ reuse `tests/common`), the §8.1 fixture and §8.2
  assertions #1–#5. `require_git!` skip guard.
- **Acceptance**: suite passes (or skips cleanly without `git`); status parity + init/update/sync
  behaviors match the CLI oracle.

### P19c — IPC triple + frontend section + open-in-tab wiring
- `types.ts`: `SubmoduleStatus` / `SubmoduleInfo` + four `IpcApi` methods; `tauri.ts`: four wrappers;
  `mock.ts`: `submodules` state + `seedSubmodules` + four command methods (§4, §5).
- `Sidebar.tsx`: Submodules section + `SubmoduleRow` + status badge + props (§6.1–§6.2).
- `RepoWorkspace.tsx`: `submodules` state + `refetchSubmodules` (into `refreshAll` + repo-changed
  batch), three handlers, `submoduleMenuItems`, `onOpenRepoPath` prop, Sidebar wiring (§6.3–§6.4).
- `App.tsx`: pass `onOpenRepoPath={(p) => void openTab(p)}` to `<RepoWorkspace>` (§6.5).
- **Acceptance**: `pnpm build`/`tsc` clean; harness shows all four badge states, the context menu with
  correct disabled states, mock Update flips a badge, Open-in-tab opens a second tab (§8.3).

---

## 10. File touch list

- `crates/bonsai-core/src/git/submodule.rs` (**new**), `crates/bonsai-core/src/git/mod.rs`
  (`pub mod submodule;`).
- `crates/bonsai-core/src/git/remote.rs` (`acquire_cred` → `pub(crate)` — one word).
- `src-tauri/src/commands.rs` (import + 4 command/_inner pairs), `src-tauri/src/lib.rs` (register 4).
- `crates/bonsai-core/tests/submodule_cli.rs` (**new**).
- `src/ipc/types.ts` (`SubmoduleStatus`/`SubmoduleInfo` + `IpcApi`), `src/ipc/tauri.ts` (4 wrappers),
  `src/ipc/mock.ts` (`submodules` state + `seedSubmodules` + 4 methods).
- `src/components/Sidebar.tsx` (Submodules section + `SubmoduleRow` + badge + props),
  `src/components/RepoWorkspace.tsx` (state, `refetchSubmodules`, handlers, menu, `onOpenRepoPath`),
  `src/App.tsx` (thread `onOpenRepoPath` → `openTab`).
- `src/components/RepoWorkspace.css` (or the shared stylesheet) — status-badge intent classes (reuse
  existing pill CSS where possible).
- **`crates/bonsai-core/src/git/status.rs` — NOT touched** (`.exclude_submodules(true)` stays, §7).
- No new `AppError` variant; no new events/channels; `notify` watcher unchanged.
```