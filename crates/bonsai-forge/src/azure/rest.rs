//! Azure DevOps REST 7.1 plumbing: endpoint URL builders, Basic-auth header
//! assembly, the HTTP-status→`AppError` mapping, and thin GET/POST helpers over
//! an injectable [`HttpTransport`].
//!
//! No provider policy lives here (that is `azure/mod.rs`) and no Azure JSON is
//! parsed here (that is `azure/dto.rs`). Two Azure specifics are enforced here:
//!   * EVERY request URL carries `api-version=7.1` and NEVER a token (the PAT
//!     lives only in the `Authorization` header).
//!   * Auth is `Authorization: Basic base64(":" + <PAT>)` — an EMPTY username
//!     with a colon prefix (contract §3c). The base64 hides the PAT and the
//!     `http.rs` redaction seam elides the whole `Authorization` value, so the
//!     PAT never reaches a log, `{:?}`, URL, or error.

use bonsai_core::error::AppError;

use crate::http::{HttpMethod, HttpRequest, HttpResponse, HttpTransport};
use crate::types::PrStateFilter;

/// Azure DevOps org-scoped REST base. Repo endpoints hang off
/// `{ORG_BASE}/{org}/{project}/_apis/git/repositories/{repo}`.
const ORG_BASE: &str = "https://dev.azure.com";

/// The REST API version pinned on every request (contract §3c).
const API_VERSION: &str = "api-version=7.1";

/// The cross-host identity endpoint (contract §3c): a DIFFERENT host from the
/// repo API, reached with the SAME Basic auth header.
const PROFILE_URL: &str =
    "https://app.vssps.visualstudio.com/_apis/profile/profiles/me?api-version=7.1";

/// Assemble the standard Azure DevOps headers. Azure authenticates a PAT as
/// `Authorization: Basic base64(":" + <PAT>)` — an empty username, colon prefix
/// (contract §3c) — added ONLY when a token is present (read paths work
/// anonymously for public repos). The base64 encoding + the `http.rs` redaction
/// seam together keep the PAT off every wire/log surface.
pub fn base_headers(token: Option<&str>) -> Vec<(String, String)> {
    let mut headers = vec![
        ("Accept".to_string(), "application/json".to_string()),
        ("User-Agent".to_string(), "Bonsai".to_string()),
    ];
    if let Some(t) = token {
        let encoded = base64_encode(format!(":{t}").as_bytes());
        headers.push(("Authorization".to_string(), format!("Basic {encoded}")));
    }
    headers
}

/// Standard base64 (RFC 4648, with `=` padding). Hand-rolled to avoid a crate
/// dependency (mirrors GitLab's hand-rolled percent-encoder). Used ONLY to
/// encode `":" + PAT` for the Basic auth header.
fn base64_encode(input: &[u8]) -> String {
    const ALPHABET: &[u8; 64] =
        b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(input.len().div_ceil(3) * 4);
    for chunk in input.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = *chunk.get(1).unwrap_or(&0) as u32;
        let b2 = *chunk.get(2).unwrap_or(&0) as u32;
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(ALPHABET[((n >> 18) & 63) as usize] as char);
        out.push(ALPHABET[((n >> 12) & 63) as usize] as char);
        out.push(if chunk.len() > 1 {
            ALPHABET[((n >> 6) & 63) as usize] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            ALPHABET[(n & 63) as usize] as char
        } else {
            '='
        });
    }
    out
}

/// Azure PR list filter → `searchCriteria.status` value: `open→active`,
/// `closed→abandoned`, `all→all` (Azure natively supports `all`, contract §3c).
fn status_param(state: PrStateFilter) -> &'static str {
    match state {
        PrStateFilter::Open => "active",
        PrStateFilter::Closed => "abandoned",
        PrStateFilter::All => "all",
    }
}

/// The repo-scoped API base for `org`/`project`/`repo`. The token is NEVER here.
fn repo_base(org: &str, project: &str, repo: &str) -> String {
    format!("{ORG_BASE}/{org}/{project}/_apis/git/repositories/{repo}")
}

// ---- endpoint URL builders (each carries api-version, never a token) ----

/// The cross-host identity endpoint (`viewer()`'s BEST-EFFORT identify step).
/// Gated on the PAT's `vso.profile` scope, which the Code (Read & Write) PAT the
/// UI asks for does NOT carry — so this URL must never gate a connect (P72 §A4).
pub fn profile_url() -> String {
    PROFILE_URL.to_string()
}

/// The repository object itself — the SCOPE-VALIDATION probe for `viewer()`.
/// Reaching it requires exactly the Code scope every other Azure call already
/// needs (`vso.code`, inherited by `vso.code_write`), so a Code-only PAT
/// validates; a 404 additionally means the org/project/repo triple is wrong.
/// Built from `repo_base` so it cannot drift from the other repo endpoints.
pub fn repository_url(org: &str, project: &str, repo: &str) -> String {
    format!("{}?{API_VERSION}", repo_base(org, project, repo))
}

