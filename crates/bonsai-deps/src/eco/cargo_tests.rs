//! Cargo adapter tests (E2 Cargo row).

use super::*;
use crate::adapter::LockEntry;
use crate::fixtures::cargo::{LOCK, MEMBER_MANIFEST, ROOT_MANIFEST};
use crate::fixtures::toml_bombs::cargo_manifest_bomb;
use crate::model::ParseErrorKind;

const DEFAULT: DepSource = DepSource::Registry(RegistryRef::Default);

fn host(h: &str) -> DepSource {
    DepSource::Registry(RegistryRef::Host(h.into()))
}

fn named(n: &str) -> DepSource {
    DepSource::Registry(RegistryRef::Named(n.into()))
}

fn try_parse(path: &str, manifest: &str, lock: Option<&str>) -> Result<ParsedGroup, ParseError> {
    let files = FileSet::default();
    CargoAdapter.parse(&ParseInput {
        manifest_path: path,
        manifest,
        lockfile: lock.map(|l| ("Cargo.lock", l)),
        files: &files,
    })
}

fn parse(path: &str, manifest: &str, lock: Option<&str>) -> ParsedGroup {
    try_parse(path, manifest, lock).unwrap()
}

fn dep<'a>(g: &'a ParsedGroup, name: &str) -> &'a Dependency {
    g.direct
        .iter()
        .find(|d| d.name == name)
        .unwrap_or_else(|| panic!("no dep {name}"))
}

fn entry<'a>(g: &'a ParsedGroup, name: &str) -> &'a LockEntry {
    g.lock_entries
        .iter()
        .find(|e| e.name == name)
        .unwrap_or_else(|| panic!("no entry {name}"))
}

#[test]
fn root_manifest_sources_scopes_and_resolution() {
    let g = parse("Cargo.toml", ROOT_MANIFEST, Some(LOCK));
    assert!(g.has_lockfile);
    let cases: &[(&str, DepSource, DepScope, Option<&str>)] = &[
        ("anyhow", DEFAULT, DepScope::Normal, Some("1.0.86")),
        ("member", DepSource::Path, DepScope::Normal, None),
        ("zz_sentinel_git", DepSource::Git, DepScope::Normal, None),
        ("zz-sentinel-reg", named("corp"), DepScope::Normal, None),
        // `package = "serde_json"`: the real crate name, optional scope.
        ("serde_json", DEFAULT, DepScope::Optional, Some("1.0.117")),
        ("patched-secret", DEFAULT, DepScope::Normal, None),
        ("pretty_assertions", DEFAULT, DepScope::Dev, None),
        ("cc", DEFAULT, DepScope::Build, Some("1.0.98")),
        ("winapi", DEFAULT, DepScope::Normal, None),
        // [workspace.dependencies]
        ("serde", DEFAULT, DepScope::Normal, Some("1.0.203")),
        ("corp-ws", named("corp"), DepScope::Normal, Some("2.1.0")),
        ("local-ws", DepSource::Path, DepScope::Normal, None),
    ];
    for (name, src, scope, resolved) in cases {
        let d = dep(&g, name);
        assert_eq!(&d.source, src, "{name}");
        assert_eq!(d.scope, *scope, "{name}");
        assert_eq!(d.resolved.as_deref(), *resolved, "{name}");
    }
    assert_eq!(dep(&g, "anyhow").declared.as_deref(), Some("1"));
    assert!(g.direct.iter().all(|d| d.name != "json"));
}

#[test]
fn workspace_inherited_deps_resolve_through_the_lock_or_stay_local() {
    let g = parse("crates/member/Cargo.toml", MEMBER_MANIFEST, Some(LOCK));
    assert_eq!(dep(&g, "serde").source, DEFAULT);
    assert_eq!(dep(&g, "serde").resolved.as_deref(), Some("1.0.203"));
    // The lock's host, userinfo stripped.
    assert_eq!(dep(&g, "corp-ws").source, host("cargo.corp.invalid"));
    assert_eq!(dep(&g, "local-ws").source, DepSource::Workspace);
    assert_eq!(dep(&g, "missing-ws").source, DepSource::Workspace);
    // Without a lock no inherited name is sent blind.
    let g = parse("crates/member/Cargo.toml", MEMBER_MANIFEST, None);
    assert!(!g.has_lockfile);
    for n in ["serde", "corp-ws", "local-ws", "missing-ws"] {
        assert_eq!(dep(&g, n).source, DepSource::Workspace, "{n}");
    }
    assert_eq!(dep(&g, "anyhow").source, DEFAULT);
}

#[test]
fn lock_sources_map_exactly() {
    let g = parse("Cargo.toml", ROOT_MANIFEST, Some(LOCK));
    assert_eq!(g.lock_entries.len(), 10);
    assert_eq!(entry(&g, "anyhow").source, DEFAULT);
    assert_eq!(entry(&g, "cc").source, DEFAULT);
    assert_eq!(entry(&g, "local-ws").source, DepSource::Workspace);
    assert_eq!(entry(&g, "member").source, DepSource::Workspace);
    assert_eq!(entry(&g, "zz_sentinel_git").source, DepSource::Git);
    assert_eq!(
        entry(&g, "zz-sentinel-trans-cargo").source,
        host("cargo.corp.invalid")
    );
    // github.com is NOT public by host: only the exact crates.io id is.
    assert_eq!(entry(&g, "gh-hosted").source, host("github.com"));
    for (s, want) in [
        (
            Some("registry+https://github.com/rust-lang/crates.io-index"),
            DEFAULT,
        ),
        (Some("sparse+https://index.crates.io/"), DEFAULT),
        (
            Some("registry+https://github.com/rust-lang/crates.io-index/"),
            host("github.com"),
        ),
        (Some("sparse+ssh://corp/index"), DepSource::Url),
        (Some("path+file:///x"), DepSource::Url),
        (None, DepSource::Workspace),
    ] {
        assert_eq!(cargo_lock::lock_source(s), want, "{s:?}");
    }
}

