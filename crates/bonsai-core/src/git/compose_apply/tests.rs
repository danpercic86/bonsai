//! compose_apply tests (§8.9–§8.12). Extracted verbatim from the former
//! inline `mod tests`; shared fixtures live in `test_support`.

use super::test_support::*;
use super::*;
use crate::git::status::read_status;

// ------------------------------------------------------------ §8.9–§8.15

/// §8.9: 3 changed files split 2+1 => two commits; commit-1's delta-to-parent
/// == group-1's files, commit-2 == group-2's file; HEAD advanced by 2. Guarded
/// by `have_git()`, the `git diff-tree` oracle confirms each per-commit delta.
#[test]
fn apply_two_groups_creates_two_commits_each_its_own_delta() {
    let dir = init_scratch();
    let p = dir.path();
    write(p, "base.txt", "base\n");
    stage(p, &["base.txt"]);
    create_commit(p, "base", None, false).expect("commit");
    let base = head_oid(p).expect("base head");

    write(p, "f1.txt", "1\n");
    write(p, "f2.txt", "2\n");
    write(p, "f3.txt", "3\n");
    let plan = ComposePlan {
        groups: vec![
            group(&["f1.txt", "f2.txt"], "feat: g1"),
            group(&["f3.txt"], "test: g2"),
        ],
    };

    let res = apply_composed_commits(p, &plan).expect("apply");
    assert_eq!(res.commits.len(), 2, "two commits created");
    for c in &res.commits {
        assert_eq!(c.oid.len(), 40, "full 40-hex oid: {}", c.oid);
        assert!(c.oid.chars().all(|ch| ch.is_ascii_hexdigit()), "hex oid");
    }
    assert_eq!(res.commits[0].summary, "feat: g1");
    assert_eq!(res.commits[1].summary, "test: g2");

    // HEAD advanced by exactly 2 (base + 2), newest = commits[1].
    assert_eq!(commit_count(p), 3, "base + 2 new commits");
    assert_eq!(head_oid(p).expect("head").to_string(), res.commits[1].oid);
    assert_ne!(head_oid(p).expect("head"), base);

    // Each commit's delta-to-parent is EXACTLY its group's files.
    assert_eq!(delta_paths(p, &res.commits[0].oid), vec!["f1.txt", "f2.txt"]);
    assert_eq!(delta_paths(p, &res.commits[1].oid), vec!["f3.txt"]);

    // CLI oracle (guarded): `git diff-tree` agrees per commit.
    if have_git() {
        assert_eq!(git_delta_names(p, &res.commits[0].oid), vec!["f1.txt", "f2.txt"]);
        assert_eq!(git_delta_names(p, &res.commits[1].oid), vec!["f3.txt"]);
    }
}

/// §8.10: a changed file in NO group is left uncommitted (still dirty in
/// `read_status`) after apply; the covered file is committed (gone from status).
#[test]
fn apply_leaves_uncovered_files_uncommitted() {
    let dir = init_scratch();
    let p = dir.path();
    write(p, "base.txt", "base\n");
    stage(p, &["base.txt"]);
    create_commit(p, "base", None, false).expect("commit");

    write(p, "covered.txt", "c\n");
    write(p, "uncovered.txt", "u\n");
    let plan = ComposePlan {
        groups: vec![group(&["covered.txt"], "only covered")],
    };
    let res = apply_composed_commits(p, &plan).expect("apply");
    assert_eq!(res.commits.len(), 1);

    let st = read_status(p).expect("status");
    let dirty: Vec<&str> = st
        .staged
        .iter()
        .chain(st.unstaged.iter())
        .chain(st.untracked.iter())
        .map(|e| e.path.as_str())
        .collect();
    assert!(dirty.contains(&"uncovered.txt"), "uncovered file stays dirty: {dirty:?}");
    assert!(!dirty.contains(&"covered.txt"), "covered file committed: {dirty:?}");
}

