//! M1 status oracle tests (contract §7): `read_status` must agree with
//! `git status --porcelain=v1 -z --untracked-files=all` on CLI-built scratch
//! repos in temp dirs. The git CLI is the independent oracle for our git2 code.
//!
//! Each test skips (passes with a note) if `git` is not on PATH.

use std::collections::BTreeSet;
use std::path::Path;
use std::process::Command;

use bonsai_lib::error::AppError;
use bonsai_lib::git::status::{read_status, FileStatus, StatusSnapshot};

/// Canonical comparison tuple: (list, path, orig_path, status).
type Tuple = (String, String, Option<String>, String);

fn have_git() -> bool {
    Command::new("git").arg("--version").output().is_ok()
}

fn git(dir: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap_or_else(|e| panic!("failed to run git {args:?}: {e}"));
    assert!(
        out.status.success(),
        "git {:?} failed: {}",
        args,
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

/// `git init` + deterministic local config in a fresh temp dir.
fn init_repo() -> tempfile::TempDir {
    let dir = tempfile::TempDir::new().expect("create temp dir");
    let path = dir.path();
    git(path, &["init"]);
    git(path, &["config", "user.name", "Test User"]);
    git(path, &["config", "user.email", "test@example.com"]);
    // Rename parity with our StatusOptions; autocrlf off for cross-side determinism.
    git(path, &["config", "status.renames", "true"]);
    git(path, &["config", "core.autocrlf", "false"]);
    dir
}

/// Repo with one committed file `tracked.txt` (multi-line so renames are detectable).
fn init_repo_with_commit() -> tempfile::TempDir {
    let dir = init_repo();
    let path = dir.path();
    std::fs::write(
        path.join("tracked.txt"),
        "line one\nline two\nline three\nline four\nline five\n",
    )
    .expect("write tracked.txt");
    git(path, &["add", "tracked.txt"]);
    git(path, &["commit", "-m", "initial commit"]);
    dir
}

fn status_name(s: FileStatus) -> &'static str {
    match s {
        FileStatus::Added => "added",
        FileStatus::Modified => "modified",
        FileStatus::Deleted => "deleted",
        FileStatus::Renamed => "renamed",
        FileStatus::Typechange => "typechange",
        FileStatus::Conflicted => "conflicted",
        FileStatus::Untracked => "untracked",
    }
}

/// Flattens our snapshot into the canonical tuple set.
fn flatten(snapshot: &StatusSnapshot) -> BTreeSet<Tuple> {
    let mut set = BTreeSet::new();
    for (list, entries) in [
        ("staged", &snapshot.staged),
        ("unstaged", &snapshot.unstaged),
        ("untracked", &snapshot.untracked),
        ("conflicted", &snapshot.conflicted),
    ] {
        for e in entries {
            set.insert((
                list.to_string(),
                e.path.clone(),
                e.orig_path.clone(),
                status_name(e.status).to_string(),
            ));
        }
    }
    set
}

fn is_conflict_code(x: char, y: char) -> bool {
    x == 'U' || y == 'U' || (x == 'A' && y == 'A') || (x == 'D' && y == 'D')
}

fn index_column_status(x: char) -> Option<&'static str> {
    match x {
        'A' => Some("added"),
        'M' => Some("modified"),
        'D' => Some("deleted"),
        'R' => Some("renamed"),
        'T' => Some("typechange"),
        _ => None,
    }
}

fn worktree_column_status(y: char) -> Option<&'static str> {
    match y {
        'M' => Some("modified"),
        'D' => Some("deleted"),
        'R' => Some("renamed"),
        'T' => Some("typechange"),
        _ => None,
    }
}

