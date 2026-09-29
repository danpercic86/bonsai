//! The E3 per-OS location table, every OS on any host (pure; injected vars).

use std::collections::HashMap;
use std::ffi::OsString;
use std::path::PathBuf;

use bonsai_deps::registry_ctx::UserConfig;

use super::host_paths::{locate, Inputs, Located, Os};

fn at(os: Os, vars: &[(&str, &str)], node: Option<&str>, which: UserConfig) -> Located {
    let map: HashMap<String, OsString> = vars
        .iter()
        .map(|(k, v)| ((*k).to_owned(), OsString::from(v)))
        .collect();
    let var = |k: &str| map.get(k).cloned();
    let node_dir = || node.map(PathBuf::from);
    locate(
        which,
        &Inputs {
            os,
            var: &var,
            node_dir: &node_dir,
        },
    )
}

fn one(base: &str, parts: &[&str]) -> Located {
    Located::Files(vec![parts
        .iter()
        .fold(PathBuf::from(base), |p, s| p.join(s))])
}

const LINUX: &[(&str, &str)] = &[("HOME", "/home/u")];
const WIN: &[(&str, &str)] = &[
    ("USERPROFILE", r"C:\Users\u"),
    ("APPDATA", r"C:\Users\u\AppData\Roaming"),
    ("LOCALAPPDATA", r"C:\Users\u\AppData\Local"),
    ("PROGRAMDATA", r"C:\ProgramData"),
    ("ProgramFiles(x86)", r"C:\Program Files (x86)"),
];

#[test]
fn go_env_follows_goenv_then_the_per_os_default() {
    use UserConfig::GoEnv;
    // [P125c1 review] Linux falls back to ~/.config/go/env without XDG.
    assert_eq!(
        at(Os::Linux, LINUX, None, GoEnv),
        one("/home/u", &[".config", "go", "env"])
    );
    assert_eq!(
        at(
            Os::Linux,
            &[("HOME", "/home/u"), ("XDG_CONFIG_HOME", "/xdg")],
            None,
            GoEnv
        ),
        one("/xdg", &["go", "env"])
    );
    assert_eq!(
        at(Os::Mac, LINUX, None, GoEnv),
        one("/home/u", &["Library", "Application Support", "go", "env"])
    );
    assert_eq!(
        at(Os::Windows, WIN, None, GoEnv),
        one(r"C:\Users\u\AppData\Roaming", &["go", "env"])
    );
    assert_eq!(
        at(Os::Linux, &[("GOENV", "off")], None, GoEnv),
        Located::Absent
    );
    assert_eq!(
        at(Os::Linux, &[("GOENV", "/etc/goenv")], None, GoEnv),
        one("/etc/goenv", &[])
    );
    // A relative override, or no home at all, fails closed.
    assert_eq!(
        at(Os::Linux, &[("GOENV", "rel/env")], None, GoEnv),
        Located::Unresolvable
    );
    assert_eq!(at(Os::Linux, &[], None, GoEnv), Located::Unresolvable);
}

#[test]
fn pnpm_config_dir_rows_match_pnpm_11() {
    use UserConfig::{PnpmAuthIni, PnpmGlobalYaml, PnpmRc};
    assert_eq!(
        at(Os::Linux, LINUX, None, PnpmGlobalYaml),
        one("/home/u", &[".config", "pnpm", "config.yaml"])
    );
    assert_eq!(
        at(Os::Mac, LINUX, None, PnpmAuthIni),
        one("/home/u", &["Library", "Preferences", "pnpm", "auth.ini"])
    );
    assert_eq!(
        at(Os::Windows, WIN, None, PnpmRc),
        one(r"C:\Users\u\AppData\Local", &["pnpm", "config", "rc"])
    );
    // $XDG_CONFIG_HOME wins on every OS.
    let xdg = &[("HOME", "/home/u"), ("XDG_CONFIG_HOME", "/xdg")];
    assert_eq!(
        at(Os::Mac, xdg, None, PnpmGlobalYaml),
        one("/xdg", &["pnpm", "config.yaml"])
    );
}

