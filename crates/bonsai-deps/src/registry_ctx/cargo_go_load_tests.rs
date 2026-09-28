//! `[HIGH-1]` `[Q7]` the Cargo half of `RegistryContext::load` (Go: `go_load_tests.rs`):
//! every read path and extractor error fails closed; the key filter keeps
//! tokens out; replace-with / named registries / GOPRIVATE route private.

use std::fs;
use std::path::PathBuf;

use super::*;
use crate::fixtures::cargo::{CLEAN_CONFIG, REPLACE_CONFIG};

struct Repo {
    _dir: tempfile::TempDir,
    root: PathBuf,
    wd: PathBuf,
}

fn repo() -> Repo {
    let dir = tempfile::tempdir().unwrap();
    let root = fs::canonicalize(dir.path()).unwrap();
    let wd = root.join("wd");
    fs::create_dir_all(wd.join("crates/a")).unwrap();
    Repo {
        _dir: dir,
        root,
        wd,
    }
}

fn manifests() -> Vec<String> {
    vec!["Cargo.toml".into(), "crates/a/Cargo.toml".into()]
}

fn load(r: &Repo, env: &FakeHostEnv) -> RegistryContext {
    RegistryContext::load(&r.wd, &manifests(), env)
}

fn cargo_closed(r: &Repo, env: &FakeHostEnv) -> bool {
    load(r, env).fail_closed(Ecosystem::Cargo)
}

const DEFAULT: DepSource = DepSource::Registry(RegistryRef::Default);

/// Configs the Cargo extractor must reject (each alone closes Cargo).
const BAD_CONFIGS: &[&str] = &[
    "[source.crates-io\n",
    "[source.corp]\nregistry = \"ssh://git@corp.invalid/index\"\n",
    "[registries.corp]\nindex = \"file:///srv/index\"\n",
    "[source.corp]\nregistry = 7\n",
    "include = [\"corp.toml\"]\n",
    "[patch.crates-io]\nfoo = { path = \"../foo\" }\n",
    "paths = [\"/src/foo\"]\n",
    "[registry]\ndefault = \"corp\"\n",
];

fn write(r: &Repo, rel: &str, body: &str) {
    let p = r.wd.join(rel);
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(p, body).unwrap();
}

#[test]
fn no_config_opens_cargo_and_go_publicly() {
    let r = repo();
    let ctx = load(&r, &FakeHostEnv::new());
    for eco in [Ecosystem::Cargo, Ecosystem::Go] {
        assert!(!ctx.fail_closed(eco), "{eco:?}");
        assert_eq!(ctx.private_reason(eco, "serde", &DEFAULT), None);
    }
}

#[test]
fn a_clean_config_with_a_registry_token_stays_open() {
    let r = repo();
    write(&r, ".cargo/config.toml", CLEAN_CONFIG);
    let env = FakeHostEnv::new().user_file(UserConfig::CargoHomeConfigToml, CLEAN_CONFIG);
    let ctx = load(&r, &env);
    assert!(!ctx.fail_closed(Ecosystem::Cargo));
    assert!(!ctx.eco(Ecosystem::Cargo).replace_all);
}

#[test]
fn replace_with_in_the_repo_walk_makes_every_crate_private() {
    let r = repo();
    // Only in the member's directory: the manifest-ancestor walk finds it.
    write(&r, "crates/a/.cargo/config.toml", REPLACE_CONFIG);
    let ctx = load(&r, &FakeHostEnv::new());
    let e = ctx.eco(Ecosystem::Cargo);
    assert!(!e.fail_closed && e.replace_all);
    assert_eq!(e.default_host.as_deref(), Some("cargo.corp.invalid"));
    assert_eq!(
        e.custom_hosts.get("other"),
        Some(&Some("other.corp.invalid".to_string()))
    );
    assert_eq!(
        ctx.private_reason(Ecosystem::Cargo, "serde", &DEFAULT),
        Some((
            PrivateReason::CustomRegistry,
            Some("cargo.corp.invalid".into())
        ))
    );
}

#[test]
fn the_legacy_extensionless_config_is_read() {
    let r = repo();
    write(&r, ".cargo/config", REPLACE_CONFIG);
    assert!(
        load(&r, &FakeHostEnv::new())
            .eco(Ecosystem::Cargo)
            .replace_all
    );
}

