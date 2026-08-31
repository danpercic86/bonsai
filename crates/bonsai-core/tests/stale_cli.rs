//! P25 B4 end-to-end CLI-oracle tests for stale-branch cleanup (contract §9.2).
//!
//! The unit tests in `git/stale.rs` cover classification/deletion in isolation
//! with git2-built fixtures; THIS file cross-checks the *destructive* path
//! (`delete_branches`) against the real `git` CLI end-to-end, on a scratch repo
//! built entirely with the `git` binary. It pins the load-bearing safety
//! guarantee: an UNMERGED branch handed to `delete_branches` survives.
//!
//! All scratch repos live under `D:\Data\Temp\bonsai-scratch` (C: is full). Each
//! test skips (passes with a note) when `git` is not on PATH.

mod common;

use std::collections::BTreeSet;
use std::path::Path;

use bonsai_core::git::stale::{
    delete_branches, find_stale_branches, BranchDeleteStatus, StaleReason,
};
use common::{commit_fixed, git, init_repo};

macro_rules! require_git {
    () => {
        if !common::have_git() {
            eprintln!("skipping: `git` CLI not found on PATH");
            return;
        }
    };
}

/// The bogus name used across tests — never a real branch.
const BOGUS: &str = "does-not-exist-xyz";

/// Builds the shared fixture repo and returns its TempDir. Layout:
///
/// - `main`: C0 → C1 → C2 (the base + tip).
/// - `merged-a` @ C0, `merged-b` @ C1 — both ancestors of `main` → **merged**.
/// - `unmerged` — off C0 with a unique commit, NO upstream → neither merged nor
///   gone → NOT stale (the safety subject).
/// - `gone` — off C0 with a unique commit + a configured upstream
///   (`branch.gone.remote/merge`) whose remote-tracking ref is absent →
///   **goneUpstream** (stale) but unmerged.
/// - `dev` @ C2 (merged), checked out → the **current** branch (so `main` and
///   the current branch are distinct, exercising SkippedBase vs SkippedCurrent).
fn build_fixture() -> tempfile::TempDir {
    let dir = init_repo();
    let path = dir.path();

    std::fs::write(path.join("a.txt"), "a\n").expect("write a");
    git(path, &["add", "-A"]);
    commit_fixed(path, "C0");
    let c0 = git(path, &["rev-parse", "HEAD"]);

    std::fs::write(path.join("b.txt"), "b\n").expect("write b");
    git(path, &["add", "-A"]);
    commit_fixed(path, "C1");
    let c1 = git(path, &["rev-parse", "HEAD"]);

    std::fs::write(path.join("c.txt"), "c\n").expect("write c");
    git(path, &["add", "-A"]);
    commit_fixed(path, "C2"); // main tip

    // Two branches fully merged into main (ancestors of the tip).
    git(path, &["branch", "merged-a", &c0]);
    git(path, &["branch", "merged-b", &c1]);

    // Unmerged: a unique commit off C0 that main never sees. No upstream.
    git(path, &["checkout", "-b", "unmerged", &c0]);
    std::fs::write(path.join("u.txt"), "u\n").expect("write u");
    git(path, &["add", "-A"]);
    commit_fixed(path, "unique on unmerged");
    git(path, &["checkout", "main"]);

    // Gone: a unique commit off C0 (so NOT merged) + a configured upstream whose
    // remote-tracking ref is missing → gone. A remote exists so the refspec maps.
    git(path, &["checkout", "-b", "gone", &c0]);
    std::fs::write(path.join("g.txt"), "g\n").expect("write g");
    git(path, &["add", "-A"]);
    commit_fixed(path, "unique on gone");
    git(path, &["checkout", "main"]);
    git(path, &["remote", "add", "origin", "https://example.invalid/x.git"]);
    git(path, &["config", "branch.gone.remote", "origin"]);
    git(path, &["config", "branch.gone.merge", "refs/heads/gone"]);

    // dev @ main tip, checked out → the current branch (merged, but excluded as current).
    git(path, &["branch", "dev"]);
    git(path, &["checkout", "dev"]);

    dir
}

/// The set of local branch names from `git branch --list`.
fn local_branches(path: &Path) -> BTreeSet<String> {
    git(path, &["branch", "--list", "--format=%(refname:short)"])
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|n| !n.is_empty())
        .collect()
}

