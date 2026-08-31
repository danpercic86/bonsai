//! compose_apply tests (§8.12 detached / §8.13–§8.15). Extracted verbatim
//! from the former inline `mod tests`; shared fixtures live in `test_support`.

use super::test_support::*;
use super::*;

/// §8.12 (detached-HEAD variant — reviewer should-fix): the same mid-sequence
/// rollback but with HEAD DETACHED (as after `git checkout <sha>`). This is the
/// one rollback HEAD-state the suite didn't cover — branch HEAD (§8.12) and
/// unborn HEAD (§8.13) are already exercised. It locks the `else` arm of
/// [`rollback`]/[`apply_composed_commits`] that re-points a detached HEAD via
/// `set_head_detached` (a branch-only rollback would either error or wrongly
/// move/create a branch). Group 2 references a staged-then-reverted file (in the
/// change set, but workdir == HEAD, so after the index reset it nets to no change
/// => `create_commit` `NothingToCommit`) to force the group-2 failure.
#[test]
fn apply_rolls_back_on_mid_sequence_failure_detached_head() {
    let dir = init_scratch();
    let p = dir.path();
    write(p, "a.txt", "a\n");
    write(p, "b.txt", "b\n");
    stage(p, &["a.txt", "b.txt"]);
    create_commit(p, "base", None, false).expect("commit");
    let orig = head_oid(p).expect("head");

    // Detach HEAD at the base commit (mirrors `git checkout <sha>`).
    {
        let repo = open_workdir_repo(p).expect("open");
        repo.set_head_detached(orig).expect("detach HEAD");
        assert!(
            !repo.head().expect("head").is_branch(),
            "precondition: HEAD is detached before apply"
        );
    }

    // a.txt: a genuine working-tree change (spans file #1). b.txt: staged then
    // reverted — its WORKDIR equals HEAD, so committing it after the reset is a
    // no-op (spans file #2, and forces the group-2 failure).
    write(p, "a.txt", "aa\n");
    write(p, "b.txt", "bb\n");
    stage(p, &["b.txt"]);
    write(p, "b.txt", "b\n");

    // Both are in the change set (pre-condition for this test's mechanism).
    let changed: Vec<String> = gather_worktree(p)
        .expect("gather")
        .iter()
        .map(|f| f.path.clone())
        .collect();
    assert!(
        changed.contains(&"a.txt".to_string()) && changed.contains(&"b.txt".to_string()),
        "both files must be in the change set: {changed:?}"
    );

    let plan = ComposePlan {
        groups: vec![
            group(&["a.txt"], "commit a"),
            group(&["b.txt"], "commit b (nets to no change)"),
        ],
    };
    let err = apply_composed_commits(p, &plan).expect_err("group 2 nets to no change");
    match err {
        AppError::Other(m) => assert!(m.contains("group 2"), "annotated with group index: {m}"),
        other => panic!("expected Other(group 2 ...), got {other:?}"),
    }

    // ROLLBACK proven on a DETACHED HEAD: HEAD restored to the original detached
    // oid, STILL detached (no branch created/moved), index back at HEAD, zero
    // commits landed.
    assert_eq!(
        head_oid(p).expect("head"),
        orig,
        "detached HEAD rolled back to the original oid"
    );
    {
        let repo = open_workdir_repo(p).expect("open");
        assert!(
            !repo.head().expect("head").is_branch(),
            "HEAD is STILL detached after rollback (not re-attached to a branch)"
        );
    }
    assert_eq!(commit_count(p), 1, "zero commits landed (only base remains)");
    assert_eq!(index_tree(p), head_tree(p), "index reset to HEAD");

    // WORKING TREE UNTOUCHED: all original on-disk content preserved.
    assert_eq!(std::fs::read_to_string(p.join("a.txt")).expect("a"), "aa\n");
    assert_eq!(std::fs::read_to_string(p.join("b.txt")).expect("b"), "b\n");
}

