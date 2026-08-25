use super::*;
use crate::error::AppError;

// ---------------------------------------------- wire shape (TS mirrors)

/// The serde tag/casing must match the TS MergeOutcome union exactly.
#[test]
fn wire_shapes_are_camel_case_tagged() {
    let v = serde_json::to_value(MergeOutcome::UpToDate).expect("json");
    assert_eq!(v, serde_json::json!({ "kind": "upToDate" }));

    let v = serde_json::to_value(MergeOutcome::FastForwarded {
        branch: "main".to_string(),
        to: "a".repeat(40),
        stashed: true,
    })
    .expect("json");
    assert_eq!(
        v,
        serde_json::json!({ "kind": "fastForwarded", "branch": "main", "to": "a".repeat(40), "stashed": true })
    );

    let v = serde_json::to_value(MergeOutcome::Merged {
        oid: "b".repeat(40),
        stashed: false,
    })
    .expect("json");
    assert_eq!(
        v,
        serde_json::json!({ "kind": "merged", "oid": "b".repeat(40), "stashed": false })
    );

    let v = serde_json::to_value(MergeOutcome::Conflicts {
        paths: vec!["README.md".to_string(), "src/auth.ts".to_string()],
        stashed: true,
    })
    .expect("json");
    assert_eq!(
        v,
        serde_json::json!({ "kind": "conflicts", "paths": ["README.md", "src/auth.ts"], "stashed": true })
    );

    let v = serde_json::to_value(MergeOutcome::StashPopConflicts {
        head: "c".repeat(40),
        paths: vec!["src/app.ts".to_string()],
    })
    .expect("json");
    assert_eq!(
        v,
        serde_json::json!({ "kind": "stashPopConflicts", "head": "c".repeat(40), "paths": ["src/app.ts"] })
    );
}

// -------------------------------------------- §4.3 prepared MERGE_MSG

#[test]
fn prepared_message_is_byte_exact() {
    assert_eq!(
        prepared_merge_message("feature/login", false),
        "Merge branch 'feature/login'"
    );
    assert_eq!(
        prepared_merge_message("origin/main", true),
        "Merge remote-tracking branch 'origin/main'"
    );
}

/// Regression (reviewer MUST-FIX): resolving every conflict as Ours
/// before Abort leaves the index == HEAD tree with zero conflicts, so the
/// `touched` set is EMPTY. A CheckoutBuilder with zero .path() calls
/// matches ALL paths — the empty set must skip the force checkout
/// entirely, or an unrelated pre-merge unstaged edit gets clobbered.
#[test]
fn abort_with_empty_touched_set_preserves_unrelated_unstaged_edit() {
    use crate::git::conflict::{resolve_conflict, ConflictResolution};
    use crate::git::stage::stage_paths;

    let dir = crate::testutil::scratch_dir();
    let repo = git2::Repository::init(dir.path()).expect("init repo");
    {
        let mut cfg = repo.config().expect("config");
        cfg.set_str("user.name", "Test User").expect("name");
        cfg.set_str("user.email", "test@example.com").expect("email");
        cfg.set_bool("core.autocrlf", false).expect("autocrlf");
    }
    let commit_all = |msg: &str, files: &[(&str, &str)]| {
        for (name, content) in files {
            std::fs::write(dir.path().join(name), content).expect("write");
        }
        stage_paths(
            dir.path(),
            &files.iter().map(|(n, _)| n.to_string()).collect::<Vec<_>>(),
        )
        .expect("stage");
        crate::git::commit::create_commit(dir.path(), msg, None, false).expect("commit")
    };

    // Base commit with the conflict file + an unrelated file.
    commit_all("base", &[("a.txt", "base\n"), ("unrelated.txt", "orig\n")]);
    let base_oid = repo.head().expect("HEAD").target().expect("oid");

    // topic edits a.txt one way; main edits it another -> guaranteed conflict.
    repo.branch("topic", &repo.find_commit(base_oid).expect("base"), false)
        .expect("branch");
    commit_all("main change", &[("a.txt", "main\n")]);
    {
        // Commit the divergent topic-side change directly on the branch.
        let sig = git2::Signature::now("Test User", "test@example.com").expect("sig");
        let base = repo.find_commit(base_oid).expect("base commit");
        let mut tb = repo.treebuilder(Some(&base.tree().expect("tree"))).expect("tb");
        let blob = repo.blob(b"topic\n").expect("blob");
        tb.insert("a.txt", blob, 0o100644).expect("insert");
        let tree = repo.find_tree(tb.write().expect("tree oid")).expect("tree");
        repo.commit(
            Some("refs/heads/topic"),
            &sig,
            &sig,
            "topic change\n",
            &tree,
            &[&base],
        )
        .expect("topic commit");
    }

    // Clean tree at merge time -> no autostash (stashed: false). The
    // unrelated UNSTAGED edit is made AFTER the merge pauses, so it is not
    // captured by the autostash and must survive the abort (this test's
    // regression concern is abort's empty-touched-set guard, not P8).
    let outcome = merge_branch(dir.path(), "topic", false).expect("merge");
    assert_eq!(
        outcome,
        MergeOutcome::Conflicts {
            paths: vec!["a.txt".to_string()],
            stashed: false,
        }
    );

    // Unrelated UNSTAGED edit (during the paused merge) that must survive.
    std::fs::write(dir.path().join("unrelated.txt"), "edited but not staged\n")
        .expect("edit unrelated");

    // Resolve the ONLY conflict as Ours: index returns to == HEAD tree,
    // zero conflicts -> abort's touched set is empty.
    resolve_conflict(dir.path(), "a.txt", ConflictResolution::Ours).expect("resolve");

    abort_merge(dir.path()).expect("abort");

    assert_eq!(repo.state(), git2::RepositoryState::Clean);
    let unrelated =
        std::fs::read_to_string(dir.path().join("unrelated.txt")).expect("read unrelated");
    assert_eq!(unrelated, "edited but not staged\n", "unstaged edit clobbered");
    let a = std::fs::read_to_string(dir.path().join("a.txt")).expect("read a.txt");
    assert_eq!(a, "main\n", "a.txt must be back at HEAD's version");
}

// ------------------------------------------------------- preconditions

#[test]
fn merge_preconditions_on_fresh_repo() {
    let dir = crate::testutil::scratch_dir();
    git2::Repository::init(dir.path()).expect("init repo");

    // Unborn HEAD refuses before branch resolution.
    let err = merge_branch(dir.path(), "topic", false).expect_err("unborn");
    match err {
        AppError::Git(m) => assert!(m.contains("no commits yet"), "got: {m}"),
        other => panic!("expected Git, got {other:?}"),
    }

    // commit_merge / abort_merge with no merge in progress.
    let err = commit_merge(dir.path(), "msg", None, false).expect_err("no merge");
    assert!(matches!(err, AppError::NoOperationInProgress(_)));
    let err = abort_merge(dir.path()).expect_err("no merge");
    assert!(matches!(err, AppError::NoOperationInProgress(_)));
}
