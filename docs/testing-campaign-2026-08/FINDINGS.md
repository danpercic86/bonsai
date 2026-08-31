# Findings log — testing campaign 2026-08

Bugs/oddities discovered while writing tests. One bullet per finding:
`- [phase] area — symptom — status (fixed in <commit> / open / by-design)`.

## FOR USER REVIEW — behavior changes made autonomously

- F-A7-8: **annotated tags now honour signing** (previously never signed — a documented divergence
  from `git tag -a`/`git tag -s`). An annotated tag is now SIGNED when git config `tag.gpgSign=true`
  OR the new optional `sign` flag is `true`, via `git tag -s` through the same exec seam as commit
  signing (respects `gpg.format` / `user.signingkey` / `gpg.program` | `gpg.ssh.program`).
  **Lightweight tags are never signed** (git parity). Signing requested with `gpg.format=ssh` and no
  `user.signingkey` now returns a clear `ConfigMissing` (no unsigned tag is created silently), exactly
  like commit signing. The unsigned annotated path is byte-unchanged. The `create_tag` IPC command
  gained an optional `sign?: boolean` (absent ⇒ config-driven, wire-compatible with the frozen
  `types.ts`). Revert = drop the `sign` param + the `signing::create_signed_tag` branch in
  `git/tags.rs::create_tag`.
- F-T5-3: working-dir **status now matches `git status` porcelain for worktree renames-to-untracked**.
  Deleting a tracked file and creating an untracked file with identical bytes previously showed as a
  single unstaged **rename** row (git2 rename-detected the untracked destination); it now shows as
  **two rows — an unstaged delete of the original + an untracked new file** (exactly what
  `git status` reports). Implemented by turning OFF `StatusOptions::renames_index_to_workdir`
  (staged-rename detection via `renames_head_to_index` is unchanged, so `git mv` staged renames still
  show as renames). Revert = set `renames_index_to_workdir(true)` in `git/status.rs::read_status`.
