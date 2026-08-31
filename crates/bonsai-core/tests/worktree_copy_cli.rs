//! P32 Part B CLI/fs-oracle tests for `worktree_copy` (contract §P32 extension
//! Part B). Exercises `list_copy_candidates`, `classify_copy`, and
//! `add_worktree_with_changes` on real scratch repos built with the `git` CLI,
//! mirroring the fixture/oracle conventions of `worktree_cli.rs`.
//!
//! Every test skips (passes with a note) if `git` is not on PATH. Scratch repos
//! live under `D:\Data\Temp\bonsai-scratch` (never the system temp).

mod common;

use std::path::{Path, PathBuf};

use bonsai_core::error::AppError;
use bonsai_core::git::worktree_copy::{
    add_worktree_with_changes, classify_copy, list_copy_candidates, CopyAction, CopyGroup,
    CopySelection, CopyVerdict,
};
use common::{commit_fixed, git};

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
    git(repo, &["config", "status.renames", "true"]);
}

/// A fresh `root/main` repo (branch `main`), identity configured, returned inside
/// a unique tempdir so `.worktrees/` containers never leak between tests.
struct Repo {
    _dir: tempfile::TempDir,
    root: PathBuf,
    main: PathBuf,
}

fn init_main() -> Repo {
    let dir = common::scratch_dir();
    let root = dir.path().to_path_buf();
    let main = root.join("main");
    std::fs::create_dir_all(&main).expect("mkdir main");
    git(&main, &["init", "-b", "main"]);
    configure_identity(&main);
    Repo {
        _dir: dir,
        root,
        main,
    }
}

fn write(dir: &Path, rel: &str, content: &str) {
    let p = dir.join(rel);
    if let Some(parent) = p.parent() {
        std::fs::create_dir_all(parent).expect("mkdir parent");
    }
    std::fs::write(&p, content).expect("write file");
}

fn read(p: &Path) -> String {
    std::fs::read_to_string(p).expect("read file")
}

fn s(x: &str) -> String {
    x.to_string()
}

// ---------------------------------------------------------------------------
// classify_copy
// ---------------------------------------------------------------------------

/// Unborn HEAD (source base tree is None): a path present on the target branch →
/// conflict (base None, target Some diverge); a path absent on target → clean.
#[test]
fn classify_unborn_head() {
    require_git!();
    let fx = init_main();
    write(&fx.main, "shared.txt", "base content\n");
    git(&fx.main, &["add", "-A"]);
    commit_fixed(&fx.main, "A");
    git(&fx.main, &["branch", "feature"]);
    git(&fx.main, &["checkout", "feature"]);
    write(&fx.main, "shared.txt", "feature diverged\n");
    git(&fx.main, &["add", "-A"]);
    commit_fixed(&fx.main, "feature edit");
    git(&fx.main, &["checkout", "main"]);
    // Make HEAD unborn: point it at a branch with no commits.
    git(&fx.main, &["symbolic-ref", "HEAD", "refs/heads/unborn"]);

    let plan = classify_copy(
        &fx.main,
        "feature",
        &[s("shared.txt"), s("ghost.txt")],
    )
    .expect("classify");
    assert_eq!(plan.len(), 2);
    // present on target, base None → conflict
    assert_eq!(plan[0].path, "shared.txt");
    assert_eq!(plan[0].verdict, CopyVerdict::Conflict);
    // absent on target → clean
    assert_eq!(plan[1].path, "ghost.txt");
    assert_eq!(plan[1].verdict, CopyVerdict::Clean);
}

/// Target blob byte-equal to base (main HEAD) → clean; target diverged → conflict.
#[test]
fn classify_equal_vs_diverged() {
    require_git!();
    let fx = init_main();
    write(&fx.main, "f.txt", "hello\n"); // unchanged on feature
    write(&fx.main, "g.txt", "orig\n"); // diverges on feature
    git(&fx.main, &["add", "-A"]);
    commit_fixed(&fx.main, "A");
    git(&fx.main, &["branch", "feature"]);
    git(&fx.main, &["checkout", "feature"]);
    write(&fx.main, "g.txt", "changed\n");
    git(&fx.main, &["add", "-A"]);
    commit_fixed(&fx.main, "feature diverge g");
    git(&fx.main, &["checkout", "main"]); // base = main HEAD

    let plan = classify_copy(&fx.main, "feature", &[s("f.txt"), s("g.txt")]).expect("classify");
    assert_eq!(plan[0].verdict, CopyVerdict::Clean, "f.txt: target==base");
    assert_eq!(plan[1].verdict, CopyVerdict::Conflict, "g.txt: target diverged");
}

