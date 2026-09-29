//! `[LOW-2]` name shapes, `[HIGH-1]` fail-closed classification and local
//! names (E4 "privacy_tests").

use super::*;
use crate::adapter::RegistryRef;
use crate::registry_ctx::{HostEnv, NugetMapping, UserConfig, UserRead};
use std::path::Path;

const DEFAULT: DepSource = DepSource::Registry(RegistryRef::Default);

fn open() -> RegistryContext {
    RegistryContext::no_signals()
}

fn cls(eco: Ecosystem, name: &str, src: &DepSource, reg: &RegistryContext) -> Visibility {
    classify(eco, name, Some("1.0.0"), src, reg, &LocalNames::new())
}

fn is_public(v: &Visibility) -> bool {
    matches!(v, Visibility::Public(_))
}

// ---- LOW-2: dot segments are never Public --------------------------------

#[test]
fn low2_dot_segments_are_not_public() {
    let reg = open();
    for (eco, name) in [
        (Ecosystem::Npm, ".."),
        (Ecosystem::Npm, "."),
        (Ecosystem::Npm, "@scope/.."),
        (Ecosystem::Go, "foo/../bar"),
        (Ecosystem::Go, "example.com/../bar"),
        (Ecosystem::Go, "../example.com"),
        (Ecosystem::Go, ".."),
        (Ecosystem::Go, "example.com/./x"),
        (Ecosystem::Nuget, ".."),
        (Ecosystem::Nuget, "."),
        (Ecosystem::Pypi, ".."),
        (Ecosystem::Cargo, ".."),
    ] {
        assert!(!valid_public_name(eco, name), "{eco:?} {name:?}");
        assert!(
            !is_public(&cls(eco, name, &DEFAULT, &reg)),
            "{eco:?} {name:?}"
        );
    }
}

#[test]
fn go_first_element_needs_dot_and_alnum() {
    assert!(valid_public_name(Ecosystem::Go, "github.com/foo/bar"));
    assert!(!valid_public_name(Ecosystem::Go, "localmod/pkg"));
    assert!(!valid_public_name(Ecosystem::Go, "./x"));
    assert!(!valid_public_name(Ecosystem::Go, "-.-/x"));
    assert!(!valid_public_name(Ecosystem::Go, "a//b.com"));
}

#[test]
fn public_name_shapes() {
    let ok = [
        (Ecosystem::Npm, "react"),
        (Ecosystem::Npm, "@types/node"),
        (Ecosystem::Npm, "lodash.merge"),
        (Ecosystem::Cargo, "serde_json"),
        (Ecosystem::Pypi, "zope-interface"),
        (Ecosystem::Nuget, "Newtonsoft.Json"),
    ];
    for (eco, n) in ok {
        assert!(valid_public_name(eco, n), "{eco:?} {n:?}");
    }
    let bad = [
        (Ecosystem::Npm, "React"),
        (Ecosystem::Npm, "@scope"),
        (Ecosystem::Npm, "a/b"),
        (Ecosystem::Npm, ".hidden"),
        (Ecosystem::Cargo, "a.b"),
        (Ecosystem::Pypi, "-x"),
        (Ecosystem::Nuget, "a/b"),
        (Ecosystem::Nuget, "a b"),
    ];
    for (eco, n) in bad {
        assert!(!valid_public_name(eco, n), "{eco:?} {n:?}");
    }
    assert!(!valid_public_name(Ecosystem::Npm, &"a".repeat(215)));
    assert!(!valid_public_name(Ecosystem::Cargo, &"a".repeat(65)));
    assert!(!valid_public_name(Ecosystem::Nuget, &"a".repeat(101)));
}

#[test]
fn public_carries_normalized_name_and_filtered_version() {
    let reg = open();
    let local = LocalNames::new();
    let v = classify(
        Ecosystem::Pypi,
        "Zope.Interface",
        Some("5.0"),
        &DEFAULT,
        &reg,
        &local,
    );
    match v {
        Visibility::Public(p) => {
            assert_eq!(p.name(), "zope-interface");
            assert_eq!(p.version(), Some("5.0"));
            assert_eq!(p.ecosystem(), Ecosystem::Pypi);
        }
        other => panic!("expected Public, got {other:?}"),
    }
    let v = classify(
        Ecosystem::Npm,
        "react",
        Some("1.0 ; rm"),
        &DEFAULT,
        &reg,
        &local,
    );
    assert!(matches!(v, Visibility::Public(p) if p.version().is_none()));
}

