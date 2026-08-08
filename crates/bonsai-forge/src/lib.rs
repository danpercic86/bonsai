//! Bonsai forge integration (P62): detect the forge from `origin`, authenticate
//! with a keychain-stored PAT, and drive PR list/detail/create + CI status
//! through a provider-neutral [`ForgeProvider`] trait. GitHub REST v3 is the
//! first (v1: only) implementation.
//!
//! Pure library — NO Tauri, NO async runtime. Every provider method is blocking;
//! the command layer wraps calls in `spawn_blocking`. All HTTP goes through the
//! injectable [`HttpTransport`] seam so the provider is unit-tested offline.
//!
//! Security spine (overview §F3): the PAT is pasted by the user, stored ONLY in
//! the OS keychain (never settings.json), reaches the wire ONLY as an
//! `Authorization: Bearer` header, and is NEVER logged or placed in a URL.

pub mod auth;
pub mod detect;
pub mod http;
pub mod provider;
pub mod types;

mod github;
mod gitlab;
mod rollup;

use std::path::Path;

use bonsai_core::error::AppError;

pub use detect::{detect_provider, ForgeTarget};
pub use http::{HttpMethod, HttpRequest, HttpResponse, HttpTransport, ReqwestTransport};
pub use provider::ForgeProvider;
pub use types::*;

use crate::github::GitHubProvider;
use crate::gitlab::GitLabProvider;

/// Construct the concrete [`ForgeProvider`] for `target` over `http`. Shared by
/// [`open`] and [`validate_token`] so both resolve the SAME provider for a host.
///
/// GitLab hosts get [`GitLabProvider`]; GitHub AND unparseable/unknown origins
/// both go through [`GitHubProvider`] (an `Unknown` target yields a friendly
/// `repo_context` but `ForgeUnsupported` on any data method — unchanged P62
/// behavior). Adding a provider = one arm here + one `detect` host mapping.
fn build_provider(
    target: ForgeTarget,
    token: Option<String>,
    http: Box<dyn HttpTransport>,
) -> Box<dyn ForgeProvider> {
    match target.kind {
        ForgeKind::GitLab => Box::new(GitLabProvider::new(target, token, http)),
        _ => Box::new(GitHubProvider::new(target, token, http)),
    }
}

/// Resolve the [`ForgeTarget`] for the repo at `workdir` from its `origin`
/// remote. An unparseable origin yields an `Unknown`-kind target with empty
/// fields (friendly `repo_context`; data methods reject); no `origin` remote at
/// all ⇒ [`AppError::NoRemote`]. Shared by [`open`], [`set_token`], and
/// [`clear_token`] so all three resolve identity identically.
fn resolve_target(workdir: &Path) -> Result<ForgeTarget, AppError> {
    let remotes = bonsai_core::git::remote::list_remotes(workdir)?;
    let origin = remotes
        .into_iter()
        .find(|r| r.name == "origin")
        .ok_or_else(|| AppError::NoRemote("no 'origin' remote is configured".to_string()))?;
    let url = origin
        .url
        .ok_or_else(|| AppError::NoRemote("the 'origin' remote has no fetch URL".to_string()))?;

    // Unparseable origin ⇒ an empty Unknown target (friendly context; data
    // methods reject with ForgeUnsupported).
    Ok(detect_provider(&url).unwrap_or_else(|| ForgeTarget {
        kind: ForgeKind::Unknown,
        host: String::new(),
        owner: String::new(),
        repo: String::new(),
        web_url: String::new(),
    }))
}

/// Open a forge provider for the repo at `workdir`.
///
/// Reads the `origin` remote, detects the provider, looks up a stored token
/// (keychain, no network), and returns a boxed [`ForgeProvider`] over a real
/// [`ReqwestTransport`]. An origin that is not a recognized `owner/repo` URL
/// yields an `Unknown`-kind provider: `repo_context()` returns a friendly
/// identity, but any DATA method returns [`AppError::ForgeUnsupported`]. No
/// `origin` remote at all ⇒ [`AppError::NoRemote`].
///
/// Tests build providers directly with a fake `HttpTransport` + explicit token,
/// bypassing this function.
pub fn open(workdir: &Path) -> Result<Box<dyn ForgeProvider>, AppError> {
    let target = resolve_target(workdir)?;

    // Presence-only token lookup (never network). A keychain read error
    // degrades to unauthenticated rather than failing open().
    let token = if target.host.is_empty() {
        None
    } else {
        auth::global().get(&target.host).unwrap_or(None)
    };

    let transport = ReqwestTransport::new()?;
    Ok(build_provider(target, token, Box::new(transport)))
}

