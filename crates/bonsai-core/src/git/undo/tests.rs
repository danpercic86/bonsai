use super::*;
use std::path::Path;
use std::process::Command;

// ------------------------------------------------------------------ wire

/// `UndoPlan` serializes with EXACTLY the camelCase keys the TS wire type
/// declares (guards the TS `UndoPlan`/`UndoKind`). Covers a mixed undoable
/// plan, a hard plan, and a not-undoable plan (resetMode → null).
#[test]
fn undo_plan_wire_shape_is_camel_case() {
    let mixed = serde_json::to_value(UndoPlan {
        kind: UndoKind::Commit,
        summary: "commit: add feature".to_string(),
        target_oid: "abc".to_string(),
        target_short: "abc".to_string(),
        reset_mode: Some(ResetMode::Mixed),
        requires_clean_worktree: false,
        worktree_dirty: true,
        undoable: true,
        reason: None,
    })
    .expect("json");
    assert_eq!(
        mixed,
        serde_json::json!({
            "kind": "commit",
            "summary": "commit: add feature",
            "targetOid": "abc",
            "targetShort": "abc",
            "resetMode": "mixed",
            "requiresCleanWorktree": false,
            "worktreeDirty": true,
            "undoable": true,
            "reason": null,
        })
    );

    let hard = serde_json::to_value(UndoPlan {
        kind: UndoKind::Merge,
        summary: "merge feature".to_string(),
        target_oid: "def".to_string(),
        target_short: "def".to_string(),
        reset_mode: Some(ResetMode::Hard),
        requires_clean_worktree: true,
        worktree_dirty: false,
        undoable: true,
        reason: None,
    })
    .expect("json");
    assert_eq!(hard["kind"], "merge");
    assert_eq!(hard["resetMode"], "hard");
    assert_eq!(hard["requiresCleanWorktree"], true);

    let blocked = serde_json::to_value(nothing_to_undo(false)).expect("json");
    assert_eq!(blocked["kind"], "unknown");
    assert_eq!(blocked["undoable"], false);
    assert_eq!(blocked["resetMode"], serde_json::Value::Null);
    assert_eq!(blocked["reason"], "nothing to undo");
}

/// Every `UndoKind` variant serializes to the exact camelCase wire string.
#[test]
fn undo_kind_wire_strings() {
    let cases = [
        (UndoKind::Commit, "commit"),
        (UndoKind::Amend, "amend"),
        (UndoKind::Merge, "merge"),
        (UndoKind::Rebase, "rebase"),
        (UndoKind::FastForward, "fastForward"),
        (UndoKind::CherryPick, "cherryPick"),
        (UndoKind::Revert, "revert"),
        (UndoKind::Reset, "reset"),
        (UndoKind::BranchSwitch, "branchSwitch"),
        (UndoKind::Unknown, "unknown"),
    ];
    for (kind, wire) in cases {
        assert_eq!(serde_json::to_value(kind).expect("json"), serde_json::json!(wire));
    }
}

// ----------------------------------------------------- classifier table

/// The normative prefix → kind truth-table over synthetic reflog messages
/// (the exact tags git writes). First-match-wins.
#[test]
fn classify_truth_table() {
    let cases: &[(&str, UndoKind)] = &[
        ("commit: add feature", UndoKind::Commit),
        ("commit (initial): base", UndoKind::Commit),
        ("commit (merge): resolve", UndoKind::Commit),
        ("commit (amend): tidy message", UndoKind::Amend),
        ("merge feature: Merge made by the 'ort' strategy.", UndoKind::Merge),
        ("pull: Merge made by the 'ort' strategy.", UndoKind::Merge),
        ("rebase (finish): returning to refs/heads/main", UndoKind::Rebase),
        ("rebase -i (finish): returning to refs/heads/main", UndoKind::Rebase),
        ("rebase (start): checkout main", UndoKind::Rebase),
        ("pull: Fast-forward", UndoKind::FastForward),
        ("merge: Fast-forward", UndoKind::FastForward),
        ("pull origin main: Fast-forward", UndoKind::FastForward),
        ("cherry-pick: add feature", UndoKind::CherryPick),
        ("revert: Revert \"add feature\"", UndoKind::Revert),
        ("reset: moving to HEAD~1", UndoKind::Reset),
        ("checkout: moving from feature to main", UndoKind::BranchSwitch),
        ("something totally unexpected", UndoKind::Unknown),
        ("", UndoKind::Unknown),
    ];
    for (msg, want) in cases {
        assert_eq!(classify(msg), *want, "classify({msg:?})");
    }
}

