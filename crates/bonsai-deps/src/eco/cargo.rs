//! Cargo adapter (E2 Cargo row): `Cargo.toml` manifests with a `Cargo.lock`
//! in the same directory or an ancestor (the workspace root).
//!
//! Every TOML parse is preceded by the §4.2 depth pre-scan `[HIGH-2]`, and
//! a `toml::de::Error` is mapped by span only `[MEDIUM-1]`.
//!
//! Decisions beyond the E2 row (fail toward "not sent"):
//! - `workspace = true` is resolved through the lock (the root manifest is
//!   not part of `ParseInput`). No lock entry ⇒ `Workspace` (Local): the
//!   dependency is still checked in the root's `[workspace.dependencies]`
//!   group, and a name is never sent blind.
//! - `[patch.*]` and `[replace]` names are DECLARED names (`declared_names`),
//!   i.e. Local for the whole repo: a patched crate is provided by a path,
//!   a git repo or another registry, and patching a name that exists only
//!   privately is the common case.

use std::collections::BTreeMap;

use serde::de::{DeserializeOwned, IgnoredAny};
use serde::Deserialize;

use crate::adapter::{
    dir_and_ancestors, join, parent_dir, DepSource, Dependency, EcosystemAdapter, ParseError,
    ParseInput, ParsedGroup, RegistryRef,
};
use crate::discover::FileSet;
use crate::model::{DepScope, Ecosystem};
use crate::registry_ctx::extract_host;
use crate::toml_scan::check_depth;

use super::cargo_lock::{self, CargoLock};

pub struct CargoAdapter;

pub(crate) const MANIFEST: &str = "Cargo.toml";
pub(crate) const LOCKFILE: &str = "Cargo.lock";

/// Registry / source names compare case-insensitively with `-` == `_`
/// (`CARGO_REGISTRIES_MY_CORP_INDEX` names the registry `my-corp`).
pub(crate) fn registry_key(name: &str) -> String {
    name.trim().to_ascii_lowercase().replace('_', "-")
}

/// Host of a Cargo index / registry URL (`sparse+https://…`,
/// `registry+https://…` or a plain http(s) URL). Anything else ⇒ None.
pub(crate) fn cargo_url_host(raw: &str) -> Option<String> {
    let v = raw.trim();
    let v = ["sparse+", "registry+"]
        .iter()
        .find_map(|p| strip_prefix_ci(v, p))
        .unwrap_or(v);
    extract_host(v)
}

fn strip_prefix_ci<'a>(s: &'a str, prefix: &str) -> Option<&'a str> {
    (s.len() >= prefix.len()
        && s.is_char_boundary(prefix.len())
        && s[..prefix.len()].eq_ignore_ascii_case(prefix))
    .then(|| &s[prefix.len()..])
}

/// `[HIGH-2]` pre-scan, then a typed parse. Errors carry kind + position.
pub(crate) fn parse_toml<T: DeserializeOwned>(path: &str, text: &str) -> Result<T, ParseError> {
    let text = text.strip_prefix('\u{FEFF}').unwrap_or(text);
    check_depth(text).map_err(|e| ParseError::too_deep(path, e))?;
    toml::from_str(text).map_err(|e| ParseError::from_toml(path, text, &e))
}

type DepTable = BTreeMap<String, DepSpec>;

#[derive(Deserialize)]
#[serde(untagged)]
enum DepSpec {
    Version(String),
    Detailed(DetailDto),
}

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
struct DetailDto {
    #[serde(default)]
    version: Option<String>,
    #[serde(default)]
    path: Option<IgnoredAny>,
    #[serde(default)]
    git: Option<IgnoredAny>,
    #[serde(default)]
    registry: Option<String>,
    #[serde(default)]
    registry_index: Option<String>,
    #[serde(default)]
    package: Option<String>,
    #[serde(default)]
    workspace: Option<bool>,
    #[serde(default)]
    optional: Option<bool>,
}

#[derive(Deserialize)]
struct Tables {
    #[serde(default)]
    dependencies: DepTable,
    #[serde(default, rename = "dev-dependencies", alias = "dev_dependencies")]
    dev: DepTable,
    #[serde(default, rename = "build-dependencies", alias = "build_dependencies")]
    build: DepTable,
}

#[derive(Deserialize)]
struct PackageDto {
    #[serde(default)]
    name: Option<String>,
}

#[derive(Deserialize)]
struct WorkspaceDto {
    #[serde(default)]
    dependencies: DepTable,
}

/// The dependency tables only; `[package]` & co. are not deserialized.
#[derive(Deserialize)]
struct ManifestDto {
    #[serde(default)]
    dependencies: DepTable,
    #[serde(default, rename = "dev-dependencies", alias = "dev_dependencies")]
    dev: DepTable,
    #[serde(default, rename = "build-dependencies", alias = "build_dependencies")]
    build: DepTable,
    #[serde(default)]
    target: BTreeMap<String, Tables>,
    #[serde(default)]
    workspace: Option<WorkspaceDto>,
}

/// Only what `declared_names` needs.
#[derive(Deserialize)]
struct NamesDto {
    #[serde(default)]
    package: Option<PackageDto>,
    #[serde(default)]
    patch: BTreeMap<String, BTreeMap<String, IgnoredAny>>,
    #[serde(default)]
    replace: BTreeMap<String, IgnoredAny>,
}

