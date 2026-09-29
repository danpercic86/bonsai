//! The NuGet half of `RegistryContext::load` (§4.4, E3 NuGet row). Same
//! fail-closed shape as the other loaders: every step is `Result<_, Closed>`
//! and the context is opened only after the whole function returned `Ok`.
//!
//! Files (the config walk VERIFIED on Windows with the .NET 10 SDK
//! `dotnet nuget config paths`: every ancestor of the project dir to the
//! drive root, then `%APPDATA%\NuGet\NuGet.Config`):
//! - repo walk, each NuGet manifest dir → workdir: `nuget.config`,
//!   `NuGet.Config`, `NuGet.config` (all three: on a case-sensitive file
//!   system they are distinct files), plus `RestoreSources` /
//!   `RestoreAdditionalProjectSources` in `Directory.Build.props` and in
//!   each project file;
//! - `[Q7]` every ancestor above the workdir to the filesystem root: the
//!   same three config names, via `HostEnv::read_above_workdir`
//!   (`[Q9-narrowed]` symlinks followed only in `$HOME`), and the
//!   RestoreSources of `Directory.Build.props` there `[P125c2 review]`;
//! - the user `NuGet.Config` and the machine-wide `*.config` set.
//!
//! Chains and precedence: `nuget_chain.rs`.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use crate::adapter::{dir_and_ancestors, file_name, join, parent_dir};
use crate::eco::nuget::{is_nuget_manifest, is_project_file, BUILD_PROPS};
use crate::fsread::{path_is_clean, safe_read, ReadOutcome, MAX_CONFIG_BYTES, MAX_FILE_BYTES};

use super::host_env::{HostEnv, UserConfig, UserRead};
use super::nuget_cfg::{nuget_config, restore_sources, FeedRef, NugetFile};
use super::nuget_chain::{resolve, Chain, Level};
use super::{Closed, EcoRegistry};

/// NuGet's `NuGet.Config` file names, in its probe order.
pub(crate) const CONFIG_NAMES: &[&str] = &["nuget.config", "NuGet.config", "NuGet.Config"];

fn from_user_read(r: UserRead, files: &mut Vec<NugetFile>) -> Result<(), Closed> {
    match r {
        UserRead::Absent => Ok(()),
        UserRead::Content(text) => {
            files.push(nuget_config(&text)?);
            Ok(())
        }
        UserRead::Unreadable => Err(Closed),
    }
}

/// A repo file: Absent ⇒ None; anything but Content ⇒ `Closed`.
fn read_repo(workdir_canon: &Path, rel: &str, cap: u64) -> Result<Option<String>, Closed> {
    match safe_read(workdir_canon, rel, cap) {
        ReadOutcome::Absent => Ok(None),
        ReadOutcome::Content(t) => Ok(Some(t)),
        // [HIGH-1 item 2] symlink, FIFO, directory, oversize, EACCES.
        ReadOutcome::NotAFile | ReadOutcome::TooLarge | ReadOutcome::Unreadable => Err(Closed),
    }
}

fn repo_level(workdir_canon: &Path, dir: &str) -> Result<Level, Closed> {
    let mut level = Level::default();
    for name in CONFIG_NAMES {
        if let Some(text) = read_repo(workdir_canon, &join(dir, name), MAX_CONFIG_BYTES)? {
            level.files.push(nuget_config(&text)?);
        }
    }
    Ok(level)
}

/// `[Q7]` the config levels above the workdir, plus `[P125c2 review]` the
/// RestoreSources feeds of every `Directory.Build.props` there (a config
/// read only — never a manifest above the workdir; same read rules).
fn above_levels(
    workdir_canon: &Path,
    env: &dyn HostEnv,
) -> Result<(Vec<Level>, Vec<FeedRef>), Closed> {
    let mut out = Vec::new();
    let mut restore = Vec::new();
    for dir in workdir_canon.ancestors().skip(1) {
        let mut level = Level::default();
        for name in CONFIG_NAMES {
            from_user_read(env.read_above_workdir(dir, name), &mut level.files)?;
        }
        match env.read_above_workdir(dir, BUILD_PROPS) {
            UserRead::Absent => {}
            UserRead::Content(text) => restore.extend(restore_sources(&text)?),
            UserRead::Unreadable => return Err(Closed),
        }
        out.push(level);
    }
    Ok((out, restore))
}

fn user_levels(env: &dyn HostEnv) -> Result<[Level; 2], Closed> {
    let mut user = Level::default();
    from_user_read(env.read_user_file(UserConfig::NugetUser), &mut user.files)?;
    let mut machine = Level::default();
    for r in env.read_user_files(UserConfig::NugetMachineWide) {
        from_user_read(r, &mut machine.files)?;
    }
    Ok([user, machine])
}

/// The NuGet registry signals, or `Closed` at the first failing step.
pub(super) fn load(
    workdir_canon: &Path,
    manifests: &[String],
    env: &dyn HostEnv,
) -> Result<EcoRegistry, Closed> {
    let nuget: Vec<&String> = manifests
        .iter()
        .filter(|m| is_nuget_manifest(file_name(m)))
        .collect();
    // One chain per manifest directory (no manifest ⇒ no chain: there is
    // no NuGet dependency to judge).
    let mut starts = BTreeSet::new();
    for m in &nuget {
        if !path_is_clean(m) {
            return Err(Closed);
        }
        starts.insert(parent_dir(m).to_string());
    }
    let all_dirs: BTreeSet<&str> = starts.iter().flat_map(|s| dir_and_ancestors(s)).collect();
    let mut levels: BTreeMap<&str, Level> = BTreeMap::new();
    let mut restore: BTreeMap<&str, Vec<FeedRef>> = BTreeMap::new();
    for d in &all_dirs {
        levels.insert(d, repo_level(workdir_canon, d)?);
        if let Some(t) = read_repo(workdir_canon, &join(d, BUILD_PROPS), MAX_FILE_BYTES)? {
            restore.entry(d).or_default().extend(restore_sources(&t)?);
        }
    }
    for m in nuget.iter().filter(|m| is_project_file(file_name(m))) {
        if let Some(t) = read_repo(workdir_canon, m, MAX_FILE_BYTES)? {
            restore
                .entry(parent_dir(m))
                .or_default()
                .extend(restore_sources(&t)?);
        }
    }
    let (above, above_restore) = above_levels(workdir_canon, env)?;
    let user = user_levels(env)?;
    let mut chains = Vec::new();
    for s in &starts {
        let dirs = dir_and_ancestors(s);
        let repo = dirs.iter().filter_map(|d| levels.get(d));
        chains.push(Chain {
            levels: repo.chain(&above).chain(&user).collect(),
            restore_feeds: dirs
                .iter()
                .filter_map(|d| restore.get(d))
                .flatten()
                .chain(&above_restore)
                .cloned()
                .collect(),
        });
    }
    resolve(&chains)
}
