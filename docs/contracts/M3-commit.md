# M3 — Stage / Unstage / Commit: Implementation Contract

Status: authoritative for M3. Implementer: senior-dev. Builds on `docs/contracts/M0-scaffold.md`
(error shape, IPC conventions), `M1-status.md` (StatusSnapshot, watcher, refetch patterns),
`M2-graph.md` (GraphLayout), `ui-reference.md` (§2 tokens, §7 file colors, §8 buttons/errors).

Scope (locked by product decisions): **file-level staging only** — no hunk staging, no amend.
Author/committer come from git config; clear error if unset, NO fallback identity.

Guardrail note: **none of M3's operations are destructive.** Stage/unstage only move content
between index and "not index" — the worktree is never modified; commit only adds objects. No UI
confirmation dialogs are required in M3. (Discard/checkout-file, which IS destructive, is not in
this milestone.)

---

## 1. New / changed files

```
src-tauri/
  src/error.rs               # + EmptyMessage, ConfigMissing, NothingToCommit variants
  src/git/stage.rs           # NEW: stage_paths / unstage_paths + tests   (git/mod.rs: pub mod stage;)
  src/git/commit.rs          # NEW: create_commit, resolve_signature, CommitResult + tests
  src/commands.rs            # + stage, unstage, commit commands
  src/lib.rs                 # register stage, unstage, commit
  tests/commit_noconfig.rs   # NEW: env-isolated missing-config integration test (own process)
src/
  ipc/types.ts               # + CommitResult; IpcApi + stage/unstage/commit; AppError kinds
  ipc/tauri.ts               # + stage, unstage, commit wrappers
  ipc/mock.ts                # stateful mock: mutating fixture status + commit simulation
  components/StatusPanel.tsx # + row/section action buttons, onStage/onUnstage/busy props
  components/CommitBox.tsx   # NEW: message textarea + Commit button + inline error
  App.tsx                    # mutation handlers, busy state, post-commit refresh
  styles.css                 # row action buttons, section actions, commit box, right-panel flex
```

## 2. Rust backend

### 2.1 Path convention & validation (both stage and unstage)

Paths on the wire are **worktree-relative with forward slashes** — the exact strings from
`StatusSnapshot` (`StatusEntry.path` / `origPath`). Backend validation, per path, before any index
mutation: reject empty strings, absolute paths (leading `/` or drive letter `X:`), and any `..`
component → `AppError::Other(format!("invalid path: {p}"))`. Pass validated strings to git2 as
`Path::new(p)` unchanged (libgit2 accepts `/` on Windows); use `workdir.join(p)` only for
filesystem existence checks. An empty `paths` vec is a no-op `Ok(())`.

**Rename expansion is the frontend's job** (§4.2): for a `renamed` entry the frontend sends BOTH
`origPath` and `path` in the same call. The backend has no rename special-casing — the
present/missing rule below handles both sides naturally.

### 2.2 Batch semantics — all-or-nothing (decision)

Each command call is atomic: validate all paths first, then apply all index operations in memory,
then a **single `index.write()`** at the end (stage) / single libgit2 reset (unstage). Any error
before the write aborts the whole call with no index change. Rationale: trivially correct with
git2's in-memory index; per-path partial results would need a new error shape and a UI that can
render them, for failures that only occur on races (file deleted mid-click) — where "nothing
happened, snapshot refreshes" is the right outcome anyway. Rejected alternative: best-effort with
per-path errors — complexity without a user-visible benefit at v1 scale.

### 2.3 `src-tauri/src/git/stage.rs`

