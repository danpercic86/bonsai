//! The transport seam (contract §7.2) and the ONLY outbound request type.
//!
//! D7 structural guarantee (§7.3): [`OutboundRequest`] has private fields
//! and its only constructors are `pub(in crate::remote)`; each takes a
//! `&PublicPackage` (constructible only in `privacy.rs`) or an
//! `&AdvisoryId` / `&PageToken` (constructible only in `remote::osv`, from
//! response data). URLs are built here from `&'static str` hosts; no caller
//! supplies a URL or a header.

use crate::model::Ecosystem;
use crate::privacy::PublicPackage;

use super::osv::{AdvisoryId, PageToken};

/// The two hosts this crate ever talks to (§7.1). HTTPS only.
pub(crate) const DEPSDEV_BASE: &str = "https://api.deps.dev";
pub(crate) const OSV_BASE: &str = "https://api.osv.dev";

/// §7.2 body caps.
pub const DEPSDEV_BODY_CAP: usize = 8 * 1024 * 1024;
pub const OSV_BATCH_BODY_CAP: usize = 16 * 1024 * 1024;
pub const OSV_VULN_BODY_CAP: usize = 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Method {
    Get,
    Post,
}

/// One request as it leaves the process. No public constructor, no
/// Deserialize / Default / From / Clone. The fields are readable only
/// inside `remote` (the transports).
pub struct OutboundRequest {
    method: Method,
    url: String,
    body: Option<String>,
}

impl OutboundRequest {
    /// `GET https://api.deps.dev/v3/systems/{system}/packages/{pct(name)}`.
    pub(in crate::remote) fn depsdev_package(p: &PublicPackage) -> Self {
        OutboundRequest {
            method: Method::Get,
            url: format!(
                "{DEPSDEV_BASE}/v3/systems/{}/packages/{}",
                depsdev_system(p.ecosystem()),
                pct_encode(p.name())
            ),
            body: None,
        }
    }

    /// `POST https://api.osv.dev/v1/querybatch`. `tokens[i]` pairs with
    /// `q[i]` (missing ⇒ none). A package without a version is sent without
    /// the `version` field (callers only pass versioned packages).
    pub(in crate::remote) fn osv_querybatch(
        q: &[PublicPackage],
        tokens: &[Option<&PageToken>],
    ) -> Self {
        let queries: Vec<serde_json::Value> = q
            .iter()
            .enumerate()
            .map(|(i, p)| {
                let mut o = serde_json::Map::new();
                o.insert(
                    "package".into(),
                    serde_json::json!({ "name": p.name(), "ecosystem": osv_ecosystem(p.ecosystem()) }),
                );
                if let Some(v) = p.version() {
                    o.insert("version".into(), v.into());
                }
                if let Some(Some(t)) = tokens.get(i) {
                    o.insert("page_token".into(), t.as_str().into());
                }
                serde_json::Value::Object(o)
            })
            .collect();
        OutboundRequest {
            method: Method::Post,
            url: format!("{OSV_BASE}/v1/querybatch"),
            body: Some(serde_json::json!({ "queries": queries }).to_string()),
        }
    }

    /// `GET https://api.osv.dev/v1/vulns/{id}` — `id` passed the mint shape
    /// check (`[A-Za-z0-9._:-]`), so it needs no encoding.
    pub(in crate::remote) fn osv_vuln(id: &AdvisoryId) -> Self {
        OutboundRequest {
            method: Method::Get,
            url: format!("{OSV_BASE}/v1/vulns/{}", id.as_str()),
            body: None,
        }
    }

    pub(in crate::remote) fn method(&self) -> Method {
        self.method
    }

    pub(in crate::remote) fn url(&self) -> &str {
        &self.url
    }

    pub(in crate::remote) fn body(&self) -> Option<&str> {
        self.body.as_deref()
    }
}

/// A response. No Debug: `body` is untrusted response text.
pub struct RawResponse {
    pub status: u16,
    pub body: Vec<u8>,
}

/// Why a send failed. No payload `[MEDIUM-1]`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransportError {
    /// Connect / DNS / TLS.
    Connect,
    Timeout,
    /// The body exceeded the per-request cap.
    TooLarge,
    /// The src-tauri `GatedTransport` refused (tracker disabled / epoch).
    Gated,
    Other,
}

pub trait Transport: Send + Sync {
    /// Sends `req`; a body longer than `cap` bytes is `TooLarge`.
    fn send(&self, req: &OutboundRequest, cap: usize) -> Result<RawResponse, TransportError>;
}

/// deps.dev system path segment (lowercase is accepted, §0.1).
pub(crate) fn depsdev_system(eco: Ecosystem) -> &'static str {
    match eco {
        Ecosystem::Npm => "npm",
        Ecosystem::Cargo => "cargo",
        Ecosystem::Pypi => "pypi",
        Ecosystem::Go => "go",
        Ecosystem::Nuget => "nuget",
    }
}

/// OSV `package.ecosystem` strings (§0.1).
pub(crate) fn osv_ecosystem(eco: Ecosystem) -> &'static str {
    match eco {
        Ecosystem::Npm => "npm",
        Ecosystem::Cargo => "crates.io",
        Ecosystem::Pypi => "PyPI",
        Ecosystem::Go => "Go",
        Ecosystem::Nuget => "NuGet",
    }
}

/// Percent-encodes every byte outside RFC 3986 `unreserved` (so `/` ⇒
/// `%2F`, `@` ⇒ `%40`).
pub(crate) fn pct_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}
