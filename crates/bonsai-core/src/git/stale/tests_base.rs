//! Base-resolution, identity-guard, tip-moved, dangling-ref, and CLI-oracle
//! tests for `stale`. Extracted verbatim from the former inline `mod tests`;
//! shared fixtures live in `test_support`.

use super::test_support::*;
use super::*;
use std::collections::BTreeSet;
use std::process::Command;

// --------------------------------------------------- base resolution ordering

/// With no explicit base and no origin/HEAD, resolution falls to local
/// `main`; a repo with neither main/master nor an attached HEAD errors.
#[test]
fn base_resolution_falls_to_main() {
    let dir = crate::testutil::scratch_dir();
    let d = dir.path();
    let repo = init(d);
    let c0 = commit(d, "C0", &[("a.txt", "a\n")]);
    branch_at(&repo, "feat", c0);

    // main exists; no origin/HEAD → base resolves to "main".
    let report = find_stale_branches(d, None).expect("classify");
    assert_eq!(report.base, "main");
}

/// Explicit base wins over everything; a bad base is a whole-call `git` error.
#[test]
fn base_resolution_explicit_and_bad() {
    let dir = crate::testutil::scratch_dir();
    let d = dir.path();
    let repo = init(d);
    let c0 = commit(d, "C0", &[("a.txt", "a\n")]);
    let _c1 = commit(d, "C1", &[("b.txt", "b\n")]);
    branch_at(&repo, "release", c0);

    let report = find_stale_branches(d, Some("release")).expect("classify");
    assert_eq!(report.base, "release");

    match find_stale_branches(d, Some("no-such-ref")) {
        Err(AppError::Git(_)) => {}
        other => panic!("bad base must be Git error, got {other:?}"),
    }
}

// --------------------------------------------- F-A7-1/4 base identity guard

/// The base given as `refs/heads/main`, a bare OID, or a tag at the tip
/// must all protect `main` (F-A7-1). A twin branch AT the base tip is only
/// protected under the OID/tag forms (no branch identity) — under
/// `refs/heads/main` it stays a normal merged candidate.
#[test]
fn base_identity_protects_main_for_refname_oid_and_tag() {
    let dir = crate::testutil::scratch_dir();
    let d = dir.path();
    let repo = init(d);

    let _c0 = commit(d, "C0", &[("a.txt", "a\n")]);
    let c1 = commit(d, "C1", &[("b.txt", "b\n")]);
    let c2 = commit(d, "C2", &[("c.txt", "c\n")]); // main tip

    branch_at(&repo, "dead", c1); // merged, below the tip
    branch_at(&repo, "twin", c2); // merged, AT the base tip
    // HEAD off main so the base guard (not the current guard) is exercised.
    branch_at(&repo, "topic", c2);
    crate::git::branches::checkout_branch(d, "topic").expect("checkout");

    let tip_commit = repo.find_commit(c2).expect("tip");
    repo.tag_lightweight("release", tip_commit.as_object(), false)
        .expect("tag");

    let oid_spec = c2.to_string();
    for (spec, twin_protected) in [
        ("refs/heads/main", false),
        (oid_spec.as_str(), true),
        ("release", true),
    ] {
        let report = find_stale_branches(d, Some(spec)).expect("classify");
        let names: Vec<&str> = report.branches.iter().map(|b| b.name.as_str()).collect();
        assert!(!names.contains(&"main"), "base {spec}: main never listed: {names:?}");
        assert!(names.contains(&"dead"), "base {spec}: dead still listed: {names:?}");
        assert_eq!(
            !names.contains(&"twin"),
            twin_protected,
            "base {spec}: twin protection mismatch: {names:?}"
        );

        let results =
            delete_branches(d, &["main".to_string()], Some(spec)).expect("delete");
        assert_eq!(
            results[0].status,
            BranchDeleteStatus::SkippedBase,
            "base {spec}: deleting main must be SkippedBase, got {results:?}"
        );
        assert!(branch_exists(d, "main"), "base {spec}: main survives");
    }
}

