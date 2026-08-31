# Pre-release testing & hardening campaign — summary (2026-08-09/10)

Read this first. Details in [FINDINGS.md](./FINDINGS.md) (every bug + fix) and
[COVERAGE.md](./COVERAGE.md) (numbers). Behavior changes made autonomously are in the
**FOR USER REVIEW** section at the top of FINDINGS.md — please skim those.

## What was done

Every public function/method in the Rust core, the Tauri command layer, the MCP servers, and the
React frontend was audited (9 ranked areas), issues fixed and documented, and the app covered by
unit, integration, component, end-to-end, property-based, and adversarial/corrupt-input tests —
including deliberately redundant "impossible-case" tests. 31 commits, `main` green at every step.
(`crates/bonsai-forge` + the forge PR UI were excluded — a separate session owns that uncommitted
work; the forge e2e spec and a few UI wirings are parked until it lands and `ipc/types.ts` unfreezes.)
*Update 2026-08-19: the parked forge e2e spec was written as `e2e/11-forge.spec.ts` (9 tests,
commit `83a9b2f`); the two remaining parked UI wirings (stash `expectedOid`, F-A7-7) are now
unblocked and tracked on `TODO.md`'s spun-out list.*

## Final gate (all green, 2026-08-10)

| Gate | Result |
|---|---|
| `cargo test --workspace` | **1571 passed, 0 failed**, 3 ignored (perf gates) |
| `cargo clippy --workspace --all-targets -D warnings` | clean |
| `pnpm test` (vitest) | **1317 passed, 0 failed** (109 files) |
| `pnpm build` (tsc + vite) | green |
| `pnpm exec playwright test` | **88 passed, 1 skipped** (forge, deferred); 3× consecutive, zero console errors |

Test growth over the campaign: Rust ≈1,208 → **~1,600** fns; frontend vitest 197 → **1,317**;
e2e 0 → **88**. Frontend statement coverage 3.98% → **61.18%** (remainder is the RepoWorkspace
container + GraphCanvas paint loop, both covered by e2e instead of unit tests). New test
infrastructure: jsdom+RTL vitest project, Playwright harness (msedge locally / chromium in CI),
`proptest`, strict `require_git!` (CI can no longer silently skip git-gated tests), v8 + llvm-cov
coverage wired.

Rust coverage (cargo llvm-cov, workspace): **88.3% lines / 90.1% regions / 75.6% functions** — the
uncovered remainder is declarative Tauri-runtime glue (`lib.rs`/`main.rs`) and the `#[tauri::command]`
wrapper shells whose logic is tested through the `_inner` seams.

## Bugs found & fixed (48 fixed, 2 documented-by-design, a few deferred)

Highlights by severity:

- **HIGH — Husky-repo commit/merge/push blocker** (F-A4-1): a missing hook under `core.hooksPath`
  made `git hook run` exit 1, which Bonsai misread as a rejection — blocking *every* commit, amend,
  merge, and push in any Husky-style repo. Fixed with `--ignore-missing`.
- **HIGH — stale-branch cleanup could delete the default branch** (F-A7-1): the base was protected by
  string comparison only, so passing it as `refs/heads/main` / an OID / a tag left `main` deletable.
  Now protected by resolved-ref identity; also re-checks tips at delete time (TOCTOU) and records the
  deleted tip for recovery.
- **HIGH — submodule remove path traversal** (F-A7-2): a hostile submodule name (`../../…`) flowed
  into `remove_dir_all` under `.git/modules` (CVE-2018-11235-style). Now validated + containment-checked.
- **MUST-FIX — stash data loss** (F-A6-A): stashing "staged only" when a file had a staged deletion +
  rewritten worktree content destroyed the new content. Now folded into the stash.
- **MUST-FIX — stash/autostash wrong-target** (F-A6-B, F-A7-6): destructive ops addressed a stale
  index; a stack change between render and confirm could drop/apply the wrong stash. Now oid-verified.
- **AI NL→git-operation planner**: proven (via an injection/adversarial-repo corpus) to never escape
  the SafeOp allowlist; model-supplied text is now sanitized (control/bidi strip + cap) and commit
  refs are hex-gated; sequencer corrupt-state no longer deadlocks the app; plain rebase-abort got the
  untracked-clobber guard the other sequencers already had.
- **Credentials** (F-A5-a/b): host-only cache keys could replay one org's token to another on the same
  host (now path-scoped under `useHttpPath`); a server-rejected credential stayed cached for the full
  TTL (now evicted).
- **MCP**: every response was serialized twice (now compact); tool counts could silently drift (now
  derived + drift-tested); a failed server bounce left a dead-but-"enabled" UI (now emits stopped).
- Plus history-index concurrency/ghost-doc, exec output-cap, several a11y/UI, and ~30 smaller items.

## ⚠️ Needs your decision (open items)

1. **F-T5-4 — RESOLVED for read surfaces (2026-08-19, commit `7edd23e`, audit #2 §3.2).** The
   recommended command-layer timeout was implemented as `run_with_git_timeout`
   (`bonsai-core/src/git/timeout.rs`: dedicated worker thread, 30 s inactivity deadline,
   `BONSAI_GIT_TIMEOUT_MS` override; on timeout the wedged worker is detached and the caller gets a
   clean `AppError::Git`). Wraps `get_status`, `get_graph`, `stream_graph` (channel now always
   terminates) and the history-index build. `corrupt_repo_cli.rs` C1 now pins **Err-not-Hung** for
   those read surfaces. `create_commit` is deliberately left UNWRAPPED — aborting a mutation on a
   false timeout could race a late commit; rationale recorded at the C1 cell.
   *(Original finding, kept for the record: a truncated HEAD loose commit object hung the app
   forever — libgit2 spins inflating truncated zlib; no bounded libgit2 probe detects it without
   also hanging.)*
2. **FOR USER REVIEW behavior changes** — see the top of FINDINGS.md. Each is strictly safer/more
   git-accurate and lists its one-line revert. Notably: AI planner rejects non-hash commit refs;
   clean merges now run the commit-msg hook; status shows worktree rename-to-untracked as
   delete+untracked (git parity); stale-cleanup is more conservative.
3. **Deferred (blocked, not bugs)** — submodule dirty-deinit force flag (F-A7-7) and any stash
   `expectedOid` UI wiring wait on `ipc/types.ts` unfreezing (the paused forge session owns it) —
   *the freeze lifted when forge landed; both are now tracked as OPEN on `TODO.md` (2026-08-19)*;
   `tag.gpgSign` (F-A7-8) documented as a known v1 limitation; F-A3-6 (Windows case-collision in the
   clobber guard) and F-A3-7 (bisect adjacent good/bad vs git) are NITs left as documented.

## Native USER CHECKPOINTs (things automation can't prove — please verify in `pnpm tauri dev`)

The browser harness + mock IPC verified all UI logic, but these need the real Tauri window / real
services: real forge PR flow against a live PAT; native OS dialogs (folder picker); real GPG/SSH
signing keys; the auto-updater against a signed release; window/scroll feel on a real 20k-commit repo;
and the F-A4-1 Husky fix against a real Husky repo on your machine.
