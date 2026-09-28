//! Host-environment seam for `RegistryContext::load` (contract §4.4). The
//! real implementation (env vars, user/system config files, the Q7
//! above-workdir walk) and the `FakeHostEnv` test double land in P125b.

use std::fmt;
use std::path::Path;

/// Result of reading one registry config file.
///
/// `[NEW-2]` The Debug impl is hand-written and prints ONLY the variant:
/// `Content` may hold registry credentials that the key-filtered extractor
/// has not yet dropped.
pub enum UserRead {
    Absent,
    Content(String),
    Unreadable,
}

impl fmt::Debug for UserRead {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            UserRead::Absent => "Absent",
            UserRead::Content(_) => "Content(..)",
            UserRead::Unreadable => "Unreadable",
        })
    }
}

/// The closed set of user / system registry config files (E3 "User /
/// system files"). The per-OS path resolution lives in the real `HostEnv`
/// (P125b); P125b/c1/c2 verify each row against the tool docs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UserConfig {
    /// `$NPM_CONFIG_USERCONFIG` else `~/.npmrc`.
    NpmUserRc,
    /// `$NPM_CONFIG_PREFIX/etc/npmrc`, else the per-OS global npmrc.
    NpmGlobalRc,
    /// pnpm `rc` in the per-OS config dir.
    PnpmRc,
    /// `~/.yarnrc` (v1).
    YarnRc,
    /// `~/.yarnrc.yml` (berry).
    YarnRcYml,
    /// `$CARGO_HOME/config.toml`.
    CargoHomeConfigToml,
    /// `$CARGO_HOME/config` (legacy).
    CargoHomeConfig,
    /// `$PIP_CONFIG_FILE`.
    PipConfigFile,
    /// The per-OS user pip config(s).
    PipUser,
    /// The per-OS site-wide pip config(s).
    PipSystem,
    /// The per-OS user `uv.toml`.
    UvUser,
    /// The per-OS system `uv.toml`.
    UvSystem,
    /// `$GOENV` else the per-OS go `env` file.
    GoEnv,
    /// The per-OS user `NuGet.Config`.
    NugetUser,
    /// The per-OS machine-wide `NuGet/Config/*.config` set.
    NugetMachineWide,
}

/// Everything `RegistryContext::load` reads from outside the repo.
pub trait HostEnv: Sync {
    fn var(&self, name: &str) -> Option<String>;
    /// `[NEW-2]` `keep` filters on the KEY before the value is collected:
    /// only registry/index keys are returned, so auth-token-style values are
    /// never read into memory.
    fn vars_with_prefix_ci(&self, prefix: &str, keep: fn(&str) -> bool) -> Vec<(String, String)>;
    fn read_user_file(&self, which: UserConfig) -> UserRead;
    /// `[Q7]` Bounded, regular-file-only, NO-FOLLOW read of `file_name` in
    /// `dir` (an ancestor ABOVE the workdir). Symlink / reparse point /
    /// non-regular / > 1 MiB ⇒ Unreadable.
    fn read_above_workdir(&self, dir: &Path, file_name: &str) -> UserRead;
}
