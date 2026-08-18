//! GitHub REST v3 plumbing (§7): endpoint URL builders, header assembly, the
//! HTTP-status→`AppError` mapping, `Link`-header pagination, and thin
//! GET/POST helpers over an injectable [`HttpTransport`].
//!
//! No provider policy lives here (that is `github/mod.rs`) and no GitHub JSON
//! is parsed here (that is `github/dto.rs`). The `Authorization` header value
//! is assembled but NEVER logged — see `http::redact_header_value`.

use bonsai_core::error::AppError;

use crate::http::{HttpMethod, HttpRequest, HttpResponse, HttpTransport};
use crate::types::PrStateFilter;

/// GitHub REST base. The token NEVER appears in a URL — only in a header.
const API_BASE: &str = "https://api.github.com";

/// Assemble the standard GitHub headers. `Authorization` is added ONLY when a
/// token is present (read paths work anonymously for public repos).
pub fn base_headers(token: Option<&str>) -> Vec<(String, String)> {
    let mut headers = vec![
        ("Accept".to_string(), "application/vnd.github+json".to_string()),
        ("X-GitHub-Api-Version".to_string(), "2022-11-28".to_string()),
        ("User-Agent".to_string(), "Bonsai".to_string()),
    ];
    if let Some(t) = token {
        headers.push(("Authorization".to_string(), format!("Bearer {t}")));
    }
    headers
}

fn state_param(state: PrStateFilter) -> &'static str {
    match state {
        PrStateFilter::Open => "open",
        PrStateFilter::Closed => "closed",
        PrStateFilter::All => "all",
    }
}

// ---- endpoint URL builders ----

pub fn user_url() -> String {
    format!("{API_BASE}/user")
}

pub fn pulls_url(
    owner: &str,
    repo: &str,
    state: PrStateFilter,
    per_page: u32,
    page: u32,
) -> String {
    format!(
        "{API_BASE}/repos/{owner}/{repo}/pulls?state={}&per_page={per_page}&page={page}",
        state_param(state)
    )
}

pub fn pull_url(owner: &str, repo: &str, number: u64) -> String {
    format!("{API_BASE}/repos/{owner}/{repo}/pulls/{number}")
}

pub fn create_pull_url(owner: &str, repo: &str) -> String {
    format!("{API_BASE}/repos/{owner}/{repo}/pulls")
}

pub fn review_comments_url(owner: &str, repo: &str, number: u64) -> String {
    format!("{API_BASE}/repos/{owner}/{repo}/pulls/{number}/comments")
}

pub fn issue_comments_url(owner: &str, repo: &str, number: u64) -> String {
    format!("{API_BASE}/repos/{owner}/{repo}/issues/{number}/comments")
}

pub fn combined_status_url(owner: &str, repo: &str, sha: &str) -> String {
    format!("{API_BASE}/repos/{owner}/{repo}/commits/{sha}/status")
}

pub fn check_runs_url(owner: &str, repo: &str, sha: &str) -> String {
    format!("{API_BASE}/repos/{owner}/{repo}/commits/{sha}/check-runs")
}

// ---- response inspection ----

/// Case-insensitive header lookup.
fn header<'a>(resp: &'a HttpResponse, name: &str) -> Option<&'a str> {
    resp.headers
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case(name))
        .map(|(_, v)| v.as_str())
}

/// Whether the `Link` header advertises a `rel="next"` page.
pub fn has_next_link(resp: &HttpResponse) -> bool {
    header(resp, "link")
        .map(|l| l.contains("rel=\"next\""))
        .unwrap_or(false)
}

/// A 403 that is actually a rate-limit (remaining == 0), distinct from bad
/// credentials.
fn is_rate_limited(resp: &HttpResponse) -> bool {
    header(resp, "x-ratelimit-remaining").map(|v| v.trim() == "0") == Some(true)
}

fn rate_limited_error(resp: &HttpResponse) -> AppError {
    match header(resp, "x-ratelimit-reset") {
        Some(reset) => AppError::ForgeRateLimited(format!(
            "GitHub API rate limit exceeded (resets at epoch {reset})"
        )),
        None => AppError::ForgeRateLimited("GitHub API rate limit exceeded".to_string()),
    }
}

