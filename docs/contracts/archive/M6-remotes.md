# M6 — Remotes (fetch / pull / push): Implementation Contract

Status: authoritative for M6. Implementer: senior-dev. Builds on `M0-scaffold.md` (error shape,
IPC conventions), `M3-commit.md` (mutation command pattern, imperative refetch, stateful mock),
`M5-branches.md` (BranchesSnapshot upstream + ahead/behind, `_inner` command pattern, sidebar
badges). Credential strategy USER-CONFIRMED 2026-07-28 (locked): credential-helper → SSH agent →
default; never prompt or store passwords in-app.

Scope (locked):
- **fetch** — fetch **all configured remotes** (decision §9).
- **pull** — fetch the current branch's upstream remote, then **fast-forward ONLY**. Not
  fast-forwardable → report it and change **nothing**. No merge, no rebase, no conflicts in v1.
- **push** — push the current branch to its upstream; **no upstream → push to
  `origin/<branch>` and set upstream** (decision §9). **NO force push** — non-FF rejection
  surfaces the remote's error clearly.
- After each op the frontend refetches imperatively (branches + graph, plus status/header where
  the tip can move). Busy treatment: indeterminate — reuse the global `mutating` flag; **no
  progress channel in v1** (decision §9).

---

## 1. New / changed files

```
src-tauri/
  src/error.rs                 # + NoRemote, NoUpstream, AuthFailed, NetworkError,
                               #   PushRejected variants
  src/git/remote.rs            # NEW: fetch_all / pull_ff / push_current + cred guard +
                               #   error mapping + tests   (git/mod.rs: pub mod remote;)
  src/commands.rs              # + fetch, pull, push (thin _inner wrappers)
  src/lib.rs                   # register the three commands
src/
  ipc/types.ts                 # + FetchResult, PullResult, PushResult; IpcApi +3;
                               #   AppError kind union +5
  ipc/tauri.ts                 # + 3 wrappers
  ipc/mock.ts                  # stateful mock fetch/pull/push + ?remote= failure triggers
  ipc/fixtures/branches.ts     # feature/sidebar keeps ahead:2 behind:1 (used by push/pull mock)
  App.tsx                      # toolbar buttons, handlers, remoteNotice/remoteError state
  styles.css                   # .toolbar-btn, .remote-notice, notice-warning variant
```

No capability changes (network access happens in Rust, not the webview).

## 2. Rust backend — `src-tauri/src/git/remote.rs`

All functions blocking (called under `spawn_blocking`); repos opened with `NO_SEARCH` like every
git/ module.

### 2.1 Wire types

```rust
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteFetchResult {
    /// Remote name, e.g. "origin".
    pub remote: String,
    /// stats().received_objects() after the fetch.
    pub received_objects: u32,
    /// Number of update_tips callback invocations where old != new
    /// (includes newly created remote-tracking refs).
    pub updated_refs: u32,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FetchResult {
    /// One entry per configured remote, in remote-list order.
    pub remotes: Vec<RemoteFetchResult>,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum PullResult {
    /// behind == 0 (local is equal to or ahead of upstream): nothing to pull.
    UpToDate,
    /// Branch ref + worktree moved from `from` to `to` (full 40-char oids).
    FastForwarded { branch: String, from: String, to: String },
    /// ahead > 0 && behind > 0. NOTHING was changed (fetch already happened —
    /// remote-tracking refs updated — but branch/worktree untouched).
    WouldNotFastForward { branch: String, ahead: u32, behind: u32 },
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum PushResult {
    /// Remote-tracking ref already equalled the local tip before the push.
    UpToDate { remote: String, branch: String },
    /// `set_upstream` true when the branch had no upstream and we configured
    /// `origin/<branch>` as part of this push (§2.6).
    Pushed { remote: String, branch: String, set_upstream: bool },
}
```

Note: `rename_all_fields` needs serde ≥ 1.0.186 (already satisfied). On the wire these are
`{ "kind": "upToDate" } | { "kind": "fastForwarded", "branch": ..., "from": ..., "to": ... }` etc.

**Decision — `WouldNotFastForward` is a RESULT, not an error** (nothing failed; the user gets an
informational warning). **Push rejection IS an error** (`pushRejected`) — the op failed and the
error banner is the established failure surface (§9).

### 2.2 Credentials callback (shared by fetch/pull/push)

The libgit2 credentials callback is **re-invoked after every auth failure** — without a guard it
loops forever. Split decision from construction so the guard is unit-testable offline:

```rust
/// Which credential source to try next. Pure decision logic — no git2 Cred construction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CredMethod { Helper, SshAgent, Default }

/// One-shot attempt flags. A fresh CredAttempts per remote operation.
#[derive(Debug, Default)]
pub(crate) struct CredAttempts { helper: bool, agent: bool, default_: bool }

/// Returns the next untried method compatible with `allowed`, marking it tried.
/// Order: Helper (USER_PASS_PLAINTEXT) -> SshAgent (SSH_KEY) -> Default (DEFAULT).
/// None => every compatible method has been tried (or none is compatible).
pub(crate) fn next_cred_method(
    attempts: &mut CredAttempts,
    allowed: git2::CredentialType,
) -> Option<CredMethod>;
```

Pseudocode (exactly this — the guard is the point):

```
next_cred_method(attempts, allowed):
    if allowed.USER_PASS_PLAINTEXT and not attempts.helper:
        attempts.helper = true;  return Some(Helper)
    if allowed.SSH_KEY and not attempts.agent:
        attempts.agent = true;   return Some(SshAgent)
    if allowed.DEFAULT and not attempts.default_:
        attempts.default_ = true; return Some(Default)
    return None
```

Callback wiring (in a helper `fn make_callbacks<'a>(attempts: &'a mut CredAttempts, ...) ->
git2::RemoteCallbacks<'a>` or an inline closure capturing `RefCell<CredAttempts>`):

```
credentials(url, username_from_url, allowed):
    loop over next_cred_method(attempts, allowed):
        Some(Helper)   -> match Cred::credential_helper(&repo_config, url, username_from_url)
                              { Ok(c) => return Ok(c), Err(_) => continue }
        Some(SshAgent) -> match Cred::ssh_key_from_agent(username_from_url.unwrap_or("git"))
                              { Ok(c) => return Ok(c), Err(_) => continue }
        Some(Default)  -> match Cred::default()
                              { Ok(c) => return Ok(c), Err(_) => continue }
        None -> return Err(git2::Error::new(
                    ErrorCode::Auth, ErrorClass::Callback, CRED_EXHAUSTED_MSG))

const CRED_EXHAUSTED_MSG: &str = "bonsai: no usable credentials";
```

