//! P29 tester-pass integration suite for the repo-health dashboard
//! (contract `docs/contracts/P29-repo-health.md` §10.1).
//!
//! The unit tests in `health.rs` cover collectors in isolation with
//! git2-built fixtures; THIS file cross-checks `collect_repo_health`
//! end-to-end against the real `git` CLI as an oracle on scratch repos
//! built entirely with the `git` binary, plus edge repos (unborn HEAD,
//! detached HEAD, `.git`-only workdir) and the READ-ONLY hard invariant
//! (§6): running the collector must leave status/refs/stashes byte-identical.
//!
//! All scratch repos live under `D:\Data\Temp\bonsai-scratch` (C: is full). Each
//! test skips (passes with a note) when `git` is not on PATH.

mod common;

use std::path::Path;

use bonsai_core::health::collect_repo_health;
use common::{commit_fixed, git, git_ok, git_raw, init_repo, porcelain_records, scratch_dir};

macro_rules! require_git {
    () => {
        if !common::have_git() {
            eprintln!("skipping: `git` CLI not found on PATH");
            return;
        }
    };
}

/// Full repo observable state for the read-only check: porcelain status,
/// all refs (incl. HEAD), stash list, and the index contents via ls-files.
#[derive(Debug, PartialEq)]
struct ObservableState {
    status: Vec<(String, Option<String>)>,
    refs: String,
    stash: String,
    index: Vec<u8>,
}

fn observable_state(dir: &Path) -> ObservableState {
    let status = porcelain_records(dir);
    let refs = git(dir, &["for-each-ref", "--format=%(refname) %(objectname)"]);
    let head = git(dir, &["rev-parse", "--symbolic-full-name", "HEAD"]);
    // stash list may be empty; ls-files -s pins the index contents.
    let stash = if git_ok(dir, &["rev-parse", "-q", "--verify", "refs/stash"]) {
        git(dir, &["stash", "list"])
    } else {
        String::new()
    };
    let index = git_raw(dir, &["ls-files", "-s", "-z"], &[]);
    ObservableState {
        status,
        refs: format!("{refs}\nHEAD={head}"),
        stash,
        index,
    }
}

// ------------------------------------------------------------ CLI oracles

/// stats section vs `git rev-list --count HEAD` and `git count-objects -v`
/// (loose `count` + `in-pack`), on a CLI-built repo with a pack (via gc).
#[test]
fn stats_match_rev_list_and_count_objects() {
    require_git!();
    let dir = init_repo();
    let d = dir.path();

    for i in 0..6 {
        std::fs::write(d.join(format!("f{i}.txt")), format!("content {i}\n")).expect("write");
        git(d, &["add", "-A"]);
        commit_fixed(d, &format!("C{i}"));
    }
    // Pack part of the history so odb.foreach must see packed AND loose objects.
    git(d, &["gc", "--quiet"]);
    std::fs::write(d.join("loose.txt"), "loose\n").expect("write");
    git(d, &["add", "-A"]);
    commit_fixed(d, "C-loose");

    let health = collect_repo_health(d);
    let stats = health
        .stats
        .data
        .unwrap_or_else(|| panic!("stats errored: {:?}", health.stats.error));

    // Oracle 1: commit count.
    let cli_commits: u32 = git(d, &["rev-list", "--count", "HEAD"]).parse().expect("count");
    assert_eq!(stats.commit_count, cli_commits, "vs git rev-list --count HEAD");
    assert!(!stats.commit_count_capped);

    // Oracle 2: object count = loose `count` + `in-pack` from count-objects -v.
    let co = git(d, &["count-objects", "-v"]);
    let field = |name: &str| -> u64 {
        co.lines()
            .find_map(|l| l.strip_prefix(&format!("{name}: ")))
            .unwrap_or("0")
            .trim()
            .parse()
            .expect("count-objects field")
    };
    let cli_objects = field("count") + field("in-pack");
    assert_eq!(stats.object_count, cli_objects, "vs git count-objects -v\n{co}");
    assert!(!stats.object_scan_capped);

    // Largest blobs are 40-hex oids, sizes descending, ≤ 10 rows.
    assert!(!stats.largest_blobs.is_empty());
    assert!(stats.largest_blobs.len() <= 10);
    for w in stats.largest_blobs.windows(2) {
        assert!(w[0].size >= w[1].size, "largestBlobs sorted desc");
    }
    for b in &stats.largest_blobs {
        assert_eq!(b.oid.len(), 40);
        assert!(b.oid.chars().all(|c| c.is_ascii_hexdigit()));
    }
}