/// Validate a candidate `token` against `target` using `http`, returning the
/// authenticated viewer on success. Stores NOTHING — the caller persists only
/// after this returns `Ok`. Split out from [`set_token`] so the validate path
/// is unit-tested with a fake transport (no network, no keychain). Rejects
/// [`AppError::ForgeUnsupported`] for an unsupported origin and
/// [`AppError::AuthFailed`] for a token the forge rejects.
///
/// Provider-aware (OQ-A4): each forge hits its OWN identity endpoint through its
/// OWN auth header — [`build_provider`] picks the right one for `target.kind`.
pub(crate) fn validate_token(
    target: ForgeTarget,
    token: &str,
    http: Box<dyn HttpTransport>,
) -> Result<ForgeViewer, AppError> {
    // `viewer()` performs the single identity (`GET /user`) validation call and,
    // on success, warms the process viewer cache for the host.
    build_provider(target, Some(token.to_string()), http).viewer()
}

/// Validate a pasted PAT for the repo at `workdir` and, on success, persist it
/// in the OS keychain keyed by host (P62b auth plumbing — the read-only
/// [`open`] cannot store).
///
/// Flow: resolve the target from `origin` → build a provider with the CANDIDATE
/// token over a real [`ReqwestTransport`] → `viewer()` (`GET /user`) to
/// validate → on success `TokenStore::set(host, token)` and return the viewer
/// (already cached by `viewer()`). A rejected token ⇒ [`AppError::AuthFailed`]
/// and NOTHING is stored; a non-GitHub origin ⇒ [`AppError::ForgeUnsupported`].
/// The token is NEVER logged or placed in a URL (the transport redacts it).
pub fn set_token(workdir: &Path, token: &str) -> Result<ForgeViewer, AppError> {
    let target = resolve_target(workdir)?;
    let host = target.host.clone();
    let transport = ReqwestTransport::new()?;
    let viewer = validate_token(target, token, Box::new(transport))?;
    // Persist ONLY after successful validation (never store a rejected token).
    if !host.is_empty() {
        auth::global().set(&host, token)?;
    }
    Ok(viewer)
}

/// Sign out the forge account for the repo at `workdir`: delete the host's PAT
/// from the keychain and evict the cached viewer (P62b). Idempotent — clearing
/// when nothing is stored is `Ok(())`, and an unparseable origin (empty host)
/// has no keychain entry, so it is a no-op success. No `origin` remote ⇒
/// [`AppError::NoRemote`].
pub fn clear_token(workdir: &Path) -> Result<(), AppError> {
    let target = resolve_target(workdir)?;
    if !target.host.is_empty() {
        auth::global().delete(&target.host)?;
        auth::evict_viewer(&target.host);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A canned transport returning one fixed status+body for every request.
    /// Zero network — exercises the validate path offline, never the keychain.
    struct CannedTransport {
        status: u16,
        body: &'static str,
    }

    impl HttpTransport for CannedTransport {
        fn send(&self, _req: &HttpRequest) -> Result<HttpResponse, AppError> {
            Ok(HttpResponse {
                status: self.status,
                headers: vec![],
                body: self.body.to_string(),
            })
        }
    }

    fn github_target() -> ForgeTarget {
        ForgeTarget {
            kind: ForgeKind::GitHub,
            host: "github.com".to_string(),
            owner: "o".to_string(),
            repo: "r".to_string(),
            web_url: "https://github.com/o/r".to_string(),
        }
    }

    /// A good token ⇒ the parsed viewer (from `GET /user`); no keychain touched.
    #[test]
    fn validate_token_good_returns_viewer() {
        let http = CannedTransport {
            status: 200,
            body: r#"{ "login": "octocat", "avatar_url": "https://a/o.png" }"#,
        };
        let viewer = validate_token(github_target(), "good-tok", Box::new(http)).unwrap();
        assert_eq!(viewer.login, "octocat");
        assert_eq!(viewer.avatar_url.as_deref(), Some("https://a/o.png"));
    }

    /// A rejected token ⇒ `AuthFailed` (401); the caller stores nothing.
    #[test]
    fn validate_token_bad_is_auth_failed() {
        let http = CannedTransport {
            status: 401,
            body: "{}",
        };
        let err = validate_token(github_target(), "bad-tok", Box::new(http)).unwrap_err();
        assert!(matches!(err, AppError::AuthFailed(_)), "got {err:?}");
    }

    /// A non-GitHub origin ⇒ `ForgeUnsupported` BEFORE any request.
    #[test]
    fn validate_token_unsupported_origin() {
        let target = ForgeTarget {
            kind: ForgeKind::Unknown,
            host: "gitlab.example.com".to_string(),
            owner: "o".to_string(),
            repo: "r".to_string(),
            web_url: "https://gitlab.example.com/o/r".to_string(),
        };
        let http = CannedTransport {
            status: 200,
            body: "{}",
        };
        let err = validate_token(target, "tok", Box::new(http)).unwrap_err();
        assert!(matches!(err, AppError::ForgeUnsupported(_)), "got {err:?}");
    }
}
