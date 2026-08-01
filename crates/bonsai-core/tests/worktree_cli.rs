//! P27a CLI-oracle worktree tests (contract §7) — READ-ONLY `list_worktrees`.
//!
//! Fixtures are built with the `git` CLI (`git worktree add` / `lock`), so the
//! P27b mutating ops are NOT exercised here; those get their own create/remove/
//! lock/unlock coverage in P27b. The load-bearing oracle is that the SET of
//! worktree paths reported by `list_worktrees` equals the set parsed from
//! `git worktree list --porcelain`.
//!
//! Every test skips (passes with a note) if `git` is not on PATH. All scratch
//! repos live under `D:\Temp\bonsai-scratch` (never the system temp).

mod common;

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use bonsai_core::git::worktree::{list_worktrees, WorktreeInfo};
use common::{commit_fixed, git, git_ok};

macro_rules! require_git {
    () => {
        if !common::have_git() {
            eprintln!("skipping: `git` CLI not found on PATH");
            return;
        }
    };
}

fn configure_identity(repo: &Path) {
    git(repo, &["config", "user.name", "Test User"]);
    git(repo, &["config", "user.email", "test@example.com"]);
    git(repo, &["config", "core.autocrlf", "false"]);
}

/// A scratch repo `main` with two commits on `main` plus a second local branch
/// `feature` (so worktrees can check out distinct branches). Returned inside a
/// unique tempdir so a sibling `.worktrees/`-style container never leaks.
struct Fixture {
    _dir: tempfile::TempDir,
    root: PathBuf, // the unique tempdir; worktrees are created under it
    main: PathBuf, // the main repo workdir
}

fn setup() -> Fixture {
    let dir = common::scratch_dir();
    let root = dir.path().to_path_buf();
    let main = root.join("main");
    std::fs::create_dir_all(&main).expect("mkdir main");

    git(&main, &["init", "-b", "main"]);
    configure_identity(&main);
    std::fs::write(main.join("a.txt"), "a\n").expect("write a");
    git(&main, &["add", "-A"]);
    commit_fixed(&main, "commit A");
    std::fs::write(main.join("a.txt"), "aa\n").expect("write aa");
    git(&main, &["add", "-A"]);
    commit_fixed(&main, "commit B");
    // A second branch to check out in a worktree.
    git(&main, &["branch", "feature"]);

    Fixture {
        _dir: dir,
        root,
        main,
    }
}

/// Best-effort canonicalization for set comparison (both oracle and SUT paths
/// go through this identically).
fn canon(p: &Path) -> PathBuf {
    std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf())
}

/// Parse `git worktree list --porcelain` in `repo` → canonicalized worktree
/// paths (the leading `worktree <path>` record of each stanza).
fn cli_worktree_paths(repo: &Path) -> BTreeSet<PathBuf> {
    let out = git(repo, &["worktree", "list", "--porcelain"]);
    out.lines()
        .filter_map(|l| l.strip_prefix("worktree "))
        .map(|p| canon(Path::new(p.trim())))
        .collect()
}

fn sut_worktree_paths(rows: &[WorktreeInfo]) -> BTreeSet<PathBuf> {
    rows.iter().map(|r| canon(Path::new(&r.abs_path))).collect()
}

/// #1: a plain repo with no linked worktrees lists exactly the main row.
#[test]
fn list_plain_repo_is_only_main() {
    require_git!();
    let fx = setup();
    let rows = list_worktrees(&fx.main).expect("list");

    assert_eq!(rows.len(), 1, "expected only the main row, got {rows:?}");
    let m = &rows[0];
    assert!(m.is_main, "main row must be is_main");
    assert!(m.is_current, "the opened workdir must be is_current");
    assert_eq!(m.branch.as_deref(), Some("main"));
    assert!(m.valid);
    assert!(!m.prunable);
    assert!(!m.locked);
    assert!(m.head_oid.is_some());

    // Oracle: paths match `git worktree list --porcelain`.
    assert_eq!(sut_worktree_paths(&rows), cli_worktree_paths(&fx.main));
}