/// workingState vs `git status --porcelain` bucket counts + `git stash list`.
#[test]
fn working_state_matches_porcelain_and_stash_list() {
    require_git!();
    let dir = init_repo();
    let d = dir.path();

    std::fs::write(d.join("a.txt"), "a\n").expect("write");
    std::fs::write(d.join("b.txt"), "b\n").expect("write");
    git(d, &["add", "-A"]);
    commit_fixed(d, "C0");

    // One stash from a tracked-file modification.
    std::fs::write(d.join("a.txt"), "stash me\n").expect("write");
    git(d, &["stash", "push", "-m", "wip"]);

    // staged / unstaged / untracked.
    std::fs::write(d.join("staged.txt"), "s\n").expect("write");
    git(d, &["add", "staged.txt"]);
    std::fs::write(d.join("b.txt"), "changed\n").expect("write");
    std::fs::write(d.join("untracked.txt"), "u\n").expect("write");

    let health = collect_repo_health(d);
    let ws = health
        .working_state
        .data
        .unwrap_or_else(|| panic!("workingState errored: {:?}", health.working_state.error));

    // Oracle: bucket the porcelain records ourselves.
    let records = porcelain_records(d);
    let mut staged = 0u32;
    let mut unstaged = 0u32;
    let mut untracked = 0u32;
    let mut conflicted = 0u32;
    for (rec, _) in &records {
        let mut cs = rec.chars();
        let x = cs.next().unwrap();
        let y = cs.next().unwrap();
        if x == '?' {
            untracked += 1;
            continue;
        }
        if x == 'U' || y == 'U' || (x == 'A' && y == 'A') || (x == 'D' && y == 'D') {
            conflicted += 1;
            continue;
        }
        if x != ' ' {
            staged += 1;
        }
        if y != ' ' {
            unstaged += 1;
        }
    }
    assert_eq!(ws.staged, staged, "staged vs porcelain X column");
    assert_eq!(ws.unstaged, unstaged, "unstaged vs porcelain Y column");
    assert_eq!(ws.untracked, untracked, "untracked vs porcelain ??");
    assert_eq!(ws.conflicted, conflicted);

    let stash_lines = git(d, &["stash", "list"])
        .lines()
        .filter(|l| !l.trim().is_empty())
        .count() as u32;
    assert_eq!(ws.stash_count, stash_lines, "vs git stash list");
    assert_eq!(ws.stash_count, 1);
    assert!(!ws.has_gitignore);
}

/// branches section vs `git for-each-ref` on a repo with locals, a
/// remote-tracking ref, tags, and a configured upstream (ahead/behind oracle
/// via `git rev-list --left-right --count`).
#[test]
fn branches_match_for_each_ref_and_ahead_behind() {
    require_git!();
    let dir = init_repo();
    let d = dir.path();

    std::fs::write(d.join("a.txt"), "a\n").expect("write");
    git(d, &["add", "-A"]);
    commit_fixed(d, "C0");
    let c0 = git(d, &["rev-parse", "HEAD"]);
    std::fs::write(d.join("b.txt"), "b\n").expect("write");
    git(d, &["add", "-A"]);
    commit_fixed(d, "C1");

    git(d, &["branch", "feature-x", &c0]);
    git(d, &["branch", "feature-y"]);
    git(d, &["tag", "v1", &c0]);
    git(d, &["tag", "v2"]);
    git(d, &["remote", "add", "origin", "https://example.invalid/x.git"]);
    // Seed origin/main at C0 → main is ahead 1 behind 0; wire the upstream.
    git(d, &["update-ref", "refs/remotes/origin/main", &c0]);
    git(d, &["branch", "--set-upstream-to=origin/main", "main"]);

    let health = collect_repo_health(d);
    let b = health
        .branches
        .data
        .unwrap_or_else(|| panic!("branches errored: {:?}", health.branches.error));

    let count_refs = |prefix: &str| -> u32 {
        git(d, &["for-each-ref", "--format=%(refname)", prefix])
            .lines()
            .filter(|l| !l.trim().is_empty())
            .count() as u32
    };
    assert_eq!(b.local_count, count_refs("refs/heads"));
    assert_eq!(b.remote_count, count_refs("refs/remotes"));
    assert_eq!(b.tag_count, count_refs("refs/tags"));
    assert_eq!(b.local_count, 3);
    assert_eq!(b.tag_count, 2);
    assert_eq!(b.current_branch.as_deref(), Some("main"));
    assert_eq!(b.upstream.as_deref(), Some("origin/main"));

    // Ahead/behind oracle.
    let lr = git(
        d,
        &["rev-list", "--left-right", "--count", "main...origin/main"],
    );
    let mut it = lr.split_whitespace();
    let cli_ahead: u32 = it.next().unwrap().parse().unwrap();
    let cli_behind: u32 = it.next().unwrap().parse().unwrap();
    assert_eq!(b.ahead, Some(cli_ahead));
    assert_eq!(b.behind, Some(cli_behind));
    assert_eq!((cli_ahead, cli_behind), (1, 0));
}

