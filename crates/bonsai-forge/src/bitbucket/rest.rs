//! Bitbucket Cloud REST 2.0 plumbing: endpoint URL builders, header assembly,
//! the HTTP-status→`AppError` mapping, and thin GET/POST helpers over an
//! injectable [`HttpTransport`].
//!
//! No provider policy lives here (that is `bitbucket/mod.rs`) and no Bitbucket
//! JSON is parsed here (that is `bitbucket/dto.rs`) — in particular pagination
//! is body-based (a `next` URL inside the JSON), so it is handled in `dto`, not
//! here. The auth header value is assembled but NEVER logged — the redaction
//! seam in `http.rs` elides `Authorization`.

use bonsai_core::error::AppError;

use crate::http::{HttpMethod, HttpRequest, HttpResponse, HttpTransport};
use crate::types::PrStateFilter;

/// Bitbucket Cloud REST base. The token NEVER appears in a URL — only in a header.
const API_BASE: &str = "https://api.bitbucket.org/2.0";

/// Assemble the standard Bitbucket headers.
///
/// Auth (OQ-A5): a Bitbucket workspace/repo/account **access token** is sent as
/// `Authorization: Bearer <token>` — the recommended scheme, which keeps the
/// single-secret keychain model. (The app-password fallback would base64-encode
/// `user:app_password` and send `Authorization: Basic <b64>`; we implement
/// Bearer.) Added ONLY when a token is present — read paths work anonymously for
/// public repos.
pub fn base_headers(token: Option<&str>) -> Vec<(String, String)> {
    let mut headers = vec![
        ("Accept".to_string(), "application/json".to_string()),
        ("User-Agent".to_string(), "Bonsai".to_string()),
    ];
    if let Some(t) = token {
        headers.push(("Authorization".to_string(), format!("Bearer {t}")));
    }
    headers
}

/// Bitbucket PR list filter → the set of `state` values emitted as **repeated**
/// query params. Bitbucket Cloud defaults to `state=OPEN` when the param is
/// omitted and has NO `state=all`, so `closed`/`all` must fan out to every
/// non-open state or MERGED PRs stay invisible (contract §3c, P64c
/// review-correction): `open→[OPEN]`, `closed→[MERGED, DECLINED, SUPERSEDED]`,
/// `all→[OPEN, MERGED, DECLINED, SUPERSEDED]`.
fn state_params(state: PrStateFilter) -> &'static [&'static str] {
    match state {
        PrStateFilter::Open => &["OPEN"],
        PrStateFilter::Closed => &["MERGED", "DECLINED", "SUPERSEDED"],
        PrStateFilter::All => &["OPEN", "MERGED", "DECLINED", "SUPERSEDED"],
    }
}

// ---- endpoint URL builders ----

pub fn user_url() -> String {
    format!("{API_BASE}/user")
}

pub fn pull_requests_url(
    workspace: &str,
    slug: &str,
    state: PrStateFilter,
    pagelen: u32,
    page: u32,
) -> String {
    let mut url =
        format!("{API_BASE}/repositories/{workspace}/{slug}/pullrequests?pagelen={pagelen}&page={page}");
    // Repeated `state` params (Bitbucket supports repetition); the token stays
    // in the Authorization header — this URL builder is tokenless.
    for s in state_params(state) {
        url.push_str("&state=");
        url.push_str(s);
    }
    url
}

pub fn pull_request_url(workspace: &str, slug: &str, id: u64) -> String {
    format!("{API_BASE}/repositories/{workspace}/{slug}/pullrequests/{id}")
}

pub fn create_pull_request_url(workspace: &str, slug: &str) -> String {
    format!("{API_BASE}/repositories/{workspace}/{slug}/pullrequests")
}

pub fn merge_pull_request_url(workspace: &str, slug: &str, id: u64) -> String {
    format!("{API_BASE}/repositories/{workspace}/{slug}/pullrequests/{id}/merge")
}

