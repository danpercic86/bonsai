//! The production [`Transport`] (contract §7.2): blocking reqwest over
//! rustls with the pure-Rust `ring` provider.
//!
//! - Only [`OutboundRequest`] can be sent, so only the §7.1 URLs.
//! - Fixed headers: `Accept`, `User-Agent`, and `Content-Type` on POST. No
//!   cookie store, no auth.
//! - `redirect::Policy::none()`: a 3xx comes back as its status and the
//!   check maps it to `Service(Redirect)`.
//! - `https_only(true)`, `connect_timeout` 10 s, `timeout` 30 s (whole
//!   request incl. body).
//! - The body is read through `take(cap + 1)`; more than `cap` ⇒ `TooLarge`.
//!   No decompression feature is compiled in, so `cap` bounds wire bytes.
//! - reqwest honours `HTTPS_PROXY` (§7.1 audit INFO).

use std::io::Read;
use std::time::Duration;

use super::transport::{Method, OutboundRequest, RawResponse, Transport, TransportError};

pub const USER_AGENT: &str = "Bonsai-dependency-tracker";
pub const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
pub const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

pub struct ReqwestTransport {
    client: reqwest::blocking::Client,
}

/// Building the client failed (TLS backend). No payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TransportInitError;

impl ReqwestTransport {
    pub fn new() -> Result<Self, TransportInitError> {
        // Process-global and first-writer-wins (tauri installs one too), so
        // the result is ignored — same pattern as bonsai-forge `http.rs`.
        let _ = rustls::crypto::ring::default_provider().install_default();
        let client = reqwest::blocking::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .https_only(true)
            .connect_timeout(CONNECT_TIMEOUT)
            .timeout(REQUEST_TIMEOUT)
            .user_agent(USER_AGENT)
            .build()
            .map_err(|_| TransportInitError)?;
        Ok(ReqwestTransport { client })
    }
}

fn map_err(e: &reqwest::Error) -> TransportError {
    if e.is_timeout() {
        TransportError::Timeout
    } else if e.is_connect() {
        TransportError::Connect
    } else {
        TransportError::Other
    }
}

impl Transport for ReqwestTransport {
    fn send(&self, req: &OutboundRequest, cap: usize) -> Result<RawResponse, TransportError> {
        let builder = match req.method() {
            Method::Get => self.client.get(req.url()),
            Method::Post => self
                .client
                .post(req.url())
                .header("Content-Type", "application/json")
                .body(req.body().unwrap_or("").to_string()),
        };
        let resp = builder
            .header("Accept", "application/json")
            .send()
            .map_err(|e| map_err(&e))?;
        let status = resp.status().as_u16();
        let limit = u64::try_from(cap).unwrap_or(u64::MAX).saturating_add(1);
        let mut body = Vec::new();
        resp.take(limit)
            .read_to_end(&mut body)
            .map_err(|e| match e.kind() {
                std::io::ErrorKind::TimedOut => TransportError::Timeout,
                _ => TransportError::Other,
            })?;
        if body.len() > cap {
            return Err(TransportError::TooLarge);
        }
        Ok(RawResponse { status, body })
    }
}
