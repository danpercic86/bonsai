//! Injectable HTTP seam (overview §F1) + header redaction.
//!
//! Provider logic depends only on the [`HttpTransport`] trait, so it is
//! unit-tested offline with a fake transport returning canned JSON. The
//! concrete [`ReqwestTransport`] is a thin adapter injected at construction.
//!
//! Security: neither [`HttpRequest`] nor [`HttpResponse`] ever prints a token.
//! Their `Debug` impls redact any `Authorization`/token header value and never
//! echo the response body, so the token cannot leak through a `{:?}` in a log
//! or an error (see `redaction_elides_token`).

use std::fmt;

use bonsai_core::error::AppError;

/// HTTP verbs the forge layer needs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HttpMethod {
    Get,
    Post,
}

/// A request built by the provider layer and handed to a transport. Headers
/// carry the `Authorization: Bearer <token>` value ONLY when authenticated.
#[derive(Clone)]
pub struct HttpRequest {
    pub method: HttpMethod,
    pub url: String,
    pub headers: Vec<(String, String)>,
    /// JSON body for POST; `None` for GET.
    pub body: Option<String>,
}

/// A transport response: status, headers (for `Link`/rate-limit parsing), and
/// the raw body string (parsed inside `github/dto.rs`).
#[derive(Clone)]
pub struct HttpResponse {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: String,
}

/// The injectable transport seam. Blocking; the command layer wraps calls in
/// `spawn_blocking`.
pub trait HttpTransport: Send + Sync {
    fn send(&self, req: &HttpRequest) -> Result<HttpResponse, AppError>;
}

/// Placeholder printed instead of any sensitive header value.
const REDACTED: &str = "<redacted>";

/// True for header names whose VALUE must never be printed (case-insensitive).
fn is_sensitive_header(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower == "authorization" || lower.contains("token")
}

/// The value to display for a header — redacted when the name is sensitive.
pub fn redact_header_value<'a>(name: &str, value: &'a str) -> &'a str {
    if is_sensitive_header(name) {
        REDACTED
    } else {
        value
    }
}

impl fmt::Debug for HttpRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let headers: Vec<(&str, &str)> = self
            .headers
            .iter()
            .map(|(k, v)| (k.as_str(), redact_header_value(k, v)))
            .collect();
        f.debug_struct("HttpRequest")
            .field("method", &self.method)
            .field("url", &self.url)
            .field("headers", &headers)
            // Body may echo user input but never a credential; still, avoid
            // dumping it — show only whether one is present.
            .field("body", &self.body.as_ref().map(|_| "<body>"))
            .finish()
    }
}

impl fmt::Debug for HttpResponse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Never echo the raw body (may contain private repo data); show length.
        f.debug_struct("HttpResponse")
            .field("status", &self.status)
            .field("headers", &self.headers)
            .field("body_len", &self.body.len())
            .finish()
    }
}

/// Concrete blocking transport over `reqwest`.
pub struct ReqwestTransport {
    client: reqwest::blocking::Client,
}

impl ReqwestTransport {
    /// Build a transport with a shared blocking client. Fails only if the TLS
    /// backend cannot be initialized.
    pub fn new() -> Result<Self, AppError> {
        let client = reqwest::blocking::Client::builder()
            .build()
            .map_err(|e| AppError::NetworkError(format!("failed to build HTTP client: {e}")))?;
        Ok(Self { client })
    }
}

impl HttpTransport for ReqwestTransport {
    fn send(&self, req: &HttpRequest) -> Result<HttpResponse, AppError> {
        let mut builder = match req.method {
            HttpMethod::Get => self.client.get(&req.url),
            HttpMethod::Post => self.client.post(&req.url),
        };
        for (k, v) in &req.headers {
            builder = builder.header(k.as_str(), v.as_str());
        }
        if let Some(body) = &req.body {
            builder = builder.body(body.clone());
        }
        // reqwest's error Display may include the URL (never a header), so it is
        // safe to surface; the token lives only in a header we never print.
        let resp = builder
            .send()
            .map_err(|e| AppError::NetworkError(format!("request failed: {e}")))?;
        let status = resp.status().as_u16();
        let headers = resp
            .headers()
            .iter()
            .map(|(k, v)| (k.as_str().to_string(), v.to_str().unwrap_or("").to_string()))
            .collect();
        let body = resp
            .text()
            .map_err(|e| AppError::NetworkError(format!("failed to read response body: {e}")))?;
        Ok(HttpResponse {
            status,
            headers,
            body,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The token must never surface through a `{:?}` of the request, and the
    /// Authorization value must render as the redaction placeholder.
    #[test]
    fn redaction_elides_token() {
        let token = "ghp_SUPERSECRETTOKENVALUE";
        let req = HttpRequest {
            method: HttpMethod::Get,
            url: "https://api.github.com/user".into(),
            headers: vec![
                ("Authorization".into(), format!("Bearer {token}")),
                ("Accept".into(), "application/vnd.github+json".into()),
            ],
            body: None,
        };
        let dbg = format!("{req:?}");
        assert!(!dbg.contains(token), "token leaked in Debug: {dbg}");
        assert!(!dbg.contains("Bearer"), "auth scheme leaked: {dbg}");
        assert!(dbg.contains(REDACTED), "expected redaction placeholder");
        // Non-sensitive headers are still visible.
        assert!(dbg.contains("application/vnd.github+json"));
    }

    #[test]
    fn redact_helper_targets_sensitive_names_only() {
        assert_eq!(redact_header_value("Authorization", "Bearer x"), REDACTED);
        assert_eq!(redact_header_value("authorization", "Bearer x"), REDACTED);
        assert_eq!(redact_header_value("X-Auth-Token", "abc"), REDACTED);
        assert_eq!(redact_header_value("Accept", "application/json"), "application/json");
    }

    #[test]
    fn response_debug_hides_body() {
        let resp = HttpResponse {
            status: 200,
            headers: vec![],
            body: "SECRET-PRIVATE-REPO-DATA".into(),
        };
        let dbg = format!("{resp:?}");
        assert!(!dbg.contains("SECRET-PRIVATE-REPO-DATA"), "body leaked: {dbg}");
        assert!(dbg.contains("body_len"));
    }
}
