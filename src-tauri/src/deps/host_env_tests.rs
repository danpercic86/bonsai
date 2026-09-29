//! The real `HostEnv`'s IO half: env lookups over `vars_os()`, the folded
//! multi-file read, the NuGet machine-wide set, and the canonical follow
//! roots (`[P125c1 review]`: canonicalized like the workdir ancestors).

use std::ffi::OsString;
use std::fs;

use bonsai_deps::fsread::read_above_workdir_config;
use bonsai_deps::registry_ctx::{HostEnv, UserRead};

use super::host_env::*;
use super::host_paths::{network_profile, Inputs, Os};
use super::test_support::scratch;

fn text(r: UserRead) -> String {
    match r {
        UserRead::Content(s) => s,
        other => panic!("expected Content, got {other:?}"),
    }
}

#[test]
fn env_lookups_match_keys_case_insensitively_and_filter_before_values() {
    // Read-only over the `CARGO_PKG_*` variables cargo / nextest set for the
    // test process: no env mutation (a data race with other threads' reads).
    let env = RealHostEnv::new();
    assert_eq!(
        env.var("cargo_pkg_name").as_deref(),
        Some(env!("CARGO_PKG_NAME"))
    );
    assert!(env.var_present_ci("Cargo_Pkg_Name"));
    assert!(!env.var_present_ci("BONSAI_P125E_ABSENT"));
    assert_eq!(env.var("BONSAI_P125E_ABSENT"), None);
    // The key filter decides: only `…_MAJOR` of the `CARGO_PKG_VERSION_*` set.
    let kept = env.vars_with_prefix_ci("cargo_pkg_version_", |k| k.ends_with("MAJOR"));
    assert_eq!(
        kept,
        vec![(
            "CARGO_PKG_VERSION_MAJOR".to_owned(),
            env!("CARGO_PKG_VERSION_MAJOR").to_owned()
        )]
    );
}

#[test]
fn fold_reads_every_candidate_and_fails_closed_on_any_unreadable() {
    let dir = scratch();
    let d = fs::canonicalize(dir.path()).expect("canonical");
    fs::write(
        d.join("a.conf"),
        "[global]\nindex-url = https://a.invalid/\n",
    )
    .expect("a");
    fs::write(
        d.join("b.conf"),
        "[global]\nextra-index-url = https://b.invalid/\n",
    )
    .expect("b");
    fs::create_dir_all(d.join("dir.conf")).expect("dir");
    assert!(matches!(fold(&[d.join("missing")]), UserRead::Absent));
    let one = text(fold(&[d.join("missing"), d.join("a.conf")]));
    assert!(one.contains("a.invalid") && !one.contains("b.invalid"));
    let both = text(fold(&[d.join("a.conf"), d.join("b.conf")]));
    assert!(both.contains("a.invalid") && both.contains("b.invalid"));
    // A directory where a config file should be ⇒ the whole kind closes.
    assert!(matches!(
        fold(&[d.join("a.conf"), d.join("dir.conf")]),
        UserRead::Unreadable
    ));
}

#[test]
fn the_machine_wide_set_is_every_config_file_sorted_and_capped() {
    let dir = scratch();
    let d = fs::canonicalize(dir.path()).expect("canonical");
    assert!(read_config_dir(&d.join("missing")).is_empty());
    fs::write(d.join("b.config"), "<configuration/>").expect("b");
    fs::write(d.join("A.CONFIG"), "<configuration>A</configuration>").expect("a");
    fs::write(d.join("notes.txt"), "ignored").expect("txt");
    let set = read_config_dir(&d);
    assert_eq!(set.len(), 2);
    let mut it = set.into_iter();
    assert!(text(it.next().expect("first")).contains('A'));
    for i in 0..MAX_MACHINE_CONFIGS {
        fs::write(d.join(format!("x{i}.config")), "<configuration/>").expect("x");
    }
    assert!(matches!(
        read_config_dir(&d).as_slice(),
        [UserRead::Unreadable]
    ));
}

#[test]
fn follow_roots_are_canonical_like_workdir_ancestors() {
    let dir = scratch();
    let home = dir.path().join("home");
    fs::create_dir_all(&home).expect("home");
    let roots =
        canonical_roots(vec![home.clone(), home.clone(), dir.path().join("gone")].into_iter());
    let canon = fs::canonicalize(&home).expect("canonical");
    assert_eq!(roots, vec![canon.clone()], "deduped, missing dropped");
    // The comparison `read_above_workdir_config` makes: an ancestor from a
    // canonical workdir equals the root, so a config there is read.
    let workdir = canon.join("repo");
    fs::create_dir_all(&workdir).expect("workdir");
    fs::write(canon.join(".npmrc"), "registry=https://x.invalid/\n").expect("rc");
    let wd = fs::canonicalize(&workdir).expect("canonical workdir");
    let parent = wd.parent().expect("parent");
    assert_eq!(parent, canon.as_path());
    assert!(matches!(
        read_above_workdir_config(parent, ".npmrc", 1024, &roots),
        bonsai_deps::fsread::ReadOutcome::Content(_)
    ));
}