- F-T5-4 **(update 2026-08-19: the recommended timeout was implemented in commit `7edd23e`, audit
  #2 §3.2 — see the F-T5-4 entry below for the resolved status)**: a repository with a
  **truncated/corrupt loose HEAD
  commit object hangs the app forever** (libgit2 spins inflating the truncated zlib during any
  HEAD-peel — graph/status/commit). A bounded pre-check was investigated and rejected:
  `git2::Odb::read_header`/`exists` return a healthy-looking result on the truncated object (the
  header inflates before the truncation point), so they cannot gate it. The real fix is a deliberate
  architecture decision for you: wrap heavy git2 reads at the Tauri command layer in a bounded
  timeout and surface a clean `AppError` instead of freezing. Left un-hacked per instructions; C1
  corrupt-matrix test still pins the current `Hung` behavior. Details in the F-T5-4 entry below.
- F-A8-a: MCP success responses no longer echo the full JSON payload in the `content` text block —
  it now carries a compact summary (e.g. `{nodes, edges, headIndex}` / `[N items]`). The complete
  data is still in `structured_content` (what every MCP client should read). A client that instead
  parsed the `content` text as JSON would break; none of ours do. Revert = restore
  `CallToolResult::structured(value)` in `ok_json`.
- F-A8-d NIT: MCP `bonsai_select_repo` on a standalone (`--repo`) server now returns error kind
  `invalidName` (was `other`), matching the unknown-repo rejection. Wire-visible `kind` change.
- F-A8-d NIT: `bonsai-mcp --help` now prints usage to STDOUT and exits 0 (was stderr + exit 1).
- F-A2-2: AI operation planner now only accepts commit references as hex hashes (4-40 chars) from the
  model; revspecs like `HEAD~1` return "Unsupported" instead of resolving. Rationale: prompt only ever
  promises hashes from the grounding state; closes a defense-in-depth gap. Revert = drop the hex gate
  at the top of revparse_commit.
- F-A3-1: plain `rebase_abort` now refuses (retryable AppError, rebase state intact) when the abort's
  hard reset would overwrite an untracked file present in the orig-head tree — same 46a34d4 guard
  bisect/interactive already run. Revert = drop the `ensure_no_untracked_collision` call in
  rebase.rs::rebase_abort.
- F-A3-2: a CORRUPT (undecodable) bonsai sequencer state file no longer deadlocks the app —
  `bisect_reset` / `interactive_abort` (and `rebase_abort` via delegation) now clear the state dir,
  leave HEAD in place, and return a distinct explanatory error instead of failing forever while all
  mutations stay blocked. Missing-state and io-error behavior unchanged. Revert = drop the
  `StateReadError::Corrupt` salvage arms.
- F-A4-2: clean (non-conflict) merge commits now run the commit-msg hook — ONLY that hook (previously
  NO hooks ran on clean merges). Matches real git's message-policy behavior; pre-merge-commit remains
  unsupported and is now documented as such. If the hook rejects, the merge is left PAUSED (MERGE_HEAD
  retained, HEAD unchanged — recover via "commit merge" with skip-hooks, or abort), which is exactly
  git's "Not committing merge; use 'git commit' to complete the merge" state. `merge_branch` gained a
  `skip_hooks` param (command arg `skipHooks` optional, absent ⇒ false — wire-compatible; UI wiring
  deferred, types.ts frozen). Revert = pass MergeHooks::Off again in merge_branch's finalize call.
- F-A4-4 (documented default, behavior unchanged): AI-composer split commits bypass ALL git hooks
  (deliberate — re-staging pre-commit hooks would corrupt the split-plan partition; commit-msg would
  rewrite generated messages). Consequence: commit-message policy hooks do not vet composer commits.
  Now documented in compose_apply.rs + P59 user checklist "Known v1 hook divergences". Revisit
  commit-msg-only execution if desired.
- F-A6-A: `Staged` stash of a staged DELETION whose file was rewritten on disk (`git rm --cached` +
  edit) now FOLDS the rewritten worktree content into the stash (the staged deletion is subsumed,
  same FOLD rule as mixed staged+unstaged edits) instead of losing the content entirely. Plain
  `rm --cached` (file == HEAD) still stashes the deletion. Revert = drop the differs-from-HEAD fold
  branch in create_staged_stash's delete arm.
- F-A6-E: applying/popping a stash that contains a non-UTF-8 path now errors ("non-unicode path",
  stash retained) instead of silently omitting that path from the reserved-path preflight/allowlist.
- F-A7-6: after merge/cherry-pick/revert with autostash, the autostash is re-applied/dropped BY
  COMMIT OID, not "stash@{0}"; if it was dropped externally mid-operation the op reports "your
  autostashed changes were not found; check `git stash list`" instead of applying a foreign stash.
- F-A7-1/3/4/5 (stale-branch cleanup, safety-tightening): base branch now protected under ANY
  spelling (refname/OID/tag); remote base protects its local counterpart; the repo default branch is
  never offered as stale; deletion re-checks the tip and refuses with "tip moved since scan"; Deleted
  rows now carry "was at <short-oid>" for manual recovery. Strictly more conservative — no previously-
  safe deletion is blocked except the tip-moved race.
- F-A3-1 (queued): plain rebase Abort will refuse (retryable) instead of silently overwriting an
  untracked file that collides with the original tip — matching bisect/interactive abort semantics.
- F-A5-b: on an operation-level auth failure (fetch/push) of a FRESHLY-filled credential (cache miss
  or a post-rejection bypass re-fill), Bonsai now EVICTS that just-stored in-process entry instead of
  re-serving it for the full 10-min TTL. Effect: the next fetch/push re-consults the configured
  credential helper (which re-prompts/re-issues per its own policy) rather than double-failing on the
  known-bad cred. We deliberately do NOT call `git credential reject` — that would require handing the
  helper the exact plaintext, which `cred_cache` intentionally never surfaces back to `remote.rs`;
  eviction is the safe, sufficient equivalent. No effect on a successful op or a first cache-HIT (the
  existing one-shot RetryAllowed bypass already covers hit-then-reject). Revert = drop the
  `evict_fresh_on_auth_fail` calls in `remote.rs::fetch_remote`/push and the `fresh_fill_url` field.

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
  control/RTL chars, social-engineering text — **fixed (pending commit)**: new
  `sanitize_model_text` (ai_operation.rs) — \n/\t → space, all other C0/C1 controls stripped, bidi
  override/isolates U+202A–E + U+2066–69 stripped, char-boundary cap 200 + `…` — applied at the
  Unsupported passthrough, every resolver echo site, and the stash-message preview · tests:
  `sanitize_model_text_truth_table`, `model_echoes_are_sanitized`,
  `stash_message_is_sanitized_in_preview`
- [T2.2] F-A2-2 · SHOULD-FIX · ai_operation.rs:334 revparse_commit accepts arbitrary revspecs
  (HEAD~50, @{2.days.ago}, :/pattern) though prompt promises short-hash-from-state. DECISION
  (orchestrator, 2026-08-09): harden — restrict model-supplied commit/atCommit to hex [0-9a-f]{4,40} —
  **fixed (pending commit)**: hex gate (case-insensitive, 4–40 chars) BEFORE revparse; non-matching →
  None → the existing "couldn't find a commit" Unsupported. PLAN_SYSTEM_PROMPT already says
  hashes-from-state-only — unchanged. Behavior change: Y, FOR USER REVIEW entry above · test:
  `revparse_commit_is_hex_gated`
- [T2.2] F-A2-3 · SHOULD-FIX · ai_operation_preview.rs:162-165 — Discard preview warning joins every
  kept path unbounded → potential MB-scale IPC/dialog payload — **fixed (pending commit)**: lists at
  most MAX_PREVIEW_DROPPED (20) paths + "(+N more)" · test: `discard_warning_caps_listed_paths`
- [T2.2] F-A2-4 · coverage gap — ai_operation_grounding.rs + ai_operation_preview.rs have 0 tests;
  injection-containment claim asserted only in a doc comment — **fixed**: inline test modules added
  to both + tests/ai_operation_safety_cli.rs (injection/malformed/adversarial-repo corpus proving the
  planner never escapes the SafeOp allowlist)
- [T2.2] NIT — **fixed (pending commit)**: `deny_unknown_fields` added to AiOpIntent (verified
  effective on the internally-tagged enum — an extra field now fails the parse ⇒ Unsupported; test
  `extra_fields_fail_closed_to_unsupported`); HashSet dedup in resolve_discard_changes. Still
  by-design/left: grounding revwalk silent truncation (quality-only), TOCTOU plan→confirm accepted
  (exec-time guards limit blast radius)
- [T2.2] Verified sound: allowlist non-escapable (typed AiOpIntent only), destructive classification
  consistent, preview derived from parsed op, execution impossible without dialog Confirm, ref-name
  argument injection inert (git2 APIs, no argv).

## T2 Area 3 — bisect + rebase sequencers (audit 2026-08-09)

- [T2.3] RESOLVED-HISTORICALLY — the "known bug" rebase skip-first-op/msgnum corruption was already
  fixed in 8219ebd (paths-only reset instead of git_reset(HARD), which deletes rebase-merge state);
  the test at rebase_cli.rs:546 is ACTIVE and green — only stale comments claim it's #[ignore]d.
  No fix needed; doc cleanup = F-A3-5.
- [T2.3] F-A3-1 · MEDIUM · rebase.rs:503-505 — plain rebase_abort is the ONLY force-checkout path
  without the 46a34d4 untracked-clobber guard (libgit2 rebase.abort() hard-resets); untracked file
  matching an orig-head path is silently overwritten — **fixed (pending commit)**:
  `ensure_no_untracked_collision(orig-head tree)` before `rebase.abort()` (orig oid via
  `rebase.orig_head_id()`, fallback `.git/rebase-{merge,apply}/orig-head`; undeterminable → unguarded
  abort, no worse than before); refusal leaves rebase state intact (retryable, same wording as
  bisect/interactive). Behavior change: Y, FOR USER REVIEW entry above · test:
  `plain_rebase_abort_refuses_untracked_clobber_then_retries` (tests/sequencer_salvage_cli.rs)
- [T2.3] F-A3-2 · MEDIUM · bisect.rs:493 + rebase_interactive.rs:632 — corrupt/truncated bonsai
  sequencer state.json ⇒ in-app deadlock: reset/abort fail on parse, require_no_bisect (existence-
  only) blocks all mutations, opstate shows None → no UI escape — **fixed (pending commit)**: salvage
  arm in bisect_reset + interactive_abort (plain rebase_abort delegates on file existence → covered):
  on a PARSE failure (file exists, undecodable) the state dir is removed, HEAD left in place.
  CHOICE: a distinct `AppError::Git` explaining corruption/clearing/HEAD-left-in-place (the
  `Result<(), _>` return has no Ok-message channel, so an error toast is the cleaner fit).
  Missing-state behavior unchanged. Behavior change: Y, FOR USER REVIEW entry above · tests:
  `corrupt_bisect_state_is_salvaged_by_reset`, `corrupt_interactive_state_is_salvaged_by_abort`,
  `missing_bisect_state_still_reports_no_operation`
- [T2.3] F-A3-3 · LOW · bisect.rs:341 + rebase_interactive.rs:264 — cross-sequencer start guards
  asymmetric (start_bisect ignores interactive state; interactive start ignores bisect) — **fixed
  (pending commit)**: symmetric cross-checks, each naming the other operation · test:
  `cross_sequencer_start_guards_are_symmetric`
- [T2.3] F-A3-4 · NIT · bisect.rs:111 — io errors reported as "state missing" — **fixed (pending
  commit)**: `StateReadError{Missing,Io,Corrupt}` in BOTH sequencers; NotFound → "missing", other io
  → "failed to read … state: {real error}" (and io errors do NOT trigger salvage) · test:
  `unreadable_bisect_state_surfaces_real_io_error`
- [T2.3] F-A3-5 · NIT · rebase_cli.rs:467/535 — stale known-bug comments + misleading test name —
  **fixed (pending commit)**: `skip_first_op_is_broken_known_bug` →
  `skip_first_conflicting_op_works`; both comment blocks rewritten to describe the 8219ebd fix
  (paths-only reset preserves rebase-merge state; branch field now asserted too)
- [T2.3] F-A3-6 · NIT · stage.rs:174 — clobber guard tree lookup is exact-case; Windows case-collision
  false negative; adversarial-test then fix-or-document — **fixed (pending commit)**:
  `ensure_no_untracked_collision` now reads `core.ignorecase` (git2 Config; unset falls back to
  `cfg!(windows)`) and, when case-insensitive, walks the target tree with ASCII case-folded name
  comparison (`ci_path_collides`), preserving the Direct + Type-swap logic; exact-case path unchanged
  when ignorecase=false. Tests (inline in stage.rs, config-forced so deterministic cross-platform):
  `ignorecase_untracked_case_variant_is_detected` (untracked `README.md` vs tree `readme.md` now
  REFUSED), `case_sensitive_case_variant_is_not_a_collision` (ignorecase=false keeps them distinct),
  `exact_case_match_is_a_collision_both_modes`. Evidence: `cargo test -p bonsai-core` 1193 passed / 0
  failed / 3 ignored; `cargo clippy -p bonsai-core --all-targets -- -D warnings` clean. Behavior
  change: on a case-insensitive FS a rebase/bisect/interactive-abort force-checkout now REFUSES when
  an untracked case-variant would be clobbered (previously silent overwrite).
- [T2.3] F-A3-7 · NIT · bisect.rs:414 — adjacent good/bad refuses vs git's immediate verdict (P39
  contract-sanctioned); pin with test — **open (tester)**