/// The crate name of a `[replace]` package-id spec (`foo:1.0.0`,
/// `foo@1.0.0`, `https://…crates.io-index#foo@1.0.0`).
fn pkgid_name(spec: &str) -> Option<&str> {
    let tail = spec.rsplit('#').next().unwrap_or(spec);
    let name = tail.split(['@', ':']).next().unwrap_or(tail).trim();
    (!name.is_empty() && !name.contains('/')).then_some(name)
}

/// How the manifest names a dependency's source, before the lock.
enum Declared {
    Source(DepSource),
    /// `workspace = true`: resolved through the lock.
    Inherited,
}

fn declared_source(spec: &DepSpec) -> Declared {
    let d = match spec {
        DepSpec::Version(_) => return Declared::Source(DepSource::Registry(RegistryRef::Default)),
        DepSpec::Detailed(d) => d,
    };
    if d.workspace == Some(true) {
        return Declared::Inherited;
    }
    let src = if d.path.is_some() {
        DepSource::Path
    } else if d.git.is_some() {
        DepSource::Git
    } else if let Some(r) = &d.registry {
        DepSource::Registry(RegistryRef::Named(registry_key(r)))
    } else if let Some(idx) = &d.registry_index {
        // An index we cannot read a host from is never sent (Url ⇒ Local).
        cargo_url_host(idx).map_or(DepSource::Url, |h| {
            DepSource::Registry(RegistryRef::Host(h))
        })
    } else {
        DepSource::Registry(RegistryRef::Default)
    };
    Declared::Source(src)
}

fn dependency(key: &str, spec: &DepSpec, scope: DepScope, lock: Option<&CargoLock>) -> Dependency {
    let (name, declared, optional) = match spec {
        DepSpec::Version(v) => (key.to_string(), Some(v.clone()), false),
        DepSpec::Detailed(d) => (
            d.package.clone().unwrap_or_else(|| key.to_string()),
            d.version.clone(),
            d.optional == Some(true),
        ),
    };
    let scope = if optional && scope == DepScope::Normal {
        DepScope::Optional
    } else {
        scope
    };
    let locked = lock.and_then(|l| l.resolve(&name, declared.as_deref()));
    let (source, resolved) = match (declared_source(spec), locked) {
        // A crates.io spec takes the lock's view (a patch to a path / git
        // source, a workspace member, …).
        (Declared::Source(DepSource::Registry(RegistryRef::Default)), Some((v, src))) => {
            (src, Some(v))
        }
        (Declared::Source(s @ DepSource::Registry(_)), Some((v, _))) => (s, Some(v)),
        (Declared::Source(s), _) => (s, None),
        (Declared::Inherited, Some((v, src))) => (src, Some(v)),
        (Declared::Inherited, None) => (DepSource::Workspace, None),
    };
    Dependency {
        name,
        scope,
        declared,
        resolved,
        source,
    }
}

fn push_tables(
    out: &mut Vec<Dependency>,
    [normal, dev, build]: [&DepTable; 3],
    lock: Option<&CargoLock>,
) {
    for (table, scope) in [
        (normal, DepScope::Normal),
        (dev, DepScope::Dev),
        (build, DepScope::Build),
    ] {
        out.extend(table.iter().map(|(k, s)| dependency(k, s, scope, lock)));
    }
}

impl EcosystemAdapter for CargoAdapter {
    fn ecosystem(&self) -> Ecosystem {
        Ecosystem::Cargo
    }

    fn is_manifest(&self, file_name: &str) -> bool {
        file_name == MANIFEST
    }

    fn is_lockfile(&self, file_name: &str) -> bool {
        file_name == LOCKFILE
    }

    fn lockfile_for(&self, manifest: &str, files: &FileSet) -> Option<String> {
        dir_and_ancestors(parent_dir(manifest))
            .into_iter()
            .map(|dir| join(dir, LOCKFILE))
            .find(|p| files.contains(p))
    }

    /// `[package].name`, plus every `[patch.*]` / `[replace]` name (see the
    /// module doc). Unparseable ⇒ nothing (the group reports the error).
    fn declared_names(&self, manifest: &str) -> Vec<String> {
        let Ok(dto) = parse_toml::<NamesDto>(MANIFEST, manifest) else {
            return Vec::new();
        };
        let mut out: Vec<String> = dto.package.and_then(|p| p.name).into_iter().collect();
        for table in dto.patch.into_values() {
            out.extend(table.into_keys());
        }
        out.extend(
            dto.replace
                .keys()
                .filter_map(|k| pkgid_name(k))
                .map(str::to_string),
        );
        out
    }

    fn parse(&self, input: &ParseInput<'_>) -> Result<ParsedGroup, ParseError> {
        let m: ManifestDto = parse_toml(input.manifest_path, input.manifest)?;
        let lock = match input.lockfile {
            Some((path, text)) => Some(cargo_lock::parse(path, text)?),
            None => None,
        };
        let mut direct = Vec::new();
        push_tables(
            &mut direct,
            [&m.dependencies, &m.dev, &m.build],
            lock.as_ref(),
        );
        for t in m.target.values() {
            push_tables(
                &mut direct,
                [&t.dependencies, &t.dev, &t.build],
                lock.as_ref(),
            );
        }
        if let Some(ws) = &m.workspace {
            direct.extend(
                ws.dependencies
                    .iter()
                    .map(|(k, s)| dependency(k, s, DepScope::Normal, lock.as_ref())),
            );
        }
        Ok(ParsedGroup {
            direct,
            lock_entries: lock.map(CargoLock::into_entries).unwrap_or_default(),
            has_lockfile: input.lockfile.is_some(),
        })
    }
}

#[cfg(test)]
#[path = "cargo_tests.rs"]
mod tests;
