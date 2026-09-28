//! D7 structural guarantee, part 1 (contract §7.3): `PublicPackage` can only
//! be made here, and only by [`classify`] returning `Visibility::Public`.
//! `classify` FAILS CLOSED `[HIGH-1]`.
//!
//! Private and local names DO appear in the local view and the cache by
//! design; D7's boundary is the socket `[MEDIUM-2]`.

use std::collections::{BTreeMap, BTreeSet};

use crate::adapter::DepSource;
use crate::model::{Ecosystem, LocalReason, PrivateReason};
use crate::registry_ctx::RegistryContext;

/// A dependency `classify` judged safe to send. Private fields, no
/// Deserialize / Default / From: the only way to get one is [`classify`].
///
/// A struct literal outside `privacy.rs` does not compile:
///
/// ```compile_fail,E0451
/// use bonsai_deps::model::Ecosystem;
/// use bonsai_deps::privacy::PublicPackage;
/// let _p = PublicPackage { eco: Ecosystem::Npm, name: "x".into(), version: None };
/// ```
///
/// Stable rustdoc does not enforce the error code above, so this positive
/// control proves the same paths resolve — the block above fails on the
/// private fields, not on a typo:
///
/// ```
/// use bonsai_deps::model::Ecosystem;
/// use bonsai_deps::privacy::PublicPackage;
/// fn _read(p: &PublicPackage) -> (Ecosystem, &str, Option<&str>) {
///     (p.ecosystem(), p.name(), p.version())
/// }
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublicPackage {
    eco: Ecosystem,
    name: String,
    version: Option<String>,
}

