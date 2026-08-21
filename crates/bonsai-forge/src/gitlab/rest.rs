//! GitLab REST v4 plumbing: endpoint URL builders, header assembly, the
//! HTTP-status→`AppError` mapping, pagination (`X-Next-Page`/`Link`/count), and
//! thin GET/POST helpers over an injectable [`HttpTransport`].
//!
//! No provider policy lives here (that is `gitlab/mod.rs`) and no GitLab JSON is
//! parsed here (that is `gitlab/dto.rs`). The auth header value is assembled but
//! NEVER logged — the redaction seam in `http.rs` elides both `Authorization`
//! and any header name containing "token" (so `PRIVATE-TOKEN` is redacted too).

use bonsai_core::error::AppError;

use crate::http::{HttpMethod, HttpRequest, HttpResponse, HttpTransport};
use crate::types::PrStateFilter;

/// GitLab REST base for a host. Unlike GitHub (separate `api.github.com`),
/// GitLab serves its API under `/api/v4` on the SAME host, so the base is
/// host-parameterized. The token NEVER appears in a URL — only in a header.
fn api_base(host: &str) -> String {
    format!("https://{host}/api/v4")
}

/// Assemble the standard GitLab headers. GitLab authenticates with a
/// `PRIVATE-TOKEN` header (contract §3c), added ONLY when a token is present
/// (read paths work anonymously for public projects).
pub fn base_headers(token: Option<&str>) -> Vec<(String, String)> {
    let mut headers = vec![
        ("Accept".to_string(), "application/json".to_string()),
        ("User-Agent".to_string(), "Bonsai".to_string()),
    ];
    if let Some(t) = token {
        headers.push(("PRIVATE-TOKEN".to_string(), t.to_string()));
    }
    headers
}

/// GitLab MR list filter: `open→opened`, `closed→closed`, `all→all`.
fn state_param(state: PrStateFilter) -> &'static str {
    match state {
        PrStateFilter::Open => "opened",
        PrStateFilter::Closed => "closed",
        PrStateFilter::All => "all",
    }
}

/// Percent-encode a GitLab project path so `group/subgroup/project` becomes the
/// single `{id}` path segment GitLab expects (`group%2Fsubgroup%2Fproject`).
/// Encodes everything outside the RFC 3986 unreserved set (so `/` → `%2F`).
pub fn project_id(owner: &str, repo: &str) -> String {
    percent_encode(&format!("{owner}/{repo}"))
}

fn percent_encode(s: &str) -> String {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    let mut out = String::with_capacity(s.len() * 3);
    for &b in s.as_bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char);
            }
            _ => {
                out.push('%');
                out.push(HEX[(b >> 4) as usize] as char);
                out.push(HEX[(b & 0x0f) as usize] as char);
            }
        }
    }
    out
}

// ---- endpoint URL builders ----

pub fn user_url(host: &str) -> String {
    format!("{}/user", api_base(host))
}

pub fn merge_requests_url(
    host: &str,
    id: &str,
    state: PrStateFilter,
    per_page: u32,
    page: u32,
) -> String {
    format!(
        "{}/projects/{id}/merge_requests?state={}&per_page={per_page}&page={page}",
        api_base(host),
        state_param(state)
    )
}

pub fn merge_request_url(host: &str, id: &str, iid: u64) -> String {
    format!("{}/projects/{id}/merge_requests/{iid}", api_base(host))
}

pub fn create_merge_request_url(host: &str, id: &str) -> String {
    format!("{}/projects/{id}/merge_requests", api_base(host))
}

/// `PUT …/merge_requests/{iid}/merge` — the accept/merge endpoint.
pub fn merge_mr_url(host: &str, id: &str, iid: u64) -> String {
    format!("{}/projects/{id}/merge_requests/{iid}/merge", api_base(host))
}

pub fn notes_url(host: &str, id: &str, iid: u64) -> String {
    format!(
        "{}/projects/{id}/merge_requests/{iid}/notes?per_page=100",
        api_base(host)
    )
}

pub fn discussions_url(host: &str, id: &str, iid: u64) -> String {
    format!(
        "{}/projects/{id}/merge_requests/{iid}/discussions?per_page=100",
        api_base(host)
    )
}

pub fn commit_statuses_url(host: &str, id: &str, sha: &str) -> String {
    format!(
        "{}/projects/{id}/repository/commits/{sha}/statuses",
        api_base(host)
    )
}

// ---- response inspection ----

/// Case-insensitive header lookup.
fn header<'a>(resp: &'a HttpResponse, name: &str) -> Option<&'a str> {
    resp.headers
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case(name))
        .map(|(_, v)| v.as_str())
}

/// Whether another page follows (contract §3c): GitLab's `X-Next-Page` header
/// is authoritative when present (non-empty ⇒ next page, empty ⇒ last page);
/// a `Link: …rel="next"` is a secondary signal; otherwise fall back to a full
/// page (`returned == per_page`).
pub fn has_next_page(resp: &HttpResponse, per_page: u32, returned: usize) -> bool {
    if let Some(v) = header(resp, "x-next-page") {
        return !v.trim().is_empty();
    }
    if let Some(l) = header(resp, "link") {
        return l.contains("rel=\"next\"");
    }
    per_page > 0 && returned as u32 == per_page
}

