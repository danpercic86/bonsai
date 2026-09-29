//! Per-OS location of every `UserConfig` (E3 "User / system files"). PURE:
//! the variables, the OS and the npm install dir are injected, so every
//! OS's table (incl. Linux `~/.config/go/env`) is unit-tested on any host.
//! The IO (safe reads, the dir listing, the PATH probe) is in `host_env.rs`.
//!
//! Resolution rules:
//! - an unset OPTIONAL override (`$PIP_CONFIG_FILE`, `$GOENV`, …) falls back
//!   to the per-OS default, or is `Absent` when the file has no default;
//! - `GOENV=off` and "npm is not installed" are `Absent`;
//! - a base directory that cannot be resolved (no home, no `%APPDATA%`) or an
//!   override that is not an absolute path is `Unresolvable` ⇒ the
//!   ecosystem fails closed (contract header: a registry signal we cannot
//!   read or resolve makes the dependency Private).

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use bonsai_deps::registry_ctx::UserConfig;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Os {
    Windows,
    Mac,
    /// Linux and every other Unix (XDG layout).
    Linux,
}

impl Os {
    pub(crate) fn current() -> Os {
        if cfg!(windows) {
            Os::Windows
        } else if cfg!(target_os = "macos") {
            Os::Mac
        } else {
            Os::Linux
        }
    }
}

/// Where one `UserConfig` lives.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Located {
    Absent,
    Unresolvable,
    /// Candidate files, each read with the following config read; the
    /// results are folded (`host_env::fold`).
    Files(Vec<PathBuf>),
    /// A directory whose `*.config` files form the set (NuGet machine-wide).
    ConfigDir(PathBuf),
}

/// The injected inputs. `var` returns a NON-EMPTY value or `None`.
pub(crate) struct Inputs<'a> {
    pub os: Os,
    pub var: &'a dyn Fn(&str) -> Option<OsString>,
    /// The directory holding the `node` executable, found on `PATH`.
    pub node_dir: &'a dyn Fn() -> Option<PathBuf>,
}

/// Absolute for `os` (checked on the text, so the table is testable on
/// any host): `X:\` / `X:/` or `\\…` on Windows, a leading `/` elsewhere.
fn is_abs(os: Os, p: &Path) -> bool {
    let s = p.to_string_lossy();
    let b = s.as_bytes();
    match os {
        Os::Windows => {
            let sep = |c: u8| c == b'\\' || c == b'/';
            (b.len() >= 3 && b[0].is_ascii_alphabetic() && b[1] == b':' && sep(b[2]))
                || (b.len() >= 2 && sep(b[0]) && sep(b[1]))
        }
        Os::Mac | Os::Linux => b.first() == Some(&b'/'),
    }
}

impl Inputs<'_> {
    /// An absolute path from `key`: `None` = unset, `Some(None)` = set but
    /// not absolute.
    fn path_var(&self, key: &str) -> Option<Option<PathBuf>> {
        let p = PathBuf::from((self.var)(key)?);
        Some(is_abs(self.os, &p).then_some(p))
    }

    /// A base directory: unset or relative ⇒ `None` (the caller maps it to
    /// `Unresolvable`).
    fn base(&self, key: &str) -> Option<PathBuf> {
        self.path_var(key).flatten()
    }

    pub(crate) fn home(&self) -> Option<PathBuf> {
        match self.os {
            Os::Windows => self.base("USERPROFILE").or_else(|| self.base("HOME")),
            Os::Mac | Os::Linux => self.base("HOME"),
        }
    }

    fn xdg_config(&self) -> Option<PathBuf> {
        self.base("XDG_CONFIG_HOME")
            .or_else(|| Some(self.home()?.join(".config")))
    }

    pub(crate) fn cargo_home(&self) -> Option<PathBuf> {
        self.base("CARGO_HOME")
            .or_else(|| Some(self.home()?.join(".cargo")))
    }

    /// pnpm 11 `getConfigDir` (VERIFIED, `host_env.rs` in bonsai-deps).
    fn pnpm_dir(&self) -> Option<PathBuf> {
        if let Some(x) = self.base("XDG_CONFIG_HOME") {
            return Some(x.join("pnpm"));
        }
        match self.os {
            Os::Mac => Some(
                self.home()?
                    .join("Library")
                    .join("Preferences")
                    .join("pnpm"),
            ),
            Os::Linux => Some(self.home()?.join(".config").join("pnpm")),
            Os::Windows => self
                .base("LOCALAPPDATA")
                .map(|l| l.join("pnpm").join("config"))
                .or_else(|| Some(self.home()?.join(".config").join("pnpm"))),
        }
    }
}