> **Superseded 2026-08-04** — the `Some(Helper)` arm above (`Cred::credential_helper`) is replaced
> by a call to `credential_fill` per the addendum at the bottom of this file. The guard
> (`next_cred_method`/`CredMethod`/`CredAttempts`) is UNCHANGED.

Key properties: each method is attempted **at most once per operation** (construction failure
consumes the attempt — `continue` re-enters the loop, it never resets flags); when everything is
exhausted the callback returns an error, which aborts the transport instead of looping. NEVER
prompt; NEVER read/store passwords.

### 2.3 Error mapping — `map_remote_err`

```rust
/// Maps a git2 error from a remote operation to an AppError. `context` is the
/// remote name or URL for message interpolation.
pub(crate) fn map_remote_err(e: git2::Error, context: &str) -> AppError;
```

Mapping table (evaluate top-down, first match wins):

| git2 condition                                              | AppError kind   | user-facing message |
|--------------------------------------------------------------|-----------------|---------------------|
| `class() == Callback` && message contains `CRED_EXHAUSTED_MSG` | `authFailed`  | `authentication failed for '<context>': no usable credentials. Configure a Git credential helper (e.g. Git Credential Manager) for HTTPS remotes, or run an SSH agent for SSH remotes.` |
| `code() == Auth` (any class: Http, Ssh, Net, Callback)       | `authFailed`    | same message as above |
| `code() == NotFastForward` (push negotiation)                 | `pushRejected`  | `push rejected: the remote contains commits you do not have. Fetch/pull first — Bonsai v1 never force-pushes.` |
| `class() == Net`                                              | `networkError`  | `network error talking to '<context>': <e.message()>` |
| `class() == Http`                                             | `networkError`  | `network error talking to '<context>': <e.message()>` |
| `class() == Ssh`                                              | `networkError`  | `network error talking to '<context>': <e.message()>` |
| anything else                                                 | `git`           | `e.message()` (existing `From<git2::Error>` behavior) |

Additionally (not via `map_remote_err`): `repo.find_remote(name)` → `ErrorCode::NotFound` maps to
`AppError::NoRemote(...)`, and a `push_update_reference` callback that receives
`status: Some(msg)` maps to `AppError::PushRejected(format!("push rejected by remote: {msg}. \
Bonsai v1 never force-pushes — fetch/pull first."))` (§2.6 step 6).

> **Optional refinement 2026-08-04** — see addendum §A.5 for a precisely-worded, opt-in variant
> of the `authFailed` message that distinguishes "no helper configured" from "helper configured
> but has no cached credentials for this remote."

### 2.4 `fetch_all`

```rust
/// Blocking. Fetches every configured remote, sequentially, in
/// repo.remotes() order. Default refspecs, AutotagOption::Auto, NO prune.
/// 1. names = repo.remotes()?; skip non-UTF-8 entries (eprintln).
/// 2. names.is_empty() -> AppError::NoRemote("no remotes configured")
/// 3. per remote: fresh CredAttempts; RemoteCallbacks {credentials: §2.2,
///    update_tips: |_, old, new| { if old != new { updated += 1 }; true }};
///    remote.fetch(&[] as &[&str], Some(&mut fetch_opts), None)
///      Err(e) -> return Err(map_remote_err(e, name))   // FAIL FAST, §9
///    received_objects = remote.stats().received_objects() as u32
/// 4. Ok(FetchResult { remotes })
/// Works fine with detached or unborn HEAD (no branch involved).
pub fn fetch_all(workdir: &Path) -> Result<FetchResult, AppError>;
```

### 2.5 `pull_ff`

```rust
/// Blocking. Fetch the current branch's upstream remote, then fast-forward ONLY.
/// 1. head = read_head_info(&repo):
///      unborn   -> AppError::Git("cannot pull: the repository has no commits yet")
///      detached -> AppError::Git("cannot pull: HEAD is detached")
/// 2. branch = repo.find_branch(&head.branch_name, Local)
///    upstream = branch.upstream()
///      Err NotFound -> AppError::NoUpstream(format!(
///          "cannot pull: branch '{name}' has no upstream configured"))
/// 3. remote_name = repo.branch_upstream_remote(&format!("refs/heads/{name}"))?
///    (utf8; else AppError::Git). Fetch ONLY that remote (same options/callbacks
///    as §2.4; fresh CredAttempts; errors via map_remote_err).
/// 4. RE-RESOLVE upstream after the fetch: upstream_oid = branch.upstream()?.get()
///    .target() (the fetch may have moved it); local_oid = branch tip.
/// 5. (ahead, behind) = repo.graph_ahead_behind(local_oid, upstream_oid)?
///      behind == 0                -> Ok(PullResult::UpToDate)       // incl. ahead>0
///      ahead > 0 && behind > 0    -> Ok(PullResult::WouldNotFastForward
///                                        { branch, ahead, behind })  // change NOTHING
/// 6. FAST-FORWARD (ahead == 0 && behind > 0), exact git2 sequence:
///    a. obj = repo.find_object(upstream_oid, None)
///    b. repo.checkout_tree(&obj, Some(CheckoutBuilder::new().safe()))
///       -- DEFAULT SAFE MODE. NEVER .force(). --
///       Err code Conflict -> AppError::CheckoutConflict(format!(
///           "cannot pull: local changes would be overwritten by the update. \
///            Commit or discard them first."))
///       (checkout_tree runs FIRST so a conflict leaves ref AND worktree untouched —
///        same ordering rationale as M5 §2.5.)
///    c. repo.find_reference(&format!("refs/heads/{name}"))?
///           .set_target(upstream_oid, &format!("pull: fast-forward to {upstream_oid}"))
///       (HEAD is symbolic to the branch; it follows automatically. Reflog gets the message.)
/// 7. Ok(PullResult::FastForwarded { branch: name, from: local_oid, to: upstream_oid })
pub fn pull_ff(workdir: &Path) -> Result<PullResult, AppError>;
```

Between 6b and 6c the worktree briefly leads the ref — acceptable inside one blocking call; no
IPC observes the intermediate state.

### 2.6 `push_current`