/// A 403/429 that is actually a rate-limit (`RateLimit-Remaining == 0`).
fn is_rate_limited(resp: &HttpResponse) -> bool {
    header(resp, "ratelimit-remaining").map(|v| v.trim() == "0") == Some(true)
}

fn rate_limited_error(resp: &HttpResponse) -> AppError {
    match header(resp, "ratelimit-reset") {
        Some(reset) => AppError::ForgeRateLimited(format!(
            "GitLab API rate limit exceeded (resets at epoch {reset})"
        )),
        None => AppError::ForgeRateLimited("GitLab API rate limit exceeded".to_string()),
    }
}

/// Map a non-2xx response to an `AppError` (mirrors `github/rest.rs`). `None` ⇒
/// success (2xx). NEVER includes a token or the auth header.
pub fn map_status(resp: &HttpResponse) -> Option<AppError> {
    let s = resp.status;
    if (200..300).contains(&s) {
        return None;
    }
    Some(match s {
        401 => AppError::AuthFailed("GitLab rejected the credentials (401)".to_string()),
        403 => {
            if is_rate_limited(resp) {
                rate_limited_error(resp)
            } else {
                AppError::AuthFailed(
                    "GitLab denied the request (403) — the token may be invalid or lack the `api` scope"
                        .to_string(),
                )
            }
        }
        429 => rate_limited_error(resp),
        404 => AppError::ForgeApi("not found".to_string()),
        // Redirects are never followed (transport pins Policy::none).
        301 | 302 | 307 | 308 => AppError::ForgeApi(format!(
            "the project has moved (HTTP {s}) — it may have been renamed; update the remote URL"
        )),
        other => AppError::ForgeApi(format!("GitLab API error (HTTP {other})")),
    })
}

// ---- transport helpers ----

/// GET `url` with GitLab headers; map a non-2xx status to an error.
pub fn get(
    http: &dyn HttpTransport,
    url: &str,
    token: Option<&str>,
) -> Result<HttpResponse, AppError> {
    let req = HttpRequest {
        method: HttpMethod::Get,
        url: url.to_string(),
        headers: base_headers(token),
        body: None,
    };
    let resp = http.send(&req)?;
    match map_status(&resp) {
        Some(err) => Err(err),
        None => Ok(resp),
    }
}

/// POST `url` with a JSON `body`; map a non-2xx status to an error. Callers
/// requiring auth check the token BEFORE calling this.
pub fn post(
    http: &dyn HttpTransport,
    url: &str,
    token: Option<&str>,
    body: String,
) -> Result<HttpResponse, AppError> {
    let mut headers = base_headers(token);
    headers.push(("Content-Type".to_string(), "application/json".to_string()));
    let req = HttpRequest {
        method: HttpMethod::Post,
        url: url.to_string(),
        headers,
        body: Some(body),
    };
    let resp = http.send(&req)?;
    match map_status(&resp) {
        Some(err) => Err(err),
        None => Ok(resp),
    }
}

/// Clear not-mergeable message for a GitLab merge GitLab refused because the MR
/// is not in a mergeable state (405 Method Not Allowed / 406 Not Acceptable /
/// 409 Conflict).
pub fn not_mergeable_error() -> AppError {
    AppError::ForgeApi(
        "GitLab could not merge this MR — it is not mergeable (conflicts, unresolved \
         discussions, or pending approvals)"
            .to_string(),
    )
}

/// PUT `url` with a JSON `body`; standard status mapping. Used for the MR-close
/// (`state_event`) call. Callers requiring auth check the token BEFORE calling.
fn send_put(
    http: &dyn HttpTransport,
    url: &str,
    token: Option<&str>,
    body: String,
    merge_call: bool,
) -> Result<HttpResponse, AppError> {
    let mut headers = base_headers(token);
    headers.push(("Content-Type".to_string(), "application/json".to_string()));
    let req = HttpRequest {
        method: HttpMethod::Put,
        url: url.to_string(),
        headers,
        body: Some(body),
    };
    let resp = http.send(&req)?;
    if merge_call && matches!(resp.status, 405 | 406 | 409) {
        return Err(not_mergeable_error());
    }
    match map_status(&resp) {
        Some(err) => Err(err),
        None => Ok(resp),
    }
}

/// PUT `url` (close/update MR); standard status mapping.
pub fn put(
    http: &dyn HttpTransport,
    url: &str,
    token: Option<&str>,
    body: String,
) -> Result<HttpResponse, AppError> {
    send_put(http, url, token, body, false)
}