/// Map a non-2xx response to an `AppError` (§7). `None` ⇒ success (2xx). NEVER
/// includes a token or the `Authorization` header.
pub fn map_status(resp: &HttpResponse) -> Option<AppError> {
    let s = resp.status;
    if (200..300).contains(&s) {
        return None;
    }
    Some(match s {
        401 => AppError::AuthFailed("GitHub rejected the credentials (401)".to_string()),
        403 => {
            if is_rate_limited(resp) {
                rate_limited_error(resp)
            } else {
                AppError::AuthFailed(
                    "GitHub denied the request (403) — the token may be invalid or lack scope"
                        .to_string(),
                )
            }
        }
        429 => rate_limited_error(resp),
        404 => AppError::ForgeApi("not found".to_string()),
        // Redirects are never followed (the transport pins Policy::none so a
        // same-host https->http hop can't re-send the token). GitHub answers
        // 301 on /repos/... after a rename — say so instead of a bare code.
        301 | 302 | 307 | 308 => AppError::ForgeApi(format!(
            "the repository has moved (HTTP {s}) — it may have been renamed; update the remote URL"
        )),
        other => AppError::ForgeApi(format!("GitHub API error (HTTP {other})")),
    })
}

// ---- transport helpers ----

/// GET `url` with GitHub headers; map a non-2xx status to an error.
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
    fn base_headers_omit_auth_without_token() {
        let h = base_headers(None);
        assert!(!h.iter().any(|(k, _)| k == "Authorization"));
        assert!(h.iter().any(|(k, v)| k == "Accept" && v == "application/vnd.github+json"));
        assert!(h.iter().any(|(k, v)| k == "User-Agent" && v == "Bonsai"));
    }

    #[test]
    fn base_headers_add_bearer_with_token() {
        let h = base_headers(Some("tok"));
        assert!(h
            .iter()
            .any(|(k, v)| k == "Authorization" && v == "Bearer tok"));
    }

    #[test]
    fn urls_never_embed_a_token() {
        let u = pulls_url("o", "r", PrStateFilter::All, 30, 2);
        assert_eq!(
            u,
            "https://api.github.com/repos/o/r/pulls?state=all&per_page=30&page=2"
        );
        assert!(!u.contains("tok"));
        assert_eq!(
            pull_url("o", "r", 5),
            "https://api.github.com/repos/o/r/pulls/5"
        );
        assert_eq!(
            check_runs_url("o", "r", "abc"),
            "https://api.github.com/repos/o/r/commits/abc/check-runs"
        );
    }

    #[test]
    fn map_status_success_is_none() {
        assert!(map_status(&resp(200, vec![])).is_none());
        assert!(map_status(&resp(201, vec![])).is_none());
    }

    #[test]
    fn map_status_401_is_auth_failed() {
        assert!(matches!(
            map_status(&resp(401, vec![])),
            Some(AppError::AuthFailed(_))
        ));
    }

    #[test]
    fn map_status_403_bad_creds_vs_rate_limit() {
        // 403 with remaining>0 ⇒ auth failure.
        assert!(matches!(
            map_status(&resp(403, vec![("X-RateLimit-Remaining", "57")])),
            Some(AppError::AuthFailed(_))
        ));
        // 403 with remaining==0 ⇒ rate limited, carrying the reset hint.
        let err = map_status(&resp(
            403,
            vec![("X-RateLimit-Remaining", "0"), ("X-RateLimit-Reset", "1700000000")],
        ))
        .unwrap();
        match err {
            AppError::ForgeRateLimited(m) => assert!(m.contains("1700000000")),
            other => panic!("expected ForgeRateLimited, got {other:?}"),
        }
    }

    #[test]
    fn map_status_429_is_rate_limited() {
        assert!(matches!(
            map_status(&resp(429, vec![])),
            Some(AppError::ForgeRateLimited(_))
        ));
    }

    #[test]
    fn map_status_404_and_other_are_forge_api() {
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
    fn has_next_link_detects_next_rel() {
        let with_next = resp(
            200,
            vec![(
                "Link",
                "<https://api.github.com/repositories/1/pulls?page=2>; rel=\"next\", <...>; rel=\"last\"",
            )],
        );
        assert!(has_next_link(&with_next));
        let last_only = resp(200, vec![("Link", "<...>; rel=\"prev\"")]);
        assert!(!has_next_link(&last_only));
        assert!(!has_next_link(&resp(200, vec![])));
    }
}
