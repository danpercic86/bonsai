//! The REAL `HostEnv` (contract §4.4, E3; §10 P125e row). Everything
//! `RegistryContext::load` reads from outside the repo comes through here.
//!
//! Environment `[P125b review]`: every lookup iterates `std::env::vars_os()`
//! and keeps ONLY the entry it asked for. `vars_os()` itself takes a
//! transient std-owned snapshot of the whole environment (it copies every
//! entry when called); this module retains none of it — every non-matching
//! entry is dropped as the iterator advances and nothing is stored in
//! `RealHostEnv`.
//! `var_present_ci` looks at KEYS only; `vars_with_prefix_ci` runs the
//! caller's key filter before a value is kept (`[NEW-2]`).
//!
//! Files: user / system configs are located by `host_paths.rs` and read with
//! `fsread::read_config_follow` (Q9: symlinks followed, network / UNC /
//! device targets refused unopened, regular file after open, `take` bounded);
//! the above-workdir walk goes through `fsread::read_above_workdir_config`
//! with the canonical `$HOME` / `$CARGO_HOME` as the only follow roots
//! (Q9 NARROWED). Every outcome maps through `UserRead::from_outcome`.

use std::ffi::{OsStr, OsString};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use bonsai_deps::fsread::{
    is_network_path, read_above_workdir_config, read_config_follow, resolve_config_dir,
    ReadOutcome, MAX_CONFIG_BYTES,
};
use bonsai_deps::registry_ctx::{HostEnv, UserConfig, UserRead};

use super::host_paths::{locate, network_profile, Inputs, Located, Os};

/// More NuGet machine-wide `*.config` files than this ⇒ fail closed.
pub(crate) const MAX_MACHINE_CONFIGS: usize = 64;

#[derive(Default)]
pub struct RealHostEnv {
    /// `[Q9-narrowed]` canonical `$HOME` / `$CARGO_HOME`, resolved once.
    follow_roots: OnceLock<Vec<PathBuf>>,
    /// The `PATH` directory holding `node`, resolved once (ASSUMED, E3).
    node_dir: OnceLock<Option<PathBuf>>,
}

impl RealHostEnv {
    pub fn new() -> Self {
        Self::default()
    }

    fn with_inputs<R>(&self, f: impl FnOnce(&Inputs) -> R) -> R {
        let var = |k: &str| env_value(k).filter(|v| !v.is_empty());
        let node = || self.node_dir.get_or_init(find_node_dir).clone();
        f(&Inputs {
            os: Os::current(),
            var: &var,
            node_dir: &node,
        })
    }

    pub(crate) fn follow_roots(&self) -> &[PathBuf] {
        self.follow_roots.get_or_init(|| {
            let bases = self.with_inputs(|i| [i.home(), i.cargo_home()]);
            canonical_roots(bases.into_iter().flatten())
        })
    }
}

/// `[P125c1 review]` canonicalized EXACTLY like the workdir ancestors
/// (`fs::canonicalize`, i.e. `\\?\C:\…` on Windows), else the `dir ==
/// root` comparison in `read_above_workdir_config` never matches and Q9
/// silently never follows. Each base is vetted by `resolve_config_dir`
/// first, so `canonicalize` never walks a network / UNC link.
pub(crate) fn canonical_roots(bases: impl Iterator<Item = PathBuf>) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = Vec::new();
    for b in bases {
        if resolve_config_dir(&b).is_err() {
            continue;
        }
        if let Ok(c) = fs::canonicalize(&b) {
            if !out.contains(&c) {
                out.push(c);
            }
        }
    }
    out
}

fn key_is(k: &OsStr, name: &str) -> bool {
    k.to_string_lossy().eq_ignore_ascii_case(name)
}

fn lossy(v: OsString) -> String {
    v.into_string()
        .unwrap_or_else(|v| v.to_string_lossy().into_owned())
}

/// The value of `name`: an exact-case key first, else the first
/// ASCII-case-insensitive one. No other value is retained.
pub(crate) fn env_value(name: &str) -> Option<OsString> {
    let mut ci = None;
    for (k, v) in std::env::vars_os() {
        if k.as_os_str() == OsStr::new(name) {
            return Some(v);
        }
        if ci.is_none() && key_is(&k, name) {
            ci = Some(v);
        }
    }
    ci
}

/// ASSUMED (E3): the first local `PATH` directory holding a `node`
/// executable. Each entry is vetted by `resolve_config_dir` (no network
/// hop) before anything inside it is stat'ed; `node` itself is not
/// followed. No process is started.
fn find_node_dir() -> Option<PathBuf> {
    let exe = if cfg!(windows) { "node.exe" } else { "node" };
    let path = env_value("PATH")?;
    std::env::split_paths(&path)
        .filter(|d| d.is_absolute() && !is_network_path(&d.to_string_lossy()))
        .filter_map(|d| resolve_config_dir(&d).ok())
        .find(|d| {
            fs::symlink_metadata(d.join(exe))
                .is_ok_and(|md| md.is_file() || md.file_type().is_symlink())
        })
}