/// A remote-tracking base (`origin/main`) protects the LOCAL `main`
/// (F-A7-4) even when it is fully merged relative to that base.
#[test]
fn remote_base_protects_local_counterpart() {
    let dir = crate::testutil::scratch_dir();
    let d = dir.path();
    let repo = init(d);

    let c0 = commit(d, "C0", &[("a.txt", "a\n")]);
    let c1 = commit(d, "C1", &[("b.txt", "b\n")]); // main tip
    repo.reference("refs/remotes/origin/main", c1, true, "seed")
        .expect("remote ref");
    branch_at(&repo, "dead", c0);
    branch_at(&repo, "topic", c1);
    crate::git::branches::checkout_branch(d, "topic").expect("checkout");

    let report = find_stale_branches(d, Some("origin/main")).expect("classify");
    let names: Vec<&str> = report.branches.iter().map(|b| b.name.as_str()).collect();
    assert!(!names.contains(&"main"), "local counterpart never listed: {names:?}");
    assert!(names.contains(&"dead"), "other merged branches still listed");

    let results = delete_branches(d, &["main".to_string()], Some("origin/main"))
        .expect("delete");
    assert_eq!(results[0].status, BranchDeleteStatus::SkippedBase);
    assert!(branch_exists(d, "main"), "local main survives a remote base");
}

/// The repo's default branch (origin/HEAD target) is never auto-classified
/// stale, whatever base the caller reviews against (F-A7-4).
#[test]
fn default_branch_never_auto_classified() {
    let dir = crate::testutil::scratch_dir();
    let d = dir.path();
    let repo = init(d);

    let _c0 = commit(d, "C0", &[("a.txt", "a\n")]);
    let c1 = commit(d, "C1", &[("b.txt", "b\n")]);
    let _c2 = commit(d, "C2", &[("c.txt", "c\n")]); // main tip, HEAD=main

    branch_at(&repo, "dev", c1); // merged — but it is the DEFAULT branch
    branch_at(&repo, "dead", c1); // merged — an ordinary candidate
    repo.reference("refs/remotes/origin/dev", c1, true, "seed")
        .expect("remote dev");
    repo.reference_symbolic(
        "refs/remotes/origin/HEAD",
        "refs/remotes/origin/dev",
        true,
        "seed origin/HEAD",
    )
    .expect("origin/HEAD");

    let report = find_stale_branches(d, Some("main")).expect("classify");
    let names: Vec<&str> = report.branches.iter().map(|b| b.name.as_str()).collect();
    assert!(!names.contains(&"dev"), "default branch never listed: {names:?}");
    assert!(names.contains(&"dead"), "ordinary merged branch still listed");

    let results = delete_branches(d, &["dev".to_string()], Some("main")).expect("delete");
    assert_eq!(results[0].status, BranchDeleteStatus::SkippedBase);
    assert!(branch_exists(d, "dev"), "default branch survives");
}

// ------------------------------------------------- F-A7-3 tip-moved guard

/// The delete-time tip recheck: unchanged tip → proceed (None); a moved
/// tip → a Failed row naming both oids, never a delete.
#[test]
fn recheck_tip_detects_moved_tip() {
    let dir = crate::testutil::scratch_dir();
    let d = dir.path();
    let repo = init(d);

    let c0 = commit(d, "C0", &[("a.txt", "a\n")]);
    let c1 = commit(d, "C1", &[("b.txt", "b\n")]);
    branch_at(&repo, "b", c1);
    let branch = repo
        .find_branch("b", git2::BranchType::Local)
        .expect("find branch");

    assert!(
        recheck_tip(&branch, "b", c1).is_none(),
        "unchanged tip → safe to delete"
    );
    let row = recheck_tip(&branch, "b", c0).expect("moved tip must be refused");
    assert_eq!(row.status, BranchDeleteStatus::Failed);
    let msg = row.message.as_deref().unwrap_or("");
    assert!(msg.contains("tip moved"), "message names the move: {msg}");
    assert!(
        msg.contains(&c0.to_string()[..7]) && msg.contains(&c1.to_string()[..7]),
        "message carries both short oids: {msg}"
    );
}

