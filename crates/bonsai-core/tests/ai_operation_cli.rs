//! P55a end-to-end: the REAL `plan_operation` spawn path (contract §11.1/§11.2).
//!
//! Lives in its OWN test binary so the process-global `BONSAI_CLAUDE_BIN` /
//! `BONSAI_STUB_MODE` env vars cannot race the lib unit tests (only ONE module
//! per process spawns the stub — the pattern every other `*_cli.rs` follows).
//! Test-fn names contain "ai_operation" so `cargo test -p bonsai-core
//! ai_operation` runs them alongside the lib unit tests. The stub's `emit_file`
//! mode feeds `plan_operation` an ARBITRARY model reply per call.
//!
//! These complement the lib unit tests (which exercise the post-CLI resolve
//! path): here the FULL `plan_operation` runs — grounding + spawn + parse +
//! resolve — and we assert it WRITES NOTHING for both a Proposed and an
//! Unsupported outcome, and that garbage output is `Unsupported` (not `AiFailed`).

mod common;

use std::path::Path;
use std::sync::{Mutex, MutexGuard};

use bonsai_core::ai::RunOpts;
use bonsai_core::error::AppError;
use bonsai_core::git::ai_operation::{plan_operation, PlanOutcome};
use bonsai_core::git::commit::create_commit;
use bonsai_core::git::stage::stage_paths;

const STUB_MODE_ENV: &str = "BONSAI_STUB_MODE";
const CLAUDE_BIN_ENV: &str = "BONSAI_CLAUDE_BIN";
const ENVELOPE_ENV: &str = "BONSAI_STUB_ENVELOPE";

/// Serialize env-mutating tests: `BONSAI_CLAUDE_BIN` / `BONSAI_STUB_MODE` are
/// process-global and the stub inherits them, so parallel tests would race.
fn env_lock() -> MutexGuard<'static, ()> {
    static LOCK: Mutex<()> = Mutex::new(());
    LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

fn commit(dir: &Path, file: &str, content: &str, msg: &str) -> String {
    std::fs::write(dir.join(file), content).expect("write");
    stage_paths(dir, &[file.to_string()]).expect("stage");
    create_commit(dir, msg, None, false).expect("commit").oid
}

/// git2-init an A→B scratch repo (identity + autocrlf off). Returns (dir, a, b).
fn linear_repo() -> (tempfile::TempDir, String, String) {
    let dir = common::scratch_dir();
    let d = dir.path();
    let repo = git2::Repository::init(d).expect("init");
    {
        let mut cfg = repo.config().expect("config");
        cfg.set_str("user.name", "Test User").expect("name");
        cfg.set_str("user.email", "test@example.com").expect("email");
        cfg.set_bool("core.autocrlf", false).expect("autocrlf");
    }
    let a = commit(d, "a.txt", "a\n", "A");
    let b = commit(d, "b.txt", "b\n", "B");
    (dir, a, b)
}

/// JSON envelope with `result` = the model's reply string. Escapes the inner
/// quotes by hand (no serde_json needed; the intents carry no backslashes).
fn envelope(result: &str) -> String {
    let escaped = result.replace('\\', "\\\\").replace('"', "\\\"");
    format!(
        "{{\"result\":\"{escaped}\",\"is_error\":false,\"total_cost_usd\":0.001,\"type\":\"result\"}}"
    )
}

/// Snapshot of the state a plan MUST NOT touch: HEAD oid, the raw index file, and
/// a worktree file.
fn snapshot(p: &Path) -> (Option<String>, Vec<u8>, Vec<u8>) {
    let repo = git2::Repository::open(p).expect("open");
    let head = repo.head().ok().and_then(|r| r.target()).map(|o| o.to_string());
    let index = std::fs::read(repo.path().join("index")).unwrap_or_default();
    let file = std::fs::read(p.join("a.txt")).unwrap_or_default();
    (head, index, file)
}

/// §11.1: the REAL `plan_operation` writes NOTHING — for a Proposed reset AND for
/// an Unsupported (undoLastMerge on a non-merge HEAD).
#[test]
fn ai_operation_plan_operation_end_to_end_writes_nothing() {
    let _g = env_lock();
    let (dir, a, _b) = linear_repo();
    let d = dir.path();
    let short_a: String = a.chars().take(7).collect();
    // Envelope lives under .git so it is invisible to status/snapshot.
    let env_file = d.join(".git").join("plan_envelope.json");

    std::env::set_var(CLAUDE_BIN_ENV, common::claude_stub_path());
    std::env::set_var(STUB_MODE_ENV, "emit_file");
    std::env::set_var(ENVELOPE_ENV, &env_file);

    let before = snapshot(d);

    // (a) resetToCommit(short A) → Proposed reset; nothing mutates.
    std::fs::write(
        &env_file,
        envelope(&format!(
            r#"{{"intent":"resetToCommit","commit":"{short_a}","keepChanges":true}}"#
        )),
    )
    .expect("write envelope");
    let outcome = plan_operation(d, "take me back to the first commit", RunOpts::default())
        .expect("plan_operation Ok");
    assert!(matches!(outcome, PlanOutcome::Proposed { .. }), "got {outcome:?}");
    assert_eq!(snapshot(d), before, "a Proposed plan must mutate nothing");

    // (b) undoLastMerge on a non-merge HEAD → Unsupported; nothing mutates.
    std::fs::write(&env_file, envelope(r#"{"intent":"undoLastMerge"}"#)).expect("write envelope");
    let outcome =
        plan_operation(d, "undo my last merge", RunOpts::default()).expect("plan_operation Ok");
    assert!(matches!(outcome, PlanOutcome::Unsupported { .. }), "got {outcome:?}");
    assert_eq!(snapshot(d), before, "an Unsupported plan must mutate nothing");

    std::env::remove_var(ENVELOPE_ENV);
    std::env::remove_var(STUB_MODE_ENV);
    std::env::remove_var(CLAUDE_BIN_ENV);
}

/// §11.2: a garbage model reply degrades to `Ok(Unsupported)` — DISTINCT from a
/// genuine CLI failure (nonzero exit → `Err(AiFailed)`).
#[test]
fn ai_operation_unparseable_reply_is_unsupported_not_failed() {
    let _g = env_lock();
    let (dir, _a, _b) = linear_repo();
    let d = dir.path();
    let env_file = d.join(".git").join("plan_envelope.json");

    std::env::set_var(CLAUDE_BIN_ENV, common::claude_stub_path());

    // Garbage reply body → Ok(Unsupported) (fail-closed), NOT an error.
    std::env::set_var(STUB_MODE_ENV, "emit_file");
    std::env::set_var(ENVELOPE_ENV, &env_file);
    std::fs::write(
        &env_file,
        envelope("I will not comply; here is a haiku instead."),
    )
    .expect("write envelope");
    let outcome = plan_operation(d, "order me a pizza", RunOpts::default())
        .expect("garbage reply → Ok(Unsupported)");
    assert!(matches!(outcome, PlanOutcome::Unsupported { .. }), "got {outcome:?}");

    // A genuine CLI failure (nonzero exit) → Err(AiFailed): the DISTINCT path.
    std::env::remove_var(ENVELOPE_ENV);
    std::env::set_var(STUB_MODE_ENV, "nonzero");
    let err = plan_operation(d, "anything", RunOpts::default()).expect_err("nonzero → Err");
    assert!(matches!(err, AppError::AiFailed(_)), "expected AiFailed, got {err:?}");

    std::env::remove_var(STUB_MODE_ENV);
    std::env::remove_var(CLAUDE_BIN_ENV);
}
