//! `summarize` — the only producer of [`DepSummary`] (contract §2.1).

use super::{Availability, DepSummary, GroupState, ManifestGroup, Relation, Severity, UpdateKind};

/// Counts ROWS across every group, except `not_tracked` / `parse_errors`,
/// which count GROUPS.
///
/// - `major`/`minor`/`patch`: rows by `update`.
/// - `vulnerable`: rows with non-empty `vulns` (direct + transitive, any
///   availability); `vulnerable_by_severity` buckets them by max severity.
/// - `up_to_date`: `Checked` DIRECT rows with `update == None`.
/// - `unknown` / `local` / `private`: rows by availability.
pub fn summarize(groups: &[ManifestGroup]) -> DepSummary {
    let mut s = DepSummary::default();
    for g in groups {
        match g.state {
            GroupState::NotTracked { .. } => s.not_tracked += 1,
            GroupState::ParseError { .. } => s.parse_errors += 1,
            GroupState::Ok | GroupState::NoLockfile => {}
        }
        for row in &g.rows {
            match row.update {
                Some(UpdateKind::Major) => s.major += 1,
                Some(UpdateKind::Minor) => s.minor += 1,
                Some(UpdateKind::Patch) => s.patch += 1,
                None => {}
            }
            if let Some(sev) = row.max_severity() {
                s.vulnerable += 1;
                let b = &mut s.vulnerable_by_severity;
                match sev {
                    Severity::Critical => b.critical += 1,
                    Severity::High => b.high += 1,
                    Severity::Medium => b.medium += 1,
                    Severity::Low => b.low += 1,
                    Severity::Unknown => b.unknown += 1,
                }
            }
            match row.availability {
                Availability::Checked => {
                    if row.relation == Relation::Direct && row.update.is_none() {
                        s.up_to_date += 1;
                    }
                }
                Availability::Unknown { .. } => s.unknown += 1,
                Availability::Local { .. } => s.local += 1,
                Availability::Private { .. } => s.private += 1,
            }
        }
    }
    s
}
