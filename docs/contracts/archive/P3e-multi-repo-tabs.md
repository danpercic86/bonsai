# P3e — Multi-repo tabs: Implementation Contract

Status: authoritative for P3e (final P3 milestone). Scope: turn Bonsai from a **one-repo-at-a-time**
client into a **multi-tab** client. Backend `AppState` becomes a keyed map of open repos; every
repo-scoped command gains a `repoId`; a new `close_repo` + session-persistence commands are added;
the `repo-changed` event payload gains `repoId`. The frontend extracts the entire per-repo state
cluster out of `App.tsx` into `RepoWorkspace.tsx` (all tabs mounted, inactive `display:none`),
replaces `RepoSwitcher` with `TabStrip`, and reopens all persisted tabs on launch.

Builds on and reuses: `P1-followups`/`settings.rs` (recents + `settings.json` persistence,
atomic save, `#[serde(default)]` additive-field precedent), `M1` (watcher design + refresh/refocus
pairing), `M2`/`GraphCanvas` (the canvas resize/paint model), and the runtime-free
`*_inner(state, …)` command pattern established since M1 and reaffirmed in P3c/P3d.

**Invariants (unchanged, enforced in review):** Rust owns all Git logic + layout math; IPC carries
compact precomputed data (commands = req/resp, events = small push signals); git2 runs under
`spawn_blocking`; every command keeps its runtime-free `*_inner(state: &AppState, …)` twin that unit
tests call with a constructed `AppState` and **no Tauri runtime** (the `test` feature crashes on this
machine — see CLAUDE.md); the `notify` watcher is always paired with a manual refresh + refocus
rescan + ~300 ms debounce; `src/ipc/mock.ts` is updated with EVERY `IpcApi` change and stays a
faithful stateful twin; destructive ops keep their `ConfirmDialog` + backend guard.

**No new `AppError` variants.** An unknown/closed `repoId` maps to the existing `AppError::NoRepo`.

---

## 1. Sub-increment decomposition

Each row is one fresh-context senior-dev pass (this file + the listed source paths). Ordered so each
compiles on top of the previous.

| # | Increment | Content | Read |
|---|-----------|---------|------|
| 1 | **P3e-a** | Backend core. `state.rs`: `RepoEntry` + `HashMap<String, RepoEntry>`. `commands.rs`: thread `repo_id` through all 26 repo-scoped `*_inner`/wrappers, `open_repo` → `OpenRepoResult`, new `close_repo`, `RepoChangedPayload` gains `repoId`. `watcher.rs`: unchanged code, per-entry watcher wired in `commands.rs`. `lib.rs`: register `close_repo`. Rewrite the existing `_require_an_open_repo` command tests + add two-repo isolation tests. | §2, §3, §4, §7, §9 |
| 2 | **P3e-b** | Backend session persistence. `settings.rs`: additive `open_repos` / `active_repo` fields (version stays 1). `commands.rs` + `lib.rs`: `get_session` / `set_session`. Unit tests (round-trip + legacy load). | §6 (backend half) |
| 3 | **P3e-c** | IPC mirror. `types.ts` (`OpenRepoResult`, `repoId` params, `closeRepo`, `SessionState`, `RepoChangedPayload.repoId`), `tauri.ts`, `index.ts`, and the **multi-repo mock** (`mock.ts` keyed per `repoId`, `?op=`/`?fixture=` still compose, path-substring seeds for distinct tabs). | §5, §8 |
| 4 | **P3e-d** | `GraphCanvas` visibility hardening: new `active` prop + zero-size guard + remeasure-on-show. Backward compatible (defaults active); harness-verifiable in isolation. | §5.4 |
| 5 | **P3e-e** | Frontend refactor. Extract `RepoWorkspace.tsx` (the per-repo cluster + all handlers), add `TabStrip.tsx` (replaces `RepoSwitcher`), slim `App.tsx` to global state + tabs + session wiring, add `ToastContext`. Reopen-all-on-launch. | §5, §6 (frontend half) |

Tester passes (after P3e-a and P3e-e land): §9.

---

## 2. `repoId` identity (decision)

**Decision: `repoId` is the canonical workdir path string that `read_repo_info` already returns as
`RepoInfo.path`.** No separate id space.

Rationale:
- `read_repo_info` canonicalizes the workdir root; the same folder deterministically yields the same
  string, so it is **stable across reopen-on-launch** — persisted `openRepos`/`activeRepo` rehydrate
  by passing the stored strings straight back to `open_repo`.
- Opening an already-open path **focuses** the existing tab for free: `open_repo` scans the map for a
  case-insensitive path match and returns that entry's existing id instead of inserting a duplicate;
  the frontend, seeing a `repoId` already in `openRepos`, just sets it active.
