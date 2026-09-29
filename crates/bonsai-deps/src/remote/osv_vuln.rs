//! `GET /v1/vulns/{id}` → [`Advisory`] (contract §5.1 step 10 + severity).
//!
//! The DTO reads only `summary`, `aliases`, `severity[]{type,score}` and
//! `database_specific.severity`; `details`, `affected`, `references`, the
//! other `database_specific` keys (e.g. a Go record's `url`) and unknown
//! fields are ignored. Response data never builds a URL: `Advisory::url` is
//! made from the minted id.

use serde::Deserialize;

use crate::check::error::{status_error, CheckError, ServiceFault};
use crate::cvss::{band, base_score_v3};
use crate::model::{clean_display, Advisory, Severity, MAX_ID_DISPLAY, MAX_SUMMARY_DISPLAY};

use super::osv::{is_id_shape, VulnRef};
use super::transport::{OutboundRequest, Transport, TransportError, OSV_VULN_BODY_CAP};

/// §2.1: at most this many aliases per advisory.
pub const MAX_ALIASES: usize = 16;

#[derive(Deserialize)]
struct VulnDto {
    #[serde(default)]
    summary: Option<String>,
    #[serde(default)]
    aliases: Vec<String>,
    #[serde(default)]
    severity: Vec<SeverityDto>,
    #[serde(default)]
    database_specific: Option<DbSpecificDto>,
}

#[derive(Deserialize)]
struct SeverityDto {
    #[serde(rename = "type", default)]
    kind: String,
    #[serde(default)]
    score: String,
}

#[derive(Deserialize)]
struct DbSpecificDto {
    #[serde(default)]
    severity: Option<serde_json::Value>,
}

/// `database_specific.severity` (GHSA convention, §0.1).
fn db_severity(v: &serde_json::Value) -> Option<Severity> {
    match v.as_str()?.to_ascii_uppercase().as_str() {
        "CRITICAL" => Some(Severity::Critical),
        "HIGH" => Some(Severity::High),
        "MODERATE" | "MEDIUM" => Some(Severity::Medium),
        "LOW" => Some(Severity::Low),
        _ => None,
    }
}

/// The highest `CVSS_V3` base score, if any vector parses.
fn best_v3(sev: &[SeverityDto]) -> Option<f32> {
    sev.iter()
        .filter(|s| s.kind == "CVSS_V3")
        .filter_map(|s| base_score_v3(&s.score))
        .fold(None, |m, x| Some(m.map_or(x, |m: f32| m.max(x))))
}

/// The OSV page for an id. Only minted ids reach here.
pub fn advisory_url(id: &str) -> String {
    format!("https://osv.dev/vulnerability/{id}")
}

/// An advisory with no record (404): severity Unknown.
pub(crate) fn unknown_advisory(r: &VulnRef) -> Advisory {
    Advisory {
        id: clean_display(r.id.as_str(), MAX_ID_DISPLAY),
        aliases: Vec::new(),
        summary: None,
        severity: Severity::Unknown,
        cvss_score: None,
        modified: r.modified.clone(),
        url: advisory_url(r.id.as_str()),
    }
}

/// Maps a `/v1/vulns/{id}` response. §5.1 severity order:
/// `database_specific.severity`, else the highest `CVSS_V3` band, else
/// Unknown. `cvss_score` is the highest `CVSS_V3` score either way.
pub(crate) fn map_vuln(r: &VulnRef, status: u16, body: &[u8]) -> Result<Advisory, CheckError> {
    match status {
        200 => {}
        404 => return Ok(unknown_advisory(r)),
        s => return Err(status_error(s)),
    }
    let dto: VulnDto =
        serde_json::from_slice(body).map_err(|_| CheckError::Service(ServiceFault::BadJson))?;
    let cvss = best_v3(&dto.severity);
    let severity = dto
        .database_specific
        .as_ref()
        .and_then(|d| d.severity.as_ref())
        .and_then(db_severity)
        .or_else(|| cvss.map(band))
        .unwrap_or(Severity::Unknown);
    let aliases = dto
        .aliases
        .iter()
        .filter(|a| is_id_shape(a))
        .take(MAX_ALIASES)
        .map(|a| clean_display(a, MAX_ID_DISPLAY))
        .collect();
    let summary = dto
        .summary
        .map(|s| clean_display(s.trim(), MAX_SUMMARY_DISPLAY))
        .filter(|s| !s.is_empty());
    Ok(Advisory {
        id: clean_display(r.id.as_str(), MAX_ID_DISPLAY),
        aliases,
        summary,
        severity,
        cvss_score: cvss,
        modified: r.modified.clone(),
        url: advisory_url(r.id.as_str()),
    })
}

/// One advisory-detail outcome (P125h §2).
#[derive(Debug, Clone, PartialEq)]
pub enum VulnDetail {
    Record(Advisory),
    /// The record exceeded `OSV_VULN_BODY_CAP`: the [`unknown_advisory`]
    /// placeholder. Every row carrying it is `ResponseTooLarge`, and it is a
    /// degraded response for the check's breaker.
    TooLarge(Advisory),
}

/// One advisory-detail GET. An oversize record is a placeholder, not an
/// error (P125h §2).
pub fn fetch_vuln(t: &dyn Transport, r: &VulnRef) -> Result<VulnDetail, CheckError> {
    match t.send(&OutboundRequest::osv_vuln(&r.id), OSV_VULN_BODY_CAP) {
        Ok(resp) => map_vuln(r, resp.status, &resp.body).map(VulnDetail::Record),
        Err(TransportError::TooLarge) => Ok(VulnDetail::TooLarge(unknown_advisory(r))),
        Err(e) => Err(e.into()),
    }
}
