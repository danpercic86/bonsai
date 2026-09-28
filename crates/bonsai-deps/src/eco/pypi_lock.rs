//! `poetry.lock` and `uv.lock` (E2 PyPI row): TOML `[[package]]` tables with
//! name / version / source. Parsed through `parse_toml` (BOM strip + the
//! §4.2 depth pre-scan `[HIGH-2]`, span-only errors `[MEDIUM-1]`).

use std::collections::BTreeMap;

use serde::de::IgnoredAny;
use serde::Deserialize;

use crate::adapter::{DepSource, LockEntry, ParseError, RegistryRef};
use crate::model::{Ecosystem, ParseErrorKind};
use crate::privacy::normalize;
use crate::registry_ctx::extract_host;

use super::cargo::parse_toml;

pub(crate) const POETRY_LOCK: &str = "poetry.lock";
pub(crate) const UV_LOCK: &str = "uv.lock";

/// Highest `uv.lock` `version` understood.
const MAX_UV_LOCK_VERSION: u32 = 1;

/// The exact PyPI simple index uv writes for the default registry.
const PYPI_SIMPLE: &[&str] = &["https://pypi.org/simple", "https://pypi.org/simple/"];

#[derive(Deserialize)]
struct PoetryLockDto {
    #[serde(default)]
    package: Vec<PoetryPkg>,
}

#[derive(Deserialize)]
struct PoetryPkg {
    name: String,
    version: String,
    #[serde(default)]
    source: Option<PoetrySource>,
}

/// `[package.source]`: `type` = legacy | git | directory | file | url.
#[derive(Deserialize)]
struct PoetrySource {
    #[serde(default, rename = "type")]
    kind: Option<String>,
    #[serde(default)]
    url: Option<String>,
}

#[derive(Deserialize)]
struct UvLockDto {
    #[serde(default)]
    version: Option<u32>,
    #[serde(default)]
    package: Vec<UvPkg>,
}

#[derive(Deserialize)]
struct UvPkg {
    name: String,
    /// Absent for `dynamic` / virtual workspace members.
    #[serde(default)]
    version: Option<String>,
    #[serde(default)]
    source: Option<UvSource>,
}

/// `source = { registry = … }` and the local / VCS kinds (`url` and any
/// unknown kind fall through to `Url`).
#[derive(Deserialize)]
struct UvSource {
    #[serde(default)]
    registry: Option<String>,
    #[serde(default)]
    git: Option<IgnoredAny>,
    #[serde(default)]
    path: Option<IgnoredAny>,
    #[serde(default)]
    directory: Option<IgnoredAny>,
    #[serde(default)]
    editable: Option<IgnoredAny>,
    #[serde(default)]
    r#virtual: Option<IgnoredAny>,
}

fn poetry_source(s: Option<&PoetrySource>) -> DepSource {
    let Some(s) = s else {
        return DepSource::Registry(RegistryRef::Default);
    };
    match s.kind.as_deref().map(str::to_ascii_lowercase).as_deref() {
        // E2: a poetry.lock `[package.source]` ⇒ CustomRegistry (a legacy
        // index that is pypi.org itself stays public by host).
        Some("legacy") => s
            .url
            .as_deref()
            .and_then(extract_host)
            .map_or(DepSource::Url, |h| {
                DepSource::Registry(RegistryRef::Host(h))
            }),
        Some("git") => DepSource::Git,
        Some("directory" | "file") => DepSource::Path,
        // `url` and anything unknown: a direct reference, never sent.
        _ => DepSource::Url,
    }
}

fn uv_source(s: Option<&UvSource>) -> DepSource {
    let Some(s) = s else {
        return DepSource::Registry(RegistryRef::Default);
    };
    if let Some(reg) = &s.registry {
        let r = reg.trim();
        if PYPI_SIMPLE.contains(&r) {
            return DepSource::Registry(RegistryRef::Default);
        }
        // A flat / local index path has no host: local, never sent.
        return extract_host(r).map_or(DepSource::Path, |h| {
            DepSource::Registry(RegistryRef::Host(h))
        });
    }
    if s.editable.is_some() || s.r#virtual.is_some() {
        DepSource::Workspace
    } else if s.git.is_some() {
        DepSource::Git
    } else if s.path.is_some() || s.directory.is_some() {
        DepSource::Path
    } else {
        // `url` or an unknown kind: a direct reference, never sent.
        DepSource::Url
    }
}

/// A parsed lock: its entries, indexed by PEP 503 name.
pub(crate) struct PyLock {
    entries: Vec<LockEntry>,
    by_name: BTreeMap<String, usize>,
}

impl PyLock {
    fn new(entries: Vec<LockEntry>) -> Self {
        let mut by_name = BTreeMap::new();
        for (i, e) in entries.iter().enumerate() {
            by_name
                .entry(normalize(Ecosystem::Pypi, &e.name))
                .or_insert(i);
        }
        PyLock { entries, by_name }
    }

    /// The first lock entry for `name` (PEP 503-normalized match).
    pub(crate) fn get(&self, name: &str) -> Option<&LockEntry> {
        self.by_name
            .get(&normalize(Ecosystem::Pypi, name))
            .and_then(|i| self.entries.get(*i))
    }

    pub(crate) fn into_entries(self) -> Vec<LockEntry> {
        self.entries
    }
}

/// Parses `poetry.lock` or `uv.lock` (by file name).
pub(crate) fn parse(path: &str, text: &str) -> Result<PyLock, ParseError> {
    let is_uv = crate::adapter::file_name(path) == UV_LOCK;
    let entries = if is_uv {
        let dto: UvLockDto = parse_toml(path, text)?;
        if dto.version.is_some_and(|v| v > MAX_UV_LOCK_VERSION) {
            return Err(ParseError::new(path, ParseErrorKind::UnsupportedVersion));
        }
        dto.package
            .into_iter()
            .filter_map(|p| {
                let source = uv_source(p.source.as_ref());
                Some(LockEntry {
                    version: p.version?,
                    name: p.name,
                    source,
                })
            })
            .collect()
    } else {
        let dto: PoetryLockDto = parse_toml(path, text)?;
        dto.package
            .into_iter()
            .map(|p| LockEntry {
                source: poetry_source(p.source.as_ref()),
                name: p.name,
                version: p.version,
            })
            .collect()
    };
    Ok(PyLock::new(entries))
}
