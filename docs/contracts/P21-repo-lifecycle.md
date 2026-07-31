# P21 — Repo lifecycle: clone + init

User request: the app can only OPEN an existing repo today. Add the two missing lifecycle
operations, both ending in the EXISTING multi-repo tab flow:

1. **Clone** a remote repository (`url` → destination folder), **streaming progress**, then open it
   in a new tab.
2. **Init** a brand-new empty repository at a chosen folder, then open it in a new tab.

New Rust module `crates/bonsai-core/src/git/clone.rs`, two commands (`clone_repo`, `init_repo`), a
`CloneProgress` wire type carried over a **Tauri `Channel`**, the IPC triple, a **CloneDialog**, and
"Clone repository…" / "New repository…" affordances in the tab `+` menu and the no-repo empty state.

Reference contracts (patterns reused verbatim): `docs/contracts/M6-remotes.md` (the credential chain
`acquire_cred`/`CredAttempts`/`map_remote_err` + `RemoteCallbacks`→`FetchOptions` wiring reused by
clone's fetch), `docs/contracts/P19-submodules.md` (module + command + IPC-triple + stateful-mock +
open-in-tab-reuses-`openTab` structural spine).

Source files to mirror (exact patterns):
- `crates/bonsai-core/src/git/remote.rs:70-189` — `CRED_EXHAUSTED_MSG`, `CredAttempts`,
  `acquire_cred`, `map_remote_err` (all `pub(crate)` — clone.rs is in the same crate and uses them
  **as-is, no visibility change**); `remote.rs:207-247` `fetch_remote` shows the exact
  `RemoteCallbacks{credentials,…}` → `FetchOptions` wiring clone mirrors (adding `transfer_progress`).
- `crates/bonsai-core/src/git/repo.rs:35-69` — `read_repo_info` (opens with `NO_SEARCH`; unborn-HEAD
  handling already exists, `repo.rs:76-88`; init produces exactly this openable empty/unborn repo).
- `src-tauri/src/commands.rs:80-103,403-419` — `open_repo` + runtime-free `open_repo_inner`;
  `commands.rs:1-38` import block; `spawn_blocking` + join-error → `AppError::Other` template.
- `src-tauri/src/lib.rs:15-85` — `generate_handler!` registration + `.plugin(tauri_plugin_dialog…)`.
- `src/App.tsx:190-220` `openTab` (calls `ipc.openRepo` + adds/focuses tab; **clone/init reuse this,
  no new open command**); `App.tsx:408-419` `handleOpenRepository` (pickFolder → openTab template);
  `App.tsx:588-593` `<TabStrip onBrowse/onOpenPath>`; the no-repo empty state (`App.tsx:660-690`).
- `src/components/TabStrip.tsx:105-155` — the `+` menu (`Browse…` item) where clone/init entries go.
- `src/components/PromptDialog.tsx` + `ConfirmDialog.tsx` — dialog shell, focus/Esc/overlay-click
  discipline, `.dialog-*` CSS classes CloneDialog reuses.
- `src/ipc/tauri.ts:1-65` — `invoke`/`open`(plugin-dialog) imports + `pickFolder`; `src/ipc/mock.ts:
  932-979` `openRepo`/`pickFolder`/`createRepoState`; `mock.ts:280-315` path-substring seeding
  (`unborn` → empty repo) — clone/init mocks exploit this.

---

## OPEN DECISIONS (recommended defaults chosen; implementation is NOT blocked)

