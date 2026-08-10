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
//! names (`dto`), and a cross-host identity endpoint for `viewer()`.

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

    fn viewer(&self) -> Result<ForgeViewer, AppError> {
        // The identity endpoint is a DIFFERENT host and needs no org/project/repo
        // — only a supported Azure target + a token (contract §3c).
        self.require_supported()?;
        let token = self.require_token()?;
        let resp = rest::get(self.transport(), &rest::profile_url(), Some(token))?;
        let viewer = dto::parse_viewer(&resp.body)?;
        if !self.target.host.is_empty() {
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
mod tests {
    use super::*;
    use crate::http::{HttpMethod, HttpRequest, HttpResponse};
    use crate::types::{CheckRollup, CommentKind, PrState, PrStateFilter};
    use std::sync::{Arc, Mutex};

    type Spy = Arc<Mutex<Vec<HttpRequest>>>;

    /// A canned transport keyed by a URL substring, recording every request into
    /// a shared [`Spy`]. Zero network.
    struct FakeTransport {
        routes: Vec<(String, u16, String)>,
        seen: Spy,
    }

    impl FakeTransport {
        fn with_seen(routes: Vec<(&str, u16, &str)>, seen: Spy) -> Self {
            Self {
                routes: routes
                    .into_iter()
                    .map(|(m, s, b)| (m.to_string(), s, b.to_string()))
                    .collect(),
                seen,
            }
        }
    }

    impl HttpTransport for FakeTransport {
        fn send(&self, req: &HttpRequest) -> Result<HttpResponse, AppError> {
            self.seen.lock().unwrap().push(req.clone());
            let (status, body) = self
                .routes
                .iter()
                .find(|(needle, _, _)| req.url.contains(needle.as_str()))
                .map(|(_, s, b)| (*s, b.clone()))
                .unwrap_or_else(|| panic!("no fake route matched {}", req.url));
            Ok(HttpResponse {
                status,
                headers: vec![],
                body,
            })
        }
    }

    fn azure_target() -> ForgeTarget {
        ForgeTarget {
            kind: ForgeKind::AzureDevOps,
            host: "dev.azure.com".to_string(),
            owner: "org".to_string(),
            repo: "repo".to_string(),
            project: Some("proj".to_string()),
            web_url: "https://dev.azure.com/org/proj/_git/repo".to_string(),
        }
    }

    fn provider_spy(
        token: Option<&str>,
        routes: Vec<(&str, u16, &str)>,
    ) -> (AzureDevOpsProvider, Spy) {
        let seen: Spy = Arc::new(Mutex::new(Vec::new()));
        let transport = FakeTransport::with_seen(routes, Arc::clone(&seen));
        let p = AzureDevOpsProvider::new(azure_target(), token.map(str::to_string), Box::new(transport));
        (p, seen)
    }

    fn provider(token: Option<&str>, routes: Vec<(&str, u16, &str)>) -> AzureDevOpsProvider {
        provider_spy(token, routes).0
    }

    #[test]
    fn viewer_maps_display_name_and_basic_auth_on_vssps_host() {
        let (p, seen) = provider_spy(
            Some("az-tok"),
            vec![(
                "/profile/profiles/me",
                200,
                r#"{ "displayName": "Ada Lovelace", "emailAddress": "ada@x" }"#,
            )],
        );
        let v = p.viewer().unwrap();
        assert_eq!(v.login, "Ada Lovelace");
        assert_eq!(v.avatar_url, None);
        let reqs = seen.lock().unwrap();
        // Cross-host identity endpoint, api-versioned.
        assert!(
            reqs[0].url.starts_with("https://app.vssps.visualstudio.com/"),
            "url: {}",
            reqs[0].url
        );
        assert!(reqs[0].url.contains("api-version=7.1"), "url: {}", reqs[0].url);
        // Basic auth (NOT Bearer / PRIVATE-TOKEN); plaintext PAT never on the wire.
        let auth = reqs[0]
            .headers
            .iter()
            .find(|(k, _)| k == "Authorization")
            .map(|(_, v)| v.as_str())
            .unwrap();
        assert!(auth.starts_with("Basic "), "auth: {auth}");
        assert!(!auth.contains("az-tok"), "PAT leaked into header: {auth}");
    }

    #[test]
    fn viewer_requires_token() {
        let p = provider(None, vec![("/profile/profiles/me", 200, "{}")]);
        assert!(matches!(p.viewer(), Err(AppError::ForgeAuthRequired(_))));
    }

    #[test]
    fn list_prs_maps_fields_pages_and_signals_next() {
        let body = r#"{
            "count": 1,
            "value": [
                { "pullRequestId": 12, "title": "One", "status": "active", "isDraft": false,
                  "createdBy": { "displayName": "a" },
                  "sourceRefName": "refs/heads/f1", "targetRefName": "refs/heads/main",
                  "creationDate": "2026-01-01T00:00:00Z",
                  "lastMergeSourceCommit": { "commitId": "s1" } }
            ]
        }"#;
        let (p, seen) = provider_spy(Some("az-tok"), vec![("/pullrequests?", 200, body)]);
        // per_page=1 ⇒ $top=1; a returned count of 1 == $top ⇒ has_next.
        let page = p
            .list_prs(&PrListQuery {
                state: PrStateFilter::Open,
                page: 1,
                per_page: 1,
            })
            .unwrap();
        assert_eq!(page.items.len(), 1);
        assert_eq!(page.items[0].number, 12);
        assert_eq!(page.items[0].state, PrState::Open);
        assert_eq!(page.items[0].head_sha, "s1");
        assert_eq!(page.items[0].source_branch, "f1", "refs/heads/ stripped");
        assert!(page.has_next, "returned == $top ⇒ has_next");
        let reqs = seen.lock().unwrap();
        assert!(
            reqs[0].url.contains("/org/proj/_apis/git/repositories/repo/pullrequests"),
            "url: {}",
            reqs[0].url
        );
        assert!(
            reqs[0].url.contains("searchCriteria.status=active"),
            "url: {}",
            reqs[0].url
        );
        assert!(reqs[0].url.contains("$top=1"), "url: {}", reqs[0].url);
        assert!(reqs[0].url.contains("$skip=0"), "url: {}", reqs[0].url);
        assert!(reqs[0].url.contains("api-version=7.1"), "url: {}", reqs[0].url);
    }

    #[test]
    fn list_prs_short_page_has_no_next_and_works_unauthenticated() {
        let (p, seen) = provider_spy(None, vec![("/pullrequests?", 200, r#"{ "value": [] }"#)]);
        let page = p
            .list_prs(&PrListQuery {
                state: PrStateFilter::All,
                page: 1,
                per_page: 30,
            })
            .unwrap();
        assert_eq!(page.items.len(), 0);
        assert!(!page.has_next, "0 < $top ⇒ no next");
        let reqs = seen.lock().unwrap();
        assert!(reqs[0].url.contains("searchCriteria.status=all"), "url: {}", reqs[0].url);
        assert!(!reqs[0].headers.iter().any(|(k, _)| k == "Authorization"));
    }

    #[test]
    fn get_pr_maps_detail() {
        let body = r#"{
            "pullRequestId": 7, "title": "Done", "status": "active", "isDraft": false,
            "createdBy": { "displayName": "Ada L" },
            "sourceRefName": "refs/heads/f", "targetRefName": "refs/heads/main",
            "creationDate": "2026-01-01T00:00:00Z",
            "lastMergeSourceCommit": { "commitId": "s" },
            "description": "hello", "mergeStatus": "conflicts",
            "labels": [ { "name": "bug" } ] }"#;
        let p = provider(Some("az-tok"), vec![("/pullrequests/7", 200, body)]);
        let d = p.get_pr(7).unwrap();
        assert_eq!(d.summary.number, 7);
        assert_eq!(d.body, "hello");
        assert_eq!(d.mergeable, Some(false), "conflicts ⇒ not mergeable");
        assert_eq!(d.labels, vec!["bug".to_string()]);
        assert_eq!(d.summary.head_sha, "s");
        assert_eq!(
            d.summary.url,
            "https://dev.azure.com/org/proj/_git/repo/pullrequest/7"
        );
    }

    #[test]
    fn create_pr_requires_token_and_posts_azure_body_with_refs() {
        let created = r#"{
            "pullRequestId": 42, "title": "New", "status": "active", "isDraft": false,
            "createdBy": { "displayName": "Ada L" },
            "sourceRefName": "refs/heads/feature", "targetRefName": "refs/heads/main",
            "creationDate": "2026-01-01T00:00:00Z",
            "lastMergeSourceCommit": { "commitId": "s" }, "description": "" }"#;
        let input = CreatePrInput {
            title: "New".into(),
            body: "".into(),
            source_branch: "feature".into(),
            target_branch: "main".into(),
            draft: false,
            maintainer_can_modify: true,
        };

        // Unauthenticated ⇒ ForgeAuthRequired, nothing sent.
        let (p_noauth, seen_noauth) = provider_spy(None, vec![("/pullrequests", 200, created)]);
        assert!(matches!(
            p_noauth.create_pr(&input),
            Err(AppError::ForgeAuthRequired(_))
        ));
        assert!(seen_noauth.lock().unwrap().is_empty(), "no request before auth check");

        // Authenticated ⇒ POST with refs re-added.
        let (p, seen) = provider_spy(Some("az-tok"), vec![("/pullrequests", 201, created)]);
        let d = p.create_pr(&input).unwrap();
        assert_eq!(d.summary.number, 42);
        let reqs = seen.lock().unwrap();
        assert_eq!(reqs[0].method, HttpMethod::Post);
        let sent: serde_json::Value = serde_json::from_str(reqs[0].body.as_ref().unwrap()).unwrap();
        assert_eq!(sent["sourceRefName"], "refs/heads/feature");
        assert_eq!(sent["targetRefName"], "refs/heads/main");
        assert_eq!(sent["title"], "New");
        assert_eq!(sent["isDraft"], false);
    }

    #[test]
    fn list_review_comments_splits_and_sorts_by_date() {
        // A review (diff) thread published later; a conversation thread earlier.
        // Sorted by created_at ⇒ conversation first.
        let body = r#"{
            "value": [
                { "id": 5,
                  "threadContext": { "filePath": "/src/x.rs", "rightFileStart": { "line": 3 } },
                  "comments": [
                    { "id": 1, "content": "line note", "author": { "displayName": "b" },
                      "publishedDate": "2026-01-02T00:00:00Z", "commentType": "text" } ] },
                { "id": 4,
                  "comments": [
                    { "id": 1, "content": "top", "author": { "displayName": "a" },
                      "publishedDate": "2026-01-01T00:00:00Z", "commentType": "text" } ] }
            ]
        }"#;
        let p = provider(Some("az-tok"), vec![("/threads", 200, body)]);
        let comments = p.list_review_comments(9).unwrap();
        assert_eq!(comments.len(), 2);
        assert_eq!(comments[0].kind, CommentKind::Conversation, "earliest first");
        assert_eq!(comments[0].id, 4001);
        assert_eq!(comments[1].kind, CommentKind::Review);
        assert_eq!(comments[1].id, 5001);
        assert_eq!(comments[1].path.as_deref(), Some("/src/x.rs"));
        assert_eq!(comments[1].line, Some(3));
    }

    #[test]
    fn combined_status_maps_azure_vocabulary() {
        let body = r#"{
            "value": [
                { "state": "succeeded", "context": { "name": "build" } },
                { "state": "failed", "context": { "name": "test" } } ]
        }"#;
        let p = provider(Some("az-tok"), vec![("/statuses", 200, body)]);
        let status = p.combined_status("sha1").unwrap();
        assert_eq!(status.state, CheckRollup::Failure);
        assert_eq!(status.total, 2);
        assert_eq!(status.passed, 1);
        assert_eq!(status.failed, 1);
    }

    #[test]
    fn commit_statuses_batch_omits_not_found_and_propagates_fatal() {
        let ok = r#"{ "value": [ { "state": "succeeded", "context": { "name": "ci" } } ] }"#;
        // aa11 resolves; bb22 404s (omitted).
        let p = provider(
            Some("az-tok"),
            vec![("aa11/statuses", 200, ok), ("bb22/statuses", 404, "{}")],
        );
        let shas = vec!["aa11".to_string(), "bb22".to_string()];
        let out = p.commit_statuses(&shas).unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].sha, "aa11");

        // A 401 on a sha ⇒ AuthFailed fails the whole batch.
        let p_fatal = provider(
            Some("bad"),
            vec![("aa11/statuses", 200, ok), ("bb22/statuses", 401, "{}")],
        );
        let err = p_fatal.commit_statuses(&shas).unwrap_err();
        assert!(matches!(err, AppError::AuthFailed(_)), "got {err:?}");
    }

    #[test]
    fn error_status_maps_to_app_error() {
        let p = provider(Some("az-tok"), vec![("/pullrequests/1", 404, "{}")]);
        assert!(matches!(p.get_pr(1), Err(AppError::ForgeApi(_))));

        let p401 = provider(Some("bad"), vec![("/profile/profiles/me", 401, "{}")]);
        assert!(matches!(p401.viewer(), Err(AppError::AuthFailed(_))));
    }

    #[test]
    fn unsupported_host_rejects_data_calls_but_gives_context() {
        let target = ForgeTarget {
            kind: ForgeKind::Unknown,
            host: "azure.example.com".to_string(),
            owner: "o".to_string(),
            repo: "r".to_string(),
            project: None,
            web_url: "https://azure.example.com/o/r".to_string(),
        };
        let transport = FakeTransport::with_seen(vec![], Arc::new(Mutex::new(Vec::new())));
        let p = AzureDevOpsProvider::new(target, None, Box::new(transport));
        let ctx = p.repo_context();
        assert_eq!(ctx.provider, ForgeKind::Unknown);
        assert_eq!(ctx.host, "azure.example.com");
        assert_eq!(ctx.project, None);
        assert!(matches!(
            p.list_prs(&PrListQuery {
                state: PrStateFilter::Open,
                page: 1,
                per_page: 10
            }),
            Err(AppError::ForgeUnsupported(_))
        ));
    }
}
