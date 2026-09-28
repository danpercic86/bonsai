//! NuGet source resolution over config CHAINS (§4.4 "NuGet precedence").
//!
//! One chain per project directory: its in-repo dirs (nearest first), then
//! every ancestor above the workdir (`[Q7]`, nearest first), then the user
//! config, then the machine-wide set. Per section, levels are taken nearest
//! first until a level whose `<clear/>` drops every farther one. A level
//! with several files (the name variants on a case-sensitive file system)
//! whose files DISAGREE on `<clear/>` is ambiguous — we cannot tell which
//! one NuGet read — and fails closed `[P125c2 review]`. Within the
//! surviving levels EVERY definition is kept (never first-seen): a key
//! redefined at a nearer level keeps both hosts.
//!
//! Judgement, all toward private:
//! - a key counts as disabled only if every surviving mention disables it;
//! - a source with no URL host (local folder, UNC, `%VAR%`) is a NON-PUBLIC
//!   source with no display host `[P125c2 review]`;
//! - a chain with an enabled non-nuget.org source and no mapping ⇒ every
//!   package CustomRegistry (`replace_all`), as does a private
//!   `RestoreSources` host or any `fallbackPackageFolders` entry (neither
//!   is subject to source mapping);
//! - a mapped chain: a pattern is public only if EVERY mapped chain maps it,
//!   and only to enabled keys whose hosts are all nuget.org; an id matching
//!   no pattern falls to a private `*` catch-all (restore would fail).

use std::collections::{BTreeMap, BTreeSet};

use crate::catalog::is_public_host;
use crate::model::Ecosystem;

use super::nuget_cfg::NugetFile;
use super::{Closed, EcoRegistry, NugetMapping};

/// One config level: the files read in one directory (or the user file, or
/// the machine-wide set).
#[derive(Default)]
pub(crate) struct Level {
    pub(crate) files: Vec<NugetFile>,
}

/// One project directory's view.
pub(crate) struct Chain<'a> {
    /// Nearest first.
    pub(crate) levels: Vec<&'a Level>,
    /// `RestoreSources` / `RestoreAdditionalProjectSources` hosts.
    pub(crate) restore_hosts: Vec<String>,
}

/// A source host: `None` = hostless (never public).
fn public(h: Option<&str>) -> bool {
    h.is_some_and(|h| is_public_host(Ecosystem::Nuget, h))
}

/// The levels a section survives in: nearest first, up to and including
/// the first level that clears it. Files of one level disagreeing on the
/// clear ⇒ `Closed`.
fn surviving<'a>(
    levels: &[&'a Level],
    clears: fn(&NugetFile) -> bool,
) -> Result<Vec<&'a Level>, Closed> {
    let mut out = Vec::new();
    for l in levels {
        out.push(*l);
        let n = l.files.iter().filter(|f| clears(f)).count();
        if n > 0 && n < l.files.len() {
            return Err(Closed);
        }
        if n > 0 {
            break;
        }
    }
    Ok(out)
}

/// The outcome for one chain.
enum Outcome {
    Public,
    /// Every package private; the first private display host.
    All(Option<String>),
    /// pattern → (public_only, first private display host).
    Mapped(BTreeMap<String, (bool, Option<String>)>),
}

type Hosts<'a> = BTreeSet<Option<&'a str>>;

/// The first non-public display host among `hosts` (hostless ones have
/// none), and whether any is non-public.
fn private_of<'a>(hosts: impl Iterator<Item = Option<&'a str>>) -> (bool, Option<String>) {
    let mut private = false;
    let mut shown = None;
    for h in hosts.filter(|h| !public(*h)) {
        private = true;
        if shown.is_none() {
            shown = h.map(str::to_string);
        }
    }
    (private, shown)
}

