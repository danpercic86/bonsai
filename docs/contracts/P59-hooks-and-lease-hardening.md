# P59 — Hooks execution + force-push-lease hardening

Two correctness/trust fixes, shippable as independent sub-increments:
- **(a) P59a — HOOKS.** git2 mutations silently bypass `pre-commit` / `commit-msg` / `post-commit`
  (and `pre-push`). Run them around the relevant ops, with a **per-repo opt-out** and a **failing hook
  that BLOCKS with its output shown** — never a silent success. (AI-agent era: lint/format/secret-scan
  hooks matter more than ever.)
- **(b) P59b — LEASE.** `force_push_with_lease` today is a client-side ls-remote-then-push
  compare-and-swap (a TOCTOU window). Convert to git's own atomic `--force-with-lease` via the git binary.

References read (current state, verified — not guessed):
`crates/bonsai-core/src/git/commit.rs` (`create_commit` L75 → `repo.commit` L133; `amend_commit` L154;
CRLF-normalize + guards; where hooks wrap),
`crates/bonsai-core/src/git/merge.rs` (`commit_merge` — another commit site),
`crates/bonsai-core/src/git/remote.rs` (`push_current`; `force_push_with_lease` L681 — the current
client-side lease: `connect_auth(Push)` + `remote.list()` + compare vs the remote-tracking baseline
L785-798, then `+`-refspec `remote.push` L819; `credential_fill` L165 = never-prompt git shell-out),
`crates/bonsai-core/src/git/search.rs` (`GitRunner`/`SpawnGitRunner` seam),
`crates/bonsai-core/src/git/maintenance.rs` (best-effort git shell-out precedent),
`crates/bonsai-core/src/error.rs` (`AppError` variants; `PushRejected`/`AuthFailed`/`NetworkError`
already exist — reused by the lease),
`src-tauri/src/commands/staging.rs` (`commit`/`commit_amend`), `src-tauri/src/commands/remote.rs`
(`push`/`force_push`), `src/components/WorkspaceDialogs.tsx` (`pendingForcePush`/`doForcePush` L88-91 —
force-push confirm), `docs/contracts/P37-force-push-with-lease.md` (the lease origin + CLI-oracle
harness). `git hook run` confirmed: Git ≥2.36; `git hook run [--ignore-missing] [--to-stdin=<path>]
<name> [-- <args>]`. CORRECTION (F-A4-1, audit 2026-08-09): a missing hook is NOT a no-op —
bare `git hook run <name>` exits 1 with "cannot find a hook named <name>" (verified git 2.51),
so a Husky-style repo (`core.hooksPath` set) missing one of the hooks had every
commit/amend/merge/push blocked. Fix: Bonsai always passes `--ignore-missing` (same ≥2.36
floor as the subcommand), which makes absent — and, on unix, present-but-non-executable —
hooks a clean exit-0 no-op, plus a `core.hooksPath`-aware existence pre-check in `plan_hook`.

**P59a: +1 `AppError` variant `HookRejected`. No new command** (hooks run inside existing
commit/amend/merge/push; the per-repo toggle rides existing `read_config`/`set_config`; a `skip_hooks`
param is added to those commands). **P59b: no IPC change at all** — `force_push` signature and the mock
`?remote=leasefail` seam are unchanged; internals + tests only. Open questions in §11.

---

# Part A — P59a: hooks execution

## A0. Key decisions (with rationale)

