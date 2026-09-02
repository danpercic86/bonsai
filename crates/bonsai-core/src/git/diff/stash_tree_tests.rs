//! A stash's file list must show the brand-new (A) files it captured, not just
//! the modified ones. Those blobs live in the stash commit's THIRD parent, so a
//! plain commit-vs-first-parent diff omits them entirely (see `stash_tree`).

use std::path::Path;

use crate::git::diff::{commit_diff, commit_file_diff};
use crate::git::stash::{create_stash, list_stashes, StashScope};

fn st_init(dir: &Path) -> git2::Repository {
    let repo =
        git2::Repository::init_opts(dir, git2::RepositoryInitOptions::new().initial_head("main"))
            .expect("init repo");
    let mut cfg = repo.config().expect("config");
    cfg.set_str("user.name", "Test User").expect("name");
    cfg.set_str("user.email", "test@example.com").expect("email");
    cfg.set_bool("core.autocrlf", false).expect("autocrlf");
    drop(cfg);
    repo
}

fn st_commit(dir: &Path, msg: &str, files: &[(&str, &str)]) {
    for (name, content) in files {
        std::fs::write(dir.join(name), content).expect("write file");
    }
    crate::git::stage::stage_paths(
        dir,
        &files.iter().map(|(n, _)| n.to_string()).collect::<Vec<_>>(),
    )
    .expect("stage");
    crate::git::commit::create_commit(dir, msg, None, false).expect("commit");
}

#[test]
fn stash_diff_lists_untracked_files_as_adds() {
    let td = tempfile::tempdir().expect("tempdir");
    let dir = td.path();
    st_init(dir);
    st_commit(dir, "base", &[("tracked.txt", "one\n")]);

    std::fs::write(dir.join("tracked.txt"), "edited\n").expect("modify");
    std::fs::write(dir.join("brand-new.txt"), "hello\n").expect("new file");
    std::fs::create_dir_all(dir.join("nested")).expect("mkdir");
    std::fs::write(dir.join("nested/also-new.txt"), "nested\n").expect("nested new");

    assert!(
        create_stash(dir, None, StashScope::AllWithUntracked)
            .expect("stash")
            .created
    );

    let entry = list_stashes(dir).expect("list").remove(0);
    let diff = commit_diff(dir, &entry.oid).expect("stash diff");
    let mut paths: Vec<&str> = diff.files.iter().map(|f| f.path.as_str()).collect();
    paths.sort();
    assert_eq!(paths, ["brand-new.txt", "nested/also-new.txt", "tracked.txt"]);

    let added = diff
        .files
        .iter()
        .find(|f| f.path == "brand-new.txt")
        .expect("added file header");
    assert_eq!(added.status, crate::git::status::FileStatus::Added);

    // The per-file diff must resolve through the SAME overlaid tree, otherwise
    // clicking the row errors with "path not changed in commit".
    let fd = commit_file_diff(dir, &entry.oid, "brand-new.txt", None, false, false)
        .expect("file diff for a stashed new file");
    assert!(!fd.hunks.is_empty(), "the new file's content should render");
}

#[test]
fn ordinary_commit_diff_is_unchanged() {
    let td = tempfile::tempdir().expect("tempdir");
    let dir = td.path();
    st_init(dir);
    st_commit(dir, "base", &[("a.txt", "one\n")]);
    st_commit(dir, "second", &[("a.txt", "two\n"), ("b.txt", "new\n")]);

    let head = git2::Repository::open(dir)
        .expect("open")
        .head()
        .expect("head")
        .peel_to_commit()
        .expect("commit")
        .id()
        .to_string();
    let diff = commit_diff(dir, &head).expect("commit diff");
    let mut paths: Vec<&str> = diff.files.iter().map(|f| f.path.as_str()).collect();
    paths.sort();
    assert_eq!(paths, ["a.txt", "b.txt"]);
}

/// A stash created WITHOUT untracked files has only two parents — no overlay,
/// and the tracked changes still list normally.
#[test]
fn tracked_only_stash_diff_lists_tracked_changes() {
    let td = tempfile::tempdir().expect("tempdir");
    let dir = td.path();
    st_init(dir);
    st_commit(dir, "base", &[("tracked.txt", "one\n")]);
    std::fs::write(dir.join("tracked.txt"), "edited\n").expect("modify");
    std::fs::write(dir.join("left-behind.txt"), "untracked\n").expect("new file");

    assert!(create_stash(dir, None, StashScope::All).expect("stash").created);

    let entry = list_stashes(dir).expect("list").remove(0);
    let diff = commit_diff(dir, &entry.oid).expect("stash diff");
    let paths: Vec<&str> = diff.files.iter().map(|f| f.path.as_str()).collect();
    assert_eq!(paths, ["tracked.txt"]);
    // Scope `all` leaves untracked files in the worktree — they are not in the
    // stash, so they must not appear in its diff either.
    assert!(dir.join("left-behind.txt").exists());
}
