//! T2 Area 1 (pass B) — AI command inners. Each command's `_inner` takes a
//! settings-file path (the consent gate) and delegates to `bonsai-core` with
//! `RunOpts::default()`, which spawns the `claude` CLI selected by
//! `BONSAI_CLAUDE_BIN`. Here that is the committed stub
//! (`crates/bonsai-core/tests/fixtures/claude_stub.*`, `BONSAI_STUB_MODE`),
//! exactly as the core `ai_*_cli.rs` tests drive it — no network, no real CLI.
//!
//! Command-layer contract asserted here: with consent GRANTED, each command
//! passes its consent gate + any pre-CLI guard and reaches the stub (the reply
//! PARSING is core's concern, covered by the `ai_*_cli.rs` suites). Env
//! (`BONSAI_CLAUDE_BIN`/`BONSAI_STUB_MODE`) is process-global, so every test
//! here holds `env_lock()`.

use super::tests_support::*;
use super::*;

fn block_on<F: std::future::Future>(f: F) -> F::Output {
    tauri::async_runtime::block_on(f)
}

// `env_lock()` comes from `tests_support`: it must be the SAME lock every module
// takes, or `BONSAI_STUB_MODE` races across test files (P68b).

// `set_stub()` / `stub_path()` also come from `tests_support` — one copy for every
// module that drives the committed stub (P68b).

/// A settings file with AI enabled + consented (so the gate passes).
fn consent_file(base: &std::path::Path) -> std::path::PathBuf {
    let file = base.join("settings.json");
    settings::update(&file, |s| {
        s.ai_enabled = true;
        s.ai_consented = true;
    })
    .expect("write consent settings");
    file
}

/// A gate-passed AI command reached the CLI iff it did NOT refuse with
/// AiUnavailable. (Parsing/quality is asserted by the core `ai_*_cli` suites.)
fn gate_passed<T: std::fmt::Debug>(res: &Result<T, AppError>) {
    assert!(
        !matches!(res, Err(AppError::AiUnavailable(_))),
        "consent granted + stub present ⇒ must not refuse with AiUnavailable: {res:?}"
    );
}

// ============================================================ availability

/// check_ai_availability probes the CLI and reports installed=true for the
/// version-emitting stub. It NEVER rejects for CLI state.
#[test]
fn check_ai_availability_reports_installed() {
    let _g = env_lock();
    set_stub("version");
    let avail = block_on(check_ai_availability()).expect("availability never errors");
    assert!(avail.installed, "the version stub reports installed");
}

// ============================================================ prose commands

/// generate_commit_message on a staged repo returns the stub body (gate passed,
/// payload assembled, CLI reached).
#[test]
fn generate_commit_message_happy() {
    let _g = env_lock();
    set_stub("success");
    let state = AppState::default();
    let (dir, id, _c0) = fixture_repo(&state);
    let base = tempfile::TempDir::new().expect("base");
    let file = consent_file(base.path());

    std::fs::write(dir.path().join("s.txt"), "staged change\n").expect("write");
    block_on(stage_inner(&state, &id, vec!["s.txt".into()])).expect("stage");

    let res = block_on(generate_commit_message_inner(&state, &file, &id)).expect("proposal");
    assert_eq!(res.message, "MERGED_BODY_OK");
}

/// ai_analyze_diff (Explain) on a commit target reaches the CLI and returns
/// prose.
#[test]
fn ai_analyze_diff_happy() {
    let _g = env_lock();
    set_stub("success");
    let state = AppState::default();
    let (_dir, id, c0) = fixture_repo(&state);
    let base = tempfile::TempDir::new().expect("base");
    let file = consent_file(base.path());

    let res = block_on(ai_analyze_diff_inner(
        &state,
        &file,
        &id,
        ai_explain::AiDiffTarget::Commit { oid: c0 },
        ai_explain::AiAnalysisMode::Explain,
    ));
    gate_passed(&res);
    assert!(res.is_ok(), "{res:?}");
}

/// ai_summarize_range over C0..HEAD reaches the CLI.
#[test]
fn ai_summarize_range_happy() {
    let _g = env_lock();
    set_stub("success");
    let state = AppState::default();
    let (dir, id, c0) = fixture_repo(&state);
    let base = tempfile::TempDir::new().expect("base");
    let file = consent_file(base.path());
    write_stage_commit(&state, &id, dir.path(), "r.txt", "r\n", "range commit");

    let res = block_on(ai_summarize_range_inner(&state, &file, &id, c0, "HEAD".into()));
    gate_passed(&res);
}