```rust
use std::path::Path;
use crate::error::AppError;

/// Blocking. Stages each path into the index (git add / git rm --cached semantics combined):
/// path exists in the worktree (symlink_metadata().is_ok(), so symlinks count)
///   -> index.add_path(p)      // covers untracked, modified, typechange, rename NEW side
/// path missing from the worktree
///   -> index.remove_path(p)   // covers deleted, rename OLD side
/// Then index.write() once. Repo opened with NO_SEARCH; bare repo -> AppError::Git.
/// Note: add_path has `git add -f` semantics (adds even ignored files); acceptable — the UI
/// only offers paths already present in StatusSnapshot.
pub fn stage_paths(workdir: &Path, paths: &[String]) -> Result<(), AppError>;

/// Blocking. Unstages each path (index entry reset to HEAD's version, worktree untouched):
/// HEAD resolvable -> repo.reset_default(Some(&head_commit.as_object()), paths)
///                    (libgit2 git_reset_default == `git restore --staged -- <paths>`)
/// HEAD unborn     -> for each path: index.remove_path(p); then index.write()
///                    (removing from index == unstaging when there is no HEAD to restore from)
/// Unborn detection: repo.head() error with code UnbornBranch or NotFound.
pub fn unstage_paths(workdir: &Path, paths: &[String]) -> Result<(), AppError>;
```

Behavioral table (must all hold; tested in §6):

| Status entry | stage sends | effect | unstage sends | effect |
|---|---|---|---|---|
| untracked | `[path]` | add_path → staged Added | — | — |
| unstaged Modified/Typechange | `[path]` | add_path | staged M/T: `[path]` | reset to HEAD version |
| unstaged Deleted | `[path]` | remove_path → staged Deleted | staged D: `[path]` | entry restored from HEAD |
| unstaged Renamed | `[origPath, path]` | remove old + add new → staged Renamed | staged R: `[origPath, path]` | old restored, new removed → back to unstaged rename |
| staged Added (unborn or not) | — | — | `[path]` | HEAD: reset (entry absent in HEAD → removed); unborn: remove_path |

Conflicted entries: no M3 UI actions (§4.2); backend does not special-case them.

### 2.4 `src-tauri/src/git/commit.rs`

```rust
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommitResult {
    /// Full 40-char hex oid of the new commit.
    pub oid: String,
    /// First line of the cleaned message.
    pub summary: String,
    /// Branch HEAD points at after the commit ("main"); None when detached.
    pub branch: Option<String>,
}

/// Reads user.name / user.email from `cfg` (a repo config snapshot — includes local, global,
/// system levels). Missing or empty value(s) -> AppError::ConfigMissing with a message that
/// NAMES each missing key, e.g.:
///   "git identity not configured: user.email is not set. Run: git config --global user.email \"you@example.com\""
///   (both missing -> name both keys in one message). Never falls back to a default identity.
/// Success -> git2::Signature::now(name, email).
pub fn resolve_signature(cfg: &git2::Config) -> Result<git2::Signature<'static>, AppError>;

/// Blocking. Creates a commit from the current index.
/// Steps (exact order — cheap checks first):
/// 1. open repo (NO_SEARCH); index = repo.index()
/// 2. index.has_conflicts() -> AppError::Git("cannot commit: unresolved conflicts")
/// 3. msg = message.trim(); msg.is_empty() -> AppError::EmptyMessage
///    (whitespace-only rejected; interior newlines/paragraphs preserved)
/// 4. sig = resolve_signature(&repo.config()?.snapshot()?)
/// 5. tree_oid = index.write_tree()
/// 6. head = repo.head() peeled to Commit; UnbornBranch/NotFound -> None
///    - Some(h) && h.tree_id() == tree_oid  -> AppError::NothingToCommit
///    - None && index.is_empty()            -> AppError::NothingToCommit
/// 7. full = format!("{msg}\n"); parents = head as 0/1-elem slice
///    oid = repo.commit(Some("HEAD"), &sig, &sig, &full, &repo.find_tree(tree_oid)?, &parents)
///    (unborn HEAD: git2 creates the branch HEAD symbolically points at — first-commit flow)
/// 8. branch = repo.head().ok().filter(|h| h.is_branch()).and_then(|h| h.shorthand().map(String::from))
///    summary = first line of msg
pub fn create_commit(workdir: &Path, message: &str) -> Result<CommitResult, AppError>;
```

