//! P121d / P124b — gitignore membership counting for the watcher.
//!
//! The watcher classes every path outside `.git` as `Worktree`, including
//! gitignored build output. P121d MEASURED how many of a fired burst's worktree
//! paths are ignored; P124b uses the same count to DROP a small burst whose
//! every path is ignored AND untracked. Only the counts leave this module; the
//! paths are never logged.

use std::path::{Path, PathBuf};

/// The ignore counts of one batch of workdir-relative paths.
///
/// `ignored` = matches an ignore rule; `ignored_untracked` = ignored AND absent
/// from the index at every stage 0..=3 AND no ancestor component is a gitlink
/// (mode 0o160000) index entry — a submodule-internal path never counts as
/// droppable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IgnoreCount {
    pub ignored: u32,
    pub ignored_untracked: u32,
}

/// The index mode of a gitlink (submodule) entry.
const GITLINK_MODE: u32 = 0o160_000;

/// Counts how many of `rel_paths` (workdir-relative) are gitignored, and how
/// many of those are also untracked (see [`IgnoreCount`]). None if the repo
/// cannot be opened or its index cannot be read. Opens the repo per call
/// (ignore rules — `.gitignore`, `info/exclude`, `core.excludesFile` — may have
/// changed). Blocking; called on the watcher debounce thread, never on a tokio
/// worker.
///
/// `NO_SEARCH`: `workdir` itself must be the repository — a directory that
/// merely sits under some other repo's worktree is not "a repo" here. An EMPTY
/// rel path (the workdir root) counts as NOT ignored. A path whose ignore
/// lookup errors counts as NOT ignored (defensive, like `health::stats`).
pub fn count_ignored_paths(workdir: &Path, rel_paths: &[PathBuf]) -> Option<IgnoreCount> {
    let repo = git2::Repository::open_ext(
        workdir,
        git2::RepositoryOpenFlags::NO_SEARCH,
        std::iter::empty::<&std::ffi::OsStr>(),
    )
    .ok()?;
    let index = repo.index().ok()?;
    let mut count = IgnoreCount {
        ignored: 0,
        ignored_untracked: 0,
    };
    for rel in rel_paths {
        // libgit2 matches ignore rules (and index paths) `/`-separated.
        let fwd = rel.to_string_lossy().replace('\\', "/");
        if fwd.is_empty() {
            continue;
        }
        if !repo.is_path_ignored(Path::new(&fwd)).unwrap_or(false) {
            continue;
        }
        count.ignored = count.ignored.saturating_add(1);
        if !is_tracked(&index, &fwd) {
            count.ignored_untracked = count.ignored_untracked.saturating_add(1);
        }
    }
    Some(count)
}