/// Mode mapping: Commit/Amend/Reset → Mixed; Merge/Rebase/FastForward/
/// CherryPick/Revert → Hard; BranchSwitch/Unknown → None.
#[test]
fn reset_mode_per_kind() {
    assert_eq!(UndoKind::Commit.reset_mode(), Some(ResetMode::Mixed));
    assert_eq!(UndoKind::Amend.reset_mode(), Some(ResetMode::Mixed));
    assert_eq!(UndoKind::Reset.reset_mode(), Some(ResetMode::Mixed));
    assert_eq!(UndoKind::Merge.reset_mode(), Some(ResetMode::Hard));
    assert_eq!(UndoKind::Rebase.reset_mode(), Some(ResetMode::Hard));
    assert_eq!(UndoKind::FastForward.reset_mode(), Some(ResetMode::Hard));
    assert_eq!(UndoKind::CherryPick.reset_mode(), Some(ResetMode::Hard));
    assert_eq!(UndoKind::Revert.reset_mode(), Some(ResetMode::Hard));
    assert_eq!(UndoKind::BranchSwitch.reset_mode(), None);
    assert_eq!(UndoKind::Unknown.reset_mode(), None);
}

// ------------------------------------------------ describe_last_undo (git2)

fn init_repo(dir: &Path) -> git2::Repository {
    let mut opts = git2::RepositoryInitOptions::new();
    opts.initial_head("main");
    let repo = git2::Repository::init_opts(dir, &opts).expect("init repo");
    {
        let mut cfg = repo.config().expect("config");
        cfg.set_str("user.name", "Test User").expect("name");
        cfg.set_str("user.email", "test@example.com").expect("email");
    }
    repo
}

fn commit_file(dir: &Path, name: &str, content: &str, msg: &str) -> String {
    std::fs::write(dir.join(name), content).expect("write");
    crate::git::stage::stage_paths(dir, &[name.to_string()]).expect("stage");
    crate::git::commit::create_commit(dir, msg, None, false).expect("commit").oid
}

/// Empty reflog / unborn HEAD → not undoable, kind Unknown, "nothing to undo".
#[test]
fn describe_unborn_is_nothing_to_undo() {
    let dir = crate::testutil::scratch_dir();
    init_repo(dir.path());
    let plan = describe_last_undo(dir.path()).expect("describe");
    assert_eq!(plan.kind, UndoKind::Unknown);
    assert!(!plan.undoable);
    assert_eq!(plan.reason.as_deref(), Some("nothing to undo"));
    assert_eq!(plan.target_oid, "");
}

/// The FIRST commit's reflog[0] is `commit (initial): …` with a 40-zero
/// oldOid → classified Commit but NOT undoable (initial-commit rule).
#[test]
fn describe_initial_commit_is_not_undoable() {
    let dir = crate::testutil::scratch_dir();
    init_repo(dir.path());
    commit_file(dir.path(), "a.txt", "one\n", "base");
    let plan = describe_last_undo(dir.path()).expect("describe");
    assert_eq!(plan.kind, UndoKind::Commit);
    assert!(!plan.undoable, "initial commit is not undoable");
    assert_eq!(plan.reason.as_deref(), Some("cannot undo the initial commit"));
    assert_eq!(plan.target_oid, "", "root target is reported as empty");
    assert_eq!(plan.reset_mode, None);
}

/// A SECOND commit → Commit / Mixed / undoable, target = the first commit
/// (reflog oldOid). Mixed classes are undoable even when the tree is dirty.
#[test]
fn describe_second_commit_is_mixed_undoable() {
    let dir = crate::testutil::scratch_dir();
    let path = dir.path();
    init_repo(path);
    let c1 = commit_file(path, "a.txt", "one\n", "base");
    commit_file(path, "a.txt", "two\n", "edit");
    let plan = describe_last_undo(path).expect("describe");
    assert_eq!(plan.kind, UndoKind::Commit);
    assert!(plan.undoable);
    assert_eq!(plan.reset_mode, Some(ResetMode::Mixed));
    assert!(!plan.requires_clean_worktree);
    assert_eq!(plan.target_oid, c1, "target = the previous HEAD (reflog oldOid)");
    assert_eq!(plan.target_short, &c1[..SHORT_LEN]);
}

// ---------------------------------------------------- CLI oracle (real git)

fn have_git() -> bool {
    let ok = Command::new("git").arg("--version").output().is_ok();
    if !ok && std::env::var("BONSAI_REQUIRE_GIT_STRICT").as_deref() == Ok("1") {
        panic!("BONSAI_REQUIRE_GIT_STRICT=1: `git` CLI required on PATH but not found");
    }
    ok
}

