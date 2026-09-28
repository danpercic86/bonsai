//! npm-family adapter tests (E2 npm row).

use super::*;
use crate::fixtures::npm::*;

fn host(h: &str) -> DepSource {
    DepSource::Registry(RegistryRef::Host(h.into()))
}

const DEFAULT: DepSource = DepSource::Registry(RegistryRef::Default);

fn parse_with(lock: Option<(&str, &str)>) -> ParsedGroup {
    parse_at("package.json", APP_MANIFEST, lock)
}

fn parse_at(manifest_path: &str, manifest: &str, lock: Option<(&str, &str)>) -> ParsedGroup {
    let files = FileSet::default();
    NpmAdapter
        .parse(&ParseInput {
            manifest_path,
            manifest,
            lockfile: lock,
            files: &files,
        })
        .unwrap()
}

fn dep<'a>(g: &'a ParsedGroup, name: &str) -> &'a Dependency {
    g.direct
        .iter()
        .find(|d| d.name == name)
        .unwrap_or_else(|| panic!("no dep {name}"))
}

#[test]
fn spec_classification_table() {
    let cases: &[(&str, DepSource)] = &[
        ("^1.2.3", DEFAULT),
        ("latest", DEFAULT),
        ("~1.2.3", DEFAULT),
        ("~1", DEFAULT),
        ("~>1.0", DEFAULT),
        (".", DepSource::Path),
        ("1.0.0 - 2.0.0", DEFAULT),
        ("file:../x", DepSource::Path),
        ("link:../x", DepSource::Path),
        ("portal:../x", DepSource::Path),
        ("../x", DepSource::Path),
        ("./x", DepSource::Path),
        ("~/x", DepSource::Path),
        ("/abs/x", DepSource::Path),
        ("C:\\x", DepSource::Path),
        ("workspace:*", DepSource::Workspace),
        ("workspace:^", DepSource::Workspace),
        ("git+https://github.com/u/r.git", DepSource::Git),
        ("git://github.com/u/r.git", DepSource::Git),
        ("github:u/r", DepSource::Git),
        ("user/repo#tag", DepSource::Git),
        ("https://example.com/t.tgz", DepSource::Url),
        ("http://example.com/t.tgz", DepSource::Url),
        (
            "gh:@acme/private@^1",
            DepSource::Registry(RegistryRef::Named("gh".into())),
        ),
        (
            "jsr:@std/path",
            DepSource::Registry(RegistryRef::Named("jsr".into())),
        ),
        ("catalog:", DEFAULT),
    ];
    for (spec, want) in cases {
        assert_eq!(&classify_spec("k", spec).source, want, "{spec}");
    }
    let alias = classify_spec("a", "npm:@scope/real@^4");
    assert_eq!(alias.name, "@scope/real");
    assert_eq!(alias.declared.as_deref(), Some("^4"));
}

#[test]
fn split_name_version_handles_scopes() {
    assert_eq!(split_name_version("a@1"), ("a", Some("1")));
    assert_eq!(split_name_version("@s/a@1"), ("@s/a", Some("1")));
    assert_eq!(split_name_version("@s/a"), ("@s/a", None));
    assert_eq!(split_name_version(""), ("", None));
}

#[test]
fn resolved_urls_become_hosts_without_userinfo() {
    assert_eq!(
        source_from_resolved("https://u:p@NPM.Corp.Invalid:8443/x.tgz"),
        host("npm.corp.invalid")
    );
    assert_eq!(source_from_resolved("file:../x"), DepSource::Path);
    assert_eq!(source_from_resolved("git+ssh://git@h/r"), DepSource::Git);
    assert_eq!(source_from_resolved("https://[bad"), DepSource::Url);
    assert_eq!(source_from_resolved(""), DEFAULT);
}

#[test]
fn lockfile_for_prefers_same_dir_then_ancestors_in_priority_order() {
    let files = FileSet::from_paths(
        [
            "package.json",
            "yarn.lock",
            "package-lock.json",
            "packages/a/package.json",
            "packages/b/package.json",
            "packages/b/pnpm-lock.yaml",
        ]
        .map(String::from),
    );
    let a = NpmAdapter;
    assert_eq!(
        a.lockfile_for("package.json", &files).as_deref(),
        Some("package-lock.json")
    );
    assert_eq!(
        a.lockfile_for("packages/a/package.json", &files).as_deref(),
        Some("package-lock.json")
    );
    assert_eq!(
        a.lockfile_for("packages/b/package.json", &files).as_deref(),
        Some("packages/b/pnpm-lock.yaml")
    );
    let none = FileSet::from_paths(["package.json".to_string()]);
    assert_eq!(a.lockfile_for("package.json", &none), None);
}

#[test]
fn declared_names_reads_name_only() {
    assert_eq!(NpmAdapter.declared_names(APP_MANIFEST), vec!["@acme/app"]);
    assert!(NpmAdapter.declared_names("{}").is_empty());
    assert!(NpmAdapter.declared_names("not json").is_empty());
}

