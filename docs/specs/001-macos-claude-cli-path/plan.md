# Plan: GUI-launched Bonsai can't find the `claude` CLI on macOS/Linux

**Spec:** ./spec.md
**Status:** done

## Approach
`ai::resolve_bin()` currently resolves `claude` by handing the bare name to `Command::new`,
which only searches whatever `PATH` the OS launcher gave the process — on a GUI launch
(double-click/Spotlight/Dock) that's a minimal launchd default that omits anything a user's shell
only adds via `.zshrc`/`.zprofile`/`.bashrc`. Fix: when `claude` isn't found on the process's own
inherited `PATH`, probe the user's **login shell**'s `PATH` (`$SHELL -ilc 'echo $PATH'`, the
standard technique other GUI apps use for this exact problem) and search that too, falling back
to a short list of well-known install directories if the shell probe itself fails. Cache the
result for the process's lifetime so this only runs once, and only the first time `claude` isn't
found where it should be (spec AC4).

This codebase already solved the sibling Windows problem — an app launched with a foreign/stale
environment missing user-only `PATH` entries — in `crates/bonsai-core/src/winenv.rs` (P71 R2,
contract `docs/contracts/P71-updater-relaunch-env.md`). Read that module's doc comment before
implementing; it's the closest precedent for shape and conventions (append-only, silent
best-effort, injectable seam for hermetic tests). **One deliberate divergence from it**, called
out because it's easy to "helpfully" copy the wrong half: winenv's fix runs synchronously as the
*first statement of `run()`*, before the window ever paints, because a registry read is cheap.
Spawning a full login shell is not — a heavy `.zshrc` (nvm, oh-my-zsh, etc.) can take hundreds of
ms to over a second. Doing that at startup would violate this spec's "no noticeable startup
delay" goal. So this fix stays **local to the `ai` module and lazy**: it only runs inside
`resolve_bin()`, already called from `spawn_blocking` contexts, and only on the failure path
where the cheap inherited-`PATH` lookup already missed — never at app launch, never for users
where discovery already works today (AC2). It also never calls `std::env::set_var`, so none of
winenv's single-threaded-`set_var`-soundness constraints apply here.

Rejected alternative: extending the shared `crate::procutil::resolve_program` (used by `gitbin`
and `external.rs` too) instead of adding `ai`-local logic. Rejected per the spec's explicit
non-goal — this fix is scoped to the `claude` CLI discovery path only; broadening the shared
helper's behavior for `git`/external-tool launching is a separate decision with its own
risk/review surface (`external.rs` launches user-facing external programs) that shouldn't ride
along with this fix.

