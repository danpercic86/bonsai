//! T2 Area 1 — external-launch precheck + AI command consent gate: the
//! shared `launch_inner` missing-path precheck rejects before spawning
//! anything, and each AI command inner enforces the backend consent gate
//! (§9.6) BEFORE touching the repo — default settings (`ai_consented=false`)
//! → `AiUnavailable` even with no repo open; once enabled+consented, an
//! unknown repo id → `NoRepo` (the gate passed, `repo_path` then fails).

use super::tests_support::*;
use super::*;

/// P49 (reviewer gap): the shared `launch_inner` missing-path precheck
/// returns `AppError::Io` — and launches **nothing** — when the target path
/// no longer exists. `reveal_in_file_manager` is the runtime-free command (it
/// takes neither an `AppHandle` nor state), so it drives the exact precheck
/// (`commands/external.rs` `launch_inner`, the `!p.exists()` guard) directly.
/// `open_in_terminal`/`open_in_editor` funnel through the *same* precheck but
/// first need an `AppHandle` to resolve the settings template, so they cannot
/// be driven runtime-free here (the tauri "test" feature is avoided on this
/// machine — see `config_commands_require_an_open_repo`); the shared seam is
/// proven via reveal. The `p.exists()` check is the first statement in the
/// spawn_blocking body — before any `SpawnRunner` is constructed — so an `Io`
/// result proves no file-manager / terminal / editor process was spawned.
#[test]
fn external_launch_rejects_missing_path_before_spawning() {
    // Parent dir exists, leaf never created ⇒ guaranteed-missing on every OS,
    // so the precheck short-circuits deterministically.
    let dir = tempfile::TempDir::new().expect("temp dir");
    let missing = dir.path().join("does-not-exist-p49");
    let missing_str = missing.to_string_lossy().into_owned();
    assert!(!missing.exists(), "precondition: the path must not exist");

    let err = tauri::async_runtime::block_on(reveal_in_file_manager(missing_str.clone()))
        .expect_err("a nonexistent path must be rejected by the precheck");
    assert!(
        matches!(err, AppError::Io(_)),
        "missing path must surface as AppError::Io, got {err:?}"
    );
    // The precheck echoes the offending path — confirms this is *our* Io
    // guard (not some incidental filesystem error) and that we never spawned.
    assert!(
        err.to_string().contains(&missing_str),
        "the precheck error must name the offending path: {err}"
    );
}

/// `ai_resolve_conflict` enforces the backend consent gate (§9.6) BEFORE
/// touching the repo: default settings (`ai_consented=false`) → `AiUnavailable`
/// even with no repo open; once enabled+consented, an unknown repo id →
/// `NoRepo` (the gate passed, `repo_path` then fails). Covers the
/// AppHandle-free part of the command via its inner (P13 §6).
#[test]
fn ai_resolve_conflict_enforces_consent_gate_then_no_repo() {
    let state = AppState::default();
    let dir = tempfile::TempDir::new().expect("temp dir");
    let file = dir.path().join("settings.json");

    // No settings file → defaults → not consented → the gate refuses.
    let err = tauri::async_runtime::block_on(ai_resolve_conflict_inner(
        &state,
        &file,
        MISSING_ID,
        "a.txt".to_string(),
    ))
    .expect_err("disabled gate must refuse");
    assert!(matches!(err, AppError::AiUnavailable(_)), "got {err:?}");

    // P13 tester: the gate is `ai_enabled && ai_consented` — the OTHER OR-half.
    // Consented but DISABLED must still refuse (proves it is AND, not OR).
    let s = settings::Settings {
        ai_enabled: false,
        ai_consented: true,
        ..settings::Settings::default()
    };
    settings::save_to(&file, &s).expect("save settings");
    let err = tauri::async_runtime::block_on(ai_resolve_conflict_inner(
        &state,
        &file,
        MISSING_ID,
        "a.txt".to_string(),
    ))
    .expect_err("enabled=false must refuse even when consented");
    assert!(matches!(err, AppError::AiUnavailable(_)), "got {err:?}");

    // Enable + consent; now the gate passes and the missing repo → NoRepo.
    let s = settings::Settings {
        ai_enabled: true,
        ai_consented: true,
        ..settings::Settings::default()
    };
    settings::save_to(&file, &s).expect("save settings");
    let err = tauri::async_runtime::block_on(ai_resolve_conflict_inner(
        &state,
        &file,
        MISSING_ID,
        "a.txt".to_string(),
    ))
    .expect_err("no repo open must be NoRepo");
    assert!(matches!(err, AppError::NoRepo), "got {err:?}");
}