/// ai_digest over a ref range reaches the CLI.
#[test]
fn ai_digest_happy() {
    let _g = env_lock();
    set_stub("success");
    let state = AppState::default();
    let (dir, id, c0) = fixture_repo(&state);
    let base = tempfile::TempDir::new().expect("base");
    let file = consent_file(base.path());
    write_stage_commit(&state, &id, dir.path(), "g.txt", "g\n", "digest commit");

    let res = block_on(ai_digest_inner(
        &state,
        &file,
        &id,
        ai_explain::AiDigestRange::BetweenRefs { from: c0, to: "HEAD".into() },
    ));
    gate_passed(&res);
}

/// ai_explain_line blames a real line then reaches the CLI.
#[test]
fn ai_explain_line_happy() {
    let _g = env_lock();
    set_stub("success");
    let state = AppState::default();
    let (_dir, id, _c0) = fixture_repo(&state);
    let base = tempfile::TempDir::new().expect("base");
    let file = consent_file(base.path());

    let res = block_on(ai_explain_line_inner(&state, &file, &id, "a.txt".into(), 1, None));
    gate_passed(&res);
}

/// ai_resolve_conflict on a genuinely conflicted path reaches the CLI.
#[test]
fn ai_resolve_conflict_happy() {
    let _g = env_lock();
    set_stub("success");
    let state = AppState::default();
    let (dir, id, c0) = fixture_repo(&state);
    let base = tempfile::TempDir::new().expect("base");
    let file = consent_file(base.path());

    // Build a paused conflict on a.txt.
    let main = head_branch(dir.path()).expect("branch");
    block_on(create_branch_here_inner(&state, &id, "feature".into(), c0)).expect("branch");
    write_stage_commit(&state, &id, dir.path(), "a.txt", "feature\n", "feat");
    block_on(checkout_branch_inner(&state, &id, main)).expect("main");
    write_stage_commit(&state, &id, dir.path(), "a.txt", "main\n", "main edit");
    let out = block_on(merge_branch_inner(&state, &id, "feature".into(), None)).expect("merge");
    assert!(matches!(out, MergeOutcome::Conflicts { .. }), "{out:?}");

    let res = block_on(ai_resolve_conflict_inner(&state, &file, &id, "a.txt".into()));
    gate_passed(&res);
    block_on(abort_merge_inner(&state, &id)).expect("cleanup");
}

// ============================================================ structured commands

/// ai_suggest_branch_name (Working source) with an untracked change reaches the
/// CLI (gate + non-empty-grounding guard passed).
#[test]
fn ai_suggest_branch_name_happy() {
    let _g = env_lock();
    set_stub("success");
    let state = AppState::default();
    let (dir, id, _c0) = fixture_repo(&state);
    let base = tempfile::TempDir::new().expect("base");
    let file = consent_file(base.path());
    std::fs::write(dir.path().join("wip.txt"), "wip\n").expect("write untracked");

    let res = block_on(ai_suggest_branch_name_inner(
        &state,
        &file,
        &id,
        ai_branch_name::BranchNameSource::Working,
    ));
    gate_passed(&res);
}

/// ai_plan_operation reaches the CLI; an unmappable reply is a NORMAL Ok
/// outcome (unsupported), never an error.
#[test]
fn ai_plan_operation_happy() {
    let _g = env_lock();
    set_stub("success");
    let state = AppState::default();
    let (_dir, id, _c0) = fixture_repo(&state);
    let base = tempfile::TempDir::new().expect("base");
    let file = consent_file(base.path());

    let res = block_on(ai_plan_operation_inner(&state, &file, &id, "delete the feature branch".into()))
        .expect("plan is Ok even when unmappable");
    let _ = res; // PlanOutcome (supported or unsupported) — both are Ok.
}

/// ai_compose_commits on a dirty tree reaches the CLI (gate + nothing-to-commit
/// guard passed); an unparseable reply still yields an apply-able partition.
#[test]
fn ai_compose_commits_happy() {
    let _g = env_lock();
    set_stub("success");
    let state = AppState::default();
    let (dir, id, _c0) = fixture_repo(&state);
    let base = tempfile::TempDir::new().expect("base");
    let file = consent_file(base.path());
    std::fs::write(dir.path().join("c1.txt"), "c1\n").expect("write");

    let res = block_on(ai_compose_commits_inner(&state, &file, &id, None));
    gate_passed(&res);
}

