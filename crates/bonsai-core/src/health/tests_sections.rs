//! Branches / working-state / structure / section-isolation / perf tests for
//! `health`. Extracted verbatim from the former inline `mod tests`; shared
//! fixtures live in `test_support`.

use super::test_support::*;
use super::*;
use std::process::Command;

#[test]
fn branches_counts_and_stale_rollup() {
    let dir = crate::testutil::scratch_dir();
    let d = dir.path();
    let repo = init(d);
    let c0 = commit(d, "C0", &[("a.txt", "a\n")]);
    let c1 = commit(d, "C1", &[("b.txt", "b\n")]);
    let c2 = commit(d, "C2", &[("c.txt", "c\n")]); // main tip

    branch_at(&repo, "merged-1", c0); // merged into main
    branch_at(&repo, "merged-2", c1); // merged into main
    repo.remote("origin", "https://example.invalid/x.git").expect("remote");
    // gone-upstream branch with a unique commit.
    branch_at(&repo, "gone", c0);
    {
        let sig = git2::Signature::now("Test User", "test@example.com").expect("sig");
        let parent = repo.find_commit(c0).expect("c0");
        let tree = parent.tree().expect("tree");
        repo.commit(Some("refs/heads/gone"), &sig, &sig, "gone work\n", &tree, &[&parent])
            .expect("commit on gone");
        let mut cfg = repo.config().expect("config");
        cfg.set_str("branch.gone.remote", "origin").expect("cfg");
        cfg.set_str("branch.gone.merge", "refs/heads/gone").expect("cfg");
    }
    // Upstream for main: remote-tracking ref at C1 → ahead 1, behind 0.
    repo.reference("refs/remotes/origin/main", c1, true, "seed").expect("ref");
    {
        let mut cfg = repo.config().expect("config");
        cfg.set_str("branch.main.remote", "origin").expect("cfg");
        cfg.set_str("branch.main.merge", "refs/heads/main").expect("cfg");
    }
    repo.reference("refs/tags/v1", c2, true, "tag").expect("tag");

    let b = collect_branches(d).expect("branches");
    assert_eq!(b.local_count, 4, "main + merged-1 + merged-2 + gone");
    assert_eq!(b.remote_count, 1);
    assert_eq!(b.tag_count, 1);
    assert_eq!(b.current_branch.as_deref(), Some("main"));
    assert!(!b.detached);
    assert!(!b.unborn);
    assert_eq!(b.upstream.as_deref(), Some("origin/main"));
    assert_eq!(b.ahead, Some(1));
    assert_eq!(b.behind, Some(0));

    // Stale rollup mirrors find_stale_branches exactly.
    let report = find_stale_branches(d, None).expect("stale");
    let stale = b.stale.expect("stale rollup present");
    assert_eq!(stale.base, report.base);
    assert_eq!(
        stale.merged_count as usize,
        report.branches.iter().filter(|x| x.merged).count()
    );
    assert_eq!(
        stale.gone_upstream_count as usize,
        report.branches.iter().filter(|x| x.gone_upstream).count()
    );
    assert_eq!(stale.merged_count, 2);
    assert_eq!(stale.gone_upstream_count, 1);
    assert!(b.stale_error.is_none());

    // CLI oracle for ref counts (skip when git absent).
    if have_git() {
        let count_refs = |prefix: &str| -> usize {
            let out = Command::new("git")
                .args(["for-each-ref", "--format=%(refname)", prefix])
                .current_dir(d)
                .output()
                .expect("git for-each-ref");
            assert!(out.status.success());
            String::from_utf8_lossy(&out.stdout)
                .lines()
                .filter(|l| !l.trim().is_empty())
                .count()
        };
        assert_eq!(b.local_count as usize, count_refs("refs/heads"));
        assert_eq!(b.remote_count as usize, count_refs("refs/remotes"));
        assert_eq!(b.tag_count as usize, count_refs("refs/tags"));
    }
}

