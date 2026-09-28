//! PyPI adapter tests (E2 PyPI row).

use super::*;
use crate::adapter::LockEntry;
use crate::fixtures::pypi::{
    POETRY_LOCK as POETRY_LOCK_FX, PYPROJECT_POETRY, PYPROJECT_UV, REQUIREMENTS,
    REQUIREMENTS_CORP_INDEX, REQUIREMENTS_PUBLIC_INDEX, UV_LOCK as UV_LOCK_FX,
};
use crate::fixtures::toml_bombs::cargo_manifest_bomb;

const DEFAULT: DepSource = DepSource::Registry(RegistryRef::Default);

fn host(h: &str) -> DepSource {
    DepSource::Registry(RegistryRef::Host(h.into()))
}

fn named(n: &str) -> DepSource {
    DepSource::Registry(RegistryRef::Named(n.into()))
}

fn try_parse(
    path: &str,
    manifest: &str,
    lock: Option<(&str, &str)>,
) -> Result<ParsedGroup, ParseError> {
    let files = FileSet::default();
    PypiAdapter.parse(&ParseInput {
        manifest_path: path,
        manifest,
        lockfile: lock,
        files: &files,
    })
}

fn parse(path: &str, manifest: &str, lock: Option<(&str, &str)>) -> ParsedGroup {
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
fn manifest_and_lock_names() {
    for m in ["requirements.txt", "requirements-dev.txt", "pyproject.toml"] {
        assert!(PypiAdapter.is_manifest(m), "{m}");
    }
    for m in ["base.txt", "requirements.in", "setup.py", "Pipfile"] {
        assert!(!PypiAdapter.is_manifest(m), "{m}");
    }
    assert!(PypiAdapter.is_lockfile("poetry.lock") && PypiAdapter.is_lockfile("uv.lock"));
    let files = FileSet::from_paths(
        ["uv.lock", "pkg/pyproject.toml", "pkg/requirements.txt"].map(String::from),
    );
    assert_eq!(
        PypiAdapter
            .lockfile_for("pkg/pyproject.toml", &files)
            .as_deref(),
        Some("uv.lock")
    );
    assert_eq!(
        PypiAdapter.lockfile_for("pkg/requirements.txt", &files),
        None
    );
}

#[test]
fn requirements_lines_bom_crlf_comments_continuations() {
    let g = parse("requirements.txt", REQUIREMENTS, None);
    let names: Vec<&str> = g.direct.iter().map(|d| d.name.as_str()).collect();
    assert_eq!(
        names,
        [
            "flask",
            "requests",
            "Django",
            "zz-sentinel-editable",
            "zz-local-path",
            "zz-url",
            "zz-at-git",
            "zz-at-file",
            "numpy"
        ]
    );
    assert_eq!(dep(&g, "flask").resolved.as_deref(), Some("3.0.2"));
    assert_eq!(dep(&g, "requests").declared.as_deref(), Some(">=2.31,<3"));
    assert_eq!(dep(&g, "Django").declared.as_deref(), Some("==4.2.*"));
    assert_eq!(dep(&g, "Django").resolved, None);
    assert_eq!(dep(&g, "zz-sentinel-editable").source, DepSource::Git);
    assert_eq!(dep(&g, "zz-local-path").source, DepSource::Path);
    assert_eq!(dep(&g, "zz-url").source, DepSource::Url);
    assert_eq!(dep(&g, "zz-at-git").source, DepSource::Git);
    assert_eq!(dep(&g, "zz-at-file").source, DepSource::Path);
    assert_eq!(dep(&g, "numpy").source, DEFAULT);
    assert!(!g.has_lockfile);
}

#[test]
fn a_file_index_routes_every_registry_dep_of_that_file() {
    let g = parse("requirements.txt", REQUIREMENTS_CORP_INDEX, None);
    assert_eq!(dep(&g, "zz-sentinel-idx").source, host("pypi.corp.invalid"));
    assert_eq!(dep(&g, "zz-local").source, DepSource::Path);
    let g = parse("requirements.txt", REQUIREMENTS_PUBLIC_INDEX, None);
    assert_eq!(dep(&g, "six").source, DEFAULT);
}

#[test]
fn hostless_indexes_find_links_and_no_index_are_unnamed_private() {
    for opts in [
        "-i ${PIP_INDEX}\n",
        "--find-links ./wheels\n",
        "-f/abs/wheels\n",
        "--no-index\n",
    ] {
        let g = parse("requirements.txt", &format!("{opts}six\n"), None);
        assert_eq!(dep(&g, "six").source, named(""), "{opts}");
    }
    let g = parse(
        "requirements.txt",
        "--find-links https://wheels.corp.invalid/\nsix\n",
        None,
    );
    assert_eq!(dep(&g, "six").source, host("wheels.corp.invalid"));
}

#[test]
fn a_malformed_requirement_is_a_textless_located_error() {
    let e = try_parse("requirements.txt", "six\n$$ zz-sentinel-token\n", None).unwrap_err();
    assert_eq!(e.kind, ParseErrorKind::InvalidSyntax);
    assert_eq!(e.line, Some(2));
    assert!(!format!("{e:?}").contains("zz-sentinel-token"));
}

#[test]
fn pyproject_pep621_pep735_and_uv_sources() {
    let g = parse(
        "pyproject.toml",
        PYPROJECT_UV,
        Some(("uv.lock", UV_LOCK_FX)),
    );
    assert!(g.has_lockfile);
    assert_eq!(dep(&g, "httpx").resolved.as_deref(), Some("0.27.2"));
    assert_eq!(dep(&g, "httpx").declared.as_deref(), Some(">=0.27"));
    assert_eq!(dep(&g, "attrs").resolved.as_deref(), Some("23.2.0"));
    assert_eq!(dep(&g, "zz-pinned-idx").source, named("corp"));
    assert_eq!(dep(&g, "zz-pinned-idx").resolved.as_deref(), Some("2.0.0"));
    assert_eq!(dep(&g, "zz-ws-member").source, DepSource::Workspace);
    assert_eq!(dep(&g, "zz-src-git").source, DepSource::Git);
    assert_eq!(dep(&g, "click").scope, DepScope::Optional);
    assert_eq!(dep(&g, "pytest").scope, DepScope::Dev);
    assert_eq!(dep(&g, "ruff").scope, DepScope::Dev);
    assert_eq!(dep(&g, "mypy").scope, DepScope::Dev);
    assert_eq!(
        entry(&g, "zz-trans-private").source,
        host("uv.corp.invalid")
    );
    assert_eq!(entry(&g, "zz-app").source, DepSource::Workspace);
    assert_eq!(entry(&g, "zz-flat").source, DepSource::Path);
    // A virtual member without a version is not a lock entry.
    assert!(g.lock_entries.iter().all(|e| e.name != "zz-ws-member"));
}

#[test]
fn poetry_specs_sources_and_lock() {
    let g = parse(
        "pyproject.toml",
        PYPROJECT_POETRY,
        Some(("poetry.lock", POETRY_LOCK_FX)),
    );
    assert!(g.direct.iter().all(|d| d.name != "python"));
    assert_eq!(dep(&g, "requests").resolved.as_deref(), Some("2.31.0"));
    assert_eq!(dep(&g, "pendulum").declared.as_deref(), Some("^3.0"));
    assert_eq!(dep(&g, "pendulum").resolved, None);
    assert_eq!(dep(&g, "zz-explicit").source, named("corp-explicit"));
    assert_eq!(dep(&g, "zz-poetry-git").source, DepSource::Git);
    assert_eq!(dep(&g, "zz-poetry-path").source, DepSource::Path);
    assert_eq!(dep(&g, "multi").source, DEFAULT);
    assert_eq!(dep(&g, "extra-opt").scope, DepScope::Optional);
    assert_eq!(dep(&g, "pytest").scope, DepScope::Dev);
    assert_eq!(entry(&g, "zz-explicit").source, host("poetry.corp.invalid"));
    assert_eq!(entry(&g, "zz-poetry-git").source, DepSource::Git);
    assert_eq!(entry(&g, "requests").source, DEFAULT);
}

#[test]
fn a_lock_source_replaces_a_default_declaration() {
    let manifest = "[project]\nname = \"a\"\ndependencies = [\"zz-trans-private\"]\n";
    let g = parse("pyproject.toml", manifest, Some(("uv.lock", UV_LOCK_FX)));
    assert_eq!(dep(&g, "zz-trans-private").source, host("uv.corp.invalid"));
}

#[test]
fn declared_names_and_lock_versions() {
    assert_eq!(PypiAdapter.declared_names(PYPROJECT_UV), ["zz-app"]);
    assert_eq!(
        PypiAdapter.declared_names(PYPROJECT_POETRY),
        ["zz-poetry-app"]
    );
    assert!(PypiAdapter.declared_names(REQUIREMENTS).is_empty());
    let e = try_parse(
        "pyproject.toml",
        "[project]\nname = \"a\"\n",
        Some(("uv.lock", "version = 2\n")),
    )
    .unwrap_err();
    assert_eq!(e.kind, ParseErrorKind::UnsupportedVersion);
}

#[test]
fn pyproject_errors_are_located_and_bounded() {
    let e = try_parse(
        "pyproject.toml",
        "[project]\ndependencies = [\"$$bad\"]\n",
        None,
    )
    .unwrap_err();
    assert_eq!(e.kind, ParseErrorKind::InvalidSyntax);
    let e = try_parse("pyproject.toml", &cargo_manifest_bomb(100_000), None).unwrap_err();
    assert_eq!(e.kind, ParseErrorKind::LimitExceeded);
    let e = try_parse("pyproject.toml", "[project\nx = 1", None).unwrap_err();
    assert_eq!(e.kind, ParseErrorKind::InvalidSyntax);
    assert!(e.line.is_some());
}

#[test]
fn bare_archive_lines_are_locations_never_registry_names() {
    let g = parse(
        "requirements.txt",
        "foo-1.0.tar.gz
bar-2.0-py3-none-any.whl
baz.zip#egg=zz-baz
qux-1.tgz ; os_name == 'nt'
zz-quux-1.0.tar.bz2
six
",
        None,
    );
    let names: Vec<&str> = g.direct.iter().map(|d| d.name.as_str()).collect();
    assert_eq!(names, ["zz-baz", "six"]);
    assert_eq!(dep(&g, "zz-baz").source, DepSource::Path);
    // A PEP 508 direct reference to an archive stays a Url row.
    let g = parse(
        "requirements.txt",
        "pkg @ https://h.invalid/pkg-1.0.whl
",
        None,
    );
    assert_eq!(dep(&g, "pkg").source, DepSource::Url);
}
