# M1 — Working-Directory Status: Implementation Contract

Status: authoritative for M1. Implementer: senior-dev. Builds on `docs/contracts/M0-scaffold.md`
(types, error shape, IPC conventions) and `docs/contracts/ui-reference.md` (§7 file status colors,
§8 empty/loading states). Do not change M0 behavior except where this contract says so.

Goal: right panel shows staged / unstaged / untracked files via git2; auto-refresh via a debounced
`notify` watcher + a functional manual refresh button + rescan on window focus.

---

## 1. New / changed files

```
src-tauri/
  Cargo.toml                 # + notify = "8"; [lib] rename (§8)
  src/main.rs                # bonsai_lib::run() after lib rename
  src/lib.rs                 # + mod watcher; register get_status
  src/state.rs               # + watcher handle slot
  src/error.rs               # + AppError::NoRepo
  src/commands.rs            # + get_status; open_repo gains AppHandle + watcher lifecycle + bare gate
  src/git/repo.rs            # RepoInfo gains `bare: bool`
  src/git/status.rs          # NEW: status core + unit tests   (git/mod.rs: pub mod status;)
  src/watcher.rs             # NEW: notify watcher + debounce thread + tests
src/
  ipc/types.ts               # + StatusSnapshot, StatusEntry, FileStatus, RepoChangedPayload; IpcApi extended
  ipc/tauri.ts               # + getStatus, onRepoChanged, onWindowFocus
  ipc/mock.ts                # + fixtures for all new methods
  components/StatusPanel.tsx # NEW: right-panel status UI
  App.tsx                    # wire StatusPanel, refresh button, event/focus subscriptions
```

## 2. Status data model

**Decision: split-lists model.** `StatusSnapshot { staged, unstaged, untracked, conflicted }`,
each a `Vec<StatusEntry>`. Rationale: the UI renders exactly these sections; Rust owns the
classification (invariant: React only renders); a file that is staged AND modified again simply
appears in both `staged` and `unstaged` — no frontend bitflag logic. Conflicts get their own list:
representable now, acted on in a later milestone (M1 renders the section only when non-empty).

