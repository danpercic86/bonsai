# P70 — Git executable resolution + honest "git not found" diagnostics

One shared resolver for the `git` binary, honest error taxonomy when it cannot be launched, and a
startup preflight that surfaces the condition once instead of as N misleading auth toasts.

References read (@ `71236f8`): `crates/bonsai-core/src/procutil.rs`, `.../error.rs`, `.../lib.rs`,
`.../ai/mod.rs` (`CLAUDE_BIN_ENV`/`resolve_bin` idiom), `.../git/{search,exec,remote,cred_cache,
maintenance}.rs`, `src-tauri/src/commands/{external,mod,registration_tests}.rs`,
`src-tauri/src/lib.rs`, `src/ipc/types.ts`, `src/ipc/mock.ts`,
`src/ipc/mock/{repoState,handlers/update}.ts`, `src/App.tsx`, `src/components/RepoWorkspace.tsx`.
House pattern: `P49-external-integrations.md` (self-contained spawn, `TargetOs`/trait injection),
`P42-packaging-autoupdate.md` (`?update=` harness seam).

**Root cause is already diagnosed — do not re-investigate.** The MSI updater relaunches
`bonsai.exe` as a child of `msiexec.exe`, so the app inherits msiexec's environment block. The
user's Git is a per-user install at `%LOCALAPPDATA%\Programs\Git\cmd\git.exe`, present only in the
**User** PATH. `Command::new("git")` therefore cannot resolve, and `credential_fill`'s
`cmd.spawn().ok()?` collapses that into `None`, which `acquire_cred` reads as "helper had nothing
cached".

**Blast radius — read this before writing any code.** A missing `git` CLI breaks only the paths
that *shell out*: commit/content search, commit-graph writes, signing, hooks, atomic
force-with-lease, and **HTTPS credential resolution via `git credential fill`**. It does **not**
break libgit2's own work (status, graph walk, diffs, fetch/push transport), and it does **not**
break **SSH remotes authenticating through a running ssh-agent** — that path lives entirely inside
libgit2 (`Cred::ssh_key_from_agent`) and never touches `git.exe`. Every message, banner, test and
checkpoint in this contract must respect that boundary.

*Amended 2026-08-19 (orchestrator): §9 decisions closed; §3.1 rewritten to remove an SSH-agent
regression; §3.3 / §4.5 / §6 copy narrowed to HTTPS-helper remotes; §5.1 added; §7.1 clarified that
the SSH guarantee rests on tests #16 + #18 + the native checkpoint (no agent seam — decided).*

---

## 0. Key decisions

**D1 — New module `crates/bonsai-core/src/gitbin.rs`, not an extension of `procutil.rs`.**
`procutil` is a generic, dependency-free "resolve a program name against PATH/PATHEXT" helper shared
by `ai::resolve_bin` and `external.rs`; it stays 46 lines and unchanged. `gitbin` is a different
concern: a git-specific candidate ladder, registry probing via a child process, a process-lifetime
cache, a serialized wire type, and a `Command` factory. Keeping them separate leaves both files far
under the ~500-line limit (`gitbin.rs` lands at ~180 lines of logic + ~150 of unit tests) and lets
`gitbin` depend on `procutil` rather than tangle with it.

**D2 — No registry crate.** `bonsai-core`'s dep set stays `git2 / serde / serde_json / thiserror`.
The registry read shells out to `%SystemRoot%\System32\reg.exe` **by absolute path** (the whole
premise of this milestone is that PATH may be unusable), with `CREATE_NO_WINDOW`, defensive
parsing, and every failure silently skipped.

**D3 — Cache is a refreshable `RwLock<Option<GitBin>>`, not `OnceLock`.** *Deviation from the
original brief, **approved 2026-08-19**.* A plain `OnceLock` cannot express "the user installed Git
and pressed **Re-check**" without an app restart, and returning `&'static GitBin` would require
leaking. The cached value is a `PathBuf` + a fieldless enum, so `git_bin()` costs one uncontended
read lock + one `PathBuf` clone — negligible against a process spawn. `refresh_git_bin()` re-runs
the ladder and replaces the cache; `reset_git_bin_cache()` (`#[cfg(test)]`) clears it.

**D4 — Resolution NEVER executes `git --version`.** The ladder validates candidates with
`is_file()` only. Rationale: on Windows a process spawn costs 20–80 ms and resolution sits on the
hot path of every search/graph/signing call. Execution-based validation happens exactly once, in
the §4 preflight (`check_availability()`), which is off the hot path and needs the version string
anyway. **Confirmed 2026-08-19: this is not a gap** — a resolved-but-corrupt `git.exe` is caught by
the preflight (which does run `--version`), so the banner stays correct.

**D5 — New `AppError::GitNotFound(String)`** (kind `"gitNotFound"`). Reusing `Git`/`Other` would
leave the frontend unable to distinguish this from a thousand ordinary git failures, and the whole
point of the milestone is that the frontend must show ONE persistent banner instead of N toasts.

**D6 — One `Command` factory, `gitbin::git_command()`.** Migration is then a one-line edit per call
site, and the child-PATH augmentation (§2.5) applies uniformly and cannot be forgotten.

**D7 — Decisions closed by the orchestrator on 2026-08-19** (previously §9 open questions):
1. `RwLock` cache instead of `OnceLock` — **approved** (D3).
2. HKCU queried before HKLM — **approved** (§2.3 step 3).
3. Child-PATH augmentation in `git_command()` — **approved, include** (§2.5).
4. `gitNotFound` suppresses the error toast entirely; the banner is the sole surface — **approved**
   (§7.4).
5. `fixture.rs::have_git()` migration — **approved, include** (§2.5).
6. No `--version` during resolution — **confirmed, not a gap** (D4).

**D8 — No injectable seam over `Cred::ssh_key_from_agent`** (decided 2026-08-19). It would add
indirection to the credential hot path for a marginal coverage gain. The SSH-agent guarantee is
carried by tests #16 + #18 plus the native checkpoint — see §7.1.

