//! PyPI adapter (E2 PyPI row): `requirements*.txt` (see `requirements.rs`)
//! and `pyproject.toml` (PEP 621 `[project]`, PEP 735 `[dependency-groups]`,
//! Poetry `[tool.poetry]`, uv `[tool.uv]`), with `poetry.lock` / `uv.lock`
//! (same dir, then ancestors ≤ workdir) for resolved versions.
//!
//! Registry routing split (fail toward "not sent"):
//! - HERE: per-dependency pins — uv `[tool.uv.sources] x = { index = "n" }`
//!   and Poetry `x = { source = "n" }` ⇒ `Registry(Named(n))` (always
//!   CustomRegistry); git / path / url / workspace sources ⇒ Local; a lock
//!   entry's source replaces a `Default` declaration (like Cargo).
//! - In the registry-context loader: every index DEFINITION
//!   (`[[tool.uv.index]]`, `[[tool.poetry.source]]`, uv `index-url`, pip /
//!   uv config and env) — a non-explicit non-public index ⇒ every PyPI dep
//!   CustomRegistry; an explicit one supplies the display host of `Named`.
//!
//! Environment markers are dropped (see `pep508.rs`).

use std::collections::BTreeMap;

use serde::de::IgnoredAny;
use serde::Deserialize;

use crate::adapter::{
    dir_and_ancestors, file_name, join, parent_dir, DepSource, Dependency, EcosystemAdapter,
    ParseError, ParseInput, ParsedGroup, RegistryRef,
};
use crate::discover::FileSet;
use crate::model::{DepScope, Ecosystem, ParseErrorKind};
use crate::privacy::normalize;

use super::cargo::parse_toml;
use super::pep508::{exact_pin, parse_req};
use super::pypi_lock::{self, PyLock, POETRY_LOCK, UV_LOCK};
use super::requirements;

pub struct PypiAdapter;

pub(crate) const PYPROJECT: &str = "pyproject.toml";

/// `requirements*.txt`.
pub(crate) fn is_requirements(file_name: &str) -> bool {
    file_name.starts_with("requirements") && file_name.ends_with(".txt")
}

#[derive(Deserialize)]
struct PyprojectDto {
    #[serde(default)]
    project: Option<ProjectDto>,
    #[serde(default, rename = "dependency-groups")]
    groups: BTreeMap<String, Vec<GroupItem>>,
    #[serde(default)]
    tool: Option<ToolDto>,
}

#[derive(Deserialize)]
struct ProjectDto {
    #[serde(default)]
    dependencies: Vec<String>,
    #[serde(default, rename = "optional-dependencies")]
    optional: BTreeMap<String, Vec<String>>,
}

/// PEP 735: a requirement string or `{include-group = "…"}` (skipped).
#[derive(Deserialize)]
#[serde(untagged)]
enum GroupItem {
    Req(String),
    Include(IgnoredAny),
}

#[derive(Deserialize)]
struct ToolDto {
    #[serde(default)]
    poetry: Option<PoetryDto>,
    #[serde(default)]
    uv: Option<UvDto>,
}

#[derive(Deserialize)]
struct PoetryDto {
    #[serde(default)]
    dependencies: BTreeMap<String, PoetrySpec>,
    #[serde(default, rename = "dev-dependencies")]
    dev: BTreeMap<String, PoetrySpec>,
    #[serde(default)]
    group: BTreeMap<String, PoetryGroup>,
}