/// True when `fwd` has an index entry at any stage, when it is a DIRECTORY
/// with any tracked entry beneath it (a notify event for a renamed / removed
/// ignored dir lists only the dir name, not the tracked files inside), or when
/// any proper ancestor prefix of it is a gitlink entry (the path lives inside
/// a submodule).
fn is_tracked(index: &git2::Index, fwd: &str) -> bool {
    let p = Path::new(fwd);
    if (0..=3).any(|stage| index.get_path(p, stage).is_some()) {
        return true;
    }
    // `git_index_find_prefix`: Ok iff some entry's path starts with the prefix
    // (the index's own search order, so `core.ignorecase` follows libgit2).
    if index.find_prefix(format!("{fwd}/")).is_ok() {
        return true;
    }
    fwd.match_indices('/').any(|(i, _)| {
        index
            .get_path(Path::new(&fwd[..i]), 0)
            .is_some_and(|e| e.mode == GITLINK_MODE)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn count(root: &Path, rel: &[PathBuf]) -> Option<IgnoreCount> {
        count_ignored_paths(root, rel)
    }

    #[test]
    fn counts_ignored_paths_in_a_repo() {
        let dir = tempfile::TempDir::new().expect("tempdir");
        git2::Repository::init(dir.path()).expect("init");
        let root = dir.path();
        std::fs::write(root.join(".gitignore"), "dist/\n*.log\n").expect("gitignore");
        std::fs::create_dir_all(root.join("dist")).expect("dist");
        std::fs::create_dir_all(root.join("src")).expect("src");
        std::fs::write(root.join("dist").join("a.js"), "x").expect("a.js");
        std::fs::write(root.join("src").join("a.ts"), "x").expect("a.ts");
        std::fs::write(root.join("x.log"), "x").expect("x.log");

        let rel = [
            PathBuf::from("dist").join("a.js"),
            PathBuf::from("src").join("a.ts"),
            PathBuf::from("x.log"),
        ];
        let c = count(root, &rel).expect("repo");
        assert_eq!(c.ignored, 2);
        assert_eq!(c.ignored_untracked, 2);
        assert_eq!(
            count(root, &[]),
            Some(IgnoreCount {
                ignored: 0,
                ignored_untracked: 0
            })
        );
    }

    /// P124b: a force-added tracked file under an ignore rule and a path inside
    /// a gitlink (submodule) are ignored but NOT droppable; the empty rel path
    /// (workdir root) is not ignored at all.
    #[test]
    fn tracked_and_submodule_paths_are_not_ignored_untracked() {
        let dir = tempfile::TempDir::new().expect("tempdir");
        let repo = git2::Repository::init(dir.path()).expect("init");
        let root = dir.path();
        std::fs::write(root.join(".gitignore"), "*.log\ndist/\n").expect("gitignore");
        std::fs::write(root.join("keep.log"), "keep").expect("keep.log");
        std::fs::create_dir_all(root.join("dist")).expect("dist");
        std::fs::create_dir_all(root.join("src")).expect("src");
        std::fs::create_dir_all(root.join("sub")).expect("sub");
        std::fs::write(root.join("x.log"), "x").expect("x.log");
        std::fs::write(root.join("dist").join("a.js"), "x").expect("a.js");
        std::fs::write(root.join("src").join("a.ts"), "x").expect("a.ts");
        std::fs::write(root.join("sub").join("y.log"), "y").expect("y.log");

        let mut index = repo.index().expect("index");
        index
            .add_path(Path::new(".gitignore"))
            .expect("add .gitignore");
        // Force-add: `add_path` does not consult ignore rules.
        index.add_path(Path::new("keep.log")).expect("add keep.log");
        let blob = repo.blob(b"sub").expect("blob");
        index
            .add(&git2::IndexEntry {
                ctime: git2::IndexTime::new(0, 0),
                mtime: git2::IndexTime::new(0, 0),
                dev: 0,
                ino: 0,
                mode: GITLINK_MODE,
                uid: 0,
                gid: 0,
                file_size: 0,
                id: blob,
                flags: 0,
                flags_extended: 0,
                path: b"sub".to_vec(),
            })
            .expect("add gitlink");
        index.write().expect("write index");
        let tree = repo
            .find_tree(index.write_tree().expect("write tree"))
            .expect("tree");
        let sig = git2::Signature::now("t", "t@example.com").expect("sig");
        repo.commit(Some("HEAD"), &sig, &sig, "init", &tree, &[])
            .expect("commit");

        let rel = [
            PathBuf::from("keep.log"),
            PathBuf::from("x.log"),
            PathBuf::from("dist").join("a.js"),
            PathBuf::from("src").join("a.ts"),
            PathBuf::from("sub").join("y.log"),
            PathBuf::new(),
        ];
        let c = count(root, &rel).expect("repo");
        assert_eq!(c.ignored_untracked, 2, "only x.log and dist/a.js: {c:?}");
        assert!(
            c.ignored >= 3,
            "keep.log, x.log, dist/a.js are ignored: {c:?}"
        );
    }

    /// P124b S1: an ignored DIRECTORY path (as a dir rename / recursive delete
    /// event reports it) that contains a force-added tracked file is NOT
    /// droppable; an ignored dir with nothing tracked inside still is.
    #[test]
    fn ignored_dir_with_tracked_file_inside_is_tracked() {
        let dir = tempfile::TempDir::new().expect("tempdir");
        let repo = git2::Repository::init(dir.path()).expect("init");
        let root = dir.path();
        // No trailing slash: the rule matches the dir name itself too.
        std::fs::write(root.join(".gitignore"), "build\nout\n").expect("gitignore");
        std::fs::create_dir_all(root.join("build")).expect("build");
        std::fs::create_dir_all(root.join("out")).expect("out");
        std::fs::write(root.join("build").join("keep.txt"), "k").expect("keep");
        std::fs::write(root.join("out").join("junk.txt"), "j").expect("junk");
        let mut index = repo.index().expect("index");
        index
            .add_path(&PathBuf::from("build").join("keep.txt"))
            .expect("force-add");
        index.write().expect("write index");

        let c = count(root, &[PathBuf::from("build")]).expect("repo");
        assert_eq!(c.ignored, 1, "{c:?}");
        assert_eq!(c.ignored_untracked, 0, "tracked file beneath: {c:?}");
        let c = count(root, &[PathBuf::from("out")]).expect("repo");
        assert_eq!(c.ignored_untracked, 1, "nothing tracked beneath: {c:?}");
    }

    #[test]
    fn non_repo_dir_is_none() {
        let dir = tempfile::TempDir::new().expect("tempdir");
        assert_eq!(count(dir.path(), &[PathBuf::from("a.txt")]), None);
    }
}
