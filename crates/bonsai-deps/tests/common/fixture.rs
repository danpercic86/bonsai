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

/// Cargo sentinels (E4 row 6): git, named registry, patched, a private
/// transitive lock entry, a workspace member inheriting a dep.
pub const CARGO_ROOT: &str = r#"
[package]
name = "cargo-app"
version = "0.1.0"

[workspace]
members = ["crates/zz-sentinel-member"]

[workspace.dependencies]
serde = "1"

[dependencies]
serde = { workspace = true }
itoa = "1"
zz-sentinel-above = "0.1"
zz_sentinel_git = { git = "https://git.corp.invalid/zz.git" }
zz-sentinel-reg = { version = "0.3", registry = "corp" }
zz-sentinel-patched = "0.1"
zz-sentinel-member = { path = "crates/zz-sentinel-member" }

[patch.crates-io]
zz-sentinel-patched = { path = "../zz-sentinel-patched" }
"#;

pub const CARGO_MEMBER: &str = r#"
[package]
name = "zz-sentinel-member"
version = "0.1.0"

[dependencies]
serde = { workspace = true }
zz-sentinel-patched = "0.1"
"#;

pub const CARGO_LOCK: &str = r#"
version = 4

[[package]]
name = "itoa"
version = "1.0.11"
source = "registry+https://github.com/rust-lang/crates.io-index"

[[package]]
name = "serde"
version = "1.0.203"
source = "sparse+https://index.crates.io/"

[[package]]
name = "zz-sentinel-above"
version = "0.1.0"
source = "registry+https://github.com/rust-lang/crates.io-index"

[[package]]
name = "zz-sentinel-member"
version = "0.1.0"

[[package]]
name = "zz-sentinel-patched"
version = "0.1.0"

[[package]]
name = "zz_sentinel_git"
version = "0.1.0"
source = "git+https://git.corp.invalid/zz.git#0123abc"

[[package]]
name = "zz-sentinel-trans-cargo"
version = "0.2.0"
source = "registry+https://cargo.corp.invalid/index"
"#;

/// Go sentinels: a GOPRIVATE module and a local replacement.
pub const GO_MOD: &str = "module example.com/app

go 1.22

require (
\tgithub.com/pkg/errors v0.9.1
\tcorp.invalid/zzsentinel/mod v1.0.0
\texample.com/zz-sentinel-local v0.0.0-00010101000000-000000000000
\tgolang.org/x/text v0.14.0 // indirect
)

replace example.com/zz-sentinel-local => ./local
";

/// `[Q7]` E4 row 6 (f): a parent-dir `.cargo/config.toml` with a
/// credentialed replacement registry.
pub const Q7_CARGO_CONFIG: &str = r#"[source.crates-io]
replace-with = "corp"

[source.corp]
registry = "https://u:zz-sentinel-token@cargo.corp.invalid/index"
"#;