/// structure section vs `git worktree list --porcelain` (locked + prunable)
/// plus a REAL drifted CLAUDE.md/AGENTS.md pair on disk.
#[test]
fn structure_matches_worktree_list_porcelain() {
    require_git!();
    let dir = init_repo();
    let d = dir.path();
    std::fs::write(d.join("a.txt"), "a\n").expect("write");
    git(d, &["add", "-A"]);
    commit_fixed(d, "C0");

    let wt_root = scratch_dir();
    let locked = wt_root.path().join("wt-locked");
    let prunable = wt_root.path().join("wt-prunable");
    git(d, &["worktree", "add", "--detach", locked.to_str().unwrap()]);
    git(d, &["worktree", "lock", locked.to_str().unwrap()]);
    git(d, &["worktree", "add", "--detach", prunable.to_str().unwrap()]);
    std::fs::remove_dir_all(&prunable).expect("delete worktree dir → prunable");

    // Drifted asset pair (differing normalized content).
    std::fs::write(d.join("CLAUDE.md"), "# Rules\nAlpha\n").expect("claude");
    std::fs::write(d.join("AGENTS.md"), "# Rules\nBeta\n").expect("agents");

    let health = collect_repo_health(d);
    let s = health
        .structure
        .data
        .unwrap_or_else(|| panic!("structure errored: {:?}", health.structure.error));

    // Oracle: parse `git worktree list --porcelain`.
    let porcelain = git(d, &["worktree", "list", "--porcelain"]);
    let cli_total = porcelain.lines().filter(|l| l.starts_with("worktree ")).count() as u32;
    let cli_locked = porcelain.lines().filter(|l| l.starts_with("locked")).count() as u32;
    let cli_prunable = porcelain.lines().filter(|l| l.starts_with("prunable")).count() as u32;
    assert_eq!(s.worktree_count, cli_total, "vs worktree list --porcelain\n{porcelain}");
    assert_eq!(s.worktree_count, 3, "main + 2 linked");
    assert_eq!(s.worktrees_locked, cli_locked);
    assert_eq!(s.worktrees_locked, 1);
    assert_eq!(s.worktrees_prunable, cli_prunable);
    assert!(s.worktrees_prunable >= 1);

    assert_eq!(s.submodule_count, 0);
    assert!(s.asset_drifted_count >= 1, "CLAUDE.md/AGENTS.md drifted");
    assert!(!s.assets_in_sync);
}

// ------------------------------------------------------------ edge repos

/// Unborn-HEAD repo (git init, zero commits): no panic; stats/branches/
/// workingState degrade per contract (zero counts, unborn=true).
#[test]
fn edge_unborn_head_repo_degrades() {
    require_git!();
    let dir = init_repo();
    let d = dir.path();

    let health = collect_repo_health(d);
    let stats = health.stats.data.expect("stats Ok on unborn");
    assert_eq!(stats.commit_count, 0);
    let b = health.branches.data.expect("branches Ok on unborn");
    assert!(b.unborn);
    assert_eq!(b.current_branch.as_deref(), Some("main"), "symbolic target name");
    assert_eq!(b.local_count, 0);
    assert!(b.stale.is_none(), "stale sub-metric unavailable on unborn");
    let ws = health.working_state.data.expect("workingState Ok on unborn");
    assert_eq!(ws.staged + ws.unstaged + ws.conflicted, 0);
    assert!(health.structure.data.is_some());
}

/// Detached HEAD via the CLI: detached=true, currentBranch=None, no upstream,
/// all four sections still produce data.
#[test]
fn edge_detached_head_via_cli() {
    require_git!();
    let dir = init_repo();
    let d = dir.path();
    std::fs::write(d.join("a.txt"), "a\n").expect("write");
    git(d, &["add", "-A"]);
    commit_fixed(d, "C0");
    let c0 = git(d, &["rev-parse", "HEAD"]);
    std::fs::write(d.join("b.txt"), "b\n").expect("write");
    git(d, &["add", "-A"]);
    commit_fixed(d, "C1");
    git(d, &["checkout", "--detach", &c0]);

    let health = collect_repo_health(d);
    let b = health.branches.data.expect("branches Ok when detached");
    assert!(b.detached);
    assert!(!b.unborn);
    assert_eq!(b.current_branch, None);
    assert_eq!(b.upstream, None);
    assert!(health.stats.data.is_some());
    assert!(health.working_state.data.is_some());
    assert!(health.structure.data.is_some());
}

