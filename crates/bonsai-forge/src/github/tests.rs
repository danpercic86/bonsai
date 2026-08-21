//! GitHub provider unit tests (split from mod.rs to keep it lean).

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
        project: None,
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

fn merge_input(method: crate::types::MergeMethod) -> MergePrInput {
    MergePrInput {
        method,
        commit_title: None,
        commit_message: None,
        delete_source_branch: false,
        head_sha: None,
    }
}

#[test]
fn merge_pr_puts_merge_method_and_refetches() {
    use crate::types::MergeMethod;
    let detail = r#"{
        "number": 7, "title": "Done", "state": "closed",
        "user": { "login": "a", "avatar_url": null },
        "head": { "ref": "f", "sha": "s" }, "base": { "ref": "main", "sha": "m" },
        "comments": 0, "created_at": "2026-01-01T00:00:00Z",
        "updated_at": "2026-01-02T00:00:00Z", "html_url": "https://x/7",
        "merged": true, "merged_at": "2026-01-03T00:00:00Z",
        "body": "", "mergeable": true, "additions": 0, "deletions": 0,
        "changed_files": 0, "labels": [] }"#;
    let (p, seen) = provider_spy(
        Some("tok"),
        vec![("/pulls/7/merge", 200, "{}"), ("/pulls/7", 200, detail)],
    );
    let d = p.merge_pr(7, &merge_input(MergeMethod::Squash)).unwrap();
    assert_eq!(d.summary.state, PrState::Merged);
    let reqs = seen.lock().unwrap();
    assert_eq!(reqs[0].method, HttpMethod::Put);
    assert!(reqs[0].url.contains("/pulls/7/merge"));
    let sent: serde_json::Value =
        serde_json::from_str(reqs[0].body.as_ref().unwrap()).unwrap();
    assert_eq!(sent["merge_method"], "squash");
}

#[test]
fn merge_pr_rejects_fast_forward_without_sending() {
    use crate::types::MergeMethod;
    let (p, seen) = provider_spy(Some("tok"), vec![("/pulls/7/merge", 200, "{}")]);
    assert!(matches!(
        p.merge_pr(7, &merge_input(MergeMethod::FastForward)),
        Err(AppError::ForgeApi(_))
    ));
    assert!(seen.lock().unwrap().is_empty(), "nothing sent for unsupported method");
}

#[test]
fn merge_pr_requires_token() {
    use crate::types::MergeMethod;
    let (p, seen) = provider_spy(None, vec![("/pulls/7/merge", 200, "{}")]);
    assert!(matches!(
        p.merge_pr(7, &merge_input(MergeMethod::Merge)),
        Err(AppError::ForgeAuthRequired(_))
    ));
    assert!(seen.lock().unwrap().is_empty());
}

#[test]
fn merge_pr_not_mergeable_maps_to_forge_api() {
    use crate::types::MergeMethod;
    let p = provider(Some("tok"), vec![("/pulls/7/merge", 405, "{}")]);
    match p.merge_pr(7, &merge_input(MergeMethod::Merge)) {
        Err(AppError::ForgeApi(m)) => assert!(m.contains("not mergeable"), "msg: {m}"),
        other => panic!("expected ForgeApi, got {other:?}"),
    }
    let p409 = provider(Some("tok"), vec![("/pulls/7/merge", 409, "{}")]);
    assert!(matches!(
        p409.merge_pr(7, &merge_input(MergeMethod::Merge)),
        Err(AppError::ForgeApi(_))
    ));
}

#[test]
fn close_pr_patches_state_closed() {
    let closed = r#"{
        "number": 7, "title": "Done", "state": "closed",
        "user": { "login": "a", "avatar_url": null },
        "head": { "ref": "f", "sha": "s" }, "base": { "ref": "main", "sha": "m" },
        "comments": 0, "created_at": "2026-01-01T00:00:00Z",
        "updated_at": "2026-01-02T00:00:00Z", "html_url": "https://x/7",
        "body": "", "mergeable": null, "additions": 0, "deletions": 0,
        "changed_files": 0, "labels": [] }"#;
    let (p, seen) = provider_spy(Some("tok"), vec![("/pulls/7", 200, closed)]);
    let d = p.close_pr(7).unwrap();
    assert_eq!(d.summary.state, PrState::Closed);
    let reqs = seen.lock().unwrap();
    assert_eq!(reqs[0].method, HttpMethod::Patch);
    let sent: serde_json::Value =
        serde_json::from_str(reqs[0].body.as_ref().unwrap()).unwrap();
    assert_eq!(sent["state"], "closed");
}