/// `git branch --merged main` minus `main` and the current branch — the CLI
/// oracle for the merged-reason set.
fn cli_merged_minus_main_current(path: &Path) -> BTreeSet<String> {
    let current = git(path, &["branch", "--show-current"]);
    git(
        path,
        &["branch", "--merged", "main", "--format=%(refname:short)"],
    )
    .lines()
    .map(|l| l.trim().trim_start_matches('*').trim().to_string())
    .filter(|n| !n.is_empty() && n != "main" && *n != current)
    .collect()
}

// ------------------------------------------------------- §9.2 (1) classify

/// `find_stale_branches(main)`: the merged-reason set equals
/// `git branch --merged main` minus main + current; the unmerged branch is
/// absent; the gone branch is present as `goneUpstream`.
#[test]
fn find_stale_matches_git_merged_and_flags_gone() {
    require_git!();
    let dir = build_fixture();
    let path = dir.path();

    let report = find_stale_branches(path, Some("main")).expect("classify");
    assert_eq!(report.base, "main");

    // Merged-reason set == CLI oracle.
    let ours_merged: BTreeSet<String> = report
        .branches
        .iter()
        .filter(|b| b.reason == StaleReason::Merged)
        .map(|b| b.name.clone())
        .collect();
    let cli_merged = cli_merged_minus_main_current(path);
    assert_eq!(
        ours_merged, cli_merged,
        "merged set must equal `git branch --merged main` minus main + current"
    );
    // The fixture pins the concrete set.
    assert_eq!(
        ours_merged,
        BTreeSet::from(["merged-a".to_string(), "merged-b".to_string()])
    );

    let names: BTreeSet<String> = report.branches.iter().map(|b| b.name.clone()).collect();
    // Unmerged (no upstream) is absent.
    assert!(!names.contains("unmerged"), "unmerged must not be listed: {names:?}");
    // The base and current branch are never listed.
    assert!(!names.contains("main"), "base never listed");
    assert!(!names.contains("dev"), "current branch never listed");

    // The gone branch is present, flagged goneUpstream (not merged).
    let gone = report
        .branches
        .iter()
        .find(|b| b.name == "gone")
        .expect("gone branch listed");
    assert_eq!(gone.reason, StaleReason::GoneUpstream);
    assert!(gone.gone_upstream, "gone flag set");
    assert!(!gone.merged, "gone branch is not merged");
    assert_eq!(gone.upstream.as_deref(), Some("origin/gone"));
}

// ------------------------------------------------- §9.2 (2) destructive path