#[test]
fn every_bad_config_fails_closed_at_every_level() {
    for bad in BAD_CONFIGS {
        let r = repo();
        write(&r, ".cargo/config.toml", bad);
        assert!(cargo_closed(&r, &FakeHostEnv::new()), "repo: {bad}");
        let r = repo();
        let above = FakeHostEnv::new().above(&r.root, ".cargo/config.toml", bad);
        assert!(cargo_closed(&r, &above), "above: {bad}");
        for which in [UserConfig::CargoHomeConfigToml, UserConfig::CargoHomeConfig] {
            let env = FakeHostEnv::new().user_file(which, bad);
            assert!(cargo_closed(&r, &env), "{which:?}: {bad}");
        }
    }
    // [HIGH-2] a nesting bomb never reaches the toml parser.
    let r = repo();
    write(
        &r,
        ".cargo/config.toml",
        &format!("x = {}", "[".repeat(100_000)),
    );
    assert!(cargo_closed(&r, &FakeHostEnv::new()));
}

#[test]
fn unreadable_or_non_file_configs_fail_closed() {
    let r = repo();
    fs::create_dir_all(r.wd.join(".cargo/config.toml")).unwrap();
    assert!(cargo_closed(&r, &FakeHostEnv::new()));
    let r = repo();
    for name in [".cargo/config.toml", ".cargo/config"] {
        let env = FakeHostEnv::new().above_unreadable(&r.root, name);
        assert!(cargo_closed(&r, &env), "{name}");
    }
    for which in [UserConfig::CargoHomeConfigToml, UserConfig::CargoHomeConfig] {
        assert!(cargo_closed(&r, &FakeHostEnv::new().user_unreadable(which)));
    }
}

#[test]
fn q7_the_above_workdir_walk_reaches_every_ancestor() {
    // [Q7] a config two levels above the workdir, served from disk.
    let r = repo();
    let deep = r.root.join("outer/inner");
    fs::create_dir_all(deep.join(".cargo")).unwrap();
    fs::create_dir_all(r.root.join(".cargo")).unwrap();
    fs::write(r.root.join(".cargo/config.toml"), REPLACE_CONFIG).unwrap();
    let wd = deep.join("wd");
    fs::create_dir_all(&wd).unwrap();
    let env = FakeHostEnv::new().above_from_disk_within(&r.root);
    let ctx = RegistryContext::load(&wd, &manifests(), &env);
    let e = ctx.eco(Ecosystem::Cargo);
    assert!(!e.fail_closed && e.replace_all);
    assert_eq!(e.default_host.as_deref(), Some("cargo.corp.invalid"));
}

#[test]
fn q9_symlinks_above_the_workdir_are_followed_only_in_home_or_cargo_home() {
    let r = repo();
    let target = r.root.join("elsewhere.toml");
    fs::write(&target, REPLACE_CONFIG).unwrap();
    fs::create_dir_all(r.root.join(".cargo")).unwrap();
    if !crate::fsread::tests::try_symlink_file(&target, &r.root.join(".cargo/config.toml")) {
        eprintln!("skipping: cannot create symlinks here");
        return;
    }
    let not_home = FakeHostEnv::new().above_from_disk_within(&r.root);
    assert!(
        cargo_closed(&r, &not_home),
        "[Q9-narrowed] non-home ancestor"
    );
    let env = FakeHostEnv::new()
        .above_from_disk_within(&r.root)
        .follow_root(&r.root);
    let ctx = load(&r, &env);
    assert!(
        !ctx.fail_closed(Ecosystem::Cargo),
        "symlinked file followed"
    );
    assert!(ctx.eco(Ecosystem::Cargo).replace_all);
    // A linked `.cargo` DIRECTORY is followed too.
    let r = repo();
    let real = r.root.join("real-cargo");
    fs::create_dir_all(&real).unwrap();
    fs::write(real.join("config.toml"), REPLACE_CONFIG).unwrap();
    assert!(crate::fsread::tests::try_symlink_dir(
        &real,
        &r.root.join(".cargo")
    ));
    let home = FakeHostEnv::new()
        .above_from_disk_within(&r.root)
        .follow_root(&r.root);
    let ctx = load(&r, &home);
    assert!(ctx.eco(Ecosystem::Cargo).replace_all);
    // A config that links to a directory, or dangles, fails closed.
    let r = repo();
    fs::create_dir_all(r.root.join(".cargo")).unwrap();
    fs::create_dir_all(r.root.join("a-dir")).unwrap();
    assert!(crate::fsread::tests::try_symlink_file(
        &r.root.join("a-dir"),
        &r.root.join(".cargo/config.toml")
    ));
    let env = FakeHostEnv::new()
        .above_from_disk_within(&r.root)
        .follow_root(&r.root);
    assert!(cargo_closed(&r, &env), "link to a directory");
    assert!(crate::fsread::tests::try_symlink_file(
        &r.root.join("gone.toml"),
        &r.root.join(".cargo/config")
    ));
    fs::remove_file(r.root.join(".cargo/config.toml")).unwrap();
    assert!(cargo_closed(&r, &env), "dangling link");
}