fn evaluate(chain: &Chain<'_>) -> Result<Outcome, Closed> {
    // key → hosts, from the surviving `packageSources`.
    let mut sources: BTreeMap<&str, Hosts<'_>> = BTreeMap::new();
    for l in surviving(&chain.levels, |f| f.sources_clear)? {
        for f in &l.files {
            for (k, h) in &f.sources {
                sources.entry(k).or_default().insert(h.as_deref());
            }
        }
    }
    // key → every surviving mention (true = disabled).
    let mut mentions: BTreeMap<&str, Vec<bool>> = BTreeMap::new();
    for l in surviving(&chain.levels, |f| f.disabled_clear)? {
        for f in &l.files {
            for (k, on) in &f.disabled {
                mentions.entry(k).or_default().push(*on);
            }
        }
    }
    let fallback = surviving(&chain.levels, |f| f.fallback_clear)?
        .iter()
        .flat_map(|l| &l.files)
        .any(|f| f.fallback > 0);
    let mut patterns: BTreeMap<String, Vec<&str>> = BTreeMap::new();
    for l in surviving(&chain.levels, |f| f.mapping_clear)? {
        for f in &l.files {
            for (k, p) in &f.mapping {
                patterns.entry(p.clone()).or_default().push(k);
            }
        }
    }
    if fallback {
        return Ok(Outcome::All(None));
    }
    if let Some(h) = chain.restore_hosts.iter().find(|h| !public(Some(h))) {
        return Ok(Outcome::All(Some(h.clone())));
    }
    let disabled = |k: &str| mentions.get(k).is_some_and(|m| m.iter().all(|on| *on));
    let enabled: BTreeMap<&str, &Hosts<'_>> = sources
        .iter()
        .filter(|(k, _)| !disabled(k))
        .map(|(k, v)| (*k, v))
        .collect();
    let (any_private, shown) = private_of(enabled.values().flat_map(|hs| hs.iter().copied()));
    if !any_private {
        return Ok(Outcome::Public);
    }
    if patterns.is_empty() {
        return Ok(Outcome::All(shown));
    }
    let mapped = patterns
        .into_iter()
        .map(|(p, keys)| {
            // Keys compare case-insensitively here (a superset of hosts).
            let hosts: Vec<Option<&str>> = keys
                .iter()
                .flat_map(|k| {
                    enabled
                        .iter()
                        .filter(|(ek, _)| ek.eq_ignore_ascii_case(k))
                        .flat_map(|(_, hs)| hs.iter().copied())
                })
                .collect();
            let unresolved = keys
                .iter()
                .any(|k| !enabled.keys().any(|ek| ek.eq_ignore_ascii_case(k)));
            let (private, host) = private_of(hosts.iter().copied());
            let public_only = !unresolved && !private && !hosts.is_empty();
            (p, (public_only, host))
        })
        .collect();
    Ok(Outcome::Mapped(mapped))
}

/// Folds every chain into the NuGet registry signals.
pub(crate) fn resolve(chains: &[Chain<'_>]) -> Result<EcoRegistry, Closed> {
    let mut e = EcoRegistry::public();
    let mut mapped: Vec<BTreeMap<String, (bool, Option<String>)>> = Vec::new();
    for c in chains {
        match evaluate(c)? {
            Outcome::Public => {}
            Outcome::All(h) => {
                e.replace_all = true;
                e.default_public = false;
                if e.default_host.is_none() {
                    e.default_host = h;
                }
            }
            Outcome::Mapped(m) => mapped.push(m),
        }
    }
    // `replace_all` must win: `private_reason` consults mappings first.
    if e.replace_all || mapped.is_empty() {
        return Ok(e);
    }
    let all: BTreeSet<&String> = mapped.iter().flat_map(|m| m.keys()).collect();
    let mut out: Vec<NugetMapping> = all
        .into_iter()
        .map(|p| {
            let per_chain: Vec<Option<&(bool, Option<String>)>> =
                mapped.iter().map(|m| m.get(p)).collect();
            let public_only = per_chain.iter().all(|x| x.is_some_and(|(ok, _)| *ok));
            let host = per_chain.iter().flatten().find_map(|(_, h)| h.clone());
            NugetMapping {
                pattern: p.clone(),
                public_only,
                host,
            }
        })
        .collect();
    if !out.iter().any(|m| m.pattern == "*") {
        out.push(NugetMapping {
            pattern: "*".to_string(),
            public_only: false,
            host: None,
        });
    }
    e.nuget_mapping = out;
    Ok(e)
}
