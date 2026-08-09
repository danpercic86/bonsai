# Findings log — testing campaign 2026-08

Bugs/oddities discovered while writing tests. One bullet per finding:
`- [phase] area — symptom — status (fixed in <commit> / open / by-design)`.

## FOR USER REVIEW — behavior changes made autonomously

- F-A2-2: AI operation planner now only accepts commit references as hex hashes (4-40 chars) from the
  model; revspecs like `HEAD~1` return "Unsupported" instead of resolving. Rationale: prompt only ever
  promises hashes from the grounding state; closes a defense-in-depth gap. Revert = drop the regex
  check in revparse_commit.
- F-A4-2: clean (non-conflict) merge commits now run the commit-msg hook (previously NO hooks ran on
  clean merges). Matches real git's message-policy behavior; pre-merge-commit remains unsupported and
  is now documented as such. Revert = pass run_hooks=false again in merge_branch's finalize call.
- F-A4-4 (no code change, awareness): AI-composed commits (P54 composer) intentionally bypass ALL
  git hooks — re-staging hooks would corrupt the split plan. Now documented; revisit if you want
  commit-msg-only enforcement there.
- F-A7-1/3/4/5 (stale-branch cleanup, safety-tightening): base branch now protected under ANY
  spelling (refname/OID/tag); remote base protects its local counterpart; the repo default branch is
  never offered as stale; deletion re-checks the tip and refuses with "tip moved since scan"; Deleted
  rows now carry "was at <short-oid>" for manual recovery. Strictly more conservative — no previously-
  safe deletion is blocked except the tip-moved race.
- F-A3-1 (queued): plain rebase Abort will refuse (retryable) instead of silently overwriting an
  untracked file that collides with the original tip — matching bisect/interactive abort semantics.

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

## T2 Area 3 — bisect + rebase sequencers (audit 2026-08-09)

- [T2.3] RESOLVED-HISTORICALLY — the "known bug" rebase skip-first-op/msgnum corruption was already
  fixed in 8219ebd (paths-only reset instead of git_reset(HARD), which deletes rebase-merge state);
  the test at rebase_cli.rs:546 is ACTIVE and green — only stale comments claim it's #[ignore]d.
  No fix needed; doc cleanup = F-A3-5.
- [T2.3] F-A3-1 · MEDIUM · rebase.rs:503-505 — plain rebase_abort is the ONLY force-checkout path
  without the 46a34d4 untracked-clobber guard (libgit2 rebase.abort() hard-resets); untracked file
  matching an orig-head path is silently overwritten. Fix: ensure_no_untracked_collision(orig_head
  tree) before abort. Behavior change: Y (abort may refuse-and-retry, matching bisect/interactive) —
  **open**
- [T2.3] F-A3-2 · MEDIUM · bisect.rs:493 + rebase_interactive.rs:632 — corrupt/truncated bonsai
  sequencer state.json ⇒ in-app deadlock: reset/abort fail on parse, require_no_bisect (existence-
  only) blocks all mutations, opstate shows None → no UI escape. Fix: salvage path in reset/abort on
  parse failure (delete state dir, leave HEAD, explanatory message) — **open**
- [T2.3] F-A3-3 · LOW · bisect.rs:341 + rebase_interactive.rs:264 — cross-sequencer start guards
  asymmetric (start_bisect ignores interactive state; interactive start ignores bisect) — reachable
  via crash window; one-line cross-checks — **open**
- [T2.3] F-A3-4 · NIT · bisect.rs:111 — io errors reported as "state missing"; map NotFound vs other —
  **open (fold in)**
- [T2.3] F-A3-5 · NIT · rebase_cli.rs:467/535 — stale known-bug comments + misleading test name; doc-
  only cleanup — **open (fold in)**
- [T2.3] F-A3-6 · NIT · stage.rs:174 — clobber guard tree lookup is exact-case; Windows case-collision
  false negative; adversarial-test then fix-or-document — **open**
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
  but-non-executable hooks on unix — **open**
- [T2.4] F-A4-2 · MED · merge.rs:311 — clean auto-merge commits run NO hooks (real git runs
  pre-merge-commit/prepare-commit-msg/commit-msg). DECISION (orchestrator, 2026-08-09): run the
  commit-msg hook on clean merge commits (git parity for message policy); pre-merge-commit stays
  unsupported+documented. Behavior change: Y, FOR USER REVIEW — **open**
- [T2.4] F-A4-3 · MED · hooks.rs:41 — prepare-commit-msg unsupported (ticket-ID/template injectors
  silently no-op). DECISION: document as a known v1 divergence in P59 checklist + user docs; not
  implementing now — **open (docs)**
