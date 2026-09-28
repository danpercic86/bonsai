//! NuGet adapter tests (E2 NuGet row).

use super::*;
use crate::fixtures::nuget::{CSPROJ, LOCK, PACKAGES_CONFIG, PACKAGES_PROPS};
use crate::fixtures::xml_bombs;
use crate::model::ParseErrorKind;

const DEFAULT: DepSource = DepSource::Registry(RegistryRef::Default);

fn try_parse(path: &str, manifest: &str, lock: Option<&str>) -> Result<ParsedGroup, ParseError> {
    let files = FileSet::default();
    NugetAdapter.parse(&ParseInput {
        manifest_path: path,
        manifest,
        lockfile: lock.map(|l| ("src/App/packages.lock.json", l)),
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

#[test]
fn manifest_and_lock_names_are_case_insensitive() {
    for m in [
        "App.csproj",
        "a.FSPROJ",
        "b.vbproj",
        "Directory.Packages.props",
        "directory.build.props",
        "Packages.Config",
    ] {
        assert!(NugetAdapter.is_manifest(m), "{m}");
    }
    for m in [
        ".csproj",
        "App.csproj.user",
        "Directory.Build.targets",
        "nuget.config",
    ] {
        assert!(!NugetAdapter.is_manifest(m), "{m}");
    }
    assert!(NugetAdapter.is_lockfile("packages.lock.json"));
    assert!(NugetAdapter.is_lockfile("packages.App.lock.json"));
    let files = FileSet::from_paths(
        [
            "src/App/App.csproj",
            "src/App/packages.App.lock.json",
            "src/App/packages.lock.json",
            "src/App/packages.config",
        ]
        .map(String::from),
    );
    assert_eq!(
        NugetAdapter
            .lockfile_for("src/App/App.csproj", &files)
            .as_deref(),
        Some("src/App/packages.App.lock.json")
    );
    assert_eq!(
        NugetAdapter.lockfile_for("src/App/packages.config", &files),
        None
    );
}

#[test]
fn package_references_versions_and_items() {
    let g = parse("src/App/App.csproj", CSPROJ, Some(LOCK));
    let names: Vec<&str> = g.direct.iter().map(|d| d.name.as_str()).collect();
    assert_eq!(
        names,
        [
            "Newtonsoft.Json",
            "Serilog",
            "Polly",
            "ZzSentinel.Pkg",
            "xunit",
            "xunit.runner.visualstudio"
        ]
    );
    let nj = dep(&g, "Newtonsoft.Json");
    assert_eq!(nj.declared.as_deref(), Some("13.0.3"));
    assert_eq!(nj.resolved.as_deref(), Some("13.0.3"));
    assert_eq!(nj.source, DEFAULT);
    let serilog = dep(&g, "Serilog");
    assert_eq!(serilog.declared.as_deref(), Some("[3.1,4.0)"));
    assert_eq!(serilog.resolved.as_deref(), Some("3.1.1"));
    assert_eq!(dep(&g, "Polly").resolved, None);
    assert_eq!(dep(&g, "ZzSentinel.Pkg").declared, None);
    assert_eq!(dep(&g, "xunit").declared.as_deref(), Some("2.9.0"));
    assert!(g.has_lockfile);
}

#[test]
fn lock_entries_dedupe_and_mark_projects() {
    let g = parse("src/App/App.csproj", CSPROJ, Some(LOCK));
    let nj: Vec<_> = g
        .lock_entries
        .iter()
        .filter(|e| e.name.eq_ignore_ascii_case("Newtonsoft.Json"))
        .collect();
    assert_eq!(nj.len(), 1, "deduped across TFMs, case-insensitively");
    let lib = g
        .lock_entries
        .iter()
        .find(|e| e.name == "ZzSentinel.Lib")
        .unwrap();
    assert_eq!(lib.source, DepSource::Workspace);
    let e = try_parse("a.csproj", "<Project/>", Some("{\"version\": 3}")).unwrap_err();
    assert_eq!(e.kind, ParseErrorKind::UnsupportedVersion);
    let e = try_parse(
        "a.csproj",
        "<Project/>",
        Some("{\"version\": zz-sentinel-token}"),
    )
    .unwrap_err();
    assert_eq!(e.kind, ParseErrorKind::InvalidSyntax);
    assert!(!format!("{e:?}").contains("zz-sentinel-token"));
}

#[test]
fn central_props_and_packages_config() {
    let g = parse("Directory.Packages.props", PACKAGES_PROPS, None);
    assert_eq!(
        dep(&g, "Newtonsoft.Json").resolved.as_deref(),
        Some("13.0.3")
    );
    assert_eq!(
        dep(&g, "StyleCop.Analyzers").declared.as_deref(),
        Some("1.1.118")
    );
    let g = parse("packages.config", PACKAGES_CONFIG, None);
    assert_eq!(dep(&g, "jQuery").resolved.as_deref(), Some("3.7.1"));
    assert_eq!(dep(&g, "jQuery").scope, DepScope::Normal);
    assert_eq!(dep(&g, "NUnit").scope, DepScope::Dev);
}

#[test]
fn declared_names_include_the_project_stem() {
    let names = NugetAdapter.declared_names_at("src/App/ZzSentinel.Stem.csproj", CSPROJ);
    assert_eq!(names, ["ZzSentinel.App", "ZzSentinel.Stem"]);
    assert!(NugetAdapter
        .declared_names_at("Directory.Build.props", "<Project/>")
        .is_empty());
}

#[test]
fn xml_bombs_and_bad_xml_are_textless_errors() {
    let e = try_parse("a.csproj", &xml_bombs::billion_laughs(), None).unwrap_err();
    assert_eq!(e.kind, ParseErrorKind::UnsupportedFeature);
    let e = try_parse("a.csproj", &xml_bombs::deep_nesting(10_000), None).unwrap_err();
    assert_eq!(e.kind, ParseErrorKind::LimitExceeded);
    let e = try_parse(
        "a.csproj",
        "<Project>\n<ItemGroup>zz-sentinel-token</Project>",
        None,
    )
    .unwrap_err();
    assert_eq!(e.kind, ParseErrorKind::InvalidSyntax);
    assert_eq!(e.line, Some(2));
    assert!(!format!("{e:?}").contains("zz-sentinel-token"));
}
