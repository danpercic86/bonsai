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

use std::path::Path;

use bonsai_core::error::AppError;

pub use detect::{detect_provider, ForgeTarget};
pub use http::{HttpMethod, HttpRequest, HttpResponse, HttpTransport, ReqwestTransport};
pub use provider::ForgeProvider;
pub use types::*;

use crate::github::GitHubProvider;

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
    let target = detect_provider(&url).unwrap_or_else(|| ForgeTarget {
        kind: ForgeKind::Unknown,
        host: String::new(),
        owner: String::new(),
        repo: String::new(),
        web_url: String::new(),
    });

    // Presence-only token lookup (never network). A keychain read error
    // degrades to unauthenticated rather than failing open().
    let token = if target.host.is_empty() {
        None
    } else {
        auth::global().get(&target.host).unwrap_or(None)
    };

    let transport = ReqwestTransport::new()?;
    Ok(Box::new(GitHubProvider::new(
        target,
        token,
        Box::new(transport),
    )))
}