/// PUT `url` (merge MR); 405/406/409 map to [`not_mergeable_error`].
pub fn put_merge(
    http: &dyn HttpTransport,
    url: &str,
    token: Option<&str>,
    body: String,
) -> Result<HttpResponse, AppError> {
    send_put(http, url, token, body, true)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn resp(status: u16, headers: Vec<(&str, &str)>) -> HttpResponse {
        HttpResponse {
            status,
            headers: headers
                .into_iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
            body: String::new(),
        }
    }

    #[test]
    fn base_headers_use_private_token() {
        let none = base_headers(None);
        assert!(!none.iter().any(|(k, _)| k.eq_ignore_ascii_case("PRIVATE-TOKEN")));
        assert!(none.iter().any(|(k, v)| k == "Accept" && v == "application/json"));

        let auth = base_headers(Some("glpat-xyz"));
        assert!(auth
            .iter()
            .any(|(k, v)| k == "PRIVATE-TOKEN" && v == "glpat-xyz"));
        // No `Authorization: Bearer` — GitLab uses PRIVATE-TOKEN.
        assert!(!auth.iter().any(|(k, _)| k == "Authorization"));
    }

    #[test]
    fn project_id_encodes_nested_namespace() {
        assert_eq!(project_id("owner", "repo"), "owner%2Frepo");
        assert_eq!(
            project_id("group/subgroup", "project"),
            "group%2Fsubgroup%2Fproject"
        );
        // Unreserved chars survive; only the separators are escaped.
        assert_eq!(project_id("a.b_c-d", "e~f"), "a.b_c-d%2Fe~f");
    }

    #[test]
    fn urls_embed_encoded_id_and_no_token() {
        let id = project_id("group/sub", "proj");
        let u = merge_requests_url("gitlab.com", &id, PrStateFilter::Open, 30, 2);
        assert_eq!(
            u,
            "https://gitlab.com/api/v4/projects/group%2Fsub%2Fproj/merge_requests?state=opened&per_page=30&page=2"
        );
        assert!(!u.contains("glpat"));
        assert_eq!(
            merge_request_url("gitlab.com", &id, 5),
            "https://gitlab.com/api/v4/projects/group%2Fsub%2Fproj/merge_requests/5"
        );
        assert_eq!(
            commit_statuses_url("gitlab.com", &id, "abc"),
            "https://gitlab.com/api/v4/projects/group%2Fsub%2Fproj/repository/commits/abc/statuses"
        );
    }

    #[test]
    fn state_param_maps_filter() {
        assert_eq!(state_param(PrStateFilter::Open), "opened");
        assert_eq!(state_param(PrStateFilter::Closed), "closed");
        assert_eq!(state_param(PrStateFilter::All), "all");
    }

    #[test]
    fn map_status_taxonomy() {
        assert!(map_status(&resp(200, vec![])).is_none());
        assert!(matches!(
            map_status(&resp(401, vec![])),
            Some(AppError::AuthFailed(_))
        ));
        // 403 with remaining>0 ⇒ auth failure.
        assert!(matches!(
            map_status(&resp(403, vec![("RateLimit-Remaining", "57")])),
            Some(AppError::AuthFailed(_))
        ));
        // 403 with remaining==0 ⇒ rate limited, carrying the reset hint.
        let err = map_status(&resp(
            403,
            vec![("RateLimit-Remaining", "0"), ("RateLimit-Reset", "1700000000")],
        ))
        .unwrap();
        match err {
            AppError::ForgeRateLimited(m) => assert!(m.contains("1700000000")),
            other => panic!("expected ForgeRateLimited, got {other:?}"),
        }
        assert!(matches!(
            map_status(&resp(429, vec![])),
            Some(AppError::ForgeRateLimited(_))
        ));
        assert!(matches!(
            map_status(&resp(404, vec![])),
            Some(AppError::ForgeApi(_))
        ));
        assert!(matches!(
            map_status(&resp(500, vec![])),
            Some(AppError::ForgeApi(_))
        ));
    }

    #[test]
    fn private_token_is_redacted_in_request_debug() {
        // The PRIVATE-TOKEN value must never surface through a `{:?}` of the
        // request — the http.rs redaction seam elides any header whose name
        // contains "token" (as well as `Authorization`).
        let req = HttpRequest {
            method: HttpMethod::Get,
            url: user_url("gitlab.com"),
            headers: base_headers(Some("glpat-SUPERSECRET")),
            body: None,
        };
        let dbg = format!("{req:?}");
        assert!(!dbg.contains("glpat-SUPERSECRET"), "token leaked: {dbg}");
        assert!(dbg.contains("<redacted>"), "expected redaction placeholder: {dbg}");
    }

    #[test]
    fn has_next_page_prefers_x_next_page_then_link_then_count() {
        // X-Next-Page present + non-empty ⇒ next.
        assert!(has_next_page(&resp(200, vec![("X-Next-Page", "3")]), 20, 20));
        // X-Next-Page present + empty ⇒ last page, even if the page is full.
        assert!(!has_next_page(&resp(200, vec![("X-Next-Page", "")]), 20, 20));
        // No X-Next-Page: a Link rel="next" ⇒ next.
        assert!(has_next_page(
            &resp(200, vec![("Link", "<https://x?page=2>; rel=\"next\"")]),
            20,
            20
        ));
        // No paging headers: fall back to a full page.
        assert!(has_next_page(&resp(200, vec![]), 20, 20));
        assert!(!has_next_page(&resp(200, vec![]), 20, 7));
        assert!(!has_next_page(&resp(200, vec![]), 0, 0));
    }
}