/// One following config read of an absolute file path.
pub(crate) fn read_one(p: &Path) -> UserRead {
    match (p.parent(), p.file_name().and_then(OsStr::to_str)) {
        (Some(dir), Some(name)) => {
            UserRead::from_outcome(read_config_follow(dir, name, MAX_CONFIG_BYTES))
        }
        _ => UserRead::Unreadable,
    }
}

/// Folds the candidates of one kind: any Unreadable ⇒ Unreadable; all
/// Absent ⇒ Absent; else the present texts joined by a newline. Only pip's
/// INI kinds have more than one candidate, and their key-filtered extractor
/// is additive (each kept value can only add a private index), so the join
/// reads exactly the signals of the separate files.
pub(crate) fn fold(paths: &[PathBuf]) -> UserRead {
    let mut texts = Vec::new();
    for p in paths {
        match read_one(p) {
            UserRead::Absent => {}
            UserRead::Unreadable => return UserRead::Unreadable,
            UserRead::Content(s) => texts.push(s),
        }
    }
    match texts.len() {
        0 => UserRead::Absent,
        1 => texts.pop().map_or(UserRead::Absent, UserRead::Content),
        _ => UserRead::Content(texts.join("\n")),
    }
}

/// `[P125c2 review]` the NuGet machine-wide set: every `*.config` in `dir`
/// (ASCII case-insensitive, sorted), each read like a user file.
pub(crate) fn read_config_dir(dir: &Path) -> Vec<UserRead> {
    let dir = match resolve_config_dir(dir) {
        Ok(d) => d,
        Err(ReadOutcome::Absent) => return Vec::new(),
        Err(_) => return vec![UserRead::Unreadable],
    };
    let Ok(entries) = fs::read_dir(&dir) else {
        return vec![UserRead::Unreadable];
    };
    let mut names = Vec::new();
    for e in entries {
        let Ok(e) = e else {
            return vec![UserRead::Unreadable];
        };
        let raw = e.file_name();
        if !raw
            .to_string_lossy()
            .to_ascii_lowercase()
            .ends_with(".config")
        {
            continue;
        }
        let Some(name) = raw.to_str() else {
            return vec![UserRead::Unreadable];
        };
        names.push(name.to_owned());
        if names.len() > MAX_MACHINE_CONFIGS {
            return vec![UserRead::Unreadable];
        }
    }
    names.sort();
    names
        .iter()
        .map(|n| UserRead::from_outcome(read_config_follow(&dir, n, MAX_CONFIG_BYTES)))
        .collect()
}

impl HostEnv for RealHostEnv {
    fn var(&self, name: &str) -> Option<String> {
        env_value(name).map(lossy)
    }

    fn var_present_ci(&self, name: &str) -> bool {
        // Keys only: never `var_os(name)`, which would copy the value.
        std::env::vars_os().any(|(k, _)| key_is(&k, name))
    }

    fn vars_with_prefix_ci(&self, prefix: &str, keep: fn(&str) -> bool) -> Vec<(String, String)> {
        let mut out = Vec::new();
        for (k, v) in std::env::vars_os() {
            let key = k.to_string_lossy();
            let prefixed = key.len() >= prefix.len()
                && key.is_char_boundary(prefix.len())
                && key[..prefix.len()].eq_ignore_ascii_case(prefix);
            // The key filter runs BEFORE the value is kept.
            if prefixed && keep(&key) {
                out.push((key.into_owned(), lossy(v)));
            }
        }
        out
    }

    fn read_user_file(&self, which: UserConfig) -> UserRead {
        match self.with_inputs(|i| locate(which, i)) {
            Located::Absent => UserRead::Absent,
            Located::Unresolvable => UserRead::Unreadable,
            Located::Files(ps) => fold(&ps),
            // A set is only ever read through `read_user_files` by the
            // loaders; the single-read form never silently picks one file.
            Located::ConfigDir(d) => {
                if read_config_dir(&d)
                    .iter()
                    .all(|r| matches!(r, UserRead::Absent))
                {
                    UserRead::Absent
                } else {
                    UserRead::Unreadable
                }
            }
        }
    }

    fn read_user_files(&self, which: UserConfig) -> Vec<UserRead> {
        match self.with_inputs(|i| locate(which, i)) {
            Located::Absent => Vec::new(),
            Located::Unresolvable => vec![UserRead::Unreadable],
            Located::Files(ps) => vec![fold(&ps)],
            Located::ConfigDir(d) => read_config_dir(&d),
        }
    }

    fn read_above_workdir(&self, dir: &Path, file_name: &str) -> UserRead {
        UserRead::from_outcome(read_above_workdir_config(
            dir,
            file_name,
            MAX_CONFIG_BYTES,
            self.follow_roots(),
        ))
    }

    fn profile_on_network(&self) -> bool {
        // `[P125h §4b]` text only: the `node` PATH probe (a stat) is NOT
        // run here — its dir is never a network path anyway (`find_node_dir`
        // skips those), and the npm global default is then `/usr/local`.
        let var = |k: &str| env_value(k).filter(|v| !v.is_empty());
        let no_node = || None;
        network_profile(&Inputs {
            os: Os::current(),
            var: &var,
            node_dir: &no_node,
        })
    }
}