/// A path that is a DIRECTORY on the target side → not a blob → treated as absent
/// → clean (NOT an error).
#[test]
fn classify_directory_treated_absent() {
    require_git!();
    let fx = init_main();
    write(&fx.main, "f.txt", "x\n");
    git(&fx.main, &["add", "-A"]);
    commit_fixed(&fx.main, "A");
    git(&fx.main, &["branch", "feature"]);
    git(&fx.main, &["checkout", "feature"]);
    write(&fx.main, "subdir/inner.txt", "inner\n");
    git(&fx.main, &["add", "-A"]);
    commit_fixed(&fx.main, "feature add subdir");
    git(&fx.main, &["checkout", "main"]);

    // "subdir" is a tree on feature → treated as absent → clean.
    let plan = classify_copy(&fx.main, "feature", &[s("subdir")]).expect("classify must not error");
    assert_eq!(plan.len(), 1);
    assert_eq!(plan[0].verdict, CopyVerdict::Clean);
}

/// Unknown / bad branch → BranchNotFound.
#[test]
fn classify_unknown_branch_is_branch_not_found() {
    require_git!();
    let fx = init_main();
    write(&fx.main, "f.txt", "x\n");
    git(&fx.main, &["add", "-A"]);
    commit_fixed(&fx.main, "A");

    match classify_copy(&fx.main, "no-such-branch", &[s("f.txt")]) {
        Err(AppError::BranchNotFound(_)) => {}
        other => panic!("expected BranchNotFound, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// add_worktree_with_changes
// ---------------------------------------------------------------------------

/// Build `root/main` with a `feature` branch. main HEAD has `conf.txt`; feature
/// diverges `conf.txt` so it is a genuine conflict when copying.
fn setup_with_feature() -> Repo {
    let fx = init_main();
    write(&fx.main, "conf.txt", "main version\n");
    write(&fx.main, "a.txt", "a\n");
    git(&fx.main, &["add", "-A"]);
    commit_fixed(&fx.main, "A");
    git(&fx.main, &["branch", "feature"]);
    git(&fx.main, &["checkout", "feature"]);
    write(&fx.main, "conf.txt", "feature version\n");
    git(&fx.main, &["add", "-A"]);
    commit_fixed(&fx.main, "feature diverge conf");
    git(&fx.main, &["checkout", "main"]);
    fx
}

/// A `copy` selection writes the MAIN WORKDIR bytes to the guarded dest inside the
/// new worktree at `.worktrees/main/<name>/<path>`.
#[test]
fn add_copy_writes_workdir_bytes() {
    require_git!();
    let fx = setup_with_feature();
    // Untracked file in the main workdir with distinctive bytes.
    write(&fx.main, "copyme.txt", "workdir bytes\n");

    let info = add_worktree_with_changes(
        &fx.main,
        "feature",
        "wt-copy",
        &[CopySelection {
            path: s("copyme.txt"),
            action: CopyAction::Copy,
        }],
    )
    .expect("add with copy");

    let wt_root = PathBuf::from(&info.abs_path);
    // Path shape: .worktrees/main/wt-copy
    let expected_root = fx.root.join(".worktrees").join("main").join("wt-copy");
    assert_eq!(
        std::fs::canonicalize(&wt_root).unwrap_or(wt_root.clone()),
        std::fs::canonicalize(&expected_root).unwrap_or(expected_root.clone())
    );
    let dest = wt_root.join("copyme.txt");
    assert!(dest.exists(), "copied file must exist at {dest:?}");
    assert_eq!(read(&dest), "workdir bytes\n");
}

/// A conflicted path with action `skip` leaves the target branch's checked-out
/// version intact in the worktree (NOT the main-workdir edit).
#[test]
fn add_skip_conflict_keeps_branch_version() {
    require_git!();
    let fx = setup_with_feature();
    // Edit the tracked conflicting file in the main workdir (unstaged).
    write(&fx.main, "conf.txt", "workdir edit\n");

    let info = add_worktree_with_changes(
        &fx.main,
        "feature",
        "wt-skip",
        &[CopySelection {
            path: s("conf.txt"),
            action: CopyAction::Skip,
        }],
    )
    .expect("add with skip");

    let dest = PathBuf::from(&info.abs_path).join("conf.txt");
    assert_eq!(
        read(&dest),
        "feature version\n",
        "skip must leave the branch's checked-out version"
    );
}

/// Containment guard: crafted escaping selection paths return AppError::Git and
/// write NOTHING outside the worktree root. Each uses a distinct branch (a branch
/// can only be checked out in one worktree at a time).
#[test]
fn add_containment_guard_rejects_escapes() {
    require_git!();
    let fx = setup_with_feature();
    git(&fx.main, &["branch", "feat2"]);
    git(&fx.main, &["branch", "feat3"]);

    // Case 1: parent-dir escape.
    let escape_target = fx.root.join(".worktrees").join("main").join("escape.txt");
    match add_worktree_with_changes(
        &fx.main,
        "feature",
        "wt-esc1",
        &[CopySelection {
            path: s("../escape.txt"),
            action: CopyAction::Copy,
        }],
    ) {
        Err(AppError::Git(_)) => {}
        other => panic!("expected Git error for ../escape, got {other:?}"),
    }
    assert!(!escape_target.exists(), "no stray file outside worktree root");

    // Case 2: absolute path.
    let abs_target = fx.root.join("abs-escape.txt");
    match add_worktree_with_changes(
        &fx.main,
        "feat2",
        "wt-esc2",
        &[CopySelection {
            path: abs_target.to_string_lossy().replace('\\', "/"),
            action: CopyAction::Copy,
        }],
    ) {
        Err(AppError::Git(_)) => {}
        other => panic!("expected Git error for absolute path, got {other:?}"),
    }
    assert!(!abs_target.exists(), "absolute escape must write nothing");

    // Case 3 (Windows): drive-prefixed path.
    #[cfg(windows)]
    {
        match add_worktree_with_changes(
            &fx.main,
            "feat3",
            "wt-esc3",
            &[CopySelection {
                path: s("C:/Windows/Temp/bonsai-escape.txt"),
                action: CopyAction::Copy,
            }],
        ) {
            Err(AppError::Git(_)) => {}
            other => panic!("expected Git error for drive-prefixed path, got {other:?}"),
        }
    }
}

/// Empty selection behaves exactly like a plain worktree (no extra files copied).
#[test]
fn add_empty_selection_is_plain_worktree() {
    require_git!();
    let fx = setup_with_feature();
    // An untracked file in main that must NOT leak into the worktree.
    write(&fx.main, "leak.txt", "should not copy\n");

    let info =
        add_worktree_with_changes(&fx.main, "feature", "wt-plain", &[]).expect("add empty");
    let wt_root = PathBuf::from(&info.abs_path);
    assert!(wt_root.join("conf.txt").exists(), "branch checkout present");
    assert!(
        !wt_root.join("leak.txt").exists(),
        "empty selection must not copy the untracked file"
    );
    assert!(
        !wt_root.join("copyme.txt").exists(),
        "no other stray files"
    );
}

// ---------------------------------------------------------------------------
// list_copy_candidates
// ---------------------------------------------------------------------------

/// The `include_ignored` pass surfaces a gitignored file once as `ignored` and
/// NOT as untracked; a plain untracked file appears once as `untracked`.
#[test]
fn list_ignored_vs_untracked() {
    require_git!();
    let fx = init_main();
    write(&fx.main, ".gitignore", "ignored.txt\n");
    write(&fx.main, "seed.txt", "seed\n");
    git(&fx.main, &["add", "-A"]);
    commit_fixed(&fx.main, "A");
    // Now create an ignored file and an untracked file.
    write(&fx.main, "ignored.txt", "secret\n");
    write(&fx.main, "untracked.txt", "new\n");

    let cands = list_copy_candidates(&fx.main).expect("list");

    let ignored: Vec<_> = cands.iter().filter(|c| c.path == "ignored.txt").collect();
    assert_eq!(ignored.len(), 1, "ignored.txt appears exactly once: {cands:?}");
    assert_eq!(ignored[0].group, CopyGroup::Ignored);

    let untracked: Vec<_> = cands.iter().filter(|c| c.path == "untracked.txt").collect();
    assert_eq!(untracked.len(), 1, "untracked.txt appears exactly once");
    assert_eq!(untracked[0].group, CopyGroup::Untracked);

    // ignored.txt is NOT also reported as untracked.
    assert!(
        !cands
            .iter()
            .any(|c| c.path == "ignored.txt" && c.group == CopyGroup::Untracked),
        "gitignored file must not appear in the untracked group"
    );
}

/// A file modified in BOTH index and workdir appears as two candidates (Staged +
/// Unstaged) with the same path.
#[test]
fn list_staged_and_unstaged_same_path() {
    require_git!();
    let fx = init_main();
    write(&fx.main, "both.txt", "v0\n");
    git(&fx.main, &["add", "-A"]);
    commit_fixed(&fx.main, "A");
    // Stage a change, then re-modify in the workdir.
    write(&fx.main, "both.txt", "v1-staged\n");
    git(&fx.main, &["add", "both.txt"]);
    write(&fx.main, "both.txt", "v2-workdir\n");

    let cands = list_copy_candidates(&fx.main).expect("list");
    let groups: Vec<_> = cands
        .iter()
        .filter(|c| c.path == "both.txt")
        .map(|c| c.group)
        .collect();
    assert!(
        groups.contains(&CopyGroup::Staged),
        "both.txt must be a Staged candidate: {cands:?}"
    );
    assert!(
        groups.contains(&CopyGroup::Unstaged),
        "both.txt must be an Unstaged candidate: {cands:?}"
    );
    assert_eq!(groups.len(), 2, "exactly two candidates for the same path");
}

/// A staged rename surfaces the NEW path as a candidate; a deleted file does NOT
/// appear at all.
#[test]
fn list_rename_new_path_delete_excluded() {
    require_git!();
    let fx = init_main();
    write(&fx.main, "oldname.txt", "same content stays identical\n");
    write(&fx.main, "todelete.txt", "goodbye\n");
    git(&fx.main, &["add", "-A"]);
    commit_fixed(&fx.main, "A");
    // Staged rename (identical content → 100% similarity → detected as rename).
    git(&fx.main, &["mv", "oldname.txt", "newname.txt"]);
    // Staged deletion.
    git(&fx.main, &["rm", "todelete.txt"]);

    let cands = list_copy_candidates(&fx.main).expect("list");
    let paths: Vec<_> = cands.iter().map(|c| c.path.as_str()).collect();
    assert!(paths.contains(&"newname.txt"), "rename new path present: {cands:?}");
    assert!(!paths.contains(&"oldname.txt"), "rename old path absent");
    assert!(!paths.contains(&"todelete.txt"), "deleted file must be excluded");
}

/// Audit §3.1: the branch being checked out into the new worktree may carry a
/// SYMLINK where a copy selection lands - at the leaf (`config -> <outside>`) or at
/// a directory component (`dir -> <outside dir>`). `fs::write`/`create_dir_all`
/// follow symlinks, so without the guard the copy would clobber a file OUTSIDE the
/// worktree ("user picks Overwrite, ~/.gitconfig is destroyed"). Both must be
/// refused with `AppError::Git` and the outside file left byte-identical.
///
/// Unix-only: creating a symlink on Windows needs privilege, and git only checks
/// one out there with `core.symlinks=true` (house pattern: stash.rs's
/// `#[cfg(not(windows))]` reserved-name tests).
#[cfg(unix)]
#[test]
fn add_refuses_symlink_write_through() {
    require_git!();
    let fx = setup_with_feature();
    // A precious file OUTSIDE the repo and outside the .worktrees container.
    let precious = fx.root.join("precious.txt");
    std::fs::write(&precious, "precious\n").expect("write precious");

    // Commit the two symlinks on `feature` (+ a twin branch: the failed call still
    // creates the worktree, and a branch can only be checked out once).
    git(&fx.main, &["checkout", "feature"]);
    std::os::unix::fs::symlink(&precious, fx.main.join("config")).expect("symlink leaf");
    std::os::unix::fs::symlink(&fx.root, fx.main.join("dir")).expect("symlink dir");
    git(&fx.main, &["add", "-A"]);
    commit_fixed(&fx.main, "plant symlinks");
    git(&fx.main, &["branch", "feature-twin"]);
    git(&fx.main, &["checkout", "main"]);

    // The main workdir holds the payload the user would be copying over.
    write(&fx.main, "config", "pwned\n");
    write(&fx.main, "dir/precious.txt", "pwned\n");

    // Case 1: the destination leaf itself is a symlink.
    match add_worktree_with_changes(
        &fx.main,
        "feature",
        "wt-sym-leaf",
        &[CopySelection {
            path: s("config"),
            action: CopyAction::Copy,
        }],
    ) {
        Err(AppError::Git(msg)) => assert!(msg.contains("symlink"), "unexpected msg: {msg}"),
        other => panic!("expected a Git refusal for the symlinked leaf, got {other:?}"),
    }
    assert_eq!(
        read(&precious),
        "precious\n",
        "symlinked leaf must NOT be written through"
    );

    // Case 2: a DIRECTORY component of the destination is a symlink.
    match add_worktree_with_changes(
        &fx.main,
        "feature-twin",
        "wt-sym-dir",
        &[CopySelection {
            path: s("dir/precious.txt"),
            action: CopyAction::Copy,
        }],
    ) {
        Err(AppError::Git(msg)) => assert!(msg.contains("symlink"), "unexpected msg: {msg}"),
        other => panic!("expected a Git refusal for the symlinked dir, got {other:?}"),
    }
    assert_eq!(
        read(&precious),
        "precious\n",
        "symlinked directory component must NOT be written through"
    );
}
