//! `nuget` (E1): `N.N[.N[.N]][-pre][+meta]`, a 4th segment allowed,
//! case-insensitive pre-release labels. NuGet SemVer 2 order: numeric
//! segments (missing = 0), then pre-release; metadata ignored.

use super::{lower_bound_scan, VersionScheme};

pub struct NugetScheme;

/// A pre-release label. Variant order is the comparison order: numeric labels
/// sort before alphanumeric ones (SemVer 2 §11).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum Label {
    Num(u64),
    /// Lowercased: NuGet compares labels case-insensitively.
    Alpha(String),
}

/// Field order is the comparison order: `is_release` (false < true) puts a
/// pre-release before its release; `pre` then compares label by label.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct NugetVersion {
    nums: [u64; 4],
    is_release: bool,
    pre: Vec<Label>,
}

fn parse_nums(core: &str, min: usize) -> Option<[u64; 4]> {
    let mut nums = [0u64; 4];
    let mut n = 0usize;
    for part in core.split('.') {
        if n == 4 || part.is_empty() || !part.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        nums[n] = part.parse().ok()?;
        n += 1;
    }
    (n >= min).then_some(nums)
}

fn parse_pre(pre: &str) -> Option<Vec<Label>> {
    let mut out = Vec::new();
    for id in pre.split('.') {
        if id.is_empty() || !id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-') {
            return None;
        }
        if id.bytes().all(|b| b.is_ascii_digit()) {
            out.push(Label::Num(id.parse().ok()?));
        } else {
            out.push(Label::Alpha(id.to_ascii_lowercase()));
        }
    }
    Some(out)
}

fn parse_with_min(s: &str, min: usize) -> Option<NugetVersion> {
    let t = s.trim();
    let no_meta = t.split('+').next().unwrap_or(t);
    let (core, pre) = match no_meta.split_once('-') {
        Some((c, p)) => (c, Some(p)),
        None => (no_meta, None),
    };
    let nums = parse_nums(core, min)?;
    let pre = match pre {
        Some(p) => parse_pre(p)?,
        None => Vec::new(),
    };
    Some(NugetVersion {
        nums,
        is_release: pre.is_empty(),
        pre,
    })
}

/// The floor of one clause: `[1.0,2.0)` / `(1.0,)` / `[1.0]` ⇒ the left
/// endpoint; a floating `1.*` / `1.2.*` / `1.0.0-*` ⇒ its fixed prefix; a
/// plain `1.2.3` ⇒ itself (NuGet "minimum"). `*` / `(,2.0]` ⇒ None.
fn clause_floor(clause: &str) -> Option<NugetVersion> {
    let c = clause.trim();
    let lower = if let Some(rest) = c.strip_prefix(['[', '(']) {
        let inner = rest.trim_end_matches([']', ')']);
        inner.split(',').next().unwrap_or("").trim()
    } else {
        c
    };
    let fixed = lower
        .trim_end_matches("-*")
        .trim_end_matches(".*")
        .trim_end_matches('*');
    if fixed.is_empty() {
        return None;
    }
    parse_with_min(fixed, 1)
}

impl VersionScheme for NugetScheme {
    type V = NugetVersion;

    fn parse(&self, s: &str) -> Option<NugetVersion> {
        parse_with_min(s, 2)
    }

    fn is_prerelease(&self, v: &NugetVersion) -> bool {
        !v.is_release
    }

    fn segments(&self, v: &NugetVersion) -> (u64, u64, u64) {
        (v.nums[0], v.nums[1], v.nums[2])
    }

    fn lower_bound(&self, constraint: &str) -> Option<NugetVersion> {
        // NuGet ranges contain `,` inside brackets, so clauses split on
        // whitespace only.
        lower_bound_scan(constraint, char::is_whitespace, clause_floor)
    }
}

#[cfg(test)]
#[path = "nuget_tests.rs"]
mod tests;
