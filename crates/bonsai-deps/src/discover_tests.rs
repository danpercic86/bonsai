//! Discovery tests (§4.1, E4 `[MEDIUM-3]` / `[LOW-3]` / AC9).

use std::fs;
use std::path::Path;

use git2::{IndexEntry, IndexTime, Oid, Repository};

use super::*;

struct Fixture {
    _dir: tempfile::TempDir,
    parent: std::path::PathBuf,
    repo: Repository,
}

impl Fixture {
    fn wd(&self) -> std::path::PathBuf {
        workdir_canon(&self.repo).unwrap()
    }

    fn write(&self, rel: &str, body: &str) {
        let p = self.wd().join(rel);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, body).unwrap();
    }

    fn track(&self, rel: &str) {
        let mut idx = self.repo.index().unwrap();
        idx.add_path(Path::new(rel)).unwrap();
        idx.write().unwrap();
    }

    /// Adds an index entry with an explicit mode (symlink / gitlink).
    fn track_mode(&self, rel: &str, mode: u32) {
        let mut idx = self.repo.index().unwrap();
        let id = if mode == MODE_GITLINK {
            Oid::from_str("0123456789abcdef0123456789abcdef01234567").unwrap()
        } else {
            self.repo.blob(b"target").unwrap()
        };
        let t = IndexTime::new(0, 0);
        idx.add(&IndexEntry {
            ctime: t,
            mtime: t,
            dev: 0,
            ino: 0,
            mode,
            uid: 0,
            gid: 0,
            file_size: 0,
            id,
            flags: 0,
            flags_extended: 0,
            path: rel.as_bytes().to_vec(),
        })
        .unwrap();
        idx.write().unwrap();
    }
}

/// The repo lives in `<tmp>/repo`, so `<tmp>` is "above the workdir".
fn fixture() -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let parent = fs::canonicalize(dir.path()).unwrap();
    let repo = Repository::init(parent.join("repo")).unwrap();
    Fixture {
        _dir: dir,
        parent,
        repo,
    }
}

#[test]
fn finds_tracked_and_untracked_but_not_ignored_or_vendored() {
    let f = fixture();
    f.write("package.json", "{}");
    f.write("pnpm-lock.yaml", "lockfileVersion: '9.0'\n");
    f.track("package.json");
    f.write("apps/web/package.json", "{}");
    f.write("node_modules/x/package.json", "{}");
    f.write("vendor/y/package.json", "{}");
    f.write(".gitignore", "ignored/\n");
    f.write("ignored/package.json", "{}");
    f.write("android/build.gradle", "");
    f.write("README.md", "");
    let set = discover_full(&f.repo).unwrap();
    assert_eq!(set.manifests(), ["apps/web/package.json", "package.json"]);
    assert_eq!(set.lockfiles(), ["pnpm-lock.yaml"]);
    assert_eq!(set.not_tracked(), ["android/build.gradle"]);
    assert_eq!(set.overflow, 0);
}

#[test]
fn index_symlinks_and_gitlinks_are_dropped() {
    let f = fixture();
    f.track_mode("linked/package.json", MODE_SYMLINK);
    f.track_mode("sub", MODE_GITLINK);
    f.write("package.json", "{}");
    f.track("package.json");
    let set = discover_full(&f.repo).unwrap();
    assert_eq!(set.manifests(), ["package.json"]);
}

#[test]
fn unclean_and_skipped_paths_are_dropped() {
    let set = FileSet::from_paths(
        [
            "../package.json",
            "/abs/package.json",
            "C:/x/package.json",
            "a/./package.json",
            "a//package.json",
            "a\\package.json",
            "a/node_modules/b/package.json",
            "a/.git/package.json",
            "a/target/package.json",
            "ok/package.json",
        ]
        .map(String::from),
    );
    assert_eq!(set.manifests(), ["ok/package.json"]);
}

#[test]
fn the_manifest_cap_counts_overflow() {
    let paths = (0..MAX_MANIFESTS + 3).map(|i| format!("p{i:05}/package.json"));
    let set = FileSet::from_paths(paths);
    assert_eq!(set.manifests().len(), MAX_MANIFESTS);
    assert_eq!(set.overflow, 3);
    // Sorted: the first MAX_MANIFESTS survive.
    assert_eq!(set.manifests()[0], "p00000/package.json");
}

#[test]
fn a_parent_dir_manifest_is_never_discovered() {
    // [Q7] configs may be read above the workdir; manifests never are.
    let f = fixture();
    fs::write(f.parent.join("package.json"), "{}").unwrap();
    fs::write(f.parent.join("Cargo.toml"), "").unwrap();
    f.write("package.json", "{}");
    let set = discover_full(&f.repo).unwrap();
    assert_eq!(set.manifests(), ["package.json"]);
}

#[test]
fn bare_repos_are_an_error() {
    let dir = tempfile::tempdir().unwrap();
    let bare = Repository::init_bare(dir.path()).unwrap();
    assert_eq!(discover_full(&bare), Err(DiscoverError::Bare));
    assert!(probe_stale(&bare, &[]));
}

#[test]
fn probe_stale_detects_edits_and_new_index_entries() {
    let f = fixture();
    f.write("package.json", "{}");
    f.track("package.json");
    let set = discover_full(&f.repo).unwrap();
    let st = stamps(&f.wd(), &set);
    assert_eq!(st.len(), 1);
    assert!(!probe_stale(&f.repo, &st));

    // An edit changes the length.
    f.write("package.json", "{\"a\":1}");
    assert!(probe_stale(&f.repo, &st));
    let st = stamps(&f.wd(), &discover_full(&f.repo).unwrap());
    assert!(!probe_stale(&f.repo, &st));

    // A newly tracked manifest is stale; a new untracked one is not (no
    // untracked walk on this path).
    f.write("b/package.json", "{}");
    assert!(!probe_stale(&f.repo, &st));
    f.track("b/package.json");
    assert!(probe_stale(&f.repo, &st));
}

#[test]
fn probe_stale_settles_for_a_deleted_tracked_manifest() {
    let f = fixture();
    f.write("package.json", "{}");
    f.track("package.json");
    fs::remove_file(f.wd().join("package.json")).unwrap();
    let st = stamps(&f.wd(), &discover_full(&f.repo).unwrap());
    assert_eq!(st.len(), 1);
    assert!(!probe_stale(&f.repo, &st));
    f.write("package.json", "{}");
    assert!(probe_stale(&f.repo, &st));
}

#[test]
fn the_not_tracked_cap_counts_into_overflow() {
    let paths = (0..MAX_MANIFESTS + 2).map(|i| format!("g{i:05}/pom.xml"));
    let set = FileSet::from_paths(paths);
    assert_eq!(set.not_tracked().len(), MAX_MANIFESTS);
    assert_eq!(set.overflow, 2);
}