#[test]
fn npm_user_global_and_builtin_rows() {
    use UserConfig::{NpmBuiltinRc, NpmGlobalRc, NpmUserRc};
    assert_eq!(
        at(Os::Linux, LINUX, None, NpmUserRc),
        one("/home/u", &[".npmrc"])
    );
    assert_eq!(
        at(
            Os::Linux,
            &[("NPM_CONFIG_USERCONFIG", "/cfg/rc")],
            None,
            NpmUserRc
        ),
        one("/cfg/rc", &[])
    );
    assert_eq!(
        at(Os::Windows, WIN, None, NpmGlobalRc),
        one(r"C:\Users\u\AppData\Roaming", &["npm", "etc", "npmrc"])
    );
    assert_eq!(
        at(Os::Linux, LINUX, Some("/opt/node/bin"), NpmGlobalRc),
        one("/opt/node", &["etc", "npmrc"])
    );
    assert_eq!(
        at(Os::Linux, LINUX, None, NpmGlobalRc),
        one("/usr/local", &["etc", "npmrc"])
    );
    assert_eq!(
        at(Os::Linux, &[("NPM_CONFIG_PREFIX", "/p")], None, NpmGlobalRc),
        one("/p", &["etc", "npmrc"])
    );
    // Builtin: next to node; npm not installed ⇒ Absent.
    assert_eq!(
        at(
            Os::Windows,
            WIN,
            Some(r"C:\Program Files\nodejs"),
            NpmBuiltinRc
        ),
        one(
            r"C:\Program Files\nodejs",
            &["node_modules", "npm", "npmrc"]
        )
    );
    assert_eq!(
        at(Os::Linux, LINUX, Some("/usr/local/bin"), NpmBuiltinRc),
        one("/usr/local", &["lib", "node_modules", "npm", "npmrc"])
    );
    assert_eq!(at(Os::Linux, LINUX, None, NpmBuiltinRc), Located::Absent);
}

#[test]
fn pip_kinds_list_every_candidate_and_the_virtualenv() {
    use UserConfig::{PipConfigFile, PipSystem, PipUser};
    let p = |b: &str, parts: &[&str]| parts.iter().fold(PathBuf::from(b), |p, s| p.join(s));
    assert_eq!(
        at(Os::Linux, LINUX, None, PipUser),
        Located::Files(vec![
            p("/home/u", &[".config", "pip", "pip.conf"]),
            p("/home/u", &[".pip", "pip.conf"]),
        ])
    );
    assert_eq!(
        at(Os::Windows, WIN, None, PipUser),
        Located::Files(vec![
            p(r"C:\Users\u\AppData\Roaming", &["pip", "pip.ini"]),
            p(r"C:\Users\u", &["pip", "pip.ini"]),
        ])
    );
    let venv = &[("HOME", "/home/u"), ("VIRTUAL_ENV", "/work/venv")];
    assert_eq!(
        at(Os::Linux, venv, None, PipSystem),
        Located::Files(vec![
            p("/etc/pip.conf", &[]),
            p("/etc/xdg/pip/pip.conf", &[]),
            p("/work/venv", &["pip.conf"]),
            p("/work/venv", &["pip.ini"]),
        ])
    );
    assert_eq!(at(Os::Linux, LINUX, None, PipConfigFile), Located::Absent);
    assert_eq!(
        at(
            Os::Linux,
            &[("PIP_CONFIG_FILE", "pip.conf")],
            None,
            PipConfigFile
        ),
        Located::Unresolvable
    );
    // Windows with no %APPDATA% cannot resolve the user kind ⇒ fail closed.
    assert_eq!(
        at(
            Os::Windows,
            &[("USERPROFILE", r"C:\Users\u")],
            None,
            PipUser
        ),
        Located::Unresolvable
    );
}

#[test]
fn cargo_uv_nuget_and_yarn_rows() {
    use UserConfig::*;
    assert_eq!(
        at(Os::Linux, LINUX, None, CargoHomeConfigToml),
        one("/home/u", &[".cargo", "config.toml"])
    );
    assert_eq!(
        at(Os::Linux, &[("CARGO_HOME", "/c")], None, CargoHomeConfig),
        one("/c", &["config"])
    );
    assert_eq!(
        at(Os::Windows, WIN, None, UvSystem),
        one(r"C:\ProgramData", &["uv", "uv.toml"])
    );
    assert_eq!(
        at(Os::Linux, LINUX, None, UvUser),
        one("/home/u", &[".config", "uv", "uv.toml"])
    );
    assert_eq!(
        at(Os::Mac, LINUX, None, NugetUser),
        one("/home/u", &[".nuget", "NuGet", "NuGet.Config"])
    );
    assert_eq!(
        at(Os::Windows, WIN, None, NugetMachineWide),
        Located::ConfigDir(
            PathBuf::from(r"C:\Program Files (x86)")
                .join("NuGet")
                .join("Config")
        )
    );
    assert_eq!(
        at(Os::Linux, LINUX, None, NugetMachineWide),
        Located::ConfigDir(PathBuf::from("/etc/opt/NuGet/Config"))
    );
    assert_eq!(
        at(Os::Mac, LINUX, None, YarnRcYml),
        one("/home/u", &[".yarnrc.yml"])
    );
}