fn file(p: Option<PathBuf>) -> Located {
    p.map_or(Located::Unresolvable, |p| Located::Files(vec![p]))
}

/// `key` overrides the default path when set; set-but-relative fails closed.
fn overridable(i: &Inputs, key: &str, default: impl FnOnce() -> Located) -> Located {
    match i.path_var(key) {
        Some(Some(p)) => Located::Files(vec![p]),
        Some(None) => Located::Unresolvable,
        None => default(),
    }
}

/// Every candidate of a multi-file kind; a missing base drops the whole
/// kind to `Unresolvable` (never silently fewer files).
fn files(parts: Vec<Option<PathBuf>>) -> Located {
    parts
        .into_iter()
        .collect::<Option<Vec<_>>>()
        .map_or(Located::Unresolvable, Located::Files)
}

fn npm_global(i: &Inputs) -> Located {
    let rc = |prefix: PathBuf| prefix.join("etc").join("npmrc");
    match i.path_var("NPM_CONFIG_PREFIX") {
        Some(Some(p)) => return Located::Files(vec![rc(p)]),
        Some(None) => return Located::Unresolvable,
        None => {}
    }
    match i.os {
        Os::Windows => file(i.base("APPDATA").map(|a| rc(a.join("npm")))),
        // ASSUMED (E3): the default prefix is where node is installed.
        Os::Mac | Os::Linux => {
            let prefix = (i.node_dir)()
                .and_then(|d| d.parent().map(Path::to_path_buf))
                .unwrap_or_else(|| PathBuf::from("/usr/local"));
            Located::Files(vec![rc(prefix)])
        }
    }
}

/// ASSUMED (E3): npm's builtin `npmrc` in the npm package next to `node`.
fn npm_builtin(i: &Inputs) -> Located {
    let Some(dir) = (i.node_dir)() else {
        return Located::Absent;
    };
    let npm = match i.os {
        Os::Windows => dir.join("node_modules").join("npm"),
        Os::Mac | Os::Linux => match dir.parent() {
            Some(prefix) => prefix.join("lib").join("node_modules").join("npm"),
            None => return Located::Absent,
        },
    };
    Located::Files(vec![npm.join("npmrc")])
}

fn pip_user(i: &Inputs) -> Located {
    let home = i.home();
    let h = |parts: &[&str]| {
        home.as_ref()
            .map(|h| parts.iter().fold(h.clone(), |p, s| p.join(s)))
    };
    match i.os {
        Os::Windows => files(vec![
            i.base("APPDATA").map(|a| a.join("pip").join("pip.ini")),
            h(&["pip", "pip.ini"]),
        ]),
        Os::Mac => files(vec![
            h(&["Library", "Application Support", "pip", "pip.conf"]),
            h(&[".config", "pip", "pip.conf"]),
            h(&[".pip", "pip.conf"]),
        ]),
        Os::Linux => files(vec![
            i.xdg_config().map(|x| x.join("pip").join("pip.conf")),
            h(&[".pip", "pip.conf"]),
        ]),
    }
}