**Command count: +1** (`check_git_availability`). `registration_tests.rs` compares the *defined* set
against the `generate_handler!` set — there is no hard-coded count to bump, but both `commands/mod.rs`
and `lib.rs` must be updated or that test fails.

---

## 1. Module boundaries / files

**New**
- `crates/bonsai-core/src/gitbin.rs` — the ladder (pure fn over an injected `GitEnv`), the cache,
  `git_command()`, `check_availability()`, `git_not_found_message()`, `GitBin`/`GitBinSource`/
  `GitAvailability`.
- `src-tauri/src/commands/git_env.rs` — the single `#[tauri::command] check_git_availability`.
- `src/hooks/useGitAvailability.ts` — one-shot startup probe + re-check + "a gitNotFound error was
  observed" latch.
- `src/ipc/errors.ts` — `isGitNotFound(e: unknown): boolean` (shared predicate, ~10 lines).
- `src/ipc/mock/handlers/gitEnv.ts` — mock handler + `?git=` seam.
- `src/components/GitMissingBanner.tsx` — **placeholder only**; the visual contract belongs to
  `ui-designer` (see §7.3).

**Edited**
- `crates/bonsai-core/src/lib.rs` — `pub mod gitbin;`
- `crates/bonsai-core/src/error.rs` — add `GitNotFound`; update the doc comment, `kind()`, `message()`.
- `crates/bonsai-core/src/git/search.rs` — `SpawnGitRunner::run` (§2.5, §3.2).
- `crates/bonsai-core/src/git/exec.rs` — `build_command` + `SpawnGitExec::exec` (§2.5, §3.2).
- `crates/bonsai-core/src/git/remote.rs` — `credential_fill`, `CredAttempts`, `acquire_cred`,
  `map_remote_err` (§3).
- `crates/bonsai-core/src/git/cred_cache.rs` — `FillFn` / `resolve` signature (§3.1).
- `crates/bonsai-core/src/fixture.rs` — `have_git()` (one line, §2.5).
- `src-tauri/src/commands/mod.rs` — `mod git_env; pub use git_env::*;`
- `src-tauri/src/lib.rs` — register `commands::check_git_availability`.
- `src/ipc/types.ts` — `GitAvailability`, `GitBinSource`, `'gitNotFound'` in the `AppError` union,
  `checkGitAvailability()` on `IpcApi`.
- `src/ipc/tauri.ts` — the real invoke.
- `src/ipc/mock.ts` — spread `gitEnvHandlers`.
- `src/App.tsx` — mount `useGitAvailability`, render the banner.
- `src/components/RepoWorkspace.tsx` — remote-op catch sites route `gitNotFound` per §7.4.

---

## 2. D1 — the resolver

### 2.1 Types

```rust
// crates/bonsai-core/src/gitbin.rs

/// Env override; mirrors the `CLAUDE_BIN_ENV` idiom in `ai/mod.rs`. Used
/// verbatim (no PATHEXT expansion, no existence check) and doubles as the
/// hermetic test seam for out-of-process integration tests.
pub const GIT_BIN_ENV: &str = "BONSAI_GIT_BIN";

/// Which rung of the ladder produced the path. `Fallback` means the ladder was
/// exhausted and we handed `Command` the bare name `git` so its own `NotFound`
/// error path still fires naturally.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum GitBinSource {
    Override,
    Path,
    Registry,
    WellKnown,
    Fallback,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitBin {
    pub path: std::path::PathBuf,
    pub source: GitBinSource,
}

impl GitBin {
    /// `false` iff `source == Fallback`.
    pub fn found(&self) -> bool;
    /// Directory to prepend to a child's PATH (§2.5). `None` for
    /// `Path`/`Fallback` sources.
    pub fn bin_dir(&self) -> Option<&std::path::Path>;
}
```

### 2.2 Injection seam (this is what makes the tests hermetic)

```rust
/// Every environment interaction the ladder performs, injected so the ladder is
/// a pure function under test. Precedent: `GitRunner` (search.rs) / `GitExec`
/// (exec.rs) / `CommandRunner` (external.rs).
pub trait GitEnv {
    fn var(&self, key: &str) -> Option<String>;
    fn is_file(&self, p: &std::path::Path) -> bool;
    /// PATH + PATHEXT resolution; production delegates to
    /// `crate::procutil::resolve_program`.
    fn resolve_on_path(&self, program: &str) -> Option<std::path::PathBuf>;
    /// Read one registry string value. `key` is a full path
    /// (`"HKCU\\SOFTWARE\\GitForWindows"`), `value` a value name
    /// (`"InstallPath"`). `None` on ANY failure. No-op on non-Windows.
    fn registry_string(&self, key: &str, value: &str) -> Option<String>;
}

/// Production implementation: real `std::env`, real fs, real `reg.exe`.
pub struct HostGitEnv;
impl GitEnv for HostGitEnv { /* … */ }
```

### 2.3 Ladder (pseudocode — implement verbatim)

```
fn resolve_ladder(env: &dyn GitEnv) -> GitBin:

    # 1. explicit override — verbatim, no validation (test seam + user escape hatch)
    if let Some(v) = env.var(GIT_BIN_ENV), non-empty after trim:
        return GitBin { path: v, source: Override }

    # 2. process PATH (fast path; PATHEXT-aware on Windows)
    if let Some(p) = env.resolve_on_path("git"):
        return GitBin { path: p, source: Path }

    # --- Windows only (cfg(windows)) ---------------------------------------
    # 3. Git for Windows canonical registry key.
    #    HKCU FIRST (decided 2026-08-19): a per-user install is the one the user
    #    actually chose and is the exact failing case here; a machine-wide
    #    install is the fallback.
    for key in [ r"HKCU\SOFTWARE\GitForWindows",
                 r"HKLM\SOFTWARE\GitForWindows",
                 r"HKLM\SOFTWARE\WOW6432Node\GitForWindows" ]:
        if let Some(install) = env.registry_string(key, "InstallPath"):
            cand = install.trim_end_matches(['\\','/']) + r"\cmd\git.exe"
            if env.is_file(cand): return GitBin { cand, source: Registry }

    # 4. well-known locations, in this order
    for (var, suffix) in [ ("LOCALAPPDATA",     r"Programs\Git\cmd\git.exe"),
                           ("ProgramFiles",     r"Git\cmd\git.exe"),
                           ("ProgramW6432",     r"Git\cmd\git.exe"),
                           ("ProgramFiles(x86)",r"Git\cmd\git.exe") ]:
        if let Some(base) = env.var(var):
            cand = base + "\" + suffix
            if env.is_file(cand): return GitBin { cand, source: WellKnown }

    # --- Unix only (cfg(not(windows))) -------------------------------------
    for cand in ["/usr/bin/git", "/usr/local/bin/git", "/opt/homebrew/bin/git"]:
        if env.is_file(cand): return GitBin { cand, source: WellKnown }

    # 5. bare name — lets the existing NotFound spawn error fire naturally
    return GitBin { path: "git", source: Fallback }
```

