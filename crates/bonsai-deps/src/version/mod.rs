//! Version rules (contract §3; per-scheme table `P125-ecosystems.md` E1).
//!
//! Every parser here is an iterative scanner: no recursion `[HIGH-2]`.

mod go;
mod nuget;
mod pep440;
mod semver_s;

pub use go::GoScheme;
pub use nuget::NugetScheme;
pub use pep440::Pep440Scheme;
pub use semver_s::SemverScheme;

use crate::model::{Ecosystem, UpdateKind};

/// §3.1. `epoch` is an additive default method: §3.2 says "epoch differs ⇒
/// Major", and only PEP 440 has an epoch.
pub trait VersionScheme: Sync {
    type V: Ord + Clone;
    /// Concrete versions only; iterative, no recursion.
    fn parse(&self, s: &str) -> Option<Self::V>;
    fn is_prerelease(&self, v: &Self::V) -> bool;
    /// First three numeric release components (missing = 0).
    fn segments(&self, v: &Self::V) -> (u64, u64, u64);
    /// Floor of a constraint (the no-lockfile case). None when the constraint
    /// has no lower bound (e.g. `*`, `<2`) or does not parse.
    fn lower_bound(&self, constraint: &str) -> Option<Self::V>;
    fn epoch(&self, _v: &Self::V) -> u64 {
        0
    }
}

/// One registry version as `select_latest` needs it (deps.dev `isDefault` /
/// `isDeprecated`; the DTO arrives in P125d).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VersionCandidate<'a> {
    pub version: &'a str,
    pub is_default: bool,
    pub is_deprecated: bool,
}

/// §3.2 on two concrete versions. None when `latest <= current` or either
/// side does not parse.
pub fn classify_update(eco: Ecosystem, current: &str, latest: &str) -> Option<UpdateKind> {
    match eco {
        Ecosystem::Npm | Ecosystem::Cargo => classify_with(&SemverScheme, current, latest),
        Ecosystem::Pypi => classify_with(&Pep440Scheme, current, latest),
        Ecosystem::Go => classify_with(&GoScheme, current, latest),
        Ecosystem::Nuget => classify_with(&NugetScheme, current, latest),
    }
}

/// §3.3 no-lockfile case: `classify_update(lower_bound(declared), latest)`.
pub fn classify_update_declared(
    eco: Ecosystem,
    declared: &str,
    latest: &str,
) -> Option<UpdateKind> {
    match eco {
        Ecosystem::Npm | Ecosystem::Cargo => declared_with(&SemverScheme, declared, latest),
        Ecosystem::Pypi => declared_with(&Pep440Scheme, declared, latest),
        Ecosystem::Go => declared_with(&GoScheme, declared, latest),
        Ecosystem::Nuget => declared_with(&NugetScheme, declared, latest),
    }
}

/// §3.3 (AC3). Returns the chosen candidate's original version string.
pub fn select_latest<'a>(
    eco: Ecosystem,
    versions: &[VersionCandidate<'a>],
    current: Option<&str>,
) -> Option<&'a str> {
    match eco {
        Ecosystem::Npm | Ecosystem::Cargo => select_with(&SemverScheme, versions, current),
        Ecosystem::Pypi => select_with(&Pep440Scheme, versions, current),
        Ecosystem::Go => select_with(&GoScheme, versions, current),
        Ecosystem::Nuget => select_with(&NugetScheme, versions, current),
    }
}

fn classify_versions<S: VersionScheme>(s: &S, cur: &S::V, latest: &S::V) -> Option<UpdateKind> {
    if latest <= cur {
        return None;
    }
    let (a, b, _) = s.segments(cur);
    let (x, y, _) = s.segments(latest);
    if s.epoch(cur) != s.epoch(latest) || x != a {
        Some(UpdateKind::Major)
    } else if y != b {
        Some(UpdateKind::Minor)
    } else {
        Some(UpdateKind::Patch)
    }
}

fn classify_with<S: VersionScheme>(s: &S, current: &str, latest: &str) -> Option<UpdateKind> {
    classify_versions(s, &s.parse(current)?, &s.parse(latest)?)
}

fn declared_with<S: VersionScheme>(s: &S, declared: &str, latest: &str) -> Option<UpdateKind> {
    classify_versions(s, &s.lower_bound(declared)?, &s.parse(latest)?)
}

fn select_with<'a, S: VersionScheme>(
    s: &S,
    versions: &[VersionCandidate<'a>],
    current: Option<&str>,
) -> Option<&'a str> {
    let allow_pre = current
        .and_then(|c| s.parse(c))
        .is_some_and(|v| s.is_prerelease(&v));
    let parsed: Vec<(S::V, &VersionCandidate<'a>)> = versions
        .iter()
        .filter_map(|c| s.parse(c.version).map(|v| (v, c)))
        .collect();
    let best = if !allow_pre {
        if let Some((_, d)) = parsed
            .iter()
            .find(|(v, c)| c.is_default && !c.is_deprecated && !s.is_prerelease(v))
        {
            return Some(d.version);
        }
        parsed
            .iter()
            .filter(|(v, c)| !c.is_deprecated && !s.is_prerelease(v))
            .max_by(|l, r| l.0.cmp(&r.0))
    } else {
        parsed
            .iter()
            .filter(|(_, c)| !c.is_deprecated)
            .max_by(|l, r| l.0.cmp(&r.0))
    };
    best.map(|(_, c)| c.version)
}

/// Shared constraint floor scan. `||` separates alternatives (the floor is the
/// MIN over alternatives); inside one alternative `is_sep` splits clauses
/// (the floor is the MAX over clauses, i.e. the intersection). A token that is
/// only operator characters is glued to the next token (`>= 1.2`). `bound`
/// maps one `op+version` clause to its floor, or None for upper-only /
/// unparseable clauses.
pub(crate) fn lower_bound_scan<V: Ord>(
    constraint: &str,
    is_sep: fn(char) -> bool,
    bound: impl Fn(&str) -> Option<V>,
) -> Option<V> {
    let mut best_alt: Option<V> = None;
    for alt in constraint.split("||") {
        let mut floor: Option<V> = None;
        let mut pending_op = String::new();
        let mut skip_next = false;
        for tok in alt.split(is_sep).filter(|t| !t.is_empty()) {
            if skip_next {
                skip_next = false;
                continue;
            }
            if tok == "-" {
                // npm hyphen range `A - B`: B is an upper bound.
                skip_next = true;
                continue;
            }
            if tok.chars().all(|c| "<>=!~^".contains(c)) {
                pending_op.push_str(tok);
                continue;
            }
            let clause = format!("{pending_op}{tok}");
            pending_op.clear();
            if let Some(v) = bound(&clause) {
                floor = Some(match floor {
                    Some(f) if f >= v => f,
                    _ => v,
                });
            }
        }
        if let Some(f) = floor {
            best_alt = Some(match best_alt {
                Some(b) if b <= f => b,
                _ => f,
            });
        }
    }
    best_alt
}

/// Strips a leading operator; None if it is an upper-bound or exclusion
/// operator (`<`, `<=`, `!=`).
pub(crate) fn strip_lower_op(clause: &str) -> Option<&str> {
    let c = clause.trim();
    if c.starts_with('<') || c.starts_with('!') {
        return None;
    }
    Some(c.trim_start_matches(['>', '=', '~', '^']).trim())
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;
