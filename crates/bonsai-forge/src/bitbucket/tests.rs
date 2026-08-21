//! Bitbucket provider unit tests (split from mod.rs to keep it under the size limit).

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

fn merge_input(method: crate::types::MergeMethod) -> crate::types::MergePrInput {
    crate::types::MergePrInput {
        method,
        commit_title: None,
        commit_message: None,
        delete_source_branch: false,
        head_sha: None,
    }
}

const PR_MERGED: &str = r#"{
    "id": 7, "title": "Done", "state": "MERGED",
    "author": { "display_name": "Ada L" },
    "source": { "branch": { "name": "f" }, "commit": { "hash": "s" } },
    "destination": { "branch": { "name": "main" } },
    "comment_count": 0, "created_on": "2026-01-01T00:00:00Z",
    "updated_on": "2026-01-02T00:00:00Z", "description": "",
    "links": { "html": { "href": "https://bitbucket.org/ws/repo/pull-requests/7" } } }"#;

const PR_DECLINED: &str = r#"{
    "id": 7, "title": "Done", "state": "DECLINED",
    "author": { "display_name": "Ada L" },
    "source": { "branch": { "name": "f" }, "commit": { "hash": "s" } },
    "destination": { "branch": { "name": "main" } },
    "comment_count": 0, "created_on": "2026-01-01T00:00:00Z",
    "updated_on": "2026-01-02T00:00:00Z", "description": "",
    "links": { "html": { "href": "https://bitbucket.org/ws/repo/pull-requests/7" } } }"#;

#[test]
fn merge_pr_posts_merge_strategy_and_close_source_branch() {
    use crate::types::MergeMethod;
    let (p, seen) = provider_spy(Some("bb-tok"), vec![("/pullrequests/7/merge", 200, PR_MERGED)]);
    let mut input = merge_input(MergeMethod::Squash);
    input.delete_source_branch = true;
    input.commit_message = Some("squashed".to_string());
    let d = p.merge_pr(7, &input).unwrap();
    assert_eq!(d.summary.state, PrState::Merged);
    let reqs = seen.lock().unwrap();
    assert_eq!(reqs[0].method, HttpMethod::Post);
    assert!(reqs[0].url.contains("/pullrequests/7/merge"), "url: {}", reqs[0].url);
    let sent: serde_json::Value = serde_json::from_str(reqs[0].body.as_ref().unwrap()).unwrap();
    assert_eq!(sent["merge_strategy"], "squash");
    assert_eq!(sent["close_source_branch"], true);
    assert_eq!(sent["message"], "squashed");
    assert_eq!(sent["type"], "commit");
}

#[test]
fn merge_pr_maps_methods_to_strategies() {
    use crate::types::MergeMethod;
    for (m, strat) in [
        (MergeMethod::Merge, "merge_commit"),
        (MergeMethod::FastForward, "fast_forward"),
    ] {
        let (p, seen) = provider_spy(Some("bb-tok"), vec![("/pullrequests/7/merge", 200, PR_MERGED)]);
        p.merge_pr(7, &merge_input(m)).unwrap();
        let reqs = seen.lock().unwrap();
        let sent: serde_json::Value = serde_json::from_str(reqs[0].body.as_ref().unwrap()).unwrap();
        assert_eq!(sent["merge_strategy"], strat, "{m:?}");
    }
}

#[test]
fn merge_pr_rejects_rebase_without_sending() {
    use crate::types::MergeMethod;
    let (p, seen) = provider_spy(Some("bb-tok"), vec![("/pullrequests/7/merge", 200, PR_MERGED)]);
    assert!(matches!(
        p.merge_pr(7, &merge_input(MergeMethod::Rebase)),
        Err(AppError::ForgeApi(_))
    ));
    assert!(seen.lock().unwrap().is_empty(), "nothing sent for unsupported method");
}

#[test]
fn merge_pr_requires_token() {
    use crate::types::MergeMethod;
    let (p, seen) = provider_spy(None, vec![("/pullrequests/7/merge", 200, PR_MERGED)]);
    assert!(matches!(
        p.merge_pr(7, &merge_input(MergeMethod::Merge)),
        Err(AppError::ForgeAuthRequired(_))
    ));
    assert!(seen.lock().unwrap().is_empty());
}

#[test]
fn merge_pr_not_mergeable_maps_to_forge_api() {
    use crate::types::MergeMethod;
    for status in [400, 409] {
        let p = provider(Some("bb-tok"), vec![("/pullrequests/7/merge", status, "{}")]);
        match p.merge_pr(7, &merge_input(MergeMethod::Merge)) {
            Err(AppError::ForgeApi(m)) => assert!(m.contains("not mergeable"), "status {status}: {m}"),
            other => panic!("expected ForgeApi for {status}, got {other:?}"),
        }
    }
}

#[test]
fn close_pr_posts_decline_with_empty_body() {
    let (p, seen) = provider_spy(Some("bb-tok"), vec![("/pullrequests/7/decline", 200, PR_DECLINED)]);
    let d = p.close_pr(7).unwrap();
    assert_eq!(d.summary.state, PrState::Closed);
    let reqs = seen.lock().unwrap();
    assert_eq!(reqs[0].method, HttpMethod::Post);
    assert!(reqs[0].url.contains("/pullrequests/7/decline"), "url: {}", reqs[0].url);
    let sent: serde_json::Value = serde_json::from_str(reqs[0].body.as_ref().unwrap()).unwrap();
    assert!(sent.as_object().unwrap().is_empty(), "decline sends an empty body");
}

#[test]
fn close_pr_requires_token() {
    let (p, seen) = provider_spy(None, vec![("/pullrequests/7/decline", 200, PR_DECLINED)]);
    assert!(matches!(p.close_pr(7), Err(AppError::ForgeAuthRequired(_))));
    assert!(seen.lock().unwrap().is_empty());
}