**Invariants.** Steps are tried strictly in order and a later step is reached ONLY when the earlier
candidate is absent (`is_file()` false) — except step 1, which short-circuits unconditionally, and
step 2, which relies on `resolve_program`'s own `is_file()` checks. No candidate is ever executed
(D4). `resolve_ladder` never panics, never returns `Err`.

**WOW64.** `bonsai.exe` is 64-bit, so `System32\reg.exe` is the 64-bit `reg.exe` and reads the
64-bit registry view. A 32-bit Git for Windows install lands under `HKLM\SOFTWARE\WOW6432Node\…`,
which the third key covers explicitly — no `/reg:32` flag needed. `HKCU\Software\GitForWindows` is
**not** subject to registry redirection (only `HKCU\Software\Classes` is), so it needs no variant.

### 2.4 `reg.exe` probe (production `HostGitEnv::registry_string`)

```
reg_exe = env("SystemRoot").unwrap_or("C:\Windows") + r"\System32\reg.exe"
if !is_file(reg_exe): return None
run: reg_exe query "<key>" /v "<value>"
     stdin=null, stdout=piped, stderr=null, CREATE_NO_WINDOW
non-zero exit or spawn error -> None
parse stdout (utf8-lossy) line by line:
    trimmed line whose first whitespace-token == <value> and which contains
    "REG_SZ" or "REG_EXPAND_SZ":
        take everything after the FIRST occurrence of that type token, trim -> Some(path)
    (a REG_EXPAND_SZ value is used as-is; Git for Windows writes a literal path)
no match -> None
```
Never panics, never logs the raw output, no timeout (a `reg query` that hangs is not a scenario we
model; it is only reached when git is already unresolvable).

### 2.5 Public API + call-site migration

```rust
/// Cached resolution. First call runs `resolve_ladder(&HostGitEnv)`; later calls
/// read the cache. Cheap enough to call per spawn.
pub fn git_bin() -> GitBin;
/// Re-runs the ladder and replaces the cache. Used by the preflight command so
/// "install Git, press Re-check" works without restarting the app.
pub fn refresh_git_bin() -> GitBin;
#[cfg(test)] pub fn reset_git_bin_cache();

/// `true` when the ladder was exhausted (`source == Fallback`). Cheap.
pub fn git_missing() -> bool;

/// THE spawn factory. Every production `git` invocation goes through this.
/// Sets: program = `git_bin().path`; `CREATE_NO_WINDOW` on Windows; and, when
/// `bin_dir()` is `Some`, prepends that directory to the CHILD's `PATH` so a
/// hook script or credential helper that itself calls `git` still works even
/// though the inherited PATH is broken (decided 2026-08-19). Sets NO other env —
/// the call sites keep their own never-prompt hardening
/// (`GIT_TERMINAL_PROMPT=0`, askpass removal), which their existing tests assert.
pub fn git_command() -> std::process::Command;

/// Classify a spawn `io::Error` from a git child.
/// `NotFound` OR `git_missing()` -> `AppError::GitNotFound(git_not_found_message())`;
/// anything else -> `AppError::Git(format!("failed to run `git {subcmd}`: {e}"))`.
pub fn spawn_error(subcmd: &str, e: &std::io::Error) -> crate::error::AppError;

/// Platform-branched user-facing copy (§3.3).
pub fn git_not_found_message() -> String;
```

**Production sites to migrate — exactly four** (line numbers @ `71236f8`):

| File:line | Site | Change |
|---|---|---|
| `crates/bonsai-core/src/git/search.rs:129` | `SpawnGitRunner::run` | `Command::new("git")` → `gitbin::git_command()`; drop the now-redundant `creation_flags` block; spawn error → `gitbin::spawn_error(subcmd, &e)` where `subcmd = args.first()` (§3.2). Covers commit/content search (P50), `maintenance::write_commit_graph_best_effort` (P52) and every other `&dyn GitRunner` consumer. |
| `crates/bonsai-core/src/git/exec.rs:72` | `build_command` (`SpawnGitExec`) | same swap; drop the redundant `creation_flags` block. Spawn-error mapping in `SpawnGitExec::exec` (§3.2). Covers signing (P58), hooks (P59), atomic force-with-lease (P59b). |
| `crates/bonsai-core/src/git/remote.rs:181` | `credential_fill` | same swap + the signature change in §3.1. |
| `crates/bonsai-core/src/fixture.rs:295` | `have_git()` | one line (approved 2026-08-19); makes fixture generation work under the same broken PATH. |

**Test-only sites — DO NOT TOUCH.** Every hit below sits inside a `#[cfg(test)] mod tests` (the
`#[cfg(test)]` line number is in parentheses) or in a test-only file. Most are `Command::new("git")
.arg("--version")` availability probes; they intentionally exercise the ambient PATH and must keep
doing so. Senior-dev: do not churn these, and do not "helpfully" migrate them.