#[test]
fn manifest_without_lock_lists_every_group() {
    let g = parse_with(None);
    assert!(!g.has_lockfile);
    assert_eq!(g.direct.len(), 12);
    assert_eq!(dep(&g, "jest").scope, DepScope::Dev);
    assert_eq!(dep(&g, "fsevents").scope, DepScope::Optional);
    assert_eq!(dep(&g, "react").scope, DepScope::Peer);
    assert_eq!(dep(&g, "left-pad").resolved, None);
    assert_eq!(dep(&g, "local-lib").source, DepSource::Path);
    assert_eq!(dep(&g, "ws-lib").source, DepSource::Workspace);
    assert_eq!(dep(&g, "gitdep").source, DepSource::Git);
    assert_eq!(dep(&g, "short").source, DepSource::Git);
    assert_eq!(dep(&g, "tarball").source, DepSource::Url);
    assert_eq!(
        dep(&g, "named").source,
        DepSource::Registry(RegistryRef::Named("work".into()))
    );
    assert_eq!(dep(&g, "real-pkg").declared.as_deref(), Some("^4.0.0"));
}

#[test]
fn package_lock_v3_resolves_direct_and_lists_entries() {
    let g = parse_with(Some(("package-lock.json", LOCK_V3)));
    assert!(g.has_lockfile);
    let lp = dep(&g, "left-pad");
    assert_eq!(lp.resolved.as_deref(), Some("1.3.0"));
    assert_eq!(lp.source, host("registry.npmjs.org"));
    let ui = dep(&g, "@corp/ui");
    assert_eq!(ui.resolved.as_deref(), Some("2.1.0"));
    assert_eq!(ui.source, host("npm.corp.invalid"));
    assert_eq!(dep(&g, "real-pkg").resolved.as_deref(), Some("4.1.0"));
    // A declared non-registry spec wins over the lock.
    assert_eq!(dep(&g, "ws-lib").source, DepSource::Workspace);
    assert_eq!(dep(&g, "local-lib").source, DepSource::Path);
    let names: Vec<_> = g.lock_entries.iter().map(|e| e.name.as_str()).collect();
    assert!(names.contains(&"nested") && names.contains(&"real-pkg"));
    let nested = g.lock_entries.iter().find(|e| e.name == "nested").unwrap();
    assert_eq!(nested.source, host("registry.yarnpkg.com"));
    let ws = g.lock_entries.iter().find(|e| e.name == "ws-lib").unwrap();
    assert_eq!(ws.source, DepSource::Workspace);
}

#[test]
fn package_lock_v3_member_prefers_nested_then_hoisted() {
    let member = r#"{"name":"ws-lib","dependencies":{"left-pad":"^1"}}"#;
    let g = parse_at(
        "packages/ws-lib/package.json",
        member,
        Some(("package-lock.json", LOCK_V3)),
    );
    assert_eq!(dep(&g, "left-pad").resolved.as_deref(), Some("1.3.0"));
}

#[test]
fn package_lock_v1_walks_the_tree_iteratively() {
    let g = parse_with(Some(("package-lock.json", LOCK_V1)));
    assert_eq!(dep(&g, "left-pad").resolved.as_deref(), Some("1.3.0"));
    let deep = g.lock_entries.iter().find(|e| e.name == "deep").unwrap();
    assert_eq!(deep.source, host("npm.corp.invalid"));
    let local = g
        .lock_entries
        .iter()
        .find(|e| e.name == "local-lib")
        .unwrap();
    assert_eq!(local.source, DepSource::Path);
    let real = g
        .lock_entries
        .iter()
        .find(|e| e.name == "real-pkg")
        .unwrap();
    assert_eq!(real.version, "4.1.0");
}

#[test]
fn unsupported_package_lock_version() {
    let files = FileSet::default();
    let err = NpmAdapter
        .parse(&ParseInput {
            manifest_path: "package.json",
            manifest: APP_MANIFEST,
            lockfile: Some((
                "package-lock.json",
                r#"{"lockfileVersion": 4, "packages": {}}"#,
            )),
            files: &files,
        })
        .unwrap_err();
    assert_eq!(err.kind, ParseErrorKind::UnsupportedVersion);
    assert_eq!(err.file, "package-lock.json");
}

