//! The npm-family half of `RegistryContext::load` (§4.4, E3 npm/pnpm and
//! yarn rows).
//!
//! Fail-closed by construction: every step returns `Result<_, Closed>` and
//! is chained with `?`, and the only fail-OPEN value is produced by
//! `NpmSignals::into_open` in [`super::RegistryContext::load`] after this
//! whole function returned `Ok`. An early return can only ever leave npm
//! closed.

use std::collections::BTreeSet;
use std::path::Path;

use crate::adapter::{dir_and_ancestors, file_name, join, parent_dir};
use crate::eco::npm::MANIFEST;
use crate::fsread::{path_is_clean, safe_read, ReadOutcome, MAX_CONFIG_BYTES};

use super::host_env::{HostEnv, UserConfig, UserRead};
use super::npm_cfg::{
    env_pair, keep_npm_env, keep_yarn_env, npmrc, pnpm_workspace, yarnrc_v1, yarnrc_yml, Closed,
    NpmSignals,
};

/// Which extractor reads a config file.
#[derive(Clone, Copy)]
enum Format {
    Npmrc,
    YarnV1,
    YarnYml,
    PnpmWorkspace,
}

/// E3 "Repo walk" file names for npm / pnpm / yarn.
const REPO_FILES: &[(&str, Format)] = &[
    (".npmrc", Format::Npmrc),
    (".yarnrc", Format::YarnV1),
    (".yarnrc.yml", Format::YarnYml),
    ("pnpm-workspace.yaml", Format::PnpmWorkspace),
];

/// E3 user / system files.
const USER_FILES: &[(UserConfig, Format)] = &[
    (UserConfig::NpmUserRc, Format::Npmrc),
    (UserConfig::NpmGlobalRc, Format::Npmrc),
    (UserConfig::PnpmRc, Format::Npmrc),
    (UserConfig::YarnRc, Format::YarnV1),
    (UserConfig::YarnRcYml, Format::YarnYml),
];

/// `[deviation, verified in yarn 4.7.0]` berry's `findRcFiles` reads
/// `.yarnrc.yml` in every directory from the project up to the filesystem
/// root, so the file is read above the workdir too.
const ABOVE_WORKDIR_FILES: &[(&str, Format)] = &[(".yarnrc.yml", Format::YarnYml)];

fn extract(sig: &mut NpmSignals, fmt: Format, text: &str) -> Result<(), Closed> {
    match fmt {
        Format::Npmrc => npmrc(sig, text),
        Format::YarnV1 => yarnrc_v1(sig, text),
        Format::YarnYml => yarnrc_yml(sig, text),
        Format::PnpmWorkspace => pnpm_workspace(sig, text),
    }
}

/// `[HIGH-1 item 1]` every directory from each npm manifest's dir up to and
/// including the workdir (`""`). An unclean manifest path is `Closed`.
fn walk_dirs(manifests: &[String]) -> Result<BTreeSet<String>, Closed> {
    let mut dirs = BTreeSet::from([String::new()]);
    for m in manifests.iter().filter(|m| file_name(m) == MANIFEST) {
        if !path_is_clean(m) {
            return Err(Closed);
        }
        dirs.extend(
            dir_and_ancestors(parent_dir(m))
                .into_iter()
                .map(str::to_string),
        );
    }
    Ok(dirs)
}

fn repo_walk(
    sig: &mut NpmSignals,
    workdir_canon: &Path,
    manifests: &[String],
) -> Result<(), Closed> {
    for dir in walk_dirs(manifests)? {
        for (name, fmt) in REPO_FILES {
            match safe_read(workdir_canon, &join(&dir, name), MAX_CONFIG_BYTES) {
                ReadOutcome::Absent => {}
                ReadOutcome::Content(text) => extract(sig, *fmt, &text)?,
                // [HIGH-1 item 2] symlink, FIFO, directory, oversize, EACCES.
                ReadOutcome::NotAFile | ReadOutcome::TooLarge | ReadOutcome::Unreadable => {
                    return Err(Closed)
                }
            }
        }
    }
    Ok(())
}

fn apply_user_read(sig: &mut NpmSignals, fmt: Format, r: UserRead) -> Result<(), Closed> {
    match r {
        UserRead::Absent => Ok(()),
        UserRead::Content(text) => extract(sig, fmt, &text),
        UserRead::Unreadable => Err(Closed),
    }
}

fn above_walk(sig: &mut NpmSignals, workdir_canon: &Path, env: &dyn HostEnv) -> Result<(), Closed> {
    for dir in workdir_canon.ancestors().skip(1) {
        for (name, fmt) in ABOVE_WORKDIR_FILES {
            apply_user_read(sig, *fmt, env.read_above_workdir(dir, name))?;
        }
    }
    Ok(())
}

fn user_files(sig: &mut NpmSignals, env: &dyn HostEnv) -> Result<(), Closed> {
    for (which, fmt) in USER_FILES {
        apply_user_read(sig, *fmt, env.read_user_file(*which))?;
    }
    Ok(())
}

fn env_vars(sig: &mut NpmSignals, env: &dyn HostEnv) -> Result<(), Closed> {
    let npm = env.vars_with_prefix_ci("npm_config_", keep_npm_env);
    let pnpm = env.vars_with_prefix_ci("pnpm_config_", keep_npm_env);
    let yarn = env.vars_with_prefix_ci("yarn_", keep_yarn_env);
    for (k, v) in npm.iter().chain(&pnpm).chain(&yarn) {
        env_pair(sig, k, v)?;
    }
    Ok(())
}

/// All npm-family signals, or `Closed` at the first failing step.
pub(super) fn load(
    workdir_canon: &Path,
    manifests: &[String],
    env: &dyn HostEnv,
) -> Result<NpmSignals, Closed> {
    let mut sig = NpmSignals::default();
    repo_walk(&mut sig, workdir_canon, manifests)?;
    above_walk(&mut sig, workdir_canon, env)?;
    user_files(&mut sig, env)?;
    env_vars(&mut sig, env)?;
    Ok(sig)
}