```rust
/// Blocking. Pushes the current branch. Never force (refspec has NO leading '+').
/// 1. head: unborn   -> AppError::Git("cannot push: the repository has no commits yet")
///          detached -> AppError::Git("cannot push: HEAD is detached")
/// 2. Resolve target:
///    - upstream configured (branch_upstream_remote + branch_upstream_name both Ok):
///        remote_name; remote_ref = branch_upstream_name (e.g. "refs/heads/main"
///        after stripping "refs/remotes/<remote>/" -> use branch.<n>.merge from
///        config, which already IS "refs/heads/<x>"); set_upstream_after = false
///    - no upstream: remote_name = "origin";
///        repo.find_remote("origin") Err NotFound -> AppError::NoRemote(
///          "cannot push: branch '<name>' has no upstream and no 'origin' remote exists")
///        remote_ref = "refs/heads/<name>"; set_upstream_after = true
/// 3. prev_remote_tip = repo.find_reference("refs/remotes/<remote_name>/<remote_branch>")
///        .ok().and_then(|r| r.target())   // None when no tracking ref yet
///    if prev_remote_tip == Some(local_tip)
///        -> Ok(PushResult::UpToDate { remote, branch })   // no network round-trip
/// 4. refspec = format!("refs/heads/{name}:{remote_ref}")   // NO '+' prefix
///    callbacks: credentials (§2.2, fresh CredAttempts) + push_update_reference:
///        |refname, status| { if let Some(msg) = status { rejected = Some(msg) }; Ok(()) }
///    remote.push(&[refspec], Some(&mut push_opts))
///        Err(e) -> map_remote_err(e, remote_name)   // incl. NotFastForward -> pushRejected
/// 5. if rejected == Some(msg) -> AppError::PushRejected(format!(
///        "push rejected by remote: {msg}. Bonsai v1 never force-pushes — fetch/pull first."))
/// 6. if set_upstream_after: branch.set_upstream(Some(&format!("{remote_name}/{name}")))?
///    (libgit2's push already updated refs/remotes/origin/<name> via the default
///     tracking refspec, so set_upstream finds the ref.)
/// 7. Ok(PushResult::Pushed { remote, branch, set_upstream: set_upstream_after })
pub fn push_current(workdir: &Path) -> Result<PushResult, AppError>;
```

### 2.7 Error variants (`error.rs`)

```rust
#[error("{0}")] NoRemote(String),      // kind() -> "noRemote"
#[error("{0}")] NoUpstream(String),    // kind() -> "noUpstream"
#[error("{0}")] AuthFailed(String),    // kind() -> "authFailed"
#[error("{0}")] NetworkError(String),  // kind() -> "networkError"
#[error("{0}")] PushRejected(String),  // kind() -> "pushRejected"
```

All five carry their full display message (extend `message()`'s `m` arm and the doc comment's
kind list). `checkoutConflict` is REUSED for the dirty-pull case (§2.5 6b) — the UI never
branches on where it came from.

### 2.8 Commands (`commands.rs`) + registration

Exact M3/M5 pattern: `_inner` core using `current_repo_path(state)`, then `spawn_blocking`
(mandatory here — these block on the NETWORK, not just the odb), join error → `AppError::Other`.
None emit `repo-changed` (frontend refetches imperatively; the watcher also fires —
`refs/remotes/**` and `packed-refs` pass `watcher.rs::is_relevant` — and is absorbed by the
request-id guards).

```rust
#[tauri::command]
pub async fn fetch(state: tauri::State<'_, AppState>) -> Result<FetchResult, AppError>;
#[tauri::command]
pub async fn pull(state: tauri::State<'_, AppState>) -> Result<PullResult, AppError>;
#[tauri::command]
pub async fn push(state: tauri::State<'_, AppState>) -> Result<PushResult, AppError>;
```

All three are **argument-less, current-repo / current-branch semantics** (decision §9).

Command surface after M6: M5 list + `fetch`, `pull`, `push`. Events: `repo-changed` (unchanged).
Channels: none (progress channel deliberately deferred — §9).

## 3. IPC layer (TypeScript)

`src/ipc/types.ts`:

```ts
export interface RemoteFetchResult {
  remote: string;
  receivedObjects: number;
  updatedRefs: number;
}

export interface FetchResult {
  remotes: RemoteFetchResult[];
}

export type PullResult =
  | { kind: 'upToDate' }
  | { kind: 'fastForwarded'; branch: string; from: string; to: string }
  | { kind: 'wouldNotFastForward'; branch: string; ahead: number; behind: number };

export type PushResult =
  | { kind: 'upToDate'; remote: string; branch: string }
  | { kind: 'pushed'; remote: string; branch: string; setUpstream: boolean };

// AppError kind union: append
//   | 'noRemote' | 'noUpstream' | 'authFailed' | 'networkError' | 'pushRejected'

export interface IpcApi {
  // ...existing members unchanged...
  /** Fetch ALL remotes. Rejects noRemote | authFailed | networkError | git | noRepo. */
  fetch(): Promise<FetchResult>;
  /** Fetch upstream remote + fast-forward only. Rejects noUpstream | authFailed
   *  | networkError | checkoutConflict | git | noRepo. */
  pull(): Promise<PullResult>;
  /** Push current branch (sets upstream to origin/<branch> when none). Rejects
   *  noRemote | authFailed | networkError | pushRejected | git | noRepo. */
  push(): Promise<PushResult>;
}
```

`src/ipc/tauri.ts`:

```ts
fetch: () => invoke<FetchResult>('fetch'),
pull:  () => invoke<PullResult>('pull'),
push:  () => invoke<PushResult>('push'),
```

## 4. Frontend

### 4.1 App wiring (`App.tsx`)

New state:

```ts
const [remoteNotice, setRemoteNotice] = useState<
  { text: string; tone: 'ok' | 'warn' } | null>(null);
const [remoteError, setRemoteError] = useState<string | null>(null);
const noticeId = useRef(0);
```

`showNotice(text, tone)`: sets `remoteNotice`, bumps `noticeId`, and clears it after **5 s** iff
the id still matches (stale-timeout guard). Errors from remote ops go to `setRemoteError`
(dismissible banner, §4.2) — never to `statusError`/`branchesError`.

Handlers (all: `setRemoteError(null); setRemoteNotice(null); setMutating(true)` on entry,
`finally { setMutating(false) }` — reuse the single global `mutating` flag):

```ts
async function handleFetch(): Promise<void>;
// res = await ipc.fetch()
// notice ok: `Fetched ${n} remote${s}` + when total updatedRefs > 0: ` — ${k} ref${s} updated`
// then: await Promise.all([refetchBranches(), refetchGraph()])   // status unaffected

async function handlePull(): Promise<void>;
// res = await ipc.pull(); switch (res.kind):
//   'upToDate'      -> notice ok  `Already up to date`
//   'fastForwarded' -> notice ok  `Fast-forwarded ${res.branch} to ${shortOid(res.to)}`
//   'wouldNotFastForward' -> notice WARN:
//     `Cannot fast-forward: '${res.branch}' has ${res.ahead} local commit(s) not on upstream. `
//     + `Bonsai v1 does not merge — push your commits or reconcile via the CLI.`
// then FULL refresh (branch tip may have moved — same as post-checkout):
//   if (repoPath !== null) setRepo(await ipc.openRepo(repoPath));
//   await Promise.all([refetchBranches(), refetchStatus(), refetchGraph()]);
// catch -> setRemoteError(errorMessage(e))

async function handlePush(): Promise<void>;
// res = await ipc.push(); switch (res.kind):
//   'upToDate' -> notice ok `Already up to date`
//   'pushed'   -> notice ok `Pushed ${res.branch} → ${res.remote}/${res.branch}`
//                 + (res.setUpstream ? ' (upstream set)' : '')
// then: await Promise.all([refetchBranches(), refetchGraph()])   // ahead badge -> 0
// catch -> setRemoteError(errorMessage(e))
```