// ---- HIGH-1 item 2: fail closed ------------------------------------------

struct NoEnv;
impl HostEnv for NoEnv {
    fn var(&self, _: &str) -> Option<String> {
        None
    }
    fn var_present_ci(&self, _: &str) -> bool {
        false
    }
    fn vars_with_prefix_ci(&self, _: &str, _: fn(&str) -> bool) -> Vec<(String, String)> {
        Vec::new()
    }
    fn read_user_file(&self, _: UserConfig) -> UserRead {
        UserRead::Absent
    }
    fn read_user_files(&self, _: UserConfig) -> Vec<UserRead> {
        Vec::new()
    }
    fn read_above_workdir(&self, _: &Path, _: &str) -> UserRead {
        UserRead::Absent
    }
    fn profile_on_network(&self) -> bool {
        false
    }
}

#[test]
fn loader_fails_closed_for_every_ecosystem_without_a_workdir() {
    // P125b: the real loader. An unusable workdir leaves every ecosystem
    // closed; ecosystems without a loader yet (Cargo, PyPI, Go, NuGet until
    // P125c1/c2) stay closed even with one (see registry_ctx load_tests).
    let reg = RegistryContext::load(Path::new("./does-not-exist-p125b"), &[], &NoEnv);
    for eco in Ecosystem::ALL {
        assert!(reg.fail_closed(eco));
        assert_eq!(
            cls(eco, "somepkg.io", &DEFAULT, &reg),
            Visibility::Private(PrivateReason::ConfigUnreadable, None),
            "{eco:?}"
        );
    }
}

#[test]
fn fail_closed_beats_everything_but_local() {
    let mut reg = open();
    reg.eco_mut(Ecosystem::Npm).fail_closed = true;
    let host = DepSource::Registry(RegistryRef::Host("registry.npmjs.org".into()));
    assert_eq!(
        cls(Ecosystem::Npm, "react", &host, &reg),
        Visibility::Private(PrivateReason::ConfigUnreadable, None)
    );
    // Other ecosystems are unaffected.
    assert!(is_public(&cls(Ecosystem::Cargo, "serde", &DEFAULT, &reg)));
    // Non-registry sources are still Local (never sent either way).
    assert_eq!(
        cls(Ecosystem::Npm, "x", &DepSource::Path, &reg),
        Visibility::Local(LocalReason::Path)
    );
}

#[test]
fn user_read_debug_never_prints_content() {
    let dbg = format!(
        "{:?}",
        UserRead::Content("//h/:_authToken=zz-sentinel-token".into())
    );
    assert_eq!(dbg, "Content(..)");
}

// ---- E2 private signals ---------------------------------------------------

#[test]
fn private_signals() {
    let mut reg = open();
    reg.eco_mut(Ecosystem::Npm)
        .scoped
        .insert("@zzsentinel".into(), Some("npm.corp.invalid".into()));
    reg.eco_mut(Ecosystem::Go)
        .go_private_globs
        .push("corp.invalid".into());
    reg.eco_mut(Ecosystem::Cargo)
        .custom_hosts
        .insert("corp".into(), Some("cargo.corp.invalid".into()));
    let npm = |n: &str, s: &DepSource| cls(Ecosystem::Npm, n, s, &reg);
    assert_eq!(
        npm("@ZZSentinel/scoped", &DEFAULT),
        Visibility::Private(
            PrivateReason::ScopedRegistry,
            Some("npm.corp.invalid".into())
        )
    );
    let corp_host = DepSource::Registry(RegistryRef::Host("npm.corp.invalid".into()));
    assert_eq!(
        npm("left-pad", &corp_host),
        Visibility::Private(
            PrivateReason::CustomRegistry,
            Some("npm.corp.invalid".into())
        )
    );
    let pub_host = DepSource::Registry(RegistryRef::Host("registry.npmjs.org".into()));
    assert!(is_public(&npm("left-pad", &pub_host)));
    assert_eq!(
        cls(Ecosystem::Go, "corp.invalid/zzsentinel/mod", &DEFAULT, &reg),
        Visibility::Private(PrivateReason::GoPrivate, None)
    );
    let named = DepSource::Registry(RegistryRef::Named("corp".into()));
    assert_eq!(
        cls(Ecosystem::Cargo, "zz-sentinel-reg", &named, &reg),
        Visibility::Private(
            PrivateReason::CustomRegistry,
            Some("cargo.corp.invalid".into())
        )
    );
}

