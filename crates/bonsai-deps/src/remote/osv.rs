//! OSV `querybatch` (contract §5.1 step 9) and THE ONLY HOME of
//! [`AdvisoryId`] and [`PageToken`] `[MEDIUM-2]`. Neither has a public
//! constructor, nor a Deserialize / TryFrom / From impl: they are minted
//! here, and only from fields of a deserialized OSV response, so response
//! data can build a URL only through a value that passed the shape check
//! (§7.5). The advisory-record mapping lives in `osv_vuln.rs`.

use serde::Deserialize;

use crate::check::error::{status_error, CheckError, ServiceFault};
use crate::privacy::PublicPackage;

use super::transport::{OutboundRequest, Transport, OSV_BATCH_BODY_CAP};

/// §0.1 VERIFIED (P125d live probe): 1000 queries ⇒ 200, 1001 ⇒ 400
/// `"too many queries"`.
pub const OSV_BATCH_MAX: usize = 1000;
/// §5.1 step 9: more pages than this for one query ⇒ `Service(PageLimit)`.
pub const OSV_MAX_PAGES: usize = 10;

/// An OSV advisory id (`GHSA-…`, `RUSTSEC-…`, `GO-…`, `PYSEC-…`, …).
///
/// It cannot be built outside `remote::osv` — not with the tuple
/// constructor:
///
/// ```compile_fail,E0603
/// use bonsai_deps::remote::osv::AdvisoryId;
/// let _id = AdvisoryId("x".into());
/// ```
///
/// nor with a parser (there is none):
///
/// ```compile_fail,E0599
/// use bonsai_deps::remote::osv::AdvisoryId;
/// let _id = AdvisoryId::parse("x");
/// ```
///
/// nor through the private mint:
///
/// ```compile_fail,E0603
/// let _id = bonsai_deps::remote::osv::mint_id("GHSA-aaaa-bbbb-cccc");
/// ```
///
/// Stable rustdoc does not enforce the error codes above, so this positive
/// control proves the same paths resolve (the blocks fail on privacy, not
/// on a typo):
///
/// ```
/// use bonsai_deps::remote::osv::{AdvisoryId, PageToken};
/// fn _read(id: &AdvisoryId, t: &PageToken) -> (String, String) {
///     (id.as_str().to_string(), t.as_str().to_string())
/// }
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct AdvisoryId(String);

/// An OSV `querybatch` `next_page_token`.
///
/// ```compile_fail,E0603
/// use bonsai_deps::remote::osv::PageToken;
/// let _t = PageToken("x".into());
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageToken(String);

impl AdvisoryId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl PageToken {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Max advisory-id length accepted by [`mint_id`] (`^[A-Za-z0-9][A-Za-z0-9._:-]{0,63}$`).
const MAX_ID_LEN: usize = 64;
/// Max page-token length accepted by [`mint_page_token`] (1 KiB).
const MAX_TOKEN_LEN: usize = 1024;

/// Call ONLY on a field of a deserialized OSV DTO.
fn mint_id(raw: &str) -> Option<AdvisoryId> {
    is_id_shape(raw).then(|| AdvisoryId(raw.to_string()))
}

/// The [`mint_id`] shape check alone, for alias strings (which are display
/// text, never a URL component).
pub(in crate::remote) fn is_id_shape(raw: &str) -> bool {
    let b = raw.as_bytes();
    let first_ok = b.first().is_some_and(|c| c.is_ascii_alphanumeric());
    let rest_ok = b
        .iter()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'.' | b'_' | b':' | b'-'));
    first_ok && rest_ok && b.len() <= MAX_ID_LEN
}

/// Call ONLY on the `next_page_token` of a deserialized OSV response.
fn mint_page_token(raw: &str) -> Option<PageToken> {
    let ok = !raw.is_empty()
        && raw.len() <= MAX_TOKEN_LEN
        && raw.bytes().all(|c| c.is_ascii_graphic() || c == b' ');
    ok.then(|| PageToken(raw.to_string()))
}

#[derive(Deserialize)]
struct BatchDto {
    #[serde(default)]
    results: Vec<ResultDto>,
}

#[derive(Deserialize)]
struct ResultDto {
    #[serde(default)]
    vulns: Vec<VulnRefDto>,
    #[serde(default)]
    next_page_token: Option<String>,
}