`handleFetch` errors also → `setRemoteError`. All backend messages are pre-worded (§2.3) — the
frontend renders `e.message` verbatim, no per-kind branching.

### 4.2 Toolbar + banners (header area)

In `<header className="header">`, LEFT of the existing Refresh button, three text+icon buttons
(class `.toolbar-btn`: 12px label, icon glyph, same height as `.btn-icon`):

- `↓ Fetch`  — `disabled={!repoOpen || refreshing || mutating}`
- `⇣ Pull`   — `disabled={!repoOpen || refreshing || mutating || !canPullPush}`
- `↑ Push`   — same `disabled` as Pull; when the current branch has an upstream,
  `title` = `Push <branch> to <upstream>`; when not, `title` =
  `Push <branch> to origin/<branch> and set upstream` (the auto-set-upstream behavior is
  spelled out in UI copy — locked decision).

`canPullPush = repo?.head != null && !repo.head.detached && !repo.head.unborn` (backend still
guards — buttons are convenience gating only).

Below the header (above `.panes`), when set:
- `remoteError !== null` → `.error-banner` with a ✕ dismiss (`setRemoteError(null)`).
- `remoteNotice !== null` → `.remote-notice` line; tone `ok` = text-2 neutral, tone `warn` =
  `--warning`-tinted background. Auto-clears after 5 s; a ✕ is not required.

Only one of banner/notice renders at a time (each handler clears both on entry; an error
replaces any notice).

## 5. Mock IPC (`src/ipc/mock.ts`) — stateful

Module state additions: `let mockFetched = false;` (reset in `openRepo` when the path changes,
alongside `mockStatus`/`mockBranches`).

Failure triggers use a NEW URL param `?remote=` (kept separate from `?fixture=`, which selects
graph variants — the two compose): `authfail | network | rejected | conflict`.

- **`fetch()`**: `delay(400)`. `remote=authfail` → throw `{ kind: 'authFailed', message:
  "authentication failed for 'origin': no usable credentials. Configure a Git credential helper
  (e.g. Git Credential Manager) for HTTPS remotes, or run an SSH agent for SSH remotes." }`;
  `remote=network` → `{ kind: 'networkError', message: "network error talking to 'origin':
  failed to resolve address" }`. Otherwise first call: set `mockFetched = true`, set `main`'s
  `behind = 1` in `mockBranches`, return `{ remotes: [{ remote: 'origin', receivedObjects: 12,
  updatedRefs: 1 }] }`; subsequent calls return `{ remotes: [{ remote: 'origin',
  receivedObjects: 0, updatedRefs: 0 }] }`.
- **`pull()`**: `delay(400)`. `remote=authfail`/`network` → same errors as fetch.
  `remote=conflict` → throw `{ kind: 'checkoutConflict', message: 'cannot pull: local changes
  would be overwritten by the update. Commit or discard them first.' }`. Current branch
  (`mockHeadBranch`) lookup in `mockBranches.local`:
  - `upstream === null` → throw `{ kind: 'noUpstream', message: "cannot pull: branch '<name>'
    has no upstream configured" }` (harness path: check out `fix/watcher-debounce`... which is
    the conflict branch — use `experiment-unmerged` instead? No: `experiment-unmerged` has
    `upstream: null` and checks out cleanly — that IS the designated no-upstream pull branch).
  - `ahead > 0 && behind > 0` (e.g. `feature/sidebar`: 2/1) → return
    `{ kind: 'wouldNotFastForward', branch, ahead, behind }`, change nothing.
  - `behind > 0` (main after a fetch) → `from = mockHeadOid`, `mockHeadOid = randomOid()`,
    set `behind = 0`, return `{ kind: 'fastForwarded', branch, from, to: mockHeadOid }`.
  - else → `{ kind: 'upToDate' }`.
- **`push()`**: `delay(400)`. `remote=authfail`/`network` → same errors. `remote=rejected` →
  throw `{ kind: 'pushRejected', message: 'push rejected: the remote contains commits you do
  not have. Fetch/pull first — Bonsai v1 never force-pushes.' }`. Current branch:
  - `upstream === null` → set `upstream = 'origin/<name>'`, `ahead = 0`, `behind = 0`, add
    `origin/<name>` to `mockBranches.remote` (sorted), return
    `{ kind: 'pushed', remote: 'origin', branch, setUpstream: true }`.
  - `ahead > 0` → set `ahead = 0`, return `{ kind: 'pushed', remote: 'origin', branch,
    setUpstream: false }`.
  - else → `{ kind: 'upToDate', remote: 'origin', branch }`.
- **`commit()` addition**: on success, ALSO bump the current branch's ahead count in
  `mockBranches` (`ahead = (ahead ?? 0) + 1` when it has an upstream) — gives the harness the
  natural commit→push story (`main` starts 0/0; commit → `↑1`; push → badge clears).

Mock never invokes credential logic (browser harness has no credentials by design).

## 6. Testing (contract for tester)

All scratch repos via the M3 `scratch_dir()` helper (**D:\Temp\bonsai-scratch — hard rule**);
run with `TMP`/`TEMP=D:\Temp`. Remote fixture pattern for every round-trip test:

```
bare   = git init --bare  <scratch>/origin.git
seed   = git clone <bare> seed   (CLI; repo-local identity)  -- pushes fixture commits
work   = git clone <bare> work   -- the repo Bonsai operates on (has origin + upstream
                                    tracking configured by clone; use plain paths — the
                                    local transport needs NO credentials)
```

**Coverage split (honest):** the `file://`/local transport never invokes the credentials
callback and never produces Net/Http/Ssh errors. Therefore: (a) the retry guard and error
mapping are covered STRUCTURALLY by direct unit tests (§6.5, §6.6) — `next_cred_method` and
`map_remote_err` are pure functions designed for exactly this; (b) the real
credential-helper/agent path is covered ONLY by the USER CHECKPOINT network round-trip. Do not
claim otherwise in the gate evidence.

### 6.1 `fetch_all`

1. seed pushes a new commit to bare → our `fetch_all(work)` → `refs/remotes/origin/<default>`
   in work equals `git rev-parse <default>` in bare; result has 1 remote, `updated_refs >= 1`,
   `received_objects > 0`. Twin oracle: `git fetch` in a clone twin yields the same
   remote-tracking oid.