/// The load-bearing destructive test. Feed `delete_branches` the full stale set
/// PLUS the unmerged branch, a bogus name, `main`, and the current branch:
/// - stale (merged + gone) → `deleted`, gone from `git branch --list`;
/// - the UNMERGED branch → skipped and STILL PRESENT (the key safety property);
/// - the bogus name → skipped, no error;
/// - `main` → `skippedBase`;
/// - the current branch → `skippedCurrent` and still present.
#[test]
fn delete_branches_destructive_path_is_safe() {
    require_git!();
    let dir = build_fixture();
    let path = dir.path();

    let current = git(path, &["branch", "--show-current"]);
    assert_eq!(current, "dev", "fixture invariant: current branch is dev");

    let before = local_branches(path);
    for b in ["main", "dev", "merged-a", "merged-b", "unmerged", "gone"] {
        assert!(before.contains(b), "fixture must contain {b}");
    }

    let names: Vec<String> = [
        "merged-a", "merged-b", "gone", // stale → deleted
        "unmerged",                     // not stale → skipped, survives
        BOGUS,                          // not a branch → skipped, no error
        "main",                         // base → skippedBase
        &current,                       // current → skippedCurrent, survives
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();

    let results = delete_branches(path, &names, Some("main")).expect("delete_branches Ok");

    let status_of = |n: &str| -> BranchDeleteStatus {
        results
            .iter()
            .find(|r| r.name == n)
            .map(|r| r.status)
            .unwrap_or_else(|| panic!("no result row for {n}"))
    };

    // Stale branches deleted.
    assert_eq!(status_of("merged-a"), BranchDeleteStatus::Deleted);
    assert_eq!(status_of("merged-b"), BranchDeleteStatus::Deleted);
    assert_eq!(status_of("gone"), BranchDeleteStatus::Deleted);

    // KEY SAFETY: the unmerged branch is refused (not in the recomputed safe set).
    assert_eq!(status_of("unmerged"), BranchDeleteStatus::SkippedNotStale);

    // A name that is not a branch is likewise refused as not-stale (the safe-set
    // membership check precedes the find_branch/not-found check, per contract §4.3).
    assert_eq!(status_of(BOGUS), BranchDeleteStatus::SkippedNotStale);

    // The base and the current branch are refused with their dedicated statuses.
    assert_eq!(status_of("main"), BranchDeleteStatus::SkippedBase);
    assert_eq!(status_of(&current), BranchDeleteStatus::SkippedCurrent);

    // A partial batch never errors: no row is Failed.
    assert!(
        results.iter().all(|r| r.status != BranchDeleteStatus::Failed),
        "no row should be Failed: {results:?}"
    );

    // Cross-check the surviving branch set against the git CLI.
    let after = local_branches(path);
    assert_eq!(
        after,
        BTreeSet::from(["main".to_string(), "dev".to_string(), "unmerged".to_string()]),
        "only main, dev (current), and unmerged survive"
    );
    // Belt-and-suspenders: the unmerged and current branches definitely survive.
    assert!(
        git(path, &["rev-parse", "--verify", "refs/heads/unmerged"]).len() == 40,
        "unmerged branch ref survives the destructive batch"
    );
    assert!(
        git(path, &["rev-parse", "--verify", "refs/heads/dev"]).len() == 40,
        "current branch ref survives"
    );
    // The stale branches are truly gone.
    for gone in ["merged-a", "merged-b", "gone"] {
        assert!(
            !common::git_ok(path, &["rev-parse", "--verify", &format!("refs/heads/{gone}")]),
            "{gone} must be deleted"
        );
    }
}

// ------------------------------------------------------- §9.2 (3) idempotent

/// Re-running `delete_branches` with the already-deleted names is idempotent:
/// the call returns Ok, deletes nothing further, and every row is skipped (never
/// `Deleted`/`Failed`). Per contract §4.3 the status is `SkippedNotStale`
/// (deleted branches are absent from the freshly-recomputed safe set, and that
/// check precedes the find_branch/not-found check).
#[test]
fn delete_branches_rerun_is_idempotent() {
    require_git!();
    let dir = build_fixture();
    let path = dir.path();

    let first: Vec<String> = ["merged-a", "merged-b", "gone"]
        .iter()
        .map(|s| s.to_string())
        .collect();
    let r1 = delete_branches(path, &first, Some("main")).expect("first delete Ok");
    assert!(
        r1.iter().all(|r| r.status == BranchDeleteStatus::Deleted),
        "first pass deletes all three: {r1:?}"
    );
    let after_first = local_branches(path);

    // Re-run with the same (now-gone) names.
    let r2 = delete_branches(path, &first, Some("main")).expect("second delete Ok (no error)");
    assert!(
        r2.iter().all(|r| {
            matches!(
                r.status,
                BranchDeleteStatus::SkippedNotStale
                    | BranchDeleteStatus::SkippedNotFound
                    | BranchDeleteStatus::SkippedCurrent
                    | BranchDeleteStatus::SkippedBase
            )
        }),
        "second pass skips every name, deletes nothing: {r2:?}"
    );
    assert!(
        r2.iter()
            .all(|r| r.status != BranchDeleteStatus::Deleted && r.status != BranchDeleteStatus::Failed),
        "idempotent re-run performs no deletion and never errors: {r2:?}"
    );
    // Concretely, per §4.3 ordering, absent branches are SkippedNotStale.
    assert!(
        r2.iter().all(|r| r.status == BranchDeleteStatus::SkippedNotStale),
        "absent branches classify as SkippedNotStale (safe-set check precedes not-found): {r2:?}"
    );

    // Branch set unchanged by the second pass.
    assert_eq!(
        local_branches(path),
        after_first,
        "the idempotent re-run changes no refs"
    );
}