**A-D1 — Run hooks via `git hook run <name> [--to-stdin=<file>] [-- <args>]` (Git ≥2.36). RECOMMENDED.**
Reimplementing hook execution in Rust means re-deriving hook discovery (`core.hooksPath`, worktree
gitdir), the executable-bit check, and — the real trap — **Windows shell-script execution** (hooks are
`#!/bin/sh` scripts git runs through its bundled `sh`, not native executables). `git hook run` is the API
git added expressly for tools that wrap git; it does ALL of that, forwards args + stdin, propagates the
exit code, and no-ops (exit 0) when the hook is absent. Consistent with the project's credential /
search-pickaxe / commit-graph shell-outs. Floor = Git 2.36 (2022); the git binary is already a hard dep.
- **Fallback (A-D6):** if `git hook run` is unavailable (git < 2.36) or errors as an unknown subcommand,
  we must NOT silently bypass a blocking hook (trust invariant). Recommend: detect once; if hook files
  exist under the resolved hooks dir, surface a one-time `Git` error ("hook execution needs Git ≥ 2.36")
  rather than committing unverified; if NO hook files exist, proceed (nothing to run). Confirm (§11 OQ-A1).

**A-D2 — Hooks run INSIDE the existing ops; no new command.** Commit flow order = git's:
`pre-commit` → (reload index) → `commit-msg` (may edit the message file) → create the commit
(git2 unsigned OR P58 `commit-tree -S` signed) → `post-commit` (failure ignored, output surfaced as info).
`pre-push` runs before the libgit2 push. This wraps whichever commit mechanism P58 chose; the hook run is
NOT part of the signed content.

**A-D3 — Per-repo opt-out via git config `bonsai.runHooks` (bool, default TRUE) + a per-invocation
`skip_hooks` override (≡ `--no-verify`). RECOMMENDED.** git's own default runs hooks; disabling silently
would surprise. `bonsai.runHooks` lives in the repo's `.git/config` (git-native, per-repo, survives) and
is read/written through the EXISTING `read_config`/`set_config` (no new command). `skip_hooks: bool` on
the commit/push commands is the per-action escape hatch (a "Skip hooks" checkbox), mirroring
`git commit --no-verify`.

**A-D4 — NEW `AppError::HookRejected(String)` carrying `"<hook> hook failed:\n<combined stdout+stderr>"`.
RECOMMENDED.** P37/P50 avoided new variants, but here the UX is different and load-bearing: the frontend
must render the hook's OWN output (why it failed) in a scrollable block, distinctly from a generic git
error. A dedicated `kind: 'hookRejected'` lets the UI do that. Serialize like the other variants
(`{ kind:'hookRejected', message }`).

**A-D5 — Index/message re-read around the hooks.** `pre-commit` runs BEFORE `index.write_tree()`; after
it, `index.read(true)` to reload from disk so a hook that re-stages (formatter) is included. `commit-msg`
receives a temp message file as its arg; re-read the (possibly rewritten) file after. This matches git.

## A1. Module boundaries / files

**New**
- `crates/bonsai-core/src/git/exec.rs` — `GitExec` trait + `SpawnGitExec` (shared with P58; introduce in
  whichever ships first). `exec(args, cwd, stdin: Option<&[u8]>, env) -> Result<GitOutput, AppError>`;
  `GitOutput { success, code, stdout, stderr }`; never-prompt env + `CREATE_NO_WINDOW`.
- `crates/bonsai-core/src/git/hooks.rs` — `HookName` enum; `hooks_enabled(cfg, skip) -> bool`
  (reads `bonsai.runHooks`); `run_hook(exec, workdir, hook, args, stdin) -> Result<(), AppError>`
  (non-zero ⇒ `HookRejected`; missing ⇒ Ok); `run_hook_nonblocking` (post-commit: capture output, never
  block); pure arg/stdin builders; CLI oracle.
- `src/components/HookOutputDialog.tsx` — modal showing a `HookRejected` message (hook name + preformatted
  output + a "Commit anyway (skip hooks)" retry that re-runs with `skipHooks:true`).

**Edited**
- `crates/bonsai-core/src/git/commit.rs` — orchestrate `pre-commit`/`commit-msg`/`post-commit` in
  `create_commit`/`amend_commit`; both gain `skip_hooks: bool`.
