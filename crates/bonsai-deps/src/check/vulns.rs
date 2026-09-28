//! §5.1 steps 8–10: deps.dev lookups, OSV querybatch chunks, and the
//! advisory details (reused from the previous report when `id` + `modified`
//! match — a string compare; an id is never minted from the cache
//! `[MEDIUM-2]`).

use std::collections::BTreeMap;

use crate::model::{Advisory, MAX_ADVISORIES};
use crate::privacy::PublicPackage;
use crate::remote::depsdev::fetch_versions;
use crate::remote::osv::{query_chunk, AdvisoryId, VulnRef, OSV_BATCH_MAX};
use crate::remote::osv_vuln::fetch_vuln;

use super::rows::{AdvisoryMap, LatestMap};
use super::{pool, CheckCtx, CheckError, Progress, ServiceFault, DEPSDEV_PARALLEL};

/// Step 8: one deps.dev GET per package, `DEPSDEV_PARALLEL` in flight; a
/// 404 is Unknown, any other failure aborts the check.
pub(crate) fn fetch_all_versions(
    ctx: &CheckCtx<'_>,
    queries: &[PublicPackage],
    progress: &Progress<'_>,
) -> Result<LatestMap, CheckError> {
    let found = pool::run(queries, DEPSDEV_PARALLEL, ctx.cancel, |p| {
        let v = fetch_versions(ctx.transport, p)?;
        progress.tick();
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
) -> Result<AdvisoryMap, CheckError> {
    let cancelled = || ctx.cancel.is_cancelled();
    let mut per_pkg: Vec<(&PublicPackage, Vec<VulnRef>)> = Vec::new();
    for chunk in queries.chunks(OSV_BATCH_MAX) {
        let hits = query_chunk(ctx.transport, chunk, &cancelled)?;
        progress.tick();
        per_pkg.extend(chunk.iter().zip(hits));
    }
    let mut unique: BTreeMap<AdvisoryId, VulnRef> = BTreeMap::new();
    for (_, hits) in &per_pkg {
        for h in hits {
            unique.entry(h.id.clone()).or_insert_with(|| h.clone());
        }
    }
    if unique.len() > MAX_ADVISORIES {
        return Err(CheckError::Service(ServiceFault::TooManyAdvisories));
    }
    let prev: BTreeMap<&str, &Advisory> = ctx
        .prev
        .into_iter()
        .flat_map(|r| &r.groups)
        .flat_map(|g| &g.rows)
        .flat_map(|row| &row.vulns)
        .map(|a| (a.id.as_str(), a))
        .collect();
    let mut details: BTreeMap<AdvisoryId, Advisory> = BTreeMap::new();
    let mut to_fetch: Vec<VulnRef> = Vec::new();
    for (id, r) in unique {
        match prev.get(id.as_str()) {
            Some(a) if a.modified == r.modified => {
                details.insert(id, (*a).clone());
            }
            _ => to_fetch.push(r),
        }
    }
    let fetched = pool::run(&to_fetch, DEPSDEV_PARALLEL, ctx.cancel, |r| {
        fetch_vuln(ctx.transport, r)
    })?;
    for (r, a) in to_fetch.into_iter().zip(fetched) {
        details.insert(r.id, a);
    }
    let mut out: AdvisoryMap = BTreeMap::new();
    for (p, hits) in per_pkg {
        let Some(version) = p.version() else { continue };
        let mut advs: Vec<Advisory> = hits
            .iter()
            .filter_map(|h| details.get(&h.id).cloned())
            .collect();
        sort_advisories(&mut advs);
        out.insert(
            (p.ecosystem(), p.name().to_string(), version.to_string()),
            advs,
        );
    }
    Ok(out)
}