- It reuses the exact identity notion already used for recents dedupe (case-insensitive path).

**Rejected alternative — opaque incrementing id / hash:** would need a separately persisted
`id ↔ path` map to survive relaunch, adding state and a failure mode for zero benefit here. Keeping
`repoId == path` means the `OpenRepoResult.repoId` and `RepoInfo.path` are equal today; the contract
still returns `repoId` **explicitly** so the frontend never hard-codes that equality and a future
switch to opaque ids is a pure backend change.

Keying details:
- The `HashMap` is keyed by the id string **exactly as returned** (deterministic from
  canonicalization), so `repo_path()` lookups use exact-string equality.
- Only `open_repo`'s dedupe scan is case-insensitive (`eq_ignore_ascii_case`), matching
  `settings::record_recent`. This is the single place case can vary.

---

## 3. `src-tauri/src/state.rs` — AppState

Replaces the current two-Mutex single-repo shape.

```rust
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;

use crate::watcher::WatcherHandle;

/// One open repository: its canonical workdir root and its own file watcher.
///
/// NOT `Clone`/`Debug` (`WatcherHandle` is neither) — entries live in the map
/// and callers clone `path` out under the lock, never the whole entry.
///
/// Future perf lever (M2 note): a cached `git2::Repository` handle could live
/// here per entry. Out of scope for P3e — every `git/*.rs` fn still opens from
/// `path`; the shape simply makes per-repo caching a localized future change.
pub struct RepoEntry {
    pub path: PathBuf,
    pub watcher: Option<WatcherHandle>,
}

/// Shared app state: every open repo, keyed by `repoId` (canonical workdir
/// path string, §2). One Mutex guards the whole map — safe because handlers
/// only hold the lock long enough to clone a `PathBuf` out (or insert/remove an
/// entry), never across the `spawn_blocking` git work.
#[derive(Default)]
pub struct AppState {
    pub repos: Mutex<HashMap<String, RepoEntry>>,
}
```

**Locking strategy (documented decision):** a single `Mutex<HashMap>` rather than per-entry locks.
Every repo-scoped handler follows the established shape — acquire the lock, clone the entry's
`path`, drop the lock, then do the blocking git work on the cloned path. The lock is therefore never
held across git2 calls and there is no cross-repo contention in practice. Per-entry `Mutex`es would
only matter if we cached live `Repository` handles (we do not).

Helper (replaces `current_repo_path`):

```rust
/// Canonical workdir path for `repo_id`, or `NoRepo` if it isn't open.
fn repo_path(state: &AppState, repo_id: &str) -> Result<std::path::PathBuf, AppError> {
    let repos = state
        .repos
        .lock()
        .map_err(|_| AppError::Other("state lock poisoned".to_string()))?;
    repos
        .get(repo_id)
        .map(|e| e.path.clone())
        .ok_or(AppError::NoRepo)
}
```

---

## 4. `src-tauri/src/commands.rs` — command surface

### 4.1 `repo-changed` payload gains `repoId`

```rust
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepoChangedPayload {
    pub repo_id: String,
    pub reason: String, // "fs" (unchanged meaning)
}
```

### 4.2 `open_repo` — returns id + info, inserts an entry, arms a per-repo watcher

```rust
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenRepoResult {
    /// Canonical workdir path (§2). Meaningful (map entry exists) only when
    /// `info` is a usable repo; still returned for non-usable opens so the
    /// frontend can key its error UI, but no entry/watcher is created.
    pub repo_id: String,
    pub info: RepoInfo,
}

#[tauri::command]
pub async fn open_repo(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    path: String,
) -> Result<OpenRepoResult, AppError>;

/// Runtime-free core. `make_on_change` is given the resolved `repo_id` and
/// returns the watcher callback for that repo; the command wires it to an
/// `app.emit("repo-changed", RepoChangedPayload { repo_id, reason: "fs" })`.
/// Tests pass `|_id| Box::new(|| {})` or a channel sender — no Tauri runtime.
async fn open_repo_inner<F>(
    state: &AppState,
    path: String,
    make_on_change: F,
) -> Result<OpenRepoResult, AppError>
where
    F: FnOnce(String) -> Box<dyn Fn() + Send + 'static>;
```

Behavior:
1. `read_repo_info(&path)` (via `spawn_blocking`) as today. Let `repo_id = info.path.clone()`.
2. **Usable** (`info.is_repo && !info.bare`):
   - Dedupe scan: if the map already contains an entry whose key
     `eq_ignore_ascii_case(&repo_id)`, use that existing key as `repo_id` (focus, no duplicate).
   - (Re)arm the watcher for this entry: build `on_change = make_on_change(repo_id.clone())`,
     `spawn_watcher(&workdir, on_change)`; **insert/replace** `RepoEntry { path, watcher }`. Re-open
     of an already-open repo therefore replaces its watcher (self-heal, same semantics as today's
     single-repo `open_repo`). Watch failure stays non-fatal (`watcher: None`, log to stderr).
   - Recents hook unchanged (record every usable open).
3. **Non-usable** (non-repo or bare): do **not** insert an entry and do **not** touch other entries.
   Return `OpenRepoResult { repo_id, info }` with no map mutation. (Contrast with the old behavior of
   clearing "the" repo — there is no single current repo anymore, so other tabs are untouched.)
4. Return `OpenRepoResult { repo_id, info }`.

> Note: opening a **bare/non-repo** no longer closes any tab. The frontend simply does not add a tab
> for a non-usable `OpenRepoResult`.

### 4.3 `close_repo` — remove an entry, tear its watcher down off-lock

```rust
#[tauri::command]
pub async fn close_repo(
    state: tauri::State<'_, AppState>,
    repo_id: String,
) -> Result<(), AppError>;

async fn close_repo_inner(state: &AppState, repo_id: &str) -> Result<(), AppError>;
```

Behavior (idempotent — closing an unknown/already-closed id is `Ok(())`, never `NoRepo`):

```rust
// Take the entry out UNDER the lock, then drop it OUTSIDE the lock so the
// WatcherHandle's debounce-thread join (≤ ~300 ms) doesn't hold the map lock.
let entry = {
    let mut repos = state.repos.lock().map_err(|_| /* poisoned */)?;
    repos.remove(repo_id)
};
drop(entry); // watcher stops, debounce thread joins here
Ok(())
```

### 4.4 Repo-scoped commands (all gain `repo_id`)

Every command below gains a leading `repo_id: String` on the wrapper and `repo_id: &str` on the
inner; each inner replaces `current_repo_path(state)?` with `repo_path(state, repo_id)?`. Everything
else (error kinds, `spawn_blocking` body, docs) is unchanged.

| Command | New wrapper signature (Tauri) | New inner signature |
|---|---|---|
| `get_status` | `(state, repo_id: String) -> Result<StatusSnapshot>` | `(state, repo_id: &str)` |
| `get_graph` | `(state, repo_id) -> Result<GraphLayout>` | `(state, repo_id: &str)` |
| `stage` | `(state, repo_id, paths: Vec<String>) -> Result<()>` | `(state, repo_id: &str, paths)` |
| `unstage` | `(state, repo_id, paths) -> Result<()>` | `(state, repo_id: &str, paths)` |
| `commit` | `(state, repo_id, message: String) -> Result<CommitResult>` | `(state, repo_id: &str, message)` |
| `get_workdir_file_diff` | `(state, repo_id, path, orig_path, staged) -> Result<FileDiff>` | `(state, repo_id: &str, …)` |
| `get_commit_diff` | `(state, repo_id, oid) -> Result<CommitDiff>` | `(state, repo_id: &str, oid)` |
| `get_commit_file_diff` | `(state, repo_id, oid, path, orig_path) -> Result<FileDiff>` | `(state, repo_id: &str, …)` |
| `list_branches` | `(state, repo_id) -> Result<BranchesSnapshot>` | `(state, repo_id: &str)` |
| `create_branch` | `(state, repo_id, name) -> Result<()>` | `(state, repo_id: &str, name)` |
| `checkout_branch` | `(state, repo_id, name) -> Result<()>` | `(state, repo_id: &str, name)` |
| `delete_branch` | `(state, repo_id, name) -> Result<()>` | `(state, repo_id: &str, name)` |
| `fetch` | `(state, repo_id) -> Result<FetchResult>` | `(state, repo_id: &str)` |
| `pull` | `(state, repo_id) -> Result<PullResult>` | `(state, repo_id: &str)` |
| `push` | `(state, repo_id) -> Result<PushResult>` | `(state, repo_id: &str)` |
| `get_op_state` | `(state, repo_id) -> Result<RepoOpState>` | `(state, repo_id: &str)` |
| `merge_branch` | `(state, repo_id, name) -> Result<MergeOutcome>` | `(state, repo_id: &str, name)` |
| `commit_merge` | `(state, repo_id, message) -> Result<CommitResult>` | `(state, repo_id: &str, message)` |
| `abort_merge` | `(state, repo_id) -> Result<()>` | `(state, repo_id: &str)` |
| `list_conflicts` | `(state, repo_id) -> Result<Vec<ConflictEntry>>` | `(state, repo_id: &str)` |
| `get_conflict` | `(state, repo_id, path) -> Result<ConflictFile>` | `(state, repo_id: &str, path)` |
| `resolve_conflict` | `(state, repo_id, path, resolution) -> Result<()>` | `(state, repo_id: &str, …)` |
| `rebase_branch` | `(state, repo_id, onto) -> Result<RebaseOutcome>` | `(state, repo_id: &str, onto)` |
| `rebase_continue` | `(state, repo_id) -> Result<RebaseOutcome>` | `(state, repo_id: &str)` |
| `rebase_skip` | `(state, repo_id) -> Result<RebaseOutcome>` | `(state, repo_id: &str)` |
| `rebase_abort` | `(state, repo_id) -> Result<()>` | `(state, repo_id: &str)` |

### 4.5 App-global commands (unchanged — NO `repo_id`)

`pick_folder` (dialog, frontend-only), `get_recent_repos`, `remove_recent_repo`, `get_ui_settings`,
`set_ui_settings`, and (new, §6) `get_session` / `set_session`. Events: `repo-changed` (now carries
`repoId`), window focus (frontend-only via the window API).

### 4.6 Command test migration (P3e-a)

The existing `*_require_an_open_repo` tests call each inner with an **empty map** — pass any dummy id
(e.g. `"missing"`) and still expect `AppError::NoRepo`. `open`/`get_status_inner` helpers in the test
module take the returned `repo_id` and thread it through. Add the §9.1 isolation tests.

### 4.7 `lib.rs`

Register `close_repo`, `get_session`, `set_session` in `generate_handler!`. `AppState::default()`
now yields an empty map. No other change.

---

## 5. Frontend

### 5.1 State partition — what moves, what stays

**Per-repo → moves into `src/components/RepoWorkspace.tsx`** (one instance per open tab, keyed by
`repoId`, holds its own copy of everything below):

- `repo: RepoInfo`, `error`/`loading` for its own open, `status` + `statusError` + `statusLoading`,
  `refreshing`, `mutating`, `branches` + `branchesError` + `branchesLoading`, `remoteOp`,
  `opState` + `conflicts` + `abortConfirmOpen`, `graph` + `graphError` + `graphLoading`,
  `selectedIndex`, `commitDiff` + `commitDiffLoading` + `commitDiffError`, `diffSlot`.
- All request-id refs (`statusReqId`, `graphReqId`, `branchesReqId`, `commitDiffReqId`,
  `fileDiffReqId`, `opStateReqId`, `statusErrorId`), `diffSlotRef`, `commitBoxRef`, `graphRef`.
- All the handlers/effects: `refetchStatus/Graph/Branches/OpState`, `clear*`, `refreshAll`,
  `openPath`-equivalent (now just its own refresh), `handleStage/Unstage/Commit/CreateBranch/
  Checkout/Delete`, `handleFetch/Pull/Push`, `handleMerge*/Rebase*/ResolveConflict/AbortMerge`,
  `fetchDiffSlot/collapseDiffSlot/fetchConflictSlot`, `overlayMeta`/`wip` memos, the
  `selectedIndex → commitDiff` effect, the Esc-layering effect, and the per-repo keydown shortcut
  effect (refresh / fetch / pull / push / arrow-page-home-end nav) — **installed only while
  `active` is true**.
- The right-panel / graph / sidebar / OpBanner / DiffOverlay / CommitBox / ConfirmDialog render tree
  (everything currently inside `<div className="panes">`).

**Every per-repo IPC call now passes the workspace's `repoId`** as the first argument
(`ipc.getStatus(repoId)`, `ipc.commit(repoId, msg)`, …). Its `onRepoChanged` subscription filters on
`p.repoId === repoId`.

**App-global → stays in `App.tsx`:**

- `theme` + `themeVersion`, `listView`, `paneWidths` (+ save timer + resize handlers), `recents`,
  `toasts` (now provided via `ToastContext`, §5.5), `overlayOpen` (the `?` ShortcutOverlay),
  `loading`/`error` of the initial pick, `launchedRef`.
- **New tab state:** `tabs: TabMeta[]` and `activeRepo: string | null` (§5.2).
- Truly global shortcuts: `Ctrl+O` open, `?` overlay, plus new `Ctrl+Tab`/`Ctrl+Shift+Tab` cycle
  tabs and `Ctrl+W` close active tab. (Refresh/fetch/pull/push/nav shortcuts belong to the active
  `RepoWorkspace`, §5.1 above.)
- `pickFolder` + open flow, `getSession`/`setSession` wiring, reopen-all-on-launch (§6),
  `getUiSettings`/`setUiSettings` for theme/panes/listView.

`switcherOpen`/`dialogOpen` lift-outs are replaced by a single `menuOpen` flag from `TabStrip`
(suppresses global shortcuts + skips the Esc keypress the strip consumed) plus each workspace's own
`dialogOpen` handling staying internal.

### 5.2 `App.tsx` tab model

```ts
interface TabMeta {
  repoId: string;
  path: string;
  // Optional nicety: workspace reports its head up (onHeadChange) so the tab
  // can show the branch name / dirty dot. Absent until first report.
  head?: HeadInfo | null;
}
```

- `openTab(path)`: `const { repoId, info } = await ipc.openRepo(path)`. If not `isUsableRepo(info)`
  → surface the empty-state error (no tab added). Else: if `repoId` already in `tabs` → just
  `setActiveRepo(repoId)` (focus). Else append `{ repoId, path: info.path }`, set active, persist
  session (§6).
- `closeTab(repoId)`: `await ipc.closeRepo(repoId)` (fire-and-forget-safe, idempotent); remove from
  `tabs`; if it was active, activate the right/left neighbor (or `null` if none left); persist
  session.
- Render: for **every** tab, mount `<RepoWorkspace key={repoId} repoId active={repoId===activeRepo}
  …/>` wrapped in a visibility container (§5.3). When `tabs.length === 0` render the existing empty
  state (Open repository + recents).

### 5.3 All tabs mounted, inactive `display:none`

App wraps each workspace:

```tsx
{tabs.map((t) => (
  <div
    key={t.repoId}
    className="workspace-host"
    style={{ display: t.repoId === activeRepo ? 'flex' : 'none' }}
  >
    <RepoWorkspace repoId={t.repoId} active={t.repoId === activeRepo} … />
  </div>
))}
```

Keeping inactive tabs mounted (not unmounted) is what makes switching **instant** and preserves each
tab's scroll position, selection, expanded diff, and in-flight state. `display:none` gives the
subtree a **zero client rect**, which is why `GraphCanvas` needs the §5.4 hardening.

### 5.4 `GraphCanvas` — zero-size guard + remeasure on show (P3e-d)

Add one prop; two guards. Backward compatible (`active` defaults to `true`).

```ts
export interface GraphCanvasProps {
  // …existing…
  /** False when the owning tab is display:none (zero-size). Defaults true. */
  active?: boolean;
}
```

Mechanism:
1. **Zero-size guard** in `resize()`: after reading `cssW = host.clientWidth`,
   `cssH = host.clientHeight`, `if (cssW === 0 || cssH === 0) return;` — do **not** shrink the
   backing store to 1×1 or paint while hidden (that would blank the bitmap and lose the rendered
   graph). The last good bitmap is retained for when the tab is shown again.
2. **Remeasure on show:** an effect keyed on `active` — when it flips to `true`, call `resize()`
   (which re-reads the now-nonzero host size, restores the backing-store dimensions, and repaints
   synchronously). This is the authoritative remeasure; it does not rely on `ResizeObserver` firing
   across the `display:none → shown` transition (which is unreliable). The existing `ResizeObserver`
   stays as the steady-state path.

The `active` prop reuses `active === (repoId === activeRepo)` from the workspace. Scroll position is
untouched (the scroller DOM persists), so a shown tab repaints at its prior `scrollTop`.

### 5.5 `ToastContext`

Toasts stay a single global stack in `App`. Expose `pushToast(tone, text)` via a
`ToastContext` (`createContext`) provided by `App`; `RepoWorkspace` and its children consume it
instead of receiving `pushToast` as a prop through the whole handler chain. A background tab's
watcher-driven refetch failure can still surface a toast — acceptable; prefixing the toast with the
repo folder name is an optional nicety (flag: minor).

### 5.6 `TabStrip.tsx` (replaces `RepoSwitcher.tsx`)

New component in `src/components/`. `RepoSwitcher.tsx` is deleted; its recents-dropdown + `Browse…`
affordances move onto the strip's `+` / overflow menu.

```ts
export interface TabStripProps {
  tabs: TabMeta[];
  activeRepo: string | null;
  recents: RecentRepo[];
  disabled: boolean;                 // during a global-busy op
  onSelect(repoId: string): void;
  onClose(repoId: string): void;
  onOpenPath(path: string): void;    // from recents (adds/focuses a tab)
  onBrowse(): void;                  // folder picker
  /** Lifts menu-open like RepoSwitcher.onOpenChange: suppresses global
   *  shortcuts and lets App's Esc effect skip the consumed keypress. */
  onMenuOpenChange?(open: boolean): void;
}
```

Behavior: one pill per tab showing `folderName(path)` (+ optional branch badge / dirty dot from
`head`), active pill highlighted, a per-tab `×` close button; a trailing `+` button opening the
recents dropdown + `Browse…` (same list/dedupe logic RepoSwitcher had, minus the current-repo
filter — instead filter out paths already open in a tab). Closing the last tab returns App to the
empty state.

---

## 6. Session persistence + reopen-on-launch

### 6.1 Backend (P3e-b)

`settings.rs` — additive fields on `Settings` (version stays `1`; both are `#[serde(default)]` via
the container-level `default`, exactly like `theme`/`list_view`; a legacy file without them loads
fine):