- `crates/bonsai-core/src/git/merge.rs` — `commit_merge` runs the same commit hooks (+ `skip_hooks`).
- `crates/bonsai-core/src/git/remote.rs` — `pre-push` in `push_current` (+ `force_push_with_lease`);
  `skip_hooks` param (P59a-2).
- `crates/bonsai-core/src/git/mod.rs` — `pub mod exec; pub mod hooks;`.
- `crates/bonsai-core/src/error.rs` — `HookRejected(String)` + its `kind` wire mapping.
- `src-tauri/src/commands/staging.rs` / `merge.rs` / `remote.rs` — `skip_hooks: bool` (serde
  `#[serde(default)]`) threaded to core.
- `src/ipc/types.ts` — `AppError` union gains `'hookRejected'`; `commit`/`commitAmend`/`commitMerge`/
  `push`/`forcePush` gain optional `skipHooks?: boolean`.
- `src/ipc/tauri.ts` — pass `skipHooks`.
- `src/ipc/mock.ts` (+ a small handler) — model a failing hook (see §A5).
- `src/components/CommitBox.tsx` — a "Skip hooks" checkbox; on `hookRejected` open `HookOutputDialog`.
- `src/components/WorkspaceDialogs.tsx` — host `HookOutputDialog`.
- `src/components/SettingsPanel.tsx` (or a repo-settings section) — a "Run git hooks" toggle bound to
  `bonsai.runHooks` via `read_config`/`set_config` (no new command).

