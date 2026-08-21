//! The Bitbucket Cloud [`ForgeProvider`] implementation (REST 2.0).
//!
//! Orchestrates neutral requests: build a URL (`rest`), send via the injected
//! [`HttpTransport`], hand the raw body to `dto` for parsing+mapping. Bitbucket
//! wire structs live only in `dto`; this file speaks provider-neutral
//! [`crate::types`] everywhere. Mirrors `gitlab/mod.rs`.
//!
//! Auth (OQ-A5): a Bitbucket **access token** is sent as `Authorization: Bearer`
//! (see `rest::base_headers`), preserving the single-secret keychain model; the
//! `user:app_password` → `Basic` scheme is a documented fallback, not implemented.

mod dto;
mod rest;

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

/// Hard cap on `pagelen` (Bitbucket's own PR max is 50).
const MAX_PER_PAGE: u32 = 50;

pub struct BitbucketProvider {
    target: ForgeTarget,
    token: Option<String>,
    http: Box<dyn HttpTransport>,
}

impl BitbucketProvider {
    pub fn new(target: ForgeTarget, token: Option<String>, http: Box<dyn HttpTransport>) -> Self {
        Self {
            target,
            token,
            http,
        }
    }

    /// Data methods are only meaningful for a recognized Bitbucket host.
    fn require_supported(&self) -> Result<(), AppError> {
        if self.target.kind == ForgeKind::Bitbucket {
            Ok(())
        } else {
            Err(AppError::ForgeUnsupported(format!(
                "{} is not a supported Bitbucket host",
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
                "this operation requires a Bitbucket token — connect an account first".to_string(),
            )
        })
    }

    /// The Bitbucket workspace (detection maps `owner` → workspace).
    fn workspace(&self) -> &str {
        &self.target.owner
    }
    /// The Bitbucket repository slug.
    fn slug(&self) -> &str {
        &self.target.repo
    }
    fn transport(&self) -> &dyn HttpTransport {
        self.http.as_ref()
    }
}

impl ForgeProvider for BitbucketProvider {
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
        let url = rest::pull_requests_url(self.workspace(), self.slug(), query.state, per_page, page);
        let resp = rest::get(self.transport(), &url, self.token.as_deref())?;
        let (items, has_next) = dto::parse_pr_list(&resp.body)?;
        Ok(PrPage {
            items,
            page,
            has_next,
        })
    }

    fn get_pr(&self, number: u64) -> Result<PrDetail, AppError> {
        self.require_supported()?;
        let url = rest::pull_request_url(self.workspace(), self.slug(), number);
        let resp = rest::get(self.transport(), &url, self.token.as_deref())?;
        dto::parse_pr_detail(&resp.body)
    }

    fn create_pr(&self, input: &CreatePrInput) -> Result<PrDetail, AppError> {
        self.require_supported()?;
        let token = self.require_token()?; // create REQUIRES auth
        let body = dto::create_pr_body(input)?;
        let url = rest::create_pull_request_url(self.workspace(), self.slug());
        let resp = rest::post(self.transport(), &url, Some(token), body)?;
        dto::parse_pr_detail(&resp.body)
    }

    fn list_review_comments(&self, number: u64) -> Result<Vec<ReviewComment>, AppError> {
        self.require_supported()?;
        let url = rest::comments_url(self.workspace(), self.slug(), number);
        let resp = rest::get(self.transport(), &url, self.token.as_deref())?;
        let mut out = dto::parse_comments(&resp.body)?;
        out.sort_by(|a, b| a.created_at.cmp(&b.created_at));
        Ok(out)
    }

    fn merge_pr(&self, number: u64, input: &MergePrInput) -> Result<PrDetail, AppError> {
        self.require_supported()?;
        let token = self.require_token()?; // merge REQUIRES auth
        // Unsupported method ⇒ error BEFORE any request is sent.
        let body = dto::merge_body(input)?;
        let url = rest::merge_pull_request_url(self.workspace(), self.slug(), number);
        // 200 returns the merged PR; not-mergeable statuses map in `post_merge`.
        let resp = rest::post_merge(self.transport(), &url, Some(token), body)?;
        dto::parse_pr_detail(&resp.body)
    }

    fn close_pr(&self, number: u64) -> Result<PrDetail, AppError> {
        self.require_supported()?;
        let token = self.require_token()?; // decline REQUIRES auth
        let url = rest::decline_pull_request_url(self.workspace(), self.slug(), number);
        let resp = rest::post(self.transport(), &url, Some(token), dto::decline_body())?;
        dto::parse_pr_detail(&resp.body)
    }

    fn combined_status(&self, sha: &str) -> Result<CommitStatus, AppError> {
        self.require_supported()?;
        let url = rest::commit_statuses_url(self.workspace(), self.slug(), sha);
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
mod tests;
