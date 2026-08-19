//! The Azure DevOps [`ForgeProvider`] implementation (REST 7.1).
//!
//! Orchestrates neutral requests: build a URL (`rest`), send via the injected
//! [`HttpTransport`], hand the raw body to `dto` for parsing+mapping. Azure wire
//! structs live only in `dto`; this file speaks provider-neutral
//! [`crate::types`] everywhere. Mirrors `bitbucket/mod.rs`.
//!
//! Azure is the most divergent provider (contract §1): it needs a 3-part
//! org/project/repo identity (`ForgeTarget.project`), Basic-auth of a
//! colon-prefixed PAT (`rest::base_headers`), `refs/heads/` stripping on branch
//! names (`dto`), and a two-step `viewer()` (P72): validate on the repository
//! endpoint (Code scope), then ONE best-effort call to the cross-host identity
//! endpoint, whose failure never fails the connect.

mod dto;
mod rest;

use bonsai_core::error::AppError;

use crate::auth;
use crate::detect::ForgeTarget;
use crate::http::HttpTransport;
use crate::provider::ForgeProvider;
use crate::types::{
    CommitStatus, CreatePrInput, ForgeKind, ForgeRepoContext, ForgeViewer, PrDetail, PrListQuery,
    PrPage, ReviewComment,
};

/// `origin` always resolves to a single remote; the provider reports it.
const REMOTE_NAME: &str = "origin";

/// Hard cap on `$top` (mirrors the other providers; Azure's own PR max is 101).
const MAX_PER_PAGE: u32 = 50;

pub struct AzureDevOpsProvider {
    target: ForgeTarget,
    token: Option<String>,
    http: Box<dyn HttpTransport>,
}

impl AzureDevOpsProvider {
    pub fn new(target: ForgeTarget, token: Option<String>, http: Box<dyn HttpTransport>) -> Self {
        Self {
            target,
            token,
            http,
        }
    }

    /// Data methods are only meaningful for a recognized Azure DevOps host.
    fn require_supported(&self) -> Result<(), AppError> {
        if self.target.kind == ForgeKind::AzureDevOps {
            Ok(())
        } else {
            Err(AppError::ForgeUnsupported(format!(
                "{} is not a supported Azure DevOps host",
                if self.target.host.is_empty() {
                    "this remote"
                } else {
                    &self.target.host
                }
            )))
        }
    }

    /// The `(org, project, repo)` triple every repo endpoint needs. Errors with
    /// `ForgeUnsupported` for a non-Azure host or a target missing the project
    /// part (detection always supplies it for Azure — this is a guard).
    fn coords(&self) -> Result<(&str, &str, &str), AppError> {
        self.require_supported()?;
        let project = self.target.project.as_deref().ok_or_else(|| {
            AppError::ForgeUnsupported(
                "Azure DevOps requires an org/project/repo remote".to_string(),
            )
        })?;
        Ok((self.target.owner.as_str(), project, self.target.repo.as_str()))
    }

    /// Return the token, or `ForgeAuthRequired` BEFORE any request.
    fn require_token(&self) -> Result<&str, AppError> {
        self.token.as_deref().ok_or_else(|| {
            AppError::ForgeAuthRequired(
                "this operation requires an Azure DevOps token — connect an account first"
                    .to_string(),
            )
        })
    }

    /// The PR page URL used to anchor per-comment discussion URLs.
    fn pr_web_url(&self, id: u64) -> String {
        format!("{}/pullrequest/{id}", self.target.web_url)
    }
    fn transport(&self) -> &dyn HttpTransport {
        self.http.as_ref()
    }
}

/// Rewrite a repo-probe [`AppError::ForgeApi`] into a message that NAMES the
/// org/project/repo triple — `map_status`'s bare `"not found"` gives the user
/// nothing to act on, and the overwhelmingly likely cause of a failed repo probe
/// is wrong coordinates or a PAT issued for a different organization. Only the
/// `ForgeApi` variant is rewritten; `AuthFailed`/rate-limit/transport errors pass
/// through untouched (matching on the message text would be fragile). Never
/// interpolates the token, the URL, or the response body.
///
/// The coords hint is **appended**, not substituted (reviewer SF-2, P72). An
/// earlier version replaced the whole message, which meant an Azure 5xx outage
/// or a 302-to-sign-in read as "check the organization, project, and repository
/// names" — misleading in a new direction. Appending keeps the status-derived
/// message (which is all we know for 5xx/3xx) while still naming the coords on
/// the 404 that actually motivated this, and needs no status-aware signal from
/// `rest::get`.
fn coords_hint(err: AppError, org: &str, project: &str, repo: &str) -> AppError {
    match err {
        AppError::ForgeApi(m) => AppError::ForgeApi(format!(
            "{m} — for the Azure DevOps repository {org}/{project}/{repo}; check the organization, project, and repository names, or whether the PAT was issued for a different organization"
        )),
        other => other,
    }
}