/// A directory containing ONLY a `.git` dir (bare-adjacent: workdir wiped).
/// Must never panic; sections report data or a clean error, whole-fn returns.
#[test]
fn edge_git_dir_only_never_panics() {
    require_git!();
    let dir = init_repo();
    let d = dir.path();
    std::fs::write(d.join("a.txt"), "a\n").expect("write");
    git(d, &["add", "-A"]);
    commit_fixed(d, "C0");
    // Wipe everything except .git.
    for entry in std::fs::read_dir(d).expect("read_dir").flatten() {
        if entry.file_name() != ".git" {
            let p = entry.path();
            if p.is_dir() {
                std::fs::remove_dir_all(&p).expect("rm dir");
            } else {
                std::fs::remove_file(&p).expect("rm file");
            }
        }
    }

    let health = collect_repo_health(d); // must not panic
    // Every section must resolve to exactly one of data/error.
    assert!(health.stats.data.is_some() ^ health.stats.error.is_some());
    assert!(health.branches.data.is_some() ^ health.branches.error.is_some());
    assert!(health.working_state.data.is_some() ^ health.working_state.error.is_some());
    assert!(health.structure.data.is_some() ^ health.structure.error.is_some());
    if let Some(stats) = &health.stats.data {
        assert_eq!(stats.workdir_file_count, 0, "workdir is empty besides .git");
    }
}

// ------------------------------------------------------------ read-only invariant

/// §6 hard invariant: collect_repo_health performs ZERO writes. Observable
/// git state (porcelain status, all refs + HEAD, stash list, index contents)
/// is byte-identical before and after, on a dirty repo with stash, upstream,
/// worktrees, and a drifted asset pair — i.e. every collector exercised.
#[test]
fn collect_repo_health_is_read_only() {
    require_git!();
    let dir = init_repo();
    let d = dir.path();

    std::fs::write(d.join("a.txt"), "a\n").expect("write");
    std::fs::write(d.join(".gitignore"), "*.log\n").expect("write");
    std::fs::write(d.join("CLAUDE.md"), "# R\nA\n").expect("write");
    std::fs::write(d.join("AGENTS.md"), "# R\nB\n").expect("write");
    git(d, &["add", "-A"]);
    commit_fixed(d, "C0");
    let c0 = git(d, &["rev-parse", "HEAD"]);
    std::fs::write(d.join("b.txt"), "b\n").expect("write");
    git(d, &["add", "-A"]);
    commit_fixed(d, "C1");
    git(d, &["branch", "merged", &c0]);
    git(d, &["tag", "v1"]);
    git(d, &["remote", "add", "origin", "https://example.invalid/x.git"]);
    git(d, &["update-ref", "refs/remotes/origin/main", &c0]);
    git(d, &["branch", "--set-upstream-to=origin/main", "main"]);
    // Stash + dirty working tree + staged + untracked.
    std::fs::write(d.join("a.txt"), "stash\n").expect("write");
    git(d, &["stash", "push", "-m", "wip"]);
    std::fs::write(d.join("staged.txt"), "s\n").expect("write");
    git(d, &["add", "staged.txt"]);
    std::fs::write(d.join("a.txt"), "dirty\n").expect("write");
    std::fs::write(d.join("untracked.txt"), "u\n").expect("write");
    // A linked worktree.
    let wt_root = scratch_dir();
    let wt = wt_root.path().join("wt-ro");
    git(d, &["worktree", "add", "--detach", wt.to_str().unwrap()]);

    let before = observable_state(d);
    let health = collect_repo_health(d);
    let after = observable_state(d);

    assert_eq!(before.status, after.status, "porcelain status changed — WRITE detected");
    assert_eq!(before.refs, after.refs, "refs/HEAD changed — WRITE detected");
    assert_eq!(before.stash, after.stash, "stash list changed — WRITE detected");
    assert_eq!(before.index, after.index, "index contents changed — WRITE detected");

    // Sanity: all sections actually ran with data on this rich fixture.
    assert!(health.stats.data.is_some(), "{:?}", health.stats.error);
    assert!(health.branches.data.is_some(), "{:?}", health.branches.error);
    assert!(health.working_state.data.is_some(), "{:?}", health.working_state.error);
    assert!(health.structure.data.is_some(), "{:?}", health.structure.error);
    assert_eq!(health.working_state.data.unwrap().stash_count, 1);
}