#[test]
fn non_public_default_and_replace_all() {
    let mut reg = open();
    {
        let e = reg.eco_mut(Ecosystem::Npm);
        e.default_public = false;
        e.default_host = Some("npm.corp.invalid".into());
    }
    let pub_host = DepSource::Registry(RegistryRef::Host("registry.npmjs.org".into()));
    assert!(matches!(
        cls(Ecosystem::Npm, "left-pad", &DEFAULT, &reg),
        Visibility::Private(PrivateReason::CustomRegistry, _)
    ));
    // A public lock URL still identifies the package as public (E2 npm).
    assert!(is_public(&cls(Ecosystem::Npm, "left-pad", &pub_host, &reg)));

    reg.eco_mut(Ecosystem::Cargo).replace_all = true;
    let crates_io = DepSource::Registry(RegistryRef::Host("index.crates.io".into()));
    assert!(matches!(
        cls(Ecosystem::Cargo, "serde", &crates_io, &reg),
        Visibility::Private(PrivateReason::CustomRegistry, _)
    ));
}

#[test]
fn nuget_source_mapping() {
    let mut reg = open();
    {
        let e = reg.eco_mut(Ecosystem::Nuget);
        e.replace_all = true;
        e.nuget_mapping.push(NugetMapping {
            pattern: "newtonsoft.*".into(),
            public_only: true,
            host: None,
        });
        e.nuget_mapping.push(NugetMapping {
            pattern: "zzsentinel.*".into(),
            public_only: false,
            host: Some("nuget.corp.invalid".into()),
        });
    }
    let nu = |n: &str| cls(Ecosystem::Nuget, n, &DEFAULT, &reg);
    assert!(is_public(&nu("Newtonsoft.Json")));
    assert_eq!(
        nu("ZzSentinel.Pkg"),
        Visibility::Private(
            PrivateReason::SourceMapping,
            Some("nuget.corp.invalid".into())
        )
    );
    assert!(matches!(
        nu("Other.Pkg"),
        Visibility::Private(PrivateReason::CustomRegistry, _)
    ));
}

// ---- HIGH-1 item 3: local names -------------------------------------------

#[test]
fn local_names_win_over_every_other_rule() {
    let mut reg = open();
    reg.eco_mut(Ecosystem::Npm).fail_closed = true;
    let mut local = LocalNames::new();
    local.insert(Ecosystem::Npm, "@zzws/b");
    local.insert(Ecosystem::Cargo, "zz_sentinel_crate");
    local.insert(Ecosystem::Pypi, "Zz.Sentinel_Py");
    local.insert(Ecosystem::Nuget, "ZzSentinel.Local");
    let ws = Visibility::Local(LocalReason::Workspace);
    let c = |eco, n| classify(eco, n, Some("1.0.0"), &DEFAULT, &reg, &local);
    assert_eq!(c(Ecosystem::Npm, "@zzws/b"), ws);
    assert_eq!(c(Ecosystem::Npm, "@ZZWS/B"), ws);
    assert_eq!(c(Ecosystem::Cargo, "zz-sentinel-crate"), ws);
    assert_eq!(c(Ecosystem::Pypi, "zz-sentinel-py"), ws);
    assert_eq!(c(Ecosystem::Nuget, "zzsentinel.local"), ws);
    // Names are per ecosystem.
    assert!(is_public(&c(Ecosystem::Cargo, "b")));
    assert!(!local.contains(Ecosystem::Go, "@zzws/b"));
}

#[test]
fn non_registry_sources_are_local() {
    let reg = open();
    for (src, reason) in [
        (DepSource::Path, LocalReason::Path),
        (DepSource::Git, LocalReason::Git),
        (DepSource::Workspace, LocalReason::Workspace),
        (DepSource::Url, LocalReason::Url),
    ] {
        assert_eq!(
            cls(Ecosystem::Npm, "left-pad", &src, &reg),
            Visibility::Local(reason)
        );
    }
}

#[test]
fn pypi_normalization_is_pep503() {
    assert_eq!(normalize(Ecosystem::Pypi, "Foo__Bar.-baz"), "foo-bar-baz");
    assert_eq!(normalize(Ecosystem::Npm, " react "), "react");
    assert_eq!(
        normalize(Ecosystem::Nuget, "Newtonsoft.Json"),
        "Newtonsoft.Json"
    );
}
