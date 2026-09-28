//! Host-environment seam for `RegistryContext::load` (contract §4.4). The
//! test double `FakeHostEnv` lives in `fake_env.rs` (`cfg(test)` or the
//! `test-support` feature). The real implementation (env vars via
//! `vars_os()`, dropping every non-matching value at once; user/system
//! config files; the Q7 above-workdir walk) lands with src-tauri in P125e.

use std::fmt;
use std::path::Path;

use crate::fsread::ReadOutcome;

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

impl UserRead {
    /// `[Q9-3]` THE mapping from a safe read to a config read, shared by
    /// every `HostEnv` (the P125e real one included): only Absent stays
    /// Absent; NotAFile (symlink where not allowed, network / device
    /// target), TooLarge and Unreadable all fail the ecosystem closed.
    /// Exhaustive on purpose — no `_` arm.
    pub fn from_outcome(o: ReadOutcome) -> UserRead {
        match o {
            ReadOutcome::Absent => UserRead::Absent,
            ReadOutcome::Content(s) => UserRead::Content(s),
            ReadOutcome::NotAFile => UserRead::Unreadable,
            ReadOutcome::TooLarge => UserRead::Unreadable,
            ReadOutcome::Unreadable => UserRead::Unreadable,
        }
    }
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
    /// `[P125b review]` npm's BUILTIN config, `<npm install dir>/npmrc`
    /// (npm docs `npmrc.md` "Built-in config file": "an unchangeable
    /// builtin configuration file that npm keeps consistent across
    /// updates"). Path resolution (the npm package dir next to the `node`
    /// executable) lands with the real `HostEnv` in P125e — ASSUMED. Read
    /// with the npmrc key filter; Absent when npm is not installed; a file
    /// that exists but cannot be read ⇒ Unreadable ⇒ npm fails closed.
    NpmBuiltinRc,
    /// pnpm `rc` in the per-OS config dir.
    PnpmRc,
    /// pnpm 11 `<configDir>/auth.ini` (same config dir as
    /// [`UserConfig::PnpmGlobalYaml`]): merged by pnpm 11.17.0
    /// `loadNpmrcConfig` as a full npmrc, so it can route `@scope:registry`.
    /// Read with the npmrc key filter; path resolution lands in P125e.
    PnpmAuthIni,
    /// pnpm 11 global `config.yaml` in the pnpm config dir — VERIFIED in
    /// pnpm 11.17.0 `getConfigDir`: `$XDG_CONFIG_HOME/pnpm` when set (any
    /// OS), else macOS `~/Library/Preferences/pnpm`, other Unix
    /// `~/.config/pnpm`, Windows `%LOCALAPPDATA%\pnpm\config` (falling back
    /// to `~/.config/pnpm`). Read with the `pnpm-workspace.yaml` extractor.
    PnpmGlobalYaml,
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
    /// `$GOENV` (unless `off`) else the per-OS go `env` file: Windows
    /// `%APPDATA%\go\env`, macOS `~/Library/Application Support/go/env`,
    /// Linux `$XDG_CONFIG_HOME/go/env`, falling back to `~/.config/go/env`
    /// when `XDG_CONFIG_HOME` is unset (ASSUMED, E3).
    GoEnv,
    /// The per-OS user `NuGet.Config`.
    NugetUser,
    /// The per-OS machine-wide `NuGet/Config/*.config` set.
    NugetMachineWide,
}

/// Everything `RegistryContext::load` reads from outside the repo.
pub trait HostEnv: Sync {
    fn var(&self, name: &str) -> Option<String>;
    /// `[P125b review]` whether variable `name` is set (ASCII
    /// case-insensitive), WITHOUT reading its value. For token-bearing
    /// variables whose mere presence must fail an ecosystem closed
    /// (`pnpm_config__auth`). No default body: every `HostEnv` implements
    /// it, and none may materialise the value.
    fn var_present_ci(&self, name: &str) -> bool;
    /// `[NEW-2]` `keep` filters on the KEY before the value is collected:
    /// only registry/index keys are returned, so auth-token-style values are
    /// never read into memory.
    fn vars_with_prefix_ci(&self, prefix: &str, keep: fn(&str) -> bool) -> Vec<(String, String)>;
    /// `[Q9]` symlinks followed (`fsread::read_config_follow`): the target
    /// must be a regular file after open, a network / UNC / DOS-device
    /// target is rejected unopened, a dangling link or > 1 MiB ⇒ Unreadable.
    /// Map with [`UserRead::from_outcome`].
    fn read_user_file(&self, which: UserConfig) -> UserRead;
    /// `[P125c2]` a user / system config that is a SET of files — only
    /// [`UserConfig::NugetMachineWide`] (`NuGet/Config/*.config`) today —
    /// one read per file, same rules as [`HostEnv::read_user_file`]. No
    /// default body `[P125c2 review]`: every `HostEnv` (the P125e real one
    /// included) must resolve the set itself.
    fn read_user_files(&self, which: UserConfig) -> Vec<UserRead>;
    /// `[Q7]` `[Q9-narrowed]` Bounded read of `file_name` in `dir` (an
    /// ancestor ABOVE the workdir) via `fsread::read_above_workdir_config`:
    /// symlinks are followed ONLY when `dir` is `$HOME` / `$CARGO_HOME`
    /// (canonical); in any other ancestor a symlinked component ⇒
    /// Unreadable (fail closed). Otherwise the `read_user_file` rules.
    /// `file_name` may be a relative `/`-separated path
    /// (`.cargo/config.toml`).
    fn read_above_workdir(&self, dir: &Path, file_name: &str) -> UserRead;
}
