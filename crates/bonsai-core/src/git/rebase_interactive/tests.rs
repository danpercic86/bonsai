//! Interactive-rebase engine tests. Extracted verbatim from the former
//! inline `mod tests` (file-size discipline).

use super::*;
use crate::testutil::scratch_dir;

fn sig() -> git2::Signature<'static> {
    git2::Signature::now("Test", "test@example.com").expect("sig")
}

/// Linear repo of `n` commits on the default branch (each adds `f{i}.txt`).
/// Returns (dir, oids oldest-first). Sets a repo-local identity.
fn linear_repo(n: usize) -> (tempfile::TempDir, Vec<String>) {
    let dir = scratch_dir();
    let repo = git2::Repository::init(dir.path()).expect("init");
    {
        let mut cfg = repo.config().expect("config");
        cfg.set_str("user.name", "Test").expect("name");
        cfg.set_str("user.email", "test@example.com").expect("email");
    }
    let s = sig();
    let mut oids = Vec::new();
    let mut parents: Vec<git2::Commit> = Vec::new();
    for i in 0..n {
        std::fs::write(dir.path().join(format!("f{i}.txt")), format!("c{i}\n")).expect("write");
        let mut idx = repo.index().expect("index");
        idx.add_all(["*"].iter(), git2::IndexAddOption::DEFAULT, None)
            .expect("add");
        idx.write().expect("write index");
        let tree = repo.find_tree(idx.write_tree().expect("tree")).expect("find tree");
        let parent_refs: Vec<&git2::Commit> = parents.iter().collect();
        let oid = repo
            .commit(Some("HEAD"), &s, &s, &format!("c{i}"), &tree, &parent_refs)
            .expect("commit");
        oids.push(oid.to_string());
        parents = vec![repo.find_commit(oid).expect("find commit")];
    }
    (dir, oids)
}

// ------------------------------------------------ wire shapes (TS mirrors)

#[test]
fn rebase_action_round_trips() {
    for (json, action) in [
        ("\"pick\"", RebaseAction::Pick),
        ("\"reword\"", RebaseAction::Reword),
        ("\"squash\"", RebaseAction::Squash),
        ("\"fixup\"", RebaseAction::Fixup),
        ("\"drop\"", RebaseAction::Drop),
    ] {
        let a: RebaseAction = serde_json::from_str(json).expect("de");
        assert_eq!(a, action);
        assert_eq!(serde_json::to_string(&action).expect("ser"), json);
    }
}

#[test]
fn todo_op_wire_shape_is_camel_case() {
    let op = RebaseTodoOp {
        oid: "a".repeat(40),
        action: RebaseAction::Reword,
        new_message: Some("hi".to_string()),
    };
    let v = serde_json::to_value(&op).expect("json");
    assert_eq!(
        v,
        serde_json::json!({ "oid": "a".repeat(40), "action": "reword", "newMessage": "hi" })
    );
    // newMessage defaults to None when absent.
    let s = format!("{{\"oid\":\"{}\",\"action\":\"pick\"}}", "b".repeat(40));
    let op2: RebaseTodoOp = serde_json::from_str(&s).expect("de");
    assert_eq!(op2.action, RebaseAction::Pick);
    assert_eq!(op2.new_message, None);
}

// ------------------------------------------------------- get plan

#[test]
fn plan_is_oldest_first_all_pick() {
    let (dir, oids) = linear_repo(3);
    let plan = get_interactive_plan(dir.path(), &oids[0]).expect("plan");
    assert_eq!(plan.len(), 2, "base..HEAD excludes the base commit");
    assert_eq!(plan[0].oid, oids[1], "oldest kept commit first");
    assert_eq!(plan[1].oid, oids[2]);
    assert!(plan
        .iter()
        .all(|t| t.action == RebaseAction::Pick && t.new_message.is_none()));
}

#[test]
fn plan_rejects_non_ancestor_base() {
    let (dir, _oids) = linear_repo(2);
    let err = get_interactive_plan(dir.path(), &"0".repeat(40)).expect_err("bad base");
    assert!(matches!(err, AppError::Git(_)));
}

// ------------------------------------------------------- validate_todos

#[test]
fn validate_rejects_bad_plans() {
    let dir = scratch_dir();
    let repo = git2::Repository::init(dir.path()).expect("init");
    let o = "a".repeat(40);

    // empty
    assert!(matches!(validate_todos(&repo, &[]), Err(AppError::Git(_))));

    // all drop
    let all_drop = vec![RebaseTodoOp {
        oid: o.clone(),
        action: RebaseAction::Drop,
        new_message: None,
    }];
    assert!(matches!(
        validate_todos(&repo, &all_drop),
        Err(AppError::Git(_))
    ));

    // squash first (no predecessor)
    let squash_first = vec![RebaseTodoOp {
        oid: o.clone(),
        action: RebaseAction::Squash,
        new_message: None,
    }];
    match validate_todos(&repo, &squash_first) {
        Err(AppError::Git(m)) => assert!(m.contains("must follow a pick"), "got: {m}"),
        other => panic!("expected Git, got {other:?}"),
    }

    // reword without a message
    let reword_no_msg = vec![RebaseTodoOp {
        oid: o.clone(),
        action: RebaseAction::Reword,
        new_message: None,
    }];
    match validate_todos(&repo, &reword_no_msg) {
        Err(AppError::Git(m)) => assert!(m.contains("reword requires a message"), "got: {m}"),
        other => panic!("expected Git, got {other:?}"),
    }
}

// ------------------------------------------------------- preconditions

#[test]
fn start_and_ops_on_fresh_repo() {
    let dir = scratch_dir();
    git2::Repository::init(dir.path()).expect("init");

    // Unborn HEAD refuses.
    let err = start_interactive_rebase(dir.path(), &"0".repeat(40), vec![]).expect_err("unborn");
    match err {
        AppError::Git(m) => assert!(m.contains("no commits yet"), "got: {m}"),
        other => panic!("expected Git, got {other:?}"),
    }

    // continue / skip / abort with no interactive rebase.
    assert!(matches!(
        interactive_continue(dir.path()).expect_err("no op"),
        AppError::NoOperationInProgress(_)
    ));
    assert!(matches!(
        interactive_skip(dir.path()).expect_err("no op"),
        AppError::NoOperationInProgress(_)
    ));
    assert!(matches!(
        interactive_abort(dir.path()).expect_err("no op"),
        AppError::NoOperationInProgress(_)
    ));
}

#[test]
fn state_round_trips_on_disk() {
    let (dir, oids) = linear_repo(2);
    let repo = git2::Repository::open(dir.path()).expect("open");
    let state = InteractiveState {
        version: 1,
        head_name: "main".to_string(),
        original_tip: oids[1].clone(),
        onto: oids[0].clone(),
        todos: vec![RebaseTodoOp {
            oid: oids[1].clone(),
            action: RebaseAction::Pick,
            new_message: None,
        }],
        cursor: 0,
        committed: 0,
        paused: false,
        warnings: Vec::new(),
    };
    assert!(!interactive_in_progress(&repo));
    write_state(&repo, &state).expect("write");
    assert!(interactive_in_progress(&repo));
    assert_eq!(read_state(&repo).expect("read"), state);
    assert_eq!(effective_total(&state), 1);
    remove_state(&repo);
    assert!(!interactive_in_progress(&repo));
}