#[derive(Deserialize)]
struct PoetryGroup {
    #[serde(default)]
    dependencies: BTreeMap<String, PoetrySpec>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum PoetrySpec {
    Version(String),
    Detail(PoetryDetail),
    Multi(Vec<PoetryDetail>),
}

#[derive(Deserialize)]
struct PoetryDetail {
    #[serde(default)]
    version: Option<String>,
    #[serde(default)]
    git: Option<IgnoredAny>,
    #[serde(default)]
    path: Option<IgnoredAny>,
    #[serde(default)]
    url: Option<IgnoredAny>,
    #[serde(default)]
    source: Option<String>,
    #[serde(default)]
    optional: Option<bool>,
}

#[derive(Deserialize)]
struct UvDto {
    #[serde(default)]
    sources: BTreeMap<String, UvSourceSpec>,
    #[serde(default, rename = "dev-dependencies")]
    dev: Vec<String>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum UvSourceSpec {
    One(UvSrc),
    Many(Vec<UvSrc>),
}

#[derive(Deserialize)]
struct UvSrc {
    #[serde(default)]
    index: Option<String>,
    #[serde(default)]
    git: Option<IgnoredAny>,
    #[serde(default)]
    path: Option<IgnoredAny>,
    #[serde(default)]
    workspace: Option<bool>,
}

/// Only what `declared_names` needs.
#[derive(Deserialize)]
struct NamesDto {
    #[serde(default)]
    project: Option<NameDto>,
    #[serde(default)]
    tool: Option<NamesToolDto>,
}

#[derive(Deserialize)]
struct NamesToolDto {
    #[serde(default)]
    poetry: Option<NameDto>,
}

#[derive(Deserialize)]
struct NameDto {
    #[serde(default)]
    name: Option<String>,
}

/// A named index (`uv` index / Poetry source name) compares lowercase.
pub(crate) fn index_key(name: &str) -> String {
    name.trim().to_ascii_lowercase()
}

fn uv_src(s: &UvSrc) -> DepSource {
    if let Some(i) = &s.index {
        DepSource::Registry(RegistryRef::Named(index_key(i)))
    } else if s.workspace == Some(true) {
        DepSource::Workspace
    } else if s.git.is_some() {
        DepSource::Git
    } else if s.path.is_some() {
        DepSource::Path
    } else {
        // `url = …` or an unknown source kind: never sent.
        DepSource::Url
    }
}

/// A uv source list: any local / VCS entry wins (never sent); else the
/// first index pin.
fn uv_sources(spec: &UvSourceSpec) -> DepSource {
    let list: Vec<&UvSrc> = match spec {
        UvSourceSpec::One(s) => vec![s],
        UvSourceSpec::Many(v) => v.iter().collect(),
    };
    let srcs: Vec<DepSource> = list.into_iter().map(uv_src).collect();
    srcs.iter()
        .find(|s| !matches!(s, DepSource::Registry(_)))
        .or_else(|| srcs.first())
        .cloned()
        .unwrap_or(DepSource::Url)
}

fn poetry_detail_source(d: &PoetryDetail) -> DepSource {
    if d.git.is_some() {
        DepSource::Git
    } else if d.path.is_some() {
        DepSource::Path
    } else if d.url.is_some() {
        DepSource::Url
    } else {
        match d.source.as_deref().map(index_key) {
            Some(n) if n == "pypi" => DepSource::Registry(RegistryRef::Default),
            Some(n) => DepSource::Registry(RegistryRef::Named(n)),
            None => DepSource::Registry(RegistryRef::Default),
        }
    }
}

/// A Poetry bare version (`"1.2.3"`) is an exact pin.
fn poetry_resolved(v: &str) -> Option<String> {
    let v = v.trim();
    let bare = !v.is_empty()
        && v.chars().next().is_some_and(|c| c.is_ascii_digit())
        && !v.contains(['^', '~', '<', '>', '=', '*', ',', '|', ' ', '!']);
    if bare {
        Some(v.to_string())
    } else {
        exact_pin(v)
    }
}

fn poetry_dep(name: &str, spec: &PoetrySpec, scope: DepScope) -> Dependency {
    let (declared, source, optional) = match spec {
        PoetrySpec::Version(v) => (
            Some(v.clone()),
            DepSource::Registry(RegistryRef::Default),
            false,
        ),
        PoetrySpec::Detail(d) => (
            d.version.clone(),
            poetry_detail_source(d),
            d.optional == Some(true),
        ),
        PoetrySpec::Multi(list) => {
            let srcs: Vec<DepSource> = list.iter().map(poetry_detail_source).collect();
            let src = srcs
                .iter()
                .find(|s| **s != DepSource::Registry(RegistryRef::Default))
                .cloned()
                .unwrap_or(DepSource::Registry(RegistryRef::Default));
            (None, src, list.iter().any(|d| d.optional == Some(true)))
        }
    };
    let scope = if optional && scope == DepScope::Normal {
        DepScope::Optional
    } else {
        scope
    };
    Dependency {
        name: name.to_string(),
        scope,
        resolved: declared.as_deref().and_then(poetry_resolved),
        declared,
        source,
    }
}

fn pep508_dep(path: &str, raw: &str, scope: DepScope) -> Result<Dependency, ParseError> {
    let r = parse_req(raw).ok_or_else(|| ParseError::new(path, ParseErrorKind::InvalidSyntax))?;
    Ok(Dependency {
        resolved: r.resolved(),
        name: r.name,
        scope,
        declared: r.spec,
        source: r.source,
    })
}

fn pyproject_deps(path: &str, m: &PyprojectDto) -> Result<Vec<Dependency>, ParseError> {
    let mut out = Vec::new();
    if let Some(p) = &m.project {
        for r in &p.dependencies {
            out.push(pep508_dep(path, r, DepScope::Normal)?);
        }
        for r in p.optional.values().flatten() {
            out.push(pep508_dep(path, r, DepScope::Optional)?);
        }
    }
    for item in m.groups.values().flatten() {
        if let GroupItem::Req(r) = item {
            out.push(pep508_dep(path, r, DepScope::Dev)?);
        }
    }
    let tool = m.tool.as_ref();
    if let Some(p) = tool.and_then(|t| t.poetry.as_ref()) {
        let tables = std::iter::once((&p.dependencies, DepScope::Normal))
            .chain(std::iter::once((&p.dev, DepScope::Dev)))
            .chain(p.group.values().map(|g| (&g.dependencies, DepScope::Dev)));
        for (table, scope) in tables {
            for (name, spec) in table {
                if !name.eq_ignore_ascii_case("python") {
                    out.push(poetry_dep(name, spec, scope));
                }
            }
        }
    }
    if let Some(uv) = tool.and_then(|t| t.uv.as_ref()) {
        for r in &uv.dev {
            out.push(pep508_dep(path, r, DepScope::Dev)?);
        }
        let pins: BTreeMap<String, DepSource> = uv
            .sources
            .iter()
            .map(|(k, s)| (normalize(Ecosystem::Pypi, k), uv_sources(s)))
            .collect();
        for d in &mut out {
            if d.source == DepSource::Registry(RegistryRef::Default) {
                if let Some(s) = pins.get(&normalize(Ecosystem::Pypi, &d.name)) {
                    d.source = s.clone();
                }
            }
        }
    }
    Ok(out)
}

/// Resolved versions (and a `Default` dep's source) from the lock.
fn apply_lock(deps: &mut [Dependency], lock: &PyLock) {
    for d in deps {
        let Some(e) = lock.get(&d.name) else {
            continue;
        };
        d.resolved = Some(e.version.clone());
        if d.source == DepSource::Registry(RegistryRef::Default) {
            d.source = e.source.clone();
        }
    }
}

impl EcosystemAdapter for PypiAdapter {
    fn ecosystem(&self) -> Ecosystem {
        Ecosystem::Pypi
    }

