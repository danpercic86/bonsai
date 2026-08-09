# Findings log — testing campaign 2026-08

Bugs/oddities discovered while writing tests. One bullet per finding:
`- [phase] area — symptom — status (fixed in <commit> / open / by-design)`.

## FOR USER REVIEW — behavior changes made autonomously

- F-A2-2: AI operation planner now only accepts commit references as hex hashes (4-40 chars) from the
  model; revspecs like `HEAD~1` return "Unsupported" instead of resolving. Rationale: prompt only ever
  promises hashes from the grounding state; closes a defense-in-depth gap. Revert = drop the regex
  check in revparse_commit.

## Findings

(T1: none — infrastructure only.)

## T2 Area 1 — Tauri command layer (audit 2026-08-09)

- [T2.1] BUG-1 · LOW · repo.rs:227/166 — open_repo dedupe + remove_recent_repo compare paths with
  `eq_ignore_ascii_case`; non-ASCII case variants (e.g. `Übung`/`übung`) bypass dedupe → duplicate
  RepoEntry + double watcher; recents entry becomes unremovable — **fixed (pending commit)**: verified
  `read_repo_info` does NOT canonicalize (returns the raw input string) so the bug was real; both sites
  now use `same_repo_path` (fs::canonicalize compare, ASCII-case fallback when a side is unresolvable,
  e.g. deleted recents dir) · behavior change? Y (better dedupe for separator/non-ASCII variants) ·
  test: `open_repo_dedupes_canonical_path_variants`
- [T2.1] BUG-2 · MED(testability) · history.rs:89-148 — history_index_build/status/history_search have
  no runtime-free `_inner` seam (AppHandle-bound), untestable under the inner-fn pattern — **fixed
  (pending commit)**: `_inner(state, base, ...)` seams extracted, Channel → `impl Fn(IndexProgress)`
  callback · behavior change? N · test: `history_index_inner_seams_guard_and_report_unbuilt`
- [T2.1] BUG-3 · LOW · mcp.rs:46-66 — register_mcp_with_claude takes a raw frontend path (not repo_id via
  repo_path guard), no existence precheck → raw OS spawn error on deleted repo dir — **fixed (pending
  commit)**: `resolve_register_cwd` precheck (`AppError::Io`, same shape as `read_repo_info`); IPC
  signature unchanged (types.ts frozen) · behavior change? Y (clean error instead of raw spawn error) ·
  test: `resolve_register_cwd_prechecks_repo_path`
- [T2.1] BUG-4 · LOW · config.rs:89-108 — apply_identity_profile has no `_inner` seam; command body has
  zero command-layer coverage — **fixed (pending commit)**: mechanical `_inner` seam · behavior change?
  N · test: `apply_identity_profile_inner_norepo_and_happy_path`
- [T2.1] NIT · repo.rs:83 — lossy `as i64` on epoch seconds — **fixed (pending commit)**:
  `i64::try_from(...).unwrap_or(i64::MAX)` · behavior change? N (unreachable until year 292e9)
- [T2.1] NIT · scheduler.rs:73/92 — "job already running" is stringly AppError::Other (untyped for
  frontend) — by-design candidate
- [T2.1] NIT · external.rs:24-42 — launch commands accept any frontend-supplied path (documented as
  intentional) — accepted-risk note
- [T2.1] Structural: consent gates, spawn_blocking, no-unwrap, event conventions all verified sound
  across 32 command files / 156 commands.

## T2 Area 2 — AI NL→operation safety (audit 2026-08-09)

- [T2.2] F-A2-1 · SHOULD-FIX · ai_operation_resolve.rs:36 + reason interpolations — model-supplied
  strings (Unsupported.reason, branch/commit/name echoes) reach the dialog verbatim: unbounded size,
  control/RTL chars, social-engineering text. Fix: cap ~200 chars + strip control chars for any
  model-derived substring — **open**
- [T2.2] F-A2-2 · SHOULD-FIX · ai_operation.rs:334 revparse_commit accepts arbitrary revspecs
  (HEAD~50, @{2.days.ago}, :/pattern) though prompt promises short-hash-from-state. DECISION
  (orchestrator, 2026-08-09): harden — restrict model-supplied commit/atCommit to hex [0-9a-f]{4,40}.
  Behavior change: model revspec answers now → Unsupported (preview already showed real target, so
  user-visible impact is nil) — **open, FOR USER REVIEW entry below**
- [T2.2] F-A2-3 · SHOULD-FIX · ai_operation_preview.rs:162-165 — Discard preview warning joins every
  kept path unbounded → potential MB-scale IPC/dialog payload. Cap like MAX_PREVIEW_DROPPED — **open**
- [T2.2] F-A2-4 · coverage gap — ai_operation_grounding.rs + ai_operation_preview.rs have 0 tests;
  injection-containment claim asserted only in a doc comment — **open (tester)**
- [T2.2] NIT — add serde deny_unknown_fields to AiOpIntent (matches "off-schema ⇒ Unsupported"
  design); HashSet for resolve_discard_changes path scan; grounding revwalk silent truncation
  (quality-only, leave); TOCTOU plan→confirm documented as accepted (exec-time guards limit blast
  radius) — **open (fold into fix pass)**
- [T2.2] Verified sound: allowlist non-escapable (typed AiOpIntent only), destructive classification
  consistent, preview derived from parsed op, execution impossible without dialog Confirm, ref-name
  argument injection inert (git2 APIs, no argv).