Decision — **empty commits are rejected** (`NothingToCommit`), matching GitKraken's default and
the UI (Commit button is disabled when staged is empty anyway; the backend check guards races).
Message cleanup: `trim()` + exactly one trailing `\n` — matches `git commit` default cleanup for
plain messages (comment-line stripping not needed; no `#`-prefilled template exists in our UI).

### 2.5 Error variants (`error.rs`)

```rust
#[error("commit message is empty")]
EmptyMessage,                    // kind() -> "emptyMessage"
#[error("{0}")]
ConfigMissing(String),           // kind() -> "configMissing"; message = the §2.4 text
#[error("nothing to commit (index matches HEAD)")]
NothingToCommit,                 // kind() -> "nothingToCommit"
```

`message()` for `ConfigMissing(m)` returns `m`; the other two return their static text.

### 2.6 Commands (`commands.rs`) + registration

Same pattern as `get_status`: `_inner` core (unit-testable, no Tauri types), `NoRepo` when nothing
open, clone the `PathBuf`, drop the lock, `tauri::async_runtime::spawn_blocking`, join error →
`AppError::Other`.

```rust
#[tauri::command]
pub async fn stage(state: tauri::State<'_, AppState>, paths: Vec<String>) -> Result<(), AppError>;
#[tauri::command]
pub async fn unstage(state: tauri::State<'_, AppState>, paths: Vec<String>) -> Result<(), AppError>;
#[tauri::command]
pub async fn commit(state: tauri::State<'_, AppState>, message: String) -> Result<CommitResult, AppError>;
```

Register all three in `generate_handler!`. Command surface after M3: `open_repo`, `get_status`,
`get_graph`, `stage`, `unstage`, `commit`. Events: `repo-changed` (unchanged). Channels: none.

### 2.7 Watcher interplay (decision)

Commands do **NOT** emit `repo-changed` themselves. Our index/HEAD writes will trip the notify
watcher (`.git/index`, `.git/refs/...` pass the M1 relevance filter) ~300 ms later — but the
frontend does not wait for it: it **refetches imperatively after every successful mutation**
(§4.4), so the UI updates at command-completion speed. The trailing watcher event triggers one
redundant refetch, absorbed by the existing request-id last-wins guards. Rationale: direct user
actions must not feel debounced; the watcher remains the mechanism for external changes only.

## 3. IPC layer (TypeScript)

`src/ipc/types.ts`:

```ts
export interface CommitResult {
  oid: string;
  summary: string;
  branch: string | null;
}

// AppError kind union becomes:
//   'git' | 'io' | 'other' | 'noRepo' | 'emptyMessage' | 'configMissing' | 'nothingToCommit'

export interface IpcApi {
  // ...existing members unchanged...
  /** Stage paths (worktree-relative, forward slashes — StatusEntry.path strings). Atomic. */
  stage(paths: string[]): Promise<void>;
  /** Unstage paths. Atomic. Safe (worktree never touched). */
  unstage(paths: string[]): Promise<void>;
  /** Create a commit from the index. Rejects with AppError kinds
   *  emptyMessage | configMissing | nothingToCommit | git | noRepo. */
  commit(message: string): Promise<CommitResult>;
}
```

`src/ipc/tauri.ts`:

```ts
stage:   (paths)   => invoke<void>('stage', { paths }),
unstage: (paths)   => invoke<void>('unstage', { paths }),
commit:  (message) => invoke<CommitResult>('commit', { message }),
```

No capability changes (custom commands are always allowed; only plugins need capability entries).

## 4. Frontend

### 4.1 Right-panel layout

`.right-panel` becomes `display:flex; flex-direction:column`: `<StatusPanel>` gets `flex:1;
overflow-y:auto`; `<CommitBox>` is pinned at the bottom with a 1px top border (`--border`),
`--bg-1` background, 12px padding.

### 4.2 StatusPanel interactions

New props (stays presentational — no ipc imports):

```ts
export interface StatusPanelProps {
  snapshot: StatusSnapshot | null;
  loading: boolean;
  error: string | null;
  /** True while any stage/unstage/commit is in flight — disables all action buttons. */
  busy: boolean;
  onStage(paths: string[]): void;
  onUnstage(paths: string[]): void;
}
```