```rust
pub struct Settings {
    // …existing…
    /// Open tabs, in display order (repoIds == canonical workdir paths).
    pub open_repos: Vec<String>,
    /// The active tab's repoId; None ⇒ activate the first still-openable one.
    pub active_repo: Option<String>,
}
```

Commands (app-global; persisted through the same `settings.json` load/save used by ui-settings):

```rust
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionState {
    pub open_repos: Vec<String>,
    pub active_repo: Option<String>,
}

#[tauri::command]
pub async fn get_session(app: tauri::AppHandle) -> Result<SessionState, AppError>;

/// Writes the WHOLE session (tabs change as a unit — no partial patch). Save
/// failure surfaces as AppError::Io (like set_ui_settings, not swallowed).
#[tauri::command]
pub async fn set_session(app: tauri::AppHandle, session: SessionState) -> Result<(), AppError>;
```

Both go through `settings::load_from`/`save_to` on `spawn_blocking`, mirroring `get_ui_settings` /
`set_ui_settings`. `get_session` never rejects for a missing/corrupt file (defaults to empty).

### 6.2 Frontend (P3e-e) — reopen-all-on-launch

On mount (guarded by `launchedRef`), after loading recents + ui-settings:

1. `const session = await ipc.getSession()`.
2. If `session.openRepos.length > 0`: for each `path` **sequentially**, `await openTab(path)` but
   **catch per-path** — a path that no longer exists (or errors) is **skipped**: push a warning toast
   `Could not reopen <folderName>: <message>` and do not add a tab. Never let one failure abort the
   loop or crash launch.
