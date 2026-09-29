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
//!   package private (`replace_all`), as does a private `RestoreSources`
//!   feed or any `fallbackPackageFolders` entry (neither is subject to
//!   source mapping);
//! - `[P125h §3]` that "every package private" is `LocalFeed` when EVERY
//!   non-public source of the chain is a local path (fallback folders
//!   count as one), else `CustomRegistry`; across chains `CustomRegistry`
//!   wins;
//! - a mapped chain: a pattern is public only if EVERY mapped chain maps it,
//!   and only to enabled keys whose hosts are all nuget.org; an id matching
//!   no pattern falls to a private `*` catch-all (restore would fail).

use std::collections::{BTreeMap, BTreeSet};

use crate::catalog::is_public_host;
use crate::model::{Ecosystem, PrivateReason};

use super::nuget_cfg::{FeedRef, NugetFile};
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
    /// `RestoreSources` / `RestoreAdditionalProjectSources` feeds.
    pub(crate) restore_feeds: Vec<FeedRef>,
}

/// Only an http(s) nuget.org host is public; hostless feeds never are.
fn public(f: &FeedRef) -> bool {
    matches!(f, FeedRef::Host(h) if is_public_host(Ecosystem::Nuget, h))
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
    /// Every package private: `CustomRegistry` or `LocalFeed`, and the
    /// first private display host.
    All(PrivateReason, Option<String>),
    /// pattern → (public_only, first private display host).
    Mapped(BTreeMap<String, (bool, Option<String>)>),
}

type Feeds<'a> = BTreeSet<&'a FeedRef>;

/// Over the NON-PUBLIC feeds among `feeds`: `None` if there is none; else
/// `LocalFeed` iff every one is a local path (`CustomRegistry` as soon as
/// one is a host or an unresolved `%VAR%`), and the first display host.
fn private_of<'a>(
    feeds: impl Iterator<Item = &'a FeedRef>,
) -> Option<(PrivateReason, Option<String>)> {
    let mut out: Option<(PrivateReason, Option<String>)> = None;
    for f in feeds.filter(|f| !public(f)) {
        let (reason, host) = out.get_or_insert((PrivateReason::LocalFeed, None));
        match f {
            FeedRef::Host(h) => {
                *reason = PrivateReason::CustomRegistry;
                if host.is_none() {
                    *host = Some(h.clone());
                }
            }
            FeedRef::Unresolved => *reason = PrivateReason::CustomRegistry,
            FeedRef::LocalPath => {}
        }
    }
    out
}

fn evaluate(chain: &Chain<'_>) -> Result<Outcome, Closed> {
    // key → feeds, from the surviving `packageSources`.
    let mut sources: BTreeMap<&str, Feeds<'_>> = BTreeMap::new();
    for l in surviving(&chain.levels, |f| f.sources_clear)? {
        for f in &l.files {
            for (k, feed) in &f.sources {
                sources.entry(k).or_default().insert(feed);
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
    let disabled = |k: &str| mentions.get(k).is_some_and(|m| m.iter().all(|on| *on));
    let enabled: BTreeMap<&str, &Feeds<'_>> = sources
        .iter()
        .filter(|(k, _)| !disabled(k))
        .map(|(k, v)| (*k, v))
        .collect();
    let enabled_feeds = || enabled.values().flat_map(|fs| fs.iter().copied());
    // Fallback folders and RestoreSources bypass source mapping: every
    // package private, judged over EVERY non-public source of the chain.
    let local = FeedRef::LocalPath;
    if fallback || chain.restore_feeds.iter().any(|f| !public(f)) {
        let all = chain
            .restore_feeds
            .iter()
            .chain(fallback.then_some(&local))
            .chain(enabled_feeds());
        // Unreachable default: the guard guarantees a non-public feed in
        // `all` (the fallback `LocalPath` or a non-public restore feed); if
        // that ever breaks, the stricter reason is the safe label.
        let (reason, host) = private_of(all).unwrap_or((PrivateReason::CustomRegistry, None));
        return Ok(Outcome::All(reason, host));
    }
    let Some((reason, shown)) = private_of(enabled_feeds()) else {
        return Ok(Outcome::Public);
    };
    if patterns.is_empty() {
        return Ok(Outcome::All(reason, shown));
    }
    let mapped = patterns
        .into_iter()
        .map(|(p, keys)| {
            // Keys compare case-insensitively here (a superset of hosts).
            let feeds: Vec<&FeedRef> = keys
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
            let private = private_of(feeds.iter().copied());
            let public_only = !unresolved && private.is_none() && !feeds.is_empty();
            let host = private.and_then(|(_, h)| h);
            (p, (public_only, host))
        })
        .collect();
    Ok(Outcome::Mapped(mapped))
}

/// Folds every chain into the NuGet registry signals.
pub(crate) fn resolve(chains: &[Chain<'_>]) -> Result<EcoRegistry, Closed> {
    let mut e = EcoRegistry::public();
    let mut mapped: Vec<BTreeMap<String, (bool, Option<String>)>> = Vec::new();
    // `[P125h §3]` LocalFeed only while EVERY all-private chain is local.
    let mut all_local = true;
    for c in chains {
        match evaluate(c)? {
            Outcome::Public => {}
            Outcome::All(reason, h) => {
                e.replace_all = true;
                e.default_public = false;
                all_local &= reason == PrivateReason::LocalFeed;
                if e.default_host.is_none() {
                    e.default_host = h;
                }
            }
            Outcome::Mapped(m) => {
                // `[P125h2 review]` a pattern mapped to a non-public source
                // is not provably local: under another chain's `replace_all`
                // it must not be labelled `LocalFeed`.
                all_local &= m.values().all(|(public_only, _)| *public_only);
                mapped.push(m);
            }
        }
    }
    if e.replace_all {
        e.all_reason = if all_local {
            PrivateReason::LocalFeed
        } else {
            PrivateReason::CustomRegistry
        };
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