#[test]
fn a_mixed_lock_prefers_the_private_candidate() {
    let lock = r#"
[[package]]
name = "dup"
version = "1.0.0"
source = "registry+https://github.com/rust-lang/crates.io-index"

[[package]]
name = "dup"
version = "1.2.0"
source = "git+https://git.corp.invalid/dup#abc"
"#;
    let m = "[dependencies]\ndup = \"1\"\n";
    assert_eq!(
        dep(&parse("Cargo.toml", m, Some(lock)), "dup").source,
        DepSource::Git
    );
    // A requirement that only one candidate satisfies picks that one.
    let m = "[dependencies]\ndup = \"=1.0.0\"\n";
    assert_eq!(
        dep(&parse("Cargo.toml", m, Some(lock)), "dup").source,
        DEFAULT
    );
}

#[test]
fn declared_names_include_patch_and_replace_targets() {
    let mut names = CargoAdapter.declared_names(ROOT_MANIFEST);
    names.sort();
    assert_eq!(names, ["old-replaced", "patched-secret", "root-app"]);
    assert!(CargoAdapter.declared_names("not = [toml").is_empty());
    assert!(CargoAdapter
        .declared_names(&cargo_manifest_bomb(100))
        .is_empty());
}

#[test]
fn lockfile_for_walks_to_the_workspace_root() {
    let files = FileSet::from_paths([
        "Cargo.toml".to_string(),
        "Cargo.lock".to_string(),
        "crates/member/Cargo.toml".to_string(),
        "tools/x/Cargo.toml".to_string(),
        "tools/x/Cargo.lock".to_string(),
    ]);
    let f = |m: &str| CargoAdapter.lockfile_for(m, &files);
    assert_eq!(f("crates/member/Cargo.toml").as_deref(), Some("Cargo.lock"));
    assert_eq!(
        f("tools/x/Cargo.toml").as_deref(),
        Some("tools/x/Cargo.lock")
    );
}

#[test]
fn toml_deep_nesting_is_limit_exceeded() {
    // [HIGH-2] rejected by the pre-scan, before the toml parser.
    let e = try_parse("Cargo.toml", &cargo_manifest_bomb(100_000), None).unwrap_err();
    assert_eq!(e.kind, ParseErrorKind::LimitExceeded);
    assert_eq!(e.line, Some(6));
    let lock = format!("version = 3\nx = {}", "{a = ".repeat(1000));
    let e = try_parse("Cargo.toml", "", Some(&lock)).unwrap_err();
    assert_eq!(
        (e.kind, e.file.as_str()),
        (ParseErrorKind::LimitExceeded, "Cargo.lock")
    );
}

#[test]
fn syntax_errors_carry_a_position_and_never_the_line() {
    let bad = "[dependencies]\nserde = \"1\"\ntoken = zz-sentinel-token\n";
    let e = try_parse("Cargo.toml", bad, None).unwrap_err();
    assert_eq!(e.kind, ParseErrorKind::InvalidSyntax);
    assert_eq!(e.line, Some(3));
    assert!(!format!("{e:?}").contains("zz-sentinel"));
    let lock = "[[package]]\nname = \"a\"\nversion = zz-sentinel-token\n";
    let e = try_parse("Cargo.toml", "", Some(lock)).unwrap_err();
    assert_eq!(
        (e.kind, e.file.as_str()),
        (ParseErrorKind::InvalidSyntax, "Cargo.lock")
    );
    assert!(!format!("{e:?}").contains("zz-sentinel"));
}

#[test]
fn unsupported_lock_versions_are_rejected() {
    let e = try_parse("Cargo.toml", "", Some("version = 5\n")).unwrap_err();
    assert_eq!(e.kind, ParseErrorKind::UnsupportedVersion);
    // v1 locks have no `version` key.
    assert!(try_parse(
        "Cargo.toml",
        "",
        Some("[[package]]\nname = \"a\"\nversion = \"1.0.0\"\n")
    )
    .is_ok());
}

#[test]
fn registry_names_and_hosts_normalise() {
    assert_eq!(registry_key(" My_Corp "), "my-corp");
    assert_eq!(
        cargo_url_host("sparse+https://u:t@Cargo.Corp.invalid:8443/x/").as_deref(),
        Some("cargo.corp.invalid")
    );
    assert_eq!(cargo_url_host("ssh://git@corp/index"), None);
    let m = "[dependencies]\na = { version = \"1\", registry = \"My_Corp\" }\n\
             b = { version = \"1\", registry-index = \"https://u:t@idx.corp.invalid/\" }\n\
             c = { version = \"1\", registry-index = \"file:///srv/index\" }\n";
    let g = parse("Cargo.toml", m, None);
    assert_eq!(dep(&g, "a").source, named("my-corp"));
    assert_eq!(dep(&g, "b").source, host("idx.corp.invalid"));
    assert_eq!(dep(&g, "c").source, DepSource::Url);
}