- [T2.3] Verified sound: atomic state writes, start rollback, autostash apply-not-pop discipline,
  interactive M1/M2/S1 edge handling, opstate priority, no reachable panics; rebase refuses dirty
  start by design (no autostash on rebase paths).

## T2 Area 4 — git hooks execution (audit 2026-08-09)

- [T2.4] F-A4-1 · HIGH · hooks.rs:161 build_hook_run_args — absent hook under core.hooksPath BLOCKS
  the operation: `git hook run` exits 1 ("cannot find a hook") and Bonsai maps it to HookRejected →
  in a Husky-style repo missing one of pre-commit/commit-msg/pre-push, EVERY commit/amend/merge/push
  is blocked (empirically verified, git 2.51). Module doc + P59 contract wrongly claim hook-absent ⇒
  Ok. Fix: add --ignore-missing to argv (+ doc corrections). Also restores git parity for present-
  but-non-executable hooks on unix — **fixed (pending commit)**: `--ignore-missing` always in the
  `git hook run` argv (same ≥2.36 floor; pre-2.36 unknown-subcommand path unchanged) + `plan_hook`
  now resolves `core.hooksPath` like git (git2 `get_path` tilde expansion; relative ⇒ worktree
  root) and skips the spawn when the hook file is absent, so `run_hook_nonblocking` reports
  `ran:false`; a doubtful resolution still delegates to git (--ignore-missing backstops). Docs
  corrected: hooks.rs module doc, run_hook doc, P59 contract L26. Regression tests (RED first —
  all 3 failed with `HookRejected("...cannot find a hook...")` pre-fix, green post-fix):
  `hooks_commit_cli.rs::absent_hook_under_hookspath_allows_{commit_and_amend,commit_merge,push}`
  + `relative_hookspath_failing_hook_still_blocks` (real hook under relative hooksPath still runs)
- [T2.4] F-A4-2 · MED · merge.rs:311 — clean auto-merge commits run NO hooks (real git runs
  pre-merge-commit/prepare-commit-msg/commit-msg). DECISION (orchestrator, 2026-08-09): run the
  commit-msg hook on clean merge commits (git parity for message policy); pre-merge-commit stays
  unsupported+documented. Behavior change: Y, FOR USER REVIEW — **fixed (pending commit)**:
  `finalize_merge_commit` takes `MergeHooks::{Off,MessageOnly,Full}`; the clean auto-merge passes
  MessageOnly (commit-msg only — no pre-commit/post-commit), commit_merge keeps Full;
  `merge_branch` gained `skip_hooks` (core bool; command `skipHooks: Option<bool>`, absent ⇒ false
  — wire-compatible, types.ts untouched; MCP passes false). Rejection outcome PINNED: merge left
  PAUSED (MERGE_HEAD retained, HEAD unchanged) = git's "Not committing merge" state, recoverable
  via commit_merge/abort_merge; autostash retained. Tests: `hooks_commit_cli.rs::
  clean_merge_runs_commit_msg_hook_not_pre_commit`, `clean_merge_commit_msg_fail_leaves_merge_paused`
  (incl. recovery), `clean_merge_skip_hooks_bypasses_commit_msg`
- [T2.4] F-A4-3 · MED · hooks.rs:41 — prepare-commit-msg unsupported (ticket-ID/template injectors
  silently no-op). DECISION: document as a known v1 divergence in P59 checklist + user docs; not
  implementing now — **done (docs, pending commit)**: "Known v1 hook divergences" section added to
  P59-user-checklist.md (prepare-commit-msg, pre-merge-commit, composer skip-hooks)
- [T2.4] F-A4-4 · MED · compose_apply.rs:147 — AI-composed commits hard-code skip_hooks=true
  (deliberate: re-staging hooks would corrupt the split plan). DECISION: keep behavior, document
  prominently + FOR USER REVIEW; revisit commit-msg-only execution later — **done (docs, pending
  commit)**: compose_apply.rs comment expanded (F-A4-4 reference + consequence for policy shops),
  P59 checklist divergences section, FOR USER REVIEW bullet above
