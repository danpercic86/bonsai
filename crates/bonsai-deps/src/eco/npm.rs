//! npm family adapter (E2 npm row): `package.json` manifests with
//! `package-lock.json` / `npm-shrinkwrap.json`, `pnpm-lock.yaml` and
//! `yarn.lock` (v1 and berry) lockfiles.

use std::collections::BTreeMap;

use serde_json::Value;

use crate::adapter::{
    dir_and_ancestors, file_name, join, parent_dir, DepSource, Dependency, EcosystemAdapter,
    LockEntry, ParseError, ParseInput, ParsedGroup, RegistryRef,
};
use crate::discover::FileSet;
use crate::model::{DepScope, Ecosystem, ParseErrorKind};
use crate::registry_ctx::extract_host;

use super::{npm_lock, pnpm_lock, yarn_lock};

pub struct NpmAdapter;

pub(crate) const MANIFEST: &str = "package.json";

/// Lockfile priority when several share a directory (deterministic; npm
/// itself prefers the shrinkwrap over package-lock).
pub(crate) const LOCKFILES: &[&str] = &[
    "npm-shrinkwrap.json",
    "package-lock.json",
    "pnpm-lock.yaml",
    "yarn.lock",
];

const DEP_GROUPS: &[(&str, DepScope)] = &[
    ("dependencies", DepScope::Normal),
    ("devDependencies", DepScope::Dev),
    ("optionalDependencies", DepScope::Optional),
    ("peerDependencies", DepScope::Peer),
];

/// One declared dependency, as the lock parsers need it.
pub(crate) struct DirectSpec {
    /// The manifest key (the install name; an alias for `npm:` specs).
    pub(crate) key: String,
    pub(crate) declared: String,
}

/// What a lock parser found: every entry, plus the resolution of each
/// direct dependency of the manifest being parsed (keyed by manifest key).
#[derive(Default)]
pub(crate) struct LockData {
    pub(crate) entries: Vec<LockEntry>,
    pub(crate) resolved: BTreeMap<String, (String, DepSource)>,
}

impl EcosystemAdapter for NpmAdapter {
    fn ecosystem(&self) -> Ecosystem {
        Ecosystem::Npm
    }

    fn is_manifest(&self, file_name: &str) -> bool {
        file_name == MANIFEST
    }

    fn is_lockfile(&self, file_name: &str) -> bool {
        LOCKFILES.contains(&file_name)
    }

    fn lockfile_for(&self, manifest: &str, files: &FileSet) -> Option<String> {
        dir_and_ancestors(parent_dir(manifest))
            .into_iter()
            .find_map(|dir| {
                LOCKFILES
                    .iter()
                    .map(|l| join(dir, l))
                    .find(|p| files.contains(p))
            })
    }

    fn declared_names(&self, manifest: &str) -> Vec<String> {
        serde_json::from_str::<Value>(manifest)
            .ok()
            .and_then(|v| v.get("name").and_then(Value::as_str).map(str::to_string))
            .into_iter()
            .collect()
    }

    fn parse(&self, input: &ParseInput<'_>) -> Result<ParsedGroup, ParseError> {
        let manifest: Value = serde_json::from_str(input.manifest)
            .map_err(|e| ParseError::from_json(input.manifest_path, &e))?;
        if !manifest.is_object() {
            return Err(ParseError::new(
                input.manifest_path,
                ParseErrorKind::InvalidSyntax,
            ));
        }
        let declared = declared_deps(&manifest);
        let specs: Vec<DirectSpec> = declared
            .iter()
            .map(|(key, spec, _)| DirectSpec {
                key: key.clone(),
                declared: spec.clone(),
            })
            .collect();
        let lock = match input.lockfile {
            Some((path, text)) => {
                let importer = importer_dir(parent_dir(input.manifest_path), parent_dir(path));
                parse_lock(path, text, &importer, &specs)?
            }
            None => LockData::default(),
        };
        let direct = declared
            .into_iter()
            .map(|(key, spec, scope)| dependency(&key, &spec, scope, &lock))
            .collect();
        Ok(ParsedGroup {
            direct,
            lock_entries: lock.entries,
            has_lockfile: input.lockfile.is_some(),
        })
    }
}

fn parse_lock(
    path: &str,
    text: &str,
    importer: &str,
    specs: &[DirectSpec],
) -> Result<LockData, ParseError> {
    match file_name(path) {
        "pnpm-lock.yaml" => pnpm_lock::parse(path, text, importer, specs),
        "yarn.lock" => yarn_lock::parse(path, text, specs),
        _ => npm_lock::parse(path, text, importer, specs),
    }
}

/// The manifest's directory relative to the lockfile's (`""` when equal).
/// `lockfile_for` only picks the same dir or an ancestor, so a strip works.
fn importer_dir(manifest_dir: &str, lock_dir: &str) -> String {
    if lock_dir.is_empty() {
        return manifest_dir.to_string();
    }
    manifest_dir
        .strip_prefix(lock_dir)
        .map(|r| r.trim_start_matches('/').to_string())
        .unwrap_or_default()
}

/// `(key, spec, scope)` for every string-valued entry of the dep groups.
fn declared_deps(manifest: &Value) -> Vec<(String, String, DepScope)> {
    let mut out = Vec::new();
    for (group, scope) in DEP_GROUPS {
        let Some(map) = manifest.get(*group).and_then(Value::as_object) else {
            continue;
        };
        for (key, spec) in map {
            if let Some(spec) = spec.as_str() {
                out.push((key.clone(), spec.to_string(), *scope));
            }
        }
    }
    out
}

