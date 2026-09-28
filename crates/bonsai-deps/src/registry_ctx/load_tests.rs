//! `[HIGH-1]` `RegistryContext::load` tests: every step and error path of the
//! npm loader fails closed; a clean load opens npm only.

use std::fs;
use std::path::{Path, PathBuf};

use super::*;

const TOKEN: &str = "zz-sentinel-token";

struct Repo {
    _dir: tempfile::TempDir,
    root: PathBuf,
    /// The workdir: a subdirectory, so "above the workdir" exists and is ours.
    wd: PathBuf,
}

fn repo() -> Repo {
    let dir = tempfile::tempdir().unwrap();
    let root = fs::canonicalize(dir.path()).unwrap();
    let wd = root.join("wd");
    fs::create_dir_all(wd.join("packages/a")).unwrap();
    fs::write(wd.join("package.json"), "{}").unwrap();
    fs::write(wd.join("packages/a/package.json"), "{}").unwrap();
    Repo {
        _dir: dir,
        root,
        wd,
    }
}

fn manifests() -> Vec<String> {
    vec!["package.json".into(), "packages/a/package.json".into()]
}

fn load(r: &Repo, env: &FakeHostEnv) -> RegistryContext {
    RegistryContext::load(&r.wd, &manifests(), env)
}

fn npm_closed(r: &Repo, env: &FakeHostEnv) -> bool {
    load(r, env).fail_closed(Ecosystem::Npm)
}

const REPO_FILES: &[&str] = &[".npmrc", ".yarnrc", ".yarnrc.yml", "pnpm-workspace.yaml"];

/// A config that parses and names only public registries, per file format.
fn clean_config(name: &str) -> &'static str {
    match name {
        ".npmrc" => "registry=https://registry.npmjs.org/\n//registry.npmjs.org/:_authToken=zz-sentinel-token\n",
        ".yarnrc" => "registry \"https://registry.yarnpkg.com\"\n",
        ".yarnrc.yml" => "npmRegistryServer: \"https://registry.yarnpkg.com\"\nnpmAuthToken: zz-sentinel-token\n",
        _ => "packages:\n  - 'packages/*'\n",
    }
}

/// Content that the extractor for `name` rejects.
fn bad_config(name: &str) -> &'static str {
    match name {
        ".npmrc" => "registry=${CORP}\n",
        ".yarnrc" => "registry \"unterminated\n\"x",
        ".yarnrc.yml" => "npmRegistryServer: [unclosed\n",
        _ => "registries: &a\n  default: https://registry.npmjs.org/\n",
    }
}

#[test]
fn a_clean_load_opens_npm_and_only_npm() {
    let r = repo();
    for name in REPO_FILES {
        fs::write(r.wd.join(name), clean_config(name)).unwrap();
    }
    let ctx = load(&r, &FakeHostEnv::new());
    assert!(!ctx.fail_closed(Ecosystem::Npm));
    for eco in [
        Ecosystem::Cargo,
        Ecosystem::Pypi,
        Ecosystem::Go,
        Ecosystem::Nuget,
    ] {
        assert!(ctx.fail_closed(eco), "{eco:?} has no loader yet");
    }
    assert!(ctx.eco(Ecosystem::Npm).default_public);
}

#[test]
fn an_unusable_workdir_leaves_everything_closed() {
    let r = repo();
    let ctx = RegistryContext::load(&r.wd.join("missing"), &manifests(), &FakeHostEnv::new());
    for eco in Ecosystem::ALL {
        assert!(ctx.fail_closed(eco));
    }
}

#[test]
fn an_unclean_manifest_path_fails_closed() {
    let r = repo();
    for bad in ["../package.json", "/abs/package.json", "a/./package.json"] {
        let ctx = RegistryContext::load(&r.wd, &[bad.to_string()], &FakeHostEnv::new());
        assert!(ctx.fail_closed(Ecosystem::Npm), "{bad}");
    }
}

#[test]
fn every_repo_walk_file_fails_closed_on_bad_content() {
    for name in REPO_FILES {
        for dir in ["", "packages", "packages/a"] {
            let r = repo();
            fs::write(r.wd.join(dir).join(name), bad_config(name)).unwrap();
            assert!(npm_closed(&r, &FakeHostEnv::new()), "{dir}/{name}");
        }
    }
}

#[test]
fn every_repo_walk_file_fails_closed_when_not_a_file_or_oversize() {
    for name in REPO_FILES {
        let r = repo();
        fs::create_dir(r.wd.join(name)).unwrap();
        assert!(npm_closed(&r, &FakeHostEnv::new()), "dir {name}");

        let r = repo();
        let big = "#".repeat(usize::try_from(crate::fsread::MAX_CONFIG_BYTES).unwrap() + 1);
        fs::write(r.wd.join(name), big).unwrap();
        assert!(npm_closed(&r, &FakeHostEnv::new()), "oversize {name}");
    }
}

#[test]
fn a_symlinked_repo_config_fails_closed() {
    for name in REPO_FILES {
        let r = repo();
        let target = r.root.join("target.cfg");
        fs::write(&target, clean_config(name)).unwrap();
        if !crate::fsread::tests::try_symlink_file(&target, &r.wd.join(name)) {
            eprintln!("skipping: cannot create symlinks here");
            return;
        }
        assert!(npm_closed(&r, &FakeHostEnv::new()), "{name}");
    }
}

