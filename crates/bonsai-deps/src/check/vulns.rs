//! §5.1 steps 8–10: deps.dev lookups, OSV querybatch chunks, and the
//! advisory details (reused from the previous report when `id` + `modified`
//! match — a string compare; an id is never minted from the cache
//! `[MEDIUM-2]`).
//!
//! P125h §2: an oversize deps.dev body or advisory record degrades only its
//! package; every such response is counted by the check's [`Breaker`].

use std::collections::BTreeMap;

use crate::model::{Advisory, MAX_ADVISORIES};
use crate::privacy::PublicPackage;
use crate::remote::depsdev::{fetch_versions, VersionsLookup};
use crate::remote::osv::{query_chunk, AdvisoryId, PackageHits, VulnRef, OSV_BATCH_MAX};
use crate::remote::osv_vuln::{fetch_vuln, VulnDetail};

use super::rows::LatestMap;
use super::verdict::{package_reason, AdvisoryMap, VulnResult};
use super::{pool, Breaker, CheckCtx, CheckError, Progress, ServiceFault, DEPSDEV_PARALLEL};

/// Step 8: one deps.dev GET per package, `DEPSDEV_PARALLEL` in flight; a
/// 404 is Unknown, an oversize body is `TooLarge` (counted by `breaker`),
/// any other failure aborts the check.
pub(crate) fn fetch_all_versions(
    ctx: &CheckCtx<'_>,
    queries: &[PublicPackage],
    progress: &Progress<'_>,
    breaker: &Breaker,
) -> Result<LatestMap, CheckError> {
    let found = pool::run(queries, DEPSDEV_PARALLEL, ctx.cancel, |p| {
        let v = fetch_versions(ctx.transport, p)?;
        progress.tick();
        if v == VersionsLookup::TooLarge {
            breaker.degraded()?;
        }
        Ok(v)
    })?;
    Ok(queries
        .iter()
        .zip(found)
        .map(|(p, v)| ((p.ecosystem(), p.name().to_string()), v))
        .collect())
}

fn sort_advisories(v: &mut Vec<Advisory>) {
    v.sort_by(|a, b| {
        b.severity
            .rank()
            .cmp(&a.severity.rank())
            .then(a.id.cmp(&b.id))
    });
    v.dedup_by(|a, b| a.id == b.id);
}

/// Steps 9–10.
pub(crate) fn fetch_advisories(
    ctx: &CheckCtx<'_>,
    queries: &[PublicPackage],
    progress: &Progress<'_>,
    breaker: &Breaker,
) -> Result<AdvisoryMap, CheckError> {
    let cancelled = || ctx.cancel.is_cancelled();
    let mut per_pkg: Vec<(&PublicPackage, PackageHits)> = Vec::new();
    for chunk in queries.chunks(OSV_BATCH_MAX) {
        let hits = query_chunk(ctx.transport, chunk, &cancelled)?;
        progress.tick();
        per_pkg.extend(chunk.iter().zip(hits));
    }
    let mut unique: BTreeMap<AdvisoryId, VulnRef> = BTreeMap::new();
    for (_, hits) in &per_pkg {
        for h in &hits.refs {
            unique.entry(h.id.clone()).or_insert_with(|| h.clone());
        }
    }
    if unique.len() > MAX_ADVISORIES {
        return Err(CheckError::Service(ServiceFault::TooManyAdvisories));
    }
    // P125h §2 reuse guard: an unverified prev row may carry an oversize
    // placeholder (whose `modified` matches the querybatch value), so NONE
    // of its advisories is reused — at most a re-fetch.
    let prev: BTreeMap<&str, &Advisory> = ctx
        .prev
        .into_iter()
        .flat_map(|r| &r.groups)
        .flat_map(|g| &g.rows)
        .filter(|row| row.vulns_unverified.is_none())
        .flat_map(|row| &row.vulns)
        .map(|a| (a.id.as_str(), a))
        .collect();
    // id ⇒ (advisory, is an oversize placeholder).
    let mut details: BTreeMap<AdvisoryId, (Advisory, bool)> = BTreeMap::new();
    let mut to_fetch: Vec<VulnRef> = Vec::new();
    for (id, r) in unique {
        match prev.get(id.as_str()) {
            Some(a) if a.modified == r.modified => {
                details.insert(id, ((*a).clone(), false));
            }
            _ => to_fetch.push(r),
        }
    }
    let fetched = pool::run(&to_fetch, DEPSDEV_PARALLEL, ctx.cancel, |r| {
        let d = fetch_vuln(ctx.transport, r)?;
        if matches!(d, VulnDetail::TooLarge(_)) {
            breaker.degraded()?;
        }
        Ok(d)
    })?;
    for (r, d) in to_fetch.into_iter().zip(fetched) {
        let entry = match d {
            VulnDetail::Record(a) => (a, false),
            VulnDetail::TooLarge(a) => (a, true),
        };
        details.insert(r.id, entry);
    }
    let mut out: AdvisoryMap = BTreeMap::new();
    for (p, hits) in per_pkg {
        let Some(version) = p.version() else { continue };
        let found: Vec<&(Advisory, bool)> = hits
            .refs
            .iter()
            .filter_map(|h| details.get(&h.id))
            .collect();
        let too_large = found.iter().any(|(_, big)| *big);
        let mut advisories: Vec<Advisory> = found.into_iter().map(|(a, _)| a.clone()).collect();
        sort_advisories(&mut advisories);
        out.insert(
            (p.ecosystem(), p.name().to_string(), version.to_string()),
            VulnResult {
                advisories,
                unverified: package_reason(hits.dropped, too_large),
            },
        );
    }
    Ok(out)
}
