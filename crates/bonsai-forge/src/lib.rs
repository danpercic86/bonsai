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
//! the OS keychain (never settings.json), reaches the wire ONLY as that
//! provider's auth header — GitHub and Bitbucket `Authorization: Bearer <token>`,
//! Azure DevOps `Authorization: Basic base64(":" + PAT)`, GitLab
//! `PRIVATE-TOKEN: <token>` — and is NEVER logged or placed in a URL (the
//! `http.rs` redaction seam elides the header value; base64 is encoding, not
//! secrecy).

pub mod auth;
pub mod detect;
pub mod http;
pub mod provider;
pub mod types;

mod azure;
mod bitbucket;
mod github;
mod gitlab;
mod ratelimit;
mod rollup;

use std::path::Path;

use bonsai_core::error::AppError;

pub use detect::{detect_provider, ForgeTarget};
pub use http::{HttpMethod, HttpRequest, HttpResponse, HttpTransport, ReqwestTransport};
pub use provider::ForgeProvider;
pub use types::*;

use crate::azure::AzureDevOpsProvider;
use crate::bitbucket::BitbucketProvider;
use crate::github::GitHubProvider;
use crate::gitlab::GitLabProvider;

/// Construct the concrete [`ForgeProvider`] for `target` over `http`. Shared by
/// [`open_with_key`] and [`validate_token`] so both resolve the SAME provider
/// for a host.
///
/// GitLab hosts get [`GitLabProvider`]; Bitbucket hosts get
/// [`BitbucketProvider`]; Azure DevOps hosts get [`AzureDevOpsProvider`]; GitHub
/// AND unparseable/unknown origins both go through [`GitHubProvider`] (an
/// `Unknown` target yields a friendly `repo_context` but `ForgeUnsupported` on
/// any data method — unchanged P62 behavior). Adding a provider = one arm here +
/// one `detect` host mapping.
fn build_provider(
    target: ForgeTarget,
    token: Option<String>,
    http: Box<dyn HttpTransport>,
) -> Box<dyn ForgeProvider> {
    match target.kind {
        ForgeKind::GitLab => Box::new(GitLabProvider::new(target, token, http)),
        ForgeKind::Bitbucket => Box::new(BitbucketProvider::new(target, token, http)),
        ForgeKind::AzureDevOps => Box::new(AzureDevOpsProvider::new(target, token, http)),
        _ => Box::new(GitHubProvider::new(target, token, http)),
    }
}

/// Resolve the [`ForgeTarget`] for the repo at `workdir` from its `origin`
/// remote. An unparseable origin yields an `Unknown`-kind target with empty
/// fields (friendly `repo_context`; data methods reject); no `origin` remote at
/// all ⇒ [`AppError::NoRemote`]. Shared by [`open_with_key`] and the validate
/// entry points so every path resolves identity identically.
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
        project: None,
        web_url: String::new(),
    }))
}

/// P80: open a forge provider for the repo at `workdir` using an EXPLICIT
/// keychain key (resolved by the command layer's `resolve_account`), rather than
/// the bare host. `keychain_key = None` (or empty) ⇒ an unauthenticated
/// provider. A keychain read error degrades to unauthenticated rather than
/// failing. No `origin` remote ⇒ [`AppError::NoRemote`].
pub fn open_with_key(
    workdir: &Path,
    keychain_key: Option<&str>,
) -> Result<Box<dyn ForgeProvider>, AppError> {
    let target = resolve_target(workdir)?;
    let token = match keychain_key {
        Some(k) if !k.is_empty() => auth::global().get(k).unwrap_or(None),
        _ => None,
    };
    let transport = ReqwestTransport::new()?;
    Ok(build_provider(target, token, Box::new(transport)))
}

/// P80: validate a pasted PAT for the repo at `workdir` WITHOUT storing it.
/// Returns the authenticated viewer plus the resolved `(host, kind)` so the
/// command layer can compute the three-part `accountId` / keychain key and store
/// the token itself (via [`store_token`]). A rejected token ⇒
/// [`AppError::AuthFailed`]; a non-forge origin ⇒ [`AppError::ForgeUnsupported`].
pub fn validate_repo_token(
    workdir: &Path,
    token: &str,
) -> Result<(ForgeViewer, String, ForgeKind), AppError> {
    let target = resolve_target(workdir)?;
    let host = target.host.clone();
    let kind = target.kind;
    let transport = ReqwestTransport::new()?;
    let viewer = validate_token(target, token, Box::new(transport))?;
    Ok((viewer, host, kind))
}

/// P80: validate a PAT against `host`/`kind` DIRECTLY (no repo) WITHOUT storing
/// it. Azure DevOps has no repo-less identity endpoint ⇒
/// [`AppError::ForgeUnsupported`] (OD-6). A rejected token ⇒
/// [`AppError::AuthFailed`]. The command layer persists under a resolved key.
pub fn validate_host_token(
    host: &str,
    kind: ForgeKind,
    token: &str,
) -> Result<ForgeViewer, AppError> {
    validate_host_token_with(host, kind, token, Box::new(ReqwestTransport::new()?))
}