// -------------------------------------------- F-A7-9 dangling-ref skipping

/// One dangling branch ref (loose ref file pointing at a nonexistent
/// object) must not abort the scan or the delete batch — it is skipped and
/// everything else still works.
#[test]
fn dangling_branch_ref_is_skipped_not_fatal() {
    let dir = crate::testutil::scratch_dir();
    let d = dir.path();
    let repo = init(d);

    let c0 = commit(d, "C0", &[("a.txt", "a\n")]);
    let _c1 = commit(d, "C1", &[("b.txt", "b\n")]); // main tip
    branch_at(&repo, "dead", c0); // merged → stale

    // A loose ref to an object that does not exist in the odb.
    std::fs::write(
        d.join(".git").join("refs").join("heads").join("dangling"),
        format!("{}\n", "a".repeat(40)),
    )
    .expect("write dangling ref");

    let report = find_stale_branches(d, Some("main")).expect("scan survives");
    let names: Vec<&str> = report.branches.iter().map(|b| b.name.as_str()).collect();
    assert!(names.contains(&"dead"), "healthy stale branch listed: {names:?}");
    assert!(!names.contains(&"dangling"), "dangling ref never classified");

    let results = delete_branches(
        d,
        &["dead".to_string(), "dangling".to_string()],
        Some("main"),
    )
    .expect("delete survives");
    let status_of = |n: &str| {
        results
            .iter()
            .find(|r| r.name == n)
            .map(|r| r.status)
            .expect("row present")
    };
    assert_eq!(status_of("dead"), BranchDeleteStatus::Deleted);
    assert_eq!(status_of("dangling"), BranchDeleteStatus::SkippedNotStale);
    assert!(!branch_exists(d, "dead"));
}

// --------------------------------------------------- §9.2 CLI oracle (git)

/// LOAD-BEARING: the merged set from `find_stale_branches(base="main")`
/// equals `git branch --merged main` minus `main` and the current branch.
/// Skips when `git` is absent (git2-only paths still cover detection).
#[test]
fn merged_matches_git_branch_merged_cli() {
    if !have_git() {
        eprintln!("skipping: `git` CLI not found on PATH");
        return;
    }
    let dir = crate::testutil::scratch_dir();
    let d = dir.path();
    let repo = init(d);

    // main: C0 -> C1 -> C2.
    let c0 = commit(d, "C0", &[("a.txt", "a\n")]);
    let c1 = commit(d, "C1", &[("b.txt", "b\n")]);
    let _c2 = commit(d, "C2", &[("c.txt", "c\n")]);

    // Two branches fully merged into main (ancestors of the tip).
    branch_at(&repo, "merged-1", c0);
    branch_at(&repo, "merged-2", c1);
    // Two branches with unique commits → NOT merged.
    branch_at(&repo, "topic-a", c0);
    commit_on_ref(&repo, "refs/heads/topic-a", c0, &[("ta.txt", "x\n")], "ta");
    branch_at(&repo, "topic-b", c1);
    commit_on_ref(&repo, "refs/heads/topic-b", c1, &[("tb.txt", "y\n")], "tb");

    // git branch --merged main → set, minus main and the current branch.
    let out = Command::new("git")
        .args(["branch", "--merged", "main", "--format=%(refname:short)"])
        .current_dir(d)
        .output()
        .expect("git branch --merged");
    assert!(out.status.success(), "git branch --merged failed");
    let current = read_head_info(&repo).ok().and_then(|h| h.branch_name);
    let cli_merged: BTreeSet<String> = String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(|l| l.trim().trim_start_matches('*').trim().to_string())
        .filter(|n| !n.is_empty() && n != "main" && Some(n) != current.as_ref())
        .collect();

    let report = find_stale_branches(d, Some("main")).expect("classify");
    let ours_merged: BTreeSet<String> = report
        .branches
        .iter()
        .filter(|b| b.reason == StaleReason::Merged)
        .map(|b| b.name.clone())
        .collect();

    assert_eq!(
        ours_merged, cli_merged,
        "our merged set must equal `git branch --merged main` (minus main + current)"
    );
}
