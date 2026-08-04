//! P27 CLI-oracle worktree tests (contract §7): the P27a read-only list plus
//! the P27b mutating ops (`add`/`remove`/`lock`/`unlock`), each cross-checked
//! against `git worktree list --porcelain`. The load-bearing oracle is that the
//! SET of worktree paths reported by `list_worktrees` equals the set parsed
//! from the porcelain output after every operation.
//!
//! Every test skips (passes with a note) if `git` is not on PATH. All scratch
//! repos live under `D:\Temp\bonsai-scratch` (never the system temp).

mod common;

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use bonsai_core::error::AppError;
use bonsai_core::git::worktree::{
    add_worktree, list_worktrees, lock_worktree, remove_worktree, unlock_worktree, WorktreeInfo,
};
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

/// P27a carry-forward: a STALE worktree (its directory deleted out-of-band)
/// never panics the list — the row survives with `valid==false` (or prunable)
/// and null branch/oid.
#[test]
fn list_stale_worktree_does_not_panic() {
    require_git!();
    let fx = setup();
    let wt_feature = fx.root.join("wt-feature");
    git(&fx.main, &["worktree", "add", wt_feature.to_str().unwrap(), "feature"]);
    std::fs::remove_dir_all(&wt_feature).expect("delete worktree dir");

    let rows = list_worktrees(&fx.main).expect("list must not fail on stale worktree");
    assert_eq!(rows.len(), 2, "main + stale linked, got {rows:?}");
    let stale = rows.iter().find(|r| !r.is_main).expect("stale row");
    assert!(
        stale.prunable || !stale.valid,
        "deleted dir must show prunable/invalid: {stale:?}"
    );
    assert_eq!(stale.branch, None);
    assert_eq!(stale.head_oid, None);
}

/// §7.2 #2: `add_worktree("feature", "feature")` creates a worktree at
/// `<parent>/.worktrees/main/feature`, returns the created row, and matches the
/// porcelain oracle + the worktree's real HEAD.
#[test]
fn add_worktree_happy_path() {
    require_git!();
    let fx = setup();

    let row = add_worktree(&fx.main, "feature", "feature").expect("add");
    assert!(!row.is_main);
    assert!(!row.is_current);
    assert!(row.valid);
    assert_eq!(row.branch.as_deref(), Some("feature"));
    assert_eq!(row.name, "feature");

    let expected = fx.root.join(".worktrees").join("main").join("feature");
    assert_eq!(canon(Path::new(&row.abs_path)), canon(&expected));
    assert!(expected.is_dir(), "worktree dir must exist on disk");

    // Oracle: git sees two worktrees, and the new one has HEAD == feature.
    let rows = list_worktrees(&fx.main).expect("list");
    assert_eq!(rows.len(), 2);
    assert_eq!(sut_worktree_paths(&rows), cli_worktree_paths(&fx.main));
    let head = git(&expected, &["rev-parse", "--abbrev-ref", "HEAD"]);
    assert_eq!(head.trim(), "feature");
}

/// P32 Part A (§A.0/A.5): the worktree NAME is decoupled from the checked-out
/// BRANCH. Creating with `branch="feature"` but a custom `name="my-feature-wt"`
/// puts the dir leaf at `.worktrees/main/my-feature-wt` while the worktree still
/// has `feature` checked out. The porcelain oracle still matches the SUT paths.
#[test]
fn add_worktree_name_decoupled_from_branch() {
    require_git!();
    let fx = setup();

    let row = add_worktree(&fx.main, "feature", "my-feature-wt").expect("add");

    // (a) the on-disk leaf uses the NAME (under the per-repo `.worktrees/main/`),
    //     not the branch.
    assert_eq!(row.name, "my-feature-wt");
    let expected = fx
        .root
        .join(".worktrees")
        .join("main")
        .join("my-feature-wt");
    assert_eq!(canon(Path::new(&row.abs_path)), canon(&expected));
    assert!(expected.is_dir(), "name-derived worktree dir must exist");
    // The branch-named leaf must NOT have been used.
    assert!(!fx.root.join(".worktrees").join("main").join("feature").exists());

    // (b) the checked-out branch is still `feature` — via the returned row AND the
    //     git CLI porcelain oracle.
    assert_eq!(row.branch.as_deref(), Some("feature"));
    let head = git(&expected, &["rev-parse", "--abbrev-ref", "HEAD"]);
    assert_eq!(head.trim(), "feature");

    // (c) the SUT path set still matches `git worktree list --porcelain`.
    let rows = list_worktrees(&fx.main).expect("list");
    assert_eq!(rows.len(), 2);
    assert_eq!(sut_worktree_paths(&rows), cli_worktree_paths(&fx.main));
}