- `crates/bonsai-core/src/health.rs` 611, 775, 951 (565)
- `crates/bonsai-core/src/git/undo.rs` 433, 442 (235)
- `crates/bonsai-core/src/git/stale.rs` 650, 1181 (567)
- `crates/bonsai-core/src/git/search.rs` 467, 906 (459)
- `crates/bonsai-core/src/git/remote.rs` 1660, 1669, 1695 (1163)
- `crates/bonsai-core/src/git/merge.rs` 976 (581)
- `crates/bonsai-core/src/git/branches.rs` 2076, 2252 (1993)
- `crates/bonsai-core/src/git/compose_apply.rs` 320, 410 (272)
- `crates/bonsai-core/src/git/hooks.rs` 511, 520 (382)
- `crates/bonsai-core/src/git/maintenance.rs` 90 (81)
- `crates/bonsai-core/src/git/exec.rs` 220 (215)
- `src-tauri/src/scheduler.rs` 734, 846, 916 (570)
- `src-tauri/src/mcp.rs` 758, 789, 806 (602)
- `src-tauri/src/commands/tests_support.rs` 157, 168 (test-only file)
- all of `crates/bonsai-core/tests/**`, `crates/bonsai-mcp/tests/**`

---

## 3. D2 — honest diagnostics

### 3.1 `credential_fill` → `acquire_cred` → `map_remote_err`

> **Rewritten 2026-08-19.** The first draft short-circuited `acquire_cred` on `git_missing()`
> *before the credential ladder*. That was a regression: an SSH remote with a running ssh-agent
> authenticates entirely inside libgit2 via `Cred::ssh_key_from_agent` and never needs `git.exe`, so
> the short-circuit would have broken a currently-working flow the instant git fell off PATH —
> converting a fix into a wider outage. **The ladder is never short-circuited.** The git-missing
> check is narrowed to the `Helper` rung, and the distinction is consumed only at exhaustion.

```rust
// remote.rs
/// Distinguishes "git could not be launched" from "the helper had nothing".
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum FillOutcome {
    Filled { username: String, password: String },
    /// git ran and exited, but produced no usable username+password (cache
    /// miss, non-zero exit, unparseable output). The pre-P70 `None` meaning.
    NoCredentials,
    /// The `git` child could NOT be launched at all. Carries the io error text
    /// for the log; the user-facing string comes from `git_not_found_message()`.
    GitUnavailable(String),
}

pub(crate) fn credential_fill(repo_path: Option<&Path>, url: &str) -> FillOutcome;
```
Mapping inside `credential_fill`: `cmd.spawn()` → `Err(e)` ⇒ `GitUnavailable(e.to_string())` (ANY
io error kind — `NotFound` and `PermissionDenied` are both "not launchable"). Every other existing
`None` path (stdin write failure, non-zero exit, non-UTF-8 stdout, missing/empty fields) ⇒
`NoCredentials`. Behaviour otherwise unchanged: still never prompts, still discards stderr, still
reaps the child.

```rust
// cred_cache.rs
type FillFn = Box<dyn Fn(Option<&Path>, &str) -> FillOutcome + Send + Sync>;

/// Replaces the `Option<Resolved>` return. Only `Resolved` is ever cached;
/// `NoCredentials`/`GitUnavailable` are never stored and never single-flight-
/// cached (a transient launch failure must not poison the cache).
pub(crate) enum CredResolve {
    Resolved(Resolved),
    NoCredentials,
    GitUnavailable(String),
}
pub(crate) fn resolve(repo_path: Option<&Path>, url: &str, bypass: bool) -> CredResolve;
```
`CredCache::resolve`'s single-flight/TTL/stale-while-revalidate machinery is unchanged; only the
"fill returned nothing" arm splits in two. `warm()` keeps its fire-and-forget signature and simply
ignores non-`Resolved` outcomes.

```rust
// remote.rs — sentinel threaded through git2's callback error, exactly like the
// existing CRED_EXHAUSTED_MSG mechanism.
pub(crate) const GIT_MISSING_MSG: &str = "bonsai: git executable not found";

/// One new field on the existing struct: remembers WHY the Helper rung failed,
/// so the reason can be consumed at ladder exhaustion instead of short-circuiting.
pub(crate) struct CredAttempts {
    // … existing fields: helper: HelperState, agent: bool, default_: bool,
    //    fresh_fill_url: Option<String> …
    /// Set when the Helper rung failed specifically because `git` could not be
    /// launched. NEVER set when the helper ran and simply had nothing.
    pub(crate) helper_git_unavailable: bool,
}
```

**`acquire_cred` — the corrected rules.**

1. **No pre-loop check. The ladder runs in full.** `next_cred_method` is unchanged; every rung
   (`Helper` → `SshAgent` → `Default`) is still offered exactly as before.
2. **`Helper` rung, cheap-fail path.** On entering the `Helper` arm, `if gitbin::git_missing() { let
   mut a = attempts.borrow_mut(); a.helper = HelperState::Done; a.helper_git_unavailable = true;
   continue; }` — the rung fails immediately **without spawning**, records the reason, and the loop
   proceeds to `SshAgent`.
3. **`Helper` rung, runtime spawn failure.** `CredResolve::GitUnavailable(_)` ⇒ same treatment:
   `helper = Done`, `helper_git_unavailable = true`, **continue the loop** (do NOT return). This
   covers the race where the resolver's cached path went stale mid-session.
4. **`CredResolve::NoCredentials`** ⇒ the existing `helper = Done` fall-through, unchanged, and
   `helper_git_unavailable` stays `false`.
5. **Exhaustion (the `None` arm) is the ONLY place the distinction is consumed:**
   ```rust
   None => {
       let msg = if attempts.borrow().helper_git_unavailable {
           GIT_MISSING_MSG          // -> AppError::GitNotFound
       } else {
           CRED_EXHAUSTED_MSG       // -> the existing auth message, unchanged
       };
       return Err(git2::Error::new(ErrorCode::Auth, ErrorClass::Callback, msg));
   }
   ```

**Consequences that must hold (assert them):**
- **SSH remote + running ssh-agent + git missing ⇒ unchanged success.** `SshAgent` is reached and
  returns a credential; exhaustion is never hit; no `GitNotFound` is ever produced.
- **SSH-only remote (no `USER_PASS_PLAINTEXT` in `allowed`) ⇒ the `Helper` rung is never selected**,
  so `helper_git_unavailable` stays `false` and a genuine SSH auth failure still reports the
  existing auth copy — not a git-not-found message.
