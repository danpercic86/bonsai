//! Detection + delete-safety tests for `stale` (wire shapes, merged, gone
//! upstream, delete-branches safety). Extracted verbatim from the former
//! inline `mod tests`; shared fixtures live in `test_support`.

use super::test_support::*;
use super::*;

// ------------------------------------------------------- §9.1(6) wire shapes

/// `serde_json` asserts camelCase keys and bare-string enum encodings.
#[test]
fn wire_shapes_serialize_camelcase() {
    let sb = StaleBranch {
        name: "feature/x".to_string(),
        tip: "a".repeat(40),
        last_commit_summary: "do a thing".to_string(),
        last_commit_author: "Test User".to_string(),
        last_commit_time: 1_700_000_000,
        reason: StaleReason::GoneUpstream,
        merged: false,
        gone_upstream: true,
        upstream: Some("origin/feature/x".to_string()),
        ahead: Some(3),
        behind: Some(1),
        is_current: false,
    };
    let report = StaleReport {
        base: "main".to_string(),
        base_oid: "b".repeat(40),
        branches: vec![sb],
    };
    let v = serde_json::to_value(&report).expect("serialize report");
    assert_eq!(v["base"], "main");
    assert_eq!(v["baseOid"], "b".repeat(40));
    let b0 = &v["branches"][0];
    assert_eq!(b0["lastCommitSummary"], "do a thing");
    assert_eq!(b0["lastCommitAuthor"], "Test User");
    assert_eq!(b0["lastCommitTime"], 1_700_000_000_i64);
    assert_eq!(b0["goneUpstream"], true);
    assert_eq!(b0["isCurrent"], false);
    // Field-less enum → bare camelCase string.
    assert_eq!(b0["reason"], "goneUpstream");

    let del = BranchDeleteResult {
        name: "feature/x".to_string(),
        status: BranchDeleteStatus::SkippedCurrent,
        message: Some("checked-out branch".to_string()),
    };
    let dv = serde_json::to_value(&del).expect("serialize delete result");
    assert_eq!(dv["name"], "feature/x");
    assert_eq!(dv["status"], "skippedCurrent");
    assert_eq!(dv["message"], "checked-out branch");
    assert_eq!(
        serde_json::to_value(StaleReason::Merged).expect("reason"),
        "merged"
    );
    assert_eq!(
        serde_json::to_value(BranchDeleteStatus::Deleted).expect("status"),
        "deleted"
    );
}

// ------------------------------------------------------- §9.1(7) merged

/// A branch fully merged into base is listed `merged` with `ahead:0`; a
/// branch with a unique commit is not; the base and the current HEAD branch
/// are never listed.
#[test]
fn merged_detection() {
    let dir = crate::testutil::scratch_dir();
    let d = dir.path();
    let repo = init(d);

    let c0 = commit(d, "C0", &[("a.txt", "a\n")]);
    let c1 = commit(d, "C1", &[("b.txt", "b\n")]);
    let _c2 = commit(d, "C2", &[("c.txt", "c\n")]); // main tip

    // Merged: tip C1 is an ancestor of main → main descends from it.
    branch_at(&repo, "feat-merged", c1);
    // Unmerged: a unique commit off C0 that main never sees.
    branch_at(&repo, "wip", c0);
    commit_on_ref(&repo, "refs/heads/wip", c0, &[("w.txt", "w\n")], "wip work");
    // A merged branch that we CHECK OUT → excluded because it is current.
    branch_at(&repo, "cur-merged", c1);
    crate::git::branches::checkout_branch(d, "cur-merged").expect("checkout");

    let report = find_stale_branches(d, Some("main")).expect("classify");
    assert_eq!(report.base, "main");

    let names: Vec<&str> = report.branches.iter().map(|b| b.name.as_str()).collect();
    assert!(names.contains(&"feat-merged"), "merged branch listed: {names:?}");
    assert!(!names.contains(&"wip"), "unmerged branch NOT listed: {names:?}");
    assert!(!names.contains(&"main"), "base never listed: {names:?}");
    assert!(
        !names.contains(&"cur-merged"),
        "current HEAD branch never listed: {names:?}"
    );

    let feat = report
        .branches
        .iter()
        .find(|b| b.name == "feat-merged")
        .expect("feat-merged present");
    assert_eq!(feat.reason, StaleReason::Merged);
    assert!(feat.merged);
    assert_eq!(feat.ahead, Some(0), "merged → 0 commits ahead of base");
}

// ------------------------------------------------------- §9.1(8) gone upstream

/// A branch with a configured upstream whose remote-tracking ref is missing
/// is listed `goneUpstream` (merged:false); a branch with a live upstream is
/// not gone (and, being unmerged, not listed at all).
#[test]
fn gone_upstream_detection() {
    let dir = crate::testutil::scratch_dir();
    let d = dir.path();
    let repo = init(d);

    let c0 = commit(d, "C0", &[("a.txt", "a\n")]);
    let _c1 = commit(d, "C1", &[("b.txt", "b\n")]); // main tip

    // A remote so upstream mapping (refspec → refs/remotes/origin/*) resolves.
    repo.remote("origin", "https://example.invalid/x.git")
        .expect("add remote");

    // `gone`: unique commit (so NOT merged) + configured upstream, but no
    // matching refs/remotes/origin/gone → upstream() errs → gone.
    branch_at(&repo, "gone", c0);
    commit_on_ref(&repo, "refs/heads/gone", c0, &[("g.txt", "g\n")], "gone work");
    {
        let mut cfg = repo.config().expect("config");
        cfg.set_str("branch.gone.remote", "origin").expect("remote");
        cfg.set_str("branch.gone.merge", "refs/heads/gone")
            .expect("merge");
    }

    // `live`: unique commit + a present remote-tracking ref → upstream Ok.
    let live_tip =
        commit_on_ref(&repo, "refs/heads/live", c0, &[("l.txt", "l\n")], "live work");
    repo.reference(
        "refs/remotes/origin/live",
        live_tip,
        true,
        "seed remote-tracking",
    )
    .expect("remote ref");
    {
        let mut cfg = repo.config().expect("config");
        cfg.set_str("branch.live.remote", "origin").expect("remote");
        cfg.set_str("branch.live.merge", "refs/heads/live")
            .expect("merge");
    }

    let report = find_stale_branches(d, Some("main")).expect("classify");
    let gone = report
        .branches
        .iter()
        .find(|b| b.name == "gone")
        .expect("gone listed");
    assert_eq!(gone.reason, StaleReason::GoneUpstream);
    assert!(gone.gone_upstream, "gone flag set");
    assert!(!gone.merged, "gone branch is not merged");
    assert_eq!(gone.upstream.as_deref(), Some("origin/gone"));

    assert!(
        !report.branches.iter().any(|b| b.name == "live"),
        "branch with a live upstream (and unmerged) is not listed"
    );
}

