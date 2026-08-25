//! The GitLab [`ForgeProvider`] implementation (REST v4).
//!
//! Orchestrates neutral requests: build a URL (`rest`), send via the injected
//! [`HttpTransport`], hand the raw body to `dto` for parsing+mapping. GitLab
//! wire structs live only in `dto`; this file speaks provider-neutral
//! [`crate::types`] everywhere. Mirrors `github/mod.rs`.

mod dto;
mod rest;

use bonsai_core::error::AppError;

use crate::auth;
use crate::detect::ForgeTarget;
use crate::http::HttpTransport;
use crate::provider::ForgeProvider;
use crate::types::{
    CommitStatus, CreatePrInput, ForgeKind, ForgeRepoContext, ForgeViewer, MergePrInput, PrDetail,
    PrListQuery, PrPage, PrRefs, ReviewComment,
};

/// `origin` always resolves to a single remote; the provider reports it.
const REMOTE_NAME: &str = "origin";

/// Hard cap on `per_page` (mirrors GitHub; GitLab's own max is 100).
const MAX_PER_PAGE: u32 = 50;

pub struct GitLabProvider {
    target: ForgeTarget,
    token: Option<String>,
    http: Box<dyn HttpTransport>,
}

impl GitLabProvider {
    pub fn new(target: ForgeTarget, token: Option<String>, http: Box<dyn HttpTransport>) -> Self {
        Self {
            target,
            token,
            http,
        }
    }

    /// Data methods are only meaningful for a recognized GitLab host.
    fn require_supported(&self) -> Result<(), AppError> {
        if self.target.kind == ForgeKind::GitLab {
            Ok(())
        } else {
            Err(AppError::ForgeUnsupported(format!(
                "{} is not a supported GitLab host",
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
                "this operation requires a GitLab token — connect an account first".to_string(),
            )
        })
    }

    fn host(&self) -> &str {
        &self.target.host
    }
    /// URL-encoded `owner/repo` project path (handles nested groups).
    fn project_id(&self) -> String {
        rest::project_id(&self.target.owner, &self.target.repo)
    }
    /// Canonical MR page URL, used to anchor per-note comment URLs.
    fn mr_web_url(&self, iid: u64) -> String {
        format!("{}/-/merge_requests/{iid}", self.target.web_url)
    }
    fn transport(&self) -> &dyn HttpTransport {
        self.http.as_ref()
    }
}

impl ForgeProvider for GitLabProvider {
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
        let resp = rest::get(self.transport(), &rest::user_url(self.host()), Some(token))?;
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
        let url = rest::merge_requests_url(self.host(), &self.project_id(), query.state, per_page, page);
        let resp = rest::get(self.transport(), &url, self.token.as_deref())?;
        let items = dto::parse_mr_list(&resp.body)?;
        let has_next = rest::has_next_page(&resp, per_page, items.len());
        Ok(PrPage {
            items,
            page,
            has_next,
        })
    }

    fn get_pr(&self, number: u64) -> Result<PrDetail, AppError> {
        self.require_supported()?;
        let url = rest::merge_request_url(self.host(), &self.project_id(), number);
        let resp = rest::get(self.transport(), &url, self.token.as_deref())?;
        dto::parse_mr_detail(&resp.body)
    }

    fn pr_refs(&self, number: u64) -> Result<PrRefs, AppError> {
        self.require_supported()?;
        let url = rest::merge_request_url(self.host(), &self.project_id(), number);
        let resp = rest::get(self.transport(), &url, self.token.as_deref())?;
        dto::parse_mr_refs(&resp.body, number)
    }

    fn create_pr(&self, input: &CreatePrInput) -> Result<PrDetail, AppError> {
        self.require_supported()?;
        let token = self.require_token()?; // create REQUIRES auth
        let body = dto::create_mr_body(input)?;
        let url = rest::create_merge_request_url(self.host(), &self.project_id());
        let resp = rest::post(self.transport(), &url, Some(token), body)?;
        dto::parse_mr_detail(&resp.body)
    }

    fn list_review_comments(&self, number: u64) -> Result<Vec<ReviewComment>, AppError> {
        self.require_supported()?;
        let id = self.project_id();
        let mr_url = self.mr_web_url(number);

        // Conversation notes from /notes (diff + system notes dropped there).
        let notes_url = rest::notes_url(self.host(), &id, number);
        let notes = rest::get(self.transport(), &notes_url, self.token.as_deref())?;
        let mut out = dto::parse_notes(&notes.body, &mr_url)?;

        // Diff (Review) notes from /discussions.
        let disc_url = rest::discussions_url(self.host(), &id, number);
        let disc = rest::get(self.transport(), &disc_url, self.token.as_deref())?;
        out.extend(dto::parse_discussions(&disc.body, &mr_url)?);

        out.sort_by(|a, b| a.created_at.cmp(&b.created_at));
        Ok(out)
    }

    fn merge_pr(&self, number: u64, input: &MergePrInput) -> Result<PrDetail, AppError> {
        self.require_supported()?;
        let token = self.require_token()?; // merge REQUIRES auth
        // Unsupported method ⇒ error BEFORE any request is sent.
        let body = dto::merge_body(input)?;
        let url = rest::merge_mr_url(self.host(), &self.project_id(), number);
        // 200 returns the updated MR; not-mergeable statuses map in `put_merge`.
        let resp = rest::put_merge(self.transport(), &url, Some(token), body)?;
        dto::parse_mr_detail(&resp.body)
    }

    fn close_pr(&self, number: u64) -> Result<PrDetail, AppError> {
        self.require_supported()?;
        let token = self.require_token()?; // close REQUIRES auth
        let url = rest::merge_request_url(self.host(), &self.project_id(), number);
        let resp = rest::put(self.transport(), &url, Some(token), dto::close_body())?;
        dto::parse_mr_detail(&resp.body)
    }

    fn combined_status(&self, sha: &str) -> Result<CommitStatus, AppError> {
        self.require_supported()?;
        let url = rest::commit_statuses_url(self.host(), &self.project_id(), sha);
        let resp = rest::get(self.transport(), &url, self.token.as_deref())?;
        dto::build_pipeline_status(sha, &resp.body)
    }

    fn commit_statuses(&self, shas: &[String]) -> Result<Vec<CommitStatus>, AppError> {
        // Dedup + cap + per-sha error classification (omit not-found, propagate
        // fatal) is provider-neutral ⇒ shared in `crate::rollup`.
        crate::rollup::batch_commit_statuses(shas, |sha| self.combined_status(sha))
    }
}

#[cfg(test)]
mod tests;