fn dependency(key: &str, spec: &str, scope: DepScope, lock: &LockData) -> Dependency {
    let decl = classify_spec(key, spec);
    let locked = lock.resolved.get(key);
    let (source, resolved) = match (&decl.source, locked) {
        // A registry spec takes the lock's view of where it came from (a
        // custom tarball host, a workspace link, …).
        (DepSource::Registry(RegistryRef::Default), Some((v, src))) => {
            (src.clone(), Some(v.clone()))
        }
        (DepSource::Registry(_), Some((v, _))) => (decl.source.clone(), Some(v.clone())),
        _ => (decl.source.clone(), None),
    };
    Dependency {
        name: decl.name,
        scope,
        declared: decl.declared,
        resolved,
        source,
    }
}

pub(crate) struct SpecInfo {
    pub(crate) source: DepSource,
    /// The package actually installed (the target of an `npm:` alias).
    pub(crate) name: String,
    pub(crate) declared: Option<String>,
}

fn starts_with_ci(s: &str, prefix: &str) -> bool {
    s.len() >= prefix.len()
        && s.is_char_boundary(prefix.len())
        && s[..prefix.len()].eq_ignore_ascii_case(prefix)
}

const PATH_PROTOCOLS: &[&str] = &["file:", "link:", "portal:", "exec:", "patch:"];
const GIT_PROTOCOLS: &[&str] = &[
    "git+",
    "git:",
    "github:",
    "gitlab:",
    "bitbucket:",
    "gist:",
    "ssh:",
];

/// E2 npm "Local" column, plus `[pnpm 11]` named-registry alias protocols:
/// any other `<alias>:` prefix routes to a registry we cannot see, so it is
/// `Registry(Named(alias))` (always CustomRegistry).
pub(crate) fn classify_spec(key: &str, spec: &str) -> SpecInfo {
    let s = spec.trim();
    let info = |source| SpecInfo {
        source,
        name: key.to_string(),
        declared: Some(s.to_string()),
    };
    if PATH_PROTOCOLS.iter().any(|p| starts_with_ci(s, p)) {
        return info(DepSource::Path);
    }
    if starts_with_ci(s, "workspace:") {
        return info(DepSource::Workspace);
    }
    if GIT_PROTOCOLS.iter().any(|p| starts_with_ci(s, p)) {
        return info(DepSource::Git);
    }
    if starts_with_ci(s, "http://") || starts_with_ci(s, "https://") {
        return info(DepSource::Url);
    }
    if starts_with_ci(s, "npm:") {
        let (name, range) = split_name_version(&s[4..]);
        return SpecInfo {
            source: DepSource::Registry(RegistryRef::Default),
            name: name.to_string(),
            declared: range.map(str::to_string),
        };
    }
    if starts_with_ci(s, "catalog:") {
        // pnpm catalogs: the range lives in pnpm-workspace.yaml; the
        // resolved version comes from the lock.
        return SpecInfo {
            source: DepSource::Registry(RegistryRef::Default),
            name: key.to_string(),
            declared: None,
        };
    }
    let b = s.as_bytes();
    let drive =
        b.len() >= 3 && b[0].is_ascii_alphabetic() && b[1] == b':' && matches!(b[2], b'/' | b'\\');
    // Paths only: `~1.2.3` is a tilde RANGE, `.5` is not a spec at all.
    let path_like =
        matches!(s, "." | ".." | "~") || ["./", "../", "/", "~/"].iter().any(|p| s.starts_with(p));
    if drive || path_like {
        return info(DepSource::Path);
    }
    if let Some((alias, _)) = s.split_once(':') {
        if !alias.is_empty()
            && alias
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_')
        {
            return info(DepSource::Registry(RegistryRef::Named(
                alias.to_ascii_lowercase(),
            )));
        }
    }
    // `user/repo[#ref]` is GitHub shorthand. Ranges and dist-tags never
    // contain `/`.
    if s.contains('/') && !s.starts_with('@') {
        return info(DepSource::Git);
    }
    info(DepSource::Registry(RegistryRef::Default))
}

/// `name@version` / `@scope/name@version` ⇒ (name, Some(version)); a bare
/// name ⇒ (name, None).
pub(crate) fn split_name_version(s: &str) -> (&str, Option<&str>) {
    let from = usize::from(s.starts_with('@'));
    match s[from..].find('@') {
        Some(i) => (&s[..from + i], Some(&s[from + i + 1..])),
        None => (s, None),
    }
}

/// Source of a lock entry from its `resolved` / `tarball` URL.
pub(crate) fn source_from_resolved(resolved: &str) -> DepSource {
    let r = resolved.trim();
    if PATH_PROTOCOLS.iter().any(|p| starts_with_ci(r, p)) {
        return DepSource::Path;
    }
    if GIT_PROTOCOLS.iter().any(|p| starts_with_ci(r, p)) {
        return DepSource::Git;
    }
    if starts_with_ci(r, "http://") || starts_with_ci(r, "https://") {
        // An unparseable URL is never sent: Url is Local.
        return extract_host(r).map_or(DepSource::Url, |h| {
            DepSource::Registry(RegistryRef::Host(h))
        });
    }
    DepSource::Registry(RegistryRef::Default)
}

#[cfg(test)]
#[path = "npm_tests.rs"]
mod tests;