- [T2.4] F-A4-5 · LOW · hooks.rs:108 — git-infrastructure failures misclassified as HookRejected;
  near-unreachable after F-A4-1; fold a prefix-match improvement into the fix pass — **fixed
  (pending commit)**: `is_git_infra_failure` classifies stderr whose FIRST line is git's own
  pre-hook failure ("cannot find a hook named", "cannot run", "cannot spawn", "not a git
  repository" under `error:`/`fatal:`) as AppError::Git; deliberately narrow so a hook's own
  git-flavored stderr stays HookRejected. Unit test `git_infra_failure_classifier_is_narrow`
- [T2.4] NITs: no hook timeout (git parity) — **documented** (hooks.rs module doc + run_hook doc);
  non-UTF-8 $1/temp-path theoretical lossiness — **documented** (run_hook + run_commit_msg_hook
  docs; non-UTF-8 rewrite ⇒ clean Io error noted); hook-writes-non-UTF-8-message Io-error test +
  pre-push stdin remote-oid fetch-time baseline — **remaining tester pins**. Fix pass also added
  tester-pinned integration cases: amend hooks (block/trailer/skip), commit_merge hooks
  (fail⇒MERGE_HEAD retained, rewrite, post-commit sees MERGE_HEAD), CRLF trailer normalization,
  hook-emptied message ⇒ EmptyMessage, skip_hooks sentinel matrix over all commit-side sites
  (tests/hooks_commit_cli.rs, 14 tests)
- [T2.4] Verified sound: hook order matches git incl. amend + commit_merge (post-commit before
  cleanup_state), skip_hooks reaches all 5 sites, force-push --no-verify prevents double execution,
  linked-worktree commondir handling, no hooks on read ops, CRLF message re-normalization.

## T2 Area 5 — cred_cache + exec seam (audit 2026-08-09)

- [T2.5] F-A5-a · SHOULD-FIX · cred_cache.rs:351 normalize_key — host-scoped key defeats
  credential.useHttpPath: token filled for org A replayed to org B on same host (dev.azure.com)
  before self-healing via 401. Fix: include path in key when useHttpPath set (mirror git) — **fixed
  (pending commit)**: `normalize_key(url, use_http_path)` appends the case-preserved URL path when set;
  new `key_for(repo_path, url)` reads `credential.<host>.useHttpPath` (URL-scoped) then unscoped from
  git2 config; resolve/evict/warm all key through it (evict + module facade gained a `repo_path` arg
  so the SAME key is computed). Behavior change? N (host-only unchanged when unset). Tests
  `normalize_key_with_http_path_includes_path`, `key_for_honors_use_http_path_config`,
  `key_for_honors_host_scoped_use_http_path`
- [T2.5] F-A5-b · SHOULD-FIX · remote.rs:268/310 — server-rejected fresh-fill cred stays cached for
  full 10-min TTL and helper never receives `credential reject` → every subsequent op double-fails.
  Fix: evict + send credential reject on op-level AuthFailed. Behavior change: Y, FOR USER REVIEW —
  **fixed (pending commit)**: `CredAttempts.fresh_fill_url` records a FRESH fill; `fetch_remote`/push
  route the mapped error through `evict_fresh_on_auth_fail`, which evicts that key on AuthFailed. NO
  `credential reject` (would need plaintext cred_cache deliberately walls off — doc-noted). FOR USER
  REVIEW bullet added. Test `evict_fresh_on_auth_fail_is_identity_and_scoped`
- [T2.5] F-A5-c · LOW · exec.rs:115 — no timeout + unbounded output capture. DECISION: add output
  size cap; document no-timeout as git-parity limitation (hooks can hang git too) — **fixed (pending
  commit)**: 64 MB combined stdout+stderr cap via `read_capped` (shared AtomicUsize counter, dual-
  thread drain-to-EOF, overflow → AppError::Git); no-timeout documented as git-parity in the module
  doc. Env-hygiene invariant now TESTED via `build_command` extraction + `get_envs`/`get_args`. Tests
  `build_command_enforces_never_prompt_env_hygiene`, `build_command_layers_caller_env_over_defaults`,
  `read_capped_flags_overflow_but_drains_to_eof`, `read_capped_shares_counter_across_streams`
- [T2.5] F-A5-d · LOW · remote.rs:929 — build_force_push_args lacks `--` end-of-options before
  positional remote/refspec (config-write-level argument-injection defense-in-depth). One-line —
  **fixed (pending commit)**: `--` inserted before `<remote> <refspec>`; `force_push_args_exact_vec` /
  `force_push_args_nested_branch` updated for the shifted indices
- [T2.5] F-A5-e · NIT · exec.rs:101 — stdin write-then-wait deadlock constraint for future callers;
  document on trait — **noted (deferred)**: the new exec body now takes+drops stdin (explicit EOF) and
  documents that git stdin is small so a pre-read write can't deadlock in practice; a formal
  trait-level constraint note stays deferred (fold in)
- [T2.5] NITs: slots never removed from map (raw URL retained, bounded); zeroization DECISION —
  docs honestly state not-implemented, no false claim ⇒ defer zeroize dep (log as deferred, not a
  bug); search.rs SpawnGitRunner askpass-hygiene drift (consider unifying on GitExec later);
  default-port duplicate keys (document). Exec env-hygiene invariant **now has its recording-fake
  test** (F-A5-c, via `build_command`).
- [T2.5] Verified sound: no secrets in Debug/errors, poison recovery, single-flight RAII, stale-
  while-revalidate math, hooks argv already uses `--`.

## T2 Area 6 — stash + search (audit 2026-08-09)

- [T2.6] F-A6-A · MUST-FIX (data loss) · stash.rs:256-262/:320-336 create_staged_stash — staged
  deletion + rewritten worktree content: delete branch captures only the deletion, then the
  force-checkout restores HEAD over the new content → content exists nowhere. Fix: fold worktree
  blob for differing delete-branch files or exclude from force-checkout — **fixed (pending
  commit)**: delete branch now folds the worktree blob when the on-disk content differs from HEAD
  (FOLD semantics, same rule as mixed staged+unstaged); file absent or ==HEAD still records the
  deletion (rm --cached case preserved, p34 case 10 still green). Test
  `staged_delete_plus_rewrite_folds_worktree_content` (stash tree holds rewritten bytes; pop
  restores them unstaged)
- [T2.6] F-A6-B · MUST-FIX (wrong-target destructive) · stash.rs apply/pop/drop + commands — index-
  only addressing, no identity check; stack shift between render and confirm (external git stash OR
  in-app autostash-retained-on-conflict) makes drop_stash(0) destroy the wrong, unrecoverable entry.
  Fix: expected_oid: Option<String> verified against stash_commit_oid before acting. NOTE: types.ts
  frozen (paused session) → core+command take the optional param now (serde default None keeps IPC
  compatible); UI wiring deferred until types.ts unfreezes — **fixed (pending commit)**:
  `verify_expected_oid` guard in core apply/pop/drop (mismatch → AppError::Git "stash list changed;
  refresh and retry", nothing touched); commands take `expectedOid: Option<String>` (missing → None
  on the wire, existing callers unchanged; MCP/branches pass None). Tests
  `expected_oid_mismatch_blocks_apply_pop_drop`, `expected_oid_on_missing_index_errors_cleanly`
- [T2.6] F-A6-C · LOW · search.rs:75/:230 — docs claim -S ignores case-insensitivity; actually -i
  sets DIFF_PICKAXE_IGNORE_CASE for both -S and -G. DECISION: keep actual behavior (case-insensitive
  -S under default), fix docs + TS doc mirror, pin with mixed-case oracle test — **fixed (pending
  commit, Rust docs)**: SearchQuery.case_sensitive doc + build_log_args comment now state -i applies
  to -S via DIFF_PICKAXE_IGNORE_CASE. TS doc mirror + oracle test deferred (types.ts frozen) —
  **tester: mixed-case -S oracle still open**
- [T2.6] F-A6-D · LOW · search.rs:426-441 seed_all_refs — one garbled loose ref aborts whole search;
  skip bad entries like the per-commit path — **fixed (pending commit)**: branch/remote/tag iterator
  entry errors now `continue` (best-effort skip); test `seed_all_refs_skips_garbled_loose_refs`
  (garbled loose branch + tag ref files; HEAD commit still seeded)
- [T2.6] F-A6-E · LOW · stash.rs:483/:494 — non-UTF-8 paths silently dropped from preflight/allowlist
  sets (skip-reserved apply silently fails to restore them). Surface an error instead — **fixed
  (pending commit)**: stash_path_sets now uses `path_bytes()` + explicit UTF-8 validation (also
  avoids git2's bytes→Path panic on Windows) and hard-errors "non-unicode path"; untracked ^3
  collection switched from Tree::walk (whose `&str` root can panic) to an empty-tree diff. Test
  `non_utf8_stash_path_errors_instead_of_silent_drop`
- [T2.6] F-A6-F · LOW · stash.rs:515-524 — skip-reserved allowlist entries act as pathspec patterns
  (untracked `foo[1].txt` may not self-match → silently not restored). Escape glob metachars —
  **fixed (pending commit)**: dual-form allowlist — raw entry (literal tracked-checkout phase, which
  honors disable_pathspec_match) + glob-escaped entry (untracked phase, which drops the flag);
  post-apply reserved-path guard unchanged as backstop. Tests `escape_pathspec_truth_table` +
  end-to-end `skip_reserved_restores_metachar_untracked_path` (non-Windows)
- [T2.6] NITs: 0x1f in subject shifts parse fields (garbage row, no panic — document); `:`-magic
  path queries (document); unicode case-fold divergence (tests compare semantics); SpawnGitRunner
  env alignment with exec seam (uniformity only) — **tester pins / docs**
- [T2.6] Verified sound: injection surface (single-token argv, --end-of-options, leading-dash scope
  rejection, --output blocked), conflict-retention on pop, bounded results + truncated signal,
  reserved-path recovery, no unwraps on user input.

## T2 Area 7 — stale/submodule/autostash/tags/opstate (audit 2026-08-09)

- [T2.7] F-A7-1 · MUST-FIX (HIGH, deletes default branch) · stale.rs:236/:332 — base excluded by
  string equality only; base passed as refs/heads/main, OID, or tag ⇒ `main` classified merged-stale
  and deletable. Fix: resolve base ref, exclude by resolved identity — **fixed (pending commit)**:
  `resolve_stale_base` → `BaseIdentity` (revparse_ext, protected-name set + OID-tip protection);
  test `base_identity_protects_main_for_refname_oid_and_tag` pins all 3 forms
- [T2.7] F-A7-2 · MUST-FIX (HIGH, path traversal) · submodule.rs:309 — remove_submodule joins
  .gitmodules-supplied name into .git/modules/<name> for remove_dir_all; name `../../dir` escapes
  .git (CVE-2018-11235 vector). Fix: reject `..`/separators or canonicalize-and-contain — **fixed
  (pending commit)**: `validate_modules_name` (rejects `..`/`.`/empty components + absolute, before
  any destructive step) + `remove_cached_git_dir` canonicalize-and-contain; tests
  `modules_name_validation_rejects_traversal`, `remove_submodule_rejects_hostile_name_before_running_git`
- [T2.7] F-A7-3 · MUST-FIX (TOCTOU) · stale.rs:315-376 — tips not re-verified at delete time;
  Branch::delete is -D-equivalent. Fix: keep tip OID in safe map, skip if moved — **fixed (pending
  commit)**: safe set is now name→scanned-tip HashMap; `recheck_tip` re-reads at delete time and
  emits a Failed "tip moved" row (no new enum variant — types.ts untouched); test
  `recheck_tip_detects_moved_tip`
- [T2.7] F-A7-4 · SHOULD-FIX · stale.rs:134/:236 — remote base (origin/main) doesn't protect local
  `main`. Protect local counterpart of base unless explicitly targeted — **fixed (pending commit)**:
  remote-tracking base protects its local counterpart; origin/HEAD's target (default branch) is
  never auto-classified; tests `remote_base_protects_local_counterpart`,
  `default_branch_never_auto_classified`
- [T2.7] F-A7-5 · SHOULD-FIX · stale.rs:93/:364 — Deleted rows carry no tip oid; goneUpstream+ahead
  branches unrecoverable and P60 undo can't restore. Add "was at <short-oid>" / tip field — **fixed
  (pending commit)**: Deleted rows' `message` = "was at <short-oid>" (existing field, no wire-type
  change); asserted in `delete_branches_safety`
- [T2.7] F-A7-6 · SHOULD-FIX (wrong-stash) · autostash.rs:41/:73/:104 — apply/drop stash@{0} blindly;
  foreign stash pushed between save and pop ⇒ wrong stash applied AND dropped. Fix: track saved Oid,
  locate by identity, error-with-retain if absent — **fixed (pending commit)**: `stash_save` returns
  the saved Oid; `rollback_and_map(Option<Oid>)` / `pop_after_success(Oid)` locate it via
  `stash_foreach` before apply/drop; absent → error "your autostashed changes were not found; check
  `git stash list`" (nothing touched); messages name the real stash@{N}. Threaded through merge.rs /
  cherrypick.rs / revert.rs (undo.rs only uses is_dirty; branches.rs uses stash:: directly). Tests
  `pop_after_success_locates_stash_by_oid_not_position`,
  `pop_after_success_missing_oid_errors_and_touches_nothing`,
  `rollback_and_map_restores_by_oid_under_foreign_stash` (+2)
- [T2.7] F-A7-7 · DECISION · submodule.rs:286 — deinit -f/rm -f destroy dirty submodule work with
  only the UI confirm as gate. Adding a force param needs types.ts (frozen). DECISION: document now
  + FOR USER REVIEW; implement refuse-unless-force after types.ts unfreezes — **open (docs)**
- [T2.7] F-A7-8 · DECISION · tags.rs — tag.gpgSign ignored (annotated tags never signed). DECISION:
  document as known v1 limitation + FINDINGS/user-docs entry; revisit with signing area —
  **fixed (pending commit)**: annotated tags now sign when `tag.gpgSign=true` OR the new optional
  `sign` flag is set, via `signing::create_signed_tag` (`git tag -s`, same exec seam / key resolution
  as commit signing). Lightweight tags never signed (git parity); ssh + no `user.signingkey` ⇒
  `ConfigMissing` (no silent unsigned tag). `create_tag` command gained optional `sign?: boolean`
  (absent ⇒ config-driven; wire-compatible with frozen types.ts). Tests: `tags_cli_2.rs` SSH-hermetic
  suite (tag.gpgSign-signs / sign-flag-signs / missing-key-ConfigMissing / lightweight-never-signed /
  unsigned-unchanged), gated by `require_git_ssh!`. Tag-signature *verification* left out of scope
  (`verify_commits` is commit-only) — FOR USER REVIEW bullet above.
- [T2.7] F-A7-9 · LOW · stale.rs:253/:271 — one dangling/corrupt branch ref aborts whole scan+delete
  batch; best-effort skip like the non-UTF-8 arm — **fixed (pending commit)**: iterator item,
  `graph_descendant_of`, and `find_commit` errors now skip that branch (eprintln) instead of `?`;
  test `dangling_branch_ref_is_skipped_not_fatal`
- [T2.7] F-A7-10 · LOW · submodule.rs:217-245 — add_submodule partial-failure residue (.gitmodules +
  config entries linger; retry hits Exists). Cleanup or clear error — **fixed (pending commit)**:
  `rollback_partial_add` (best-effort: .gitmodules entries, .git/config `submodule.<name>.*`,
  partial checkout dir, cached .git/modules dir; original error returned; residual limits
  documented on the fn); test `add_submodule_rolls_back_on_clone_failure` (fail → retry succeeds)
- [T2.7] NITs: delete_tag skips validate_tag_name — **fixed (pending commit)** (same validation as
  create; test `delete_tag_validates_name`); autostash conflicted-path list_conflicts `?` loses
  safety message — **fixed (pending commit)** (falls back to plain "safe at stash@{0}" message in
  both `pop_after_success` arms). Still open, fold into a later pass: create_tag can tag tree/blob
  (pin); opstate current_step>total cosmetic; eprintln! diagnostics invisible in-app
- [T2.7] Verified sound: submodule add/update pure libgit2 (ext:: transport unreachable), deinit/rm
  argv injection-safe, push_tag refspec injection precluded, opstate reads panic-free, stale
  recompute-fresh design + per-branch result rows.

## T2 Area 8 — MCP servers (audit 2026-08-09)

- [T2.8] F-A8-a · SHOULD-FIX (perf) · bonsai-mcp/server.rs:189 — ok_json double-transmits every
  response (structured + full text echo via rmcp structured()); multi-MB graph responses sent twice.
  Replace text echo with compact summary — **fixed (pending commit)**: `ok_json` now builds the
  result via `structured()` then OVERWRITES the echoed text block with a payload-free
  `compact_summary` (array→`[N items]`, object→top-level keys capped at 12, scalar→char-safe
  truncation). Full payload still in `structured_content` (client unaffected); doc comment
  corrected. Units `ok_json_puts_full_payload_in_structured_and_compact_text`, `compact_summary_shapes`.
- [T2.8] F-A8-b · SHOULD-FIX (drift) · src-tauri/mcp.rs:54 — READ/WRITE tool counts hard-coded (3
  catalog copies, no sync test); derive from routers + drift test — **fixed (pending commit)**: added
  `BonsaiServer::{read,write}_tool_{names,count}()` deriving from the live routers; src-tauri's
  `read_tool_count()`/`write_tool_count()` now call them (no more `const 14`/`20`). Drift test
  `tool_catalogs_match_live_routers` (mcp_stdio_3.rs) asserts catalogs == routers == counts (14/20);
  `live_tools_list_matches_router_names` asserts a live `tools/list` == the routers. The src-tauri
  `tool_count_reflects_write_gate` (14/34) now also guards drift.
- [T2.8] F-A8-c · SHOULD-FIX · src-tauri/mcp.rs:224 — set_allow_write bounce failure leaves server
  down, settings say enabled, no mcp-server-changed emitted → stale UI. Emit stopped status on
  bounce failure — **fixed (pending commit)**: new `start_or_signal_stopped` wraps `start`; on
  failure it persists `mcp_enabled=false` (best-effort) and emits a stopped `mcp-server-changed`
  before returning the error. Used by BOTH the `set_allow_write` bounce restart AND `set_enabled`'s
  initial start, so a failed (re)start can never leave a dead-but-"enabled" server in the UI.
- [T2.8] F-A8-d · LOW · bonsai_get_graph unbounded (P65 deferred); warn in tool description —
  **fixed (pending commit)**: `bonsai_get_graph` doc now warns the whole layout is returned in one
  (possibly multi-MB) response and P65 paging is deferred.
- [T2.8] NITs — **fixed (pending commit)**: lib.rs tool-count doc 32→34; `--help` now prints usage
  to STDOUT and exits SUCCESS (was stderr + FAILURE) via a `ParseOutcome::Help` arm; select_repo on a
  `Fixed` (standalone) server now returns `invalidName` (was `other`) to match the unknown-repo
  rejection; parse_args/validate_repo gained 8 inline units, parse_resolution/err_result gained units.
- [T2.8] Verified sound: write gating structural (20 mutation tools only in write_router, both
  constructors funnel through the gate), select_repo restricted to open tabs, validate_rel_path on
  conflict paths, no locks across await, git2 in spawn_blocking, constant-time token auth +
  Origin/Host checks, rmcp survives garbage frames.

## T2 Area 9 — history_index/error/external/maintenance/image_diff (audit 2026-08-09)

- [T2.9] F-A9-1 · SHOULD-FIX · history_index/store.rs:73 — non-unique store.json.tmp races
  concurrent builds (corrupted rename-into-place; Windows ACCESS_DENIED). Unique tmp suffix or
  per-dir lock — **fixed (pending commit)**: tmp is now `store.json.<pid>.<nonce>.tmp` (per-process
  AtomicU64 nonce); `load` best-effort reclaims stale `store.json.*.tmp` (an in-flight tmp still open
  can't be deleted on Windows → left alone). Tests `concurrent_saves_use_isolated_tmp_and_leave_no_leftover`
  (8-thread storm), `load_cleans_up_stale_tmp`
- [T2.9] F-A9-2 · SHOULD-FIX · history_index/mod.rs:117 — add-only store retains rewritten-away
  commits forever (dead oids in results, skewed idf, unbounded growth). Prune docs absent from
  reachable_oids during build — **fixed (pending commit)**: `build_index` prunes `store.docs` to the
  reachable oid set BEFORE extraction; SKIPS pruning when the walk hit `MAX_INDEX_COMMITS` (can't tell
  a ghost from a beyond-cap live commit — also closes the F-A9-4 cap-before-filter drift). Test
  `build_prunes_ghost_docs_absent_from_reachable`
- [T2.9] F-A9-3 · LOW — one bad object aborts build — **fixed (pending commit)**: extraction loop
  skips-and-counts an unreadable object (eprintln diagnostic per skip + total) instead of `?`-aborting;
  the skipped oid stays out of the store so the next build retries it. Test
  `build_skips_unreadable_object_and_indexes_the_rest` (corrupts a loose blob).
  F-A9-4 (cap-before-filter) folded into F-A9-2 (prune only when not truncated). F-A9-5..7 (CJK
  single-token, repo_key FNV collision) remain documented-accept, no code change.
- [T2.9] F-A9-8..13 · LOW/NIT — **fixed (pending commit)** where actionable: external.rs got the
  accepted-risk module-doc note (`.cmd`/`.bat` shim `%VAR%` expansion + `wt` `;` sub-command
  splitting — user-owned template, not attacker-controlled); maintenance.rs now `eprintln!`s the
  `Skipped` reason (bonsai-core has no `log` dep — mirrors other git/ diagnostics); image_diff.rs
  module+field docs corrected (invalid path → `AppError::Other`, and the 0-byte=absent behavior);
  F-A9-13 0-byte image side now returns `None` (absent) not empty base64 — tests
  `zero_byte_side_is_absent_not_empty_base64` + `make_side_flags_over_cap_and_encodes` (empty arm).
  Unix zombie-on-spawn-and-drop stays cosmetic (no change).
- [T2.9] Verified clean: error.rs full wire parity with types.ts (29 kinds), base64 RFC 4648-correct,
  store load never panics on garbage/foreign schema, no partial index ever visible, maintenance
  genuinely best-effort, external.rs argv discrete-token injection-safe (post-CVE-2024-24576).

## T3 pass 2b — repoWorkspace hooks + updater (2026-08-09)

- [T3.2b] F-T32b-1 · SHOULD-FIX · src/components/ShortcutOverlay.tsx:10 — the documented
  shortcut table is stale: useWorkspaceKeyboard.ts also binds Ctrl/Cmd-F (commit search, P50b)
  and Ctrl/Cmd-K (command palette, P50c), but the overlay never lists them. Repro:
  useWorkspaceKeyboard.test.tsx › "ShortcutOverlay sync" — **fixed** (commit 7f547f4): Ctrl+F /
  Ctrl+K rows added to the overlay; sync test un-skipped and passing
- [T3.2b] F-T32b-2 · NIT/fragility — the binding table exists twice (SHORTCUTS literal in
  ShortcutOverlay.tsx, not exported; imperative handler in useWorkspaceKeyboard.ts). The sync
  test necessarily duplicates key literals a third time; exporting SHORTCUTS (or a shared bindings
  module) would make divergence a compile-time/test-time certainty instead of a manual audit —
  **finding candidate for a cohesion pass**
- [T3.2b] Verified clean: Esc peel order (palette>composer>typing-bail>aiPanel>blame>history>
  reflog>commitBrowser>search>historySearch>diffSlot>compare>deselect), Ctrl-vs-Cmd via
  `ctrlKey||metaKey`, typing/dialog guards, nav clamping; commit/history search debounce +
  last-wins + empty-reset; read-overlay cross-invalidation + stale-guards + reflog-restore
  refetch; verify batching (512 chunk) + cache + enable/repo transitions; hook-gate
  park/skip/cancel incl. cancel-during-retry race; updater state machine guards + ?update= seam.
- [T3.3a] F-T33a-1 · MINOR (a11y) — PrCreateForm.tsx: the "✨ Generate with AI" button is
  rendered INSIDE the Description `<label>`, so its computed accessible name is "Description"
  (label text names descendants) — screen readers announce it as "Description", and
  role-based queries can't target it. Fix: move the button out of the `<label>` (or add an
  explicit aria-label). Tests select it by class as a workaround (see note in
  PrCreateForm.test.tsx) — **fixed** (commit 8a8d70c): aria-label="Generate description with AI"
- [T3.3a] Verified clean: destructive-dialog safety (initial focus on Cancel ⇒ stray Enter
  cancels, never confirms — verified on ConfirmDialog + a DestructiveDialogs instance);
  PromptDialog Enter-submits with validation gating; CommitBox gating/sign/skip-hooks/generate
  replace-confirm; PrCreateForm generate fill-never-submit + generating lock + rejection toast;
  CommandPalette nav/dispatch/dynamic rows; ErrorBoundary catch+reset; Tree, ContextMenu
  (submenu/danger/dismiss), Combobox (strict revert/free input), TabStrip (recents filter/DnD
  reorder), PaneDivider (delta normalization), Toasts (order/roles). Note: Toasts is
  presentational — auto-dismiss timers live in App, not testable here; TabStrip middle-click
  close is not implemented (no auxclick handler) — by design, not a bug.
- [T3.4] F-T34-1 · FIXED (mock) — merge.ts: a FRESH `mergeBranch('…conflict…')` returned the
  conflicts outcome WITHOUT seeding opState/conflict entries/texts/conflicted status (the T4
  contract's known gap): getOpState stayed `none`, listConflicts was empty, and
  getConflict/commitMerge/abortMerge were unreachable without the `?op=merge` URL seed.
  Fixed by reusing `seedOpState(state,'merge')` on that path, then overriding
  `incoming`/`message` with the actual branch name; the returned outcome paths now mirror the
  seeded conflicts (['README.md','src/auth.ts'] path-ascending, replacing the incoherent
  ['src/app.ts','README.md'] that never matched listConflicts). `?op=merge` behavior unchanged;
  e2e/06-merge-conflicts.spec.ts (incl. its render-only downgrade test) still passes 7/7 —
  its §5.06.7 [RENDER] downgrade can now be upgraded to the full editor flow.
- [T3.4] Note (fixture semantics, not a bug) — getGraph injects stash offshoot rows at
  nodes[0..2] with `author: ''` (withStashNodes prepends), while getCommitDiff /
  getInteractivePlan resolve rows WITHOUT the stash nodes. Tests (and any harness automation)
  must skip `author === ''` rows / use the raw fixture layout when picking commit oids.
- [T3.4] Verified clean: persistence corrupt-storage matrix (garbage JSON, wrong-shape JSON,
  partial objects, huge blob, `__proto__` pollution, per-field clamps, profile sanitize);
  repoState seeding/canonicalization/stale report; stage↔unstage↔commit↔graph coherence +
  identity/emptyMessage/nothingToCommit gates; composer atomic rollback; branches
  create/checkout(FF/dirty/conflicted)/rename/delete/remote-checkout/stale-cleanup safety
  rules; fresh-merge full conflict cycle + guards; rebase clean/paused/interactive
  (reword/drop/squash guards/conflict pause) + bisect converge/skip/cannotDetermine; stash
  scopes/reserved-path recovery/amend; undo ?undo= seam plans; fetch→pull FF/diverged/push
  upstream-create/force-push lease; diff routing + ref-tip fallback + image seams; search
  caps/#fail; history build→status→retrieve→AI answer; signing ?sign= seam + deterministic
  verify; worktrees lifecycle/copy-plan; submodules transitions/#fail; scheduler backoff
  table + event ordering + timer arming; ?ai=off / ?historyFail / ?hooks= / ?fixture=noconfig
  / 20k / ?op=merge / ?branch=cbhconflict seams. (13 new files, 208 tests; vitest 776→984.)

- [T5.fe] F-T5fe-1 · LOW (defensive-contract violation) · src/utils/intralineSegments.ts:39-47 —
  segmentLine DUPLICATES text on a zero-length-after-clamp span: when a span's clamped range is
  empty but starts past the cursor (len <= 0, or start >= end e.g. +Infinity / start > n with the
  gap still pending), the unchanged gap is emitted WITHOUT advancing the cursor, so later spans /
  the final tail re-emit the same characters. Minimal repro: `segmentLine('hello', [[2, 0]])` →
  'he' + 'hello' = 'hehello'. Contradicts the module doc ("out-of-order / overlapping /
  out-of-range entries are clamped defensively so a bad payload can never throw or duplicate
  text"). Backend never emits len-0 spans, so unreachable via well-formed IPC — but this is
  exactly the hostile-payload defense the doc claims. Fix (app code, senior-dev): advance
  `cursor = s` when emitting the gap, or drop `len <= 0`/empty-after-clamp spans before the gap
  emit. Repro pinned as it.skip 'F-T5fe-1' in src/utils/intralineSegments.adversarial.test.ts
  (regression test un-skipped; fuzz generator widened to Infinity/past-end starts + len 0/-3) —
  **fixed**: gap emit now advances `cursor = s` independent of the changed run. behavior change? Y
  (hostile payload no longer duplicates text; well-formed IPC unaffected).
- [T5.fe] NIT (awareness, not a bug) — StatusPanel/Sidebar key rows by path/branch-name; a
  hostile snapshot with DUPLICATE paths/names renders with React duplicate-key warnings
  (rows may collapse) but never crashes. Real backends never emit duplicates within a section.
  Pinned in adversarial-dto.test.tsx with the duplicate-key console pattern allowlisted.
- [T5.fe] Verified clean (T5b adversarial pass, 2026-08-09): GraphCanvas mounts inside the real
  ErrorBoundary under 7 hostile GraphLayout shapes (negative/NaN/1e9 lanes, parents[99999],
  edge from>to, laneCount 0 with nodes, 0-row layout with edges + headIndex) — no throw escapes;
  edgeIndex/viewport/geometry pure math no-throw + well-formed for finite inputs (NaN scrollTop
  propagates NaN — unreachable from the DOM scroller, pinned as observed); DiffView renders
  out-of-bounds/overlapping/negative/NaN spans with content verbatim, 0-line + out-of-order hunks
  fine; CommitPanel survives 10k-char summary, NUL+RTL message, null author, NaN timestamps,
  out-of-layout parents (plain text, no jump); Sidebar renders 1000 branches + `<script>` name as
  inert text (no live element). Rapid-fire: CommitBox double-click/Ctrl-Enter-spam/split-control
  = exactly ONE commit call; palette Enter-spam dispatches once (closes first), 10× toggle +
  type/arrow spam no desync, fresh query per open; 10× interleaved mock stage/unstage settles
  last-wins with the file in exactly one section. Pure-fn fuzz (seeded, no fast-check):
  segmentLine 200 iters (concat-exact, astral-safe, 1MB), pairSplitRows 150 iters (identity
  placement, no both-null), conflictRegions 150 iters (parse shape, bounded resolution to
  marker-free, 20k-line doc), buildPathTree 150 iters (leaf multiset identity, unique dir
  prefixes, deep/huge/unicode paths). §5.1 persistence garbage: T3.4 already covers all listed
  cases incl. "null" and __proto__ — nothing added. (4 new files, 41 tests; vitest 1197→1237.)

- [T3.6] Canvas-internals refactor (behavior-preserving): extracted the pure logic out of
  GraphCanvas.tsx (1071→821 LOC) and draw.ts (459→381) into geometry.ts (laneX/rowY/refColArea/
  summaryStartX/initials/avatarColor/avatarHit — re-exported from draw.ts for existing callers),
  viewport.ts (visible-row range/overscan, scroll-into-view, tooltip clamp, DPR backing-store,
  spacer height), hitTest.ts (row/pill/chip/PR/CI hit resolution, targetRefOf, tooltip identity)
  and selfTest.ts (the mock-only p7SelfTest body, moved verbatim). All logic moved verbatim —
  NO bugs found in the pure hit-test/viewport math (boundary semantics confirmed by tests:
  row bottom edge belongs to the next row, pill/badge edges inclusive, tooltip right/bottom
  overflow boundaries exclusive). +84 unit tests (geometry/viewport/hitTest .test.ts; vitest
  1113→1197); pnpm build green; full Playwright suite 88 passed / 1 skipped (specs 02/03
  exercise the canvas directly — behavior-preservation oracle).

## T5a Rust — property-based + corrupt-repo + race/lifecycle pass (2026-08-10)

New suites (bonsai-core dev-dep `proptest = "1.6"`, in-file default 64 cases, local
`PROPTEST_CASES=256`): `tests/prop_common/mod.rs` (RepoShape strategy + repo builder + diff_pair +
status oracle mapping), `prop_graph_layout.rs`, `prop_intraline.rs`, `prop_history_index.rs`,
`prop_status.rs` (32-cap in-file), `prop_stash_roundtrip.rs`, `corrupt_repo_cli.rs`,
`race_lifecycle_cli.rs`. All green; clippy `-D warnings` clean on `--all-targets`.

- [T5a] TEST-SEAM (not a bug) — added `#[doc(hidden)] pub fn annotate_hunk_for_tests(&mut Hunk)` to
  `src/git/intraline.rs`, a one-line verbatim forwarder to the `pub(crate) annotate_hunk` so the
  cross-crate property suite can reach it. ZERO behavior change. Contract §2.2/§8.2-approved.

- [T5a] F-T5-4 · HIGH (UI freeze / DoS) · `graph::compute_graph`, `status::read_status`,
  `commit::create_commit` — truncating the HEAD **loose commit** object to half length makes every
  surface that peels HEAD **HANG (effectively forever)** instead of returning an `AppError`; only
  `git2::Repository::open_ext` (which does not peel HEAD) stays responsive. A truncated **tree** or
  **blob** is handled cleanly (`Ok`). Root cause is a libgit2 spin inflating a truncated zlib loose
  object during the revwalk / HEAD-peel. A single corrupt loose commit thus freezes the app.
  Repro pinned deterministically by `corrupt_repo_cli.rs` cell C1 (10s watchdog ⇒ `Hung`).
  **Status: RESOLVED for read surfaces (2026-08-19, commit `7edd23e`, audit #2 §3.2).** The
  recommended command-layer timeout landed as `run_with_git_timeout` (`git/timeout.rs`: worker
  thread + 30 s inactivity deadline, `BONSAI_GIT_TIMEOUT_MS` override, wedged worker detached,
  clean `AppError::Git` returned). Wraps `get_status`, `get_graph`, `stream_graph` and the
  history-index build; `corrupt_repo_cli.rs` C1 now pins **Err-not-Hung** for those surfaces.
  `create_commit` is deliberately NOT wrapped (a false timeout on a mutation could race a late
  commit — rationale at the C1 cell). Earlier status, kept for the record: **DOCUMENTED — bounded
  probe NOT viable; command-layer timeout wrapper is the real fix (architecture decision → FOR
  USER REVIEW).** Timeboxed mitigation investigation (2026-08-09):
  probed the `git2::Odb` header path against the truncated HEAD commit oid on a watchdog thread.
  Result — `odb.read_header(oid)` returns **`Ok((size=213, Commit))`** and `odb.exists(oid)` returns
  **`true`**, i.e. neither hangs BUT neither detects the truncation: the loose-object header
  (`commit <size>\0`) sits at the START of the zlib stream and inflates fine, while the truncation
  removed the LATTER half of the compressed body — so `read_header` reports the object's *declared*
  size and a healthy type. Only the full-inflate paths (`odb.read(oid)` / `repo.find_commit(oid)`)
  HANG. A `read_header`-based "is HEAD structurally readable" gate is therefore a **false-negative**
  for this corruption (it passes the truncated commit) and cannot prevent the hang. No other bounded
  libgit2 API validates a loose object without full inflation. Per the task's explicit guardrail,
  NO thread-kill hack was added and the corrupt-matrix C1 pin (watchdog ⇒ `Hung`) is retained as the
  honest recorded behavior. Recommended real fix (deliberate, user-owned): wrap each heavy git2 read
  at the Tauri command layer in `spawn_blocking` + a bounded `timeout`, surfacing a clean
  `AppError::Git("operation timed out — repository may be corrupt")` and abandoning the worker
  (libgit2 offers no cooperative cancellation for a zlib spin, so the worker thread is leaked until
  process exit — acceptable for a rare corrupt-repo case, and strictly better than a frozen UI).

- [T5a] F-T5-3 · MEDIUM (porcelain-equivalence violation) · `status::read_status` — when a tracked
  file is deleted in the worktree and an untracked file with identical bytes appears, `read_status`
  (git2 `renames_index_to_workdir`) reported ONE unstaged **rename** (orig→new), whereas
  `git status --porcelain` reports the two events separately (`D <orig>` + `?? <new>`): git2
  rename-detects an untracked destination, the git CLI does not. **Status: FIXED (2026-08-09).**
  `read_status` now calls `StatusOptions::renames_index_to_workdir(false)` (staged
  `renames_head_to_index(true)` is unchanged) — git porcelain never rename-detects an untracked
  worktree destination, so with detection off git2 reports the delete as `WT_DELETED` and the new
  file as `WT_NEW`/untracked, matching git byte-for-byte. Evidence:
  `prop_status.rs::regression_f_t5_3_untracked_worktree_rename` now asserts
  `read_status == git porcelain` (unstaged delete `a` + untracked `b`, NO rename row); the broad
  `status_matches_porcelain` property had its **fs-rename exclusion widened back in** (new op kind 5
  moves a tracked file to an untracked path preserving bytes — the exact former divergence) and
  stays green at 32 in-file / 256-case runs. FOR USER REVIEW below.

- [T5a] F-T5-1 · LOW/DESIGN (scroll-color-stability promise) · `graph::compute_graph` — appending a
  commit to the HEAD branch can change the **lane** (hence color) of commits that were NOT in HEAD's
  lane, because the lane assignment re-runs over a re-seeded topological+time walk. The PROVABLE
  invariant (identical repo ⇒ identical layout, contract item 7) holds and is asserted as a property;
  the stronger append-stability clause (item 8) does not. Note this is instability under a NEW COMMIT
  (full recompute), NOT under scrolling a fixed layout (which never recomputes), so the CLAUDE.md
  "lanes stay stable while scrolling" promise is not directly broken. Pinned by
  `prop_graph_layout.rs::regression_f_t5_1_lane_shift_on_head_append`. Status: **by-design (orchestrator
  decision 2026-08-10)** — accept same-input determinism only. The CLAUDE.md promise is scroll-stability
  of a fixed layout (holds); full-recompute reshuffle on a NEW commit is normal for a GitKraken-style
  engine and not worth the complexity/perf cost of incremental-stable lane assignment now. Revisit if
  users report jarring lane jumps after commits. FOR USER REVIEW.

- [T5a] F-T5-2 · BY-DESIGN (pinned behavior, not a defect) · `git/intraline.rs` token diff — the
  changed-code-point SET is NOT symmetric under swapping old/new (`(a,b)` vs `(b,a)`): the LCS
  backtrack tie-break (`>=`, biased to advance OLD) picks a different common subsequence, so the
  highlighted chars differ by direction. Per-side spans stay individually well-formed (ascending,
  in-bounds, coalesced — asserted). Inherent to a directional diff. Pinned by
  `prop_intraline.rs::regression_f_t5_2_intraline_diff_is_directional`. No action needed.

- [T5a] Corrupt-repo matrix — pinned behavior (no panics anywhere; only C1 hangs, see F-T5-4).
  C2 corrupt-pack ⇒ open/status Ok, graph/commit Err · C3 dangling-symref HEAD ⇒ all Ok (unborn-like)
  · C4 HEAD=missing-oid ⇒ graph/commit Err · C5 garbage ref ⇒ graph Ok, commit Err · C6 objects dir
  removed ⇒ all Err · C7 truncated index / C8 garbage index ⇒ status+commit Err, graph Ok ·
  C9 invalid config ⇒ all Err · C10 binary COMMIT_EDITMSG ⇒ no-op (open/status/graph Ok) ·
  X1 bogus rebase-merge / X2 bogus BISECT_LOG ⇒ read surfaces Ok · X3 invalid-UTF-8 index path ⇒
  lossy, all Ok. No panic, no lock left behind.

- [T5a] Race/lifecycle — all green. Scenario 1 adapted: the `notify` watcher lives in `src-tauri`
  (not core), so the "watcher emits ≥1 signal" clause is out of scope for a core test; substituted a
  worktree write-storm during `create_commit` (commit succeeds, no thread panics, `git fsck` clean).
  Scenario 2 (read_status×50 ∥ stage+commit×10) and Scenario 3 (ops on a deleted repo all Err, no
  panic) pass as specified.

- [T5a] Timing (`PROPTEST_CASES=256`, Windows debug): prop_graph_layout 107s (2 props, ~100-commit
  temp repos built + walked twice/case) · prop_stash_roundtrip 33.5s · prop_status 58s (git shellout
  per case) · prop_history_index 0.33s · prop_intraline 0.35s. corrupt_repo_cli ~35s (C1 10s
  watchdog). In-file defaults (64; status 32; stash 48) keep the normal `cargo test` fast.
