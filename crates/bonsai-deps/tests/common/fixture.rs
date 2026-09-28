//! git2-built fixture repos for the sentinels. The repo sits in a
//! subdirectory of a tempdir so "above the workdir" is test-owned.

use std::fs;
use std::path::{Path, PathBuf};

use git2::Repository;

pub struct FixtureRepo {
    _dir: tempfile::TempDir,
    pub parent: PathBuf,
    pub wd: PathBuf,
    repo: Repository,
}

impl FixtureRepo {
    pub fn new() -> Self {
        let dir = tempfile::tempdir().expect("tempdir");
        let parent = fs::canonicalize(dir.path()).expect("canon");
        let wd = parent.join("repo");
        let repo = Repository::init(&wd).expect("init");
        FixtureRepo {
            _dir: dir,
            parent,
            wd,
            repo,
        }
    }

    /// Writes a file WITHOUT tracking it.
    pub fn write(&self, rel: &str, body: &str) -> &Self {
        let p = self.wd.join(rel);
        fs::create_dir_all(p.parent().expect("parent")).expect("mkdir");
        fs::write(p, body).expect("write");
        self
    }

    /// Writes and stages a file.
    pub fn track(&self, rel: &str, body: &str) -> &Self {
        self.write(rel, body);
        let mut idx = self.repo.index().expect("index");
        idx.add_path(Path::new(rel)).expect("add");
        idx.write().expect("write index");
        self
    }
}

/// Creates a file symlink; false when the platform refuses.
pub fn try_symlink_file(target: &Path, link: &Path) -> bool {
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(target, link).is_ok()
    }
    #[cfg(windows)]
    {
        std::os::windows::fs::symlink_file(target, link).is_ok()
    }
}

pub const YARN_V1_ROOT: &str = r#"# yarn lockfile v1


left-pad@^1.3.0:
  version "1.3.0"
  resolved "https://registry.yarnpkg.com/left-pad/-/left-pad-1.3.0.tgz"

"@zzsentinel/scoped@^1.0.0":
  version "1.0.0"
  resolved "https://npm.corp.invalid/@zzsentinel/scoped/-/scoped-1.0.0.tgz"

zz-sentinel-trans@^1.0.0:
  version "1.0.0"
  resolved "https://npm.corp.invalid/zz-sentinel-trans/-/zz-sentinel-trans-1.0.0.tgz"
"#;

/// pnpm v9: no tarball URLs (the registry host is NOT in the lock).
pub const PNPM_V9_IGN: &str = "lockfileVersion: '9.0'

importers:

  .:
    dependencies:
      '@zzign/pkg':
        specifier: ^2.0.0
        version: 2.0.0
      is-odd:
        specifier: ^3.0.0
        version: 3.0.1

packages:

  '@zzign/pkg@2.0.0':
    resolution: {integrity: sha512-a}

  is-odd@3.0.1:
    resolution: {integrity: sha512-b}
";
