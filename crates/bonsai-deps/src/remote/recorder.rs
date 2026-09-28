//! `RecordingTransport` — writes the responses of a real transport into the
//! `FakeTransport` fixture layout (`cfg(test)` / `test-support` only). Used
//! ONLY by the ignored live recorder (`tests/live_record.rs`), which runs
//! `run_check` over a test-built repo of well-known public packages, so the
//! recorded set is exactly what the pipeline sends (no repo-derived names).
//!
//! 200 responses are written; a 404 writes nothing (the fake's default is
//! 404). A querybatch response is split into one file per query.

use std::path::{Path, PathBuf};

use super::fake_transport::{
    depsdev_fixture_path, parse_queries, query_fixture_path, vuln_fixture_path,
};
use super::transport::{
    OutboundRequest, RawResponse, Transport, TransportError, DEPSDEV_BASE, OSV_BASE,
};

pub struct RecordingTransport<'a> {
    inner: &'a dyn Transport,
    dir: PathBuf,
}

impl<'a> RecordingTransport<'a> {
    pub fn new(inner: &'a dyn Transport, dir: &Path) -> Self {
        RecordingTransport {
            inner,
            dir: dir.to_path_buf(),
        }
    }

    fn write(&self, rel: &str, bytes: &[u8]) {
        let p = self.dir.join(rel);
        if let Some(parent) = p.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::write(p, bytes);
    }

    fn record(&self, req: &OutboundRequest, resp: &RawResponse) {
        if resp.status != 200 {
            return;
        }
        let url = req.url();
        if let Some(rest) = url.strip_prefix(&format!("{DEPSDEV_BASE}/v3/systems/")) {
            if let Some(p) = depsdev_fixture_path(rest) {
                self.write(&p, &resp.body);
            }
        } else if url == format!("{OSV_BASE}/v1/querybatch") {
            let (Some(queries), Ok(v)) = (
                parse_queries(req.body().unwrap_or("")),
                serde_json::from_slice::<serde_json::Value>(&resp.body),
            ) else {
                return;
            };
            let results = v.get("results").and_then(|r| r.as_array());
            for (q, r) in queries.iter().zip(results.into_iter().flatten()) {
                self.write(&query_fixture_path(q), r.to_string().as_bytes());
            }
        } else if let Some(id) = url.strip_prefix(&format!("{OSV_BASE}/v1/vulns/")) {
            self.write(&vuln_fixture_path(id), &resp.body);
        }
    }
}

impl Transport for RecordingTransport<'_> {
    fn send(&self, req: &OutboundRequest, cap: usize) -> Result<RawResponse, TransportError> {
        let resp = self.inner.send(req, cap)?;
        self.record(req, &resp);
        Ok(resp)
    }
}