#[test]
fn a_replacement_resolving_to_the_public_index_is_not_private() {
    let r = repo();
    write(
        &r,
        ".cargo/config.toml",
        "[source.crates-io]\nreplace-with = \"sparse-io\"\n[source.sparse-io]\nregistry = \"sparse+https://index.crates.io/\"\n",
    );
    let ctx = load(&r, &FakeHostEnv::new());
    let e = ctx.eco(Ecosystem::Cargo);
    assert!(!e.fail_closed && !e.replace_all && e.default_public);
    // Mixed with a private one, the private one still wins.
    let env = FakeHostEnv::new()
        .var("CARGO_SOURCE_CRATES_IO_REPLACE_WITH", "corp")
        .var("CARGO_REGISTRIES_CORP_INDEX", "https://cargo.corp.invalid/");
    let ctx = load(&r, &env);
    assert!(ctx.eco(Ecosystem::Cargo).replace_all);
}

#[test]
fn cargo_env_signals_route_and_fail_closed() {
    let r = repo();
    let env = FakeHostEnv::new()
        .var("CARGO_SOURCE_CRATES_IO_REPLACE_WITH", "corp")
        .var(
            "CARGO_REGISTRIES_CORP_INDEX",
            "sparse+https://env.corp.invalid/",
        )
        .var("CARGO_REGISTRIES_CORP_TOKEN", "zz-sentinel-token")
        .var("CARGO_REGISTRY_TOKEN", "zz-sentinel-token")
        .var(
            "CARGO_REGISTRIES_CORP_CREDENTIAL_PROVIDER",
            "zz-sentinel-token",
        )
        .var("CARGO_HOME", "/home/x/.cargo");
    let ctx = load(&r, &env);
    let e = ctx.eco(Ecosystem::Cargo);
    assert!(!e.fail_closed && e.replace_all);
    // `replace-with = "corp"` resolves through [registries.corp].
    assert_eq!(e.default_host.as_deref(), Some("env.corp.invalid"));
    let mut read = env.values_read();
    read.sort();
    assert_eq!(
        read,
        [
            "CARGO_REGISTRIES_CORP_INDEX",
            "CARGO_SOURCE_CRATES_IO_REPLACE_WITH"
        ]
    );
    for (k, v) in [
        ("CARGO_REGISTRIES_CORP_INDEX", "ssh://git@corp/index"),
        ("cargo_registries_x_index", "not a url"),
        ("CARGO_REGISTRY_DEFAULT", "corp"),
    ] {
        assert!(cargo_closed(&r, &FakeHostEnv::new().var(k, v)), "{k}");
    }
    assert!(!cargo_closed(
        &r,
        &FakeHostEnv::new().var("CARGO_REGISTRY_DEFAULT", "crates_io")
    ));
}

#[test]
fn named_registry_names_match_across_spellings() {
    let r = repo();
    let env = FakeHostEnv::new().var("CARGO_REGISTRIES_MY_CORP_INDEX", "https://my.corp.invalid/");
    let ctx = load(&r, &env);
    let named = DepSource::Registry(RegistryRef::Named(crate::eco::cargo::registry_key(
        "my-corp",
    )));
    assert_eq!(
        ctx.private_reason(Ecosystem::Cargo, "x", &named),
        Some((
            PrivateReason::CustomRegistry,
            Some("my.corp.invalid".into())
        ))
    );
}

#[test]
fn a_vendored_replacement_is_private_without_a_host() {
    let r = repo();
    write(
        &r,
        ".cargo/config.toml",
        "[source.crates-io]\nreplace-with = \"vendored-sources\"\n\n[source.vendored-sources]\ndirectory = \"vendor\"\n",
    );
    let ctx = load(&r, &FakeHostEnv::new());
    let e = ctx.eco(Ecosystem::Cargo);
    assert!(e.replace_all);
    assert_eq!(e.default_host, None);
    // A replace-with cycle terminates.
    let r = repo();
    write(
        &r,
        ".cargo/config.toml",
        "[source.crates-io]\nreplace-with = \"a\"\n[source.a]\nreplace-with = \"b\"\n[source.b]\nreplace-with = \"a\"\n",
    );
    assert_eq!(
        load(&r, &FakeHostEnv::new())
            .eco(Ecosystem::Cargo)
            .default_host,
        None
    );
}