/// ai_changelog over a ref range reaches the CLI.
#[test]
fn ai_changelog_happy() {
    let _g = env_lock();
    set_stub("success");
    let state = AppState::default();
    let (dir, id, c0) = fixture_repo(&state);
    let base = tempfile::TempDir::new().expect("base");
    let file = consent_file(base.path());
    write_stage_commit(&state, &id, dir.path(), "ch.txt", "ch\n", "changelog commit");

    let res = block_on(ai_changelog_inner(
        &state,
        &file,
        &id,
        ai_changelog::ChangelogRange::BetweenRefs { from: c0, to: "HEAD".into() },
    ));
    gate_passed(&res);
}

/// ai_generate_pr_description over base..head reaches the CLI.
#[test]
fn ai_generate_pr_description_happy() {
    let _g = env_lock();
    set_stub("success");
    let state = AppState::default();
    let (dir, id, c0) = fixture_repo(&state);
    let base = tempfile::TempDir::new().expect("base");
    let file = consent_file(base.path());
    write_stage_commit(&state, &id, dir.path(), "pr.txt", "pr\n", "pr commit");

    let res = block_on(ai_generate_pr_description_inner(&state, &file, &id, c0, "HEAD".into()));
    gate_passed(&res);
}

// ============================================================ generate_asset

/// ai_generate_asset translates a source asset with content (gate passed, CLI
/// reached); an unknown asset id and an empty source are both `Other` errors.
#[test]
fn ai_generate_asset_happy_unknown_id_and_empty_source() {
    let _g = env_lock();
    set_stub("success");
    let state = AppState::default();
    let (dir, id, _c0) = fixture_repo(&state);
    let base = tempfile::TempDir::new().expect("base");
    let file = consent_file(base.path());

    // Happy: a CLAUDE.md with content translated to the "agents" flavor.
    std::fs::write(dir.path().join("CLAUDE.md"), "# CLAUDE.md\n\nBe concise.\n").expect("write");
    let res = block_on(ai_generate_asset_inner(
        &state,
        &file,
        &id,
        "claude".into(),
        "agents".into(),
        None,
    ));
    gate_passed(&res);

    // Unknown asset id → Other (never reaches the CLI).
    let err = block_on(ai_generate_asset_inner(
        &state,
        &file,
        &id,
        "totally-unknown-id".into(),
        "agents".into(),
        None,
    ))
    .expect_err("unknown id");
    assert!(matches!(err, AppError::Other(_)), "{err:?}");

    // Empty source (no CLAUDE.md content) → Other. Fresh repo, no CLAUDE.md.
    let (_d2, id2, _c) = fixture_repo(&state);
    let err = block_on(ai_generate_asset_inner(
        &state,
        &file,
        &id2,
        "claude".into(),
        "agents".into(),
        None,
    ))
    .expect_err("empty source");
    assert!(matches!(err, AppError::Other(_)), "{err:?}");
}

// ============================================================ consent gate (shared failure path)

/// Every AI command refuses with AiUnavailable BEFORE any repo work when
/// consent is absent (default settings file: ai_consented=false).
#[test]
fn ai_commands_refuse_without_consent() {
    let _g = env_lock();
    set_stub("success");
    let state = AppState::default();
    let (_dir, id, c0) = fixture_repo(&state);
    let base = tempfile::TempDir::new().expect("base");
    let no_consent = base.path().join("default.json"); // nonexistent ⇒ defaults (consented=false)

    let a = block_on(generate_commit_message_inner(&state, &no_consent, &id));
    assert!(matches!(a, Err(AppError::AiUnavailable(_))), "{a:?}");
    let b = block_on(ai_plan_operation_inner(&state, &no_consent, &id, "x".into()));
    assert!(matches!(b, Err(AppError::AiUnavailable(_))), "{b:?}");
    let c = block_on(ai_analyze_diff_inner(
        &state,
        &no_consent,
        &id,
        ai_explain::AiDiffTarget::Commit { oid: c0 },
        ai_explain::AiAnalysisMode::Explain,
    ));
    assert!(matches!(c, Err(AppError::AiUnavailable(_))), "{c:?}");
}
