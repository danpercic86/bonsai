//! §5.1 steps 4, 5 and 11: classification, the two query sets, and row /
//! group assembly (§5.2 ordering). Every repo- or response-derived display
//! string goes through `clean_display` here `[LOW-3]`.

use std::collections::{BTreeMap, BTreeSet};

use crate::adapter::ParseError;
use crate::catalog::registry_page_template;
use crate::model::{
    clean_display, Advisory, Availability, DepRow, DepScope, Ecosystem, GroupState, ManifestGroup,
    ParseErrorKind, Relation, UpdateKind, MAX_DISPLAY,
};
use crate::privacy::{classify, normalize, LocalNames, PublicPackage, Visibility};
use crate::registry_ctx::RegistryContext;
use crate::remote::depsdev::RegistryVersion;
use crate::version::{classify_update, classify_update_declared, select_latest, VersionCandidate};

use super::groups::GroupInput;

pub(crate) struct Direct {
    name: String,
    /// `[P125d review, MEDIUM-D1]` the name is in the casing OSV matches.
    /// Always true except for a NuGet id with no lockfile entry of exactly
    /// this casing (OSV is case-sensitive for NuGet, §0.1).
    osv_name_exact: bool,
    scope: DepScope,
    declared: Option<String>,
    current: Option<String>,
    vis: Visibility,
}

pub(crate) struct Transitive {
    name: String,
    version: String,
    vis: Visibility,
}

pub(crate) enum Classified {
    Parsed {
        manifest_path: String,
        lockfile_path: Option<String>,
        eco: Ecosystem,
        has_lockfile: bool,
        direct: Vec<Direct>,
        transitive: Vec<Transitive>,
    },
    Failed {
        manifest_path: String,
        lockfile_path: Option<String>,
        eco: Option<Ecosystem>,
        err: ParseError,
    },
    NotTracked {
        path: String,
        label: &'static str,
    },
}

/// `(eco, wire name)` ⇒ deps.dev versions (`None` = Unknown).
pub(crate) type LatestMap = BTreeMap<(Ecosystem, String), Option<Vec<RegistryVersion>>>;
/// `(eco, wire name, version)` ⇒ advisories (sorted).
pub(crate) type AdvisoryMap = BTreeMap<(Ecosystem, String, String), Vec<Advisory>>;

fn key(eco: Ecosystem, name: &str) -> String {
    normalize(eco, name).to_lowercase()
}

/// Step 4. The version handed to `classify` is the RESOLVED one only; a
/// declared constraint is never sent.
pub(crate) fn classify_all(
    inputs: Vec<GroupInput>,
    reg: &RegistryContext,
    local: &LocalNames,
) -> Vec<Classified> {
    inputs
        .into_iter()
        .map(|g| match g {
            GroupInput::Parsed {
                manifest_path,
                lockfile_path,
                eco,
                group,
            } => {
                // [P125d review] a lock entry is the direct dep's own copy
                // only at the SAME version: `lodash@4.17.21` direct next to a
                // nested `lodash@4.17.4`, or Cargo `syn 2` next to `syn 1`,
                // keeps the second copy as a transitive entry.
                let direct_pairs: BTreeSet<(String, String)> = group
                    .direct
                    .iter()
                    .filter_map(|d| Some((key(eco, &d.name), d.resolved.clone()?)))
                    .collect();
                // NuGet: verified only when the lock writes this id in
                // exactly one casing, and it is this one.
                let mut casings: BTreeMap<String, BTreeSet<&str>> = BTreeMap::new();
                for e in &group.lock_entries {
                    casings
                        .entry(e.name.to_ascii_lowercase())
                        .or_default()
                        .insert(e.name.as_str());
                }
                let exact = |name: &str| {
                    eco != Ecosystem::Nuget
                        || casings
                            .get(&name.to_ascii_lowercase())
                            .is_some_and(|s| s.len() == 1 && s.contains(name))
                };
                let exacts: Vec<bool> = group.direct.iter().map(|d| exact(&d.name)).collect();
                let direct = group
                    .direct
                    .into_iter()
                    .zip(exacts)
                    .map(|(d, osv_name_exact)| Direct {
                        osv_name_exact,
                        vis: classify(eco, &d.name, d.resolved.as_deref(), &d.source, reg, local),
                        name: d.name,
                        scope: d.scope,
                        declared: d.declared,
                        current: d.resolved,
                    })
                    .collect();
                let mut seen = BTreeSet::new();
                let transitive = group
                    .lock_entries
                    .into_iter()
                    .filter(|e| !direct_pairs.contains(&(key(eco, &e.name), e.version.clone())))
                    .filter(|e| seen.insert((key(eco, &e.name), e.version.clone())))
                    .map(|e| Transitive {
                        vis: classify(eco, &e.name, Some(&e.version), &e.source, reg, local),
                        name: e.name,
                        version: e.version,
                    })
                    .collect();
                Classified::Parsed {
                    manifest_path,
                    lockfile_path,
                    eco,
                    has_lockfile: group.has_lockfile,
                    direct,
                    transitive,
                }
            }
            GroupInput::Failed {
                manifest_path,
                lockfile_path,
                eco,
                err,
            } => Classified::Failed {
                manifest_path,
                lockfile_path,
                eco,
                err,
            },
            GroupInput::NotTracked { path, label } => Classified::NotTracked { path, label },
        })
        .collect()
}

