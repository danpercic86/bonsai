//! `Cargo.lock` (`[[package]]` name / version / source), lock versions 1–4.

use serde::Deserialize;

use crate::adapter::{DepSource, LockEntry, ParseError, RegistryRef};
use crate::model::ParseErrorKind;

use super::cargo::{cargo_url_host, parse_toml};

/// The two exact crates.io source ids (git index and sparse index). The
/// P125a catalog deliberately leaves `github.com` out of the public hosts,
/// so these map to `Default` by exact string, never by host.
const CRATES_IO_SOURCES: &[&str] = &[
    "registry+https://github.com/rust-lang/crates.io-index",
    "sparse+https://index.crates.io/",
];

/// Highest lock `version` understood (Cargo 1.78+ writes 4).
const MAX_LOCK_VERSION: u32 = 4;

#[derive(Deserialize)]
struct LockDto {
    #[serde(default)]
    version: Option<u32>,
    #[serde(default)]
    package: Vec<PackageDto>,
}

#[derive(Deserialize)]
struct PackageDto {
    name: String,
    version: String,
    #[serde(default)]
    source: Option<String>,
}

/// A parsed lock: every `[[package]]`.
pub(crate) struct CargoLock {
    entries: Vec<LockEntry>,
}

/// E2: no `source` ⇒ a workspace member / path crate; the exact crates.io
/// ids ⇒ `Default`; `git+` ⇒ Git; another registry ⇒ its host. Anything
/// unrecognised is `Url` (Local, never sent).
pub(crate) fn lock_source(source: Option<&str>) -> DepSource {
    let Some(s) = source.map(str::trim) else {
        return DepSource::Workspace;
    };
    if CRATES_IO_SOURCES.contains(&s) {
        return DepSource::Registry(RegistryRef::Default);
    }
    if s.starts_with("git+") {
        return DepSource::Git;
    }
    if s.starts_with("registry+") || s.starts_with("sparse+") {
        return cargo_url_host(s).map_or(DepSource::Url, |h| {
            DepSource::Registry(RegistryRef::Host(h))
        });
    }
    DepSource::Url
}

pub(crate) fn parse(path: &str, text: &str) -> Result<CargoLock, ParseError> {
    let dto: LockDto = parse_toml(path, text)?;
    if dto.version.is_some_and(|v| v > MAX_LOCK_VERSION) {
        return Err(ParseError::new(path, ParseErrorKind::UnsupportedVersion));
    }
    let entries = dto
        .package
        .into_iter()
        .map(|p| LockEntry {
            source: lock_source(p.source.as_deref()),
            name: p.name,
            version: p.version,
        })
        .collect();
    Ok(CargoLock { entries })
}

fn version_key(v: &str) -> Option<semver::Version> {
    semver::Version::parse(v.trim()).ok()
}

impl CargoLock {
    pub(crate) fn into_entries(self) -> Vec<LockEntry> {
        self.entries
    }

    /// `(version, source)` a dependency named `name` (requirement `req`)
    /// resolved to. Entries matching `req` are preferred; when the
    /// candidates disagree on the source, a non-`Default` one wins (fail
    /// toward private), else the highest version.
    pub(crate) fn resolve(&self, name: &str, req: Option<&str>) -> Option<(String, DepSource)> {
        let named: Vec<&LockEntry> = self.entries.iter().filter(|e| e.name == name).collect();
        let req = req.and_then(|r| semver::VersionReq::parse(r.trim()).ok());
        let matching: Vec<&LockEntry> = match &req {
            Some(r) => named
                .iter()
                .copied()
                .filter(|e| version_key(&e.version).is_some_and(|v| r.matches(&v)))
                .collect(),
            None => Vec::new(),
        };
        let pool = if matching.is_empty() { named } else { matching };
        let default = DepSource::Registry(RegistryRef::Default);
        let pick = pool
            .iter()
            .copied()
            .find(|e| e.source != default)
            .or_else(|| pool.iter().copied().max_by_key(|e| version_key(&e.version)))?;
        Some((pick.version.clone(), pick.source.clone()))
    }
}