3. Activate `session.activeRepo` if it ended up open, else the first successfully opened tab.
4. After the loop, `setSession` the surviving set (prunes dead paths from disk).
5. **Back-compat fallback:** if `session.openRepos` is empty but `recents.length > 0`, open
   `recents[0]` as a single tab (preserves the pre-P3e "reopen last repo on launch" behavior for
   existing users whose settings.json predates `openRepos`).

Persist (`setSession`, debounced ~300 ms like pane widths) on every `openTab` / `closeTab` /
active-tab change.

---

## 7. Watcher-per-repo

- Each `RepoEntry` owns its own `WatcherHandle` (unchanged `watcher.rs` — one recursive watch on the
  workdir, 300 ms debounce, `.git`-internals filter, clean drop-join). N open repos ⇒ N watchers.
- `open_repo_inner` builds the entry's `on_change` from `make_on_change(repo_id)`; the command wires
  it to `app.emit("repo-changed", RepoChangedPayload { repo_id, reason: "fs" })`. Reopening a repo
  replaces its watcher (self-heal).
- `close_repo` drops the entry off-lock (§4.3), stopping just that repo's watcher; other watchers are
  untouched.
- **Frontend routing:** each `RepoWorkspace` subscribes to `ipc.onRepoChanged` and ignores payloads
  whose `repoId !== myRepoId`; on a match it runs its composite refetch (status/graph/branches/
  opstate) — **regardless of active state**, so a background tab stays fresh when its watcher fires.