pub fn decline_pull_request_url(workspace: &str, slug: &str, id: u64) -> String {
    format!("{API_BASE}/repositories/{workspace}/{slug}/pullrequests/{id}/decline")
}

pub fn comments_url(workspace: &str, slug: &str, id: u64) -> String {
    format!("{API_BASE}/repositories/{workspace}/{slug}/pullrequests/{id}/comments?pagelen=100")
}

pub fn commit_statuses_url(workspace: &str, slug: &str, sha: &str) -> String {
    format!("{API_BASE}/repositories/{workspace}/{slug}/commit/{sha}/statuses?pagelen=100")
}

// ---- response inspection ----

/// Case-insensitive header lookup.
fn header<'a>(resp: &'a HttpResponse, name: &str) -> Option<&'a str> {
    resp.headers
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case(name))
        .map(|(_, v)| v.as_str())
}

/// A 429 rate-limit error, carrying the `Retry-After` hint when present.
fn rate_limited_error(resp: &HttpResponse) -> AppError {
    match header(resp, "retry-after") {
        Some(retry) => AppError::ForgeRateLimited(format!(
            "Bitbucket API rate limit exceeded (retry after {retry}s)"
        )),
        None => AppError::ForgeRateLimited("Bitbucket API rate limit exceeded".to_string()),
    }
}

/// Map a non-2xx response to an `AppError` (mirrors `gitlab/rest.rs`). `None` ⇒
/// success (2xx). Bitbucket signals rate limits with 429 (not an overloaded
/// 403), so 401/403 are always credential failures. NEVER includes a token.
pub fn map_status(resp: &HttpResponse) -> Option<AppError> {
    let s = resp.status;
    if (200..300).contains(&s) {
        return None;
    }
    Some(match s {
        401 => AppError::AuthFailed("Bitbucket rejected the credentials (401)".to_string()),
        403 => AppError::AuthFailed(
            "Bitbucket denied the request (403) — the token may be invalid or lack pull-request access"
                .to_string(),
        ),
        429 => rate_limited_error(resp),
        404 => AppError::ForgeApi("not found".to_string()),
        // Redirects are never followed (transport pins Policy::none).
        301 | 302 | 307 | 308 => AppError::ForgeApi(format!(
            "the repository has moved (HTTP {s}) — it may have been renamed; update the remote URL"
        )),
        other => AppError::ForgeApi(format!("Bitbucket API error (HTTP {other})")),
    })
}

// ---- transport helpers ----

/// GET `url` with Bitbucket headers; map a non-2xx status to an error.
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

/// Clear not-mergeable message for a Bitbucket merge Bitbucket refused because
/// the PR is not in a mergeable state (400 Bad Request / 409 Conflict).
pub fn not_mergeable_error() -> AppError {
    AppError::ForgeApi(
        "Bitbucket could not merge this PR — it is not mergeable (conflicts or unmet merge \
         checks)"
            .to_string(),
    )
}

