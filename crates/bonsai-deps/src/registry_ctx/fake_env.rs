//! `FakeHostEnv` — the test double for [`HostEnv`] (§4.4). Public (and
//! `#[doc(hidden)]`) so the integration sentinels in `tests/` can use it; it
//! performs no IO unless `above_from_disk_within` is set, and then only
//! below the given root (so a stray config on the real machine cannot
//! affect a test).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use crate::fsread::{read_above_workdir_config, read_config_follow, MAX_CONFIG_BYTES};

use super::host_env::{HostEnv, UserConfig, UserRead};

enum FakeFile {
    Content(String),
    Unreadable,
    /// `[Q9]` read from this real path with the following config read.
    Disk(PathBuf),
}

impl FakeFile {
    fn read(&self) -> UserRead {
        match self {
            FakeFile::Content(s) => UserRead::Content(s.clone()),
            FakeFile::Unreadable => UserRead::Unreadable,
            FakeFile::Disk(p) => {
                let (dir, name) = match (p.parent(), p.file_name()) {
                    (Some(d), Some(n)) => (d, n.to_string_lossy().into_owned()),
                    _ => return UserRead::Unreadable,
                };
                UserRead::from_outcome(read_config_follow(dir, &name, MAX_CONFIG_BYTES))
            }
        }
    }
}

#[doc(hidden)]
#[derive(Default)]
pub struct FakeHostEnv {
    vars: Vec<(String, String)>,
    user: HashMap<UserConfig, FakeFile>,
    /// Extra files of a set-valued config (`read_user_files`).
    user_extra: HashMap<UserConfig, Vec<FakeFile>>,
    above: HashMap<(PathBuf, String), FakeFile>,
    /// Serve above-workdir reads from disk for dirs under this root.
    above_from_disk: Option<PathBuf>,
    /// `[Q9-narrowed]` ancestors standing in for `$HOME` / `$CARGO_HOME`.
    follow_roots: Vec<PathBuf>,
    /// Names of the variables whose VALUE was handed out.
    values_read: Mutex<Vec<String>>,
}

impl FakeHostEnv {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn var(mut self, name: &str, value: &str) -> Self {
        self.vars.push((name.to_string(), value.to_string()));
        self
    }

    pub fn user_file(mut self, which: UserConfig, content: &str) -> Self {
        self.user
            .insert(which, FakeFile::Content(content.to_string()));
        self
    }

    /// `[Q9]` serve `which` from a real file (symlinks followed, as the
    /// production user-file read does).
    pub fn user_file_from_disk(mut self, which: UserConfig, path: &Path) -> Self {
        self.user.insert(which, FakeFile::Disk(path.to_path_buf()));
        self
    }

    /// `[P125c2]` one more file of a set-valued config (NuGet
    /// machine-wide), after the one `user_file` configured.
    pub fn user_file_also(mut self, which: UserConfig, content: &str) -> Self {
        self.user_extra
            .entry(which)
            .or_default()
            .push(FakeFile::Content(content.to_string()));
        self
    }

    pub fn user_unreadable(mut self, which: UserConfig) -> Self {
        self.user.insert(which, FakeFile::Unreadable);
        self
    }

    pub fn above(mut self, dir: &Path, name: &str, content: &str) -> Self {
        self.above.insert(
            (dir.to_path_buf(), name.to_string()),
            FakeFile::Content(content.to_string()),
        );
        self
    }

    pub fn above_unreadable(mut self, dir: &Path, name: &str) -> Self {
        self.above
            .insert((dir.to_path_buf(), name.to_string()), FakeFile::Unreadable);
        self
    }

    /// Serve `read_above_workdir` from the real filesystem (the production
    /// `[Q9-narrowed]` read: no-follow unless the ancestor is a follow root)
    /// for unconfigured paths at or below `root`; anything above `root`
    /// reads as Absent (hermetic).
    pub fn above_from_disk_within(mut self, root: &Path) -> Self {
        self.above_from_disk = Some(root.to_path_buf());
        self
    }

    /// `[Q9-narrowed]` treat `dir` as `$HOME` / `$CARGO_HOME`: symlinks in
    /// above-workdir entries of exactly this ancestor are followed.
    pub fn follow_root(mut self, dir: &Path) -> Self {
        self.follow_roots.push(dir.to_path_buf());
        self
    }

    /// `[NEW-2]` the variable names whose values were materialised.
    pub fn values_read(&self) -> Vec<String> {
        self.values_read
            .lock()
            .map(|v| v.clone())
            .unwrap_or_default()
    }

    fn note(&self, name: &str) {
        if let Ok(mut v) = self.values_read.lock() {
            v.push(name.to_string());
        }
    }
}

impl HostEnv for FakeHostEnv {
    fn var(&self, name: &str) -> Option<String> {
        let hit = self
            .vars
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.clone());
        if hit.is_some() {
            self.note(name);
        }
        hit
    }

    fn var_present_ci(&self, name: &str) -> bool {
        // Presence only: never `note()`d, the value is not touched.
        self.vars.iter().any(|(k, _)| k.eq_ignore_ascii_case(name))
    }

    fn vars_with_prefix_ci(&self, prefix: &str, keep: fn(&str) -> bool) -> Vec<(String, String)> {
        let mut out = Vec::new();
        for (k, v) in &self.vars {
            let prefixed = k.len() >= prefix.len()
                && k.is_char_boundary(prefix.len())
                && k[..prefix.len()].eq_ignore_ascii_case(prefix);
            // The key filter runs BEFORE the value is touched.
            if prefixed && keep(k) {
                self.note(k);
                out.push((k.clone(), v.clone()));
            }
        }
        out
    }

    fn read_user_file(&self, which: UserConfig) -> UserRead {
        self.user
            .get(&which)
            .map_or(UserRead::Absent, FakeFile::read)
    }

    fn read_user_files(&self, which: UserConfig) -> Vec<UserRead> {
        let mut out = vec![self.read_user_file(which)];
        out.extend(
            self.user_extra
                .get(&which)
                .into_iter()
                .flatten()
                .map(FakeFile::read),
        );
        out
    }

    fn read_above_workdir(&self, dir: &Path, file_name: &str) -> UserRead {
        if let Some(f) = self.above.get(&(dir.to_path_buf(), file_name.to_string())) {
            return f.read();
        }
        if !self
            .above_from_disk
            .as_ref()
            .is_some_and(|r| dir.starts_with(r))
        {
            return UserRead::Absent;
        }
        UserRead::from_outcome(read_above_workdir_config(
            dir,
            file_name,
            MAX_CONFIG_BYTES,
            &self.follow_roots,
        ))
    }
}