- **Focus rescan:** the **active** workspace refetches on `ipc.onWindowFocus` (the visible tab is the
  one a user just returned to). Background tabs rely on their own watchers plus a **composite refresh
  on activation** (the `active`-flip effect in `RepoWorkspace` calls `refreshAll`, self-healing any
  missed Windows events while it was hidden).

---

## 8. IPC surface (TypeScript) — exact changes

`src/ipc/types.ts` additions/changes:

```ts
export interface OpenRepoResult {
  repoId: string;
  info: RepoInfo;
}

export interface RepoChangedPayload {
  repoId: string;   // NEW
  reason: string;
}

export interface SessionState {
  openRepos: string[];
  activeRepo: string | null;
}
```

`IpcApi` — changed/new members (every repo-scoped method gains `repoId` as the **first** arg):

```ts
export interface IpcApi {
  openRepo(path: string): Promise<OpenRepoResult>;           // was Promise<RepoInfo>
  closeRepo(repoId: string): Promise<void>;                  // NEW
  pickFolder(): Promise<string | null>;                      // unchanged

  getStatus(repoId: string): Promise<StatusSnapshot>;
  getGraph(repoId: string): Promise<GraphLayout>;
  stage(repoId: string, paths: string[]): Promise<void>;
  unstage(repoId: string, paths: string[]): Promise<void>;
  commit(repoId: string, message: string): Promise<CommitResult>;
  getWorkdirFileDiff(repoId: string, path: string, origPath: string | null, staged: boolean): Promise<FileDiff>;
  getCommitDiff(repoId: string, oid: string): Promise<CommitDiff>;
  getCommitFileDiff(repoId: string, oid: string, path: string, origPath: string | null): Promise<FileDiff>;
  listBranches(repoId: string): Promise<BranchesSnapshot>;
  createBranch(repoId: string, name: string): Promise<void>;
  checkoutBranch(repoId: string, name: string): Promise<void>;
  deleteBranch(repoId: string, name: string): Promise<void>;
  fetch(repoId: string): Promise<FetchResult>;
  pull(repoId: string): Promise<PullResult>;
  push(repoId: string): Promise<PushResult>;
  getOpState(repoId: string): Promise<RepoOpState>;
  mergeBranch(repoId: string, name: string): Promise<MergeOutcome>;
  commitMerge(repoId: string, message: string): Promise<CommitResult>;
  abortMerge(repoId: string): Promise<void>;
  listConflicts(repoId: string): Promise<ConflictEntry[]>;
  getConflict(repoId: string, path: string): Promise<ConflictFile>;
  resolveConflict(repoId: string, path: string, resolution: ConflictResolution): Promise<void>;
  rebaseBranch(repoId: string, onto: string): Promise<RebaseOutcome>;
  rebaseContinue(repoId: string): Promise<RebaseOutcome>;
  rebaseSkip(repoId: string): Promise<RebaseOutcome>;
  rebaseAbort(repoId: string): Promise<void>;

  getRecentRepos(): Promise<RecentRepo[]>;                   // unchanged
  removeRecentRepo(path: string): Promise<RecentRepo[]>;     // unchanged
  onRepoChanged(cb: (p: RepoChangedPayload) => void): Promise<Unsubscribe>; // payload has repoId
  onWindowFocus(cb: () => void): Promise<Unsubscribe>;       // unchanged
  getUiSettings(): Promise<UiSettings>;                      // unchanged
  setUiSettings(patch: UiSettingsPatch): Promise<UiSettings>;// unchanged
  getSession(): Promise<SessionState>;                       // NEW
  setSession(session: SessionState): Promise<void>;          // NEW
}
```

