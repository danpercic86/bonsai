//! P115 — pruning stale repo paths out of `settings.json`.
//!
//! Evidence (a real `%APPDATA%\com.bonsai.app\settings.json`): `D:\Repos` was
//! moved to `D:\Data\Repos`, leaving five dead `recentRepos`, a dead
//! `repoForgeOverrides` pin whose live repo silently lost its forge-account
//! binding, and `hooksAckRepos` carrying BOTH the old and the new path for two
//! repos. This module is the once-per-launch housekeeping pass that fixes that.
//!
//! Three rules shape everything here (contract §1):
//!
//! * `recent_repos` — a convenience list of clickable things; a dead entry is
//!   pure noise and evicts live ones (it is the only capped list). **Pruned.**
//! * `hooks_ack_repos` — gates a one-time SECURITY disclosure. A wrong
//!   *migration* would silently hand a repo an acknowledgement the user never
//!   gave; a wrong *prune* costs one extra prompt. **Pruned, NEVER migrated.**
//! * `repo_forge_overrides` — the only record of which account a user pinned to
//!   a repo, and inert once its repo is gone (`resolve_account` matches by
//!   path). **NEVER deleted**; migrated to the repo's new location only under
//!   §4.2's six-clause rule, otherwise left byte-identical in place.
//!
//! `open_repos` / `active_repo` are read as migration candidates and never
//! mutated (§1.4): the frontend's session persist already self-heals them.
//!
//! Three invariants a reviewer should check by eye:
//!
//! 1. **No probe I/O and no git2 I/O ever runs under `SETTINGS_IO`.** Phase 1a
//!    ([`classify_all`]) and phase 1b ([`resolve_candidate_hosts`]) run against
//!    a lock-free `load_from` snapshot; [`prune_stale_paths`] is pure and is the
//!    only part that runs inside `update_if`. A dead UNC path can block
//!    `fs::metadata` for tens of seconds — inside the mutator that would queue
//!    every settings writer at launch behind it.
//! 2. **Only [`PathState::ConfirmedGone`] authorizes a mutation.** Everything
//!    else, including [`PathState::Unreachable`], is kept verbatim.
//! 3. **A migration needs positive forge identity** (rule 6). Without it a pin
//!    could land on a repo whose remote is on another host, where it resolves
//!    to nothing, renders no `accountSource: 'override'`, and therefore cannot
//!    be cleared from the UI at all — a permanent, invisible wrong binding,
//!    i.e. exactly the loss P115 exists to prevent.

use std::collections::HashSet;
use std::path::Path;

use super::Settings;

#[path = "prune_gate.rs"]
mod gate;

use gate::{basename, norm};
pub use gate::{classify, classify_all, collect_paths, PathFacts, PathState, Probe};

/// Forge-identity seam (§5.1). `None` covers every declining outcome the
/// contract lists — no `origin` (`AppError::NoRemote`), any other error, or an
/// unparseable origin — because rule (6) treats them identically; production
/// passes `|p| resolve_forge_identity(p).ok().map(|(host, _, _)| host)`.
pub type HostResolver<'a> = &'a mut dyn FnMut(&Path) -> Option<String>;

/// Phase 1b — all forge-identity I/O, no lock held. Fills `facts.hosts`.
///
/// Two narrowings, both deliberate: the whole phase returns immediately (and
/// **opens no repository at all**) unless some override is `ConfirmedGone`, and
/// within it only candidates whose basename matches a dead override's are
/// resolved, since rule (3) can never select any other. The typical dead-pin
/// launch therefore costs exactly one git2 open, and the common launch zero.
///
/// The resolved host is stored verbatim, never re-cased: both sides are
/// lowercased at write time (`settings/forge_accounts.rs`), and rule (6)
/// compares with `==`, so a future writer that stops lowercasing fails closed
/// (a declined migration) instead of matching something it should not.
pub fn resolve_candidate_hosts(s: &Settings, facts: &mut PathFacts, resolve: HostResolver<'_>) {
    let dead_basenames: HashSet<String> = s
        .repo_forge_overrides
        .iter()
        .filter(|o| facts.state(&o.repo_path) == PathState::ConfirmedGone)
        .map(|o| basename(&o.repo_path))
        .filter(|b| !b.is_empty())
        .collect();
    if dead_basenames.is_empty() {
        return; // fast path: opens NO repository
    }

    // `live_candidate_paths` has already deduplicated by this very key.
    for path in live_candidate_paths(s, facts) {
        let Some(key) = facts.key(&path).cloned() else {
            continue; // no canonical key ⇒ never a migration target anyway
        };
        if !dead_basenames.contains(&basename(&path)) {
            continue; // rule (3) could never pick it
        }
        if let Some(host) = resolve(Path::new(&path)) {
            if !host.is_empty() {
                facts.hosts.insert(key, host);
            }
        }
    }
}

