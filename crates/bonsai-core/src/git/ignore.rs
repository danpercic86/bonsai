//! P121d — gitignore membership counting for the watcher's dev-mode
//! measurement of "redundant" worktree refresh rounds.
//!
//! The watcher classes every path outside `.git` as `Worktree`, including
//! gitignored build output. Before deciding whether an all-ignored burst may be
//! dropped, we MEASURE how many of a fired burst's worktree paths are ignored.
//! Only the count leaves this module; the paths are never logged.

use std::path::{Path, PathBuf};

/// How many of `rel_paths` (workdir-relative) are gitignored. None if the repo
/// cannot be opened. Opens the repo per call (ignore rules may have changed).
/// Blocking; called on the watcher debounce thread, never on a tokio worker.
///
/// `NO_SEARCH`: `workdir` itself must be the repository — a directory that
/// merely sits under some other repo's worktree is not "a repo" here. A path
/// whose ignore lookup errors counts as NOT ignored (defensive, like
/// `health::stats`).
pub fn count_ignored(workdir: &Path, rel_paths: &[PathBuf]) -> Option<u32> {
    let repo = git2::Repository::open_ext(
        workdir,
        git2::RepositoryOpenFlags::NO_SEARCH,
        std::iter::empty::<&std::ffi::OsStr>(),
    )
    .ok()?;
    let mut ignored = 0u32;
    for rel in rel_paths {
        // libgit2 matches ignore rules against `/`-separated paths.
        let fwd = rel.to_string_lossy().replace('\\', "/");
        if repo.is_path_ignored(Path::new(&fwd)).unwrap_or(false) {
            ignored = ignored.saturating_add(1);
        }
    }
    Some(ignored)
}

#[cfg(test)]
mod tests {
    use super::*;

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
        assert_eq!(count_ignored(root, &rel), Some(2));
        assert_eq!(count_ignored(root, &[]), Some(0));
    }

    #[test]
    fn non_repo_dir_is_none() {
        let dir = tempfile::TempDir::new().expect("tempdir");
        assert_eq!(count_ignored(dir.path(), &[PathBuf::from("a.txt")]), None);
    }
}