2. Second fetch with nothing new → `updated_refs == 0`.
3. Repo with no remotes → `Err(NoRemote)`.
4. Two remotes (add bare2 as `backup`) → result lists both in `repo.remotes()` order.
5. Fetch works with detached HEAD (detach work first) → `Ok`.

### 6.2 `pull_ff`

1. **FF pull**: seed pushes commit adding `new-file.txt`; work fetches nothing manually —
   `pull_ff(work)` → `FastForwarded { from: old, to: bare tip }`; work's branch ref == bare tip
   (`git rev-parse`), `new-file.txt` exists, `git status --porcelain` empty. Twin oracle:
   `git pull --ff-only` produces identical ref + worktree.
2. **Up to date**: immediate second pull → `UpToDate`.
3. **Ahead only**: local commit in work, bare unchanged → `UpToDate` (behind == 0), ref
   unchanged.
4. **Diverged**: local commit in work AND seed pushes a different commit →
   `WouldNotFastForward { ahead: 1, behind: 1 }`; work's branch ref unchanged, worktree
   unchanged, porcelain unchanged (remote-tracking ref DID move — assert that too).
5. **Dirty conflict**: seed pushes a commit modifying `shared.txt`; work modifies `shared.txt`
   uncommitted → `Err(CheckoutConflict)`; branch ref unchanged, file content unchanged.
   (Remote-tracking ref moved — fetch happened — that is correct and asserted.)
6. **No upstream**: `git branch --unset-upstream` in work → `Err(NoUpstream)`.
7. **Detached** → `Err(Git)` with the §2.5 message. **Unborn** (fresh init, remote added) →
   `Err(Git)`.

### 6.3 `push_current`

1. **Push**: local commit in work → `Pushed { set_upstream: false }`; `git rev-parse <branch>`
   in bare equals work's tip; remote-tracking ref updated. Twin oracle: `git push` from a twin
   clone state agrees.
2. **Up to date**: immediate second push → `UpToDate` (and completes without touching the
   remote — assert via §2.6 step 3 short-circuit; behavior test: works even if bare dir is
   renamed away… skip the rename trick, just assert the variant).
3. **No upstream, origin exists**: new branch `topic` in work (no upstream), commit, push →
   `Pushed { set_upstream: true }`; bare has `refs/heads/topic`; work config has
   `branch.topic.remote == origin` and `branch.topic.merge == refs/heads/topic`
   (`git config` oracle); `refs/remotes/origin/topic` exists.
4. **No upstream, no origin**: remove origin → `Err(NoRemote)`.
5. **Non-FF rejection**: seed pushes to bare after work's last fetch; work has its own commit →
   `push_current(work)` → `Err(PushRejected)`, message non-empty; bare's ref UNCHANGED
   (`git rev-parse` before == after). Twin oracle: `git push` also fails non-zero.
6. **Detached / unborn** → `Err(Git)` with §2.6 messages.

### 6.4 Watcher interplay

After `fetch_all` updates a remote-tracking ref, assert the changed path
(`refs/remotes/...` or `packed-refs`) passes `watcher.rs::is_relevant` (unit-level; no timing
test).

### 6.5 Credential guard unit tests (`next_cred_method`)

1. allowed = USER_PASS_PLAINTEXT|SSH_KEY|DEFAULT → sequence Helper, SshAgent, Default, None,
   None (idempotent exhaustion).
2. allowed = SSH_KEY only → SshAgent, None.
3. allowed = empty → None immediately.
4. Second call with same single-method allowed → None (each method at most once).

### 6.6 `map_remote_err` unit tests

Construct `git2::Error::new(code, class, msg)` for every table row in §2.3 (Callback+sentinel →
AuthFailed; Auth code under class Http and class Ssh → AuthFailed; NotFastForward →
PushRejected; class Net / Http / Ssh non-auth → NetworkError; GenericError/class None → Git)
and assert the AppError variant + that the message contains the interpolated context.

### 6.7 Command-level tests (`commands.rs`)

`fetch_inner` / `pull_inner` / `push_inner` with no repo open → `AppError::NoRepo` (extend the
existing pattern).

### 6.8 Frontend smoke (browser harness, `VITE_MOCK_IPC=1 pnpm dev`)

1. Header shows Fetch / Pull / Push left of Refresh; all disabled during any op (`mutating`).
2. Fetch → notice `Fetched 1 remote — 1 ref updated`; sidebar `main` gains `↓1` badge.
3. Pull (on main, after fetch) → notice `Fast-forwarded main to <short>`; header oid changes;
   `↓1` badge clears; second Pull → `Already up to date`.
4. Checkout `feature/sidebar` → Pull → WARN notice with the wouldNotFastForward copy; badges
   unchanged. Push → `Pushed feature/sidebar → origin/feature/sidebar`; `↑2` clears.
5. Checkout `experiment-unmerged` → Pull → error banner `cannot pull: ... no upstream
   configured` (kind noUpstream); Push → `Pushed ... (upstream set)`; row now shows upstream
   badge behavior (0/0) and REMOTES gains `origin/experiment-unmerged`.
6. Commit flow: stage + commit on main → `↑1` appears; Push → clears.
7. `?remote=authfail` → each button shows the authFailed banner (dismissible), nothing else
   changes; `?remote=network` → networkError banner; `?remote=rejected` → Push shows the
   pushRejected banner; `?remote=conflict` → Pull shows the checkoutConflict banner.
8. Notice auto-clears after ~5 s; a new op immediately replaces banner/notice.
9. `?fixture=detached` → Pull and Push disabled, Fetch enabled.
10. No `@tauri-apps/*` module executed; no console errors.

## 7. Sub-increment split for senior-dev

- **M6a — Rust backend + tests.** `error.rs` variants, `git/remote.rs` (+ `git/mod.rs`),
  commands + registration, tests §6.1–§6.7.
  Gate: `cargo test` green, `cargo clippy -- -D warnings` clean, scratch dirs on D:.
- **M6b — Frontend + IPC/mock.** `types.ts`/`tauri.ts`, stateful `mock.ts` + `?remote=`
  triggers + commit-ahead bump, `App.tsx` toolbar/handlers/notice, styles.
  Gate: `pnpm build` green; §6.8 smoke passes in the harness.

## 8. Acceptance criteria

AI gate:
- §6.1–§6.7 Rust tests pass against local bare-repo remotes (git CLI as oracle);
  `cargo check`/`clippy`/`test`, `pnpm build` green.
- Browser harness passes §6.8 (screenshots: toolbar, ff-pull notice, wouldNotFF warning,
  authFailed banner, badge transitions).