- **HTTPS remote + git missing ⇒ `GitNotFound`**, because `Helper` is offered, fails for the right
  reason, and the ladder ultimately exhausts.

**`map_remote_err`** — one new check, **first**, before the `CRED_EXHAUSTED_MSG` and
`ErrorCode::Auth` arms:
```rust
if e.message().contains(GIT_MISSING_MSG) {
    return AppError::GitNotFound(gitbin::git_not_found_message());
}
```
Everything below it is untouched, so the existing auth copy still fires for genuine auth failures
(including every SSH failure).

### 3.2 The other shell-out paths

Both go through `gitbin::spawn_error`, so a missing git yields the SAME `GitNotFound` message
everywhere rather than N raw texts:

- `SpawnGitRunner::run` (search.rs): `cmd.output()` `Err(e)` ⇒ `gitbin::spawn_error(subcmd, &e)`.
  **Also fix the existing lie**: the message hard-codes ``failed to run `git log` `` even when the
  runner is writing a commit-graph; derive `subcmd` from `args.first()` (fallback `"git"`).
  Non-zero-exit messages likewise use `subcmd`, not the literal `log`.
- `SpawnGitExec::exec` (exec.rs): spawn/`wait` io `Err(e)` ⇒ `gitbin::spawn_error(args.first(), &e)`.
  A non-zero exit is still NOT an error (unchanged contract).

**Out of scope, stated explicitly**: `hooks.rs` spawns hook *scripts*, not `git.exe` — it is not
migrated. It benefits indirectly from the §2.5 child-PATH augmentation when a hook shells out to
`git`.

### 3.3 Exact user-facing copy

`AppError::GitNotFound` carries `git_not_found_message()`, `#[cfg(windows)]`-branched. The copy must
name the real problem, must not claim anything about cached credentials except to deny it, and —
per the 2026-08-19 amendment — **must not tell an SSH-agent user that their fetch is broken**, since
it is not.

**Windows**
```
Git is not available. Bonsai could not find a runnable `git` executable — it checked
BONSAI_GIT_BIN, PATH, the Git for Windows registry key, and the standard install
folders. This is NOT an authentication failure: your saved credentials were never
consulted, because Bonsai could not start the credential helper. This affects HTTPS
remotes (which resolve credentials through Git's credential helper) plus commit search
and signing; SSH remotes using an ssh-agent are unaffected. Fix: quit Bonsai and
relaunch it from the Start menu (an in-app update can leave the app running with an
incomplete PATH), or install Git for Windows, or set BONSAI_GIT_BIN to the full path of
git.exe and restart.
```

**macOS / Linux**
```
Git is not available. Bonsai could not find a runnable `git` executable — it checked
BONSAI_GIT_BIN, PATH, and the standard install locations (/usr/bin, /usr/local/bin,
/opt/homebrew/bin). This is NOT an authentication failure: your saved credentials were
never consulted, because Bonsai could not start the credential helper. This affects
HTTPS remotes (which resolve credentials through Git's credential helper) plus commit
search and signing; SSH remotes using an ssh-agent are unaffected. Fix: install Git, or
set BONSAI_GIT_BIN to the full path of the git binary and restart Bonsai.
```

Forbidden in this string: the word "authentication" other than in the denial; any claim that the
credential helper "has no cached credentials"; any flat claim that "fetch, pull and push" are broken.

### 3.4 `AppError`

Add, after `HookRejected` and before the forge block:
```rust
/// P70: no runnable `git` executable could be resolved (PATH inherited from an
/// installer, Git not installed, override pointing nowhere). Distinct from
/// `Git` so the frontend can show ONE persistent banner instead of N toasts,
/// and distinct from `AuthFailed` so a launch failure is never reported as a
/// credential problem. Raised only by paths that actually shell out — SSH-agent
/// authentication never produces it.
#[error("{0}")]
GitNotFound(String),
```
Exhaustive matches to update — **only `crates/bonsai-core/src/error.rs`** (verified by grep; every
other `AppError::` hit in the tree is a construction site):
- the module doc comment listing serialized kinds → add `| "gitNotFound"`,
- `kind()` → `AppError::GitNotFound(_) => "gitNotFound"`,
- `message()` → add `GitNotFound(m)` to the `|`-chain.

TS mirror: add `| 'gitNotFound'` to the `AppError['kind']` union in `src/ipc/types.ts` (~line 1955),
with a doc comment pointing at the banner behaviour in §7.4.

---

## 4. D3 — startup preflight

### 4.1 Rust

```rust
// crates/bonsai-core/src/gitbin.rs
/// Blocking. Re-runs the ladder (`refresh_git_bin`), then — only when
/// `found()` — executes `<path> --version` ONCE and parses the version token.
/// NEVER returns `Err`: an unresolvable or unspawnable git yields
/// `{ found: false, .. }`, mirroring `ai::AiAvailability`'s contract.
pub fn check_availability() -> GitAvailability;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitAvailability {
    /// A git executable was resolved AND `--version` exited 0.
    pub found: bool,
    /// Absolute path actually used; `None` when the ladder fell back.
    pub path: Option<String>,
    /// e.g. `"2.47.1.windows.1"` — parsed from `git version <X>`; `None` when
    /// not found or unparseable.
    pub version: Option<String>,
    /// Which rung produced the path.
    pub source: GitBinSource,
    /// Human one-liner. Found: `"Git 2.47.1.windows.1 — C:\\…\\cmd\\git.exe (registry)"`.
    /// Not found: the full `git_not_found_message()` text (§3.3).
    pub detail: String,
}
```
`--version` runs via `git_command()` with `stdin(null)`, both pipes captured, no timeout (a
`--version` that hangs is not a modeled scenario). Spawn error or non-zero exit ⇒ `found: false`
with `source` still reporting where the (bad) path came from.

### 4.2 Tauri command

