//! The Cargo half of `RegistryContext::load` (§4.4, E3 Cargo row).
//!
//! Same fail-closed shape as `load_npm`: every step is `Result<_, Closed>`,
//! and the only fail-OPEN value is `CargoSignals::into_open`, called by
//! [`super::RegistryContext::load`] after this whole function returned `Ok`.
//!
//! Files, VERIFIED against the cargo book (`reference/config.html`, 1.97
//! toolchain docs): cargo probes `.cargo/config.toml` in the current
//! directory and EVERY parent up to the filesystem root, then
//! `$CARGO_HOME/config.toml`; the extensionless legacy `.cargo/config` is
//! still read (and preferred when both exist). Both names are read at every
//! level here — a superset. `credentials{,.toml}` is never read: there is no
//! `UserConfig` variant for it.

use std::path::Path;

use crate::eco::cargo::MANIFEST;

use super::cargo_cfg::{cargo_config, cargo_env_pair, keep_cargo_env, CargoSignals};
use super::host_env::{HostEnv, UserConfig};
use super::walk::{above_walk, manifest_dirs, repo_walk, user_files};
use super::Closed;

/// E3 repo-walk and `[Q7]` above-workdir file names (config files only).
const CONFIG_FILES: &[(&str, ())] = &[(".cargo/config.toml", ()), (".cargo/config", ())];

/// E3 user files.
const USER_FILES: &[(UserConfig, ())] = &[
    (UserConfig::CargoHomeConfigToml, ()),
    (UserConfig::CargoHomeConfig, ()),
];

fn env_vars(sig: &mut CargoSignals, env: &dyn HostEnv) -> Result<(), Closed> {
    for (k, v) in env.vars_with_prefix_ci("CARGO_", keep_cargo_env) {
        cargo_env_pair(sig, &k, &v)?;
    }
    Ok(())
}

/// All Cargo signals, or `Closed` at the first failing step.
pub(super) fn load(
    workdir_canon: &Path,
    manifests: &[String],
    env: &dyn HostEnv,
) -> Result<CargoSignals, Closed> {
    let mut sig = CargoSignals::default();
    let mut f = |(): (), text: &str| cargo_config(&mut sig, text);
    let dirs = manifest_dirs(manifests, MANIFEST)?;
    repo_walk(workdir_canon, &dirs, CONFIG_FILES, &mut f)?;
    // [Q7] every ancestor above the workdir, to the filesystem root.
    above_walk(workdir_canon, env, CONFIG_FILES, &mut f)?;
    user_files(env, USER_FILES, &mut f)?;
    env_vars(&mut sig, env)?;
    Ok(sig)
}