- Reviewer confirms: NO force refspec (`+`) anywhere; NO `.force()` checkout; credentials
  callback has the §2.2 exhaustion guard; no password prompt/storage code exists; the
  coverage-split statement (§6 preamble) is reflected honestly in the gate evidence.

USER CHECKPOINT (never self-declared): in the native app against a REAL network remote
(e.g. a GitHub repo already set up with Git Credential Manager or an SSH agent): Fetch succeeds;
commit locally and Push (badge clears; remote shows the commit); create a divergence upstream
and confirm Pull reports the wouldNotFastForward warning changing nothing, then after
reconciling via CLI a Pull fast-forwards. Also confirm a bogus remote URL yields the
networkError banner, not a hang or crash.

## 9. Ambiguities resolved here (flag to orchestrator if disagreed)

- **Fetch = ALL remotes, sequential, fail-fast.** One toolbar button needs one obvious meaning;
  GitKraken semantics; the common case is a single `origin`. Fail-fast on the first failing
  remote (named in the error) instead of partial-success aggregation — aggregation needs a
  mixed-result UI that isn't worth it in v1. (Pull fetches ONLY the upstream's remote — it is a
  targeted operation; the Fetch button covers the rest.)
- **Push with no upstream → push to `origin/<branch>` AND set upstream**, spelled out in the
  button tooltip copy. Erroring instead would make the first push of every new branch a
  CLI trip, gutting the feature; matches `push.default=current` + `--set-upstream` muscle
  memory. No origin remote → clear `noRemote` error.
- **Commands are argument-less** (`fetch`/`pull`/`push` on the open repo / current branch) —
  smallest surface, matches the toolbar, and per-branch/per-remote parameterization has no UI
  in v1.
- **`wouldNotFastForward` is a success variant; push rejection is an error** (`pushRejected`).
  Pull-not-possible is information (nothing failed, fetch DID land); push rejection is a failed
  operation. Warning-toned notice vs. error banner respectively.
- **Five new error kinds** — `noRemote`, `noUpstream`, `authFailed`, `networkError`,
  `pushRejected`. `nonFastForward` (candidate) is NOT added: the pull case is a result variant
  and the push case maps to `pushRejected`. Detached/unborn guards reuse kind `git` with
  bespoke messages (M5 precedent — UI never branches on them; buttons are disabled anyway).
- **No progress channel in v1** — indeterminate busy via the existing global `mutating` flag;
  transfer-progress streaming is Polish. Rationale: correct cancellation + throttling is real
  surface, and the ops are seconds-scale on typical repos. Consequence (documented): a slow
  network op pins the UI in busy state until libgit2 returns; no in-app cancel in v1.
- **Success feedback = transient 5 s notice line under the header**, not a toast system —
  smallest thing that closes the feedback loop; toasts are Polish.
- **Dirty pull reuses `checkoutConflict`** — identical semantics and recovery action as M5
  dirty checkout; a new kind would duplicate copy.
- **Push up-to-date is detected locally** (remote-tracking ref == local tip) and skips the
  network. Stale-tracking edge: if the remote moved meanwhile, the user Fetches first — same
  contract as git itself.
- **No prune, default tags (Auto), no `remote=` parameter on fetch** — v1 keeps libgit2
  defaults; prune is Polish.
- **`?remote=` is a separate mock URL param** from `?fixture=` so failure triggers compose with
  graph variants.
- **Mock `commit()` now bumps the current branch's ahead count** — minimal coupling, buys the
  natural commit→push harness story (unlike graph-fixture coupling, this is one integer).

---

## Addendum 2026-08-04 — Helper step delegates to real `git credential fill`

**Bug:** libgit2's `Cred::credential_helper` (§2.2 old Helper arm) is libgit2's OWN
reimplementation of the credential-helper protocol. It can fail to correctly invoke some
configured helpers even when the user's actual `git` CLI resolves the identical config
successfully (confirmed: `git pull`/`git push` succeed from the very terminal Bonsai is launched
from; Bonsai fails with the generic `authFailed` message). Root cause is libgit2's internal
implementation, not PATH/env/config on the user's machine.

**Fix:** the Helper step shells out to the REAL `git credential fill` — the same resolution the
CLI itself uses — instead of asking libgit2 to reimplement it. Everything else in §2.2/§2.3
(guard, method order, exhaustion, error mapping) is UNCHANGED. No OS branching: `git` resolves
whatever helper is configured for the platform it runs on (GCM, `osxkeychain`, `wincred`,
`libsecret`, `store`, ...) — Bonsai does not need to know which.

No new crate dependency — `std::process::Command` (already `std`) is sufficient.

### A.1 New function — `credential_fill`

```rust
use std::io::Write;
use std::process::{Command, Stdio};

/// Resolves HTTPS credentials via the user's REAL configured credential
/// helper by shelling out to `git credential fill` — NOT libgit2's own
/// reimplementation (see addendum preamble). `repo_path`: cwd for the child
/// process when `Some`, so repo-local `credential.helper` config resolves
/// exactly like the `git` CLI does (it also reads cwd's repo config); `None`
/// when no repo exists yet (clone, §A.3) — global/system config still
/// resolves without a cwd, matching what `git clone` itself does before a
/// repo exists.
///
/// NEVER prompts (`GIT_TERMINAL_PROMPT=0` is set unconditionally on the
/// child — a cache miss must fail fast, not block on an interactive
/// prompt — this preserves the locked never-prompt policy, §2.2). NEVER
/// panics. Returns `None` on ANY failure path: binary not found / spawn
/// error, non-zero exit status, I/O error writing stdin, stdout not valid
/// UTF-8, or `username`/`password` missing or empty in the parsed output.
/// The caller (`acquire_cred`, §A.2) treats `None` exactly like the old
/// `Cred::credential_helper(..).is_err()` branch: fall through to the next
/// credential method.
fn credential_fill(repo_path: Option<&Path>, url: &str) -> Option<(String, String)> {
    let mut cmd = Command::new("git");
    cmd.args(["credential", "fill"])
        .env("GIT_TERMINAL_PROMPT", "0") // REQUIRED — never block on a prompt
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null()); // discarded — never logged, never in an error path
    if let Some(p) = repo_path {
        cmd.current_dir(p);
    }

    let mut child = cmd.spawn().ok()?;
    {
        let stdin = child.stdin.as_mut()?;
        stdin.write_all(format!("url={url}\n\n").as_bytes()).ok()?;
    } // stdin dropped here -> EOF sent to the child before we wait on it

    let output = child.wait_with_output().ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8(output.stdout).ok()?;

    let (mut username, mut password) = (None, None);
    for line in stdout.lines() {
        let Some((key, value)) = line.split_once('=') else { continue };
        match key {
            "username" => username = Some(value.to_string()),
            "password" => password = Some(value.to_string()),
            _ => {} // ignore unknown keys (protocol/host/path/url echo, etc.)
        }
    }
    match (username, password) {
        (Some(u), Some(p)) if !u.is_empty() && !p.is_empty() => Some((u, p)),
        _ => None,
    }
}
```