/// §8.13: unborn HEAD + 2 groups => 2 commits, the first is the root (0
/// parents), each with its own delta; and a forced rollback from the unborn
/// anchor returns HEAD to unborn + an empty index (working tree untouched).
#[test]
fn apply_first_commits_on_unborn_head() {
    // --- success path: 2 groups => 2 commits, first is root ---
    let dir = init_scratch();
    let p = dir.path();
    assert!(head_oid(p).is_none(), "starts unborn");
    write(p, "f1.txt", "1\n");
    write(p, "f2.txt", "2\n");
    let plan = ComposePlan {
        groups: vec![group(&["f1.txt"], "root: f1"), group(&["f2.txt"], "second: f2")],
    };
    let res = apply_composed_commits(p, &plan).expect("apply");
    assert_eq!(res.commits.len(), 2);
    assert_eq!(commit_count(p), 2);
    assert_eq!(head_oid(p).expect("head").to_string(), res.commits[1].oid);

    let repo = open_workdir_repo(p).expect("open");
    let root = repo
        .find_commit(git2::Oid::from_str(&res.commits[0].oid).expect("oid"))
        .expect("commit");
    assert_eq!(root.parent_count(), 0, "first commit is the root");
    assert_eq!(delta_paths(p, &res.commits[0].oid), vec!["f1.txt"]);
    assert_eq!(delta_paths(p, &res.commits[1].oid), vec!["f2.txt"]);

    // --- forced rollback from unborn: HEAD returns to unborn + empty index ---
    let dir2 = init_scratch();
    let p2 = dir2.path();
    write(p2, "g1.txt", "1\n");
    write(p2, "g2.txt", "2\n");
    let repo2 = open_workdir_repo(p2).expect("open");
    assert!(repo2.head().is_err(), "unborn anchor");
    // Mirror the apply loop up to a group-2 failure: reset (clear), land group
    // 1's root commit, then roll back from the `None` (unborn) anchor.
    reset_index_to_head(&repo2, None).expect("reset");
    stage(p2, &["g1.txt"]);
    create_commit(p2, "root: g1", None, false).expect("commit");
    assert!(head_oid(p2).is_some(), "root landed");
    rollback(&repo2, None).expect("rollback");

    assert!(head_oid(p2).is_none(), "HEAD back to unborn");
    assert_eq!(commit_count(p2), 0, "no commit reachable");
    let repo3 = open_workdir_repo(p2).expect("open");
    assert!(repo3.index().expect("index").is_empty(), "index emptied");
    assert_eq!(std::fs::read_to_string(p2.join("g1.txt")).expect("g1"), "1\n");
    assert_eq!(std::fs::read_to_string(p2.join("g2.txt")).expect("g2"), "2\n");
}

/// §8.14: a SUCCESSFUL apply never touches the working tree — every changed
/// file's bytes on disk are byte-identical before and after (only index/refs
/// move). Covers a tracked-modified file AND untracked additions.
#[test]
fn apply_does_not_touch_workdir() {
    let dir = init_scratch();
    let p = dir.path();
    write(p, "tracked.txt", "orig\n");
    stage(p, &["tracked.txt"]);
    create_commit(p, "base", None, false).expect("commit");

    // A tracked modification + two untracked additions.
    write(p, "tracked.txt", "modified body\n");
    write(p, "new1.txt", "new one\n");
    write(p, "new2.txt", "new two\n");
    let before: Vec<(String, String)> = ["tracked.txt", "new1.txt", "new2.txt"]
        .iter()
        .map(|f| (f.to_string(), std::fs::read_to_string(p.join(f)).expect("read")))
        .collect();

    let plan = ComposePlan {
        groups: vec![
            group(&["tracked.txt", "new1.txt"], "g1"),
            group(&["new2.txt"], "g2"),
        ],
    };
    apply_composed_commits(p, &plan).expect("apply");

    for (f, bytes) in &before {
        assert_eq!(
            &std::fs::read_to_string(p.join(f)).expect("read after"),
            bytes,
            "working-tree bytes of {f} must be byte-identical after apply"
        );
    }
}

/// §8.15: the result/plan/commit wire shapes are camelCase and match the TS
/// types. `ComposePlan` DESERIALIZES (command input); the result/commit
/// SERIALIZE (command output).
#[test]
fn apply_result_wire_shape_is_camel_case() {
    let v = serde_json::to_value(ComposeApplyResult {
        commits: vec![ComposeCommit {
            oid: "a".repeat(40),
            summary: "feat: x".to_string(),
        }],
    })
    .expect("json");
    assert_eq!(
        v,
        serde_json::json!({ "commits": [{ "oid": "a".repeat(40), "summary": "feat: x" }] })
    );

    // ComposePlan deserializes from the exact JSON the TS `ComposePlan` sends.
    let plan: ComposePlan =
        serde_json::from_str(r#"{"groups":[{"files":["src/a.rs"],"message":"m"}]}"#)
            .expect("deserialize plan");
    assert_eq!(plan.groups.len(), 1);
    assert_eq!(plan.groups[0].files, vec!["src/a.rs".to_string()]);
    assert_eq!(plan.groups[0].message, "m");

    // ComposeCommit standalone casing.
    let c = serde_json::to_value(ComposeCommit {
        oid: "deadbeef".to_string(),
        summary: "s".to_string(),
    })
    .expect("json");
    assert_eq!(c, serde_json::json!({ "oid": "deadbeef", "summary": "s" }));
}