- [T2.4] F-A4-4 · MED · compose_apply.rs:147 — AI-composed commits hard-code skip_hooks=true
  (deliberate: re-staging hooks would corrupt the split plan). DECISION: keep behavior, document
  prominently + FOR USER REVIEW; revisit commit-msg-only execution later — **open (docs)**
- [T2.4] F-A4-5 · LOW · hooks.rs:108 — git-infrastructure failures misclassified as HookRejected;
  near-unreachable after F-A4-1; fold a prefix-match improvement into the fix pass — **open**
- [T2.4] NITs: no hook timeout (git parity, document); non-UTF-8 $1/temp-path theoretical lossiness;
  hook-writes-non-UTF-8-message → clean Io error (test it); pre-push stdin remote-oid is fetch-time
  baseline (documented) — **tester pins**
- [T2.4] Verified sound: hook order matches git incl. amend + commit_merge (post-commit before
  cleanup_state), skip_hooks reaches all 5 sites, force-push --no-verify prevents double execution,
  linked-worktree commondir handling, no hooks on read ops, CRLF message re-normalization.

## T2 Area 5 — cred_cache + exec seam (audit 2026-08-09)

- [T2.5] F-A5-a · SHOULD-FIX · cred_cache.rs:351 normalize_key — host-scoped key defeats
  credential.useHttpPath: token filled for org A replayed to org B on same host (dev.azure.com)
  before self-healing via 401. Fix: include path in key when useHttpPath set (mirror git) — **open**
- [T2.5] F-A5-b · SHOULD-FIX · remote.rs:268/310 — server-rejected fresh-fill cred stays cached for
  full 10-min TTL and helper never receives `credential reject` → every subsequent op double-fails.
  Fix: evict + send credential reject on op-level AuthFailed. Behavior change: Y, FOR USER REVIEW —
  **open**
- [T2.5] F-A5-c · LOW · exec.rs:115 — no timeout + unbounded output capture. DECISION: add output
  size cap; document no-timeout as git-parity limitation (hooks can hang git too) — **open**
- [T2.5] F-A5-d · LOW · remote.rs:929 — build_force_push_args lacks `--` end-of-options before
  positional remote/refspec (config-write-level argument-injection defense-in-depth). One-line —
  **open**
- [T2.5] F-A5-e · NIT · exec.rs:101 — stdin write-then-wait deadlock constraint for future callers;
  document on trait — **open (fold in)**
- [T2.5] NITs: slots never removed from map (raw URL retained, bounded); zeroization DECISION —
  docs honestly state not-implemented, no false claim ⇒ defer zeroize dep (log as deferred, not a
  bug); search.rs SpawnGitRunner askpass-hygiene drift (consider unifying on GitExec later);
  default-port duplicate keys (document). Exec env-hygiene invariant needs its recording-fake test.
- [T2.5] Verified sound: no secrets in Debug/errors, poison recovery, single-flight RAII, stale-
  while-revalidate math, hooks argv already uses `--`.

## T2 Area 6 — stash + search (audit 2026-08-09)

- [T2.6] F-A6-A · MUST-FIX (data loss) · stash.rs:256-262/:320-336 create_staged_stash — staged
  deletion + rewritten worktree content: delete branch captures only the deletion, then the
  force-checkout restores HEAD over the new content → content exists nowhere. Fix: fold worktree
  blob for differing delete-branch files or exclude from force-checkout — **open**
- [T2.6] F-A6-B · MUST-FIX (wrong-target destructive) · stash.rs apply/pop/drop + commands — index-
  only addressing, no identity check; stack shift between render and confirm (external git stash OR
  in-app autostash-retained-on-conflict) makes drop_stash(0) destroy the wrong, unrecoverable entry.
  Fix: expected_oid: Option<String> verified against stash_commit_oid before acting. NOTE: types.ts
  frozen (paused session) → core+command take the optional param now (serde default None keeps IPC
  compatible); UI wiring deferred until types.ts unfreezes — **open**
- [T2.6] F-A6-C · LOW · search.rs:75/:230 — docs claim -S ignores case-insensitivity; actually -i
  sets DIFF_PICKAXE_IGNORE_CASE for both -S and -G. DECISION: keep actual behavior (case-insensitive
  -S under default), fix docs + TS doc mirror, pin with mixed-case oracle test — **open (docs+test)**
- [T2.6] F-A6-D · LOW · search.rs:426-441 seed_all_refs — one garbled loose ref aborts whole search;
  skip bad entries like the per-commit path — **open**