**Exact parsing rules** (for the tester + reviewer to check byte-for-byte):
- Each stdout line is split on the FIRST `=` only (`str::split_once('=')`).
- A line with no `=` is ignored (not an error).
- Any key other than `username`/`password` is ignored (the helper may echo back `protocol=`,
  `host=`, `path=`, `url=`, or emit nothing else at all).
- Both `username` AND `password` must have been seen, and both non-empty after trimming (`!=
  ""`), or the function returns `None`. Do NOT trim the values themselves before returning —
  only the emptiness check is a `!is_empty()` on the raw parsed value (a helper is not expected
  to pad with whitespace; do not silently rewrite what it returns).

### A.2 `acquire_cred` — new signature and Helper arm

`config: &git2::Config` is DROPPED from the signature (after this change nothing in the function
body needs it — the SshAgent and Default arms never used it, and Helper no longer does either).
`repo_path: Option<&Path>` is added as the new first parameter:

```rust
pub(crate) fn acquire_cred(
    repo_path: Option<&Path>,
    attempts: &RefCell<CredAttempts>,
    url: &str,
    username_from_url: Option<&str>,
    allowed: git2::CredentialType,
) -> Result<git2::Cred, git2::Error> {
    loop {
        let method = next_cred_method(&mut attempts.borrow_mut(), allowed);
        match method {
            Some(CredMethod::Helper) => {
                if let Some((user, pass)) = credential_fill(repo_path, url) {
                    if let Ok(cred) = git2::Cred::userpass_plaintext(&user, &pass) {
                        return Ok(cred);
                    }
                    // userpass_plaintext failing is theoretical (string-only
                    // validation) — treat identically to a construction
                    // failure: the guard already marked Helper as tried, so
                    // the loop's next iteration moves on to SshAgent.
                }
                // credential_fill returned None (no cached credentials / any
                // failure mode, §A.1) -> same fall-through as before.
            }
            Some(CredMethod::SshAgent) => { /* UNCHANGED */ }
            Some(CredMethod::Default) => { /* UNCHANGED */ }
            None => { /* UNCHANGED — CRED_EXHAUSTED_MSG error */ }
        }
    }
}
```

`next_cred_method`, `CredMethod`, `CredAttempts`, `CRED_EXHAUSTED_MSG`, the SshAgent arm, the
Default arm, and the exhaustion arm are BYTE-FOR-BYTE UNCHANGED — only the Helper arm's body and
the function's parameter list change.

### A.3 Call sites (all 5, confirmed by reading each file)

| Site | File:~line | What's in scope | New call |
|---|---|---|---|
| `fetch_remote` | `remote.rs:~233` | `repo: &git2::Repository` in scope | `acquire_cred(repo.workdir(), &attempts, url, username_from_url, allowed)`. The `let config = repo.config()?;` line (currently `remote.rs:227`) becomes DEAD (its only use was this call) — DELETE it. |
| `push_current` | `remote.rs:~463` | `repo` in scope; `config` ALSO still needed later in this function for `config.get_string(&format!("branch.{name}.merge"))` (§2.6 step 2) | `acquire_cred(repo.workdir(), &attempts, url, username_from_url, allowed)`. KEEP `let config = repo.config()?;` — it is still used elsewhere in `push_current`. |
| `tags.rs` `push_tag` | `tags.rs:~144` | `repo: git2::Repository` (from `open_repo_at(workdir)`) in scope; `config` (`tags.rs:140`) is used ONLY for this call in the whole function | `acquire_cred(repo.workdir(), &attempts, url, username_from_url, allowed)`. DELETE the now-dead `let config = repo.config()?;` at `tags.rs:140`. |
| `submodule.rs` `update_submodule` | `submodule.rs:~157` | `repo: git2::Repository` (from `open_workdir_repo(workdir)`) in scope; `config` (`submodule.rs:154`) is used ONLY for this call | `acquire_cred(repo.workdir(), &attempts, url, username_from_url, allowed)`. DELETE the now-dead `let config = repo.config()?;` at `submodule.rs:154`. |
| `clone.rs` `clone_repo` | `clone.rs:~85` | NO repo exists yet — the destination doesn't exist until the clone succeeds; `let config = git2::Config::open_default()?;` (`clone.rs:77`) was only ever used for this call | `acquire_cred(None, &attempts, url, username_from_url, allowed)`. DELETE the now-dead `let config = git2::Config::open_default()?;` at `clone.rs:77`. Update the adjacent comment ("2. Credentials: no repo yet -> default (global+system) config (§OPEN-5)") to note `credential_fill(None, url)` similarly falls back to global/system git config with no cwd override — matching what `git clone` itself does before a repo exists. |

`git2::Repository::workdir()` returns `Option<&Path>` — pass it straight through, no
`unwrap`/`expect` (a bare repo would yield `None`, which is fine: `credential_fill` simply omits
`current_dir`, same as the clone case).

### A.4 Safety requirements (explicit, testable — reviewer MUST-FIX if any fail)

1. **No interactive prompt, ever.** `credential_fill` sets `GIT_TERMINAL_PROMPT=0`
   UNCONDITIONALLY on every child process it spawns — not behind a flag, not only when a helper
   is absent. Reviewer: grep the diff for exactly one `Command::new("git")` construction site and
   confirm `.env("GIT_TERMINAL_PROMPT", "0")` is on it.
2. **No panic on any subprocess failure mode.** Every fallible step inside `credential_fill` uses
   `?`/`.ok()?`/`match` inside a function returning `Option<...>` — NO `.unwrap()`, `.expect()`,
   or indexing that could panic anywhere in the function body. Binary-not-found, spawn failure,
   non-zero exit, stdin write failure, non-UTF-8 stdout, and missing/empty fields ALL fall
   through to `None` without unwinding.
