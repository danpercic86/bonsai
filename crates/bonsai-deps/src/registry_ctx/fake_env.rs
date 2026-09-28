//! `FakeHostEnv` — the test double for [`HostEnv`] (§4.4). Public (and
//! `#[doc(hidden)]`) so the integration sentinels in `tests/` can use it; it
//! performs no IO unless `above_from_disk_within` is set, and then only
//! below the given root (so a stray config on the real machine cannot
//! affect a test).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use crate::fsread::{read_nofollow, ReadOutcome, MAX_CONFIG_BYTES};

use super::host_env::{HostEnv, UserConfig, UserRead};

enum FakeFile {
    Content(String),
    Unreadable,
}

impl FakeFile {
    fn read(&self) -> UserRead {
        match self {
            FakeFile::Content(s) => UserRead::Content(s.clone()),
            FakeFile::Unreadable => UserRead::Unreadable,
        }
    }
}

#[doc(hidden)]
#[derive(Default)]
pub struct FakeHostEnv {
    vars: Vec<(String, String)>,
    user: HashMap<UserConfig, FakeFile>,
    above: HashMap<(PathBuf, String), FakeFile>,
    /// Serve above-workdir reads from disk for dirs under this root.
    above_from_disk: Option<PathBuf>,
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

    /// Serve `read_above_workdir` from the real filesystem (no-follow,
    /// regular files only, 1 MiB cap) for unconfigured paths at or below
    /// `root`; anything above `root` reads as Absent (hermetic).
    pub fn above_from_disk_within(mut self, root: &Path) -> Self {
        self.above_from_disk = Some(root.to_path_buf());
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
        match read_nofollow(&dir.join(file_name), MAX_CONFIG_BYTES) {
            ReadOutcome::Absent => UserRead::Absent,
            ReadOutcome::Content(s) => UserRead::Content(s),
            ReadOutcome::NotAFile | ReadOutcome::TooLarge | ReadOutcome::Unreadable => {
                UserRead::Unreadable
            }
        }
    }
}