- Helper `entryPaths(e: StatusEntry): string[]` → `e.origPath !== null ? [e.origPath, e.path] :
  [e.path]` (rename expansion, §2.1). Used by every button below.
- **Row buttons — hover-revealed** (decision: matches the GitButler-clean minimal feel; rows stay
  quiet until pointed at). One 20×20 icon button at the row's right edge, `opacity:0` →
  `opacity:1` on `.file-row:hover` AND on `:focus-visible` (keyboard accessible, real `<button>`
  with `aria-label`). Staged rows: `−` ("Unstage <path>") → `onUnstage(entryPaths(e))`.
  Unstaged + untracked rows: `+` ("Stage <path>") → `onStage(entryPaths(e))`. Conflicted rows:
  **no button** (conflict UX is post-M6).
- **Section-header actions** — small text buttons (11px, `--text-3`, hover `--text-1`) right-aligned
  in the header row: Staged → "Unstage all" (`onUnstage(flat entryPaths of staged)`); Unstaged →
  "Stage all"; Untracked → "Stage all". Hidden when the section is empty. Each section's button
  sends only that section's paths (one atomic call).
- All buttons `disabled` when `busy || loading`.

### 4.3 CommitBox (`src/components/CommitBox.tsx`)

```ts
export interface CommitBoxProps {
  stagedCount: number;
  busy: boolean;                                   // app-wide mutation in flight
  /** Resolves on success (box clears its textarea); rejects with AppError on failure. */
  onCommit(message: string): Promise<void>;
}
```

- Local state: `message: string`, `submitting: boolean`, `error: { kind: string; text: string } | null`.
- Multiline `<textarea>` (3 rows, resize vertical up to ~6), placeholder `"Commit message"`,
  mono NOT required (UI font), `--bg-2` background per ui-reference inputs.
- **72-char summary guidance:** when the first line exceeds 72 chars, show `"<n>/72"` right-aligned
  below the textarea in `--warning`; otherwise nothing (no counter noise).
- Primary button `Commit` (accent, full width). Label `Committing…` while submitting.
  `disabled` when `stagedCount === 0 || message.trim() === '' || busy || submitting`.
- **Ctrl+Enter** in the textarea triggers commit when the button would be enabled.
- Submit: `try { await onCommit(message); setMessage(''); setError(null) } catch (e) { setError(...) }`.
  Error rendering: inline `--danger` banner (ui-reference §8) above the button, dismissible.
  `kind === 'configMissing'`: prefix the banner with `"Set your Git identity: "` + backend message
  (which already names the missing key(s) and the `git config` commands) — this is the distinctive
  identity-error surface. All other kinds: show `message` as-is.
- **Pessimistic UI throughout (decision):** no optimistic list moves; controls disable in flight,
  state comes back via refetch. Simpler, and refetch latency is one status scan (~ms).

### 4.4 App wiring (`App.tsx`)