### Rust (`src-tauri/src/git/status.rs`)

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum FileStatus {
    Added,
    Modified,
    Deleted,
    Renamed,
    Typechange,
    Conflicted,
    Untracked,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusEntry {
    /// Repo-relative path, forward slashes (as git2 reports). For renames: the NEW path.
    pub path: String,
    /// For renames: the OLD path. `None` otherwise.
    pub orig_path: Option<String>,
    pub status: FileStatus,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusSnapshot {
    pub staged: Vec<StatusEntry>,     // index vs HEAD
    pub unstaged: Vec<StatusEntry>,   // workdir vs index (tracked files only)
    pub untracked: Vec<StatusEntry>,  // status == Untracked
    pub conflicted: Vec<StatusEntry>, // status == Conflicted
}
```

Each list is sorted by `path` ascending, ordinal (byte-wise) comparison — deterministic, matches
git's own ordering closely enough for tests.

### git2::Status bitflag mapping (exact)

One `git2::StatusEntry` may set several bits; emit one `StatusEntry` per matching row below (so
`INDEX_MODIFIED | WT_MODIFIED` produces one staged entry AND one unstaged entry).

| git2 bit | → list | → FileStatus | orig_path source |
|---|---|---|---|
| `INDEX_NEW` | staged | Added | — |
| `INDEX_MODIFIED` | staged | Modified | — |
| `INDEX_DELETED` | staged | Deleted | — |
| `INDEX_RENAMED` | staged | Renamed | `entry.head_to_index().old_file().path()` |
| `INDEX_TYPECHANGE` | staged | Typechange | — |
| `WT_MODIFIED` | unstaged | Modified | — |
| `WT_DELETED` | unstaged | Deleted | — |
| `WT_RENAMED` | unstaged | Renamed | `entry.index_to_workdir().old_file().path()` |
| `WT_TYPECHANGE` | unstaged | Typechange | — |
| `WT_NEW` | untracked | Untracked | — |
| `CONFLICTED` | conflicted | Conflicted | — |
| `IGNORED` | excluded entirely | — | — |
| `CURRENT` (no bits) | excluded | — | — |

A `CONFLICTED` entry is emitted ONLY to `conflicted` (suppress its INDEX_*/WT_* companions —
conflict resolution is a later milestone; double-listing would confuse the UI).

`path` comes from the relevant delta's `new_file().path()` when a delta exists, else
`entry.path()`. Non-UTF-8 paths: use `String::from_utf8_lossy` — do not error.

## 3. `get_status` command

### Core function (pure, testable, no Tauri types)

```rust
// git/status.rs
/// Blocking. Opens the repo at `workdir` (NO_SEARCH, like read_repo_info) and computes the
/// snapshot. Errors: path not a repo -> AppError::Git; bare repo -> AppError::Git with message
/// "cannot compute status: repository is bare" (defensive; open_repo already gates bare, §3.3).
pub fn read_status(workdir: &std::path::Path) -> Result<StatusSnapshot, AppError>;
```

`git2::StatusOptions` — exactly:

```rust
let mut opts = git2::StatusOptions::new();
opts.include_untracked(true)
    .recurse_untracked_dirs(true)   // individual files, matching --untracked-files=all
    .include_ignored(false)
    .include_unmodified(false)
    .renames_head_to_index(true)    // staged rename detection
    .renames_index_to_workdir(true) // worktree rename detection
    .exclude_submodules(true);      // v1: no submodule support
// Do NOT set update_index(true): status stays strictly read-only.
```

### Command (`commands.rs`)

```rust
#[tauri::command]
pub async fn get_status(state: tauri::State<'_, AppState>) -> Result<StatusSnapshot, AppError> {
    // 1. lock state.repo; None -> return Err(AppError::NoRepo)
    // 2. clone the PathBuf, drop the lock
    // 3. tauri::async_runtime::spawn_blocking(move || read_status(&path)).await
    //    (join error -> AppError::Other, same pattern as open_repo)
}
```

New error variant (`error.rs`):

```rust
#[error("no repository is open")]
NoRepo,
// kind() -> "noRepo"; message() -> "no repository is open"
```

TS: `AppError.kind` union becomes `'git' | 'io' | 'other' | 'noRepo'`.

### Bare repos — decision (M0 carry-over, locked)

Bonsai v1 is a working-copy client: status, staging, diffs, and the watcher all require a workdir.
**Bare repos are rejected at open**, not half-supported:

- `RepoInfo` gains `pub bare: bool` (`repo.is_bare()`); TS mirror gains `bare: boolean`.
  `read_repo_info` fills it; existing M0 tests add `assert!(!info.bare)` where relevant, plus a new
  test: `Repository::init_bare` fixture → `is_repo: true, bare: true`, head still reported.
- `open_repo`: when `info.bare`, do NOT store the repo in state and do NOT start a watcher; return
  the info as-is. Frontend treats `bare: true` like `isRepo: false` — stays on the empty state with
  banner "Bare repositories are not supported" + path.
- `read_status` still carries the defensive bare error (§3 core fn) in case of races.

### Unborn HEAD

No special casing: git2 diffs the index against the empty tree when HEAD is unborn, so staged files
appear as `staged: Added`. Must work (test in §7) — this is the "first commit" flow.

## 4. Watcher (`src-tauri/src/watcher.rs`)

Dependency: `notify = "8"` (uses `ReadDirectoryChangesWatcher` on Windows via
`notify::recommended_watcher`). Remember the platform truth: this watcher misses events on Windows,
which is why the manual refresh button and focus rescan (§5) are mandatory companions, not extras.

### Public surface

```rust
pub struct WatcherHandle {
    watcher: notify::RecommendedWatcher,          // dropped first -> callbacks stop, tx drops
    debounce_thread: Option<std::thread::JoinHandle<()>>,
}

impl Drop for WatcherHandle {
    // drop self.watcher (implicit field order: declare watcher first), then
    // join debounce_thread (it exits promptly on channel disconnect).
}

/// Non-blocking to callers beyond initial watch registration (fast).
/// `on_change` fires on the debounce thread, at most once per quiet period.
pub fn spawn_watcher(
    workdir: &std::path::Path,
    git_dir: &std::path::Path,   // repo.path() — pass in, don't reopen the repo here
    on_change: Box<dyn Fn() + Send + 'static>,
) -> Result<WatcherHandle, AppError>;
```

Decoupled from Tauri on purpose: `commands.rs` wires `on_change` to
`app.emit("repo-changed", RepoChangedPayload { reason: "fs" })`; tests wire it to a channel.

### What is watched

One recursive watch on the **workdir** (`.git` lives inside it, so its events arrive on the same
watch). Do NOT add a second full-`.git` watch. Filter every event path:

```text
is_relevant(path, git_dir):
    if path is not under git_dir:            return true    # workdir content change
    rel = path relative to git_dir
    if rel filename ends with ".lock":       return false   # index.lock etc. — churn
    if rel == "HEAD" or rel == "index"
       or rel starts_with "refs"
       or rel starts_with "packed-refs":     return true    # checkout/commit/branch/fetch
    return false                                            # objects/, logs/, gc noise
```

Notes: git worktrees (git_dir outside workdir) are out of scope for v1 — if
`!git_dir.starts_with(workdir)`, additionally watch `git_dir` non-recursively plus `git_dir/refs`
recursively is NOT required; document the limitation in a code comment and move on.
Error events from notify (`Err(_)` in the handler) count as relevant (trigger a refresh — cheap
and safe).

### Debounce — 300 ms trailing edge

Mechanism: `std::sync::mpsc` + one dedicated thread (no tokio dependency in this module).
The notify event handler (runs on notify's thread) applies `is_relevant` and on a match does
`tx.send(())` (ignore send errors). Debounce thread:

```text
loop:
    match rx.recv():                        # block until a storm starts
        Err(Disconnected) -> return         # watcher dropped -> clean shutdown
        Ok(()) ->
            loop:                           # drain until 300 ms of quiet
                match rx.recv_timeout(300ms):
                    Ok(())            -> continue      # storm ongoing, keep absorbing
                    Err(Timeout)      -> { on_change(); break }
                    Err(Disconnected) -> return
```

Properties: never fires mid-storm; exactly one callback per quiet period; zero timers when idle.

### Lifecycle & state

`state.rs`:

```rust
#[derive(Default)]
pub struct AppState {
    pub repo: std::sync::Mutex<Option<OpenRepo>>,
    pub watcher: std::sync::Mutex<Option<crate::watcher::WatcherHandle>>, // separate lock: OpenRepo stays Clone/Debug
}
```

(`AppState` loses `derive(Debug)` — `WatcherHandle` isn't Debug; that's fine.)

`open_repo` gains `app: tauri::AppHandle` as its first param and, after a successful non-bare open:
1. `*state.watcher.lock() = None;` — drops the old handle: old watcher stops, old debounce thread
   joins. Do this BEFORE storing the new repo path.
2. `spawn_watcher(workdir, git_dir, Box::new(move || { let _ = app.emit("repo-changed", ...); }))`.
   `git_dir` comes from the `Repository` opened inside `read_repo_info` — extend `RepoInfo` is NOT
   needed; instead have `read_repo_info` internals stay as-is and compute `git_dir` in the command
   as `workdir.join(".git")` after a cheap `Repository::open_ext` — OR simpler and preferred:
   `spawn_watcher` takes only `workdir` and derives `git_dir = workdir.join(".git")` itself
   (valid because bare repos and worktrees are excluded above). **Pick the simpler form:**
   `pub fn spawn_watcher(workdir: &Path, on_change: ...)`.
3. Watch failure is non-fatal: log `eprintln!`, leave `watcher = None`, still return `Ok(info)` —
   manual refresh + focus rescan keep the app correct.

Re-invoking `open_repo` on the same path (used by the refresh button, §5) is idempotent and
self-healing: it replaces the watcher.

Thread-safety: `RecommendedWatcher` is `Send` on Windows; holding it inside a `Mutex<Option<...>>`
in managed state satisfies Tauri's `Sync` requirement. Never move the watcher across an `.await`.

### Event

- Name: `"repo-changed"` (app-global `app.emit`).
- Payload: `#[derive(Serialize)] #[serde(rename_all = "camelCase")] pub struct RepoChangedPayload { pub reason: String }`
  — always `"fs"` in M1. Future reasons (e.g. `"op"` after commit) reuse this event.
- No data beyond the reason: the frontend re-invokes `get_status` (invariant: events are small
  push signals, commands carry the data).

## 5. Frontend

### IPC layer (`src/ipc/*`)

`types.ts` additions:

```ts
export type FileStatus =
  | 'added' | 'modified' | 'deleted' | 'renamed'
  | 'typechange' | 'conflicted' | 'untracked';

export interface StatusEntry {
  path: string;
  origPath: string | null;
  status: FileStatus;
}

export interface StatusSnapshot {
  staged: StatusEntry[];
  unstaged: StatusEntry[];
  untracked: StatusEntry[];
  conflicted: StatusEntry[];
}

export interface RepoChangedPayload { reason: string }

export type Unsubscribe = () => void;

export interface IpcApi {
  openRepo(path: string): Promise<RepoInfo>;
  pickFolder(): Promise<string | null>;
  getStatus(): Promise<StatusSnapshot>;                                   // rejects AppError
  onRepoChanged(cb: (p: RepoChangedPayload) => void): Promise<Unsubscribe>;
  onWindowFocus(cb: () => void): Promise<Unsubscribe>;
}
```

`RepoInfo` gains `bare: boolean`; `AppError['kind']` gains `'noRepo'`.

`tauri.ts`:
- `getStatus: () => invoke<StatusSnapshot>('get_status')`
- `onRepoChanged`: `import { listen } from '@tauri-apps/api/event'`;
  `listen<RepoChangedPayload>('repo-changed', (e) => cb(e.payload))` — returns the `UnlistenFn`.
- `onWindowFocus`: `import { getCurrentWindow } from '@tauri-apps/api/window'`;
  `getCurrentWindow().onFocusChanged(({ payload: focused }) => { if (focused) cb(); })`.
- Capabilities: **no change needed.** `core:default` already includes `core:event:default`
  (event listen) and `core:window:default`; `onFocusChanged` only listens to `tauri://focus` /
  `tauri://blur`. Verify at implement time that `capabilities/default.json` still contains
  `core:default`; add nothing new.

`mock.ts`:
- `getStatus()` → `delay(150)` then the fixture below.
- `onRepoChanged(cb)` → resolves to a no-op unsubscribe; never fires (documented in a comment).
- `onWindowFocus(cb)` → wraps the browser's `window.addEventListener('focus', cb)`; unsubscribe
  removes it. (This lets the harness exercise the focus-refetch path for real.)

Mock `StatusSnapshot` fixture (exercises every render path, including the both-lists file):

```ts
{
  staged: [
    { path: 'src/app.rs',            origPath: null,              status: 'added'    },
    { path: 'src/main.rs',           origPath: null,              status: 'modified' },
    { path: 'docs/getting-started.md', origPath: 'docs/intro.md', status: 'renamed'  },
    { path: 'src/shared/util.rs',    origPath: null,              status: 'modified' }, // also unstaged below
  ],
  unstaged: [
    { path: 'src/shared/util.rs',    origPath: null,              status: 'modified' },
    { path: 'README.md',             origPath: null,              status: 'modified' },
    { path: 'old-config.toml',       origPath: null,              status: 'deleted'  },
  ],
  untracked: [
    { path: 'notes/todo.txt',        origPath: null,              status: 'untracked' },
    { path: 'scratch.rs',            origPath: null,              status: 'untracked' },
  ],
  conflicted: [],
}
```

### Status panel (`src/components/StatusPanel.tsx`)

Props: `{ snapshot: StatusSnapshot | null; loading: boolean; error: string | null }` — pure
presentational; all fetching lives in `App.tsx`.

- Sections in order: **Staged / Unstaged / Untracked**, plus **Conflicts** (danger-styled header)
  only when `conflicted.length > 0`. Section header: ui-reference §1 style (11px uppercase,
  text-3) + count, e.g. `STAGED (4)`.
- File row: letter badge (mono 11px) + colors per ui-reference §7 — A `--success`, M `--warning`,
  D `--danger`, R `--accent`, T `--warning` (badge `T`), U `--text-3` italic, C `--danger`
  (badge `C`). Path rendering: directory part in `--text-3`, filename in `--text-1`; renames show
  `origPath → path` (mono, title attr = full text). Row height 24px, truncate with ellipsis.
- Empty: all four lists empty → centered text-3 "No changes".
- Loading: skeleton rows (ui-reference §8) ONLY when `snapshot === null`; refreshes keep showing
  the previous snapshot (no flicker).
- Error: inline dismissible banner at panel top (ui-reference §8).

### App wiring (`App.tsx`)

- New state: `status: StatusSnapshot | null`, `statusError: string | null`,
  `statusLoading: boolean`.
- `refetchStatus()` with a **request-id guard** (module-level or ref counter): increment id, call
  `ipc.getStatus()`, apply result/error only if the captured id still equals the latest. No
  frontend debounce beyond this — the backend already debounces; last-wins is sufficient.
- On successful repo open (non-bare): call `refetchStatus()`; subscribe `ipc.onRepoChanged(() =>
  refetchStatus())` and `ipc.onWindowFocus(() => refetchStatus())` in a `useEffect` keyed on the
  open repo path; clean up both unsubscribes on change/unmount.
- **Refresh button** (header, now enabled when a repo is open): calls `ipc.openRepo(repo.path)`
  (re-reads HEAD for the header AND self-heals the watcher) then `refetchStatus()`. Disabled while
  either is in flight.
- `bare: true` open result → stay on empty state, banner "Bare repositories are not supported"
  (see §3.3).
- Right panel placeholder is replaced by `<StatusPanel …/>`.

## 6. IPC / lib.rs registration

`lib.rs`: add `pub mod watcher;`, register `commands::get_status` in `generate_handler!`.
Command surface after M1: `open_repo(path)`, `get_status()`. Events: `repo-changed`. Channels:
none yet.

## 7. Testing (contract for tester)

### Status correctness — compare against `git status --porcelain=v1 -z --untracked-files=all`

Porcelain **v1** (locked): its two XY columns map 1:1 onto our staged/worktree split and `-z`
removes quoting/escaping concerns; v2's extra data (modes, oids) is unused here. Scratch repos are
built with the **git CLI** in `tempfile::TempDir`s (this is the point: independent oracle vs our
git2 code); set `user.name`/`user.email` via `git -C <dir> config`. Also run
`git config status.renames true` explicitly (rename parity with our options).

Comparison approach (helper in `#[cfg(test)]` of `git/status.rs` or `tests/status_cli.rs`):
1. Run the porcelain command; split output on NUL. Entries: `XY path` (2 chars, space, path);
   rename entries (`X == 'R'` or `Y == 'R'`) consume the NEXT NUL-separated token as `orig_path`.
2. Map each entry to the same canonical tuples our snapshot produces:
   - X (index) column: `A→(staged, Added)`, `M→(staged, Modified)`, `D→(staged, Deleted)`,
     `R→(staged, Renamed + orig)`, `T→(staged, Typechange)`.
   - Y (worktree) column: `M→(unstaged, Modified)`, `D→(unstaged, Deleted)`,
     `R→(unstaged, Renamed + orig)`, `T→(unstaged, Typechange)`.
   - `??` → `(untracked, Untracked)`; `UU`/`AA`/`DD`/etc. → `(conflicted, Conflicted)` once.
3. Flatten our `StatusSnapshot` to the same tuple set; assert set equality (sorted vecs).

Required scenarios (one test each):
1. clean repo (one commit) → all lists empty.
2. untracked files, including one nested in a new directory (verifies recurse_untracked_dirs).
3. staged new file.
4. modified tracked file, unstaged.
5. staged modification, then modified again → appears in BOTH staged and unstaged.
6. deleted file: (a) deletion staged (`git rm`), (b) deletion unstaged (fs delete only).
7. staged rename (`git mv`) → `staged: Renamed` with correct `orig_path` both sides.
8. unborn repo with a staged file → `staged: Added`; porcelain agrees (`A ` on unborn HEAD).
9. bare repo → `read_status` returns `Err(AppError::Git(_))`.
10. no-repo path → `Err` (not a panic).

### Watcher tests (`watcher.rs` `#[cfg(test)]`, timing-sensitive — generous timeouts)

Wire `on_change` to an `mpsc::Sender<Instant>`; fixture = git2-init'd repo in a temp dir.
- `fires_once_after_touch`: write one file → expect exactly 1 callback within **5 s**; then assert
  no second callback for 1 s.
- `storm_coalesces`: write 50 files in a tight loop (< 300 ms total) → expect ≥1 and **≤ 2**
  callbacks within 5 s total observation.
- `git_internals_filtered`: write into `.git/objects/aa/dummy` → expect NO callback within 1.5 s;
  then touch `.git/HEAD` → expect 1 callback within 5 s.
- `drop_is_clean`: create handle, drop it, assert no hang (test simply completes) and no further
  callbacks after a workdir write.
Mark all four `#[test]` (not `#[ignore]`) but keep every wait generous as above; if CI flakiness
appears later, promote to `#[ignore]`-by-default — orchestrator's call, not tester's.

### Frontend smoke (browser harness, `VITE_MOCK_IPC=1 pnpm dev`)

Open mock repo → Staged (4) / Unstaged (3) / Untracked (2) render with correct badges/colors;
`src/shared/util.rs` appears in both sections; rename row shows `docs/intro.md →
docs/getting-started.md`; refresh button click re-renders without errors; browser-window
focus/blur triggers a refetch (visible in the network-less console via a debug log or React
DevTools); no `@tauri-apps/*` module executed.

## 8. pdb collision fix (M0 carry-over, locked)

On Windows/MSVC the bin `bonsai.exe` and lib `bonsai` both emit `bonsai.pdb` → collision warning.
**Fix: rename the lib to the create-tauri-app convention** in `src-tauri/Cargo.toml`:

```toml
[lib]
name = "bonsai_lib"
crate-type = ["staticlib", "cdylib", "rlib"]
```

and change `main.rs` to `bonsai_lib::run()`. Package name stays `bonsai` (bin stays
`bonsai.exe`). Chosen over renaming the bin because CTA convention keeps future tooling/doc
expectations aligned; churn is a two-line diff.

## 9. Sub-increment split for senior-dev

- **M1a — Status core + command.** `git/status.rs` (+ porcelain comparison tests §7),
  `AppError::NoRepo`, `RepoInfo.bare` (+ bare tests), `get_status` command registered, §8 lib
  rename. Gate: `cargo test` green, `cargo clippy -- -D warnings` clean.
- **M1b — Watcher + event.** `watcher.rs` (+ §7 watcher tests), `AppState.watcher`, `open_repo`
  lifecycle (AppHandle param, replace-on-open, bare gate, non-fatal watch failure), `notify` dep.
  Gate: `cargo test` green including watcher tests; manual sanity: `pnpm tauri dev`, touch a file,
  see one `repo-changed` in devtools console (orchestrator may defer this bit to the USER
  CHECKPOINT).
- **M1c — Frontend.** `types.ts`/`tauri.ts`/`mock.ts` extensions, `StatusPanel.tsx`, `App.tsx`
  wiring (refetch guard, subscriptions, refresh button, bare banner). Gate: `pnpm build` green;
  §7 frontend smoke passes in the harness.

## 10. Acceptance criteria (from CLAUDE.md)

AI gate:
- Rust tests compare `read_status` output to `git status --porcelain` on scratch repos (§7
  scenarios all pass); watcher tests pass.
- `cargo check`/`clippy`, `cargo test`, `pnpm build` all green; no pdb collision warning.
- Browser harness renders the three status sections from mock data per §7 smoke list.

USER CHECKPOINT (never self-declared):
- In the native app (`pnpm tauri dev`) on a real repo: editing/creating/deleting files updates the
  panel automatically (within ~1 s after the debounce); the manual refresh button works; alt-tabbing
  away, changing files externally, and refocusing the window rescans and shows the changes.

## 11. Ambiguities resolved here (flag to orchestrator if disagreed)

- **Bare repos rejected at open** (RepoInfo.bare + frontend banner) rather than half-opened —
  every M1+ feature assumes a workdir. Revisit only if a bare-repo browsing use case appears.
- **Split-lists StatusSnapshot** over a flat entry list with two status fields — moves all
  classification into Rust, keeps React render-only.
- **Conflicted entries suppress their INDEX_/WT_ companions** and live in their own list; UI shows
  the section only when non-empty. Full conflict UX is out of scope until merges exist (post-M6).
- **Refresh button reuses `open_repo`** (then `get_status`) instead of a new `get_repo_info`
  command — it refreshes HEAD for the header and self-heals a dead watcher for free. Add a
  dedicated lighter command only if this ever shows latency.
- **No frontend debounce** on `repo-changed` — backend debounce (300 ms trailing) plus a
  request-id last-wins guard is sufficient; double-debouncing adds latency for nothing.
- **Porcelain v1 (with `-z`)** as the test oracle, not v2 — simpler parsing, exactly the
  information we compare.
- **`spawn_watcher` derives `git_dir = workdir/.git`** — safe because bare repos are rejected and
  linked worktrees are declared out of scope for v1 (comment in code).
- **notify watch failure is non-fatal** — the mandated manual refresh + focus rescan are the
  fallback path; the repo still opens.