- [T2.6] F-A6-E · LOW · stash.rs:483/:494 — non-UTF-8 paths silently dropped from preflight/allowlist
  sets (skip-reserved apply silently fails to restore them). Surface an error instead — **open**
- [T2.6] F-A6-F · LOW · stash.rs:515-524 — skip-reserved allowlist entries act as pathspec patterns
  (untracked `foo[1].txt` may not self-match → silently not restored). Escape glob metachars —
  **open**
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
  locate by identity, error-with-retain if absent — **open**
- [T2.7] F-A7-7 · DECISION · submodule.rs:286 — deinit -f/rm -f destroy dirty submodule work with
  only the UI confirm as gate. Adding a force param needs types.ts (frozen). DECISION: document now
  + FOR USER REVIEW; implement refuse-unless-force after types.ts unfreezes — **open (docs)**
- [T2.7] F-A7-8 · DECISION · tags.rs — tag.gpgSign ignored (annotated tags never signed). DECISION:
  document as known v1 limitation + FINDINGS/user-docs entry; revisit with signing area — **open (docs)**
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
  Replace text echo with compact summary — **open**
- [T2.8] F-A8-b · SHOULD-FIX (drift) · src-tauri/mcp.rs:54 — READ/WRITE tool counts hard-coded (3
  catalog copies, no sync test); derive from routers + drift test — **open**
- [T2.8] F-A8-c · SHOULD-FIX · src-tauri/mcp.rs:224 — set_allow_write bounce failure leaves server
  down, settings say enabled, no mcp-server-changed emitted → stale UI. Emit stopped status on
  bounce failure — **open**
- [T2.8] F-A8-d · LOW · bonsai_get_graph unbounded (P65 deferred); warn in tool description — **open**
- [T2.8] NITs: lib.rs tool-count doc (34 not 32); --help to stderr+FAILURE; select_repo-on-Fixed
  error kind `other`; parse_args/validate_repo have 0 tests — **fold into fix/test pass**
- [T2.8] Verified sound: write gating structural (20 mutation tools only in write_router, both
  constructors funnel through the gate), select_repo restricted to open tabs, validate_rel_path on
  conflict paths, no locks across await, git2 in spawn_blocking, constant-time token auth +
  Origin/Host checks, rmcp survives garbage frames.

## T2 Area 9 — history_index/error/external/maintenance/image_diff (audit 2026-08-09)

- [T2.9] F-A9-1 · SHOULD-FIX · history_index/store.rs:73 — non-unique store.json.tmp races
  concurrent builds (corrupted rename-into-place; Windows ACCESS_DENIED). Unique tmp suffix or
  per-dir lock — **open**
- [T2.9] F-A9-2 · SHOULD-FIX · history_index/mod.rs:117 — add-only store retains rewritten-away
  commits forever (dead oids in results, skewed idf, unbounded growth). Prune docs absent from
  reachable_oids during build — **open**
- [T2.9] F-A9-3..7 · LOW/NIT — one bad object aborts build (skip+count); 50k cap-before-filter
  drift (document); CJK single-token limitation (document); repo_key FNV collision (accept, note);
  orphan tmp cleanup — **fold into fix pass**
- [T2.9] F-A9-8..13 · LOW/NIT — cmd.exe %VAR% expansion in .cmd shims (accepted-risk comment +
  Windows test); wt `;` splitting (comment); Unix zombie on spawn-and-drop (cosmetic); maintenance
  Skipped never logged (add debug log or fix doc); image_diff error-kind doc drift (fix Rust doc;
  types.ts frozen); 0-byte image side → empty data URL (decide render-vs-absent; DECISION: treat
  0-byte side as absent=None, cleaner UI) — **fold in**
- [T2.9] Verified clean: error.rs full wire parity with types.ts (29 kinds), base64 RFC 4648-correct,
  store load never panics on garbage/foreign schema, no partial index ever visible, maintenance
  genuinely best-effort, external.rs argv discrete-token injection-safe (post-CVE-2024-24576).

## T3 pass 2b — repoWorkspace hooks + updater (2026-08-09)

- [T3.2b] F-T32b-1 · SHOULD-FIX · src/components/ShortcutOverlay.tsx:10 — the documented
  shortcut table is stale: useWorkspaceKeyboard.ts also binds Ctrl/Cmd-F (commit search, P50b)
  and Ctrl/Cmd-K (command palette, P50c), but the overlay never lists them. Repro:
  useWorkspaceKeyboard.test.tsx › "ShortcutOverlay sync" (test.skip'd assertion) — **open**
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
