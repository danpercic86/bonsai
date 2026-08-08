//! M3 CLI-oracle commit tests (contract §6.2).
//!
//! Twin repos built by identical scripts with fixed base-commit dates, so
//! base oids match. Repo A commits via `create_commit`, twin B via
//! `git commit -m`. We compare `git cat-file commit HEAD` FIELDS, not oids
//! (author/committer timestamps differ): tree oid, parent lines, author
//! name+email, committer name+email, full message body.
//!
//! Each test skips (passes with a note) if `git` is not on PATH.

mod common;

use std::path::Path;

use bonsai_core::error::AppError;
use bonsai_core::git::commit::create_commit;
use bonsai_core::git::stage::stage_paths;
use bonsai_core::git::status::read_status;
use common::{commit_fixed, git, git_ok, git_raw, init_repo};

macro_rules! require_git {
    () => {
        if !common::have_git() {
            eprintln!("skipping: `git` CLI not found on PATH");
            return;
        }
    };
}

/// Base fixture: one committed file with a fixed-date commit.
fn base_repo() -> tempfile::TempDir {
    let dir = init_repo();
    let path = dir.path();
    std::fs::write(path.join("tracked.txt"), "base content\n").expect("write tracked.txt");
    git(path, &["add", "-A"]);
    commit_fixed(path, "base");
    dir
}

fn twins(
    base: fn() -> tempfile::TempDir,
    setup: impl Fn(&Path),
) -> (tempfile::TempDir, tempfile::TempDir) {
    let a = base();
    let b = base();
    setup(a.path());
    setup(b.path());
    (a, b)
}

/// Parsed `git cat-file commit HEAD`.
#[derive(Debug, PartialEq)]
struct CommitObj {
    tree: String,
    parents: Vec<String>,
    /// "Name <email>" (timestamp/timezone stripped).
    author: String,
    committer: String,
    /// Raw message bytes after the header block (trailing newline included).
    message: String,
}

fn cat_file_head(dir: &Path) -> CommitObj {
    let raw = git_raw(dir, &["cat-file", "commit", "HEAD"], &[]);
    let text = String::from_utf8(raw).expect("commit object is UTF-8 in these fixtures");
    let (headers, message) = text
        .split_once("\n\n")
        .expect("commit object has a header/message separator");

    let mut tree = String::new();
    let mut parents = Vec::new();
    let mut author = String::new();
    let mut committer = String::new();
    for line in headers.lines() {
        if let Some(v) = line.strip_prefix("tree ") {
            tree = v.to_string();
        } else if let Some(v) = line.strip_prefix("parent ") {
            parents.push(v.to_string());
        } else if let Some(v) = line.strip_prefix("author ") {
            author = strip_timestamp(v);
        } else if let Some(v) = line.strip_prefix("committer ") {
            committer = strip_timestamp(v);
        }
    }
    assert!(!tree.is_empty(), "tree header present");
    CommitObj {
        tree,
        parents,
        author,
        committer,
        message: message.to_string(),
    }
}

/// "Name <email> 1234567890 +0000" -> "Name <email>".
fn strip_timestamp(v: &str) -> String {
    // Drop the last two whitespace-separated fields (timestamp, timezone).
    let mut parts: Vec<&str> = v.split(' ').collect();
    assert!(parts.len() >= 3, "identity line too short: {v:?}");
    parts.truncate(parts.len() - 2);
    parts.join(" ")
}

fn assert_same_commit_fields(a: &Path, b: &Path) -> CommitObj {
    let ours = cat_file_head(a);
    let cli = cat_file_head(b);
    assert_eq!(ours, cli, "git2 commit (left) differs from CLI twin (right)");
    ours
}

// Scenario 1: normal commit on existing history.
#[test]
fn normal_commit_matches_cli() {
    require_git!();
    let (a, b) = twins(base_repo, |p| {
        std::fs::write(p.join("new.txt"), "new file\n").expect("write new.txt");
        git(p, &["add", "--", "new.txt"]);
    });

    let res = create_commit(a.path(), "add new file", None, false).expect("create_commit");
    git(b.path(), &["commit", "-m", "add new file"]);

    let obj = assert_same_commit_fields(a.path(), b.path());
    assert_eq!(obj.parents.len(), 1);
    assert_eq!(obj.message, "add new file\n");
    assert_eq!(obj.author, "Test User <test@example.com>");
    assert_eq!(obj.committer, "Test User <test@example.com>");

    assert_eq!(res.oid, git(a.path(), &["rev-parse", "HEAD"]));
    assert_eq!(res.summary, "add new file");
    assert_eq!(res.branch.as_deref(), Some("main"));
}

// Scenario 2: multi-line message — body preserved verbatim + one trailing \n.
#[test]
fn multiline_message_preserved() {
    require_git!();
    let msg = "subject line\n\nbody first line\nbody second line";
    let (a, b) = twins(base_repo, |p| {
        std::fs::write(p.join("new.txt"), "content\n").expect("write new.txt");
        git(p, &["add", "--", "new.txt"]);
    });

    let res = create_commit(a.path(), msg, None, false).expect("create_commit");
    git(b.path(), &["commit", "-m", msg]);

    let obj = assert_same_commit_fields(a.path(), b.path());
    assert_eq!(obj.message, format!("{msg}\n"));
    assert_eq!(res.summary, "subject line");
}

