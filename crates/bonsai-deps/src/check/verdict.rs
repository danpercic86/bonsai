//! P125h §1 / §4c: whether a public package's OSV result may be read as
//! "checked", and if not, why. `rows.rs` builds the rows; this file owns the
//! verdict so a dropped or oversize advisory can never read as clean.

use std::collections::BTreeMap;

use crate::model::{Advisory, Ecosystem, VulnsUnverifiedReason};
use crate::privacy::PublicPackage;

/// One versioned package's OSV result.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct VulnResult {
    /// Sorted: severity desc, then id.
    pub(crate) advisories: Vec<Advisory>,
    /// `MalformedAdvisory` / `ResponseTooLarge`, or `None`.
    pub(crate) unverified: Option<VulnsUnverifiedReason>,
}

/// `(eco, wire name, version)` ⇒ the OSV result.
pub(crate) type AdvisoryMap = BTreeMap<(Ecosystem, String, String), VulnResult>;

/// The per-package reason from what `fetch_advisories` saw: a dropped id
/// outranks an oversize record (either way the row is not verified).
pub(crate) fn package_reason(dropped: u32, any_too_large: bool) -> Option<VulnsUnverifiedReason> {
    if dropped > 0 {
        Some(VulnsUnverifiedReason::MalformedAdvisory)
    } else if any_too_large {
        Some(VulnsUnverifiedReason::ResponseTooLarge)
    } else {
        None
    }
}

fn lookup<'a>(p: &PublicPackage, adv: &'a AdvisoryMap) -> Option<&'a VulnResult> {
    let v = p.version()?;
    adv.get(&(p.ecosystem(), p.name().to_string(), v.to_string()))
}

/// A direct public row's `(vulns, vulns_unverified)`; `vulns_checked` is
/// `vulns_unverified.is_none()`.
pub(crate) fn direct(
    p: &PublicPackage,
    osv_name_exact: bool,
    adv: &AdvisoryMap,
) -> (Vec<Advisory>, Option<VulnsUnverifiedReason>) {
    // Every versioned public package is queried, so a miss means "no
    // version"; should that ever break, the row still fails safe.
    let Some(r) = lookup(p, adv) else {
        return (Vec::new(), Some(VulnsUnverifiedReason::NoVersion));
    };
    // [MEDIUM-D1] a NuGet id in an unverified casing with zero OSV hits is
    // "not verified", never "clean".
    let reason = r.unverified.or_else(|| {
        (!osv_name_exact && r.advisories.is_empty()).then_some(VulnsUnverifiedReason::NameCasing)
    });
    (r.advisories.clone(), reason)
}

/// A transitive package gets a row iff it has advisories OR its result is
/// unverified (P125h R2: all-malformed hits are shown as "not verified",
/// never hidden as clean).
pub(crate) fn transitive<'a>(p: &PublicPackage, adv: &'a AdvisoryMap) -> Option<&'a VulnResult> {
    lookup(p, adv).filter(|r| !r.advisories.is_empty() || r.unverified.is_some())
}

/// Counted in `ManifestGroup::transitive_checked`: queried AND verified.
pub(crate) fn transitive_verified(p: &PublicPackage, adv: &AdvisoryMap) -> bool {
    lookup(p, adv).is_some_and(|r| r.unverified.is_none())
}