1. **Channel vs busy-state for clone progress.** → **DECISION: a Tauri `Channel<CloneProgress>`.**
   There is currently **no `tauri::ipc::Channel` usage anywhere in `src-tauri`** (grep: zero hits), so
   this establishes the channel precedent that CLAUDE.md always anticipated ("channels = streaming
   large/incremental data"). Clone is the textbook case: a single, potentially minutes-long transfer
   whose git2 `transfer_progress` callback emits natural, monotonic increments that map 1:1 to a
   determinate progress bar. Unlike M6 fetch/pull (seconds-scale, multi-remote progress is muddy, so
   M6 §9 deferred to the global `mutating` flag), a clone with no progress bar looks hung. `Channel`
   is `Clone + Send + Sync + 'static`, so `channel.send(p)` bridges cleanly out of `spawn_blocking`
   (§2.3). *(Rejected alternative — busy-state only: simplest, but a large clone would pin the UI in
   an indeterminate spinner for minutes; the payload the task pre-specified exists precisely to avoid
   this.)* **No cancellation in v1** (documented consequence): the progress callback always returns
   `true`; there is no in-app abort — closing the dialog stops updating the UI but libgit2 runs to
   completion on the blocking thread. A cancel token is Polish.
2. **Does the command open/register the repo, or just return the path?** → **Recommend: the command
   returns the absolute repo path; the FRONTEND calls the existing `openTab(path)`** (which invokes
   `open_repo`, registers the `RepoEntry`, arms the watcher, records recents, adds/focuses the tab).
   This mirrors P19 "open submodule in new tab" (reuse `openTab`, no new open path) and keeps ONE
   registration codepath. *(Rejected: clone/init also register the repo themselves — duplicates
   `open_repo`'s dedupe/watcher/recents logic and splits the invariant.)*
3. **`init` on a path that is already a repo.** → **Recommend: open it (idempotent), return its
   workdir path — do NOT error and do NOT reinitialize.** "New repository here" on an existing repo
   most kindly means "just open it." Detected via `Repository::open_ext(NO_SEARCH)` before init.
   *(Alternative: error `alreadyRepo` — rejected: no user value; the tab flow dedupes anyway.)*
4. **`dest` already exists and is non-empty (clone).** → **Recommend: pre-check and reject with
   `AppError::Io`** (a clear message) BEFORE starting the transfer, plus map any late git2
   "exists and is not an empty directory" as a fallback. **NO new `AppError` variant** (P19
   discipline). An empty existing dir or a missing dir is fine (git2 creates it). *(Alternative:
   dedicated `DestExists` kind — deferred; the message is self-explanatory and the UI shows it
   verbatim.)*
5. **Where do clone credentials come from (no repo config yet)?** → **`git2::Config::open_default()`**
   (global + system config — where the credential helper / GCM is configured). M6 uses `repo.config()`
   because a repo exists; clone has none until the transfer starts, so the default config is the
   correct source. Same `acquire_cred` chain otherwise (helper → SSH agent → default).
6. **Destination-folder picker.** → **Reuse the existing `ipc.pickFolder()`** for BOTH the clone
   destination and the init folder (its dialog title reads "Open repository" — cosmetic, acceptable in
   v1; a `title` param is a trivial Polish follow-up). No new picker IPC.

All defaults are non-destructive (clone writes only into a new/empty dest; init only creates a repo;
neither touches any existing tab's repo). No new `AppError` variant; the only new IPC push surface is
the clone progress **Channel** (no new event).

---

## 1. Overview & invariants held

- **Rust owns all Git logic.** `clone.rs` wraps `RepoBuilder`/`Repository::init`; React only fires the
  two commands, renders progress, and calls the existing `openTab`.
- **IPC carries compact precomputed data.** `CloneProgress` is five scalar counters; the commands
  return a single absolute path `String`. No raw libgit2 objects, no per-object round-trips — progress
  is streamed as small structs over a channel, not polled.
- **Command / Channel split.** `clone_repo` / `init_repo` are request/response **commands**; clone
  progress streams over a **`Channel<CloneProgress>`** (the streaming-data role). No new **event**.
- **git2 is blocking → `spawn_blocking`.** Both commands run their blocking core under
  `spawn_blocking`; the channel handle is moved in and `.send()` from the blocking thread (§2.3).
- **Runtime-free core.** `clone.rs` takes `&str`/`&Path` + a plain `FnMut(CloneProgress)` progress
  sink — **no `tauri::` types** → unit/CLI-testable without the Tauri "test" feature (stash/remote
  rule).
- **Credential reuse.** Clone's fetch reuses the M6 chain (`acquire_cred`) verbatim; never prompts,
  never stores passwords.
- **Not repo-scoped.** Neither command takes `state`/`repoId` — they CREATE a repo. Registration
  happens exactly once, later, when the frontend calls `openTab(path)` (decision §OPEN-2).
- **Mock-implementable.** Both commands + the progress channel have `mock.ts` implementations
  (progress ticks fired via the same callback the real channel drives), so `VITE_MOCK_IPC=1` runs the
  whole feature — including the progress bar and the resulting new tab — in a plain browser.

---

## 2. New Rust module `crates/bonsai-core/src/git/clone.rs`

Register in `crates/bonsai-core/src/git/mod.rs`: add `pub mod clone;` **after** `pub mod cherrypick;`
and **before** `pub mod commit;` (alphabetical: `cherrypick` < `clone` < `commit`).

### 2.1 Wire type

```rust
/// Streamed clone transfer progress (one per git2 `transfer_progress` tick).
/// Wire: camelCase. Sent over a Tauri `Channel<CloneProgress>` (contract §OPEN-1);
/// carries NO libgit2 handles — five scalar counters only.
#[derive(Debug, Clone, Copy, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CloneProgress {
    /// git2 `Progress::received_objects()` — objects downloaded so far.
    pub received_objects: u32,
    /// git2 `Progress::total_objects()` — total to download (0 until known).
    pub total_objects: u32,
    /// git2 `Progress::indexed_deltas()` — deltas resolved so far.
    pub indexed_deltas: u32,
    /// git2 `Progress::total_deltas()` — total deltas to resolve.
    pub total_deltas: u32,
    /// git2 `Progress::received_bytes()` — bytes over the wire so far.
    pub received_bytes: u64,
}
```

Progress-phase note (documented so the UI bar is deterministic): git2 fills the **receiving objects**
phase first (`received_objects`→`total_objects`, `received_bytes` climbing, deltas 0), THEN the
**resolving deltas** phase (`indexed_deltas`→`total_deltas`). The UI treats the fraction as
`received_objects/total_objects` while `total_deltas == 0`, else `indexed_deltas/total_deltas` (§4.2).

### 2.2 Function signatures

```rust
/// Blocking. Clones `url` into `dest` using git2 `RepoBuilder` + `FetchOptions`
/// with the SHARED M6 credential callbacks (helper → SSH agent → default) and a
/// `transfer_progress` callback that calls `on_progress` once per tick. Returns
/// the ABSOLUTE workdir path of the cloned repo (feed to `openTab`).
///
/// `on_progress` is a plain sink so the command layer can bridge it to a Tauri
/// `Channel` WITHOUT pulling `tauri::` into core (contract §OPEN-1, §2.3).
/// Errors: dest exists & non-empty (`Io`, §OPEN-4); auth exhausted (`AuthFailed`
/// via `map_remote_err`); transport/invalid-URL (`NetworkError`/`Git` via
/// `map_remote_err`).
pub fn clone_repo(
    url: &str,
    dest: &Path,
    on_progress: impl FnMut(CloneProgress) + Send,
) -> Result<String, AppError>;

/// Blocking. Initializes a NON-bare repository at `path` (`Repository::init`),
/// creating the directory if needed. If `path` is ALREADY a repo, opens it
/// instead of reinitializing (idempotent, §OPEN-3). Returns the ABSOLUTE
/// workdir path (feed to `openTab`). Errors: `path` exists as a FILE (`Io`).
pub fn init_repo(path: &Path) -> Result<String, AppError>;
```

### 2.3 `clone_repo` internals + threading

```rust
use std::cell::RefCell;

// 1. dest pre-check (§OPEN-4): if dest exists AND is a non-empty dir → Io error.
if dest.exists() {
    if dest.is_file() {
        return Err(AppError::Io(format!("destination is a file: {}", dest.display())));
    }
    let mut entries = std::fs::read_dir(dest)?;   // ? -> AppError::Io via From
    if entries.next().is_some() {
        return Err(AppError::Io(format!(
            "destination directory is not empty: {}", dest.display()
        )));
    }
}

// 2. Credentials: no repo yet -> default (global+system) config (§OPEN-5).
let config = git2::Config::open_default()?;
let attempts = RefCell::new(crate::git::remote::CredAttempts::default());
let on_progress = RefCell::new(on_progress);   // shared mutable sink for the FnMut callback

// 3. Callbacks mirror remote.rs::fetch_remote, ADDING transfer_progress.
let mut callbacks = git2::RemoteCallbacks::new();
callbacks.credentials(|url, username_from_url, allowed| {
    crate::git::remote::acquire_cred(&config, &attempts, url, username_from_url, allowed)
});
callbacks.transfer_progress(|stats: git2::Progress| {
    (on_progress.borrow_mut())(CloneProgress {
        received_objects: to_u32(stats.received_objects()),
        total_objects:    to_u32(stats.total_objects()),
        indexed_deltas:   to_u32(stats.indexed_deltas()),
        total_deltas:     to_u32(stats.total_deltas()),
        received_bytes:   stats.received_bytes() as u64,
    });
    true    // never cancel in v1 (§OPEN-1)
});

let mut fo = git2::FetchOptions::new();
fo.remote_callbacks(callbacks);

let mut builder = git2::build::RepoBuilder::new();
builder.fetch_options(fo);

// 4. Clone. Map transfer/auth errors via the shared M6 mapper (context = url).
let repo = builder.clone(url, dest).map_err(|e| crate::git::remote::map_remote_err(e, url))?;

// 5. Return the absolute workdir path (non-bare clone always has one).
let workdir = repo.workdir()
    .ok_or_else(|| AppError::Git("cloned repository has no working directory".to_string()))?;
Ok(workdir.to_string_lossy().into_owned())
```

`to_u32(n: usize) -> u32` is the saturating helper from `remote.rs:201` — **duplicate it privately in
clone.rs** (it is `fn`, not `pub(crate)`) OR promote it to `pub(crate)` in remote.rs and reuse;
**recommend duplicate** (one trivial line, avoids touching remote.rs).

**Threading (documented):** the whole body runs inside `spawn_blocking` on ONE blomcking thread;
libgit2 invokes `credentials`/`transfer_progress` synchronously on that SAME thread, so `RefCell`
(not `Mutex`) is correct and there is no cross-thread aliasing. The command layer passes an
`on_progress` closure that captures a **`Channel<CloneProgress>`** (which is `Clone + Send + Sync +
'static`) and calls `let _ = channel.send(p);` — send failures (frontend dropped the channel) are
ignored, the clone still completes.

### 2.4 `init_repo` internals

```rust
// path exists as a file -> Io error.
if path.is_file() {
    return Err(AppError::Io(format!("path is a file, not a directory: {}", path.display())));
}
// Already a repo? Open it (idempotent, §OPEN-3) rather than reinitializing.
let repo = match git2::Repository::open_ext(
    path, git2::RepositoryOpenFlags::NO_SEARCH, std::iter::empty::<&std::ffi::OsStr>(),
) {
    Ok(r) => r,
    Err(e) if e.code() == git2::ErrorCode::NotFound => git2::Repository::init(path)?, // non-bare
    Err(e) => return Err(e.into()),
};
let workdir = repo.workdir()
    .ok_or_else(|| AppError::Git("initialized repository has no working directory".to_string()))?;
Ok(workdir.to_string_lossy().into_owned())
```

`Repository::init` creates a non-bare repo with an **unborn HEAD** — exactly the empty-state
`read_repo_info` already reports (`repo.rs:76-88`, unborn branch) and M0/Polish already render.

### 2.5 Error mapping (→ `AppError`, no new variant)

| Situation | AppError |
|---|---|
| clone dest is a file / non-empty dir | `Io` (§OPEN-4) |
| clone auth exhausted / auth code | `AuthFailed` (via `map_remote_err`) |
| clone transport (Net/Http/Ssh) / bad URL | `NetworkError` (via `map_remote_err`) |
| any other clone libgit2 error | `Git` (via `map_remote_err`) |
| init path is a file | `Io` |
| any other init libgit2 error | `Git` |

---

## 3. Commands (`src-tauri/src/commands.rs`) + registration

Add to the import block (`commands.rs:1-38`):
`use bonsai_core::git::clone::{clone_repo as clone_repo_core, init_repo as init_repo_core, CloneProgress};`

Neither command takes `state` (§OPEN-2). Both wrap the blocking core in `spawn_blocking`; join error →
`AppError::Other(format!("task join error: {e}"))` verbatim.

```rust
/// Clones `url` into `dest`, streaming `CloneProgress` over `on_progress`.
/// Returns the absolute workdir path of the clone (frontend then calls
/// `open_repo`/openTab). Rejects io | authFailed | networkError | git.
#[tauri::command]
pub async fn clone_repo(
    url: String,
    dest: String,
    on_progress: tauri::ipc::Channel<CloneProgress>,
) -> Result<String, AppError> {
    tauri::async_runtime::spawn_blocking(move || {
        clone_repo_core(&url, std::path::Path::new(&dest), move |p| {
            let _ = on_progress.send(p);   // Channel is Send+Sync+'static; ignore drop
        })
    })
    .await
    .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Initializes (or opens, if already a repo) a repository at `path`. Returns
/// the absolute workdir path. Rejects io | git.
#[tauri::command]
pub async fn init_repo(path: String) -> Result<String, AppError> {
    tauri::async_runtime::spawn_blocking(move || init_repo_core(std::path::Path::new(&path)))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}
```

Register both in `src-tauri/src/lib.rs` `generate_handler!` (`lib.rs:15-85`), appended after
`commands::sync_submodule` (add a trailing comma to that line):

```rust
        commands::sync_submodule,
        commands::clone_repo,
        commands::init_repo
```

Command surface after P21: existing list + `clone_repo`, `init_repo`. Events: unchanged. Channels:
**NEW** — `clone_repo`'s `on_progress: Channel<CloneProgress>` (the first channel in the app).

No `#[cfg(test)]` "requires open repo" test applies (these are not repo-scoped); the core is covered
by §5. A tiny command-level smoke (dest-non-empty → `Io`) may live in `clone.rs` tests instead.

---

## 4. IPC layer (TypeScript)

### 4.1 `src/ipc/types.ts`

```ts
export interface CloneProgress {
  receivedObjects: number;
  totalObjects: number;
  indexedDeltas: number;
  totalDeltas: number;
  receivedBytes: number;   // u64 on the wire; safe as JS number for realistic repos
}
```

`IpcApi` additions (near `openRepo`, `types.ts` open-flow region; mirror the JSDoc style):

```ts
/** Clone `url` into `dest`, streaming progress via `onProgress`. Resolves to the
 *  absolute workdir path of the clone (caller then opens it as a tab). The frontend
 *  passes a plain callback; the Tauri impl bridges it through a `Channel`, the mock
 *  invokes it directly. Rejects io | authFailed | networkError | git. */
cloneRepo(url: string, dest: string, onProgress: (p: CloneProgress) => void): Promise<string>;

/** Initialize (or open, if already a repo) a repository at `path`. Resolves to the
 *  absolute workdir path. Rejects io | git. */
initRepo(path: string): Promise<string>;
```

Add `CloneProgress` to the `index.ts` re-export list (§4.4).

### 4.2 `src/ipc/tauri.ts`

Extend the core import to include `Channel`:
`import { invoke, Channel } from '@tauri-apps/api/core';`
Add `CloneProgress` to the `import type { … } from './types'` block.

```ts
cloneRepo(url: string, dest: string, onProgress: (p: CloneProgress) => void): Promise<string> {
  const channel = new Channel<CloneProgress>();
  channel.onmessage = onProgress;
  // Tauri auto-serializes the Channel as the `on_progress` argument.
  return invoke<string>('clone_repo', { url, dest, onProgress: channel });
},

initRepo(path: string): Promise<string> {
  return invoke<string>('init_repo', { path });
},
```

`pickFolder` (`tauri.ts:58`) is reused unchanged for both destination and init pickers (§OPEN-6).

### 4.3 `src/ipc/mock.ts` — stateful, honest

Import `CloneProgress` (with the other types). No new module state is required (both reuse
`openRepo`/`createRepoState`). Both live near `openRepo`/`pickFolder` (`mock.ts:932-979`).

```ts
async cloneRepo(url: string, dest: string, onProgress: (p: CloneProgress) => void): Promise<string> {
  // Derive a repo folder name from the URL (strip trailing .git), like real clone.
  const name = (url.split(/[\\/]/).pop() ?? 'repo').replace(/\.git$/i, '') || 'repo';
  // Simulate a few monotonic progress ticks: an object-download phase then a
  // delta-resolve phase (§2.1 phase note), so the harness bar animates end-to-end.
  const total = 20;
  for (let i = 1; i <= total; i++) {
    await delay(120);
    onProgress({
      receivedObjects: i, totalObjects: total,
      indexedDeltas: 0, totalDeltas: 0,
      receivedBytes: i * 4096,
    });
  }
  for (let i = 1; i <= 10; i++) {
    await delay(80);
    onProgress({
      receivedObjects: total, totalObjects: total,
      indexedDeltas: i, totalDeltas: 10,
      receivedBytes: total * 4096,
    });
  }
  // Return a path the EXISTING openRepo can seed as a normal default repo.
  // Avoid the reserved substrings ('error'|'not-a-repo'|'bare'|'unborn').
  return `${dest}/${name}`;
},

async initRepo(path: string): Promise<string> {
  await delay(150);
  // Return a path containing 'unborn' so createRepoState seeds an EMPTY (unborn)
  // repo — honest: init makes a brand-new repo with no commits (mock.ts:280-315).
  return `${path}/new-unborn-repo`;
},
```

Failure triggers (compose with the existing `?remote=` mock param, `mock.ts:1434`): when the clone
`url` contains `authfail` / `network`, throw the SAME `authFailed` / `networkError` `AppError`s M6's
`fetch`/`pull` mocks throw (reuse those message strings verbatim), AFTER a couple of progress ticks,
so the harness exercises the in-dialog error path.

### 4.4 `src/ipc/index.ts`

Add `CloneProgress` to the `export type { … } from './types'` list.

---

## 5. Testing (AI gate) — `crates/bonsai-core/tests/lifecycle_cli.rs`

CLI-oracle suite mirroring `tests/remote_cli.rs` (LOCAL transport only — `file://` or a local path;
NO network, autonomous; `require_git!` skip when `git` is absent). **Env (tester):** `TMP`/`TEMP` →
`D:\Temp`; scratch under `D:\Temp\bonsai-scratch` via `common::scratch_dir()`; forward slashes in
Bash-tool paths; run `cargo test` and `clippy` **sequentially** (never concurrent — target-dir race).

### 5.1 Fixture (built with `git`/git2, like remote_cli)

1. Bare source origin: `git init --bare origin.git`; a seed clone commits **A** then **B** on `main`
   (two files/commits) and pushes → the origin has real history + refs.
2. The clone source URL is `file:///<origin.git>` (or the plain local path — the local transport needs
   NO credentials; the credential callback is never invoked, same honest-coverage note as remote_cli).

### 5.2 Assertions

1. **Clone round-trip.** `clone_repo(file://origin.git, <scratch>/work, sink)` → returns an absolute
   path that `read_repo_info` reports as `is_repo && !bare` with a **non-unborn** HEAD. Cross-check:
   `git -C work rev-parse HEAD` == origin's `main` tip (**B**); `git -C work rev-parse HEAD` ==
   `git -C origin.git rev-parse main`; the tree matches (`git -C work status --porcelain` empty).
2. **Refs/remote wired.** `work` has `refs/remotes/origin/main` and `origin` remote configured
   (`git -C work remote get-url origin` == the source URL) — proves it is a real clone, not a copy.
3. **Progress callback fires monotonically.** Collect every `CloneProgress` into a `Vec` via the
   sink; assert it is **non-empty**, `received_objects` is **non-decreasing**, and the final tick has
   `received_objects == total_objects` (objects fully received). (`total_objects > 0` for a non-empty
   origin.)
4. **Dest errors.** dest is an existing **non-empty** dir → `Err(Io)`, nothing written into it; dest
   is an existing **file** → `Err(Io)`. An existing **empty** dir clones fine.
5. **Init produces a valid, unborn repo.** `init_repo(<scratch>/fresh)` → returns an absolute path;
   `read_repo_info` reports `is_repo && !bare` and `head.unborn == true`, `head.oid == ""`; cross-check
   `git -C fresh rev-parse --is-inside-work-tree` == `true` and `git -C fresh status` shows "No commits
   yet". A subsequent stage+commit succeeds (repo is usable).
6. **Init reuse (§OPEN-3).** `init_repo` on a path that is ALREADY a repo returns that repo's workdir
   and does **not** disturb its HEAD/refs (rev-parse HEAD unchanged before/after).
7. **Wire shape** (unit test in `clone.rs` `#[cfg(test)]`): `serde_json` asserts `CloneProgress` →
   camelCase keys `{receivedObjects,totalObjects,indexedDeltas,totalDeltas,receivedBytes}`.

Coverage-split honesty (state in gate evidence): the local transport **never invokes the credential
callback**, so clone-over-auth is covered ONLY by the USER CHECKPOINT (§6). The shared `acquire_cred`
guard + `map_remote_err` are already unit-tested by M6 (`remote.rs` tests) — clone reuses them
unchanged, so no re-test is required.

### 5.3 Browser-harness (orchestrator-verifiable)

- `pnpm build` + `tsc` clean; no `@tauri-apps/*` module executed in mock mode; no console errors.
- The tab `+` menu shows **Clone repository…** and **New repository…** below Browse…; the no-repo
  empty state shows the same three affordances.
- **Clone:** open CloneDialog, type a URL, pick a destination (mock pickFolder), Clone → the progress
  bar animates through both phases (object → delta) → dialog closes → a NEW tab opens showing the
  seeded default repo (graph + status). Screenshot the mid-clone progress bar.
- Clone with a URL containing `authfail` → the dialog shows the `authFailed` message inline and adds
  no tab; `network` → the `networkError` message.
- **Init:** New repository… → folder picker → a NEW tab opens showing the empty/unborn state (empty
  graph + first-commit-ready status panel).

### 5.4 USER CHECKPOINT (native `pnpm tauri dev`)

- **Clone a REAL network remote** (HTTPS via Git Credential Manager, or SSH via agent — the M6
  credential path): progress bar advances, the clone completes, the repo opens in a new tab with its
  real history; a bad URL yields the `networkError` message (not a hang/crash); a private repo with no
  configured credentials yields the `authFailed` message (no password prompt).
- **Init** a brand-new empty folder: a new tab opens in the empty/unborn state; a first stage+commit
  works end-to-end.

---

## 6. Frontend

### 6.1 New component `src/components/CloneDialog.tsx`

Modeled on `PromptDialog`/`ConfirmDialog` (same `.dialog-overlay`/`.dialog-card`/`.dialog-*` shell,
focus/Esc-capture/overlay-click discipline). Props:

```ts
export interface CloneDialogProps {
  open: boolean;
  busy: boolean;                       // true while a clone is in flight
  progress: CloneProgress | null;      // latest tick (null before the first)
  error: string | null;               // inline error (authFailed/networkError/io/git)
  onPickDest(): void;                  // App wires to ipc.pickFolder()
  dest: string | null;                 // chosen destination (App-owned)
  onSubmit(url: string): void;         // App runs the clone
  onCancel(): void;
}
```

Body: a URL text input (label "Repository URL"); a **destination row** — a read-only display of `dest`
plus a "Choose…" button calling `onPickDest`; a **progress region** shown while `busy`: a determinate
`<progress>`/bar whose fraction is `totalDeltas > 0 ? indexedDeltas/totalDeltas :
totalObjects > 0 ? receivedObjects/totalObjects : 0`, with a phase caption ("Receiving objects…" /
"Resolving deltas…") and a received-bytes readout; an inline `.dialog-error` when `error !== null`.
Confirm ("Clone") is disabled unless a non-empty URL AND a `dest` are set and `!busy`; Cancel is always
enabled (closes the dialog — the clone continues on the backend, §OPEN-1). Enter submits (create
action, `.btn-primary`), matching PromptDialog.

### 6.2 App wiring (`src/App.tsx`)

State (near the existing dialog/loading state):

```ts
const [cloneOpen, setCloneOpen] = useState(false);
const [cloneDest, setCloneDest] = useState<string | null>(null);
const [cloneProgress, setCloneProgress] = useState<CloneProgress | null>(null);
const [cloneBusy, setCloneBusy] = useState(false);
const [cloneError, setCloneError] = useState<string | null>(null);
```

Handlers (mirror `handleOpenRepository`, `App.tsx:408-419`):

```ts
const handleCloneOpen = useCallback(() => {
  setCloneDest(null); setCloneProgress(null); setCloneError(null); setCloneBusy(false);
  setCloneOpen(true);
}, []);

const handleClonePickDest = useCallback(async () => {
  const path = await ipc.pickFolder();
  if (path !== null) setCloneDest(path);
}, []);

const handleCloneSubmit = useCallback(async (url: string) => {
  if (cloneDest === null) return;
  setCloneBusy(true); setCloneError(null); setCloneProgress(null);
  try {
    const path = await ipc.cloneRepo(url, cloneDest, (p) => setCloneProgress(p));
    setCloneOpen(false);
    await openTab(path);                         // EXISTING tab flow (§OPEN-2)
  } catch (e) {
    setCloneError(errorMessage(e));              // stays open so the user can retry
  } finally {
    setCloneBusy(false);
  }
}, [cloneDest, openTab]);

// New repository: folder picker → init → openTab (no dialog needed).
const handleInitRepository = useCallback(async () => {
  setError(null); setLoading(true);
  try {
    const path = await ipc.pickFolder();
    if (path === null) return;
    const repoPath = await ipc.initRepo(path);
    await openTab(repoPath);
  } catch (e) {
    const msg = errorMessage(e);
    if (tabsRef.current.length > 0) pushToast('error', msg); else setError(msg);
  } finally {
    setLoading(false);
  }
}, [openTab, pushToast]);
```

Render `<CloneDialog open={cloneOpen} busy={cloneBusy} progress={cloneProgress} error={cloneError}
dest={cloneDest} onPickDest={() => void handleClonePickDest()} onSubmit={(u) => void
handleCloneSubmit(u)} onCancel={() => setCloneOpen(false)} />` alongside the other App-level dialogs.

### 6.3 Affordances — tab `+` menu + empty state

- **`TabStrip.tsx`**: add two props `onClone(): void` and `onInit(): void`; render two items in the
  `+` menu below the `Browse…` item (`TabStrip.tsx:143-152`), styled as `repo-switcher-item`
  ("Clone repository…", "New repository…"), each `close()`-ing the menu then calling the handler.
  `App.tsx:588-593` passes `onClone={handleCloneOpen}` and `onInit={() => void handleInitRepository()}`.
- **No-repo empty state** (`App.tsx:660-690`, where `onBrowse`/recents live): add "Clone repository…"
  and "New repository…" buttons beside the existing Browse/open affordance, wired to the same two
  handlers.

---

## 7. Sub-increments (each a single fresh-context senior-dev pass)

### P21a — Rust: `clone.rs` + commands + registration + tests
- New `crates/bonsai-core/src/git/clone.rs` (§2): `CloneProgress`, `clone_repo`, `init_repo` with the
  exact git2 calls, credential reuse (`crate::git::remote::*`, no change to remote.rs), the private
  `to_u32` helper. `git/mod.rs` `pub mod clone;`.
- `commands.rs`: `clone_repo` + `init_repo` `#[tauri::command]`s (§3), `Channel<CloneProgress>` arg;
  `lib.rs` registration.
- Unit test in `clone.rs` `#[cfg(test)]`: `CloneProgress` wire shape (§5.2 #7).
- `crates/bonsai-core/tests/lifecycle_cli.rs`: §5.1 fixture + §5.2 assertions #1–#6, `require_git!`.
- **Acceptance**: `cargo check`/`clippy -D warnings` clean; unit + CLI tests pass (or skip cleanly
  without `git`); no frontend change needed to compile.

### P21b — IPC triple + CloneDialog + affordances
- `types.ts`: `CloneProgress` + `cloneRepo`/`initRepo` on `IpcApi`; `index.ts` re-export.
- `tauri.ts`: `Channel` import + `cloneRepo` (channel wiring) + `initRepo` wrappers.
- `mock.ts`: `cloneRepo` (progress ticks + derived path + `authfail`/`network` triggers) + `initRepo`
  (unborn-path) (§4.3).
- `CloneDialog.tsx` (new, §6.1) + `.css` (reuse `.dialog-*`; add a progress-bar class if needed).
- `App.tsx`: clone/init state + handlers, `<CloneDialog>` render, thread `onClone`/`onInit` to
  `TabStrip` + the empty state (§6.2–§6.3). `TabStrip.tsx`: two new props + menu items.
- **Acceptance**: `pnpm build`/`tsc` clean; harness §5.3 (progress bar animates, new tab opens for both
  clone and init, error path shows inline).

---

## 8. File touch list

- `crates/bonsai-core/src/git/clone.rs` (**new**), `crates/bonsai-core/src/git/mod.rs`
  (`pub mod clone;`).
- `crates/bonsai-core/src/git/remote.rs` — **NOT touched** (credential chain already `pub(crate)`).
- `src-tauri/src/commands.rs` (import + 2 commands), `src-tauri/src/lib.rs` (register 2).
- `crates/bonsai-core/tests/lifecycle_cli.rs` (**new**).
- `src/ipc/types.ts` (`CloneProgress` + 2 `IpcApi` methods), `src/ipc/tauri.ts` (`Channel` import + 2
  wrappers), `src/ipc/mock.ts` (2 methods), `src/ipc/index.ts` (re-export `CloneProgress`).
- `src/components/CloneDialog.tsx` (**new**), `src/components/TabStrip.tsx` (2 props + 2 menu items),
  `src/App.tsx` (state + handlers + `<CloneDialog>` + empty-state buttons + TabStrip wiring).
- `src/styles.css` — progress-bar / dialog tweaks (reuse `.dialog-*`; add `.clone-progress` if needed).
- No new `AppError` variant; no new **event**; the only new push surface is the clone **Channel**.
```