3. **Credentials never logged.** No `eprintln!`/`println!`/`tracing::*`/`dbg!` anywhere in
   `credential_fill` or the Helper arm of `acquire_cred` prints `url`, `username`, `password`, or
   raw subprocess stdout. The child's stderr is `Stdio::null()` — discarded, never captured into
   any variable, error, or log. `map_remote_err`'s messages (§2.3) interpolate only `context`
   (remote name) and `e.message()` (libgit2's own diagnostic text) — neither can contain
   plaintext credentials since libgit2 never receives them in the failure case. Reviewer: grep
   the diff for `pass`/`password`/`user`/`stdout` reaching any `format!`/print/error string
   outside `credential_fill`'s own return value.
4. **Fall-through semantics preserved exactly.** SshAgent and Default arms of `acquire_cred`, and
   `next_cred_method`/`CredMethod`/`CredAttempts`, are untouched — Helper still consumes its
   one-shot attempt (via `next_cred_method`) regardless of whether `credential_fill` succeeds or
   returns `None`, so SSH agent and Default still get exactly one chance each afterward, same as
   before this change.

### A.5 OPTIONAL — sharper `authFailed` wording when no helper is configured

Not required for the fix; nice-to-have only. Self-contained — NO signature change to
`map_remote_err` (it stays `fn(e: git2::Error, context: &str) -> AppError`). Inside the existing
`auth_msg` closure (`remote.rs:171-177`), check whether a credential helper is configured at all
via a fresh `git2::Config::open_default()` (advisory only — this reads global/system config,
which is an acceptable approximation for wording purposes even though the real resolution is
repo-local; it does NOT affect the actual credential flow, which already correctly uses
`repo_path`/cwd via `git credential fill`):

```rust
let auth_msg = || {
    let helper_configured = git2::Config::open_default()
        .ok()
        .and_then(|c| c.get_string("credential.helper").ok())
        .map(|s| !s.trim().is_empty())
        .unwrap_or(false); // covers Err(NotFound) (unset) and any other lookup failure

    if helper_configured {
        format!(
            "authentication failed for '{context}': the configured credential helper has no \
             cached credentials for this remote. Run the equivalent git command in a terminal \
             once to (re-)authenticate, or run an SSH agent for SSH remotes."
        )
    } else {
        format!(
            "authentication failed for '{context}': no usable credentials and no Git credential \
             helper is configured. Configure one (e.g. Git Credential Manager) for HTTPS \
             remotes, or run an SSH agent for SSH remotes."
        )
    }
};
```

These are the ONLY two variants — do not improvise a third. If implemented, update the §6.6
`map_cred_exhausted_to_auth_failed` test's message assertions accordingly (it currently checks
for the substring `"credential helper"`, which both variants above still contain — verify, don't
assume).

### A.6 Test requirements (for the tester)

Existing `next_cred_method` / `map_remote_err` unit tests (`remote.rs:715-845`, i.e. §6.5/§6.6)
are UNCHANGED by this addendum and MUST still pass as-is (unless A.5 is implemented, in which
case only the message-substring assertion needs revisiting per A.5's last paragraph — the test
STRUCTURE is unchanged).

**Fixture: fake credential-helper scripts.** Use `common::scratch_dir()` (the existing
`D:\Temp\bonsai-scratch` helper, per §6 preamble — this project's test suite targets Windows;
do not introduce a second scratch-dir convention). Real `git` invokes a `credential.helper`
value as a shell command (`sh -c "<helper> $*"`) on every platform it runs on, including via
Git Bash's `sh` on Windows — so a POSIX shell script is the right fixture format (not a
`.bat`/`.ps1`, which `git` would not invoke the same way; not Python, to avoid a second runtime
dependency in the test fixture). Write these into the scratch dir per test:

```sh
#!/bin/sh
# fixtures/good-helper.sh — responds to `git credential fill` with fixed creds.
cat > /dev/null    # drain stdin (the "url=...\n\n" payload), then respond
echo "username=bonsai-test-user"
echo "password=bonsai-test-pass"
```

```sh
#!/bin/sh
# fixtures/bad-exit-helper.sh — simulates a broken helper.
cat > /dev/null
exit 1
```

```sh
#!/bin/sh
# fixtures/partial-helper.sh — responds but omits password (simulates a
# helper that recognizes the request but has no cached secret).
cat > /dev/null
echo "username=bonsai-test-user"
```

`chmod +x` each script after writing (`std::os::unix::fs::PermissionsExt` — guard with
`#[cfg(unix)]`/skip on Windows if the CI runner cannot chmod, since Git for Windows invokes the
script via its own `sh` regardless of the executable bit in some configurations; if this turns
out to be flaky on the Windows runner, document the workaround rather than silently skipping the
test). Wire up per test via `git(dir, &["config", "credential.helper", script_path])` in a
scratch repo (repo-local — this is what makes `repo_path` threading in §A.3 meaningful to test).

Required cases:

**(a) Well-formed response.** Repo-local `credential.helper` = `good-helper.sh`. Call
`credential_fill(Some(repo_dir), "https://example.com/repo.git")` directly (unit/integration test
in `remote.rs` or a new `credential_fill_cli.rs` under `tests/`, per the project's existing
`#[cfg(test)]`-in-lib vs. `tests/` split — follow whichever `remote_cli.rs` already uses for
similarly-scoped local-only tests). Assert `Some(("bonsai-test-user".into(),
"bonsai-test-pass".into()))`.

**(b) Failure modes fall through to `None` without panicking or hanging** (assert each completes,
via `Instant`, within a few seconds — see (c) for the exact pattern):
   - `credential.helper` = `bad-exit-helper.sh` → `None`.
   - `credential.helper` = `/path/does/not/exist` (nonexistent binary) → `None` (git itself fails
     to invoke it; our function must not panic on that failure either).
   - `credential.helper` = `partial-helper.sh` (missing `password=`) → `None`.

**(c) `GIT_TERMINAL_PROMPT=0` prevents a hang.** In a scratch repo with NO `credential.helper`
configured at all (unset — the platform default resolves to nothing usable in the test sandbox)
call `credential_fill(Some(repo_dir), "https://example-nonexistent-host.invalid/repo.git")` and
assert it returns within a generous bound using wall-clock timing (we cannot literally wait for
"would have hung" — this is the practical proxy):

```rust
let start = std::time::Instant::now();
let result = credential_fill(Some(repo_dir), "https://example-nonexistent-host.invalid/repo.git");
assert!(start.elapsed() < std::time::Duration::from_secs(5), "credential_fill took too long — \
    possible interactive-prompt hang (GIT_TERMINAL_PROMPT not honored?)");
assert_eq!(result, None);
```

Document in the test's doc comment that this bounds a hang empirically (a genuinely interactive
prompt would block indefinitely, not merely run slow) rather than proving non-blocking behavior
by construction — combined with the source-level check in §A.4 item 1, this is the practical
verification available without spawning and killing a truly-hung child process.

**(d) Regression guard.** `remote.rs:720-780` (`cred_guard_full_sequence`,
`cred_guard_ssh_only`, `cred_guard_empty_allowed`, `cred_guard_single_method_once`) and
`remote.rs:762-845` (`map_remote_err` tests) still compile and pass unmodified — `cargo test` in
CI is sufficient proof; no new assertions needed for these specifically.