- New state: `mutating: boolean` (single flag for stage/unstage/commit; passed as `busy` to both
  components and OR-ed into the refresh button's `disabled`).
- `handleStage(paths)` / `handleUnstage(paths)`:
  `setMutating(true); try { await ipc.stage/unstage(paths); await refetchStatus(); }
   catch (e) { setStatusError(errorMessage(e)); } finally { setMutating(false); }`
  (stage/unstage cannot change the graph — no graph refetch.)
- `handleCommit(message)`: `setMutating(true); try { await ipc.commit(message);` then the
  post-commit refresh: `const info = await ipc.openRepo(repoPath)` (updates the header HEAD oid
  and self-heals the watcher, same as handleRefresh), `setRepo(info)`, then
  `await Promise.all([refetchStatus(), refetchGraph()])`. `} finally { setMutating(false); }`
  Errors: **rethrow** so CommitBox displays them (App does not surface commit errors itself);
  errors from the post-commit refresh (commit already succeeded) are caught and routed to
  `setStatusError`, not rethrown.
- No new subscriptions; the existing repo-changed/focus refetches stay as-is (§2.7).

## 5. Mock IPC (`src/ipc/mock.ts`) — stateful

The mock must round-trip visibly in the browser harness:

- Module state: `let mockStatus = structuredClone(INITIAL_STATUS)` (the current fixture becomes
  `INITIAL_STATUS`), `let mockHeadOid = MOCK_OID`, `let openedPath: string | null = null`.
- `openRepo(path)`: resets `mockStatus`/`mockHeadOid` **only when `path !== openedPath`** (so the
  post-commit `openRepo` call does not resurrect staged files); returns `head.oid = mockHeadOid`.
- `getStatus()`: `structuredClone(mockStatus)` (unchanged pattern).
- `stage(paths)`: for each entry in `unstaged`/`untracked` whose `path` OR `origPath` is in
  `paths`: remove it and upsert into `staged` (dedupe by `path`), mapping `untracked → 'added'`,
  everything else keeps its status. `delay(150)` first, like the rest.
- `unstage(paths)`: matching `staged` entries move back — `'added' → untracked ('untracked')`,
  all others → `unstaged` (same status, `origPath` preserved). Keep each list sorted by `path`.
- `commit(message)`: validations mirror the backend —
  `message.trim() === ''` → throw `{ kind: 'emptyMessage', message: 'commit message is empty' }`;
  `mockStatus.staged.length === 0` → throw `{ kind: 'nothingToCommit', ... }`;
  URL `?fixture=noconfig` → throw `{ kind: 'configMissing', message: 'git identity not configured:
  user.name and user.email are not set. Run: git config --global user.name "Your Name" ...' }`.
  Success: `mockStatus.staged = []`, `mockHeadOid` = new random 40-hex string, return
  `{ oid: mockHeadOid, summary: first line, branch: 'main' }`.
- **Graph is NOT updated by the mock commit** (decision): the graph fixtures are generator
  functions; prepending a synthetic row buys little harness value for real coupling cost. The
  visible harness proof of commit is: staged section empties, header oid changes, commit box
  clears. (Optional nicety, non-blocking: leave a `// TODO(polish)` note.)

## 6. Testing (contract for tester)

### 6.0 Scratch location — HARD RULE: everything on D:

C: is critically full. **No test may create temp files outside D:.** Shared helper (put it in a
small `#[cfg(test)]`-reachable module, e.g. `src-tauri/src/testutil.rs` behind `#[cfg(test)]` +
re-exported for integration tests via a tiny `tests/common/mod.rs`):

```rust
/// All scratch repos live under D:\Temp\bonsai-scratch (created if absent).
pub fn scratch_dir() -> tempfile::TempDir {
    let root = std::path::Path::new("D:\\Temp\\bonsai-scratch");
    std::fs::create_dir_all(root).expect("create scratch root");
    tempfile::Builder::new().prefix("bonsai-").tempdir_in(root).expect("scratch dir")
}
```

Every new M3 test uses `scratch_dir()` — never `TempDir::new()`. Additionally, run test sessions
with `TMP`/`TEMP` set to `D:\Temp` so pre-existing M0–M2 tests (which use `TempDir::new()`, honoring
TMP/TEMP) also land on D:. Orchestrator: export those env vars in the shell that runs `cargo test`.

### 6.1 CLI-oracle staging tests (`src-tauri/src/git/stage.rs` `#[cfg(test)]` or `tests/stage_cli.rs`)

Pattern — **twin repos**: build two identical scratch repos with the same script (files + CLI
commits; set repo-local `user.name`/`user.email` via `git -C <dir> config`). Apply our git2 op to
repo A, the equivalent CLI op to repo B, then assert
`git status --porcelain=v1 -z --untracked-files=all` output is **byte-identical** between A and B
(sort NUL-split records before comparing to dodge ordering differences). CLI equivalents:
stage → `git add -- <paths>` / `git add -A -- <path>` for deletions (or `git rm --cached` where
noted); unstage → `git restore --staged -- <paths>`.

Scenarios (one test each):
1. stage untracked file (incl. one nested in a new directory).
2. stage modified tracked file.
3. stage deleted file (fs-deleted, not `git rm`) → our `remove_path` branch vs CLI `git add -A -- <path>`.
4. stage a worktree rename: delete+recreate as rename (`git mv` on twin B is fine for the CLI side;
   for A do the fs rename then `stage_paths(&[old, new])`) → porcelain shows `R ` staged rename in both.
5. batch: stage `[untracked, modified, deleted]` in ONE call → all three staged.
6. atomicity: `stage_paths(&[valid_path, "../escape"])` → `Err(AppError::Other)`, and porcelain
   output UNCHANGED from before the call.
7. unstage staged modification → parity with `git restore --staged`.
8. unstage staged deletion → entry restored in index (porcelain ` D` again).
9. unstage staged rename (both paths) → back to worktree-rename state.
10. unborn repo: stage file → porcelain `A `; unstage it → porcelain `??` (remove_path branch).
11. empty paths vec → `Ok(())`, no change.

### 6.2 CLI-oracle commit tests (`src-tauri/src/git/commit.rs` tests or `tests/commit_cli.rs`)

Twin repos again; ours commits via `create_commit`, twin via `git commit -m <same message>`
(identity: identical repo-local user.name/user.email in both). Compare `git cat-file commit HEAD`
**fields**, not oids (timestamps differ): tree oid equal, parent line(s) equal (both repos built
identically so parent oids match — build base history with fixed `GIT_AUTHOR_DATE`/
`GIT_COMMITTER_DATE` env on the CLI commits in BOTH twins so base oids are identical), author name
+ email, committer name + email, full message body (ours must equal CLI's, i.e. trimmed +
trailing newline).

Scenarios:
1. normal commit on existing history: fields match; `CommitResult.oid` = repo A's `rev-parse HEAD`;
   `summary` = first line; `branch = Some("main")` (or init default — set `init.defaultBranch=main`
   locally in fixtures).
2. multi-line message (subject + blank + body) → body preserved verbatim + one trailing `\n`.
3. unborn-HEAD first commit → branch created, `parents` empty in cat-file, `branch = Some(...)`,
   subsequent `read_status` clean.
4. empty / whitespace-only message → `Err(AppError::EmptyMessage)`, no commit created
   (`git rev-parse HEAD` unchanged / still unborn).
5. nothing staged (clean repo) → `Err(AppError::NothingToCommit)`; also on unborn repo with empty
   index.
6. detached HEAD commit → succeeds, `branch = None`, HEAD advanced.
7. commit-then-status round-trip: stage file → `create_commit` → `read_status` all lists empty.

### 6.3 Missing-config isolation — exactly how

Two layers (decision):

- **Unit (primary):** `resolve_signature` takes `&git2::Config`, so tests build a fully isolated
  config with `git2::Config::open(&scratch_file)` containing (a) neither key, (b) only user.name,
  (c) only user.email, (d) both. Assert (a–c) → `AppError::ConfigMissing` whose message **contains
  each missing key name**; (d) → signature with exact name/email. No env vars, no global config
  visible, race-free.
- **Integration (command path):** separate test binary `src-tauri/tests/commit_noconfig.rs` —
  cargo runs each integration binary in its **own process**, so env mutation cannot race other
  tests. At the top of the test: `std::env::set_var("GIT_CONFIG_GLOBAL", <empty file in
  scratch_dir>)`, same for `GIT_CONFIG_SYSTEM`, and `GIT_CONFIG_NOSYSTEM=1` (libgit2 ≥ 1.5 honors
  these). Init a repo WITHOUT local identity, stage a file, call `create_commit` → assert
  `ConfigMissing`. Then set repo-local identity via git2 (`repo.config().set_str(...)` — not the
  CLI, which would need the same env) and assert commit succeeds.

### 6.4 Frontend smoke (browser harness, `VITE_MOCK_IPC=1 pnpm dev`)

1. Hover an unstaged row → `+` appears; click → row moves to Staged after ~150 ms; buttons
   disabled during flight.
2. "Stage all" on Untracked → both untracked files staged (`untracked → A`).
3. "Unstage all" on Staged → staged empties; `src/app.rs` (added) lands in Untracked; rename entry
   returns to Unstaged as one entry.
4. Commit button disabled with empty message or empty staged; type a message with a > 72-char
   first line → counter appears in `--warning`.
5. Stage files, type message, **Ctrl+Enter** → staged section empties, textarea clears, header
   HEAD oid changes.
6. Commit with staged empty (force via devtools or by unstaging first while message present) →
   button disabled (no call).
7. `?fixture=noconfig` → commit shows the identity error banner naming user.name/user.email;
   dismissible; staged list unchanged.
8. No `@tauri-apps/*` module executed; no console errors.

## 7. Sub-increment split for senior-dev

- **M3a — Rust commands + CLI-oracle tests.** `error.rs` variants, `git/stage.rs`,
  `git/commit.rs`, commands + registration, `scratch_dir()` helper, tests §6.1–§6.3.
  Gate: `cargo test` green, `cargo clippy -- -D warnings` clean, all scratch dirs under
  `D:\Temp\bonsai-scratch`.
- **M3b — Frontend + IPC/mock.** `types.ts`/`tauri.ts`/`mock.ts` (stateful), StatusPanel actions,
  `CommitBox.tsx`, `App.tsx` wiring, styles. Gate: `pnpm build` green; §6.4 smoke passes in the
  harness.

## 8. Acceptance criteria

AI gate:
- All §6.1–§6.3 Rust tests pass (CLI as oracle); `cargo check`/`clippy`/`test`, `pnpm build` green.
- Browser harness passes the §6.4 smoke list (screenshots of before/after stage, commit flow,
  noconfig error).

USER CHECKPOINT (never self-declared): in the native app on a scratch repo — stage a file, unstage
it, stage again, commit with a message; the file lists update immediately (not after a debounce
pause), the header HEAD advances, the new commit appears at the top of the graph with the branch
pill, and `git log`/`git status` in a terminal agree.

## 9. Ambiguities resolved here (flag to orchestrator if disagreed)

- **All-or-nothing batch semantics** with single index write, over best-effort per-path errors
  (§2.2) — atomicity is free with git2's in-memory index; partial-failure UI isn't worth building
  for race-only errors.
- **Rename expansion in the frontend** (`[origPath, path]`) instead of a structured
  `{path, origPath}` wire type — keeps the command surface `Vec<String>` and the backend
  rename-agnostic (present→add / missing→remove covers both sides).
- **Unborn unstage via `index.remove_path`** rather than relying on `reset_default(None, ..)`
  semantics — explicit, binding-version-proof, identical result.
- **Empty commits rejected** (`nothingToCommit`), GitKraken-style; allow-empty is not a v1 need.
- **Message cleanup = trim + one trailing `\n`**, no comment stripping (we never prefill `#` lines).
- **Commands don't emit `repo-changed`; frontend refetches imperatively after each mutation**
  (§2.7) — the debounced watcher event still arrives and is absorbed by request-id guards.
- **Post-commit refresh reuses `openRepo(repoPath)`** (header HEAD + watcher self-heal) — same
  trade-off as M1's refresh-button decision; add a lighter `get_repo_info` only if latency shows.
- **Hover-revealed row buttons** (+ focus-visible for keyboard) over always-visible — quieter rows,
  per the GitButler-clean directive; section-level "all" buttons carry the bulk workflow.
- **Pessimistic UI** (disable during flight, refetch after) over optimistic moves — correctness for
  free; local status scans make latency negligible.
- **Mock does not prepend a graph row on commit** — graph fixtures are generated; harness proof of
  commit is the emptied staged list + changed header oid + cleared textarea.
- **`CommitResult = { oid, summary, branch }`** — enough for a future toast; nothing speculative.
- **Conflicted rows get no stage/unstage buttons in M3** — conflict UX arrives with merges (post-M6).
- **Scratch-on-D: enforcement** via `scratch_dir()` helper + `TMP`/`TEMP=D:\Temp` for legacy tests —
  hard user mandate, non-negotiable.