`src/ipc/tauri.ts` — thread the arg through each `invoke` (Tauri converts `repoId` → `repo_id`):
`invoke('get_status', { repoId })`, `invoke('commit', { repoId, message })`, `invoke('close_repo',
{ repoId })`, `invoke('open_repo', { path })` now typed `Promise<OpenRepoResult>`, `invoke(
'get_session')`, `invoke('set_session', { session })`. `onRepoChanged` passes the full payload
(now includes `repoId`).

### 8.1 `src/ipc/mock.ts` — multi-repo stateful twin (P3e-c)

The current module-level singletons (`mockStatus`, `mockHeadOid`, `mockBranches`, `opState`, …)
become **per-repo**:

```ts
interface MockRepoState { status; headOid; branches; headBranch; fetched; commits; opState; conflicts; … }
const repos = new Map<string /*repoId*/, MockRepoState>();
```

- `openRepo(path)`: derive `repoId = MOCK canonical of path`; if absent in `repos`, create a
  `MockRepoState` seeded from the path (see below) and the existing `?op=`/`?fixture=` query params;
  return `{ repoId, info }`. Idempotent per `repoId` (re-open does not reset an existing tab's
  state — matches the real backend + the current `path !== openedPath` reset guard, now per-repo).
- **Per-repo seeding for distinct tabs:** query params (`?op=merge`, `?op=rebase`, `?fixture=
  detached`) still seed the **default** repo so single-tab harness flows are unchanged; additionally,
  a path containing `merge` / `rebase` / `detached` / `unborn` / `not-a-repo` / `bare` seeds that
  distinct state, so the harness can open multiple tabs with independent states by opening multiple
  mock paths. Document the substrings.