    fn is_manifest(&self, file_name: &str) -> bool {
        file_name == PYPROJECT || is_requirements(file_name)
    }

    fn is_lockfile(&self, file_name: &str) -> bool {
        file_name == POETRY_LOCK || file_name == UV_LOCK
    }

    /// `pyproject.toml` only: `poetry.lock`, then `uv.lock`, in the same dir
    /// then each ancestor (a uv workspace locks at its root).
    fn lockfile_for(&self, manifest: &str, files: &FileSet) -> Option<String> {
        if file_name(manifest) != PYPROJECT {
            return None;
        }
        dir_and_ancestors(parent_dir(manifest))
            .into_iter()
            .flat_map(|d| [join(d, POETRY_LOCK), join(d, UV_LOCK)])
            .find(|p| files.contains(p))
    }

    /// `project.name` and `tool.poetry.name`. A requirements file (not
    /// TOML) or an unparseable pyproject declares nothing.
    fn declared_names(&self, manifest: &str) -> Vec<String> {
        let Ok(dto) = parse_toml::<NamesDto>(PYPROJECT, manifest) else {
            return Vec::new();
        };
        let poetry = dto.tool.and_then(|t| t.poetry).and_then(|p| p.name);
        dto.project
            .and_then(|p| p.name)
            .into_iter()
            .chain(poetry)
            .collect()
    }

    fn parse(&self, input: &ParseInput<'_>) -> Result<ParsedGroup, ParseError> {
        let path = input.manifest_path;
        if file_name(path) != PYPROJECT {
            return Ok(ParsedGroup {
                direct: requirements::parse(path, input.manifest)?,
                lock_entries: Vec::new(),
                has_lockfile: false,
            });
        }
        let m: PyprojectDto = parse_toml(path, input.manifest)?;
        let mut direct = pyproject_deps(path, &m)?;
        let lock = match input.lockfile {
            Some((lp, text)) => Some(pypi_lock::parse(lp, text)?),
            None => None,
        };
        if let Some(l) = &lock {
            apply_lock(&mut direct, l);
        }
        Ok(ParsedGroup {
            direct,
            lock_entries: lock.map(PyLock::into_entries).unwrap_or_default(),
            has_lockfile: input.lockfile.is_some(),
        })
    }
}

#[cfg(test)]
#[path = "pypi_tests.rs"]
mod tests;