#[test]
fn q7_a_non_directory_config_component_above_the_workdir_fails_closed() {
    // Runs on every OS (no symlink privilege needed): `.cargo` is a FILE,
    // so the intermediate-component check of the no-follow read rejects it.
    let r = repo();
    fs::write(r.root.join(".cargo"), "not a directory").unwrap();
    let env = FakeHostEnv::new().above_from_disk_within(&r.root);
    assert!(cargo_closed(&r, &env));
}

#[test]
fn a_symlinked_cargo_home_dir_without_the_legacy_file_stays_open() {
    // [Q9-2] `~/.cargo` is a symlinked directory holding only config.toml:
    // the missing legacy `config` is Absent, not dangling.
    let r = repo();
    let real = r.root.join("real-cargo-home");
    fs::create_dir_all(&real).unwrap();
    fs::write(real.join("config.toml"), REPLACE_CONFIG).unwrap();
    let home = r.root.join("home");
    fs::create_dir_all(&home).unwrap();
    if !crate::fsread::tests::try_symlink_dir(&real, &home.join(".cargo")) {
        eprintln!("skipping: cannot create dir symlinks here");
        return;
    }
    let env = FakeHostEnv::new()
        .user_file_from_disk(
            UserConfig::CargoHomeConfigToml,
            &home.join(".cargo/config.toml"),
        )
        .user_file_from_disk(UserConfig::CargoHomeConfig, &home.join(".cargo/config"));
    let ctx = load(&r, &env);
    let e = ctx.eco(Ecosystem::Cargo);
    assert!(!e.fail_closed && e.replace_all);
}

#[test]
fn a_nearer_level_redefining_a_public_source_is_not_masked() {
    // Review round 3: the root maps crates-io to a PUBLIC mirror name; a
    // deeper config redefines that name to a corp registry. Cargo's nearest
    // wins, so crates would route to corp: every definition counts.
    let r = repo();
    write(
        &r,
        ".cargo/config.toml",
        "[source.crates-io]\nreplace-with = \"m\"\n[source.m]\nregistry = \"sparse+https://index.crates.io/\"\n",
    );
    write(
        &r,
        "crates/a/.cargo/config.toml",
        "[source.m]\nregistry = \"https://corp.invalid/index\"\n",
    );
    let ctx = load(&r, &FakeHostEnv::new());
    let e = ctx.eco(Ecosystem::Cargo);
    assert!(e.replace_all);
    assert_eq!(e.default_host.as_deref(), Some("corp.invalid"));
    // Without the deeper redefinition the public mirror stays public.
    fs::remove_file(r.wd.join("crates/a/.cargo/config.toml")).unwrap();
    assert!(
        !load(&r, &FakeHostEnv::new())
            .eco(Ecosystem::Cargo)
            .replace_all
    );
}

#[test]
fn definitions_from_the_user_file_and_env_are_all_kept() {
    let r = repo();
    // The user file maps crates-io → `m` (public); env defines registry
    // `m` as corp. Either alone would be judged differently.
    let user = "[source.crates-io]\nreplace-with = \"m\"\n[source.m]\nregistry = \"sparse+https://index.crates.io/\"\n";
    let env = FakeHostEnv::new()
        .user_file(UserConfig::CargoHomeConfigToml, user)
        .var("CARGO_REGISTRIES_M_INDEX", "https://env.corp.invalid/");
    let e = load(&r, &env);
    assert!(e.eco(Ecosystem::Cargo).replace_all);
    assert_eq!(
        e.eco(Ecosystem::Cargo).default_host.as_deref(),
        Some("env.corp.invalid")
    );
    // Two levels disagree on a named registry: the non-public host shows.
    write(
        &r,
        ".cargo/config.toml",
        "[registries.corp]\nindex = \"https://a.corp.invalid/\"\n",
    );
    let env = FakeHostEnv::new().var("CARGO_REGISTRIES_CORP_INDEX", "https://index.crates.io/");
    let ctx = load(&r, &env);
    assert_eq!(
        ctx.eco(Ecosystem::Cargo).custom_hosts.get("corp"),
        Some(&Some("a.corp.invalid".to_string()))
    );
}