// Scenario 2b: message cleanup — trim + exactly one trailing newline.
#[test]
fn message_trimmed_with_single_trailing_newline() {
    require_git!();
    let a = base_repo();
    std::fs::write(a.path().join("new.txt"), "content\n").expect("write new.txt");
    git(a.path(), &["add", "--", "new.txt"]);

    let res = create_commit(a.path(), "   hello world  \n\n", None, false).expect("create_commit");
    assert_eq!(res.summary, "hello world");
    assert_eq!(cat_file_head(a.path()).message, "hello world\n");
}

// Scenario 3: unborn-HEAD first commit — branch created, no parents, clean status.
#[test]
fn unborn_first_commit() {
    require_git!();
    let (a, b) = twins(init_repo, |p| {
        std::fs::write(p.join("first.txt"), "first\n").expect("write first.txt");
        git(p, &["add", "--", "first.txt"]);
    });

    let res = create_commit(a.path(), "first commit", None, false).expect("create_commit");
    git(b.path(), &["commit", "-m", "first commit"]);

    let obj = assert_same_commit_fields(a.path(), b.path());
    assert!(obj.parents.is_empty(), "first commit has no parents");
    assert_eq!(res.branch.as_deref(), Some("main"));
    assert_eq!(git(a.path(), &["rev-parse", "--abbrev-ref", "HEAD"]), "main");

    let snapshot = read_status(a.path()).expect("read_status");
    assert!(snapshot.staged.is_empty());
    assert!(snapshot.unstaged.is_empty());
    assert!(snapshot.untracked.is_empty());
}

// Scenario 4: empty / whitespace-only message — no commit created.
#[test]
fn empty_message_rejected() {
    require_git!();
    let a = base_repo();
    std::fs::write(a.path().join("new.txt"), "content\n").expect("write new.txt");
    git(a.path(), &["add", "--", "new.txt"]);
    let head_before = git(a.path(), &["rev-parse", "HEAD"]);

    for msg in ["", "   ", " \n\t \n"] {
        let err = create_commit(a.path(), msg, None, false).expect_err("empty message must be rejected");
        assert!(matches!(err, AppError::EmptyMessage), "got: {err:?}");
    }
    assert_eq!(git(a.path(), &["rev-parse", "HEAD"]), head_before);

    // Unborn repo: still unborn afterwards.
    let unborn = init_repo();
    std::fs::write(unborn.path().join("f.txt"), "x\n").expect("write f.txt");
    git(unborn.path(), &["add", "--", "f.txt"]);
    let err = create_commit(unborn.path(), "  ", None, false).expect_err("empty message on unborn");
    assert!(matches!(err, AppError::EmptyMessage), "got: {err:?}");
    assert!(
        !git_ok(unborn.path(), &["rev-parse", "--verify", "HEAD"]),
        "HEAD must still be unborn"
    );
}

// Scenario 5: nothing staged — clean repo and unborn repo with empty index.
#[test]
fn nothing_to_commit_rejected() {
    require_git!();
    let clean = base_repo();
    let head_before = git(clean.path(), &["rev-parse", "HEAD"]);
    let err = create_commit(clean.path(), "no changes", None, false).expect_err("clean repo");
    assert!(matches!(err, AppError::NothingToCommit), "got: {err:?}");
    assert_eq!(git(clean.path(), &["rev-parse", "HEAD"]), head_before);

    let unborn = init_repo();
    let err = create_commit(unborn.path(), "nothing yet", None, false).expect_err("unborn empty index");
    assert!(matches!(err, AppError::NothingToCommit), "got: {err:?}");
    assert!(!git_ok(unborn.path(), &["rev-parse", "--verify", "HEAD"]));
}

// Scenario 6: detached HEAD — commit succeeds, branch = None, HEAD advances.
#[test]
fn detached_head_commit() {
    require_git!();
    let a = base_repo();
    let base_oid = git(a.path(), &["rev-parse", "HEAD"]);
    git(a.path(), &["checkout", "--detach"]);

    std::fs::write(a.path().join("tracked.txt"), "detached change\n").expect("modify");
    stage_paths(a.path(), &["tracked.txt".to_string()]).expect("stage_paths");

    let res = create_commit(a.path(), "detached commit", None, false).expect("create_commit");
    assert_eq!(res.branch, None);
    let new_head = git(a.path(), &["rev-parse", "HEAD"]);
    assert_ne!(new_head, base_oid, "HEAD must advance");
    assert_eq!(res.oid, new_head);
    assert_eq!(cat_file_head(a.path()).parents, vec![base_oid]);
}

// Scenario 7: stage -> create_commit -> read_status round-trip is clean.
#[test]
fn commit_then_status_clean() {
    require_git!();
    let a = base_repo();
    std::fs::write(a.path().join("feature.rs"), "fn main() {}\n").expect("write feature.rs");
    stage_paths(a.path(), &["feature.rs".to_string()]).expect("stage_paths");

    create_commit(a.path(), "add feature", None, false).expect("create_commit");

    let snapshot = read_status(a.path()).expect("read_status");
    assert!(snapshot.staged.is_empty());
    assert!(snapshot.unstaged.is_empty());
    assert!(snapshot.untracked.is_empty());
    assert!(snapshot.conflicted.is_empty());
}