/// P15a §8.5: `generate_commit_message` enforces the same backend consent
/// gate BEFORE touching the repo: default settings (`ai_consented=false`) →
/// `AiUnavailable`; once enabled+consented, an unknown repo id → `NoRepo`
/// (the gate passed, `repo_path` then fails). No CLI needed.
#[test]
fn generate_commit_message_enforces_consent_gate_then_no_repo() {
    let state = AppState::default();
    let dir = tempfile::TempDir::new().expect("temp dir");
    let file = dir.path().join("settings.json");

    // No settings file → defaults → not consented → the gate refuses.
    let err = tauri::async_runtime::block_on(generate_commit_message_inner(
        &state, &file, MISSING_ID,
    ))
    .expect_err("disabled gate must refuse");
    assert!(matches!(err, AppError::AiUnavailable(_)), "got {err:?}");

    // Enable + consent; now the gate passes and the missing repo → NoRepo.
    let s = settings::Settings {
        ai_enabled: true,
        ai_consented: true,
        ..settings::Settings::default()
    };
    settings::save_to(&file, &s).expect("save settings");
    let err = tauri::async_runtime::block_on(generate_commit_message_inner(
        &state, &file, MISSING_ID,
    ))
    .expect_err("no repo open must be NoRepo");
    assert!(matches!(err, AppError::NoRepo), "got {err:?}");
}

/// P54a §9: `ai_compose_commits` enforces the same backend consent gate
/// BEFORE touching the repo: default settings (`ai_consented=false`) →
/// `AiUnavailable`; once enabled+consented, an unknown repo id → `NoRepo`
/// (the gate passed, `repo_path` then fails). No CLI needed.
#[test]
fn ai_compose_commits_enforces_consent_gate_then_no_repo() {
    let state = AppState::default();
    let dir = tempfile::TempDir::new().expect("temp dir");
    let file = dir.path().join("settings.json");

    // No settings file → defaults → not consented → the gate refuses.
    let err = tauri::async_runtime::block_on(ai_compose_commits_inner(
        &state, &file, MISSING_ID, None,
    ))
    .expect_err("disabled gate must refuse");
    assert!(matches!(err, AppError::AiUnavailable(_)), "got {err:?}");

    // Enable + consent; now the gate passes and the missing repo → NoRepo.
    let s = settings::Settings {
        ai_enabled: true,
        ai_consented: true,
        ..settings::Settings::default()
    };
    settings::save_to(&file, &s).expect("save settings");
    let err = tauri::async_runtime::block_on(ai_compose_commits_inner(
        &state,
        &file,
        MISSING_ID,
        Some("keep tests separate".to_string()),
    ))
    .expect_err("no repo open must be NoRepo");
    assert!(matches!(err, AppError::NoRepo), "got {err:?}");
}

/// P53a: `ai_explain_line` enforces the same backend consent gate BEFORE
/// touching the repo: default settings (`ai_consented=false`) →
/// `AiUnavailable`; consented-but-disabled must also refuse (AND, not OR);
/// once enabled+consented, an unknown repo id → `NoRepo` (the gate passed,
/// `repo_path` then fails). No CLI needed.
#[test]
fn ai_explain_line_enforces_consent_gate_then_no_repo() {
    let state = AppState::default();
    let dir = tempfile::TempDir::new().expect("temp dir");
    let file = dir.path().join("settings.json");

    // No settings file → defaults → not consented → the gate refuses.
    let err = tauri::async_runtime::block_on(ai_explain_line_inner(
        &state,
        &file,
        MISSING_ID,
        "a.txt".to_string(),
        1,
        None,
    ))
    .expect_err("disabled gate must refuse");
    assert!(matches!(err, AppError::AiUnavailable(_)), "got {err:?}");

    // Consented but DISABLED must still refuse (the gate is AND, not OR).
    let s = settings::Settings {
        ai_enabled: false,
        ai_consented: true,
        ..settings::Settings::default()
    };
    settings::save_to(&file, &s).expect("save settings");
    let err = tauri::async_runtime::block_on(ai_explain_line_inner(
        &state,
        &file,
        MISSING_ID,
        "a.txt".to_string(),
        1,
        None,
    ))
    .expect_err("enabled=false must refuse even when consented");
    assert!(matches!(err, AppError::AiUnavailable(_)), "got {err:?}");

    // Enable + consent; now the gate passes and the missing repo → NoRepo.
    let s = settings::Settings {
        ai_enabled: true,
        ai_consented: true,
        ..settings::Settings::default()
    };
    settings::save_to(&file, &s).expect("save settings");
    let err = tauri::async_runtime::block_on(ai_explain_line_inner(
        &state,
        &file,
        MISSING_ID,
        "a.txt".to_string(),
        1,
        Some("0123456789abcdef0123456789abcdef01234567".to_string()),
    ))
    .expect_err("no repo open must be NoRepo");
    assert!(matches!(err, AppError::NoRepo), "got {err:?}");
}