/// pip's GLOBAL files, plus the SITE file of an activated virtualenv
/// outside the repo (`$VIRTUAL_ENV/pip.{conf,ini}`, both names — a superset).
fn pip_system(i: &Inputs) -> Located {
    let mut out: Vec<Option<PathBuf>> = match i.os {
        Os::Windows => vec![i.base("PROGRAMDATA").map(|p| p.join("pip").join("pip.ini"))],
        Os::Mac => vec![Some(PathBuf::from(
            "/Library/Application Support/pip/pip.conf",
        ))],
        Os::Linux => vec![
            Some(PathBuf::from("/etc/pip.conf")),
            Some(PathBuf::from("/etc/xdg/pip/pip.conf")),
        ],
    };
    match i.path_var("VIRTUAL_ENV") {
        Some(Some(v)) => {
            out.push(Some(v.join("pip.conf")));
            out.push(Some(v.join("pip.ini")));
        }
        Some(None) => return Located::Unresolvable,
        None => {}
    }
    files(out)
}

fn go_env(i: &Inputs) -> Located {
    if (i.var)("GOENV").is_some_and(|v| v == "off") {
        return Located::Absent;
    }
    overridable(i, "GOENV", || match i.os {
        Os::Windows => file(i.base("APPDATA").map(|a| a.join("go").join("env"))),
        Os::Mac => file(i.home().map(|h| {
            h.join("Library")
                .join("Application Support")
                .join("go")
                .join("env")
        })),
        // `$XDG_CONFIG_HOME/go/env`, else `~/.config/go/env` (ASSUMED, E3).
        Os::Linux => file(i.xdg_config().map(|x| x.join("go").join("env"))),
    })
}

pub(crate) fn locate(which: UserConfig, i: &Inputs) -> Located {
    let home = |rel: &[&str]| file(i.home().map(|h| rel.iter().fold(h, |p, s| p.join(s))));
    match which {
        UserConfig::NpmUserRc => overridable(i, "NPM_CONFIG_USERCONFIG", || home(&[".npmrc"])),
        UserConfig::NpmGlobalRc => npm_global(i),
        UserConfig::NpmBuiltinRc => npm_builtin(i),
        UserConfig::PnpmRc => file(i.pnpm_dir().map(|d| d.join("rc"))),
        UserConfig::PnpmAuthIni => file(i.pnpm_dir().map(|d| d.join("auth.ini"))),
        UserConfig::PnpmGlobalYaml => file(i.pnpm_dir().map(|d| d.join("config.yaml"))),
        UserConfig::YarnRc => home(&[".yarnrc"]),
        UserConfig::YarnRcYml => home(&[".yarnrc.yml"]),
        UserConfig::CargoHomeConfigToml => file(i.cargo_home().map(|c| c.join("config.toml"))),
        UserConfig::CargoHomeConfig => file(i.cargo_home().map(|c| c.join("config"))),
        UserConfig::PipConfigFile => overridable(i, "PIP_CONFIG_FILE", || Located::Absent),
        UserConfig::PipUser => pip_user(i),
        UserConfig::PipSystem => pip_system(i),
        UserConfig::UvUser => match i.os {
            Os::Windows => file(i.base("APPDATA").map(|a| a.join("uv").join("uv.toml"))),
            Os::Mac | Os::Linux => file(i.xdg_config().map(|x| x.join("uv").join("uv.toml"))),
        },
        UserConfig::UvSystem => match i.os {
            Os::Windows => file(i.base("PROGRAMDATA").map(|p| p.join("uv").join("uv.toml"))),
            Os::Mac | Os::Linux => Located::Files(vec![PathBuf::from("/etc/uv/uv.toml")]),
        },
        UserConfig::GoEnv => go_env(i),
        UserConfig::NugetUser => match i.os {
            Os::Windows => file(
                i.base("APPDATA")
                    .map(|a| a.join("NuGet").join("NuGet.Config")),
            ),
            Os::Mac | Os::Linux => home(&[".nuget", "NuGet", "NuGet.Config"]),
        },
        UserConfig::NugetMachineWide => {
            let dir = match i.os {
                Os::Windows => i
                    .base("ProgramFiles(x86)")
                    .or_else(|| i.base("ProgramFiles"))
                    .map(|p| p.join("NuGet").join("Config")),
                Os::Mac => Some(PathBuf::from("/Library/Application Support/NuGet/Config")),
                Os::Linux => Some(PathBuf::from("/etc/opt/NuGet/Config")),
            };
            dir.map_or(Located::Unresolvable, Located::ConfigDir)
        }
    }
}
