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
    CommitStatus, CreatePrInput, ForgeKind, ForgeRepoContext, ForgeViewer, PrDetail, PrListQuery,
    PrPage, ReviewComment,
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

    /// A gitlab.com target with a NESTED group so tests also prove the project id
    /// is URL-encoded (`group%2Fsub%2Fproj`).
    fn gitlab_target() -> ForgeTarget {
        ForgeTarget {
            kind: ForgeKind::GitLab,
            host: "gitlab.com".to_string(),
            owner: "group/sub".to_string(),
            repo: "proj".to_string(),
            project: None,
            web_url: "https://gitlab.com/group/sub/proj".to_string(),
        }
    }

    fn provider_spy(token: Option<&str>, routes: Vec<(&str, u16, &str)>) -> (GitLabProvider, Spy) {
        let seen: Spy = Arc::new(Mutex::new(Vec::new()));
        let transport = FakeTransport::with_seen(routes, Arc::clone(&seen));
        let p = GitLabProvider::new(gitlab_target(), token.map(str::to_string), Box::new(transport));
        (p, seen)
    }

    fn provider(token: Option<&str>, routes: Vec<(&str, u16, &str)>) -> GitLabProvider {
        provider_spy(token, routes).0
    }

    #[test]
    fn viewer_maps_get_user_username() {
        let (p, seen) = provider_spy(
            Some("glpat"),
            vec![("/user", 200, r#"{ "username": "tanuki", "avatar_url": "https://a/t.png" }"#)],
        );
        let v = p.viewer().unwrap();
        assert_eq!(v.login, "tanuki");
        assert_eq!(v.avatar_url.as_deref(), Some("https://a/t.png"));
        // Auth uses the PRIVATE-TOKEN header, hits /api/v4/user.
        let reqs = seen.lock().unwrap();
        assert!(reqs[0].url.contains("/api/v4/user"), "url: {}", reqs[0].url);
        assert!(reqs[0]
            .headers
            .iter()
            .any(|(k, v)| k == "PRIVATE-TOKEN" && v == "glpat"));
    }

    #[test]
    fn viewer_requires_token() {
        let p = provider(None, vec![("/user", 200, "{}")]);
        assert!(matches!(p.viewer(), Err(AppError::ForgeAuthRequired(_))));
    }

    #[test]
    fn list_prs_caps_per_page_encodes_id_and_maps_iid() {
        let body = r#"[
            { "id": 500, "iid": 12, "title": "One", "state": "opened", "draft": false,
              "author": { "username": "a", "avatar_url": null },
              "source_branch": "f1", "target_branch": "main", "user_notes_count": 0,
              "created_at": "2026-01-01T00:00:00Z", "updated_at": "2026-01-01T00:00:00Z",
              "web_url": "https://gitlab.com/group/sub/proj/-/merge_requests/12", "sha": "s1" }
        ]"#;
        let (p, seen) = provider_spy(Some("glpat"), vec![("/merge_requests?", 200, body)]);
        let page = p
            .list_prs(&PrListQuery {
                state: PrStateFilter::Open,
                page: 1,
                per_page: 999,
            })
            .unwrap();
        assert_eq!(page.items.len(), 1);
        assert_eq!(page.items[0].number, 12, "number is iid, not id");
        assert_eq!(page.items[0].state, PrState::Open);
        assert!(!page.has_next, "no paging headers + short page ⇒ no next");
        let reqs = seen.lock().unwrap();
        // per_page clamped to 50; nested project id URL-encoded; state=opened.
        assert!(reqs[0].url.contains("per_page=50"), "url: {}", reqs[0].url);
        assert!(
            reqs[0].url.contains("/projects/group%2Fsub%2Fproj/merge_requests"),
            "url: {}",
            reqs[0].url
        );
        assert!(reqs[0].url.contains("state=opened"), "url: {}", reqs[0].url);
    }

    #[test]
    fn list_prs_works_unauthenticated() {
        let (p, seen) = provider_spy(None, vec![("/merge_requests?", 200, "[]")]);
        let page = p
            .list_prs(&PrListQuery {
                state: PrStateFilter::All,
                page: 1,
                per_page: 30,
            })
            .unwrap();
        assert_eq!(page.items.len(), 0);
        let reqs = seen.lock().unwrap();
        assert!(!reqs[0]
            .headers
            .iter()
            .any(|(k, _)| k.eq_ignore_ascii_case("PRIVATE-TOKEN")));
    }

    #[test]
    fn get_pr_maps_detail() {
        let body = r#"{
            "iid": 7, "title": "Done", "state": "opened", "draft": false,
            "author": { "username": "a", "avatar_url": null },
            "source_branch": "f", "target_branch": "main", "user_notes_count": 1,
            "created_at": "2026-01-01T00:00:00Z", "updated_at": "2026-01-02T00:00:00Z",
            "web_url": "https://gitlab.com/group/sub/proj/-/merge_requests/7",
            "sha": "s", "description": "hello", "detailed_merge_status": "mergeable",
            "labels": ["bug"] }"#;
        let p = provider(Some("glpat"), vec![("/merge_requests/7", 200, body)]);
        let d = p.get_pr(7).unwrap();
        assert_eq!(d.summary.number, 7);
        assert_eq!(d.body, "hello");
        assert_eq!(d.mergeable, Some(true));
        assert_eq!(d.labels, vec!["bug".to_string()]);
        assert_eq!(d.summary.head_sha, "s");
    }

    #[test]
    fn create_pr_requires_token_and_posts_gitlab_body() {
        let created = r#"{
            "iid": 42, "title": "Draft: New", "state": "opened", "draft": true,
            "author": { "username": "a", "avatar_url": null },
            "source_branch": "feature", "target_branch": "main", "user_notes_count": 0,
            "created_at": "2026-01-01T00:00:00Z", "updated_at": "2026-01-01T00:00:00Z",
            "web_url": "https://gitlab.com/group/sub/proj/-/merge_requests/42",
            "sha": "s", "description": "", "labels": [] }"#;
        let input = CreatePrInput {
            title: "New".into(),
            body: "".into(),
            source_branch: "feature".into(),
            target_branch: "main".into(),
            draft: true,
            maintainer_can_modify: true,
        };

        // Unauthenticated ⇒ ForgeAuthRequired, nothing sent.
        let (p_noauth, seen_noauth) = provider_spy(None, vec![("/merge_requests", 200, created)]);
        assert!(matches!(
            p_noauth.create_pr(&input),
            Err(AppError::ForgeAuthRequired(_))
        ));
        assert!(seen_noauth.lock().unwrap().is_empty(), "no request before auth check");

        // Authenticated ⇒ POST with GitLab body keys + draft title prefix.
        let (p, seen) = provider_spy(Some("glpat"), vec![("/merge_requests", 201, created)]);
        let d = p.create_pr(&input).unwrap();
        assert_eq!(d.summary.number, 42);
        assert!(d.summary.is_draft);
        let reqs = seen.lock().unwrap();
        assert_eq!(reqs[0].method, HttpMethod::Post);
        let sent: serde_json::Value = serde_json::from_str(reqs[0].body.as_ref().unwrap()).unwrap();
        assert_eq!(sent["source_branch"], "feature");
        assert_eq!(sent["target_branch"], "main");
        assert_eq!(sent["title"], "Draft: New", "draft prefixes the title");
    }

    #[test]
    fn list_review_comments_merges_notes_and_discussions_sorted() {
        // A conversation note (earlier) from /notes; a diff note (later) from
        // /discussions. Sorted by created_at ⇒ conversation first.
        let notes = r#"[
            { "id": 1, "body": "top", "author": { "username": "a" },
              "created_at": "2026-01-01T00:00:00Z", "system": false },
            { "id": 9, "body": "auto", "author": { "username": "sys" },
              "created_at": "2026-01-01T12:00:00Z", "system": true }
        ]"#;
        let discussions = r#"[
            { "id": "d1", "notes": [
                { "id": 2, "body": "line note", "author": { "username": "b" },
                  "created_at": "2026-01-02T00:00:00Z", "system": false,
                  "position": { "new_path": "src/x.rs", "new_line": 3 } }
            ] }
        ]"#;
        let p = provider(
            Some("glpat"),
            vec![("/notes", 200, notes), ("/discussions", 200, discussions)],
        );
        let comments = p.list_review_comments(9).unwrap();
        assert_eq!(comments.len(), 2, "system note dropped");
        assert_eq!(comments[0].id, 1);
        assert_eq!(comments[0].kind, CommentKind::Conversation);
        assert_eq!(comments[1].id, 2);
        assert_eq!(comments[1].kind, CommentKind::Review);
        assert_eq!(comments[1].path.as_deref(), Some("src/x.rs"));
        assert_eq!(comments[1].line, Some(3));
        assert_eq!(
            comments[1].url,
            "https://gitlab.com/group/sub/proj/-/merge_requests/9#note_2"
        );
    }

    #[test]
    fn combined_status_maps_pipeline_vocabulary() {
        let body = r#"[
            { "name": "build", "status": "success", "target_url": null },
            { "name": "test", "status": "failed", "target_url": null } ]"#;
        let p = provider(Some("glpat"), vec![("/statuses", 200, body)]);
        let status = p.combined_status("sha1").unwrap();
        assert_eq!(status.state, CheckRollup::Failure);
        assert_eq!(status.total, 2);
        assert_eq!(status.passed, 1);
        assert_eq!(status.failed, 1);
    }

    #[test]
    fn commit_statuses_batch_omits_not_found_and_propagates_fatal() {
        let ok = r#"[ { "name": "ci", "status": "success" } ]"#;
        // aa11 resolves; bb22 404s (omitted).
        let p = provider(
            Some("glpat"),
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
        let p = provider(Some("glpat"), vec![("/merge_requests/1", 404, "{}")]);
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
            project: None,
            web_url: "https://gitlab.example.com/o/r".to_string(),
        };
        let transport = FakeTransport::with_seen(vec![], Arc::new(Mutex::new(Vec::new())));
        let p = GitLabProvider::new(target, None, Box::new(transport));
        let ctx = p.repo_context();
        assert_eq!(ctx.provider, ForgeKind::Unknown);
        assert_eq!(ctx.host, "gitlab.example.com");
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