#[test]
fn configs_outside_the_manifest_ancestry_are_not_read() {
    let r = repo();
    fs::create_dir_all(r.wd.join("docs")).unwrap();
    fs::write(r.wd.join("docs/.npmrc"), bad_config(".npmrc")).unwrap();
    assert!(!npm_closed(&r, &FakeHostEnv::new()));
}

#[test]
fn a_nested_scope_config_is_found_by_the_ancestor_walk() {
    let r = repo();
    fs::write(
        r.wd.join("packages/a/.npmrc"),
        "@zzign:registry=https://u:zz-sentinel-token@ign.corp.invalid/\n",
    )
    .unwrap();
    let ctx = load(&r, &FakeHostEnv::new());
    let e = ctx.eco(Ecosystem::Npm);
    assert!(!e.fail_closed);
    assert_eq!(
        e.scoped.get("@zzign"),
        Some(&Some("ign.corp.invalid".into()))
    );
}

#[test]
fn above_workdir_yarnrc_yml_is_read_and_fails_closed() {
    let r = repo();
    let env = FakeHostEnv::new().above_unreadable(&r.root, ".yarnrc.yml");
    assert!(npm_closed(&r, &env));

    let env = FakeHostEnv::new().above(&r.root, ".yarnrc.yml", "npmRegistryServer: [x\n");
    assert!(npm_closed(&r, &env));

    // From disk: a real parent config routes the default registry.
    fs::write(
        r.root.join(".yarnrc.yml"),
        "npmRegistryServer: \"https://above.corp.invalid\"\n",
    )
    .unwrap();
    let ctx = load(&r, &FakeHostEnv::new().above_from_disk_within(&r.root));
    let e = ctx.eco(Ecosystem::Npm);
    assert!(!e.fail_closed);
    assert_eq!(e.default_host.as_deref(), Some("above.corp.invalid"));
}

#[test]
fn a_symlinked_above_workdir_config_fails_closed() {
    let r = repo();
    let target = r.wd.join("real.yml");
    fs::write(
        &target,
        "npmRegistryServer: \"https://registry.yarnpkg.com\"\n",
    )
    .unwrap();
    if !crate::fsread::tests::try_symlink_file(&target, &r.root.join(".yarnrc.yml")) {
        eprintln!("skipping: cannot create symlinks here");
        return;
    }
    assert!(npm_closed(
        &r,
        &FakeHostEnv::new().above_from_disk_within(&r.root)
    ));
}

const USER_FILES: &[(UserConfig, &str)] = &[
    (UserConfig::NpmUserRc, "registry=${X}\n"),
    (UserConfig::NpmGlobalRc, "@a:registry=nope\n"),
    (UserConfig::PnpmRc, "registry=file:///x\n"),
    (UserConfig::YarnRc, "\"broken\n"),
    (UserConfig::YarnRcYml, "a: &b 1\n"),
];

#[test]
fn every_user_file_fails_closed_when_unreadable_or_bad() {
    for (which, bad) in USER_FILES {
        let r = repo();
        assert!(
            npm_closed(&r, &FakeHostEnv::new().user_unreadable(*which)),
            "{which:?} unreadable"
        );
        assert!(
            npm_closed(&r, &FakeHostEnv::new().user_file(*which, bad)),
            "{which:?} bad"
        );
    }
}

#[test]
fn every_env_signal_fails_closed_when_bad() {
    for (k, v) in [
        ("NPM_CONFIG_REGISTRY", "${X}"),
        ("npm_config_@zzenv:registry", "not a url"),
        ("pnpm_config_registry", "//h/"),
        ("PNPM_CONFIG_@a:REGISTRY", "file:///x"),
        ("YARN_REGISTRY", "x"),
        ("YARN_NPM_REGISTRY_SERVER", "${Y}"),
        ("YARN_NPM_SCOPES", "{\"a\":{}}"),
        ("YARN_RC_FILENAME", ".corprc.yml"),
    ] {
        let r = repo();
        assert!(npm_closed(&r, &FakeHostEnv::new().var(k, v)), "{k}");
    }
}

#[test]
fn env_loader_never_reads_auth_values() {
    let r = repo();
    let env = FakeHostEnv::new()
        .var("npm_config_//npm.corp.invalid/:_authToken", TOKEN)
        .var("NPM_TOKEN", TOKEN)
        .var("pnpm_config__auth", TOKEN)
        .var("YARN_NPM_AUTH_TOKEN", TOKEN)
        .var("npm_config_@zzenv:registry", "https://env.corp.invalid/");
    let ctx = load(&r, &env);
    let e = ctx.eco(Ecosystem::Npm);
    assert!(!e.fail_closed);
    assert!(e.scoped.contains_key("@zzenv"));
    assert_eq!(
        env.values_read(),
        vec!["npm_config_@zzenv:registry".to_string()]
    );
}

#[test]
fn loaded_hosts_never_carry_userinfo() {
    let r = repo();
    fs::write(
        r.wd.join(".npmrc"),
        "registry=https://u:zz-sentinel-token@mirror.corp.invalid:444/npm/\n",
    )
    .unwrap();
    let ctx = load(&r, &FakeHostEnv::new());
    let e = ctx.eco(Ecosystem::Npm);
    let host = e.default_host.clone().unwrap();
    assert_eq!(host, "mirror.corp.invalid");
    assert!(!host.contains('@') && !host.contains(TOKEN));
}

#[test]
fn the_real_load_signature_matches_the_contract() {
    // §4.4: `load(workdir: &Path, manifests: &[String], env: &dyn HostEnv)`.
    let f: fn(&Path, &[String], &dyn HostEnv) -> RegistryContext = RegistryContext::load;
    let _ = f;
}
