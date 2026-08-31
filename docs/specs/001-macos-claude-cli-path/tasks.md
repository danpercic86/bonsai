# Tasks: GUI-launched Bonsai can't find the `claude` CLI on macOS/Linux

**Plan:** ./plan.md
**Status:** done

- [x] 1. Implement the macOS/Linux resolution ladder: new `crates/bonsai-core/src/ai/bin_resolve.rs`
      (`resolve`, `current_path_dirs`, `login_shell_path_dirs` (`OnceLock`-cached),
      `probe_login_shell_path`, `fallback_dirs`, `is_executable_file`, `find_in`) exactly as shaped
      in `plan.md`'s "New files" section — `crates/bonsai-core/src/ai/bin_resolve.rs` — owner:
      senior-dev
- [x] 2. Wire it in: `mod bin_resolve;` (gated `#[cfg(not(windows))]`) and rewrite `resolve_bin()`
      to branch Windows (unchanged `procutil::resolve_program` call) vs. non-Windows
      (`bin_resolve::resolve("claude")`), per `plan.md`'s "Files touched" — `crates/bonsai-core/src/ai/mod.rs`
      — owner: senior-dev
- [x] 3. Review tasks 1-2's diff: correctness of the search-order ladder, the timeout/kill logic
      against `run_process`'s existing pattern in `ai/mod.rs`, that Windows behavior (AC5) and the
      `CLAUDE_BIN_ENV` override (AC6) are provably untouched, and that nothing here calls
      `std::env::set_var` or otherwise touches global process state — owner: reviewer
- [x] 4. Write and run the test suite from `plan.md`'s "Testing" section: `find_in`/
      `is_executable_file` cases (temp dirs via `tempfile`), a stub login-shell fixture script
      (sibling to `tests/fixtures/claude_stub.sh`, following its executable-bit-forcing pattern)
      exercising `probe_login_shell_path`'s parsing + last-line defensiveness + timeout/kill path,
      and an end-to-end `resolve()` case proving the ladder finds a binary that's only reachable
      via the shell-probe/fallback tiers, not the direct `PATH` search. All env-mutating cases MUST
      take `ai::testutil::env_lock()` for the duration. Then run the full `cargo test -p
      bonsai-core` suite and confirm no regressions (AC6 in particular) — new test module in
      `crates/bonsai-core/src/ai/bin_resolve.rs` (or a sibling `bin_resolve_tests.rs` if that
      keeps the file within the ~500-line soft limit better) + a new fixture under
      `crates/bonsai-core/tests/fixtures/` — owner: tester
- [ ] 5. Orchestrator: `cargo check -p bonsai-core` / `cargo clippy`, integrate, commit
      `wip(spec-001): resolve claude CLI via login-shell PATH fallback on macOS/Linux`, then ask
      the user to rebuild and relaunch Bonsai **by double-click** (the exact repro) and confirm an
      AI feature now works — this is the one part of the fix nothing here can verify without the
      real GUI launch path.

## Notes
- Pass `docs/specs/001-macos-claude-cli-path/plan.md` (and `spec.md` for the acceptance criteria
  numbers) by **file path** to every subagent — the plan already contains the concrete function
  shapes, search order, and rationale; no subagent should re-derive the design.
- Task 1 also names `crates/bonsai-core/src/winenv.rs` in the plan as the closest precedent for
  conventions (append-only search order, silent best-effort failure, doc-comment style) — point
  senior-dev at it, but flag explicitly that this fix does NOT follow winenv's "run at startup"
  placement (see plan.md's "Approach" for why: lazy + cached inside `resolve_bin()`, called only
  from existing `spawn_blocking` contexts).
- Tasks 1-2 are not parallelizable — task 2's `resolve_bin()` rewrite depends on task 1's module
  existing (same file-touch dependency the plan lays out), so hand both to one senior-dev pass
  rather than splitting them.
- `ai/mod.rs` is already over the 500-line soft limit before this change (plan.md's Risks) — do
  not fold a mod.rs split into this fix; that's a separate follow-up if the team wants one.