```rust
// src-tauri/src/commands/git_env.rs
/// Cheap one-shot git preflight (P70). NEVER rejects for git state — a missing
/// git is `{ found: false, .. }`, not an error. Safe to re-invoke (the banner's
/// Re-check button does).
#[tauri::command]
pub async fn check_git_availability() -> Result<GitAvailability, AppError>;
```
Body: `tauri::async_runtime::spawn_blocking(|| Ok(bonsai_core::gitbin::check_availability())).await`
— the resolver touches the filesystem and spawns a child, so it must not run on the UI thread. No
`repo_path`, no state, no `opActive` gating (it is git-state-free, like the P49 commands).
Register in `commands/mod.rs` and `lib.rs`'s `generate_handler!`.

### 4.3 TS surface

```ts
// src/ipc/types.ts
export type GitBinSource = 'override' | 'path' | 'registry' | 'wellKnown' | 'fallback';

/** P70: startup git preflight. `found:false` is a normal result, never a
 *  rejection. Mirrors the Rust `GitAvailability`. */
export interface GitAvailability {
  found: boolean;
  path: string | null;
  version: string | null;
  source: GitBinSource;
  detail: string;
}

// on IpcApi:
/** P70: resolve the `git` executable and report availability. Cheap, one-shot at
 *  startup, re-invocable from the banner's Re-check. Never rejects for git state. */
checkGitAvailability(): Promise<GitAvailability>;
```

### 4.4 Frontend wiring

`src/hooks/useGitAvailability.ts`:
```ts
export interface GitAvailabilityState {
  status: GitAvailability | null;   // null = not probed yet -> render nothing
  checking: boolean;
  recheck: () => void;              // re-invokes checkGitAvailability
  noteGitNotFound: () => void;      // latch set by any observed gitNotFound error
}
export function useGitAvailability(): GitAvailabilityState;
```
- Probes **once** on mount from inside a `useEffect` — i.e. after first paint; nothing awaits it and
  no render is gated on it. Failure of the invoke itself is swallowed (`status` stays `null`).
- `noteGitNotFound()` flips an internal latch that forces the banner visible even if the probe
  raced or the state changed mid-session; a successful `recheck()` (`found === true`) clears it.
- Mounted in `src/App.tsx` alongside `useUpdateController` (same shape: state machine in a hook,
  App only wires the surface).

### 4.5 Mock IPC — `src/ipc/mock/handlers/gitEnv.ts`

Harness seam `?git=`, read once at module init via `query('git')` (mirrors `AI_OFF` / `?update=`):

| value | behaviour |
|---|---|
| *(absent)* / anything else | `{ found: true, path: '/usr/bin/git', version: '2.47.1', source: 'path', detail: 'Git 2.47.1 — /usr/bin/git (path)' }` |
| `missing` | `{ found: false, path: null, version: null, source: 'fallback', detail: <the §3.3 Windows text> }`, **and** `fetch` / `pull` / `push` / `fetchAll` reject with `{ kind: 'gitNotFound', message: <same text> }` |
| `registry` | found, `source: 'registry'`, `path: 'C:\\Users\\dev\\AppData\\Local\\Programs\\Git\\cmd\\git.exe'` — proves the "found via a non-PATH rung" detail line renders |

The `?git=missing` remote rejections model an **HTTPS remote whose credential helper cannot be
launched** — that is the real-world case. The mock deliberately does not model an SSH remote here;
per §3.1 an ssh-agent remote would keep working, so there is nothing for the banner path to show.
Add a code comment saying so, so nobody later "fixes" the mock by rejecting SSH too.

Spread into `src/ipc/mock.ts` next to `updateHandlers`. The `?git=missing` remote rejections are the
mechanism by which the harness proves the honest copy replaced the auth copy without a native run.

---

## 5. What is deliberately NOT changing

- The credential ladder itself (Helper → SshAgent → Default), the invalidation state machine, TTL,
  stale-while-revalidate, single-flight — all unchanged; only the "fill produced nothing" arm splits
  in two and one field is added to `CredAttempts`.
- **SSH-agent authentication.** It never shells out and is untouched by every part of P70. No seam
  is introduced over `Cred::ssh_key_from_agent` (D8).
- The never-prompt policy: `GIT_TERMINAL_PROMPT=0`, `-c core.askpass=`, `GIT_ASKPASS`/`SSH_ASKPASS`
  removal stay exactly where they are, at the call sites.
- The genuine-auth-failure copy in `map_remote_err` (both the helper-configured and
  no-helper-configured variants) is untouched.
- No timeouts are added or removed anywhere; `run_with_git_timeout` coverage is unchanged.
- libgit2's own operations (fetch/push transport, status, graph walk, diffs) do not shell out and
  are entirely unaffected — a missing `git` CLI never breaks them.
- Test-only `Command::new("git")` availability probes keep using ambient PATH (§2.5).
- No new crate dependency in any workspace member.
- The banner's visual design, placement, tone, motion and copy *styling* — `ui-designer` owns them
  (§7.3).

### 5.1 Upstream cause — documented, out of P70 scope

For the record: the trigger is that the **MSI updater relaunches the app as a child of
`msiexec.exe`**, so `bonsai.exe` inherits the installer's environment block rather than the user's
interactive environment (User PATH included). P70 makes the **app resilient** to being launched with
a degraded environment — it does not change how the updater relaunches.

Changing the relaunch parent (e.g. having the installer hand the relaunch to `explorer.exe` or to a
helper that re-reads `HKCU\Environment`, or re-broadcasting the environment before relaunch) is a
**separate follow-up outside P70**. Do not design or implement it here. Flagged for the orchestrator
to schedule independently; P70 must ship complete without it.

---

## 6. Acceptance criteria

### 6.1 Machine-verifiable (orchestrator's AI gate)

**Rust unit — `gitbin.rs`, all against a `FakeGitEnv`, zero `std::env` mutation:**
1. `BONSAI_GIT_BIN=/x/y/git` wins verbatim even when PATH would resolve and the file does not exist
   → `source: Override`.
2. Empty/whitespace `BONSAI_GIT_BIN` is ignored → ladder continues.
3. PATH hit → `source: Path`; registry/well-known are never consulted (fake records zero
   `registry_string` calls).
4. No PATH, `HKCU` `InstallPath` present, `<install>\cmd\git.exe` is a file → `source: Registry`,
   that exact path.
5. `HKCU` present but its `cmd\git.exe` is NOT a file → falls through to `HKLM`, then
   `WOW6432Node`, then well-known.