#[derive(Deserialize)]
struct VulnRefDto {
    id: String,
    #[serde(default)]
    modified: String,
}

/// One querybatch hit: a minted id and its `modified` (the §5.1 step 10
/// reuse key — the querybatch value, whose precision differs from the
/// `/v1/vulns/{id}` record's, §0.1).
#[derive(Clone, PartialEq, Eq)]
pub struct VulnRef {
    pub id: AdvisoryId,
    pub modified: String,
}

/// Every hit OSV reported for one package, across all its pages.
#[derive(Clone, Default)]
pub struct PackageHits {
    /// The hits whose id passed the shape check.
    pub refs: Vec<VulnRef>,
    /// P125h §1: hits whose id FAILED the shape check. Non-zero ⇒ the
    /// package is `MalformedAdvisory`, never "clean".
    pub dropped: u32,
}

/// Mints every id of one result. `[P125d review, LOW-D2]` an id failing
/// the shape check is DROPPED (§7.5), not a failed check — but it is
/// COUNTED, so the package cannot read as clean (P125h §1).
fn mint_refs(r: Vec<VulnRefDto>) -> (Vec<VulnRef>, u32) {
    let mut dropped = 0u32;
    let refs = r
        .into_iter()
        .filter_map(|v| {
            let Some(id) = mint_id(&v.id) else {
                dropped = dropped.saturating_add(1);
                return None;
            };
            Some(VulnRef {
                id,
                modified: crate::model::clean_display(&v.modified, crate::model::MAX_ID_DISPLAY),
            })
        })
        .collect();
    (refs, dropped)
}

/// §5.1 step 9 for ONE chunk (≤ [`OSV_BATCH_MAX`] versioned packages):
/// the batch, then re-sends of only the paged queries with their OSV-issued
/// tokens, ≤ [`OSV_MAX_PAGES`] requests per query. `cancelled` is polled
/// before every send. Returns one [`PackageHits`] per input package (the
/// dropped counts summed across pages). An oversize querybatch body stays a
/// whole-check `Service(TooLarge)` (P125h §2): it cannot be tied to one
/// package.
pub fn query_chunk(
    t: &dyn Transport,
    chunk: &[PublicPackage],
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<PackageHits>, CheckError> {
    let mut hits: Vec<PackageHits> = vec![PackageHits::default(); chunk.len()];
    // (index into `chunk`, token for the next page).
    let mut pending: Vec<(usize, Option<PageToken>)> =
        (0..chunk.len()).map(|i| (i, None)).collect();
    let mut pages = 0usize;
    while !pending.is_empty() {
        if pages == OSV_MAX_PAGES {
            return Err(CheckError::Service(ServiceFault::PageLimit));
        }
        pages += 1;
        if cancelled() {
            return Err(CheckError::Cancelled);
        }
        let pkgs: Vec<PublicPackage> = pending.iter().map(|(i, _)| chunk[*i].clone()).collect();
        let tokens: Vec<Option<&PageToken>> = pending.iter().map(|(_, t)| t.as_ref()).collect();
        let req = OutboundRequest::osv_querybatch(&pkgs, &tokens);
        let resp = t.send(&req, OSV_BATCH_BODY_CAP)?;
        if resp.status != 200 {
            return Err(status_error(resp.status));
        }
        let dto: BatchDto = serde_json::from_slice(&resp.body)
            .map_err(|_| CheckError::Service(ServiceFault::BadJson))?;
        if dto.results.len() != pending.len() {
            return Err(CheckError::Service(ServiceFault::BadJson));
        }
        let mut next = Vec::new();
        for ((i, _), r) in pending.iter().zip(dto.results) {
            let (refs, dropped) = mint_refs(r.vulns);
            let h = &mut hits[*i];
            h.refs.extend(refs);
            h.dropped = h.dropped.saturating_add(dropped);
            if let Some(raw) = r.next_page_token.filter(|s| !s.is_empty()) {
                let tok =
                    mint_page_token(&raw).ok_or(CheckError::Service(ServiceFault::BadJson))?;
                next.push((*i, Some(tok)));
            }
        }
        pending = next;
    }
    Ok(hits)
}

#[cfg(test)]
#[path = "osv_tests.rs"]
mod tests;