impl ForgeProvider for AzureDevOpsProvider {
    fn repo_context(&self) -> ForgeRepoContext {
        let viewer = if self.target.host.is_empty() {
            None
        } else {
            auth::cached_viewer(&self.target.host)
        };
        ForgeRepoContext {
            provider: self.target.kind,
            host: self.target.host.clone(),
            owner: self.target.owner.clone(),
            repo: self.target.repo.clone(),
            project: self.target.project.clone(),
            remote_name: REMOTE_NAME.to_string(),
            web_url: self.target.web_url.clone(),
            authenticated: self.token.is_some(),
            viewer,
        }
    }

    /// VALIDATE (must succeed) then IDENTIFY (best effort) — P72 §A4.
    ///
    /// Validation probes the REPOSITORY endpoint, which needs exactly the Code
    /// scope every other Azure call already needs. The old implementation probed
    /// the cross-host profile endpoint, which is gated on `vso.profile`: a PAT
    /// scoped Code (Read & Write) — precisely what the Connect panel asks for —
    /// has no profile scope, so Azure answered 401 and a perfectly valid token
    /// was reported as rejected. The profile call therefore survives only as an
    /// optional display-name lookup whose failure is swallowed.
    fn viewer(&self) -> Result<ForgeViewer, AppError> {
        let (org, project, repo) = self.coords()?;
        let token = self.require_token()?;

        // VALIDATE: a 401/203 here is a genuine credential failure; nothing is
        // cached and (in `lib.rs::set_token`) nothing is stored.
        let resp = rest::get(
            self.transport(),
            &rest::repository_url(org, project, repo),
            Some(token),
        )
        .map_err(|e| coords_hint(e, org, project, repo))?;
        dto::parse_repo_probe(&resp.body)?;

        // IDENTIFY: exactly ONE attempt, and EVERY error is swallowed — a missing
        // profile scope, a rate limit, or a network blip must never turn a
        // successful validation into a failed connect. `login` has no render site
        // today, so an empty login costs nothing.
        let viewer = match rest::get(self.transport(), &rest::profile_url(), Some(token))
            .and_then(|r| dto::parse_viewer(&r.body))
        {
            Ok(v) => v,
            Err(_) => ForgeViewer {
                login: String::new(),
                avatar_url: None,
            },
        };

        // Never cache an empty login: `repo_context().viewer` must not be able to
        // serve `login: ""` as if it were a resolved identity.
        if !self.target.host.is_empty() && !viewer.login.is_empty() {
            auth::cache_viewer(&self.target.host, viewer.clone());
        }
        Ok(viewer)
    }

    fn list_prs(&self, query: &PrListQuery) -> Result<PrPage, AppError> {
        let (org, project, repo) = self.coords()?;
        let page = query.page.max(1);
        let top = query.per_page.clamp(1, MAX_PER_PAGE);
        let skip = (page - 1) * top;
        let url = rest::pull_requests_url(org, project, repo, query.state, top, skip);
        let resp = rest::get(self.transport(), &url, self.token.as_deref())?;
        let items = dto::parse_pr_list(&resp.body, &self.target.web_url)?;
        // Azure pages by $skip/$top; a full page (returned == $top) ⇒ maybe more.
        let has_next = items.len() as u32 == top;
        Ok(PrPage {
            items,
            page,
            has_next,
        })
    }

    fn get_pr(&self, number: u64) -> Result<PrDetail, AppError> {
        let (org, project, repo) = self.coords()?;
        let url = rest::pull_request_url(org, project, repo, number);
        let resp = rest::get(self.transport(), &url, self.token.as_deref())?;
        dto::parse_pr_detail(&resp.body, &self.target.web_url)
    }

    fn create_pr(&self, input: &CreatePrInput) -> Result<PrDetail, AppError> {
        let (org, project, repo) = self.coords()?;
        let token = self.require_token()?; // create REQUIRES auth
        let body = dto::create_pr_body(input)?;
        let url = rest::create_pull_request_url(org, project, repo);
        let resp = rest::post(self.transport(), &url, Some(token), body)?;
        dto::parse_pr_detail(&resp.body, &self.target.web_url)
    }

    fn list_review_comments(&self, number: u64) -> Result<Vec<ReviewComment>, AppError> {
        let (org, project, repo) = self.coords()?;
        let url = rest::threads_url(org, project, repo, number);
        let resp = rest::get(self.transport(), &url, self.token.as_deref())?;
        let mut out = dto::parse_threads(&resp.body, &self.pr_web_url(number))?;
        out.sort_by(|a, b| a.created_at.cmp(&b.created_at));
        Ok(out)
    }

    fn combined_status(&self, sha: &str) -> Result<CommitStatus, AppError> {
        let (org, project, repo) = self.coords()?;
        let url = rest::commit_statuses_url(org, project, repo, sha);
        let resp = rest::get(self.transport(), &url, self.token.as_deref())?;
        dto::build_commit_status(sha, &resp.body)
    }

    fn commit_statuses(&self, shas: &[String]) -> Result<Vec<CommitStatus>, AppError> {
        // Dedup + cap + per-sha error classification (omit not-found, propagate
        // fatal) is provider-neutral ⇒ shared in `crate::rollup`.
        crate::rollup::batch_commit_statuses(shas, |sha| self.combined_status(sha))
    }
}

#[cfg(test)]
#[path = "testkit.rs"]
mod testkit;

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "viewer_tests.rs"]
mod viewer_tests;
