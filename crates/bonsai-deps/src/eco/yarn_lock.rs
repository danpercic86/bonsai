//! `yarn.lock`: the v1 custom format (an iterative line scanner, no
//! recursion `[HIGH-2]`) and berry (YAML with `__metadata`, through the
//! bounded event reader).
//!
//! Berry locks, like pnpm's, name `npm:` resolutions without a host, so for
//! those the registry config is the only signal `[HIGH-1]`.

use std::collections::BTreeMap;

use crate::adapter::{DepSource, LockEntry, ParseError, RegistryRef};
use crate::model::ParseErrorKind;
use crate::yaml_events::{parse_first, YNode};

use super::npm::{classify_spec, source_from_resolved, split_name_version, DirectSpec, LockData};

pub(crate) fn parse(path: &str, text: &str, direct: &[DirectSpec]) -> Result<LockData, ParseError> {
    if text.lines().any(|l| l.starts_with("__metadata:")) {
        parse_berry(path, text, direct)
    } else {
        parse_v1(path, text, direct)
    }
}

fn unquote(s: &str) -> &str {
    let s = s.trim();
    s.strip_prefix('"')
        .and_then(|x| x.strip_suffix('"'))
        .unwrap_or(s)
}

/// One lock record: its descriptors plus the resolved version/source.
struct Record {
    descriptors: Vec<String>,
    version: Option<String>,
    resolved: Option<String>,
}

fn u32_line(i: usize) -> Option<u32> {
    u32::try_from(i + 1).ok()
}

/// The v1 format: an unindented `"a@^1", "a@^1.2":` header, then two-space
/// `key value` fields; deeper lines (`dependencies:` bodies) are skipped.
fn parse_v1(path: &str, text: &str, direct: &[DirectSpec]) -> Result<LockData, ParseError> {
    let mut records: Vec<Record> = Vec::new();
    for (i, line) in text.lines().enumerate() {
        let trimmed = line.trim_end();
        if trimmed.trim_start().is_empty() || trimmed.trim_start().starts_with('#') {
            continue;
        }
        let indent = trimmed.len() - trimmed.trim_start().len();
        if indent == 0 {
            let Some(header) = trimmed.strip_suffix(':') else {
                return Err(ParseError::at(
                    path,
                    ParseErrorKind::InvalidSyntax,
                    u32_line(i),
                    Some(1),
                ));
            };
            records.push(Record {
                descriptors: header.split(',').map(|d| unquote(d).to_string()).collect(),
                version: None,
                resolved: None,
            });
            continue;
        }
        let Some(rec) = records.last_mut() else {
            return Err(ParseError::at(
                path,
                ParseErrorKind::InvalidSyntax,
                u32_line(i),
                Some(1),
            ));
        };
        if indent != 2 {
            continue;
        }
        let field = trimmed.trim_start();
        let (key, value) = field.split_once(' ').unwrap_or((field, ""));
        match unquote(key) {
            "version" => rec.version = Some(unquote(value).to_string()),
            "resolved" => rec.resolved = Some(unquote(value).to_string()),
            _ => {}
        }
    }
    let mut data = LockData::default();
    let mut by_descriptor: BTreeMap<&str, usize> = BTreeMap::new();
    for rec in &records {
        let Some(first) = rec.descriptors.first() else {
            continue;
        };
        let Some(version) = rec.version.clone() else {
            continue;
        };
        let (key, range) = split_name_version(first);
        let spec = classify_spec(key, range.unwrap_or(""));
        let source = match (&spec.source, &rec.resolved) {
            (DepSource::Registry(_), Some(r)) => source_from_resolved(r),
            (s, _) => s.clone(),
        };
        for d in &rec.descriptors {
            by_descriptor.insert(d.as_str(), data.entries.len());
        }
        data.entries.push(LockEntry {
            name: spec.name.clone(),
            version,
            source,
        });
    }
    for d in direct {
        let want = format!("{}@{}", d.key, d.declared);
        if let Some(e) = by_descriptor.get(want.as_str()).map(|&i| &data.entries[i]) {
            let hit = (e.version.clone(), e.source.clone());
            data.resolved.insert(d.key.clone(), hit);
        }
    }
    Ok(data)
}

fn keep_berry(p: &[String]) -> bool {
    match p.len() {
        0 | 1 => true,
        2 => matches!(p[1].as_str(), "version" | "resolution"),
        _ => false,
    }
}

/// Source of a berry `resolution` (`name@<protocol>:<rest>`).
fn berry_source(resolution: &str) -> DepSource {
    let (_, rest) = split_name_version(resolution);
    let rest = rest.unwrap_or("");
    let proto = rest.split_once(':').map_or("", |(p, _)| p);
    match proto {
        "npm" => DepSource::Registry(RegistryRef::Default),
        "workspace" => DepSource::Workspace,
        "file" | "link" | "portal" | "exec" | "patch" => DepSource::Path,
        "git" | "git+ssh" | "git+https" | "github" | "ssh" => DepSource::Git,
        // `http(s):` tarballs and anything unknown are never sent (Local).
        _ => DepSource::Url,
    }
}

fn parse_berry(path: &str, text: &str, direct: &[DirectSpec]) -> Result<LockData, ParseError> {
    let doc = parse_first(text, &keep_berry).map_err(|e| ParseError::from_yaml(path, e))?;
    let mut data = LockData::default();
    let mut by_descriptor: BTreeMap<&str, usize> = BTreeMap::new();
    for (key, entry) in doc.entries() {
        if key == "__metadata" {
            continue;
        }
        let (Some(version), Some(resolution)) = (
            entry.get("version").and_then(YNode::as_str),
            entry.get("resolution").and_then(YNode::as_str),
        ) else {
            continue;
        };
        let (name, _) = split_name_version(resolution);
        let source = berry_source(resolution);
        for d in key.split(',') {
            by_descriptor.insert(d.trim(), data.entries.len());
        }
        data.entries.push(LockEntry {
            name: name.to_string(),
            version: version.to_string(),
            source,
        });
    }
    for d in direct {
        let plain = format!("{}@{}", d.key, d.declared);
        let npm = format!("{}@npm:{}", d.key, d.declared);
        let hit = by_descriptor
            .get(plain.as_str())
            .or_else(|| by_descriptor.get(npm.as_str()))
            .map(|&i| &data.entries[i]);
        if let Some(e) = hit {
            let hit = (e.version.clone(), e.source.clone());
            data.resolved.insert(d.key.clone(), hit);
        }
    }
    Ok(data)
}