/// §7.2 #3: sanitizer (`feat/x` → `feat-x`) + collision suffix (`-2` when the
/// slug is already taken by an existing worktree).
#[test]
fn add_worktree_sanitizes_and_suffixes_collisions() {
    require_git!();
    let fx = setup();
    git(&fx.main, &["branch", "feat/x"]);
    // A branch whose slug collides with `feature` ("feature+" → "feature").
    git(&fx.main, &["branch", "feature+"]);

    let slugged = add_worktree(&fx.main, "feat/x", "feat/x").expect("add feat/x");
    assert_eq!(slugged.name, "feat-x");
    assert_eq!(
        canon(Path::new(&slugged.abs_path)),
        canon(&fx.root.join(".worktrees").join("main").join("feat-x"))
    );

    add_worktree(&fx.main, "feature", "feature").expect("add feature");
    let collided = add_worktree(&fx.main, "feature+", "feature+").expect("add feature+");
    assert_eq!(collided.name, "feature-2", "collision must suffix -2");
    assert_eq!(
        canon(Path::new(&collided.abs_path)),
        canon(&fx.root.join(".worktrees").join("main").join("feature-2"))
    );
    // Both collision-sibling directories REALLY exist on disk (canon() falls
    // back lexically, so set-equality alone does not prove existence).
    assert!(fx.root.join(".worktrees").join("main").join("feature").is_dir());
    assert!(fx.root.join(".worktrees").join("main").join("feature-2").is_dir());

    assert_eq!(
        sut_worktree_paths(&list_worktrees(&fx.main).expect("list")),
        cli_worktree_paths(&fx.main)
    );
}

/// §7.2 #4: nonexistent branch → BranchNotFound; a branch already checked out
/// in another worktree (here: `main`, checked out in the main worktree) → Git.
#[test]
fn add_worktree_refusals() {
    require_git!();
    let fx = setup();

    match add_worktree(&fx.main, "no-such-branch", "no-such-branch") {
        Err(AppError::BranchNotFound(_)) => {}
        other => panic!("expected BranchNotFound, got {other:?}"),
    }
    match add_worktree(&fx.main, "main", "main") {
        Err(AppError::Git(_)) => {}
        other => panic!("expected Git error for already-checked-out branch, got {other:?}"),
    }
    // Nothing was created. (The per-repo `.worktrees/main/` container may be
    // pre-created by add_worktree before the checkout refusal fires; the load-
    // bearing check is that no worktree LEAF was created for the refused add.)
    assert_eq!(list_worktrees(&fx.main).expect("list").len(), 1);
    assert!(!fx.root.join(".worktrees").join("main").join("main").exists());
}

/// §7.2 #5: lock (with reason) / unlock round-trip, cross-checked with the
/// porcelain `locked` attribute.
#[test]
fn lock_unlock_round_trip() {
    require_git!();
    let fx = setup();
    let row = add_worktree(&fx.main, "feature", "feature").expect("add");

    lock_worktree(&fx.main, &row.name, Some("pinned for QA")).expect("lock");
    let rows = list_worktrees(&fx.main).expect("list");
    let feat = rows.iter().find(|r| r.name == row.name).expect("row");
    assert!(feat.locked);
    assert_eq!(feat.lock_reason.as_deref(), Some("pinned for QA"));
    let porcelain = git(&fx.main, &["worktree", "list", "--porcelain"]);
    assert!(porcelain.contains("locked"), "oracle must show locked:\n{porcelain}");

    unlock_worktree(&fx.main, &row.name).expect("unlock");
    let rows = list_worktrees(&fx.main).expect("list");
    let feat = rows.iter().find(|r| r.name == row.name).expect("row");
    assert!(!feat.locked);
    assert_eq!(feat.lock_reason, None);
    let porcelain = git(&fx.main, &["worktree", "list", "--porcelain"]);
    assert!(!porcelain.contains("locked"), "oracle must be unlocked:\n{porcelain}");

    // Blank/unknown names are refused.
    match lock_worktree(&fx.main, "no-such-wt", None) {
        Err(AppError::Git(_)) => {}
        other => panic!("expected Git not-found, got {other:?}"),
    }
}