/// Detached HEAD → detached=true, currentBranch=None, section still Ok.
#[test]
fn branches_detached_head() {
    let dir = crate::testutil::scratch_dir();
    let d = dir.path();
    let repo = init(d);
    let c0 = commit(d, "C0", &[("a.txt", "a\n")]);
    commit(d, "C1", &[("b.txt", "b\n")]);
    repo.set_head_detached(c0).expect("detach");

    let b = collect_branches(d).expect("branches");
    assert!(b.detached);
    assert_eq!(b.current_branch, None);
    assert_eq!(b.ahead, None);
    assert_eq!(b.upstream, None);
}

/// Unborn repo: the section succeeds; the stale SUB-metric fails into
/// stale_error (D9) without failing the section.
#[test]
fn branches_unborn_repo_ok() {
    let dir = crate::testutil::scratch_dir();
    init(dir.path());
    let b = collect_branches(dir.path()).expect("branches section Ok on unborn");
    assert!(b.unborn);
    assert_eq!(b.local_count, 0);
    assert!(b.stale.is_none());
    assert!(b.stale_error.is_some(), "stale base unresolvable → sub-error");
}

// ------------------------------------------------------- working state

/// Counts match read_status; stash (created via git2 in the FIXTURE only)
/// counts 1; .gitignore flag flips with the file.
#[test]
fn working_state_counts_stash_gitignore() {
    let dir = crate::testutil::scratch_dir();
    let d = dir.path();
    init(d);
    commit(d, "C0", &[("a.txt", "a\n"), ("tracked.txt", "t\n")]);

    // stash: modify a tracked file, then stash it away.
    std::fs::write(d.join("tracked.txt"), "changed\n").expect("modify");
    {
        let mut repo = git2::Repository::open(d).expect("open");
        let sig = git2::Signature::now("Test User", "test@example.com").expect("sig");
        repo.stash_save(&sig, "wip", None).expect("stash save");
    }

    // staged: new file staged; unstaged: modify a.txt; untracked: new file.
    std::fs::write(d.join("staged.txt"), "s\n").expect("write");
    crate::git::stage::stage_paths(d, &["staged.txt".to_string()]).expect("stage");
    std::fs::write(d.join("a.txt"), "a2\n").expect("modify a");
    std::fs::write(d.join("untracked.txt"), "u\n").expect("write untracked");

    let ws = collect_working_state(d).expect("working state");
    let snap = read_status(d).expect("status");
    assert_eq!(ws.staged as usize, snap.staged.len());
    assert_eq!(ws.unstaged as usize, snap.unstaged.len());
    assert_eq!(ws.untracked as usize, snap.untracked.len());
    assert_eq!(ws.conflicted as usize, snap.conflicted.len());
    assert_eq!(ws.staged, 1);
    assert_eq!(ws.unstaged, 1);
    assert_eq!(ws.untracked, 1);
    assert_eq!(ws.conflicted, 0);
    assert_eq!(ws.stash_count, 1);
    assert_eq!(ws.op_state, RepoOpState::None);
    assert!(!ws.has_gitignore);

    std::fs::write(d.join(".gitignore"), "*.log\n").expect("write gitignore");
    let ws2 = collect_working_state(d).expect("working state 2");
    assert!(ws2.has_gitignore);
}

// ------------------------------------------------------- structure

/// Locked + prunable worktrees roll up to matching counts; a drifted
/// CLAUDE.md/AGENTS.md pair yields assetDriftedCount ≥ 1 (in_sync false).
#[test]
fn structure_worktrees_and_drift() {
    let dir = crate::testutil::scratch_dir();
    let d = dir.path();
    let repo = init(d);
    commit(d, "C0", &[("a.txt", "a\n")]);

    // Linked worktrees live as siblings in their own scratch dir.
    let wt_root = crate::testutil::scratch_dir();
    let locked_path = wt_root.path().join("wt-locked");
    let prunable_path = wt_root.path().join("wt-prunable");
    let wt1 = repo
        .worktree("wt-locked", &locked_path, None)
        .expect("add worktree 1");
    wt1.lock(Some("testing")).expect("lock");
    repo.worktree("wt-prunable", &prunable_path, None)
        .expect("add worktree 2");
    // Make the second prunable: remove its working directory.
    std::fs::remove_dir_all(&prunable_path).expect("remove wt dir");

    // Drifted AI-asset pair (different normalized content).
    std::fs::write(d.join("CLAUDE.md"), "# Rules\nAlpha\n").expect("claude");
    std::fs::write(d.join("AGENTS.md"), "# Rules\nBeta\n").expect("agents");

    let s = collect_structure(d).expect("structure");
    let wts = list_worktrees(d).expect("worktrees");
    assert_eq!(s.worktree_count as usize, wts.len());
    assert_eq!(s.worktree_count, 3, "main + 2 linked");
    assert_eq!(s.worktrees_locked, 1);
    assert_eq!(
        s.worktrees_prunable as usize,
        wts.iter().filter(|w| w.prunable).count()
    );
    assert!(s.worktrees_prunable >= 1, "deleted workdir → prunable");
    assert_eq!(s.submodule_count, 0);
    assert!(s.asset_drifted_count >= 1, "CLAUDE.md vs AGENTS.md drifted");
    assert!(!s.assets_in_sync);
}