## A2. Backend core — `crates/bonsai-core/src/git/hooks.rs`

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HookName { PreCommit, CommitMsg, PostCommit, PrePush }
impl HookName { pub fn as_str(&self) -> &'static str /* "pre-commit" | … */ }

/// Effective hook toggle: `!skip && bonsai.runHooks (default true)`. A missing
/// key => true (git's default). Read from the repo's merged config.
pub fn hooks_enabled(cfg: &git2::Config, skip: bool) -> bool;

/// Run one BLOCKING hook via `git hook run <name> [-- <args>]`, optionally
/// streaming `stdin` (written to a temp file passed as `--to-stdin`). Exit 0 or
/// hook-absent => Ok(()). Non-zero => Err(HookRejected("<name> hook failed:\n"
/// + stdout + stderr)). Spawn failure => Err(Git). NEVER panics.
pub fn run_hook(exec: &dyn GitExec, workdir: &Path, hook: HookName,
                args: &[String], stdin: Option<&[u8]>) -> Result<(), AppError>;

/// post-commit: run it, IGNORE a non-zero exit (git semantics), return the
/// captured output for optional info-surfacing. Never Err on a hook failure.
pub fn run_hook_nonblocking(exec: &dyn GitExec, workdir: &Path, hook: HookName,
                            args: &[String]) -> HookRunInfo; // { ran: bool, success: bool, output: String }

// pure, unit-tested:
fn build_hook_run_args(hook: HookName, args: &[String], stdin_path: Option<&Path>) -> Vec<String>;
// => ["hook","run", <name>, ("--to-stdin=<path>")?, ("--")?, <args>…]
```

`GIT_DIR`/cwd: `SpawnGitExec` sets `current_dir(workdir)`; git resolves `.git` (incl. worktree gitdir
files) itself. `git hook run` honors `core.hooksPath`. No env plumbing beyond never-prompt.

## A3. Commit orchestration — `commit.rs` (normative)

```
create_commit(workdir, message, sign, skip_hooks):
    repo = open; guards (bisect, clean-state, conflicts) as today
    cfg = repo.config().snapshot()
    hooks = hooks_enabled(&cfg, skip_hooks)
    if hooks: run_hook(exec, workdir, PreCommit, &[], None)?      # non-zero => HookRejected, ABORT
    index = repo.index(); if hooks { index.read(true)? }          # pick up hook re-staging
    if index.has_conflicts(): Err(...)                            # (unchanged)
    normalized/trim msg; if empty => EmptyMessage
    if hooks:
        write `full` (msg + "\n") to a temp msg file (e.g. .git/COMMIT_EDITMSG via repo.path())
        run_hook(exec, workdir, CommitMsg, &[msg_file.to_string()], None)?   # may edit the file
        msg = re-read + re-normalize + trim; if empty => EmptyMessage
    resolve sig; tree = index.write_tree()
    NothingToCommit guard (unchanged)
    oid = if resolve_signing(sign).sign { signing::create_signed_commit(...) } else { repo.commit(...) }
    if hooks: let _ = run_hook_nonblocking(exec, workdir, PostCommit, &[])   # never blocks
    Ok(CommitResult { oid, summary, branch })
```
`amend_commit` mirrors this (same three hooks; git runs commit hooks on amend). `commit_merge` runs
`pre-commit`/`commit-msg`/`post-commit` around the merge-commit creation. **A blocking hook's `Err`
propagates up as `HookRejected` — no partial commit, no ref move.** `resolve_signature`'s `ConfigMissing`
still fires before any write.

## A4. Pre-push — `remote.rs` (P59a-2)

Our push is libgit2, so run `pre-push` ourselves BEFORE it. Args = `<remote-name> <remote-url>`; stdin =
one line per pushed ref: `<local-ref> SP <local-oid> SP <remote-ref> SP <remote-oid> LF`
(`<remote-oid>` = the baseline/expected, or 40 zeros if the remote ref is new). Non-zero ⇒ `HookRejected`,
abort (do NOT call `remote.push`). Wire into `push_current` and `force_push_with_lease`. Scope note: this
is the lower-value slice; land P59a-core (commit hooks) first, pre-push as P59a-2.

## A5. Mock

- `commit`/`commitAmend`/`commitMerge` accept `skipHooks?`. A `?hooks=fail` query (RepoState flag) OR a
  `#hookfail` message sentinel ⇒ throw `{ kind:'hookRejected', message: 'pre-commit hook failed:\n<mock
  lint output>' }` UNLESS `skipHooks===true` or `bonsai.runHooks` is false in the mock config. This drives
  `HookOutputDialog` + the "Commit anyway" retry in the harness.
- The "Run git hooks" toggle round-trips through the existing mock `read_config`/`set_config` on key
  `bonsai.runHooks` (already generic section.key). `push`/`forcePush` accept `skipHooks?` (ignored by the
  mock beyond plumbing).
- Add `'hookRejected'` to the mock `AppError` kind union.

## A6. CLI-oracle test plan (`#[cfg(test)]` in `hooks.rs`) — FULLY AI-gate-testable (no key material)

Guard `have_git()` + a Git-version ≥ 2.36 check (skip otherwise). `TMP`/`TEMP=D:\Temp` (Windows). Hook
scripts are `#!/bin/sh` with **LF** endings (`.gitattributes` enforces) + the executable bit on Unix;
`git hook run` handles the Windows shell path — this is the whole point of A-D1, so a Windows CI run
exercises it. Pure units: `build_hook_run_args` exact vecs (with/without `--to-stdin`, args after `--`);
`hooks_enabled` truth table (`skip`, `bonsai.runHooks` true/false/unset). Oracle (real hooks):
- **pre-commit fail blocks:** a `.git/hooks/pre-commit` that `echo "lint failed" >&2; exit 1` ⇒
  `create_commit` ⇒ `Err(HookRejected)` whose message contains "lint failed"; `git rev-parse HEAD`
  UNCHANGED (no commit).
- **pre-commit pass allows:** `exit 0` ⇒ commit succeeds.
- **commit-msg rewrites:** a hook that appends `\nSigned-off-by: X` to `$1` ⇒ the committed message
  contains it (`git log -1 --format=%B`).
- **commit-msg fail blocks:** non-zero ⇒ `HookRejected`, no commit.
- **post-commit non-blocking:** a `post-commit` that `exit 1` ⇒ the commit STILL succeeds (`HEAD` moved);
  `HookRunInfo.success == false` captured.
- **core.hooksPath:** point it at a sibling dir whose `pre-commit` fails ⇒ blocks (proves discovery is
  git's, not a hardcoded `.git/hooks`).
- **opt-out:** `bonsai.runHooks=false` (and separately `skip_hooks=true`) ⇒ a failing `pre-commit` is NOT
  run; commit succeeds.
- **pre-push (P59a-2):** a `pre-push` that inspects stdin and exits 1 ⇒ push to a local bare remote is
  aborted; remote ref UNCHANGED.
- **re-stage:** a `pre-commit` that writes+`git add`s a file ⇒ the reloaded index includes it in the tree.

---

# Part B — P59b: force-push-lease hardening

## B0. Key decisions (with rationale)

**B-D1 — git2 0.21 CANNOT do an atomic server-side lease; the git binary can. RECOMMENDED FINDING.**
libgit2's push does not implement the protocol-v2 `--force-with-lease` expected-old-value compare-and-swap
sent to the server. `RemoteCallbacks::push_negotiation` (where exposed) is only an inspect/veto
notification of the updates libgit2 already computed locally — it does NOT transmit an expected-old-oid,
so it cannot close the race. Therefore the current `force_push_with_lease` (our own `remote.list()` then
`remote.push`) inherently has a TOCTOU window (P37 flagged this). The ONLY way to get git's atomic lease
is `git push --force-with-lease` via the git binary.

**B-D2 — REPLACE the client-side compare with `git push --force-with-lease=<ref>:<expected>
[--force-if-includes]`. FIRM RECOMMENDATION.** Keep the git2-based resolution of branch / upstream remote
/ `remote_ref` / baseline `expected` oid (from the remote-tracking ref) EXACTLY as today; change only the
push MECHANISM:
```
git push --force-with-lease=<remote_ref>:<expected_hex> [--force-if-includes] \
         <remote_name> +refs/heads/<branch>:<remote_ref>
```
run via `SpawnGitExec` (never-prompt: `GIT_TERMINAL_PROMPT=0`, `-c core.askpass=`, `env_remove` askpass,
`CREATE_NO_WINDOW`). git performs the expected-old-value check at push-negotiation time (atomic on capable
servers), collapsing our two-step ls-remote→push into git's single negotiated push and eliminating OUR
window. Keep the `expected == local_tip` ⇒ `UpToDate` short-circuit in git2 before shelling.
- **Trade-off (reverses P37 D1):** this ONE op no longer reuses the in-process P35 credential CACHE; it
  uses git's OWN credential helper — the SAME helper `credential_fill` already shells for reads, under the
  same never-prompt policy. For a rare, deliberate force-push, relying on the configured helper is
  acceptable and is the price of the atomic guarantee. `--force-if-includes` (Git ≥2.30) additionally
  guards the "we rewrote without having seen the remote's tip" case — recommend include (§11 OQ-B1).
- **Alternative (documented, NOT recommended):** keep libgit2 + the client-side compare and simply DOCUMENT
  the residual TOCTOU (status quo). Rejected — hardening is the milestone's point.

**B-D3 — Result mapping, no new `AppError`.** exit 0 ⇒ `Pushed { set_upstream:false }`; stderr matching
`stale info` / `[rejected]` / `force-with-lease` / `remote ref updated since checkout` ⇒
`PushRejected` (lease failed — reuse `lease_moved_msg`); auth patterns (`Authentication failed`,
`could not read Username`, `terminal prompts disabled`) ⇒ `AuthFailed`; connect/DNS/TLS ⇒ `NetworkError`;
else ⇒ `Git(stderr_tail)`. Keep the existing `NoUpstream`/no-baseline `PushRejected` pre-checks.

## B1. Module boundaries / files
- `crates/bonsai-core/src/git/remote.rs` — rewrite the BODY of `force_push_with_lease` from
  `connect_auth`+`list`+`push` (L775-822) to the git-binary push; add `runner: &dyn GitExec` param
  (mirrors `search_commits`), plus pure `build_force_push_args` + `classify_push_stderr`.
- `src-tauri/src/commands/remote.rs` — `force_push` passes `&SpawnGitExec`.
- Frontend: **no change required**; optionally tweak the `WorkspaceDialogs` confirm copy to say the lease
  is now checked atomically by git. Mock `?remote=leasefail` seam UNCHANGED.

## B2. Rust — normative
```rust
/// Now: git-binary --force-with-lease (atomic). Signature gains `runner`.
pub fn force_push_with_lease(workdir: &Path, runner: &dyn GitExec) -> Result<PushResult, AppError>;

fn build_force_push_args(remote: &str, branch: &str, remote_ref: &str, expected_hex: &str) -> Vec<String>;
// ["push", format!("--force-with-lease={remote_ref}:{expected_hex}"), "--force-if-includes",
//  remote, format!("+refs/heads/{branch}:{remote_ref}")]
fn classify_push_stderr(stderr: &str) -> AppError; // PushRejected | AuthFailed | NetworkError | Git
```
Algorithm: keep L682-760 (open/head/branch/upstream/baseline/`UpToDate` short-circuit) unchanged. Then:
`out = runner.exec(&build_force_push_args(...), workdir, None, &[])?`; `if out.success { Ok(Pushed{
remote, branch, set_upstream:false }) } else { Err(classify_push_stderr(&out.stderr)) }`. Drop the
`connect_auth`/`list`/`RemoteCallbacks`/`+`-refspec `remote.push` block. `expected_hex` = the baseline oid
we already resolved.

## B3. CLI-oracle test plan (`tests/force_push_cli.rs` — extend P37's) — AI-gate-testable
Local bare `origin` + `work` clone (P37's `init_origin_and_clone`), `file://` remote (no creds → hermetic).
Guard `have_git()`; reset `credential.helper` empty. Pure: `build_force_push_args` exact vec;
`classify_push_stderr` maps representative stderr strings to the right variants.
- **A. Lease refuses (git's atomic check):** a 2nd clone advances `origin/main` to Y; in `work`
  (remote-tracking still X) rewrite `main`→Z; `force_push_with_lease` ⇒ `Err(PushRejected)` (message from
  git's stale-info stderr); `git --git-dir=origin.git rev-parse main` still Y (UNCHANGED).
- **B. Lease succeeds:** no third-party push; amend `main`→Z; ⇒ `Ok(Pushed{set_upstream:false})`;
  origin `main` == Z.
- **C. Up-to-date:** baseline == local tip ⇒ `Ok(UpToDate)`, no mutation, no git spawn (assert via a fake
  `GitExec` that panics if `exec` is called).
- **D. No upstream / E. No baseline:** unchanged from P37 (`NoUpstream` / `PushRejected` "Fetch first"),
  asserted before any push.

---

## 10. Sub-increments

- **P59a — commit hooks (core).** `exec.rs` (if not from P58); `hooks.rs` (`run_hook`,
  `run_hook_nonblocking`, `hooks_enabled`, pure builders, oracle); `commit.rs`/`merge.rs` orchestration +
  `skip_hooks`; `error.rs` `HookRejected`; commands `skip_hooks`; `types.ts`/`tauri.ts`/`mock`;
  `HookOutputDialog` + `CommitBox` "Skip hooks" + the "Run git hooks" settings toggle.
  **Acceptance:** §A6 oracle green (fail blocks with output; pass allows; commit-msg rewrites; post-commit
  non-blocking; hooksPath; opt-out; re-stage); `clippy`/`tsc`/`build` clean. Harness: `?hooks=fail` →
  commit shows `HookOutputDialog` with the output; "Commit anyway" retries with `skipHooks` and succeeds;
  the settings toggle round-trips `bonsai.runHooks`.
- **P59a-2 — pre-push (optional).** `pre-push` in `push_current` + `force_push_with_lease`; stdin
  synthesis; oracle A.pre-push. Ships after core if time-boxed.
- **P59b — lease hardening.** `force_push_with_lease` rewrite + `runner` param + pure builders/classifier;
  `force_push` command passes `SpawnGitExec`; extend `force_push_cli.rs`. No IPC/mock/UI change (optional
  confirm-copy tweak). **Acceptance:** §B3 oracle green (atomic refuse A, succeed B, up-to-date C no-spawn,
  D/E); `clippy` clean.

(P59a and P59b are order-independent. P59a-core is the higher-value trust fix; recommend it first.)

---

## 11. Acceptance criteria (summary) + invariants

**AI gate:** all oracle + pure tests green (`cargo test -p bonsai-core hooks` / `force_push_cli`);
`clippy`/`tsc`/`build` clean; no new command (count unchanged); harness proves the `HookRejected` dialog +
"Commit anyway" + `bonsai.runHooks` toggle, and (P59b) the lease refuse/succeed via `?remote=leasefail`.
Invariants: Rust owns all git logic (hooks + lease via git binary, `&Path`+injected `GitExec` ⇒
CLI-testable); heavy calls in `spawn_blocking`; a failing blocking hook NEVER yields a silent success
(trust); destructive force-push keeps its UI confirm (P37) + now git's atomic server check; never-prompt
policy preserved; mock covers every new surface.

**USER CHECKPOINT (native; cannot be AI-judged):**
- **P59a:** real-world hook managers (Husky, `pre-commit`, lint-staged, gitleaks) in a native repo:
  a failing hook blocks the commit and the dialog shows the tool's real output legibly; a passing run
  commits; **Windows** shell-script hooks execute (validates the `git hook run` shell path); the per-repo
  "Run git hooks" toggle + "Skip hooks" behave; commit-msg rewrites (e.g. commitizen/gitmoji) reflect in
  the created message.
- **P59b:** against a real remote (GitHub/GitLab): force-push-with-lease publishes an amend/rebase; when a
  teammate has pushed, git's atomic lease refuses with a readable message and changes nothing;
  credential resolution via the user's helper works with NO interactive prompt.

---

## 12. Open questions (flag to orchestrator; recommendation in bold)

- **OQ-A1 — Git < 2.36 fallback.** **Recommend:** require ≥2.36 for hook enforcement; if unavailable AND
  hook files exist, surface a one-time `Git` error rather than silently bypassing (trust). Alternative:
  a Rust direct-exec fallback (re-introduces the Windows-shell complexity A-D1 avoids). Confirm.
- **OQ-A2 — `HookRejected` as a new variant.** **Recommend ADD** (distinct UX: render the hook's output).
  Alternative: reuse `Git` with a prefix (loses the structured dialog). Confirm.
- **OQ-A3 — pre-push scope.** **Recommend include as P59a-2** (real trust value: secret-scan-on-push), but
  it is lower priority than commit hooks; confirm whether to ship in the same milestone or defer.
- **OQ-A4 — Where the "Run git hooks" toggle lives.** **Recommend a repo-scoped Settings row** bound to
  `bonsai.runHooks` via the existing config commands (no new command). Confirm placement (Settings vs a
  commit-box affordance vs both).
- **OQ-B1 — `--force-if-includes`.** **Recommend include** (extra safety, Git ≥2.30). Drop only if a
  minimum-git-version concern arises. Confirm.
- **OQ-B2 — Lease auth via git's helper vs libgit2 cache.** **Recommend accept git's helper** for this one
  op (required for the atomic lease; same never-prompt helper used for reads). If reusing the P35 in-process
  cache is a hard requirement, the atomic lease is not achievable and we must keep the documented-TOCTOU
  status quo — call it explicitly. Confirm the trade.