// ----------------------------------------------- §9.1(9) delete-branches safety

/// A set mixing a stale name, the current branch, the base, a non-stale
/// branch, and a missing name yields the matching statuses; ONLY the stale
/// branch is actually gone afterward; a fabricated non-stale name is NEVER
/// deleted (defense-in-depth — the server ignores the caller's list).
#[test]
fn delete_branches_safety() {
    let dir = crate::testutil::scratch_dir();
    let d = dir.path();
    let repo = init(d);

    let c0 = commit(d, "C0", &[("a.txt", "a\n")]);
    let c1 = commit(d, "C1", &[("b.txt", "b\n")]);
    let _c2 = commit(d, "C2", &[("c.txt", "c\n")]); // main tip, HEAD=main (current+base)

    branch_at(&repo, "merged-stale", c1); // merged → safe
    branch_at(&repo, "not-stale", c0);
    commit_on_ref(&repo, "refs/heads/not-stale", c0, &[("n.txt", "n\n")], "unique");

    // Sanity: the classifier sees exactly `merged-stale` as safe.
    let report = find_stale_branches(d, Some("main")).expect("classify");
    let safe: Vec<&str> = report.branches.iter().map(|b| b.name.as_str()).collect();
    assert_eq!(safe, vec!["merged-stale"], "only merged-stale is safe");

    let names = vec![
        "merged-stale".to_string(), // Deleted
        "main".to_string(),         // SkippedCurrent (main is BOTH current HEAD and base;
        // the current check runs first per §4.3)
        "not-stale".to_string(), // SkippedNotStale
        "ghost".to_string(),     // SkippedNotStale (not even a branch, never in safe set)
    ];
    let results = delete_branches(d, &names, Some("main")).expect("delete");

    let status_of = |n: &str| {
        results
            .iter()
            .find(|r| r.name == n)
            .map(|r| r.status)
            .unwrap_or_else(|| panic!("no result for {n}"))
    };
    assert_eq!(status_of("merged-stale"), BranchDeleteStatus::Deleted);
    // `main` is BOTH current and base; the current check runs first (§4.3 order).
    assert_eq!(status_of("main"), BranchDeleteStatus::SkippedCurrent);
    assert_eq!(status_of("not-stale"), BranchDeleteStatus::SkippedNotStale);
    assert_eq!(status_of("ghost"), BranchDeleteStatus::SkippedNotStale);

    // F-A7-5: the Deleted row records the deleted tip for recovery.
    let deleted_row = results
        .iter()
        .find(|r| r.name == "merged-stale")
        .expect("row present");
    assert!(
        deleted_row.message.as_deref().unwrap_or("").starts_with("was at "),
        "Deleted row must carry 'was at <short-oid>', got {:?}",
        deleted_row.message
    );

    // Only the stale branch is gone; every other ref survives.
    assert!(!branch_exists(d, "merged-stale"), "stale branch deleted");
    assert!(branch_exists(d, "not-stale"), "non-stale branch untouched");
    assert!(branch_exists(d, "main"), "base branch untouched");
}

/// The current HEAD branch is refused even if the caller forces it, and a
/// non-stale name is never deleted even when explicitly listed.
#[test]
fn delete_branches_refuses_current() {
    let dir = crate::testutil::scratch_dir();
    let d = dir.path();
    let repo = init(d);

    let c0 = commit(d, "C0", &[("a.txt", "a\n")]);
    let c1 = commit(d, "C1", &[("b.txt", "b\n")]); // main tip

    // Check out a merged branch so current != base.
    branch_at(&repo, "topic", c1);
    crate::git::branches::checkout_branch(d, "topic").expect("checkout topic");
    // A stale (merged) branch to prove the batch still deletes the safe one.
    branch_at(&repo, "old", c0);

    // current=topic, base=main → `main` exercises the distinct SkippedBase arm.
    let results = delete_branches(
        d,
        &["topic".to_string(), "main".to_string(), "old".to_string()],
        Some("main"),
    )
    .expect("delete");

    let status_of = |n: &str| {
        results
            .iter()
            .find(|r| r.name == n)
            .map(|r| r.status)
            .expect("result present")
    };
    assert_eq!(status_of("topic"), BranchDeleteStatus::SkippedCurrent);
    assert_eq!(status_of("main"), BranchDeleteStatus::SkippedBase);
    assert_eq!(status_of("old"), BranchDeleteStatus::Deleted);
    assert!(branch_exists(d, "topic"), "current branch never deleted");
    assert!(branch_exists(d, "main"), "base branch never deleted");
    assert!(!branch_exists(d, "old"), "merged branch deleted");
}

