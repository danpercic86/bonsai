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
    CommitStatus, CreatePrInput, ForgeKind, ForgeRepoContext, ForgeViewer, PrDetail, PrListQuery,
    PrPage, ReviewComment,
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
            remote_name: REMOTE_NAME.to_string(),
            web_url: self.target.web_url.clone(),
            authenticated: self.token.is_some(),
            viewer,
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

    fn combined_status(&self, sha: &str) -> Result<CommitStatus, AppError> {
        self.require_supported()?;
        let status_url = rest::combined_status_url(self.owner(), self.repo(), sha);
        let combined = rest::get(self.transport(), &status_url, self.token.as_deref())?;

        let checks_url = rest::check_runs_url(self.owner(), self.repo(), sha);
        let checks = rest::get(self.transport(), &checks_url, self.token.as_deref())?;

        dto::build_combined_status(sha, &combined.body, &checks.body)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::http::{HttpMethod, HttpRequest, HttpResponse};
    use crate::types::{CheckRollup, CommentKind, ForgeKind, PrState, PrStateFilter};
    use std::sync::{Arc, Mutex};

    /// Shared handle to the requests a fake transport observed.
    type Spy = Arc<Mutex<Vec<HttpRequest>>>;

    /// A canned transport keyed by a URL substring, recording every request
    /// into a shared [`Spy`] so tests can inspect them. Zero network.
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

    fn github_target() -> ForgeTarget {
        ForgeTarget {
            kind: ForgeKind::GitHub,
            host: "github.com".to_string(),
            owner: "o".to_string(),
            repo: "r".to_string(),
            web_url: "https://github.com/o/r".to_string(),
        }
    }

    /// A provider whose transport records requests into the returned [`Spy`].
    fn provider_spy(token: Option<&str>, routes: Vec<(&str, u16, &str)>) -> (GitHubProvider, Spy) {
        let seen: Spy = Arc::new(Mutex::new(Vec::new()));
        let transport = FakeTransport::with_seen(routes, Arc::clone(&seen));
        let p = GitHubProvider::new(
            github_target(),
            token.map(str::to_string),
            Box::new(transport),
        );
        (p, seen)
    }

    /// A provider whose request log is not inspected.
    fn provider(token: Option<&str>, routes: Vec<(&str, u16, &str)>) -> GitHubProvider {
        provider_spy(token, routes).0
    }

    #[test]
    fn viewer_maps_get_user() {
        let p = provider(
            Some("tok"),
            vec![("/user", 200, r#"{ "login": "octocat", "avatar_url": "https://a/o.png" }"#)],
        );
        let v = p.viewer().unwrap();
        assert_eq!(v.login, "octocat");
        assert_eq!(v.avatar_url.as_deref(), Some("https://a/o.png"));
    }

    #[test]
    fn viewer_requires_token() {
        let p = provider(None, vec![("/user", 200, "{}")]);
        assert!(matches!(
            p.viewer(),
            Err(AppError::ForgeAuthRequired(_))
        ));
    }

    #[test]
    fn list_prs_caps_per_page_and_maps() {
        let body = r#"[
            { "number": 1, "title": "One", "state": "open", "draft": false,
              "user": { "login": "a", "avatar_url": null },
              "head": { "ref": "f1", "sha": "s1" }, "base": { "ref": "main", "sha": "m" },
              "comments": 0, "created_at": "2026-01-01T00:00:00Z",
              "updated_at": "2026-01-01T00:00:00Z", "html_url": "https://x/1", "merged_at": null }
        ]"#;
        let (p, seen) = provider_spy(Some("tok"), vec![("/pulls?", 200, body)]);
        let page = p
            .list_prs(&PrListQuery {
                state: PrStateFilter::Open,
                page: 1,
                per_page: 999,
            })
            .unwrap();
        assert_eq!(page.items.len(), 1);
        assert_eq!(page.items[0].state, PrState::Open);
        assert!(!page.has_next, "no Link header ⇒ no next page");
        // per_page clamped to 50 in the outgoing URL.
        let reqs = seen.lock().unwrap();
        assert!(reqs[0].url.contains("per_page=50"), "url: {}", reqs[0].url);
    }

    #[test]
    fn list_prs_works_unauthenticated() {
        let (p, seen) = provider_spy(None, vec![("/pulls?", 200, "[]")]);
        let page = p
            .list_prs(&PrListQuery {
                state: PrStateFilter::All,
                page: 1,
                per_page: 30,
            })
            .unwrap();
        assert_eq!(page.items.len(), 0);
        // No Authorization header when unauthenticated.
        let reqs = seen.lock().unwrap();
        assert!(!reqs[0].headers.iter().any(|(k, _)| k == "Authorization"));
    }

    #[test]
    fn get_pr_maps_detail() {
        let body = r#"{
            "number": 7, "title": "Done", "state": "open",
            "user": { "login": "a", "avatar_url": null },
            "head": { "ref": "f", "sha": "s" }, "base": { "ref": "main", "sha": "m" },
            "comments": 1, "created_at": "2026-01-01T00:00:00Z",
            "updated_at": "2026-01-02T00:00:00Z", "html_url": "https://x/7",
            "body": "hello", "mergeable": true,
            "additions": 5, "deletions": 1, "changed_files": 2,
            "labels": [ { "name": "bug" } ]
        }"#;
        let p = provider(Some("tok"), vec![("/pulls/7", 200, body)]);
        let d = p.get_pr(7).unwrap();
        assert_eq!(d.summary.number, 7);
        assert_eq!(d.body, "hello");
        assert_eq!(d.mergeable, Some(true));
        assert_eq!(d.labels, vec!["bug".to_string()]);
    }

    #[test]
    fn create_pr_requires_token_and_posts_body() {
        let created = r#"{
            "number": 42, "title": "New", "state": "open",
            "user": { "login": "a", "avatar_url": null },
            "head": { "ref": "feature", "sha": "s" }, "base": { "ref": "main", "sha": "m" },
            "comments": 0, "created_at": "2026-01-01T00:00:00Z",
            "updated_at": "2026-01-01T00:00:00Z", "html_url": "https://x/42",
            "body": "", "mergeable": null, "additions": 0, "deletions": 0,
            "changed_files": 0, "labels": [] }"#;
        let input = CreatePrInput {
            title: "New".into(),
            body: "".into(),
            source_branch: "feature".into(),
            target_branch: "main".into(),
            draft: false,
            maintainer_can_modify: true,
        };

        // Unauthenticated ⇒ ForgeAuthRequired, and NOTHING is sent.
        let (p_noauth, seen_noauth) = provider_spy(None, vec![("/pulls", 200, created)]);
        assert!(matches!(
            p_noauth.create_pr(&input),
            Err(AppError::ForgeAuthRequired(_))
        ));
        assert!(
            seen_noauth.lock().unwrap().is_empty(),
            "no request before auth check"
        );

        // Authenticated ⇒ POST with head/base body.
        let (p, seen) = provider_spy(Some("tok"), vec![("/pulls", 201, created)]);
        let d = p.create_pr(&input).unwrap();
        assert_eq!(d.summary.number, 42);
        let reqs = seen.lock().unwrap();
        assert_eq!(reqs[0].method, HttpMethod::Post);
        let sent: serde_json::Value =
            serde_json::from_str(reqs[0].body.as_ref().unwrap()).unwrap();
        assert_eq!(sent["head"], "feature");
        assert_eq!(sent["base"], "main");
    }

    #[test]
    fn list_review_comments_merges_and_sorts() {
        let review = r#"[
            { "id": 2, "user": { "login": "b", "avatar_url": null }, "body": "later",
              "path": "x.rs", "line": 3, "created_at": "2026-01-02T00:00:00Z",
              "html_url": "https://x/2" }
        ]"#;
        let issue = r#"[
            { "id": 1, "user": { "login": "a", "avatar_url": null }, "body": "earlier",
              "created_at": "2026-01-01T00:00:00Z", "html_url": "https://x/1" }
        ]"#;
        let p = provider(
            Some("tok"),
            vec![
                ("/pulls/9/comments", 200, review),
                ("/issues/9/comments", 200, issue),
            ],
        );
        let comments = p.list_review_comments(9).unwrap();
        assert_eq!(comments.len(), 2);
        // Sorted by created_at ⇒ the issue (earlier) comes first.
        assert_eq!(comments[0].id, 1);
        assert_eq!(comments[0].kind, CommentKind::Conversation);
        assert_eq!(comments[1].id, 2);
        assert_eq!(comments[1].kind, CommentKind::Review);
    }

    #[test]
    fn combined_status_merges_status_and_checks() {
        let combined = r#"{ "state": "success", "statuses": [
            { "state": "success", "context": "ci", "description": null, "target_url": null } ] }"#;
        let checks = r#"{ "check_runs": [
            { "name": "build", "status": "completed", "conclusion": "failure", "details_url": null } ] }"#;
        let p = provider(
            Some("tok"),
            vec![("/status", 200, combined), ("/check-runs", 200, checks)],
        );
        let status = p.combined_status("sha1").unwrap();
        // one success + one failure ⇒ overall Failure.
        assert_eq!(status.state, CheckRollup::Failure);
        assert_eq!(status.total, 2);
        assert_eq!(status.passed, 1);
        assert_eq!(status.failed, 1);
    }

    #[test]
    fn error_status_maps_to_app_error() {
        let p = provider(Some("tok"), vec![("/pulls/1", 404, "{}")]);
        assert!(matches!(p.get_pr(1), Err(AppError::ForgeApi(_))));

        let p401 = provider(Some("bad"), vec![("/user", 401, "{}")]);
        assert!(matches!(p401.viewer(), Err(AppError::AuthFailed(_))));
    }

    #[test]
    fn unsupported_host_rejects_data_calls_but_gives_context() {
        let target = ForgeTarget {
            kind: ForgeKind::Unknown,
            host: "gitlab.example.com".to_string(),
            owner: "o".to_string(),
            repo: "r".to_string(),
            web_url: "https://gitlab.example.com/o/r".to_string(),
        };
        let transport = FakeTransport::with_seen(vec![], Arc::new(Mutex::new(Vec::new())));
        let p = GitHubProvider::new(target, None, Box::new(transport));
        // repo_context is a friendly identity, NOT an error.
        let ctx = p.repo_context();
        assert_eq!(ctx.provider, ForgeKind::Unknown);
        assert_eq!(ctx.host, "gitlab.example.com");
        // but data calls are unsupported.
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