fn public(v: &Visibility) -> Option<&PublicPackage> {
    match v {
        Visibility::Public(p) => Some(p),
        _ => None,
    }
}

fn parsed(groups: &[Classified]) -> impl Iterator<Item = (&[Direct], &[Transitive])> {
    groups.iter().filter_map(|g| match g {
        Classified::Parsed {
            direct, transitive, ..
        } => Some((direct.as_slice(), transitive.as_slice())),
        _ => None,
    })
}

/// Step 5: Public DIRECT deps, deduped by `(eco, name)`.
pub(crate) fn latest_queries(groups: &[Classified]) -> Vec<PublicPackage> {
    let mut seen = BTreeSet::new();
    parsed(groups)
        .flat_map(|(d, _)| d.iter().filter_map(|d| public(&d.vis)))
        .filter(|p| seen.insert((p.ecosystem(), p.name().to_string())))
        .cloned()
        .collect()
}

/// Step 5: Public deps / lock entries WITH a version, deduped by
/// `(eco, name, version)`.
pub(crate) fn vuln_queries(groups: &[Classified]) -> Vec<PublicPackage> {
    let mut seen = BTreeSet::new();
    parsed(groups)
        .flat_map(|(d, t)| {
            d.iter()
                .map(|d| &d.vis)
                .chain(t.iter().map(|t| &t.vis))
                .filter_map(public)
        })
        .filter(|p| p.version().is_some())
        .filter(|p| {
            seen.insert((
                p.ecosystem(),
                p.name().to_string(),
                p.version().unwrap_or("").to_string(),
            ))
        })
        .cloned()
        .collect()
}

fn disp(s: &str) -> String {
    clean_display(s, MAX_DISPLAY)
}

fn registry_url(p: &PublicPackage) -> String {
    let enc: String = p
        .name()
        .bytes()
        .map(|b| {
            if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~' | b'/' | b'@') {
                (b as char).to_string()
            } else {
                format!("%{b:02X}")
            }
        })
        .collect();
    registry_page_template(p.ecosystem()).replace("{}", &enc)
}

fn vulns_for(p: &PublicPackage, adv: &AdvisoryMap) -> Vec<Advisory> {
    p.version()
        .and_then(|v| adv.get(&(p.ecosystem(), p.name().to_string(), v.to_string())))
        .cloned()
        .unwrap_or_default()
}

fn non_public(vis: &Visibility) -> Availability {
    match vis {
        Visibility::Local(reason) => Availability::Local { reason: *reason },
        Visibility::Private(reason, host) => Availability::Private {
            reason: *reason,
            registry_host: host.as_deref().map(disp),
        },
        // Unreachable for callers; Unknown is the safe reading.
        Visibility::Public(_) => Availability::Unknown,
    }
}

fn direct_row(eco: Ecosystem, d: Direct, latest: &LatestMap, adv: &AdvisoryMap) -> DepRow {
    let mut row = DepRow {
        name: disp(&d.name),
        ecosystem: eco,
        relation: Relation::Direct,
        scope: d.scope,
        declared: d.declared.as_deref().map(disp),
        current: d.current.as_deref().map(disp),
        latest: None,
        update: None,
        availability: non_public(&d.vis),
        vulns_checked: false,
        vulns: Vec::new(),
        registry_url: None,
    };
    let Visibility::Public(p) = &d.vis else {
        return row;
    };
    row.registry_url = Some(registry_url(p));
    row.vulns = vulns_for(p, adv);
    // [MEDIUM-D1] a NuGet id in an unverified casing with zero OSV hits is
    // "not verified", never "clean".
    row.vulns_checked = p.version().is_some() && (d.osv_name_exact || !row.vulns.is_empty());
    let versions = latest
        .get(&(eco, p.name().to_string()))
        .and_then(Option::as_ref);
    let Some(versions) = versions else {
        row.availability = Availability::Unknown;
        return row;
    };
    row.availability = Availability::Checked;
    let cands: Vec<VersionCandidate<'_>> = versions
        .iter()
        .map(|v| VersionCandidate {
            version: &v.version,
            is_default: v.is_default,
            is_deprecated: v.is_deprecated,
        })
        .collect();
    let best = select_latest(eco, &cands, d.current.as_deref());
    row.update = best.and_then(|l| match (&d.current, &d.declared) {
        (Some(c), _) => classify_update(eco, c, l),
        (None, Some(decl)) => classify_update_declared(eco, decl, l),
        (None, None) => None,
    });
    row.latest = best.map(disp);
    row
}

