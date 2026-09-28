//! `pnpm-lock.yaml` v6–v9 through the bounded YAML event reader `[HIGH-2]`.
//!
//! `[HIGH-1]` pnpm v6–v9 locks omit the registry host for registry
//! packages, so a plain entry is `RegistryRef::Default` and the registry
//! config (`RegistryContext`) is the only signal.
//!
//! pnpm 11 may write a second YAML document (config / package-manager
//! dependencies); `packages` of every document are read, the importers of
//! the first document that has them.

use crate::adapter::{DepSource, LockEntry, ParseError, RegistryRef};
use crate::model::ParseErrorKind;
use crate::yaml_events::{parse_documents, YNode};

use super::npm::{source_from_resolved, split_name_version, DirectSpec, LockData};

const DEP_GROUPS: &[&str] = &["dependencies", "devDependencies", "optionalDependencies"];

/// Materialise only what this parser reads.
fn keep(p: &[String]) -> bool {
    let at = |i: usize| p.get(i).map(String::as_str);
    match at(0) {
        None => true,
        Some("lockfileVersion") => true,
        Some("importers") => match p.len() {
            1 | 2 => true,
            3 => DEP_GROUPS.contains(&p[2].as_str()),
            4 => true,
            _ => matches!(at(4), Some("specifier" | "version")) && p.len() == 5,
        },
        // v6 single-project layout: dependency groups at the top level.
        Some(g) if DEP_GROUPS.contains(&g) => {
            p.len() <= 2 || (p.len() == 3 && matches!(at(2), Some("specifier" | "version")))
        }
        Some("packages") => match p.len() {
            1 | 2 => true,
            3 => matches!(at(2), Some("resolution" | "version" | "name")),
            4 => at(2) == Some("resolution"),
            _ => false,
        },
        _ => false,
    }
}

fn major(v: &YNode) -> Option<u32> {
    let s = v.as_str()?;
    s.trim_matches(['\'', '"']).split('.').next()?.parse().ok()
}

pub(crate) fn parse(
    path: &str,
    text: &str,
    importer: &str,
    direct: &[DirectSpec],
) -> Result<LockData, ParseError> {
    let docs = parse_documents(text, &keep).map_err(|e| ParseError::from_yaml(path, e))?;
    let main = docs
        .iter()
        .find(|d| d.get("lockfileVersion").is_some())
        .ok_or_else(|| ParseError::new(path, ParseErrorKind::InvalidSyntax))?;
    match main.get("lockfileVersion").and_then(major) {
        Some(6..=9) => {}
        _ => return Err(ParseError::new(path, ParseErrorKind::UnsupportedVersion)),
    }
    let mut data = LockData::default();
    for doc in &docs {
        for (key, entry) in doc.get("packages").map_or(&[][..], YNode::entries) {
            if let Some(e) = package_entry(key, entry) {
                data.entries.push(e);
            }
        }
    }
    let importer_key = if importer.is_empty() { "." } else { importer };
    let project = docs
        .iter()
        .find_map(|d| d.get("importers"))
        .map_or(Some(main), |imps| imps.get(importer_key));
    if let Some(project) = project {
        for group in DEP_GROUPS {
            for (name, dep) in project.get(group).map_or(&[][..], YNode::entries) {
                if !direct.iter().any(|d| &d.key == name) {
                    continue;
                }
                // v6+ `{specifier, version}`; tolerate a bare version scalar.
                let raw = dep.get("version").and_then(YNode::as_str).or(dep.as_str());
                if let Some(raw) = raw {
                    data.resolved.insert(
                        name.clone(),
                        resolve_importer_version(name, raw, &data.entries),
                    );
                }
            }
        }
    }
    Ok(data)
}

/// Drops the v6 leading `/` and any `(peer@x)` suffix.
fn clean_key(key: &str) -> &str {
    let k = key.strip_prefix('/').unwrap_or(key);
    k.split_once('(').map_or(k, |(a, _)| a)
}

fn package_entry(key: &str, entry: &YNode) -> Option<LockEntry> {
    let (key_name, key_ver) = split_name_version(clean_key(key));
    let name = entry
        .get("name")
        .and_then(YNode::as_str)
        .unwrap_or(key_name);
    let version = entry.get("version").and_then(YNode::as_str).or(key_ver)?;
    let res = entry.get("resolution");
    let source = if let Some(t) = res.and_then(|r| r.get("tarball")).and_then(YNode::as_str) {
        source_from_resolved(t)
    } else if res.is_some_and(|r| r.get("directory").is_some()) {
        DepSource::Path
    } else if res.is_some_and(|r| {
        r.get("repo").is_some() || r.get("type").and_then(YNode::as_str) == Some("git")
    }) {
        DepSource::Git
    } else {
        // A non-semver key version (`file:…`, `https://…`, `github.com/…`).
        version_source(version)
    };
    Some(LockEntry {
        name: name.to_string(),
        version: version.to_string(),
        source,
    })
}

/// Source implied by a version string that is not a plain version.
fn version_source(v: &str) -> DepSource {
    if v.starts_with("link:") {
        return DepSource::Workspace;
    }
    let src = source_from_resolved(v);
    if src != DepSource::Registry(RegistryRef::Default) {
        return src;
    }
    // `github.com/u/r/<sha>` style keys (pnpm git deps).
    if v.contains('/') {
        return DepSource::Git;
    }
    src
}

/// An importer `version`: `1.2.3`, `1.2.3(peer@1)`, `link:../x`,
/// `file:…`, or an alias `real@1.0.0` / `/real@1.0.0`.
fn resolve_importer_version(key: &str, raw: &str, entries: &[LockEntry]) -> (String, DepSource) {
    let v = clean_key(raw);
    let (name, ver) = match split_name_version(v) {
        (n, Some(ver)) if !n.is_empty() && !v.starts_with("link:") && !v.starts_with("file:") => {
            (n, ver)
        }
        _ => (key, v),
    };
    let src = entries
        .iter()
        .find(|e| e.name == name && e.version == ver)
        .map_or_else(|| version_source(ver), |e| e.source.clone());
    (ver.to_string(), src)
}