- Every repo-scoped mock method takes `repoId` first, looks up `repos.get(repoId)` (throws
  `{ kind: 'noRepo' }` if absent), and mutates only that entry — this is what makes the harness
  isolation flow real.
- `closeRepo(repoId)`: `repos.delete(repoId)`; resolves void (idempotent).
- `getSession`/`setSession`: back a small module-level `SessionState` (localStorage-backed so
  reopen-all survives a harness reload, mirroring how ui-settings/recents persist in the mock).
- `onRepoChanged`: still a no-op unsubscribe (no browser watcher). `onWindowFocus`: unchanged.

---

## 9. Testing

Scratch repos **only** under `D:\Temp\bonsai-scratch`; cargo tests set `TMP`/`TEMP` = `D:\Temp`
(C: is critically full). Oracle pattern where applicable: compare Bonsai results against a `git` CLI
twin repo.

### 9.1 Rust unit/integration (P3e-a) — two-repo isolation

In `commands.rs` tests (runtime-free, via `AppState::default()` + `open_repo_inner(state, path,
|_id| Box::new(|| {}))`):

- **Independent status/commit:** open repos A and B (two temp git2 repos). Stage+commit in A; assert
  `get_status_inner(state, id_b)` and `get_graph_inner(state, id_b)` are unaffected, and A reflects
  the change.