/// POST `url` with a JSON `body` for a merge; 400/409 map to
/// [`not_mergeable_error`], otherwise standard status mapping. Callers requiring
/// auth check the token BEFORE calling this.
pub fn post_merge(
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
    if matches!(resp.status, 400 | 409) {
        return Err(not_mergeable_error());
    }
    match map_status(&resp) {
        Some(err) => Err(err),
        None => Ok(resp),
    }
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
    fn base_headers_use_bearer() {
        let none = base_headers(None);
        assert!(!none.iter().any(|(k, _)| k == "Authorization"));
        assert!(none.iter().any(|(k, v)| k == "Accept" && v == "application/json"));

        let auth = base_headers(Some("bb-token"));
        assert!(auth
            .iter()
            .any(|(k, v)| k == "Authorization" && v == "Bearer bb-token"));
        // Not the PRIVATE-TOKEN header — that's GitLab.
        assert!(!auth.iter().any(|(k, _)| k.eq_ignore_ascii_case("PRIVATE-TOKEN")));
    }

    #[test]
    fn state_params_fan_out_per_filter() {
        assert_eq!(state_params(PrStateFilter::Open), &["OPEN"]);
        // `closed`/`all` must include MERGED so merged PRs are visible.
        assert_eq!(
            state_params(PrStateFilter::Closed),
            &["MERGED", "DECLINED", "SUPERSEDED"]
        );
        assert_eq!(
            state_params(PrStateFilter::All),
            &["OPEN", "MERGED", "DECLINED", "SUPERSEDED"]
        );
    }

    #[test]
    fn pr_list_url_emits_repeated_state_params_per_filter() {
        // `open` ⇒ a single state param.
        let open = pull_requests_url("ws", "repo", PrStateFilter::Open, 30, 2);
        assert_eq!(
            open,
            "https://api.bitbucket.org/2.0/repositories/ws/repo/pullrequests?pagelen=30&page=2&state=OPEN"
        );

        // `closed` ⇒ repeated state params INCLUDING MERGED (was the bug).
        let closed = pull_requests_url("ws", "repo", PrStateFilter::Closed, 30, 1);
        assert_eq!(
            closed,
            "https://api.bitbucket.org/2.0/repositories/ws/repo/pullrequests?pagelen=30&page=1&state=MERGED&state=DECLINED&state=SUPERSEDED"
        );
        assert!(closed.contains("state=MERGED"), "closed must include MERGED");

        // `all` ⇒ every state fanned out (no `state=all` exists on Bitbucket).
        let all = pull_requests_url("ws", "repo", PrStateFilter::All, 30, 1);
        assert_eq!(
            all,
            "https://api.bitbucket.org/2.0/repositories/ws/repo/pullrequests?pagelen=30&page=1&state=OPEN&state=MERGED&state=DECLINED&state=SUPERSEDED"
        );
        assert!(all.contains("state=MERGED"), "all must include MERGED");
        assert!(!all.contains("state=all"), "Bitbucket has no state=all");
    }

    #[test]
    fn urls_embed_workspace_slug_and_no_token() {
        let open = pull_requests_url("ws", "repo", PrStateFilter::Open, 30, 2);
        assert!(!open.contains("bb-token"));
        let all = pull_requests_url("ws", "repo", PrStateFilter::All, 30, 1);
        assert!(!all.contains("bb-token"));

        assert_eq!(
            pull_request_url("ws", "repo", 5),
            "https://api.bitbucket.org/2.0/repositories/ws/repo/pullrequests/5"
        );
        assert_eq!(
            commit_statuses_url("ws", "repo", "abc"),
            "https://api.bitbucket.org/2.0/repositories/ws/repo/commit/abc/statuses?pagelen=100"
        );
    }

    #[test]
    fn map_status_taxonomy() {
        assert!(map_status(&resp(200, vec![])).is_none());
        assert!(map_status(&resp(201, vec![])).is_none());
        assert!(matches!(
            map_status(&resp(401, vec![])),
            Some(AppError::AuthFailed(_))
        ));
        assert!(matches!(
            map_status(&resp(403, vec![])),
            Some(AppError::AuthFailed(_))
        ));
        // 429 ⇒ rate limited, carrying the Retry-After hint when present.
        let err = map_status(&resp(429, vec![("Retry-After", "120")])).unwrap();
        match err {
            AppError::ForgeRateLimited(m) => assert!(m.contains("120")),
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
    fn bearer_token_is_redacted_in_request_debug() {
        // The Authorization value must never surface through a `{:?}` of the
        // request — the http.rs redaction seam elides `Authorization`.
        let req = HttpRequest {
            method: HttpMethod::Get,
            url: user_url(),
            headers: base_headers(Some("bb-SUPERSECRET")),
            body: None,
        };
        let dbg = format!("{req:?}");
        assert!(!dbg.contains("bb-SUPERSECRET"), "token leaked: {dbg}");
        assert!(dbg.contains("<redacted>"), "expected redaction placeholder: {dbg}");
    }
}