/// #2: two linked worktrees are listed with correct branch/oid/flags; the main
/// row is present and is_current; the linked rows are not. Path set matches the
/// CLI oracle.
#[test]
fn list_with_linked_worktrees() {
    require_git!();
    let fx = setup();
    let wt_feature = fx.root.join("wt-feature");
    let wt_detached = fx.root.join("wt-detached");

    git(&fx.main, &["worktree", "add", wt_feature.to_str().unwrap(), "feature"]);
    // A detached worktree at HEAD (branch == None on our side).
    git(
        &fx.main,
        &["worktree", "add", "--detach", wt_detached.to_str().unwrap(), "HEAD"],
    );

    let rows = list_worktrees(&fx.main).expect("list");
    assert_eq!(rows.len(), 3, "main + 2 linked, got {rows:?}");

    let main = rows.iter().find(|r| r.is_main).expect("main row");
    assert!(main.is_current);
    assert_eq!(main.branch.as_deref(), Some("main"));

    let feat = rows
        .iter()
        .find(|r| r.branch.as_deref() == Some("feature"))
        .expect("feature worktree row");
    assert!(!feat.is_main);
    assert!(!feat.is_current);
    assert!(feat.valid);
    assert!(!feat.prunable);
    assert_eq!(canon(Path::new(&feat.abs_path)), canon(&wt_feature));
    // headOid matches the worktree's actual HEAD.
    let feat_head = git(&wt_feature, &["rev-parse", "HEAD"]);
    assert_eq!(feat.head_oid.as_deref(), Some(feat_head.as_str()));

    let det = rows
        .iter()
        .find(|r| canon(Path::new(&r.abs_path)) == canon(&wt_detached))
        .expect("detached worktree row");
    assert!(!det.is_main);
    assert_eq!(det.branch, None, "detached HEAD → no branch");
    assert!(det.head_oid.is_some());

    // Oracle.
    assert_eq!(sut_worktree_paths(&rows), cli_worktree_paths(&fx.main));
}

/// #2b: opening `list_worktrees` FROM a linked worktree still synthesizes the
/// main row (via commondir) and marks the linked one is_current.
#[test]
fn list_from_linked_worktree_finds_main() {
    require_git!();
    let fx = setup();
    let wt_feature = fx.root.join("wt-feature");
    git(&fx.main, &["worktree", "add", wt_feature.to_str().unwrap(), "feature"]);

    let rows = list_worktrees(&wt_feature).expect("list from linked");
    let main = rows.iter().find(|r| r.is_main).expect("main row present");
    assert!(!main.is_current, "main is not the opened worktree here");
    assert_eq!(canon(Path::new(&main.abs_path)), canon(&fx.main));

    let feat = rows
        .iter()
        .find(|r| r.branch.as_deref() == Some("feature"))
        .expect("feature row");
    assert!(feat.is_current, "the opened linked worktree is current");
}

/// #3: a locked worktree reports locked==true with the reason.
#[test]
fn list_reports_locked_worktree() {
    require_git!();
    let fx = setup();
    let wt_feature = fx.root.join("wt-feature");
    git(&fx.main, &["worktree", "add", wt_feature.to_str().unwrap(), "feature"]);

    // `git worktree lock --reason` may vary by version; fall back to no reason.
    let with_reason = git_ok(
        &fx.main,
        &[
            "worktree",
            "lock",
            "--reason",
            "pinned for QA",
            wt_feature.to_str().unwrap(),
        ],
    );
    if !with_reason {
        assert!(
            git_ok(&fx.main, &["worktree", "lock", wt_feature.to_str().unwrap()]),
            "git worktree lock failed"
        );
    }

    let rows = list_worktrees(&fx.main).expect("list");
    let feat = rows
        .iter()
        .find(|r| r.branch.as_deref() == Some("feature"))
        .expect("feature row");
    assert!(feat.locked, "locked worktree must report locked==true");
    if with_reason {
        assert_eq!(feat.lock_reason.as_deref(), Some("pinned for QA"));
    }

    // Still consistent with the oracle.
    assert_eq!(sut_worktree_paths(&rows), cli_worktree_paths(&fx.main));
}