/// `[P125h §4b]` the pure detector behind `RealHostEnv::profile_on_network`
/// (OS and variables injected, so every row runs on any host).
fn on_network(os: Os, vars: &[(&str, &str)]) -> bool {
    let var = |k: &str| {
        vars.iter()
            .find(|(n, _)| *n == k)
            .map(|(_, v)| OsString::from(v))
    };
    let no_node = || None;
    network_profile(&Inputs {
        os,
        var: &var,
        node_dir: &no_node,
    })
}

const WIN_LOCAL: &[(&str, &str)] = &[
    ("USERPROFILE", r"C:\Users\u"),
    ("APPDATA", r"C:\Users\u\AppData\Roaming"),
    ("LOCALAPPDATA", r"C:\Users\u\AppData\Local"),
    ("PROGRAMDATA", r"C:\ProgramData"),
    ("ProgramFiles(x86)", r"C:\Program Files (x86)"),
];

/// `WIN_LOCAL` with `key` set to (or replaced by) `value`.
fn win_with(key: &'static str, value: &'static str) -> Vec<(&'static str, &'static str)> {
    let mut v: Vec<(&str, &str)> = WIN_LOCAL
        .iter()
        .filter(|(k, _)| *k != key)
        .copied()
        .collect();
    v.push((key, value));
    v
}

#[test]
fn a_unc_profile_dir_is_a_network_profile() {
    assert!(on_network(
        Os::Windows,
        &win_with("APPDATA", r"\\srv\p\AppData\Roaming")
    ));
    assert!(on_network(
        Os::Windows,
        &win_with("USERPROFILE", r"\\srv\home\u")
    ));
    assert!(on_network(
        Os::Windows,
        &win_with("LOCALAPPDATA", r"\\?\UNC\srv\p\AppData\Local")
    ));
    assert!(on_network(
        Os::Windows,
        &win_with("CARGO_HOME", "//srv/cargo")
    ));
    assert!(on_network(
        Os::Linux,
        &[("HOME", "/home/u"), ("XDG_CONFIG_HOME", "//srv/cfg")]
    ));
}

#[test]
fn a_local_profile_is_not_a_network_profile() {
    assert!(!on_network(Os::Windows, WIN_LOCAL));
    // A verbatim LOCAL drive is not a network path.
    assert!(!on_network(
        Os::Windows,
        &win_with("USERPROFILE", r"\\?\C:\Users\u")
    ));
    assert!(!on_network(Os::Linux, &[("HOME", "/home/u")]));
    assert!(!on_network(Os::Mac, &[("HOME", "/Users/u")]));
    // Nothing resolvable ⇒ no claim (the configs are Unresolvable instead).
    assert!(!on_network(Os::Windows, &[]));
    // LOW-E1 (deferred): a mapped network drive letter is NOT detected — a
    // false negative by design, never a false positive.
    assert!(!on_network(
        Os::Windows,
        &win_with("APPDATA", r"H:\AppData\Roaming")
    ));
}

#[test]
fn overrides_and_system_dirs_on_a_share_do_not_set_the_flag() {
    // Only USER settings folders count; these still fail their own
    // ecosystem closed (`configUnreadable` rows), just not report-wide.
    for (k, v) in [
        ("PIP_CONFIG_FILE", r"\\srv\cfg\pip.ini"),
        ("VIRTUAL_ENV", r"\\srv\venv"),
        ("GOENV", r"\\srv\go\env"),
        ("NPM_CONFIG_USERCONFIG", r"\\srv\npm\.npmrc"),
        ("NPM_CONFIG_PREFIX", r"\\srv\npm"),
        ("PROGRAMDATA", r"\\srv\ProgramData"),
        ("ProgramFiles(x86)", r"\\srv\Program Files (x86)"),
    ] {
        assert!(!on_network(Os::Windows, &win_with(k, v)), "{k}");
    }
    assert!(!on_network(
        Os::Linux,
        &[("HOME", "/home/u"), ("VIRTUAL_ENV", "//srv/venv")]
    ));
    // …while a UNC user base alongside them still does.
    let mut v = win_with("PIP_CONFIG_FILE", r"\\srv\cfg\pip.ini");
    v.retain(|(k, _)| *k != "APPDATA");
    v.push(("APPDATA", r"\\srv\p\AppData\Roaming"));
    assert!(on_network(Os::Windows, &v));
}