// ------------------------------------------------------- section isolation

/// One failing collector → its Section carries the error while the other
/// three carry data (D4 fold, exercised at the envelope level), and
/// `collect_repo_health` on a healthy repo yields all four with data.
#[test]
fn section_isolation() {
    let dir = crate::testutil::scratch_dir();
    let d = dir.path();
    init(d);
    commit(d, "C0", &[("a.txt", "a\n")]);

    // The fold: a failing collector never panics/propagates.
    let failing: Section<StatsSection> =
        run_section(|| Err(AppError::Git("simulated odb corruption".to_string())));
    assert!(failing.data.is_none());
    assert_eq!(
        failing.error.as_deref(),
        Some("git error: simulated odb corruption")
    );

    // Sibling sections on the same repo still produce data.
    let health = collect_repo_health(d);
    assert!(health.stats.data.is_some(), "{:?}", health.stats.error);
    assert!(health.branches.data.is_some(), "{:?}", health.branches.error);
    assert!(
        health.working_state.data.is_some(),
        "{:?}",
        health.working_state.error
    );
    assert!(health.structure.data.is_some(), "{:?}", health.structure.error);
    assert!(health.generated_at > 0);

    // Whole-fn never errs even on a non-repo dir: every section reports
    // its own error instead.
    let empty = crate::testutil::scratch_dir();
    let health = collect_repo_health(empty.path());
    assert!(health.stats.data.is_none() && health.stats.error.is_some());
    assert!(health.branches.data.is_none() && health.branches.error.is_some());
    assert!(health.working_state.error.is_some());
    assert!(health.structure.error.is_some());
}

/// MIXED state with a REAL failing collector (P29a review carry-forward):
/// deleting a parent commit's loose object makes the stats revwalk error
/// mid-iteration, while branches / workingState / structure (which only
/// need refs, the HEAD commit + its tree, and fs facts) still succeed.
///
/// Note the companion carry-forward (health.rs `find_commit` degrade):
/// a missing object aborts the revwalk ITERATOR itself (`oid?`), before
/// `find_commit` runs, so the `if let Ok` path cannot be reached with a
/// plain missing-object fixture — the degrade is covered by review, this
/// test pins the section-isolation behavior around the same failure.
#[test]
fn mixed_state_real_collector_failure() {
    let dir = crate::testutil::scratch_dir();
    let d = dir.path();
    init(d);
    let c0 = commit(d, "C0", &[("a.txt", "a\n")]);
    commit(d, "C1", &[("b.txt", "b\n")]);

    // Remove C0's loose commit object; clear read-only first (Windows
    // loose objects are written read-only).
    let hex = c0.to_string();
    let obj = d.join(".git/objects").join(&hex[..2]).join(&hex[2..]);
    let mut perms = std::fs::metadata(&obj).expect("object exists").permissions();
    #[allow(clippy::permissions_set_readonly_false)]
    perms.set_readonly(false);
    std::fs::set_permissions(&obj, perms).expect("clear readonly");
    std::fs::remove_file(&obj).expect("delete loose object");

    let health = collect_repo_health(d);
    assert!(
        health.stats.data.is_none() && health.stats.error.is_some(),
        "stats must fail on the missing parent object (error: {:?})",
        health.stats.error
    );
    assert!(health.branches.data.is_some(), "{:?}", health.branches.error);
    assert!(
        health.working_state.data.is_some(),
        "{:?}",
        health.working_state.error
    );
    assert!(health.structure.data.is_some(), "{:?}", health.structure.error);
}