pub fn pull_requests_url(
    org: &str,
    project: &str,
    repo: &str,
    state: PrStateFilter,
    top: u32,
    skip: u32,
) -> String {
    format!(
        "{}/pullrequests?searchCriteria.status={}&$top={top}&$skip={skip}&{API_VERSION}",
        repo_base(org, project, repo),
        status_param(state),
    )
}

pub fn pull_request_url(org: &str, project: &str, repo: &str, id: u64) -> String {
    format!(
        "{}/pullrequests/{id}?{API_VERSION}",
        repo_base(org, project, repo)
    )
}

pub fn create_pull_request_url(org: &str, project: &str, repo: &str) -> String {
    format!(
        "{}/pullrequests?{API_VERSION}",
        repo_base(org, project, repo)
    )
}

pub fn threads_url(org: &str, project: &str, repo: &str, id: u64) -> String {
    format!(
        "{}/pullrequests/{id}/threads?{API_VERSION}",
        repo_base(org, project, repo)
    )
}

/// Commit-status endpoint (contract §3c: `combined_status` keys off a sha, so it
/// uses the per-commit statuses, not per-PR statuses).
pub fn commit_statuses_url(org: &str, project: &str, repo: &str, sha: &str) -> String {
    format!(
        "{}/commits/{sha}/statuses?{API_VERSION}",
        repo_base(org, project, repo)
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

/// A 429 rate-limit error, carrying the `Retry-After` hint when present.
fn rate_limited_error(resp: &HttpResponse) -> AppError {
    match header(resp, "retry-after") {
        Some(retry) => AppError::ForgeRateLimited(format!(
            "Azure DevOps API rate limit exceeded (retry after {retry}s)"
        )),
        None => AppError::ForgeRateLimited("Azure DevOps API rate limit exceeded".to_string()),
    }
}

/// Map a non-2xx response to an `AppError` (mirrors `bitbucket/rest.rs`). `None`
/// ⇒ success (2xx). NEVER includes a token or the auth header.
pub fn map_status(resp: &HttpResponse) -> Option<AppError> {
    let s = resp.status;
    // FIRST — before the 2xx success check (P72 §A3). Azure answers `203
    // Non-Authoritative Information` plus an HTML sign-in page for an
    // invalid/expired PAT; landing that in the success branch made the HTML
    // reach `dto::from_json` and surface as "malformed response".
    if s == 203 {
        return Some(AppError::AuthFailed(
            "Azure DevOps did not accept the personal access token (HTTP 203 sign-in page) — it is invalid or expired; create a new PAT with Code (Read & Write)"
                .to_string(),
        ));
    }
    if (200..300).contains(&s) {
        return None;
    }
    Some(match s {
        401 => AppError::AuthFailed(
            "Azure DevOps rejected the personal access token (401) — it is invalid or expired, or it lacks Code (Read & Write) for this repository"
                .to_string(),
        ),
        403 => AppError::AuthFailed(
            "Azure DevOps denied the request (403) — the PAT may be invalid or lack Code (Read & Write)"
                .to_string(),
        ),
        429 => rate_limited_error(resp),
        404 => AppError::ForgeApi("not found".to_string()),
        // Redirects are never followed (transport pins Policy::none).
        301 | 302 | 307 | 308 => AppError::ForgeApi(format!(
            "the repository has moved (HTTP {s}) — it may have been renamed; update the remote URL"
        )),
        other => AppError::ForgeApi(format!("Azure DevOps API error (HTTP {other})")),
    })
}

// ---- transport helpers ----

/// GET `url` with Azure headers; map a non-2xx status to an error.
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
    fn base64_matches_known_vectors() {
        // Standard RFC 4648 vectors, incl. the colon-prefix the auth uses.
        assert_eq!(base64_encode(b""), "");
        assert_eq!(base64_encode(b":"), "Og==");
        assert_eq!(base64_encode(b"pat"), "cGF0");
        assert_eq!(base64_encode(b"Man"), "TWFu");
        assert_eq!(base64_encode(b"Ma"), "TWE=");
        assert_eq!(base64_encode(b"M"), "TQ==");
    }

    #[test]
    fn base_headers_use_basic_colon_prefixed_pat() {
        let none = base_headers(None);
        assert!(!none.iter().any(|(k, _)| k == "Authorization"));
        assert!(none.iter().any(|(k, v)| k == "Accept" && v == "application/json"));

        // Basic base64(":" + PAT) — empty username, colon prefix.
        let auth = base_headers(Some("pat"));
        let expected = format!("Basic {}", base64_encode(b":pat"));
        assert!(auth
            .iter()
            .any(|(k, v)| k == "Authorization" && *v == expected));
        // Hard-coded pin (P72 §3.1 n): `expected` above is computed with the very
        // `base64_encode` under test, so a bug in the encoder would pass. These
        // are the literal header bytes Azure must receive for the PAT "pat"
        // (`":pat"` ⇒ `OnBhdA==`).
        assert!(auth.iter().any(|(k, v)| k == "Authorization" && v == "Basic OnBhdA=="));

        // NOT Bearer (Bitbucket) and NOT PRIVATE-TOKEN (GitLab).
        assert!(!auth
            .iter()
            .any(|(_, v)| v.starts_with("Bearer")));
        assert!(!auth.iter().any(|(k, _)| k.eq_ignore_ascii_case("PRIVATE-TOKEN")));
    }

    #[test]
    fn status_param_maps_filter() {
        assert_eq!(status_param(PrStateFilter::Open), "active");
        assert_eq!(status_param(PrStateFilter::Closed), "abandoned");
        assert_eq!(status_param(PrStateFilter::All), "all");
    }

    #[test]
    fn urls_carry_api_version_coords_and_no_token() {
        let list = pull_requests_url("org", "proj", "repo", PrStateFilter::Open, 30, 60);
        assert_eq!(
            list,
            "https://dev.azure.com/org/proj/_apis/git/repositories/repo/pullrequests?searchCriteria.status=active&$top=30&$skip=60&api-version=7.1"
        );
        assert!(!list.contains("SECRET"));

        assert_eq!(
            pull_request_url("org", "proj", "repo", 5),
            "https://dev.azure.com/org/proj/_apis/git/repositories/repo/pullrequests/5?api-version=7.1"
        );
        assert_eq!(
            threads_url("org", "proj", "repo", 5),
            "https://dev.azure.com/org/proj/_apis/git/repositories/repo/pullrequests/5/threads?api-version=7.1"
        );
        assert_eq!(
            commit_statuses_url("org", "proj", "repo", "abc"),
            "https://dev.azure.com/org/proj/_apis/git/repositories/repo/commits/abc/statuses?api-version=7.1"
        );
        // The scope-validation probe: the repository object itself (P72 §A2).
        let probe = repository_url("org", "proj", "repo");
        assert_eq!(
            probe,
            "https://dev.azure.com/org/proj/_apis/git/repositories/repo?api-version=7.1"
        );
        assert!(probe.contains("api-version=7.1"));
        assert!(!probe.contains("SECRET"));

        // The identity endpoint is a DIFFERENT host, still api-versioned.
        assert!(profile_url().starts_with("https://app.vssps.visualstudio.com/"));
        assert!(profile_url().contains("api-version=7.1"));
        // EVERY repo builder pins the api version (list asserted exactly above).
        assert!(list.contains("api-version=7.1"), "missing api-version: {list}");
    }

    #[test]
    fn map_status_taxonomy() {
        assert!(map_status(&resp(200, vec![])).is_none());
        assert!(map_status(&resp(201, vec![])).is_none());
        // 203 is a 2xx but means "here is a sign-in page" ⇒ an AUTH failure, and
        // it must be caught BEFORE the success early-return (P72 §A3).
        match map_status(&resp(203, vec![])) {
            Some(AppError::AuthFailed(m)) => {
                assert!(m.contains("203"), "message: {m}");
                assert!(m.contains("invalid or expired"), "message: {m}");
                assert!(m.contains("Code (Read & Write)"), "message: {m}");
            }
            other => panic!("expected AuthFailed for 203, got {other:?}"),
        }
        match map_status(&resp(401, vec![])) {
            Some(AppError::AuthFailed(m)) => {
                assert!(m.contains("invalid or expired"), "message: {m}");
                assert!(m.contains("Code (Read & Write)"), "message: {m}");
            }
            other => panic!("expected AuthFailed for 401, got {other:?}"),
        }
        assert!(matches!(
            map_status(&resp(403, vec![])),
            Some(AppError::AuthFailed(_))
        ));
        let err = map_status(&resp(429, vec![("Retry-After", "90")])).unwrap();
        match err {
            AppError::ForgeRateLimited(m) => assert!(m.contains("90")),
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
    fn pat_is_redacted_in_request_debug_and_absent_from_wire() {
        // The PAT is base64-encoded into the Basic value, and the http.rs
        // redaction seam elides the whole `Authorization` value — so neither the
        // plaintext PAT nor its base64 encoding can surface through `{:?}`.
        let pat = "az-SUPERSECRET";
        let req = HttpRequest {
            method: HttpMethod::Get,
            url: profile_url(),
            headers: base_headers(Some(pat)),
            body: None,
        };
        let dbg = format!("{req:?}");
        assert!(!dbg.contains(pat), "plaintext PAT leaked: {dbg}");
        assert!(
            !dbg.contains(&base64_encode(format!(":{pat}").as_bytes())),
            "base64 PAT leaked: {dbg}"
        );
        assert!(dbg.contains("<redacted>"), "expected redaction placeholder: {dbg}");

        // And the PAT is never placed in any URL the provider builds.
        let url = pull_requests_url("org", "proj", "repo", PrStateFilter::All, 30, 0);
        assert!(!url.contains(pat), "PAT leaked into URL: {url}");
    }
}