/// A transitive row exists only with vulns (AC4). Its availability is
/// `Checked`: OSV was asked; deps.dev is never asked for transitives.
fn transitive_row(eco: Ecosystem, t: &Transitive, adv: &AdvisoryMap) -> Option<DepRow> {
    let p = public(&t.vis)?;
    let vulns = vulns_for(p, adv);
    (!vulns.is_empty()).then(|| DepRow {
        name: disp(&t.name),
        ecosystem: eco,
        relation: Relation::Transitive,
        scope: DepScope::Normal,
        declared: None,
        current: Some(disp(&t.version)),
        latest: None,
        update: None,
        availability: Availability::Checked,
        vulns_checked: true,
        vulns,
        registry_url: Some(registry_url(p)),
    })
}

/// `[MEDIUM-1]` the fixed message table: category + position, no content.
pub(crate) fn parse_message(kind: ParseErrorKind, line: Option<u32>, col: Option<u32>) -> String {
    let what = match kind {
        ParseErrorKind::TooLarge => "file too large",
        ParseErrorKind::NotAFile => "not a regular file",
        ParseErrorKind::Unreadable => "file unreadable",
        ParseErrorKind::InvalidSyntax => "invalid syntax",
        ParseErrorKind::UnsupportedFeature => "unsupported feature",
        ParseErrorKind::UnsupportedVersion => "unsupported lockfile version",
        ParseErrorKind::LimitExceeded => "limit exceeded",
        ParseErrorKind::Internal => "internal parser error",
    };
    match (line, col) {
        (Some(l), Some(c)) => format!("{what} at {l}:{c}"),
        (Some(l), None) => format!("{what} at line {l}"),
        _ => what.to_string(),
    }
}

fn update_rank(u: Option<UpdateKind>) -> u8 {
    match u {
        Some(UpdateKind::Major) => 3,
        Some(UpdateKind::Minor) => 2,
        Some(UpdateKind::Patch) => 1,
        None => 0,
    }
}

/// §5.2 row order.
pub(crate) fn sort_rows(rows: &mut [DepRow]) {
    rows.sort_by(|a, b| {
        let sev = |r: &DepRow| r.max_severity().map_or(-1, |s| i16::from(s.rank()));
        sev(b)
            .cmp(&sev(a))
            .then(update_rank(b.update).cmp(&update_rank(a.update)))
            .then((a.relation == Relation::Transitive).cmp(&(b.relation == Relation::Transitive)))
            .then(a.name.cmp(&b.name))
    });
}

/// §5.2 group order: root-level first, then `manifest_path` ascending; the
/// `MAX_MANIFESTS` overflow group last.
pub(crate) fn sort_groups(groups: &mut [ManifestGroup]) {
    groups.sort_by(|a, b| {
        let k = |g: &ManifestGroup| {
            let overflow =
                g.ecosystem.is_none() && matches!(g.state, GroupState::ParseError { .. });
            (overflow, g.manifest_path.contains('/'))
        };
        k(a).cmp(&k(b)).then(a.manifest_path.cmp(&b.manifest_path))
    });
}

/// Step 11.
pub(crate) fn assemble(
    groups: Vec<Classified>,
    latest: &LatestMap,
    adv: &AdvisoryMap,
) -> Vec<ManifestGroup> {
    let mut out: Vec<ManifestGroup> = groups
        .into_iter()
        .map(|g| match g {
            Classified::Parsed {
                manifest_path,
                lockfile_path,
                eco,
                has_lockfile,
                direct,
                transitive,
            } => {
                let checked = transitive
                    .iter()
                    .filter(|t| public(&t.vis).is_some_and(|p| p.version().is_some()))
                    .count();
                let mut rows: Vec<DepRow> = transitive
                    .iter()
                    .filter_map(|t| transitive_row(eco, t, adv))
                    .collect();
                rows.extend(direct.into_iter().map(|d| direct_row(eco, d, latest, adv)));
                sort_rows(&mut rows);
                ManifestGroup {
                    manifest_path: disp(&manifest_path),
                    lockfile_path: lockfile_path.as_deref().map(disp),
                    ecosystem: Some(eco),
                    state: if has_lockfile {
                        GroupState::Ok
                    } else {
                        GroupState::NoLockfile
                    },
                    rows,
                    transitive_checked: u32::try_from(checked).unwrap_or(u32::MAX),
                }
            }
            Classified::Failed {
                manifest_path,
                lockfile_path,
                eco,
                err,
            } => ManifestGroup {
                manifest_path: disp(&manifest_path),
                lockfile_path: lockfile_path.as_deref().map(disp),
                ecosystem: eco,
                state: GroupState::ParseError {
                    file: disp(&err.file),
                    category: err.kind,
                    line: err.line,
                    col: err.col,
                    message: parse_message(err.kind, err.line, err.col),
                },
                rows: Vec::new(),
                transitive_checked: 0,
            },
            Classified::NotTracked { path, label } => ManifestGroup {
                manifest_path: disp(&path),
                lockfile_path: None,
                ecosystem: None,
                state: GroupState::NotTracked {
                    label: label.to_string(),
                },
                rows: Vec::new(),
                transitive_checked: 0,
            },
        })
        .collect();
    sort_groups(&mut out);
    out
}