/// §7.2 #6: remove refusals — main, current, locked, dirty. None delete
/// anything from disk.
#[test]
fn remove_worktree_refusals() {
    require_git!();
    let fx = setup();
    let feat = add_worktree(&fx.main, "feature", "feature").expect("add");
    let feat_dir = PathBuf::from(&feat.abs_path);

    // Main (by its basename, "main").
    match remove_worktree(&fx.main, "main") {
        Err(AppError::Git(msg)) => assert!(msg.contains("main worktree"), "{msg}"),
        other => panic!("expected Git refusal for main, got {other:?}"),
    }
    // Current: opened FROM the linked worktree, removing itself.
    match remove_worktree(&feat_dir, &feat.name) {
        Err(AppError::Git(msg)) => assert!(msg.contains("currently have open"), "{msg}"),
        other => panic!("expected Git refusal for current, got {other:?}"),
    }
    // Locked.
    lock_worktree(&fx.main, &feat.name, None).expect("lock");
    match remove_worktree(&fx.main, &feat.name) {
        Err(AppError::Git(msg)) => assert!(msg.contains("locked"), "{msg}"),
        other => panic!("expected Git refusal for locked, got {other:?}"),
    }
    unlock_worktree(&fx.main, &feat.name).expect("unlock");
    // Dirty (an untracked file counts).
    std::fs::write(feat_dir.join("scratch.txt"), "wip\n").expect("write");
    match remove_worktree(&fx.main, &feat.name) {
        Err(AppError::Git(msg)) => assert!(msg.contains("uncommitted"), "{msg}"),
        other => panic!("expected Git refusal for dirty, got {other:?}"),
    }
    // Unknown name.
    match remove_worktree(&fx.main, "no-such-wt") {
        Err(AppError::Git(_)) => {}
        other => panic!("expected Git not-found, got {other:?}"),
    }

    // Nothing was deleted; the oracle still lists both worktrees.
    assert!(fx.main.is_dir());
    assert!(feat_dir.is_dir());
    assert_eq!(
        sut_worktree_paths(&list_worktrees(&fx.main).expect("list")),
        cli_worktree_paths(&fx.main)
    );
    assert_eq!(cli_worktree_paths(&fx.main).len(), 2);
}

/// E2E: `add_worktree`, then open the CREATED worktree as its own repo and
/// list from inside it — `is_current` flips to the new row, the main row is
/// still synthesized (via commondir), and the oracle path set matches when
/// queried from inside the worktree too.
#[test]
fn add_then_list_from_inside_new_worktree_flips_current() {
    require_git!();
    let fx = setup();
    let row = add_worktree(&fx.main, "feature", "feature").expect("add");
    let wt_dir = PathBuf::from(&row.abs_path);

    // From the MAIN repo the new row is NOT current.
    let from_main = list_worktrees(&fx.main).expect("list from main");
    let r = from_main.iter().find(|r| r.name == row.name).expect("row");
    assert!(!r.is_current);
    assert!(from_main.iter().find(|r| r.is_main).expect("main").is_current);

    // From INSIDE the created worktree, is_current flips.
    let from_wt = list_worktrees(&wt_dir).expect("list from inside worktree");
    assert_eq!(from_wt.len(), 2);
    let main_row = from_wt.iter().find(|r| r.is_main).expect("main row");
    assert!(!main_row.is_current, "main must not be current here");
    assert_eq!(canon(Path::new(&main_row.abs_path)), canon(&fx.main));
    let cur = from_wt.iter().find(|r| r.is_current).expect("current row");
    assert!(!cur.is_main);
    assert_eq!(cur.name, row.name);
    assert_eq!(cur.branch.as_deref(), Some("feature"));

    // Oracle holds when queried from inside the worktree as well.
    assert_eq!(sut_worktree_paths(&from_wt), cli_worktree_paths(&wt_dir));
}