/// Runs the porcelain oracle and maps its output to the canonical tuple set.
fn porcelain_tuples(dir: &Path) -> BTreeSet<Tuple> {
    let out = Command::new("git")
        .args(["status", "--porcelain=v1", "-z", "--untracked-files=all"])
        .current_dir(dir)
        .output()
        .expect("run git status");
    assert!(
        out.status.success(),
        "git status failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let raw = String::from_utf8_lossy(&out.stdout).into_owned();
    let mut tokens = raw.split('\0').filter(|t| !t.is_empty());

    let mut set = BTreeSet::new();
    while let Some(token) = tokens.next() {
        let mut chars = token.chars();
        let x = chars.next().expect("X column");
        let y = chars.next().expect("Y column");
        assert_eq!(chars.next(), Some(' '), "porcelain separator in {token:?}");
        let path: String = chars.collect();

        // Rename entries carry the ORIG path as the next NUL token.
        let orig = if x == 'R' || y == 'R' {
            Some(tokens.next().expect("rename orig path token").to_string())
        } else {
            None
        };

        if x == '?' && y == '?' {
            set.insert((
                "untracked".to_string(),
                path,
                None,
                "untracked".to_string(),
            ));
            continue;
        }
        if is_conflict_code(x, y) {
            set.insert((
                "conflicted".to_string(),
                path,
                None,
                "conflicted".to_string(),
            ));
            continue;
        }
        if let Some(status) = index_column_status(x) {
            let orig_for_row = if x == 'R' { orig.clone() } else { None };
            set.insert((
                "staged".to_string(),
                path.clone(),
                orig_for_row,
                status.to_string(),
            ));
        }
        if let Some(status) = worktree_column_status(y) {
            let orig_for_row = if y == 'R' { orig.clone() } else { None };
            set.insert((
                "unstaged".to_string(),
                path.clone(),
                orig_for_row,
                status.to_string(),
            ));
        }
    }
    set
}

/// Asserts snapshot == porcelain oracle for the repo at `dir`, returning the snapshot.
fn assert_matches_porcelain(dir: &Path) -> StatusSnapshot {
    let snapshot = read_status(dir).expect("read_status");
    assert_eq!(
        flatten(&snapshot),
        porcelain_tuples(dir),
        "read_status disagrees with `git status --porcelain=v1 -z`"
    );
    snapshot
}

macro_rules! require_git {
    () => {
        if !have_git() {
            eprintln!("skipping: `git` CLI not found on PATH");
            return;
        }
    };
}

// Scenario 1: clean repo (one commit) -> all lists empty.
#[test]
fn clean_repo_is_empty() {
    require_git!();
    let dir = init_repo_with_commit();
    let snapshot = assert_matches_porcelain(dir.path());
    assert_eq!(snapshot, StatusSnapshot::default());
}

// Scenario 2: untracked files, incl. one nested in a new directory.
#[test]
fn untracked_files_including_nested() {
    require_git!();
    let dir = init_repo_with_commit();
    let path = dir.path();
    std::fs::write(path.join("loose.txt"), "loose\n").expect("write loose.txt");
    std::fs::create_dir(path.join("newdir")).expect("create newdir");
    std::fs::write(path.join("newdir").join("nested.txt"), "nested\n").expect("write nested");

    let snapshot = assert_matches_porcelain(path);
    let untracked: Vec<&str> = snapshot.untracked.iter().map(|e| e.path.as_str()).collect();
    assert_eq!(untracked, vec!["loose.txt", "newdir/nested.txt"]);
    assert!(snapshot.staged.is_empty());
    assert!(snapshot.unstaged.is_empty());
}

// Scenario 3: staged new file.
#[test]
fn staged_new_file() {
    require_git!();
    let dir = init_repo_with_commit();
    let path = dir.path();
    std::fs::write(path.join("new.txt"), "new\n").expect("write new.txt");
    git(path, &["add", "new.txt"]);

    let snapshot = assert_matches_porcelain(path);
    assert_eq!(snapshot.staged.len(), 1);
    assert_eq!(snapshot.staged[0].path, "new.txt");
    assert_eq!(snapshot.staged[0].status, FileStatus::Added);
}

// Scenario 4: modified tracked file, unstaged.
#[test]
fn modified_unstaged() {
    require_git!();
    let dir = init_repo_with_commit();
    let path = dir.path();
    std::fs::write(path.join("tracked.txt"), "changed\n").expect("modify tracked.txt");

    let snapshot = assert_matches_porcelain(path);
    assert_eq!(snapshot.unstaged.len(), 1);
    assert_eq!(snapshot.unstaged[0].path, "tracked.txt");
    assert_eq!(snapshot.unstaged[0].status, FileStatus::Modified);
    assert!(snapshot.staged.is_empty());
}

// Scenario 5: staged modification, then modified again -> BOTH lists.
#[test]
fn staged_then_remodified_appears_in_both_lists() {
    require_git!();
    let dir = init_repo_with_commit();
    let path = dir.path();
    std::fs::write(path.join("tracked.txt"), "staged change\n").expect("modify");
    git(path, &["add", "tracked.txt"]);
    std::fs::write(path.join("tracked.txt"), "staged change\nplus more\n").expect("re-modify");

    let snapshot = assert_matches_porcelain(path);
    assert!(snapshot
        .staged
        .iter()
        .any(|e| e.path == "tracked.txt" && e.status == FileStatus::Modified));
    assert!(snapshot
        .unstaged
        .iter()
        .any(|e| e.path == "tracked.txt" && e.status == FileStatus::Modified));
}

// Scenario 6a: deletion staged via `git rm`.
#[test]
fn deleted_staged() {
    require_git!();
    let dir = init_repo_with_commit();
    let path = dir.path();
    git(path, &["rm", "tracked.txt"]);

    let snapshot = assert_matches_porcelain(path);
    assert_eq!(snapshot.staged.len(), 1);
    assert_eq!(snapshot.staged[0].path, "tracked.txt");
    assert_eq!(snapshot.staged[0].status, FileStatus::Deleted);
    assert!(snapshot.unstaged.is_empty());
}

// Scenario 6b: deletion unstaged (fs delete only).
#[test]
fn deleted_unstaged() {
    require_git!();
    let dir = init_repo_with_commit();
    let path = dir.path();
    std::fs::remove_file(path.join("tracked.txt")).expect("delete tracked.txt");

    let snapshot = assert_matches_porcelain(path);
    assert_eq!(snapshot.unstaged.len(), 1);
    assert_eq!(snapshot.unstaged[0].path, "tracked.txt");
    assert_eq!(snapshot.unstaged[0].status, FileStatus::Deleted);
    assert!(snapshot.staged.is_empty());
}

// Scenario 7: staged rename (`git mv`) with correct orig_path on both sides.
#[test]
fn staged_rename_with_orig_path() {
    require_git!();
    let dir = init_repo_with_commit();
    let path = dir.path();
    git(path, &["mv", "tracked.txt", "renamed.txt"]);

    let snapshot = assert_matches_porcelain(path);
    assert_eq!(snapshot.staged.len(), 1);
    let entry = &snapshot.staged[0];
    assert_eq!(entry.status, FileStatus::Renamed);
    assert_eq!(entry.path, "renamed.txt");
    assert_eq!(entry.orig_path.as_deref(), Some("tracked.txt"));
    assert!(snapshot.unstaged.is_empty());
}

// Scenario 8: unborn repo with a staged file -> staged: Added.
#[test]
fn unborn_head_with_staged_file() {
    require_git!();
    let dir = init_repo();
    let path = dir.path();
    std::fs::write(path.join("first.txt"), "first\n").expect("write first.txt");
    git(path, &["add", "first.txt"]);

    let snapshot = assert_matches_porcelain(path);
    assert_eq!(snapshot.staged.len(), 1);
    assert_eq!(snapshot.staged[0].path, "first.txt");
    assert_eq!(snapshot.staged[0].status, FileStatus::Added);
}

// Scenario 9: bare repo -> Err(AppError::Git(_)).
#[test]
fn bare_repo_is_an_error() {
    let dir = tempfile::TempDir::new().expect("create temp dir");
    git2::Repository::init_bare(dir.path()).expect("init bare repo");

    let err = read_status(dir.path()).expect_err("bare repo must be an error");
    match err {
        AppError::Git(msg) => assert!(
            msg.contains("bare"),
            "error must mention bare repos, got: {msg}"
        ),
        other => panic!("expected AppError::Git, got: {other:?}"),
    }
}

// Scenario 10: no-repo path -> Err (not a panic).
#[test]
fn non_repo_path_is_an_error() {
    let dir = tempfile::TempDir::new().expect("create temp dir");
    let err = read_status(dir.path()).expect_err("non-repo dir must be an error");
    assert!(matches!(err, AppError::Git(_)), "got: {err:?}");
}