6. No PATH, no registry, `%LOCALAPPDATA%\Programs\Git\cmd\git.exe` exists → `source: WellKnown`;
   the three `ProgramFiles*` vars are checked only after LOCALAPPDATA misses (assert call order).
7. Everything missing → `GitBin { path: "git", source: Fallback }`, `found() == false`.
8. Unix ladder ordering: `/usr/bin` → `/usr/local/bin` → `/opt/homebrew/bin` → fallback.
9. `resolve_ladder` never spawns anything (fake `GitEnv` counts spawns; must be 0 — proves D4).
10. `reg.exe` output parser: real-format `REG_SZ` block, `REG_EXPAND_SZ`, a value name that is a
    prefix of another, empty output, garbage/localized output → correct `Some`/`None` with no panic.
11. `bin_dir()` is `Some` for Registry/WellKnown/Override-with-parent, `None` for Path/Fallback.
12. `git_command()` prepends `bin_dir()` to the child's `PATH` when `bin_dir()` is `Some`, and
    leaves `PATH` untouched when it is `None` (assert via `Command::get_envs`, no spawn).

**Rust unit — diagnostics:**
13. `credential_fill` with `BONSAI_GIT_BIN` pointing at a nonexistent path returns
    `FillOutcome::GitUnavailable(_)` (out-of-process integration test, `Command::env`).
14. `credential_fill` with `BONSAI_GIT_BIN` pointing at a stub that exits 0 with no output returns
    `NoCredentials` — proving the two are distinguishable.
15. **SSH end-to-end (skip-gated).** With git unavailable and a real ssh-agent credential
    obtainable, `acquire_cred` returns the agent credential — `Ok(_)`, no error, no
    `GIT_MISSING_MSG`. Gate behind the existing "skip when unavailable" idiom; this test is a bonus,
    **not** the guarantee (see §7.1).
16. **SSH-only exhaustion guard (unconditional).** `allowed = SSH_KEY` only (no
    `USER_PASS_PLAINTEXT`), agent unavailable, git unavailable ⇒ exhaustion carries
    `CRED_EXHAUSTED_MSG`, **not** `GIT_MISSING_MSG` (`helper_git_unavailable` was never set because
    the Helper rung was never offered) ⇒ `map_remote_err` → `AppError::AuthFailed`, not
    `GitNotFound`.
17. **HTTPS positive case.** `allowed = USER_PASS_PLAINTEXT`, git unavailable, no agent ⇒ exhaustion
    carries `GIT_MISSING_MSG` ⇒ `AppError::GitNotFound`.
18. **Helper rung does not spawn (unconditional).** With `git_missing() == true`, the `Helper` rung
    performs **zero spawns** — assert via an injected `FillFn` that panics if called, or a spawn
    counter. This is what proves the ladder still reaches `SshAgent`.
19. `map_remote_err(git2::Error::new(Auth, Callback, GIT_MISSING_MSG), "origin")` →
    `AppError::GitNotFound`; message contains "NOT an authentication failure" and "SSH remotes using
    an ssh-agent are unaffected", and does NOT contain "cached credentials for this remote".
20. `map_remote_err` for a plain `ErrorCode::Auth` still → `AppError::AuthFailed` with the
    pre-P70 copy (regression guard).
21. `AppError::GitNotFound("x")` serializes to `{"kind":"gitNotFound","message":"x"}`.
22. `gitbin::spawn_error("log", &io::Error::from(ErrorKind::NotFound))` → `GitNotFound`;
    `spawn_error("log", &io::Error::from(ErrorKind::Interrupted))` with a resolvable git → `Git`.
23. `SpawnGitRunner` non-zero-exit message names the ACTUAL subcommand (`commit-graph`, not `log`).

**Frontend (vitest + harness):**
24. `checkGitAvailability` exists on `IpcApi`, in `tauri.ts`, and in the mock; the existing
    IPC-parity test (mock implements every `IpcApi` member) stays green.
25. `?git=missing`: `get_page_text` shows the banner with the §3.3 copy; a fetch attempt produces
    NO auth toast; console shows no unhandled rejection.
26. `?git=missing` → `recheck()` re-invokes and (mock still missing) keeps the banner.
27. Default (no `?git=`): no banner, zero extra IPC calls beyond the one probe.
28. `isGitNotFound` unit test over `{kind:'gitNotFound'}`, `{kind:'authFailed'}`, `null`, a string.

**Build gates:** `cargo clippy --workspace -- -D warnings` clean; `cargo test --workspace` green
with no net test-count regression; `tsc` + `vite build` clean; `registration_tests` green (proves
the new command is both defined and registered).

### 6.2 USER CHECKPOINT (native, `pnpm tauri dev` / installed build)

- **The actual bug**: on the reporting user's machine, with Git only in the User PATH, launch the
  MSI-installed Bonsai (or reproduce by launching `bonsai.exe` from a process with a Machine-only
  PATH) → the app resolves Git via HKCU and every feature works; commit search returns results;
  fetch/pull/push on an **HTTPS** remote authenticate through GCM as before.
- **The honest failure**: temporarily set `BONSAI_GIT_BIN` to a nonexistent path and relaunch → the
  banner appears once, the copy names the real problem, and a manual Fetch on an **HTTPS** remote
  does NOT produce the "no cached credentials" toast.
- **SSH not collateral damage (added 2026-08-19)**: with `BONSAI_GIT_BIN` still pointing at nothing
  and an ssh-agent loaded, fetch/pull/push against an **SSH** remote still succeed. The banner may
  be visible (search/signing genuinely are degraded), but no remote operation fails and no
  `gitNotFound` toast appears. **This checkpoint is the end-to-end half of the SSH guarantee** — it
  is not optional (see §7.1).
- Re-check button: install/point `BONSAI_GIT_BIN` at a real git while the app runs, press Re-check →
  banner clears, HTTPS remote ops work without restarting.
- First paint is not delayed (subjective): the window renders before/independently of the probe.
- macOS and Linux: normal launch resolves via PATH, no banner.

---

## 7. Notes for the other agents