#[test]
fn pnpm_v9_importers_and_packages() {
    let g = parse_with(Some(("pnpm-lock.yaml", PNPM_V9)));
    let ui = dep(&g, "@corp/ui");
    assert_eq!(ui.resolved.as_deref(), Some("2.1.0"));
    // [HIGH-1] pnpm omits the host: the config is the only signal.
    assert_eq!(ui.source, DEFAULT);
    assert_eq!(dep(&g, "real-pkg").resolved.as_deref(), Some("4.1.0"));
    assert_eq!(dep(&g, "jest").resolved.as_deref(), Some("29.7.0"));
    let tar = g.lock_entries.iter().find(|e| e.name == "tar").unwrap();
    assert_eq!(tar.source, host("npm.corp.invalid"));
    assert_eq!(tar.version, "1.0.0");
    let member = r#"{"name":"ws-lib","dependencies":{"left-pad":"^1.0.0"}}"#;
    let m = parse_at(
        "packages/ws-lib/package.json",
        member,
        Some(("pnpm-lock.yaml", PNPM_V9)),
    );
    assert_eq!(dep(&m, "left-pad").resolved.as_deref(), Some("1.3.0"));
}

#[test]
fn pnpm_v6_top_level_dependencies_and_slash_keys() {
    let g = parse_with(Some(("pnpm-lock.yaml", PNPM_V6)));
    assert_eq!(dep(&g, "left-pad").resolved.as_deref(), Some("1.3.0"));
    let ui = g
        .lock_entries
        .iter()
        .find(|e| e.name == "@corp/ui")
        .unwrap();
    assert_eq!(ui.version, "2.1.0");
}

#[test]
fn pnpm_rejects_old_versions_and_anchors() {
    let files = FileSet::default();
    let run = |lock: &str| {
        NpmAdapter
            .parse(&ParseInput {
                manifest_path: "package.json",
                manifest: APP_MANIFEST,
                lockfile: Some(("pnpm-lock.yaml", lock)),
                files: &files,
            })
            .unwrap_err()
            .kind
    };
    assert_eq!(
        run("lockfileVersion: 5.4\n"),
        ParseErrorKind::UnsupportedVersion
    );
    assert_eq!(
        run("lockfileVersion: '9.0'\npackages: &p\n  a@1: {}\n"),
        ParseErrorKind::UnsupportedFeature
    );
}

#[test]
fn yarn_v1_descriptors_and_hosts() {
    let g = parse_with(Some(("yarn.lock", YARN_V1)));
    let ui = dep(&g, "@corp/ui");
    assert_eq!(ui.resolved.as_deref(), Some("2.1.0"));
    assert_eq!(ui.source, host("npm.corp.invalid"));
    assert_eq!(dep(&g, "left-pad").resolved.as_deref(), Some("1.3.0"));
    let real = g
        .lock_entries
        .iter()
        .find(|e| e.name == "real-pkg")
        .unwrap();
    assert_eq!(real.version, "4.1.0");
}

#[test]
fn yarn_berry_protocols() {
    let g = parse_with(Some(("yarn.lock", YARN_BERRY)));
    let ui = dep(&g, "@corp/ui");
    assert_eq!(ui.resolved.as_deref(), Some("2.1.0"));
    assert_eq!(ui.source, DEFAULT);
    assert_eq!(dep(&g, "left-pad").resolved.as_deref(), Some("1.3.0"));
    let ws = g.lock_entries.iter().find(|e| e.name == "ws-lib").unwrap();
    assert_eq!(ws.source, DepSource::Workspace);
}

#[test]
fn yarn_v1_ten_megabyte_line_parses_iteratively() {
    let long = "x".repeat(10 * 1024 * 1024);
    let lock = format!("# yarn lockfile v1\n\nleft-pad@^1.3.0:\n  version \"1.3.0\"\n  resolved \"https://registry.yarnpkg.com/{long}\"\n");
    let g = parse_with(Some(("yarn.lock", &lock)));
    assert_eq!(dep(&g, "left-pad").resolved.as_deref(), Some("1.3.0"));
    // A 10 MB header line with no `:` is a located syntax error, not a hang.
    let files = FileSet::default();
    let err = NpmAdapter
        .parse(&ParseInput {
            manifest_path: "package.json",
            manifest: APP_MANIFEST,
            lockfile: Some(("yarn.lock", &long)),
            files: &files,
        })
        .unwrap_err();
    assert_eq!(err.kind, ParseErrorKind::InvalidSyntax);
    assert_eq!(err.line, Some(1));
}

#[test]
fn malformed_manifest_error_carries_no_text() {
    let files = FileSet::default();
    let err = NpmAdapter
        .parse(&ParseInput {
            manifest_path: "package.json",
            manifest: "{\"name\": \"x\",\n  zz-sentinel-token }",
            lockfile: None,
            files: &files,
        })
        .unwrap_err();
    assert_eq!(err.kind, ParseErrorKind::InvalidSyntax);
    assert_eq!(err.line, Some(2));
    assert!(!format!("{err:?}").contains("zz-sentinel-token"));
    let not_obj = NpmAdapter.parse(&ParseInput {
        manifest_path: "package.json",
        manifest: "[1]",
        lockfile: None,
        files: &files,
    });
    assert_eq!(not_obj.unwrap_err().kind, ParseErrorKind::InvalidSyntax);
}