/// §8.11: EVERY validation failure rejects BEFORE any mutation — HEAD unchanged
/// and NOTHING committed. Covers empty message, empty file list, duplicate path,
/// path not in the change set, empty plan, and unset identity.
#[test]
fn apply_rejects_before_any_commit() {
    // --- born repo with identity + two real changes (f1, f2) ---
    let dir = init_scratch();
    let p = dir.path();
    write(p, "base.txt", "base\n");
    stage(p, &["base.txt"]);
    create_commit(p, "base", None, false).expect("commit");
    write(p, "f1.txt", "1\n");
    write(p, "f2.txt", "2\n");
    let orig = head_oid(p).expect("head");

    // Each case: apply, expect the named error, assert NOTHING mutated.
    let expect_untouched = |res: Result<ComposeApplyResult, AppError>| {
        assert!(res.is_err(), "must reject");
        assert_eq!(head_oid(p).expect("head"), orig, "HEAD unchanged");
        assert_eq!(commit_count(p), 1, "no commit landed");
    };

    // empty message => EmptyMessage.
    let e = apply_composed_commits(p, &ComposePlan { groups: vec![group(&["f1.txt"], "   ")] })
        .expect_err("empty message");
    assert!(matches!(e, AppError::EmptyMessage), "got {e:?}");
    expect_untouched(Err(e));

    // empty file list => Other.
    let e = apply_composed_commits(p, &ComposePlan { groups: vec![group(&[], "msg")] })
        .expect_err("empty files");
    assert!(matches!(e, AppError::Other(_)), "got {e:?}");
    expect_untouched(Err(e));

    // duplicate path across groups => Other.
    let e = apply_composed_commits(
        p,
        &ComposePlan {
            groups: vec![group(&["f1.txt"], "a"), group(&["f1.txt"], "b")],
        },
    )
    .expect_err("duplicate path");
    match e {
        AppError::Other(m) => assert!(m.contains("more than one group"), "got {m}"),
        other => panic!("expected Other, got {other:?}"),
    }
    assert_eq!(head_oid(p).expect("head"), orig);
    assert_eq!(commit_count(p), 1);

    // path not in the change set => Other.
    let e = apply_composed_commits(p, &ComposePlan { groups: vec![group(&["ghost.txt"], "m")] })
        .expect_err("unknown path");
    match e {
        AppError::Other(m) => assert!(m.contains("not in the working changes"), "got {m}"),
        other => panic!("expected Other, got {other:?}"),
    }
    assert_eq!(head_oid(p).expect("head"), orig);
    assert_eq!(commit_count(p), 1);

    // empty plan => NothingToCommit.
    let e = apply_composed_commits(p, &ComposePlan { groups: vec![] })
        .expect_err("empty plan");
    assert!(matches!(e, AppError::NothingToCommit), "got {e:?}");
    expect_untouched(Err(e));

    // --- unset identity => ConfigMissing (unborn, no-identity repo) ---
    let dir2 = init_scratch_no_identity();
    let p2 = dir2.path();
    write(p2, "f1.txt", "1\n");
    let e = apply_composed_commits(p2, &ComposePlan { groups: vec![group(&["f1.txt"], "m")] })
        .expect_err("no identity");
    assert!(matches!(e, AppError::ConfigMissing(_)), "got {e:?}");
    assert!(head_oid(p2).is_none(), "still unborn — nothing committed");
}

/// §8.12: a mid-sequence failure rolls back EVERYTHING. Group 2 references a
/// staged-then-reverted file (in the change set — git2's index-aware worktree
/// diff surfaces it — but whose workdir matches HEAD, so after the index reset
/// staging it nets to no change => `create_commit` `NothingToCommit`). Assert
/// HEAD == original, index == HEAD, the working tree STILL holds ALL original
/// changes, and zero commits landed.
#[test]
fn apply_rolls_back_on_mid_sequence_failure() {
    let dir = init_scratch();
    let p = dir.path();
    write(p, "a.txt", "a\n");
    write(p, "b.txt", "b\n");
    stage(p, &["a.txt", "b.txt"]);
    create_commit(p, "base", None, false).expect("commit");
    let orig = head_oid(p).expect("head");

    // a.txt: a genuine working-tree change. b.txt: staged then reverted — its
    // WORKDIR equals HEAD, so committing it after the reset is a no-op.
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

    // ROLLBACK proven: HEAD restored, index back at HEAD, zero commits landed.
    assert_eq!(head_oid(p).expect("head"), orig, "HEAD rolled back to original");
    assert_eq!(commit_count(p), 1, "zero commits landed (only base remains)");
    assert_eq!(index_tree(p), head_tree(p), "index reset to HEAD");

    // WORKING TREE UNTOUCHED: all original on-disk content preserved.
    assert_eq!(std::fs::read_to_string(p.join("a.txt")).expect("a"), "aa\n");
    assert_eq!(std::fs::read_to_string(p.join("b.txt")).expect("b"), "b\n");
}