#[test]
fn close_pr_requires_token() {
    let (p, seen) = provider_spy(None, vec![("/pulls/7", 200, "{}")]);
    assert!(matches!(p.close_pr(7), Err(AppError::ForgeAuthRequired(_))));
    assert!(seen.lock().unwrap().is_empty());
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
fn commit_statuses_batch_resolves_each_sha() {
    // Two shas with DISTINCT rollups. Routes are keyed by "{sha}/status" +
    // "{sha}/check-runs" so each sha returns its own body (the URL is
    // `.../commits/{sha}/status`), letting us assert each resolves to its
    // own rollup (the hook keys by `status.sha`, so order is irrelevant).
    let ok = r#"{ "state": "success", "statuses": [
        { "state": "success", "context": "ci", "description": null, "target_url": null } ] }"#;
    let bad = r#"{ "state": "failure", "statuses": [
        { "state": "failure", "context": "ci", "description": null, "target_url": null } ] }"#;
    let no_checks = r#"{ "check_runs": [] }"#;
    let p = provider(
        Some("tok"),
        vec![
            ("aa11/status", 200, ok),
            ("aa11/check-runs", 200, no_checks),
            ("bb22/status", 200, bad),
            ("bb22/check-runs", 200, no_checks),
        ],
    );
    let shas = vec!["aa11".to_string(), "bb22".to_string()];
    let out = p.commit_statuses(&shas).unwrap();
    // Both shas resolve, each with its own rollup.
    assert_eq!(out.len(), 2);
    let find = |sha: &str| out.iter().find(|s| s.sha == sha);
    assert_eq!(find("aa11").unwrap().state, CheckRollup::Success);
    assert_eq!(find("bb22").unwrap().state, CheckRollup::Failure);
}

#[test]
fn commit_statuses_omits_not_found_and_propagates_fatal() {
    let ok = r#"{ "state": "success", "statuses": [
        { "state": "success", "context": "ci", "description": null, "target_url": null } ] }"#;
    let bad = r#"{ "state": "failure", "statuses": [
        { "state": "failure", "context": "ci", "description": null, "target_url": null } ] }"#;
    let no_checks = r#"{ "check_runs": [] }"#;

    // (a) 3 shas, one of which 404s on its status URL (not on the remote):
    // the two resolved come back with their rollups intact, the 404 sha is
    // OMITTED — not an error that nukes the whole batch.
    let p = provider(
        Some("tok"),
        vec![
            ("aa11/status", 200, ok),
            ("aa11/check-runs", 200, no_checks),
            ("bb22/status", 200, bad),
            ("bb22/check-runs", 200, no_checks),
            ("cc33/status", 404, "{}"), // 404 ⇒ ForgeApi ⇒ omit this sha
        ],
    );
    let shas = vec!["aa11".to_string(), "bb22".to_string(), "cc33".to_string()];
    let out = p.commit_statuses(&shas).unwrap();
    assert_eq!(out.len(), 2, "the 404 sha is omitted, the two resolved remain");
    let find = |sha: &str| out.iter().find(|s| s.sha == sha);
    assert_eq!(find("aa11").unwrap().state, CheckRollup::Success);
    assert_eq!(find("bb22").unwrap().state, CheckRollup::Failure);
    assert!(find("cc33").is_none(), "not-found sha omitted from the batch");

    // (b) a FATAL error (401 on a sha's status URL ⇒ AuthFailed) fails the
    // WHOLE batch — account/transport-level errors are not silently dropped.
    let p_fatal = provider(
        Some("bad"),
        vec![
            ("aa11/status", 200, ok),
            ("aa11/check-runs", 200, no_checks),
            ("bb22/status", 401, "{}"), // rejected token ⇒ AuthFailed ⇒ propagate
        ],
    );
    let shas2 = vec!["aa11".to_string(), "bb22".to_string()];
    let err = p_fatal.commit_statuses(&shas2).unwrap_err();
    assert!(matches!(err, AppError::AuthFailed(_)), "got {err:?}");
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
        project: None,
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
