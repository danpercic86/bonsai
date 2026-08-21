//! The GitHub [`ForgeProvider`] implementation.
//!
//! Orchestrates neutral requests: build a URL (`rest`), send via the injected
//! [`HttpTransport`], hand the raw body to `dto` for parsing+mapping. GitHub
//! wire structs live only in `dto`; this file speaks provider-neutral
//! [`crate::types`] everywhere.

mod dto;
mod rest;
mod rollup;

use bonsai_core::error::AppError;

use crate::auth;
use crate::detect::ForgeTarget;
use crate::http::HttpTransport;
use crate::provider::ForgeProvider;
use crate::types::{
    CommitStatus, CreatePrInput, ForgeKind, ForgeRepoContext, ForgeViewer, MergePrInput, PrDetail,
    PrListQuery, PrPage, ReviewComment,
};

/// `origin` always resolves to a single remote; the provider reports it.
const REMOTE_NAME: &str = "origin";

/// Hard cap on `per_page` (§3).
const MAX_PER_PAGE: u32 = 50;

pub struct GitHubProvider {
    target: ForgeTarget,
    token: Option<String>,
    http: Box<dyn HttpTransport>,
}

impl GitHubProvider {
    pub fn new(target: ForgeTarget, token: Option<String>, http: Box<dyn HttpTransport>) -> Self {
        Self {
            target,
            token,
            http,
        }
    }

    /// Data methods are only meaningful for a recognized GitHub host; anything
    /// else (enterprise / unparseable origin) ⇒ `ForgeUnsupported`.
    fn require_supported(&self) -> Result<(), AppError> {
        if self.target.kind == ForgeKind::GitHub {
            Ok(())
        } else {
            Err(AppError::ForgeUnsupported(format!(
                "{} is not a supported forge (only github.com in v1)",
                if self.target.host.is_empty() {
                    "this remote"
                } else {
                    &self.target.host
                }
            )))
        }
    }

    /// Return the token, or `ForgeAuthRequired` BEFORE any request.
    fn require_token(&self) -> Result<&str, AppError> {
        self.token.as_deref().ok_or_else(|| {
            AppError::ForgeAuthRequired(
                "this operation requires a GitHub token — connect an account first".to_string(),
            )
        })
    }

    fn owner(&self) -> &str {
        &self.target.owner
    }
    fn repo(&self) -> &str {
        &self.target.repo
    }
    fn transport(&self) -> &dyn HttpTransport {
        self.http.as_ref()
    }
}

impl ForgeProvider for GitHubProvider {
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
            resolved_account_id: None,
            account_source: crate::types::AccountSource::None,
        }
    }

    fn viewer(&self) -> Result<ForgeViewer, AppError> {
        self.require_supported()?;
        let token = self.require_token()?;
        let resp = rest::get(self.transport(), &rest::user_url(), Some(token))?;
        let viewer = dto::parse_viewer(&resp.body)?;
        if !self.target.host.is_empty() {
            auth::cache_viewer(&self.target.host, viewer.clone());
        }
        Ok(viewer)
    }

    fn list_prs(&self, query: &PrListQuery) -> Result<PrPage, AppError> {
        self.require_supported()?;
        let page = query.page.max(1);
        let per_page = query.per_page.clamp(1, MAX_PER_PAGE);
        let url = rest::pulls_url(self.owner(), self.repo(), query.state, per_page, page);
        let resp = rest::get(self.transport(), &url, self.token.as_deref())?;
        let items = dto::parse_pr_list(&resp.body)?;
        let has_next = rest::has_next_link(&resp);
        Ok(PrPage {
            items,
            page,
            has_next,
        })
    }

    fn get_pr(&self, number: u64) -> Result<PrDetail, AppError> {
        self.require_supported()?;
        let url = rest::pull_url(self.owner(), self.repo(), number);
        let resp = rest::get(self.transport(), &url, self.token.as_deref())?;
        dto::parse_pr_detail(&resp.body)
    }

    fn create_pr(&self, input: &CreatePrInput) -> Result<PrDetail, AppError> {
        self.require_supported()?;
        let token = self.require_token()?; // create REQUIRES auth
        let body = dto::create_pull_body(input)?;
        let url = rest::create_pull_url(self.owner(), self.repo());
        let resp = rest::post(self.transport(), &url, Some(token), body)?;
        dto::parse_pr_detail(&resp.body)
    }

    fn list_review_comments(&self, number: u64) -> Result<Vec<ReviewComment>, AppError> {
        self.require_supported()?;
        let review_url = rest::review_comments_url(self.owner(), self.repo(), number);
        let review = rest::get(self.transport(), &review_url, self.token.as_deref())?;
        let mut out = dto::parse_review_comments(&review.body)?;

        let issue_url = rest::issue_comments_url(self.owner(), self.repo(), number);
        let issue = rest::get(self.transport(), &issue_url, self.token.as_deref())?;
        out.extend(dto::parse_issue_comments(&issue.body)?);

        out.sort_by(|a, b| a.created_at.cmp(&b.created_at));
        Ok(out)
    }

    fn merge_pr(&self, number: u64, input: &MergePrInput) -> Result<PrDetail, AppError> {
        self.require_supported()?;
        let token = self.require_token()?; // merge REQUIRES auth
        // Unsupported method ⇒ error BEFORE any request is sent.
        let body = dto::merge_body(input)?;
        let url = rest::merge_pull_url(self.owner(), self.repo(), number);
        // 200 on success; the merge response has no full PR, so re-fetch it.
        rest::put_merge(self.transport(), &url, Some(token), body)?;
        self.get_pr(number)
    }

    fn close_pr(&self, number: u64) -> Result<PrDetail, AppError> {
        self.require_supported()?;
        let token = self.require_token()?; // close REQUIRES auth
        let url = rest::pull_url(self.owner(), self.repo(), number);
        let resp = rest::patch(self.transport(), &url, Some(token), dto::close_body())?;
        dto::parse_pr_detail(&resp.body)
    }

    fn combined_status(&self, sha: &str) -> Result<CommitStatus, AppError> {
        self.require_supported()?;
        let status_url = rest::combined_status_url(self.owner(), self.repo(), sha);
        let combined = rest::get(self.transport(), &status_url, self.token.as_deref())?;

        let checks_url = rest::check_runs_url(self.owner(), self.repo(), sha);
        let checks = rest::get(self.transport(), &checks_url, self.token.as_deref())?;

        dto::build_combined_status(sha, &combined.body, &checks.body)
    }

    fn commit_statuses(&self, shas: &[String]) -> Result<Vec<CommitStatus>, AppError> {
        // Dedup + cap + per-sha error classification (omit not-found, propagate
        // fatal) is provider-neutral ⇒ shared in `crate::rollup`.
        crate::rollup::batch_commit_statuses(shas, |sha| self.combined_status(sha))
    }
}

#[cfg(test)]
mod tests;