/// §OPEN-3 data-loss pin: a dirty-refused remove leaves the dirty file's
/// CONTENT byte-for-byte intact (both an untracked file and a modified
/// tracked file).
#[test]
fn remove_refusal_preserves_dirty_file_content() {
    require_git!();
    let fx = setup();
    let feat = add_worktree(&fx.main, "feature", "feature").expect("add");
    let feat_dir = PathBuf::from(&feat.abs_path);

    let untracked = feat_dir.join("precious.txt");
    std::fs::write(&untracked, "precious untracked bytes\n").expect("write untracked");
    let tracked = feat_dir.join("a.txt");
    std::fs::write(&tracked, "modified tracked bytes\n").expect("write tracked");

    match remove_worktree(&fx.main, &feat.name) {
        Err(AppError::Git(msg)) => assert!(msg.contains("uncommitted"), "{msg}"),
        other => panic!("expected Git refusal for dirty, got {other:?}"),
    }

    assert_eq!(
        std::fs::read_to_string(&untracked).expect("untracked survives"),
        "precious untracked bytes\n"
    );
    assert_eq!(
        std::fs::read_to_string(&tracked).expect("tracked survives"),
        "modified tracked bytes\n"
    );
    // Still listed by both sides.
    assert_eq!(
        sut_worktree_paths(&list_worktrees(&fx.main).expect("list")),
        cli_worktree_paths(&fx.main)
    );
}

/// §OPEN-4 end-to-end: lock → remove refused → unlock → remove succeeds; the
/// directory is gone and the porcelain oracle no longer lists it.
#[test]
fn unlock_then_remove_succeeds_end_to_end() {
    require_git!();
    let fx = setup();
    let feat = add_worktree(&fx.main, "feature", "feature").expect("add");
    let feat_dir = PathBuf::from(&feat.abs_path);

    lock_worktree(&fx.main, &feat.name, Some("hold")).expect("lock");
    match remove_worktree(&fx.main, &feat.name) {
        Err(AppError::Git(msg)) => assert!(msg.contains("locked"), "{msg}"),
        other => panic!("expected locked refusal, got {other:?}"),
    }
    assert!(feat_dir.is_dir(), "refusal must not delete anything");

    unlock_worktree(&fx.main, &feat.name).expect("unlock");
    remove_worktree(&fx.main, &feat.name).expect("remove after unlock");

    assert!(!feat_dir.exists(), "working directory must be deleted");
    let cli = cli_worktree_paths(&fx.main);
    assert_eq!(cli.len(), 1, "oracle must only list main: {cli:?}");
    let rows = list_worktrees(&fx.main).expect("list");
    assert_eq!(rows.len(), 1);
    assert!(rows[0].is_main);
}

/// §7.2 #7: remove happy path — a clean, unlocked, non-current worktree is
/// pruned: directory gone AND no longer in the porcelain oracle.
#[test]
fn remove_worktree_happy_path() {
    require_git!();
    let fx = setup();
    let feat = add_worktree(&fx.main, "feature", "feature").expect("add");
    let feat_dir = PathBuf::from(&feat.abs_path);
    assert!(feat_dir.is_dir());

    remove_worktree(&fx.main, &feat.name).expect("remove");

    assert!(!feat_dir.exists(), "working directory must be deleted");
    let cli = cli_worktree_paths(&fx.main);
    assert_eq!(cli.len(), 1, "oracle must no longer list it: {cli:?}");
    let rows = list_worktrees(&fx.main).expect("list");
    assert_eq!(rows.len(), 1);
    assert!(rows[0].is_main);
    assert_eq!(sut_worktree_paths(&rows), cli);
}