- **Independent branches/op-state:** `create_branch_inner(state, id_a, "x")`; assert
  `list_branches_inner(state, id_b)` has no `x`. Start a merge/rebase in A (or assert op-state reads
  independently); B's `get_op_state_inner` stays `none`.
- **Close isolation:** `close_repo_inner(state, id_a)`; assert `get_status_inner(state, id_a)` ⇒
  `NoRepo` while B still works; the map has exactly one entry.
- **Focus/dedupe:** opening A's path twice (incl. a case-variant on Windows) returns the same
  `repo_id` and leaves the map with one A entry.
- **Idempotent close:** `close_repo_inner` on an unknown id ⇒ `Ok(())`.
- Rewrite the existing `*_require_an_open_repo` tests to pass a dummy id against an empty map ⇒
  `NoRepo` for all 26 commands.

Session (P3e-b): `settings.rs` round-trip of `open_repos`/`active_repo`; a legacy `settings.json`
without those keys loads with empty defaults (extends the existing forward-compat tests).

### 9.2 Frontend / harness (orchestrator-verifiable) — AI GATE

- `pnpm build` (tsc) green with the new `IpcApi`; `mock.ts` compiles and is a faithful multi-repo
  twin.
- Harness multi-tab flow (`VITE_MOCK_IPC=1`): open 2+ tabs on distinct mock paths (e.g. a default,
  a `…merge`, a `…rebase` path); switch between them and confirm each tab's **graph, status, and
  op-state banner are independent** and that switching is instant with scroll/selection preserved
  (screenshot each tab). Close a tab (neighbor activates). Reload the harness and confirm
  reopen-all rehydrates the tabs from the persisted session; a removed/bogus persisted path is
  skipped with a warning toast, not a crash.
- `GraphCanvas` (P3e-d): switching to a previously-hidden tab shows a correctly-sized, non-blank
  graph (remeasure works); console frame-timing shows no sustained `>33 ms` frames after activation.
- `cargo test` (with `TMP`/`TEMP`=`D:\Temp`) + `cargo clippy -- -D warnings` + `pnpm build` all
  green.

Per CLAUDE.md, the AI gate is what the orchestrator verifies alone; the checkpoint below is
native-only and MUST NOT be self-declared.

### 9.3 USER CHECKPOINT (native `pnpm tauri dev`)

- Open multiple **real** repos; switching tabs feels instant and preserves each tab's scroll +
  selection.
- Each tab's file watcher auto-refreshes **independently** (edit a file in a background repo; when
  that tab is shown / when its watcher fires, its status updates) — plus manual refresh and refocus
  rescan behave per tab.
- Close a tab (its watcher stops) and confirm the others keep working; quit and relaunch — all tabs
  reopen with the previously-active tab focused; a repo whose folder was deleted between sessions is
  skipped with a toast rather than blocking launch.

---

## 10. Open questions / flags for the orchestrator

1. **`repoId == canonical path` (recommended, §2).** Simplest, stable, dedupes for free. Alternative
   opaque ids rejected (need a persisted id↔path map). Confirm before implementing if you foresee
   opening the *same* repo via two different worktree paths (out of scope in v1).
2. **Focus rescan scope (§7):** recommended = active tab refetches on window focus; background tabs
   self-heal on activation. Alternative (refetch *all* tabs on focus) is heavier and mostly wasted on
   hidden tabs — flag if you want all-tabs-on-focus instead.
3. **Tab branch/dirty badge (§5.2/§5.6):** optional nicety requiring an `onHeadChange` lift from
   workspace to `App`. Marked minor; drop it if it complicates the P3e-e pass.
4. **Toast repo-name prefixing (§5.5):** optional; helps disambiguate toasts from background tabs.

---

Report: contract written. This file is authoritative for P3e.