/// `recent_repos` ++ `open_repos`, restricted to `Live`, in list order and
/// **deduplicated by canonical key** (§4.1) — falling back to the `norm` key
/// only for a path that would not canonicalize, which can then create ambiguity
/// (the safe direction) but never win a migration.
fn live_candidate_paths(s: &Settings, facts: &PathFacts) -> Vec<String> {
    let mut seen: HashSet<String> = HashSet::new();
    let mut out: Vec<String> = Vec::new();
    for path in s
        .recent_repos
        .iter()
        .map(|r| r.path.as_str())
        .chain(s.open_repos.iter().map(String::as_str))
    {
        if facts.state(path) != PathState::Live {
            continue;
        }
        let key = facts.key(path).cloned().unwrap_or_else(|| norm(path));
        if seen.insert(key) {
            out.push(path.to_string());
        }
    }
    out
}

/// What one pass changed. Counts only, no paths — the caller decides what to log.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PruneReport {
    pub recents_pruned: usize,
    pub hooks_acks_pruned: usize,
    pub overrides_migrated: usize,
    /// Dead overrides deliberately left in place (ambiguous, no candidate, or
    /// rule (6) declined).
    pub overrides_kept_dead: usize,
    /// Distinct paths classified [`PathState::Unreachable`] — the "we did
    /// nothing on purpose" counter, so a log line can tell "nothing stale" from
    /// "the drive was offline".
    pub unreachable_skipped: usize,
}

impl PruneReport {
    /// `true` iff the pass mutated [`Settings`] — the `update_if` return value.
    /// `overrides_kept_dead` and `unreachable_skipped` are observations, not
    /// mutations, and deliberately do NOT make this true: a launch that only
    /// observes must never rewrite `settings.json`.
    pub fn changed(&self) -> bool {
        self.recents_pruned > 0 || self.hooks_acks_pruned > 0 || self.overrides_migrated > 0
    }
}

/// Phase 2 — pure. Applies §1 and §4 using `facts`; performs **no I/O of any
/// kind**, which is what lets it run inside `settings::update_if` without
/// holding `SETTINGS_IO` across a blocking `fs::metadata` or a git2 open. Rule
/// (6)'s host check is a `HashMap` read of a value phase 1b computed.
///
/// A path absent from `facts.states` is treated as [`PathState::Live`]: never
/// mutate a path that was not probed.
///
/// Mutates `recent_repos`, `hooks_ack_repos`, and
/// `repo_forge_overrides[*].repo_path` only. NEVER touches `open_repos` or
/// `active_repo`, and never removes an override.
pub fn prune_stale_paths(s: &mut Settings, facts: &PathFacts) -> PruneReport {
    let mut report = PruneReport::default();
    let live_candidates = live_candidate_paths(s, facts);

    // --- overrides: migrated or kept, NEVER deleted ------------------------
    // Decided against a snapshot of the list, then applied, so a migration
    // earlier in the list cannot change how a later one evaluates rules (4)/(5).
    let overrides: Vec<(String, String, PathState)> = s
        .repo_forge_overrides
        .iter()
        .map(|o| {
            (
                o.repo_path.clone(),
                o.account_id.clone(),
                facts.state(&o.repo_path),
            )
        })
        .collect();
    let accounts: Vec<(String, String)> = s
        .forge_accounts
        .iter()
        .map(|a| (a.account_id.clone(), a.host.clone()))
        .collect();

    let mut migrations: Vec<(usize, String)> = Vec::new();
    for (i, (path, account_id, st)) in overrides.iter().enumerate() {
        if *st != PathState::ConfirmedGone {
            continue; // rule (1): only a proven-gone override may move
        }
        let target = migration_target(i, path, &overrides, &live_candidates, facts)
            .filter(|target| rule6_ok(target, account_id, &accounts, facts));
        match target {
            Some(target) => migrations.push((i, target)),
            None => report.overrides_kept_dead += 1,
        }
    }
    for (i, target) in migrations {
        if let Some(o) = s.repo_forge_overrides.get_mut(i) {
            o.repo_path = target;
            report.overrides_migrated += 1;
        }
    }

    // --- recents: the one list where deletion is the right answer ----------
    let before = s.recent_repos.len();
    s.recent_repos
        .retain(|r| facts.state(&r.path) != PathState::ConfirmedGone);
    report.recents_pruned = before - s.recent_repos.len();

    // --- hook acks: pruned, never migrated (a wrong migration would be
    //     consent the user never gave, and it would be invisible) -----------
    let before = s.hooks_ack_repos.len();
    s.hooks_ack_repos
        .retain(|p| facts.state(p) != PathState::ConfirmedGone);
    report.hooks_acks_pruned = before - s.hooks_ack_repos.len();

    report.unreachable_skipped = facts
        .states
        .values()
        .filter(|st| **st == PathState::Unreachable)
        .count();
    report
}

