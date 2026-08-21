//! Azure provider tests for the PR/status/data surface. The `viewer()` contract
//! lives in `viewer_tests.rs`; the shared transport harness in `testkit.rs`.

use super::testkit::*;
use super::*;
use crate::http::HttpMethod;
use crate::types::{CheckRollup, CommentKind, PrState, PrStateFilter};
use std::sync::{Arc, Mutex};

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

    // `viewer()`'s status mapping now lives with the validate-then-identify
    // tests above (P72 cases d/e/f), which pin the request count too.
    let p401 = provider(Some("bad"), vec![(REPO_NEEDLE, 401, "{}")]);
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
    let seen: Spy = Arc::new(Mutex::new(Vec::new()));
    let transport = FakeTransport::with_seen(vec![], Arc::clone(&seen));
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
    // P72 case (k): `viewer()` rejects a non-Azure kind through `coords()` →
    // `require_supported()`, without issuing a request.
    assert!(matches!(p.viewer(), Err(AppError::ForgeUnsupported(_))));
    assert!(seen.lock().unwrap().is_empty(), "no request for an unsupported host");
}

fn merge_input(method: crate::types::MergeMethod) -> crate::types::MergePrInput {
    crate::types::MergePrInput {
        method,
        commit_title: None,
        commit_message: None,
        delete_source_branch: false,
        head_sha: Some("headsha".to_string()),
    }
}

const PR_COMPLETED: &str = r#"{
    "pullRequestId": 7, "title": "Done", "status": "completed", "isDraft": false,
    "createdBy": { "displayName": "Ada L" },
    "sourceRefName": "refs/heads/f", "targetRefName": "refs/heads/main",
    "creationDate": "2026-01-01T00:00:00Z",
    "lastMergeSourceCommit": { "commitId": "s" }, "description": "" }"#;

const PR_ABANDONED: &str = r#"{
    "pullRequestId": 7, "title": "Done", "status": "abandoned", "isDraft": false,
    "createdBy": { "displayName": "Ada L" },
    "sourceRefName": "refs/heads/f", "targetRefName": "refs/heads/main",
    "creationDate": "2026-01-01T00:00:00Z",
    "lastMergeSourceCommit": { "commitId": "s" }, "description": "" }"#;

#[test]
fn merge_pr_patches_completed_with_strategy_and_head_sha() {
    use crate::types::MergeMethod;
    let (p, seen) = provider_spy(Some("az-tok"), vec![("/pullrequests/7", 200, PR_COMPLETED)]);
    let mut input = merge_input(MergeMethod::Squash);
    input.delete_source_branch = true;
    let d = p.merge_pr(7, &input).unwrap();
    assert_eq!(d.summary.state, PrState::Merged);
    let reqs = seen.lock().unwrap();
    assert_eq!(reqs[0].method, HttpMethod::Patch);
    assert!(reqs[0].url.contains("/pullrequests/7"), "url: {}", reqs[0].url);
    let sent: serde_json::Value = serde_json::from_str(reqs[0].body.as_ref().unwrap()).unwrap();
    assert_eq!(sent["status"], "completed");
    assert_eq!(sent["lastMergeSourceCommit"]["commitId"], "headsha");
    assert_eq!(sent["completionOptions"]["mergeStrategy"], "squash");
    assert_eq!(sent["completionOptions"]["deleteSourceBranch"], true);
}

#[test]
fn merge_pr_maps_methods_to_strategies() {
    use crate::types::MergeMethod;
    for (m, strat) in [
        (MergeMethod::Merge, "noFastForward"),
        (MergeMethod::Rebase, "rebase"),
    ] {
        let (p, seen) = provider_spy(Some("az-tok"), vec![("/pullrequests/7", 200, PR_COMPLETED)]);
        p.merge_pr(7, &merge_input(m)).unwrap();
        let reqs = seen.lock().unwrap();
        let sent: serde_json::Value = serde_json::from_str(reqs[0].body.as_ref().unwrap()).unwrap();
        assert_eq!(sent["completionOptions"]["mergeStrategy"], strat, "{m:?}");
    }
}

#[test]
fn merge_pr_rejects_fast_forward_without_sending() {
    use crate::types::MergeMethod;
    let (p, seen) = provider_spy(Some("az-tok"), vec![("/pullrequests/7", 200, PR_COMPLETED)]);
    assert!(matches!(
        p.merge_pr(7, &merge_input(MergeMethod::FastForward)),
        Err(AppError::ForgeApi(_))
    ));
    assert!(seen.lock().unwrap().is_empty(), "nothing sent for unsupported method");
}

#[test]
fn merge_pr_missing_head_sha_errors_without_sending() {
    use crate::types::MergeMethod;
    let (p, seen) = provider_spy(Some("az-tok"), vec![("/pullrequests/7", 200, PR_COMPLETED)]);
    let mut input = merge_input(MergeMethod::Merge);
    input.head_sha = None;
    assert!(matches!(p.merge_pr(7, &input), Err(AppError::ForgeApi(_))));
    assert!(seen.lock().unwrap().is_empty(), "no request without a head sha");
}

#[test]
fn merge_pr_requires_token() {
    use crate::types::MergeMethod;
    let (p, seen) = provider_spy(None, vec![("/pullrequests/7", 200, PR_COMPLETED)]);
    assert!(matches!(
        p.merge_pr(7, &merge_input(MergeMethod::Merge)),
        Err(AppError::ForgeAuthRequired(_))
    ));
    assert!(seen.lock().unwrap().is_empty());
}

#[test]
fn merge_pr_not_completable_maps_to_forge_api() {
    use crate::types::MergeMethod;
    for status in [400, 409] {
        let p = provider(Some("az-tok"), vec![("/pullrequests/7", status, "{}")]);
        match p.merge_pr(7, &merge_input(MergeMethod::Merge)) {
            Err(AppError::ForgeApi(m)) => {
                assert!(m.contains("could not complete"), "status {status}: {m}")
            }
            other => panic!("expected ForgeApi for {status}, got {other:?}"),
        }
    }
}

#[test]
fn close_pr_patches_status_abandoned() {
    let (p, seen) = provider_spy(Some("az-tok"), vec![("/pullrequests/7", 200, PR_ABANDONED)]);
    let d = p.close_pr(7).unwrap();
    assert_eq!(d.summary.state, PrState::Closed);
    let reqs = seen.lock().unwrap();
    assert_eq!(reqs[0].method, HttpMethod::Patch);
    let sent: serde_json::Value = serde_json::from_str(reqs[0].body.as_ref().unwrap()).unwrap();
    assert_eq!(sent["status"], "abandoned");
}

#[test]
fn close_pr_requires_token() {
    let (p, seen) = provider_spy(None, vec![("/pullrequests/7", 200, PR_ABANDONED)]);
    assert!(matches!(p.close_pr(7), Err(AppError::ForgeAuthRequired(_))));
    assert!(seen.lock().unwrap().is_empty());
}