/// P53c: `ai_suggest_branch_name` enforces the same backend consent gate
/// BEFORE touching the repo: default settings (`ai_consented=false`) →
/// `AiUnavailable`; consented-but-disabled must also refuse (AND, not OR);
/// once enabled+consented, an unknown repo id → `NoRepo`. Exercises BOTH
/// `BranchNameSource` variants across the stages. No CLI needed.
#[test]
fn ai_suggest_branch_name_enforces_consent_gate_then_no_repo() {
    let state = AppState::default();
    let dir = tempfile::TempDir::new().expect("temp dir");
    let file = dir.path().join("settings.json");

    // No settings file → defaults → not consented → the gate refuses.
    let err = tauri::async_runtime::block_on(ai_suggest_branch_name_inner(
        &state,
        &file,
        MISSING_ID,
        BranchNameSource::Working,
    ))
    .expect_err("disabled gate must refuse");
    assert!(matches!(err, AppError::AiUnavailable(_)), "got {err:?}");

    // Consented but DISABLED must still refuse (the gate is AND, not OR).
    let s = settings::Settings {
        ai_enabled: false,
        ai_consented: true,
        ..settings::Settings::default()
    };
    settings::save_to(&file, &s).expect("save settings");
    let err = tauri::async_runtime::block_on(ai_suggest_branch_name_inner(
        &state,
        &file,
        MISSING_ID,
        BranchNameSource::CommitRange {
            from: "a".repeat(40),
            to: "b".repeat(40),
        },
    ))
    .expect_err("enabled=false must refuse even when consented");
    assert!(matches!(err, AppError::AiUnavailable(_)), "got {err:?}");

    // Enable + consent; now the gate passes and the missing repo → NoRepo.
    let s = settings::Settings {
        ai_enabled: true,
        ai_consented: true,
        ..settings::Settings::default()
    };
    settings::save_to(&file, &s).expect("save settings");
    let err = tauri::async_runtime::block_on(ai_suggest_branch_name_inner(
        &state,
        &file,
        MISSING_ID,
        BranchNameSource::Working,
    ))
    .expect_err("no repo open must be NoRepo");
    assert!(matches!(err, AppError::NoRepo), "got {err:?}");
}

/// P28 §5: `ai_digest` enforces the same backend consent gate BEFORE
/// touching the repo: default settings (`ai_consented=false`) →
/// `AiUnavailable`; once enabled+consented, an unknown repo id → `NoRepo`
/// (the gate passed, `repo_path` then fails). No CLI needed.
#[test]
fn ai_digest_enforces_consent_gate_then_no_repo() {
    let state = AppState::default();
    let dir = tempfile::TempDir::new().expect("temp dir");
    let file = dir.path().join("settings.json");
    let range = || AiDigestRange::LastDays { days: 7 };

    // No settings file → defaults → not consented → the gate refuses.
    let err =
        tauri::async_runtime::block_on(ai_digest_inner(&state, &file, MISSING_ID, range()))
            .expect_err("disabled gate must refuse");
    assert!(matches!(err, AppError::AiUnavailable(_)), "got {err:?}");

    // Enable + consent; now the gate passes and the missing repo → NoRepo.
    let s = settings::Settings {
        ai_enabled: true,
        ai_consented: true,
        ..settings::Settings::default()
    };
    settings::save_to(&file, &s).expect("save settings");
    let err =
        tauri::async_runtime::block_on(ai_digest_inner(&state, &file, MISSING_ID, range()))
            .expect_err("no repo open must be NoRepo");
    assert!(matches!(err, AppError::NoRepo), "got {err:?}");
}

