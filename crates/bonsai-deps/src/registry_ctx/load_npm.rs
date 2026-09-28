//! The npm-family half of `RegistryContext::load` (§4.4, E3 npm/pnpm and
//! yarn rows).
//!
//! Fail-closed by construction: every step returns `Result<_, Closed>` and
//! is chained with `?`, and the only fail-OPEN value is produced by
//! `NpmSignals::into_open` in [`super::RegistryContext::load`] after this
//! whole function returned `Ok`. An early return can only ever leave npm
//! closed.

use std::path::Path;

use crate::eco::npm::MANIFEST;

use super::host_env::{HostEnv, UserConfig};
use super::npm_cfg::{
    env_pair, keep_npm_env, keep_yarn_env, npmrc, pnpm_global_yaml, pnpm_workspace, yarnrc_v1,
    yarnrc_yml, Closed, NpmSignals,
};
use super::walk::{above_walk, manifest_dirs, repo_walk, user_files};

/// Which extractor reads a config file.
#[derive(Clone, Copy)]
enum Format {
    Npmrc,
    YarnV1,
    YarnYml,
    PnpmWorkspace,
    PnpmGlobal,
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
    (UserConfig::NpmBuiltinRc, Format::Npmrc),
    (UserConfig::PnpmRc, Format::Npmrc),
    (UserConfig::PnpmAuthIni, Format::Npmrc),
    (UserConfig::YarnRc, Format::YarnV1),
    (UserConfig::YarnRcYml, Format::YarnYml),
    (UserConfig::PnpmGlobalYaml, Format::PnpmGlobal),
];

/// `[Q7]` config files read in every ancestor ABOVE the workdir:
/// - `.yarnrc.yml` `[deviation, verified in yarn 4.7.0]`: berry's
///   `findRcFiles` reads it in every directory from the project up to the
///   filesystem root;
/// - `.npmrc` and `pnpm-workspace.yaml` `[P125b review (i)]`: npm reads the
///   `.npmrc` of the `workspaces` root and pnpm the `.npmrc` +
///   `pnpm-workspace.yaml` of the workspace root, which sit ABOVE a
///   submodule workdir. Finding the npm root would need a `package.json`
///   read above the workdir, which MEDIUM-3 forbids, so both are read in
///   EVERY ancestor — a superset that can only add private signals.
const ABOVE_WORKDIR_FILES: &[(&str, Format)] = &[
    (".npmrc", Format::Npmrc),
    (".yarnrc.yml", Format::YarnYml),
    ("pnpm-workspace.yaml", Format::PnpmWorkspace),
];

fn extract(sig: &mut NpmSignals, fmt: Format, text: &str) -> Result<(), Closed> {
    match fmt {
        Format::Npmrc => npmrc(sig, text),
        Format::YarnV1 => yarnrc_v1(sig, text),
        Format::YarnYml => yarnrc_yml(sig, text),
        Format::PnpmWorkspace => pnpm_workspace(sig, text),
        Format::PnpmGlobal => pnpm_global_yaml(sig, text),
    }
}

/// `[P125b review]` token-bearing variables that can change registry
/// routing. Their VALUE is never read: presence alone fails npm closed.
const PRESENCE_CLOSES: &[&str] = &["pnpm_config__auth"];

fn env_vars(sig: &mut NpmSignals, env: &dyn HostEnv) -> Result<(), Closed> {
    if PRESENCE_CLOSES.iter().any(|n| env.var_present_ci(n)) {
        return Err(Closed);
    }
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
    let mut f = |fmt: Format, text: &str| extract(&mut sig, fmt, text);
    let dirs = manifest_dirs(manifests, MANIFEST)?;
    repo_walk(workdir_canon, &dirs, REPO_FILES, &mut f)?;
    above_walk(workdir_canon, env, ABOVE_WORKDIR_FILES, &mut f)?;
    user_files(env, USER_FILES, &mut f)?;
    env_vars(&mut sig, env)?;
    Ok(sig)
}