## Rust/TS boundary
Rust-only. No IPC surface change — `check_ai_availability` and every AI command already call into
`ai::resolve_bin()` indirectly; this changes what that function returns, not its callers' shapes.
Nothing for React to know about; `src/ipc/mock.ts` and the fixtures under `src/ipc/mock/handlers/`
are unaffected (the mock layer never spawns a real `claude`, so the "not found" fixture state
stays exactly as it is today, satisfying AC3's frontend side for free).

## Files touched
- `crates/bonsai-core/src/ai/mod.rs` — add `mod bin_resolve;` (gated `#[cfg(not(windows))]`, it's
  a Unix-shell technique); rewrite `resolve_bin()` to branch: Windows keeps today's
  `procutil::resolve_program` call unchanged (AC5), non-Windows delegates to
  `bin_resolve::resolve("claude")`. Update `resolve_bin`'s doc comment to describe the new
  ladder. ~15-20 line net diff. **Already over the 500-line soft limit today (673 lines)** — this
  fix adds a small, self-contained amount to it rather than shrinking it; not this fix's job to
  fix, flagged below under Risks.

## New files
- `crates/bonsai-core/src/ai/bin_resolve.rs` — macOS/Linux-only resolution ladder, module-private
  to `ai` (no `pub`, only `resolve_bin()` calls it). Roughly:
  - `resolve(program: &str) -> PathBuf` — the public entry point: search the process's own
    current `PATH` first (same outcome `Command::new` alone would reach — AC2 unchanged
    behavior), then the cached login-shell `PATH`, then the fallback dirs, then give up and
    return the bare `program` name unresolved so the existing spawn-`NotFound` →
    `AppError::AiUnavailable` path still fires naturally (AC3), exactly mirroring
    `procutil::resolve_program`'s documented Windows-fallback convention.
  - `current_path_dirs()` — reads `std::env::var_os("PATH")`, split via `std::env::split_paths`.
  - `login_shell_path_dirs() -> &'static [PathBuf]` — `OnceLock`-cached (AC4: one probe per
    process, not per call), delegates to `probe_login_shell_path()`.
  - `probe_login_shell_path() -> Option<Vec<PathBuf>>` — reads `$SHELL` (fallback `/bin/zsh` if
    unset), spawns `<shell> -ilc "echo $PATH"` with stdin/stderr discarded, polls `try_wait()`
    against a short deadline (propose 2s — generous for a slow rc file, bounded so a hung/broken
    shell can't stall an AI feature per the spec's edge case), kills+reaps on timeout exactly like
    `run_process`'s deadline loop in `ai/mod.rs` does (same `try_wait`/sleep/kill pattern — no
    need for the concurrent reader threads that pattern also uses, since `echo $PATH`'s output is
    tiny and read once after exit). Takes the LAST non-empty stdout line (defensive against a
    startup banner/plugin `echo` landing before it) and `std::env::split_paths` it. Any failure
    (spawn error, non-zero exit, timeout, empty output) → `None`, so callers fall through to
    `fallback_dirs()`.
  - `fallback_dirs() -> Vec<PathBuf>` — last-resort well-known install locations for when the
    shell probe itself fails (broken `$SHELL`, edge case in spec): `~/.local/bin` (this project's
    own repro case, also the Claude Code standalone installer's default), `~/.claude/local`
    (older installer layout), `/opt/homebrew/bin` (Apple Silicon Homebrew — not on launchd's
    default `PATH` either), `/usr/local/bin` (Intel Homebrew / most manual installs). Best-effort,
    not exhaustive — the login-shell probe is the primary mechanism.
  - `is_executable_file(&Path) -> bool` — `unix::fs::PermissionsExt` mode-bit check (mirrors
    `testutil.rs`'s existing executable-bit handling), not just `is_file()`, so a same-named
    non-executable file never wins.
  - `find_in(dirs, program) -> Option<PathBuf>` — shared helper for all three search passes.

## Data model / types
No new public types, no serde shapes, nothing crossing the IPC boundary. Everything here is
`PathBuf`/`Option`/`Vec` plumbing internal to `bin_resolve.rs`.

## Testing
Unit tests in `bin_resolve.rs` (or a sibling `bin_resolve_tests.rs` if the file would otherwise
cross ~250-300 lines combined with tests, matching the existing `session_tests.rs` /
`session_io_tests.rs` split convention):
- `find_in`/`is_executable_file`: a `tempfile::tempdir()` with an executable file, a non-executable
  file, and a missing name — asserts the right one wins / `None` on miss (dev-dependency
  `tempfile` is already available per `Cargo.toml`).
- `probe_login_shell_path`: point `$SHELL` at a small stub script (fixture, alongside the existing
  `tests/fixtures/claude_stub.sh` pattern) that echoes a known fake `PATH` — asserts it's parsed
  correctly, including the "last line" defensiveness (stub prints a banner line first, then the
  `PATH` line). A second stub that `sleep`s past the deadline asserts the probe times out and
  returns `None` rather than hanging the test.
- `resolve()`: end-to-end with a temp dir standing in for a "shell-only" location not on the
  current process `PATH`, proving the ladder finds it there when the direct `PATH` search misses,
  and that `CLAUDE_BIN_ENV` (unrelated, tested at the `ai::resolve_bin` level, not here) still
  wins when set.
- **Env-mutating tests MUST take `ai::testutil::env_lock()`** for the duration (same reason
  `CLAUDE_BIN_ENV`/`STUB_MODE_ENV` tests already do — `$SHELL`/`PATH` are process-global and the
  probe's child inherits them, so parallel tests would otherwise race).
- Regression check: existing `ai::tests`/`ai::session_tests` cases that rely on `CLAUDE_BIN_ENV`
  overriding resolution must still pass unchanged (AC6) — no new test needed, just confirm the
  full `cargo test -p bonsai-core` suite stays green.
- No frontend/mock-IPC changes needed (nothing observable through `VITE_MOCK_IPC=1` changes), so
  no browser-harness verification step for this fix.

## Risks / open questions
- `crates/bonsai-core/src/ai/mod.rs` is already over the 500-line soft limit (673 lines) before
  this change. This fix adds a small, self-contained amount to an already-oversized file rather
  than making it worse in a load-bearing way — CLAUDE.md asks that a file crossing the limit get
  split *in the same increment*, but a full split of a 670+ line pre-existing file is a
  substantially larger, unrelated undertaking than this bug fix. Recommend flagging it separately
  (e.g. a `refactorer` follow-up) rather than folding a mod.rs reorganization into this fix.
- `-ilc` (interactive + login) is the standard technique for this problem (it sources both
  `.zprofile`/`.zlogin`-style login files and `.zshrc`/`.bashrc`-style interactive files, since a
  user's `PATH` addition could live in either), but it means an interactive shell without a TTY —
  harmless in the confirmed case, but a sufficiently exotic shell config (a plugin that writes to
  stdout on startup, or that hangs waiting on a TTY) could still confuse or stall the probe. The
  timeout bounds the "stall" case (falls through to `fallback_dirs()`/bare name); a confused
  parse just yields no extra directories found, i.e. degrades to today's behavior, never worse.
- Precedence when a user has multiple `claude` installs is whichever is found first by the ladder
  order above (current `PATH` → login-shell `PATH` → fallback dirs) — spec left exact precedence
  to `/plan`; this order was chosen because it matches "trust what already resolves things
  normally first, then trust the shell's own idea of `PATH` order, then guess."
- This is scoped to macOS/Linux (`#[cfg(not(windows))]`) per the spec; Linux desktop-launcher
  environments (`.desktop` files via a display manager) have the same class of problem as macOS
  and get the same fix for free, which is in-scope and desired, not scope creep.

This is small and self-contained enough (one new ~120-150 line module + tests, a ~15-20 line
change to one existing function, no IPC/UI surface) to hand straight to `senior-dev` via
`/tasks` — no need to escalate to a full milestone or the `architect` agent.
