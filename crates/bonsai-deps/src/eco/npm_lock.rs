//! `package-lock.json` / `npm-shrinkwrap.json`: v2/v3 `packages{}` and the
//! v1 `dependencies{}` tree (walked with an explicit stack, no recursion).

use serde_json::{Map, Value};

use crate::adapter::{DepSource, LockEntry, ParseError, RegistryRef};
use crate::model::ParseErrorKind;

use super::npm::{classify_spec, source_from_resolved, split_name_version, DirectSpec, LockData};

const NM: &str = "node_modules/";

pub(crate) fn parse(
    path: &str,
    text: &str,
    importer: &str,
    direct: &[DirectSpec],
) -> Result<LockData, ParseError> {
    let v: Value = serde_json::from_str(text).map_err(|e| ParseError::from_json(path, &e))?;
    let unsupported = || ParseError::new(path, ParseErrorKind::UnsupportedVersion);
    match v.get("lockfileVersion").and_then(Value::as_u64) {
        Some(2 | 3) => {
            let packages = v
                .get("packages")
                .and_then(Value::as_object)
                .ok_or_else(|| ParseError::new(path, ParseErrorKind::InvalidSyntax))?;
            Ok(parse_packages(packages, importer, direct))
        }
        Some(1) => Ok(parse_v1(
            v.get("dependencies").and_then(Value::as_object),
            direct,
        )),
        _ => Err(unsupported()),
    }
}

/// Source from an entry's `resolved` URL; none ⇒ the default registry.
fn resolved_source(entry: &Value) -> DepSource {
    str_field(entry, "resolved").map_or(
        DepSource::Registry(RegistryRef::Default),
        source_from_resolved,
    )
}

fn str_field<'a>(e: &'a Value, k: &str) -> Option<&'a str> {
    e.get(k).and_then(Value::as_str)
}

/// v2/v3: `""` is the root project; `…node_modules/<name>` is an installed
/// package; any other key is a workspace member folder.
fn parse_packages(
    packages: &Map<String, Value>,
    importer: &str,
    direct: &[DirectSpec],
) -> LockData {
    let mut data = LockData::default();
    for (key, entry) in packages {
        if key.is_empty() {
            continue;
        }
        let installed = key.rfind(NM).map(|i| &key[i + NM.len()..]);
        let name = str_field(entry, "name").or(installed);
        let Some(name) = name else { continue };
        let source =
            if installed.is_none() || entry.get("link").and_then(Value::as_bool) == Some(true) {
                DepSource::Workspace
            } else {
                resolved_source(entry)
            };
        let Some(version) = str_field(entry, "version") else {
            // A link entry has no version; it is still a workspace edge.
            if source == DepSource::Workspace {
                data.entries.push(LockEntry {
                    name: name.to_string(),
                    version: String::new(),
                    source,
                });
            }
            continue;
        };
        data.entries.push(LockEntry {
            name: name.to_string(),
            version: version.to_string(),
            source,
        });
    }
    for d in direct {
        let nested = (!importer.is_empty()).then(|| format!("{importer}/{NM}{}", d.key));
        let hoisted = format!("{NM}{}", d.key);
        let hit = nested
            .as_deref()
            .and_then(|k| packages.get(k))
            .or_else(|| packages.get(&hoisted));
        let Some(entry) = hit else { continue };
        let linked = entry.get("link").and_then(Value::as_bool) == Some(true);
        let source = if linked {
            DepSource::Workspace
        } else {
            resolved_source(entry)
        };
        let version = str_field(entry, "version").unwrap_or_default().to_string();
        data.resolved.insert(d.key.clone(), (version, source));
    }
    data
}

/// A v1 entry's (name, version, source). `version` may itself be a spec
/// (`file:…`, `github:…`, `npm:real@1.0.0`).
fn v1_entry(key: &str, entry: &Value) -> Option<(String, String, DepSource)> {
    let raw = str_field(entry, "version")?;
    let spec = classify_spec(key, raw);
    if let Some(rest) = raw.strip_prefix("npm:") {
        let (name, ver) = split_name_version(rest);
        let src = resolved_source(entry);
        return Some((name.to_string(), ver.unwrap_or_default().to_string(), src));
    }
    let src = match spec.source {
        DepSource::Registry(_) => resolved_source(entry),
        other => other,
    };
    Some((key.to_string(), raw.to_string(), src))
}

fn parse_v1(root: Option<&Map<String, Value>>, direct: &[DirectSpec]) -> LockData {
    let mut data = LockData::default();
    let Some(root) = root else { return data };
    let mut stack: Vec<&Map<String, Value>> = vec![root];
    while let Some(map) = stack.pop() {
        for (key, entry) in map {
            if let Some((name, version, source)) = v1_entry(key, entry) {
                data.entries.push(LockEntry {
                    name,
                    version,
                    source,
                });
            }
            if let Some(nested) = entry.get("dependencies").and_then(Value::as_object) {
                stack.push(nested);
            }
        }
    }
    for d in direct {
        if let Some((_, version, source)) = root.get(&d.key).and_then(|e| v1_entry(&d.key, e)) {
            data.resolved.insert(d.key.clone(), (version, source));
        }
    }
    data
}
