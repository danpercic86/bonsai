//! `FakeTransport` — the recording test double for [`Transport`]
//! (`cfg(test)` or the `test-support` feature only). It performs no network
//! IO. Every request is recorded exactly as a real transport would send it
//! (method, URL, body), so the sentinels assert on the real outbound set
//! `[NEW-1]`.
//!
//! Responses, in order:
//! 1. a test `responder` closure, if it returns `Some`;
//! 2. a recorded fixture file under `fixture_dir` (layout below);
//! 3. the empty default: deps.dev 404, a querybatch of empty results sized
//!    to the query count, advisory 404.
//!
//! Fixture layout (written by `recorder.rs`, names percent-encoded):
//! - `depsdev/<system>/<name>.json` — the raw deps.dev body;
//! - `osv/query/<ecosystem>/<name>@<version>.json` — ONE `results[]`
//!   element of a querybatch response;
//! - `osv/vulns/<id>.json` — the raw advisory record.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use super::transport::{
    pct_encode, Method, OutboundRequest, RawResponse, Transport, TransportError, DEPSDEV_BASE,
    OSV_BASE,
};

/// One request as it left the process.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordedRequest {
    pub method: Method,
    pub url: String,
    pub body: Option<String>,
}

/// A querybatch query as the fake reads it back from the body.
pub(in crate::remote) struct Query {
    pub(in crate::remote) ecosystem: String,
    pub(in crate::remote) name: String,
    pub(in crate::remote) version: String,
}

pub type Responder =
    Box<dyn Fn(&RecordedRequest) -> Option<Result<RawResponse, TransportError>> + Send + Sync>;

#[derive(Default)]
pub struct FakeTransport {
    fixture_dir: Option<PathBuf>,
    responder: Option<Responder>,
    recorded: Mutex<Vec<RecordedRequest>>,
}

impl FakeTransport {
    /// Empty defaults only (deps.dev 404, no advisories).
    pub fn new() -> Self {
        Self::default()
    }

    /// Serve recorded fixtures from `dir` (see the module doc).
    pub fn with_fixtures(mut self, dir: &Path) -> Self {
        self.fixture_dir = Some(dir.to_path_buf());
        self
    }

    /// Answer first with `f` (a `None` falls through to fixtures/defaults).
    pub fn with_responder(mut self, f: Responder) -> Self {
        self.responder = Some(f);
        self
    }

    /// Every request sent so far, in order.
    pub fn requests(&self) -> Vec<RecordedRequest> {
        self.recorded.lock().map(|v| v.clone()).unwrap_or_default()
    }

    fn fixture(&self, rel: &str) -> Option<Vec<u8>> {
        std::fs::read(self.fixture_dir.as_ref()?.join(rel)).ok()
    }

    /// [`Self::fixture`], but the file NAME must match byte-for-byte even on
    /// a case-insensitive filesystem: OSV matches NuGet ids case-sensitively
    /// (§0.1), so `newtonsoft.json` must not find `Newtonsoft.Json`'s file.
    fn fixture_exact(&self, rel: &str) -> Option<Vec<u8>> {
        let path = self.fixture_dir.as_ref()?.join(rel);
        let want = path.file_name()?.to_os_string();
        let found = std::fs::read_dir(path.parent()?)
            .ok()?
            .flatten()
            .any(|e| e.file_name() == want);
        if found {
            std::fs::read(path).ok()
        } else {
            None
        }
    }

    fn respond(&self, r: &RecordedRequest) -> Result<RawResponse, TransportError> {
        if let Some(out) = self.responder.as_ref().and_then(|f| f(r)) {
            return out;
        }
        let ok = |body: Vec<u8>| Ok(RawResponse { status: 200, body });
        let not_found = || {
            Ok(RawResponse {
                status: 404,
                body: b"{}".to_vec(),
            })
        };
        if let Some(rest) = r.url.strip_prefix(&format!("{DEPSDEV_BASE}/v3/systems/")) {
            return match depsdev_fixture_path(rest).and_then(|p| self.fixture(&p)) {
                Some(b) => ok(b),
                None => not_found(),
            };
        }
        if r.url == format!("{OSV_BASE}/v1/querybatch") {
            let queries =
                parse_queries(r.body.as_deref().unwrap_or("")).ok_or(TransportError::Other)?;
            let results: Vec<serde_json::Value> = queries
                .iter()
                .map(|q| {
                    self.fixture_exact(&query_fixture_path(q))
                        .and_then(|b| serde_json::from_slice(&b).ok())
                        .unwrap_or_else(|| serde_json::json!({}))
                })
                .collect();
            return ok(serde_json::json!({ "results": results })
                .to_string()
                .into_bytes());
        }
        if let Some(id) = r.url.strip_prefix(&format!("{OSV_BASE}/v1/vulns/")) {
            return match self.fixture(&vuln_fixture_path(id)) {
                Some(b) => ok(b),
                None => not_found(),
            };
        }
        Err(TransportError::Other)
    }
}

impl Transport for FakeTransport {
    fn send(&self, req: &OutboundRequest, cap: usize) -> Result<RawResponse, TransportError> {
        let r = RecordedRequest {
            method: req.method(),
            url: req.url().to_string(),
            body: req.body().map(str::to_string),
        };
        if let Ok(mut v) = self.recorded.lock() {
            v.push(r.clone());
        }
        let resp = self.respond(&r)?;
        if resp.body.len() > cap {
            return Err(TransportError::TooLarge);
        }
        Ok(resp)
    }
}

/// `<system>/packages/<pct-name>` ⇒ `depsdev/<system>/<pct-name>.json`.
pub(in crate::remote) fn depsdev_fixture_path(rest: &str) -> Option<String> {
    let (system, name) = rest.split_once("/packages/")?;
    Some(format!("depsdev/{system}/{name}.json"))
}

pub(in crate::remote) fn query_fixture_path(q: &Query) -> String {
    format!(
        "osv/query/{}/{}@{}.json",
        pct_encode(&q.ecosystem),
        pct_encode(&q.name),
        pct_encode(&q.version)
    )
}

pub(in crate::remote) fn vuln_fixture_path(id: &str) -> String {
    format!("osv/vulns/{}.json", pct_encode(id))
}

/// The queries of a querybatch body.
pub(in crate::remote) fn parse_queries(body: &str) -> Option<Vec<Query>> {
    let v: serde_json::Value = serde_json::from_str(body).ok()?;
    let arr = v.get("queries")?.as_array()?;
    arr.iter()
        .map(|q| {
            let pkg = q.get("package")?;
            Some(Query {
                ecosystem: pkg.get("ecosystem")?.as_str()?.to_string(),
                name: pkg.get("name")?.as_str()?.to_string(),
                version: q
                    .get("version")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
            })
        })
        .collect()
}