// ------------------------------------------------------- perf ceiling (§5)

/// On the shared 20k+ fixture the whole scan stays < 2 s and the stats
/// section < 1.5 s. Coarse ceiling, not a benchmark: warm-up + best of 3.
///
/// `#[ignore]`d to match the established perf-gate convention
/// (`perf_gate.rs::layout_31k_under_500ms` / `serialize_31k_report`): it
/// depends on the multi-second 31k fixture and is release-oriented, and in
/// the *parallel* default `cargo test` suite CPU contention from the other
/// ~430 tests inflates the best-of-3 (~3.1 s observed) even though the scan
/// itself is well under budget in isolation. Run it EXPLICITLY (isolated),
/// like the other perf gates, for perf tracking:
///
/// ```text
/// cargo test --release -p bonsai-core --lib \
///     health::tests::perf_ceiling_on_20k_fixture -- --ignored --nocapture
/// ```
///
/// P52 (commit-graph) result — measured isolated, best-of-3, on the fixture
/// carrying `.git/objects/info/commit-graph`: stats ~300 ms (was ~1558),
/// branches ~1280 ms (was ~6000), total ~1600 ms (was ~8100) — under the
/// UNCHANGED 1500 / 2000 ms budgets. The residual cost is the branches
/// merge-base scan; the graph already cut it ~4.7x and further cuts would
/// need app-logic changes (out of P52 scope).
#[test]
#[ignore] // perf gate: run explicitly + isolated; see doc comment
fn perf_ceiling_on_20k_fixture() {
    let repo_path = crate::fixture::ensure_default_fixture().expect("fixture");

    // P52: the fixture carries a commit-graph (written once by
    // ensure_default_fixture when git is available), so this gate measures
    // the realistic opened-repo state — libgit2 consumes the graph
    // unconditionally, cutting the branches merge-base + stats revwalk cost.
    if have_git() {
        assert!(
            repo_path.join(".git/objects/info/commit-graph").exists(),
            "P52: fixture must carry a commit-graph for the perf measurement"
        );
    }

    // Warm-up (page cache, odb) + correctness assertions.
    let warm = collect_repo_health(&repo_path);
    let stats = warm.stats.data.as_ref().unwrap_or_else(|| {
        panic!("stats section failed: {:?}", warm.stats.error)
    });
    assert!(
        stats.commit_count >= 20_000,
        "fixture has 20k+ commits, got {}",
        stats.commit_count
    );

    let mut best_total = u128::MAX;
    let mut best_stats = u32::MAX;
    let mut best_branches = u32::MAX;
    let mut best_working = u32::MAX;
    let mut best_structure = u32::MAX;
    for _ in 0..3 {
        let start = std::time::Instant::now();
        let health = collect_repo_health(&repo_path);
        let total = start.elapsed().as_millis();
        eprintln!(
            "[health perf] stats={}ms branches={}ms workingState={}ms structure={}ms total={}ms",
            health.stats.elapsed_ms,
            health.branches.elapsed_ms,
            health.working_state.elapsed_ms,
            health.structure.elapsed_ms,
            total
        );
        best_total = best_total.min(total);
        best_stats = best_stats.min(health.stats.elapsed_ms);
        best_branches = best_branches.min(health.branches.elapsed_ms);
        best_working = best_working.min(health.working_state.elapsed_ms);
        best_structure = best_structure.min(health.structure.elapsed_ms);
    }
    // Best-of-3 per-section summary so the commit-graph effect (mainly on
    // the branches merge-base scan) is visible under `--nocapture`.
    eprintln!(
        "[health perf] best-of-3: stats={best_stats}ms branches={best_branches}ms \
         workingState={best_working}ms structure={best_structure}ms total={best_total}ms"
    );
    assert!(
        best_stats < 1_500,
        "stats section best-of-3 took {best_stats} ms (budget 1500)"
    );
    assert!(
        best_total < 2_000,
        "collect_repo_health best-of-3 took {best_total} ms (budget 2000)"
    );
}