### 7.1 Test plan sketch for `tester`

- **Hermetic ladder tests are the core.** Do NOT mutate `std::env` in unit tests — the suite runs
  in-process and in parallel, so an env write is a cross-test hazard (this is exactly why `GitEnv`
  exists). Build a `FakeGitEnv { vars: HashMap<String,String>, files: HashSet<PathBuf>, registry:
  HashMap<(String,String),String>, calls: RefCell<Vec<String>> }` and assert both the *result* and
  the *call order* (the "later rungs only when the earlier candidate is absent" invariant is an
  ordering assertion, not just a result assertion).
- **Simulating "PATH without git" deterministically**: two hermetic routes, both used —
  (a) unit level: `FakeGitEnv` with `resolve_on_path` returning `None` and an empty `files` set;
  (b) integration level (`crates/bonsai-core/tests/*.rs`): spawn nothing — instead set
  `BONSAI_GIT_BIN` to a path under `TempDir` that does not exist, via `Command::env` on a
  **separate process** (the `BONSAI_CLAUDE_BIN` precedent from P13). Never clear the parent
  process's PATH.
- **Stub-git for the `NoCredentials` vs `GitUnavailable` split**: a tiny script/binary written into
  a `TempDir` (`.cmd` on Windows, `.sh` + chmod elsewhere) that exits 0 with empty stdout, pointed
  at by `BONSAI_GIT_BIN`. Same technique already used for the `claude` stub.
- **The SSH-agent guarantee — read this before calling #15 a coverage hole.** There is deliberately
  **no injectable seam** over `Cred::ssh_key_from_agent` (D8): it would put indirection on the
  credential hot path for a marginal gain. The regression is pinned **unconditionally** by two
  hermetic tests plus one native checkpoint:
  - **#18** proves the `Helper` rung performs zero spawns when git is missing — i.e. it fails
    cheaply and the loop *continues* to `SshAgent` instead of short-circuiting.
  - **#16** proves an SSH-only exhaustion still maps to `AuthFailed`, never `GitNotFound` — the
    exact failure mode this design was corrected to avoid.
  - **§6.2 "SSH not collateral damage"** is the end-to-end proof (SSH fetch succeeds while the
    banner shows) and is a required checkpoint item.
  #15 (a live-agent `Ok(_)` assertion) stays **gated behind the existing skip idiom** and is a bonus
  on machines that happen to have an agent. A skipped #15 is NOT a coverage gap and must not be
  reported as one.
- **Do not** add a test that requires the machine to lack Git; the existing `have_git()` skip guards
  stay as they are.
- Frontend: extend `src/ipc/mock/urlSeams.test.tsx` with the `?git=missing` seam, and add a
  `useGitAvailability` hook test (probe once on mount, latch, recheck).

### 7.2 Suggested sub-increments for the orchestrator

1. `gitbin.rs` + `GitEnv`/ladder/cache/`git_command()` + unit tests + the 4 call-site migrations
   (backend only, no behaviour change beyond resolution).
2. `AppError::GitNotFound` + `FillOutcome`/`CredResolve` + `CredAttempts.helper_git_unavailable` +
   `acquire_cred`/`map_remote_err` + `spawn_error` wiring. **The SSH regression guards (6.1 #16 and
   #18) belong to this increment and are must-pass for it.**
3. `check_availability` + the Tauri command + TS types + mock handler + `useGitAvailability` +
   placeholder banner. (Then `ui-designer`, then the banner's real implementation.)

### 7.3 Explicitly left to `ui-designer` (`docs/contracts/P70-ui.md`)

Banner placement (above the tab strip vs inside the workspace), tone/severity token, iconography,
whether the Re-check button is primary or ghost, how `detail` is truncated/wrapped, whether the path
is copy-to-clipboard, empty/loading appearance while `status === null`, dark/light treatment,
a11y (role, focus order, whether it is an `alert` live region), and reduced-motion behaviour.

**Behaviour this contract fixes and UI must honour:**
- The banner is non-dismissable while `found === false`; it never appears when `found === true`; it
  renders nothing while `status === null`.
- **Its copy must not claim that fetch/pull/push are broken.** The accurate scope is: HTTPS remotes
  that resolve credentials through Git's credential helper, plus commit search, signing and hooks.
  SSH remotes using an ssh-agent keep working. (This is the §5.4 phrasing the ui-designer flagged.)

### 7.4 Frontend error routing (behaviour, not visuals)

- `src/ipc/errors.ts` exports `isGitNotFound(e: unknown): e is AppError` (narrow on
  `kind === 'gitNotFound'`).
- Every remote-op catch site that would push an error toast (`RepoWorkspace.tsx` fetch / pull /
  push / fetch-all handlers, `CloneDialog.tsx`) must, when `isGitNotFound(e)`, call
  `noteGitNotFound()` and **skip the toast** (decided 2026-08-19) — the persistent banner is the
  single surface, and suppressing the toast is what kills the "three repeated toasts" symptom.
  Errors of any other kind, including `authFailed` on an SSH remote, keep their existing toast.
- Background scheduler failures already stay silent (`RepoWorkspace.tsx` only toasts on
  `enteredBackoff`) — no change needed there.

---

## 8. Contracts referenced

`M6-remotes.md` §A (credential ladder, `credential_fill`), `P35-credential-cache.md` (cache seam),
`P49-external-integrations.md` (spawn idiom, `procutil`), `P42-packaging-autoupdate.md` (`?update=`
seam pattern), `P58-commit-signing.md`/`P59-hooks-and-lease-hardening.md` (`GitExec`),
`P50-search-command-palette.md` (`GitRunner`), `P13-ai-foundation.md` (`CLAUDE_BIN_ENV`,
`AiAvailability` shape).

---

## 9. Decisions — CLOSED 2026-08-19

All formerly-open questions were resolved by the orchestrator on 2026-08-19 and are recorded in
**§0 D7** (the original six) and **§0 D8** (no ssh-agent seam). Nothing in this contract is open.
The one substantive change made at the same time was the **§3.1 rewrite** removing the SSH-agent
short-circuit regression — that section, not this one, is authoritative for the credential path.