/// Run `git <args>` in `dir`, asserting success; returns trimmed stdout.
fn run_git(dir: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .current_dir(dir)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .args(args)
        .output()
        .expect("spawn git");
    assert!(
        out.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

/// CLI oracle: build a scratch repo doing commit → merge → reset → branch
/// switch and assert `describe_last_undo` classifies each LATEST op with the
/// right kind and target (target == `git rev-parse HEAD@{1}`, i.e. the
/// reflog oldOid), matching the real git binary.
#[test]
fn describe_matches_git_reflog_oracle() {
    if !have_git() {
        eprintln!("skipping describe_matches_git_reflog_oracle: git not on PATH");
        return;
    }
    let dir = crate::testutil::scratch_dir();
    let d = dir.path();

    run_git(d, &["init", "-q"]);
    run_git(d, &["config", "user.name", "Test User"]);
    run_git(d, &["config", "user.email", "test@example.com"]);
    run_git(d, &["config", "commit.gpgsign", "false"]);
    // `symbolic-ref` (not `rev-parse --abbrev-ref`) resolves the branch name
    // while HEAD is still unborn (before the first commit). main | master.
    let main = run_git(d, &["symbolic-ref", "--short", "HEAD"]);

    // --- commit (initial) → not undoable (root oldOid)
    std::fs::write(d.join("a.txt"), "one\n").expect("write");
    run_git(d, &["add", "a.txt"]);
    run_git(d, &["commit", "-q", "-m", "c1"]);
    let p = describe_last_undo(d).expect("describe c1");
    assert_eq!(p.kind, UndoKind::Commit);
    assert!(!p.undoable, "initial commit is not undoable");

    // --- second commit → Commit / Mixed / undoable, target == HEAD@{1}
    std::fs::write(d.join("a.txt"), "two\n").expect("write");
    run_git(d, &["add", "a.txt"]);
    run_git(d, &["commit", "-q", "-m", "c2"]);
    let p = describe_last_undo(d).expect("describe c2");
    assert_eq!(p.kind, UndoKind::Commit);
    assert!(p.undoable);
    assert_eq!(p.reset_mode, Some(ResetMode::Mixed));
    assert_eq!(p.target_oid, run_git(d, &["rev-parse", "HEAD@{1}"]));

    // --- diverge & merge → Merge / Hard, target == HEAD@{1} (pre-merge tip)
    run_git(d, &["checkout", "-q", "-b", "feature"]);
    std::fs::write(d.join("b.txt"), "feat\n").expect("write");
    run_git(d, &["add", "b.txt"]);
    run_git(d, &["commit", "-q", "-m", "c3 on feature"]);
    run_git(d, &["checkout", "-q", &main]);
    std::fs::write(d.join("a.txt"), "three\n").expect("write");
    run_git(d, &["add", "a.txt"]);
    run_git(d, &["commit", "-q", "-m", "c4 on main"]);
    let pre_merge = run_git(d, &["rev-parse", "HEAD"]);
    run_git(d, &["merge", "--no-ff", "--no-edit", "-q", "feature"]);
    let p = describe_last_undo(d).expect("describe merge");
    assert_eq!(p.kind, UndoKind::Merge, "reflog msg: {:?}", p.summary);
    assert_eq!(p.reset_mode, Some(ResetMode::Hard));
    assert!(p.requires_clean_worktree);
    assert!(p.undoable);
    assert_eq!(p.target_oid, run_git(d, &["rev-parse", "HEAD@{1}"]));
    assert_eq!(p.target_oid, pre_merge, "undo target == pre-merge tip");

    // --- reset → Reset / Mixed, target == HEAD@{1} (pre-reset tip)
    let pre_reset = run_git(d, &["rev-parse", "HEAD"]);
    run_git(d, &["reset", "-q", "--soft", "HEAD~1"]);
    let p = describe_last_undo(d).expect("describe reset");
    assert_eq!(p.kind, UndoKind::Reset, "reflog msg: {:?}", p.summary);
    assert_eq!(p.reset_mode, Some(ResetMode::Mixed));
    assert!(p.undoable);
    assert_eq!(p.target_oid, run_git(d, &["rev-parse", "HEAD@{1}"]));
    assert_eq!(p.target_oid, pre_reset, "undo target == pre-reset tip");

    // --- branch switch → BranchSwitch, not undoable
    run_git(d, &["checkout", "-q", "feature"]);
    let p = describe_last_undo(d).expect("describe checkout");
    assert_eq!(p.kind, UndoKind::BranchSwitch, "reflog msg: {:?}", p.summary);
    assert!(!p.undoable);
    assert!(p.reason.is_some());
}
