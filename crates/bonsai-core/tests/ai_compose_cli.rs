//! P54a scratch-repo test for `compose_commits` (contract §8.6, end-to-end).
//!
//! Drives a real HEAD→working-tree change set on a git2 scratch repo with the
//! local `claude` CLI replaced by the committed stub
//! (`tests/fixtures/claude_stub.cmd` / `.sh`) selected via `BONSAI_CLAUDE_BIN` +
//! `BONSAI_STUB_MODE`. No network, no real CLI. Mirrors the `ai_commit_cli.rs`
//! harness. Lives in its OWN test binary so the process-global
//! `BONSAI_CLAUDE_BIN` cannot race the lib unit tests.
//!
//! Proves: (1) the grounding payload that reaches the CLI's stdin carries the
//! CHANGED FILES header, the EXACT paths, and per-file `===== FILE:` blocks
//! (the WHY-not-WHAT grounding); (2) the returned `ComposeProposal` carries the
//! parsed `cost_usd`; (3) the referee degrades a non-JSON stub body to an
//! all-`unassigned` apply-able partition (never an error).
//!
//! Scratch repos are built with git2 only (compose_commits reads via git2, not
//! the `git` CLI), so this test does not depend on `git` being on PATH.

mod common;

use std::path::Path;
use std::sync::{Mutex, MutexGuard};

use bonsai_core::ai::RunOpts;
use bonsai_core::git::ai_compose::compose_commits;
use bonsai_core::git::commit::create_commit;
use bonsai_core::git::stage::stage_paths;

const STUB_MODE_ENV: &str = "BONSAI_STUB_MODE";
const CLAUDE_BIN_ENV: &str = "BONSAI_CLAUDE_BIN";
const STDIN_DUMP_ENV: &str = "BONSAI_STUB_STDIN_DUMP";

/// Serialize env-mutating tests: `BONSAI_CLAUDE_BIN` / `BONSAI_STUB_MODE` are
/// process-global and the stub inherits them, so parallel tests would race.
fn env_lock() -> MutexGuard<'static, ()> {
    static LOCK: Mutex<()> = Mutex::new(());
    LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

fn write(dir: &Path, name: &str, content: &str) {
    std::fs::write(dir.join(name), content).expect("write fixture file");
}

/// git2-init a scratch repo with identity + autocrlf off; base-commit two files
/// so the later edits register as HEAD→workdir changes.
fn dirty_repo() -> tempfile::TempDir {
    let dir = common::scratch_dir();
    let d = dir.path();
    let repo = git2::Repository::init(d).expect("init repo");
    {
        let mut cfg = repo.config().expect("config");
        cfg.set_str("user.name", "Test User").expect("name");
        cfg.set_str("user.email", "test@example.com").expect("email");
        cfg.set_bool("core.autocrlf", false).expect("autocrlf");
    }
    write(d, "app.rs", "fn main() {}\n");
    write(d, "lib.rs", "pub fn a() {}\n");
    stage_paths(d, &["app.rs".into(), "lib.rs".into()]).expect("stage base");
    create_commit(d, "base", None).expect("base commit");

    // Three distinct change kinds vs HEAD: a STAGED edit, an UNSTAGED edit, and
    // an UNTRACKED add — all must appear in the one grounding payload.
    write(d, "app.rs", "fn main() { STAGED_EDIT(); }\n");
    stage_paths(d, &["app.rs".into()]).expect("stage edit");
    write(d, "lib.rs", "pub fn a() { UNSTAGED_EDIT(); }\n");
    write(d, "notes.md", "UNTRACKED_DOC\n");
    dir
}

/// §8.6: the grounding reaches the CLI stdin (exact paths + FILE blocks) and the
/// proposal carries the parsed cost; a non-JSON stub body degrades to an
/// all-`unassigned` apply-able partition (the referee never errors).
#[test]
fn ai_compose_grounding_reaches_stdin_and_returns_proposal() {
    let _g = env_lock();
    let dir = dirty_repo();
    let d = dir.path();

    // Capture the stdin the stub receives; `dump_stdin` still emits the success
    // envelope body ("MERGED_BODY_OK", cost 0.012).
    let dump = d.join("stdin_dump.txt");
    std::env::set_var(CLAUDE_BIN_ENV, common::claude_stub_path());
    std::env::set_var(STUB_MODE_ENV, "dump_stdin");
    std::env::set_var(STDIN_DUMP_ENV, &dump);

    let proposal = compose_commits(d, Some("keep docs separate"), RunOpts::default())
        .expect("proposal (dump_stdin stub)");

    std::env::remove_var(STDIN_DUMP_ENV);
    std::env::remove_var(STUB_MODE_ENV);
    std::env::remove_var(CLAUDE_BIN_ENV);

    let payload = std::fs::read_to_string(&dump).expect("stub must have written the stdin dump");

    // Grounding shape: the CHANGED FILES header, the exact paths, and FILE blocks.
    assert!(
        payload.contains("CHANGED FILES (assign each to exactly one group; use these exact paths):"),
        "payload should carry the CHANGED FILES header; got:\n{payload}"
    );
    for p in ["app.rs", "lib.rs", "notes.md"] {
        assert!(payload.contains(p), "payload should list {p}; got:\n{payload}");
    }
    assert!(
        payload.contains("===== FILE:"),
        "payload should carry per-file diff blocks; got:\n{payload}"
    );
    // The edits reach stdin as +-prefixed diff lines (intent context).
    assert!(
        payload.contains("+fn main() { STAGED_EDIT(); }")
            && payload.contains("UNSTAGED_EDIT")
            && payload.contains("UNTRACKED_DOC"),
        "payload should carry the staged/unstaged/untracked change bodies; got:\n{payload}"
    );

    // The cost is parsed from the envelope.
    assert_eq!(proposal.cost_usd, Some(0.012), "cost parsed from the stub envelope");

    // The non-JSON stub body ("MERGED_BODY_OK") is UNPARSEABLE => the referee
    // degrades to an all-unassigned apply-able partition (never an error).
    assert!(
        proposal.groups.is_empty(),
        "a non-JSON body yields no groups; got {:?}",
        proposal.groups
    );
    let mut got = proposal.unassigned.clone();
    got.sort();
    assert_eq!(
        got,
        vec!["app.rs".to_string(), "lib.rs".to_string(), "notes.md".to_string()],
        "every changed file must land in `unassigned` when parsing degrades"
    );
    assert!(!proposal.notes.is_empty(), "the degrade must be noted for the UI");
}