/// P15b §5/§8.5: `ai_analyze_diff` enforces the same backend consent gate
/// BEFORE touching the repo: default settings (`ai_consented=false`) →
/// `AiUnavailable`; once enabled+consented, an unknown repo id → `NoRepo`
/// (the gate passed, `repo_path` then fails). No CLI needed.
#[test]
fn ai_analyze_diff_enforces_consent_gate_then_no_repo() {
    let state = AppState::default();
    let dir = tempfile::TempDir::new().expect("temp dir");
    let file = dir.path().join("settings.json");

    // No settings file → defaults → not consented → the gate refuses.
    let err = tauri::async_runtime::block_on(ai_analyze_diff_inner(
        &state,
        &file,
        MISSING_ID,
        AiDiffTarget::Staged,
        AiAnalysisMode::Review,
    ))
    .expect_err("disabled gate must refuse");
    assert!(matches!(err, AppError::AiUnavailable(_)), "got {err:?}");

    // Enable + consent; now the gate passes and the missing repo → NoRepo.
    let s = settings::Settings {
        ai_enabled: true,
        ai_consented: true,
        ..settings::Settings::default()
    };
    settings::save_to(&file, &s).expect("save settings");
    let err = tauri::async_runtime::block_on(ai_analyze_diff_inner(
        &state,
        &file,
        MISSING_ID,
        AiDiffTarget::Commit {
            oid: "0123456789abcdef0123456789abcdef01234567".to_string(),
        },
        AiAnalysisMode::Explain,
    ))
    .expect_err("no repo open must be NoRepo");
    assert!(matches!(err, AppError::NoRepo), "got {err:?}");
}

/// P64 §4c: `ai_generate_pr_description` enforces the same backend consent
/// gate BEFORE touching the repo: default settings (`ai_consented=false`) →
/// `AiUnavailable`; once enabled+consented, an unknown repo id → `NoRepo`
/// (the gate passed, `repo_path` then fails). No CLI needed.
#[test]
fn ai_generate_pr_description_enforces_consent_gate_then_no_repo() {
    let state = AppState::default();
    let dir = tempfile::TempDir::new().expect("temp dir");
    let file = dir.path().join("settings.json");

    // No settings file → defaults → not consented → the gate refuses.
    let err = tauri::async_runtime::block_on(ai_generate_pr_description_inner(
        &state,
        &file,
        MISSING_ID,
        "main".to_string(),
        "feature".to_string(),
    ))
    .expect_err("disabled gate must refuse");
    assert!(matches!(err, AppError::AiUnavailable(_)), "got {err:?}");

    // Enable + consent; now the gate passes and the missing repo → NoRepo.
    let s = settings::Settings {
        ai_enabled: true,
        ai_consented: true,
        ..settings::Settings::default()
    };
    settings::save_to(&file, &s).expect("save settings");
    let err = tauri::async_runtime::block_on(ai_generate_pr_description_inner(
        &state,
        &file,
        MISSING_ID,
        "main".to_string(),
        "feature".to_string(),
    ))
    .expect_err("no repo open must be NoRepo");
    assert!(matches!(err, AppError::NoRepo), "got {err:?}");
}

/// P15c §5/§8.5: `ai_summarize_range` enforces the same backend consent gate
/// BEFORE touching the repo: default settings (`ai_consented=false`) →
/// `AiUnavailable`; once enabled+consented, an unknown repo id → `NoRepo`
/// (the gate passed, `repo_path` then fails). No CLI needed.
#[test]
fn ai_summarize_range_enforces_consent_gate_then_no_repo() {
    let state = AppState::default();
    let dir = tempfile::TempDir::new().expect("temp dir");
    let file = dir.path().join("settings.json");

    // No settings file → defaults → not consented → the gate refuses.
    let err = tauri::async_runtime::block_on(ai_summarize_range_inner(
        &state,
        &file,
        MISSING_ID,
        "main".to_string(),
        "feature".to_string(),
    ))
    .expect_err("disabled gate must refuse");
    assert!(matches!(err, AppError::AiUnavailable(_)), "got {err:?}");

    // Enable + consent; now the gate passes and the missing repo → NoRepo.
    let s = settings::Settings {
        ai_enabled: true,
        ai_consented: true,
        ..settings::Settings::default()
    };
    settings::save_to(&file, &s).expect("save settings");
    let err = tauri::async_runtime::block_on(ai_summarize_range_inner(
        &state,
        &file,
        MISSING_ID,
        "main".to_string(),
        "feature".to_string(),
    ))
    .expect_err("no repo open must be NoRepo");
    assert!(matches!(err, AppError::NoRepo), "got {err:?}");
}

