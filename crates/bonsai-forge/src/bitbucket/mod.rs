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
    CommitStatus, CreatePrInput, ForgeKind, ForgeRepoContext, ForgeViewer, PrDetail, PrListQuery,
    PrPage, ReviewComment,
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

    fn bitbucket_target() -> ForgeTarget {
        ForgeTarget {
            kind: ForgeKind::Bitbucket,
            host: "bitbucket.org".to_string(),
            owner: "ws".to_string(),
            repo: "repo".to_string(),
            project: None,
            web_url: "https://bitbucket.org/ws/repo".to_string(),
        }
    }

    fn provider_spy(token: Option<&str>, routes: Vec<(&str, u16, &str)>) -> (BitbucketProvider, Spy) {
        let seen: Spy = Arc::new(Mutex::new(Vec::new()));
        let transport = FakeTransport::with_seen(routes, Arc::clone(&seen));
        let p = BitbucketProvider::new(bitbucket_target(), token.map(str::to_string), Box::new(transport));
        (p, seen)
    }

    fn provider(token: Option<&str>, routes: Vec<(&str, u16, &str)>) -> BitbucketProvider {
        provider_spy(token, routes).0
    }

    #[test]
    fn viewer_maps_get_user_username_and_bearer_auth() {
        let (p, seen) = provider_spy(
            Some("bb-tok"),
            vec![(
                "/user",
                200,
                r#"{ "username": "ada", "display_name": "Ada L",
                     "links": { "avatar": { "href": "https://a/ada.png" } } }"#,
            )],
        );
        let v = p.viewer().unwrap();
        assert_eq!(v.login, "ada");
        assert_eq!(v.avatar_url.as_deref(), Some("https://a/ada.png"));
        let reqs = seen.lock().unwrap();
        assert!(reqs[0].url.contains("/2.0/user"), "url: {}", reqs[0].url);
        assert!(reqs[0]
            .headers
            .iter()
            .any(|(k, v)| k == "Authorization" && v == "Bearer bb-tok"));
    }

    #[test]
    fn viewer_requires_token() {
        let p = provider(None, vec![("/user", 200, "{}")]);
        assert!(matches!(p.viewer(), Err(AppError::ForgeAuthRequired(_))));
    }

    #[test]
    fn list_prs_caps_pagelen_maps_id_and_signals_next() {
        let body = r#"{
            "next": "https://api.bitbucket.org/2.0/repositories/ws/repo/pullrequests?page=2",
            "values": [
                { "id": 12, "title": "One", "state": "OPEN",
                  "author": { "display_name": "a" },
                  "source": { "branch": { "name": "f1" }, "commit": { "hash": "s1" } },
                  "destination": { "branch": { "name": "main" } },
                  "comment_count": 0, "created_on": "2026-01-01T00:00:00Z",
                  "updated_on": "2026-01-01T00:00:00Z",
                  "links": { "html": { "href": "https://bitbucket.org/ws/repo/pull-requests/12" } } }
            ]
        }"#;
        let (p, seen) = provider_spy(Some("bb-tok"), vec![("/pullrequests?", 200, body)]);
        let page = p
            .list_prs(&PrListQuery {
                state: PrStateFilter::Open,
                page: 1,
                per_page: 999,
            })
            .unwrap();
        assert_eq!(page.items.len(), 1);
        assert_eq!(page.items[0].number, 12, "number is the PR id");
        assert_eq!(page.items[0].state, PrState::Open);
        assert_eq!(page.items[0].head_sha, "s1");
        assert!(page.has_next, "a `next` URL in the body ⇒ has_next");
        let reqs = seen.lock().unwrap();
        // pagelen clamped to 50; workspace/slug in the path; state=OPEN.
        assert!(reqs[0].url.contains("pagelen=50"), "url: {}", reqs[0].url);
        assert!(
            reqs[0].url.contains("/repositories/ws/repo/pullrequests"),
            "url: {}",
            reqs[0].url
        );
        assert!(reqs[0].url.contains("state=OPEN"), "url: {}", reqs[0].url);
    }

    #[test]
    fn list_prs_works_unauthenticated() {
        let (p, seen) = provider_spy(None, vec![("/pullrequests?", 200, r#"{ "values": [] }"#)]);
        let page = p
            .list_prs(&PrListQuery {
                state: PrStateFilter::All,
                page: 1,
                per_page: 30,
            })
            .unwrap();
        assert_eq!(page.items.len(), 0);
        let reqs = seen.lock().unwrap();
        assert!(!reqs[0].headers.iter().any(|(k, _)| k == "Authorization"));
    }

    #[test]
    fn get_pr_maps_detail() {
        let body = r#"{
            "id": 7, "title": "Done", "state": "OPEN",
            "author": { "display_name": "Ada L" },
            "source": { "branch": { "name": "f" }, "commit": { "hash": "s" } },
            "destination": { "branch": { "name": "main" } },
            "comment_count": 1, "created_on": "2026-01-01T00:00:00Z",
            "updated_on": "2026-01-02T00:00:00Z", "description": "hello",
            "links": { "html": { "href": "https://bitbucket.org/ws/repo/pull-requests/7" } } }"#;
        let p = provider(Some("bb-tok"), vec![("/pullrequests/7", 200, body)]);
        let d = p.get_pr(7).unwrap();
        assert_eq!(d.summary.number, 7);
        assert_eq!(d.body, "hello");
        assert_eq!(d.mergeable, None);
        assert!(d.labels.is_empty());
        assert_eq!(d.summary.head_sha, "s");
    }

    #[test]
    fn create_pr_requires_token_and_posts_bitbucket_body() {
        let created = r#"{
            "id": 42, "title": "New", "state": "OPEN",
            "author": { "display_name": "Ada L" },
            "source": { "branch": { "name": "feature" }, "commit": { "hash": "s" } },
            "destination": { "branch": { "name": "main" } },
            "comment_count": 0, "created_on": "2026-01-01T00:00:00Z",
            "updated_on": "2026-01-01T00:00:00Z", "description": "",
            "links": { "html": { "href": "https://bitbucket.org/ws/repo/pull-requests/42" } } }"#;
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

        // Authenticated ⇒ POST with Bitbucket source/destination body.
        let (p, seen) = provider_spy(Some("bb-tok"), vec![("/pullrequests", 201, created)]);
        let d = p.create_pr(&input).unwrap();
        assert_eq!(d.summary.number, 42);
        let reqs = seen.lock().unwrap();
        assert_eq!(reqs[0].method, HttpMethod::Post);
        let sent: serde_json::Value = serde_json::from_str(reqs[0].body.as_ref().unwrap()).unwrap();
        assert_eq!(sent["source"]["branch"]["name"], "feature");
        assert_eq!(sent["destination"]["branch"]["name"], "main");
        assert_eq!(sent["title"], "New");
    }

    #[test]
    fn list_review_comments_splits_inline_and_general_sorted() {
        // A general (conversation) comment created later; an inline (review)
        // comment created earlier. Sorted by created_at ⇒ inline first.
        let body = r#"{
            "values": [
                { "id": 1, "content": { "raw": "top" }, "user": { "display_name": "a" },
                  "created_on": "2026-01-02T00:00:00Z",
                  "links": { "html": { "href": "https://x/1" } } },
                { "id": 2, "content": { "raw": "line note" }, "user": { "display_name": "b" },
                  "created_on": "2026-01-01T00:00:00Z",
                  "inline": { "path": "src/x.rs", "to": 3 },
                  "links": { "html": { "href": "https://x/2" } } }
            ]
        }"#;
        let p = provider(Some("bb-tok"), vec![("/comments", 200, body)]);
        let comments = p.list_review_comments(9).unwrap();
        assert_eq!(comments.len(), 2);
        assert_eq!(comments[0].id, 2, "earliest first after sort");
        assert_eq!(comments[0].kind, CommentKind::Review);
        assert_eq!(comments[0].path.as_deref(), Some("src/x.rs"));
        assert_eq!(comments[0].line, Some(3));
        assert_eq!(comments[1].id, 1);
        assert_eq!(comments[1].kind, CommentKind::Conversation);
    }

    #[test]
    fn combined_status_maps_build_vocabulary() {
        let body = r#"{
            "values": [
                { "key": "BUILD", "state": "SUCCESSFUL" },
                { "key": "TEST", "state": "FAILED" } ]
        }"#;
        let p = provider(Some("bb-tok"), vec![("/statuses", 200, body)]);
        let status = p.combined_status("sha1").unwrap();
        assert_eq!(status.state, CheckRollup::Failure);
        assert_eq!(status.total, 2);
        assert_eq!(status.passed, 1);
        assert_eq!(status.failed, 1);
    }

    #[test]
    fn commit_statuses_batch_omits_not_found_and_propagates_fatal() {
        let ok = r#"{ "values": [ { "key": "ci", "state": "SUCCESSFUL" } ] }"#;
        // aa11 resolves; bb22 404s (omitted).
        let p = provider(
            Some("bb-tok"),
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
        let p = provider(Some("bb-tok"), vec![("/pullrequests/1", 404, "{}")]);
        assert!(matches!(p.get_pr(1), Err(AppError::ForgeApi(_))));

        let p401 = provider(Some("bad"), vec![("/user", 401, "{}")]);
        assert!(matches!(p401.viewer(), Err(AppError::AuthFailed(_))));
    }

    #[test]
    fn unsupported_host_rejects_data_calls_but_gives_context() {
        let target = ForgeTarget {
            kind: ForgeKind::Unknown,
            host: "bitbucket.example.com".to_string(),
            owner: "o".to_string(),
            repo: "r".to_string(),
            project: None,
            web_url: "https://bitbucket.example.com/o/r".to_string(),
        };
        let transport = FakeTransport::with_seen(vec![], Arc::new(Mutex::new(Vec::new())));
        let p = BitbucketProvider::new(target, None, Box::new(transport));
        let ctx = p.repo_context();
        assert_eq!(ctx.provider, ForgeKind::Unknown);
        assert_eq!(ctx.host, "bitbucket.example.com");
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