impl PublicPackage {
    pub fn ecosystem(&self) -> Ecosystem {
        self.eco
    }
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn version(&self) -> Option<&str> {
        self.version.as_deref()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Visibility {
    Public(PublicPackage),
    Local(LocalReason),
    /// The host is display-only; it is never sent.
    Private(PrivateReason, Option<String>),
}

/// `[HIGH-1 item 3]` names the repo's own manifests DECLARE (package.json
/// `name`, `[package].name`, `module`, …), per ecosystem. A dependency on
/// one of them is `Local(Workspace)` before any other rule runs.
#[derive(Default)]
pub struct LocalNames {
    by_eco: BTreeMap<Ecosystem, BTreeSet<String>>,
}

impl LocalNames {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, eco: Ecosystem, name: &str) {
        self.by_eco
            .entry(eco)
            .or_default()
            .insert(match_key(eco, name));
    }

    pub fn contains(&self, eco: Ecosystem, name: &str) -> bool {
        self.by_eco
            .get(&eco)
            .is_some_and(|s| s.contains(&match_key(eco, name)))
    }
}

/// The comparison key for local-name matching. Deliberately LOOSER than the
/// registry's own identity (case-folded everywhere; Cargo `_` ≡ `-`; PyPI
/// PEP 503): over-matching can only turn a dependency Local, never Public.
fn match_key(eco: Ecosystem, name: &str) -> String {
    let n = normalize(eco, name).to_lowercase();
    match eco {
        Ecosystem::Cargo => n.replace('_', "-"),
        _ => n,
    }
}

/// The wire name for an ecosystem: PyPI is PEP 503-normalized (lowercase,
/// runs of `-` `_` `.` collapsed to `-`); every other ecosystem is the
/// trimmed name as declared.
pub fn normalize(eco: Ecosystem, name: &str) -> String {
    let t = name.trim();
    if eco != Ecosystem::Pypi {
        return t.to_string();
    }
    let mut out = String::with_capacity(t.len());
    let mut in_sep = false;
    for c in t.chars() {
        if matches!(c, '-' | '_' | '.') {
            if !in_sep {
                out.push('-');
            }
            in_sep = true;
        } else {
            out.push(c.to_ascii_lowercase());
            in_sep = false;
        }
    }
    out
}

/// §7.3. Order matters: local names, then non-registry sources, then the
/// fail-closed flag, then the E2 private signals, then the name shape.
pub fn classify(
    eco: Ecosystem,
    name: &str,
    version: Option<&str>,
    src: &DepSource,
    reg: &RegistryContext,
    local: &LocalNames,
) -> Visibility {
    let n = normalize(eco, name);
    if local.contains(eco, &n) {
        return Visibility::Local(LocalReason::Workspace);
    }
    match src {
        DepSource::Path => return Visibility::Local(LocalReason::Path),
        DepSource::Git => return Visibility::Local(LocalReason::Git),
        DepSource::Workspace => return Visibility::Local(LocalReason::Workspace),
        DepSource::Url => return Visibility::Local(LocalReason::Url),
        DepSource::Registry(_) => {}
    }
    if reg.fail_closed(eco) {
        return Visibility::Private(PrivateReason::ConfigUnreadable, None);
    }
    if let Some((reason, host)) = reg.private_reason(eco, &n, src) {
        return Visibility::Private(reason, host);
    }
    if !valid_public_name(eco, &n) {
        return Visibility::Private(PrivateReason::CustomRegistry, None);
    }
    Visibility::Public(PublicPackage {
        eco,
        name: n,
        version: version
            .filter(|v| valid_version_chars(v))
            .map(str::to_string),
    })
}

fn all_bytes(s: &str, ok: impl Fn(u8) -> bool) -> bool {
    s.bytes().all(ok)
}

/// npm name part: first byte `[a-z0-9-~]`, rest `[a-z0-9-._~]`.
fn npm_part(s: &str) -> bool {
    let mut b = s.bytes();
    let first_ok = b
        .next()
        .is_some_and(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-' || c == b'~');
    first_ok
        && b.all(|c| {
            c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, b'-' | b'.' | b'_' | b'~')
        })
}

/// PEP 508 name: `^([A-Z0-9]|[A-Z0-9][A-Z0-9._-]*[A-Z0-9])$`, case-insensitive.
fn pep508_name(s: &str) -> bool {
    let bytes = s.as_bytes();
    let (Some(first), Some(last)) = (bytes.first(), bytes.last()) else {
        return false;
    };
    first.is_ascii_alphanumeric()
        && last.is_ascii_alphanumeric()
        && all_bytes(s, |c| {
            c.is_ascii_alphanumeric() || matches!(c, b'.' | b'_' | b'-')
        })
}

/// §7.3 name shape. For every ecosystem the name and each `/`-separated
/// element must be neither `.` nor `..` (nor empty) `[LOW-2]`.
pub fn valid_public_name(eco: Ecosystem, n: &str) -> bool {
    if n.is_empty() || n.split('/').any(|e| e.is_empty() || e == "." || e == "..") {
        return false;
    }
    let len = n.chars().count();
    match eco {
        Ecosystem::Npm => {
            if len > 214 {
                return false;
            }
            match n.strip_prefix('@') {
                Some(rest) => match rest.split_once('/') {
                    Some((scope, pkg)) => npm_part(scope) && npm_part(pkg),
                    None => false,
                },
                None => npm_part(n),
            }
        }
        Ecosystem::Cargo => {
            len <= 64 && all_bytes(n, |c| c.is_ascii_alphanumeric() || matches!(c, b'_' | b'-'))
        }
        Ecosystem::Pypi => len <= 128 && pep508_name(n),
        Ecosystem::Go => {
            let charset = all_bytes(n, |c| {
                c.is_ascii_alphanumeric() || matches!(c, b'.' | b'_' | b'~' | b'/' | b'-')
            });
            let first = n.split('/').next().unwrap_or("");
            // `[LOW-2]` the first element contains a `.` AND a letter or digit.
            len <= 256
                && charset
                && first.contains('.')
                && first.bytes().any(|c| c.is_ascii_alphanumeric())
        }
        Ecosystem::Nuget => {
            len <= 100
                && all_bytes(n, |c| {
                    c.is_ascii_alphanumeric() || matches!(c, b'.' | b'_' | b'-')
                })
        }
    }
}

/// `^[A-Za-z0-9.+_!~-]{1,128}$`.
fn valid_version_chars(v: &str) -> bool {
    !v.is_empty()
        && v.len() <= 128
        && all_bytes(v, |c| {
            c.is_ascii_alphanumeric() || matches!(c, b'.' | b'+' | b'_' | b'!' | b'~' | b'-')
        })
}

#[cfg(test)]
#[path = "privacy_tests.rs"]
mod tests;