/// Transport-injected core of [`validate_host_token`] (unit-tested offline).
fn validate_host_token_with(
    host: &str,
    kind: ForgeKind,
    token: &str,
    http: Box<dyn HttpTransport>,
) -> Result<ForgeViewer, AppError> {
    if kind == ForgeKind::AzureDevOps {
        return Err(AppError::ForgeUnsupported(
            "Azure DevOps accounts must be added from an open Azure DevOps repository".to_string(),
        ));
    }
    let host_l = host.to_ascii_lowercase();
    let target = ForgeTarget {
        kind,
        host: host_l,
        owner: String::new(),
        repo: String::new(),
        project: None,
        web_url: String::new(),
    };
    validate_token(target, token, http)
}

/// P80: store `token` in the OS keychain under an EXPLICIT `keychain_key`
/// (== a three-part `accountId` for a P80 account, or the bare host for a
/// migrated legacy one). No-op for an empty key. The token never lands in
/// settings.json, a URL, or a log.
pub fn store_token(keychain_key: &str, token: &str) -> Result<(), AppError> {
    if !keychain_key.is_empty() {
        auth::global().set(keychain_key, token)?;
    }
    Ok(())
}

/// P80: delete the token stored under `keychain_key` from the keychain.
/// Idempotent — deleting an absent entry is `Ok(())`. Does NOT touch the viewer
/// cache (that is keyed by host; the command layer evicts it when appropriate).
pub fn delete_token(keychain_key: &str) -> Result<(), AppError> {
    if !keychain_key.is_empty() {
        auth::global().delete(keychain_key)?;
    }
    Ok(())
}

/// Validate a candidate `token` against `target` using `http`, returning the
/// authenticated viewer on success. Stores NOTHING — the caller persists only
/// after this returns `Ok`. Split out from the entry points so the validate
/// path is unit-tested with a fake transport (no network, no keychain). Rejects
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
    // `viewer()` performs that provider's validation call and, on success, warms
    // the process viewer cache for the host: GitHub / GitLab / Bitbucket probe
    // `GET /user`; Azure DevOps validates on the REPOSITORY endpoint and then
    // makes ONE best-effort profile call for a display name (P72 — the profile
    // endpoint needs a scope the Code-scoped PAT the UI asks for lacks).
    build_provider(target, Some(token.to_string()), http).viewer()
}

/// Network-free: resolve the `(lowercased host, kind)` for the repo at `workdir`
/// from its `origin` remote, so the command layer can key the known-hosts index
/// after a per-repo set/clear WITHOUT a second network call. No `origin` remote
/// ⇒ [`AppError::NoRemote`]; an unparseable origin ⇒ an empty host with
/// [`ForgeKind::Unknown`] (same degradation as [`resolve_target`]).
pub fn resolve_forge_host(workdir: &Path) -> Result<(String, ForgeKind), AppError> {
    let target = resolve_target(workdir)?;
    Ok((target.host, target.kind))
}

/// P80: network-free `(lowercased host, owner/namespace, kind)` for the repo at
/// `workdir` from `origin`, so the command layer can run `resolve_account` (which
/// needs the owner for the owner-match step) without a second remote read. No
/// `origin` remote ⇒ [`AppError::NoRemote`]; an unparseable origin ⇒ empty
/// host/owner with [`ForgeKind::Unknown`] (same degradation as [`resolve_target`]).
pub fn resolve_forge_identity(workdir: &Path) -> Result<(String, String, ForgeKind), AppError> {
    let target = resolve_target(workdir)?;
    Ok((target.host, target.owner, target.kind))
}

/// Drop the cached viewer for `host` WITHOUT deleting the token (the expiry
/// flow — keep the PAT, stop surfacing a warm "connected" identity so the panel
/// routes to re-auth). Pub wrapper over [`auth::evict_viewer`]. Infallible.
pub fn invalidate_viewer(host: &str) {
    auth::evict_viewer(host);
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
            project: None,
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
            project: None,
            web_url: "https://gitlab.example.com/o/r".to_string(),
        };
        let http = CannedTransport {
            status: 200,
            body: "{}",
        };
        let err = validate_token(target, "tok", Box::new(http)).unwrap_err();
        assert!(matches!(err, AppError::ForgeUnsupported(_)), "got {err:?}");
    }

    /// Validating a token for an Azure DevOps host with no repo ⇒
    /// `ForgeUnsupported` (OD-6), returned BEFORE any transport access.
    ///
    /// Ported from the retired `set_token_for_host_with` (audit INFO-1 deleted
    /// the bare-host storing entry points) onto the surviving repo-less
    /// validate seam, which carries the same Azure guard: the coverage is about
    /// the guard, not about who stores afterwards.
    #[test]
    fn validate_host_token_azure_is_unsupported() {
        let http = CannedTransport {
            status: 200,
            body: "{}",
        };
        let err = validate_host_token_with(
            "dev.azure.com",
            ForgeKind::AzureDevOps,
            "tok",
            Box::new(http),
        )
        .unwrap_err();
        assert!(matches!(err, AppError::ForgeUnsupported(_)), "got {err:?}");
    }

    /// A rejected token for a repo-less host ⇒ `AuthFailed`, so the command
    /// layer never reaches its keychain store. Ported from the retired
    /// `set_token_for_host_with` (audit INFO-1).
    #[test]
    fn validate_host_token_bad_is_auth_failed() {
        let http = CannedTransport {
            status: 401,
            body: "{}",
        };
        let err =
            validate_host_token_with("github.com", ForgeKind::GitHub, "bad-tok", Box::new(http))
                .unwrap_err();
        assert!(matches!(err, AppError::AuthFailed(_)), "got {err:?}");
    }
}