/// Rules (3)-(5) of §4.2 for the dead override at `index`. `Some(L)` iff:
///
/// 3. **exactly one** live candidate shares the dead path's basename (zero ⇒
///    nothing to move to; two or more ⇒ ambiguous);
/// 4. no OTHER override resolves to the same repo as `L` — by canonical key
///    when that other override is Live, by `norm` otherwise (a dead `O2` can
///    then only ever *block*, the conservative direction). A live pin is never
///    overwritten;
/// 5. no other **dead** override shares that basename (two dead `backend`
///    entries ⇒ both ambiguous, neither migrates).
///
/// Rules (1) and (2) are the caller's; rule (6) is [`rule6_ok`].
fn migration_target(
    index: usize,
    dead_path: &str,
    overrides: &[(String, String, PathState)],
    live_candidates: &[String],
    facts: &PathFacts,
) -> Option<String> {
    let base = basename(dead_path);
    if base.is_empty() {
        return None;
    }

    // (5) another dead override with the same basename makes both ambiguous.
    let dead_twin = overrides.iter().enumerate().any(|(j, (p, _, st))| {
        j != index && *st == PathState::ConfirmedGone && basename(p) == base
    });
    if dead_twin {
        return None;
    }

    // (3) exactly one live candidate with that basename.
    let mut matches = live_candidates.iter().filter(|l| basename(l) == base);
    let target = matches.next()?;
    if matches.next().is_some() {
        return None;
    }

    // (4) that candidate must not already be pinned by another override.
    let target_key = facts.key(target);
    let target_norm = norm(target);
    let taken = overrides.iter().enumerate().any(|(j, (p, _, st))| {
        if j == index {
            return false;
        }
        if *st == PathState::Live {
            if let (Some(a), Some(b)) = (facts.key(p), target_key) {
                return a == b;
            }
        }
        // Case-insensitive on purpose, and NOT a contradiction of F4 (which
        // made `norm`'s case fold Windows-only). Folding case is dangerous as a
        // *state key* — it applies one verdict to two directories — and safe as
        // a *blocking predicate*: over-blocking only declines a migration and
        // keeps the override, the conservative direction. It is needed because
        // default APFS is case-INsensitive but case-preserving, so `/x/Repos/y`
        // and `/x/repos/y` are one repo whose two spellings `realpath` returns
        // as given: the canonical branch above misses, and without this the
        // dead pin would land on a repo that already has one.
        norm(p).eq_ignore_ascii_case(&target_norm)
    });
    if taken {
        return None;
    }

    Some(target.clone())
}

/// Rule (6) — **positive forge identity**. The candidate's `origin` host, as
/// resolved in phase 1b, must be present, non-empty, and equal to the host of
/// the account the pin names.
///
/// Every other outcome declines (the override is kept, never deleted): no
/// `forge_accounts` record for that `account_id` (a pin to a deleted account
/// has nothing to preserve — `remove_forge_account` already drops overrides);
/// no canonical key for the candidate; no resolved host (error, no `origin`, or
/// an unparseable one). An **empty host is never a wildcard** on either side —
/// `bonsai_forge` degrades an unparseable origin to `host: String::new()`, and
/// matching that against an equally empty account host would re-admit exactly
/// the cross-host binding this rule exists to prevent.
fn rule6_ok(
    target: &str,
    account_id: &str,
    accounts: &[(String, String)],
    facts: &PathFacts,
) -> bool {
    let Some((_, account_host)) = accounts.iter().find(|(id, _)| id == account_id) else {
        return false;
    };
    let Some(key) = facts.key(target) else {
        return false;
    };
    let Some(host) = facts.hosts.get(key) else {
        return false;
    };
    !host.is_empty() && !account_host.is_empty() && host == account_host
}

/// One line, only when something changed. Counts only — **NEVER a path**: a
/// settings path carries the OS account name, and obs redaction is not in play
/// here. `eprintln!` is this crate's facade for non-fatal diagnostics (see
/// `note_if_dropped` in `settings/external_tools.rs`); an obs record would mean
/// a new closed-enum `LogPayload` variant plus a redaction review, which is
/// disproportionate for once-per-launch housekeeping (§7).
pub fn note_prune(report: &PruneReport) {
    if !report.changed() {
        return;
    }
    let mut line = format!(
        "bonsai: settings housekeeping — pruned {} recent repo(s) and {} hook \
         acknowledgement(s) whose folder is gone, moved {} forge-account pin(s) to the \
         repo's new location, kept {} pin(s) whose new location is ambiguous",
        report.recents_pruned,
        report.hooks_acks_pruned,
        report.overrides_migrated,
        report.overrides_kept_dead,
    );
    if report.unreachable_skipped > 0 {
        line.push_str(&format!(
            ", left {} path(s) alone on unreachable volumes",
            report.unreachable_skipped
        ));
    }
    eprintln!("{line}");
}

#[cfg(test)]
#[path = "prune_fixtures.rs"]
mod fixtures;

#[cfg(test)]
#[path = "prune_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "prune_migration_tests.rs"]
mod migration_tests;

#[cfg(test)]
#[path = "prune_forge_tests.rs"]
mod forge_tests;
