//! Unit tests for the Azure DevOps DTO mappers (extracted from `dto.rs` to keep
//! that file under the ~500-line soft limit). `tests` is a child module of
//! `dto`, so `use super::*;` resolves to `dto`'s items — including the private
//! `Az*` wire structs and mapper fns.

use super::*;

/// The repo browser-URL base the provider passes for building PR/comment URLs.
const WEB_BASE: &str = "https://dev.azure.com/org/proj/_git/repo";

#[test]
fn parse_pr_list_maps_fields_strips_refs_and_builds_url() {
    let body = r#"{
        "count": 1,
        "value": [
            {
                "pullRequestId": 12, "title": "Add feature", "status": "active",
                "isDraft": true,
                "createdBy": { "displayName": "Ada L", "imageUrl": "https://a/ada.png" },
                "sourceRefName": "refs/heads/feature",
                "targetRefName": "refs/heads/main",
                "creationDate": "2026-01-01T00:00:00Z",
                "lastMergeSourceCommit": { "commitId": "abc123" }
            }
        ]
    }"#;
    let list = parse_pr_list(body, WEB_BASE).unwrap();
    assert_eq!(list.len(), 1);
    let pr = &list[0];
    assert_eq!(pr.number, 12, "number is pullRequestId");
    assert_eq!(pr.state, PrState::Open);
    assert!(pr.is_draft);
    assert_eq!(pr.author, "Ada L");
    assert_eq!(pr.author_avatar_url.as_deref(), Some("https://a/ada.png"));
    // refs/heads/ stripped for the neutral branch names.
    assert_eq!(pr.source_branch, "feature");
    assert_eq!(pr.target_branch, "main");
    assert_eq!(pr.head_sha, "abc123", "lastMergeSourceCommit.commitId → headSha");
    // Browser URL synthesized from the repo base + pullRequestId.
    assert_eq!(pr.url, "https://dev.azure.com/org/proj/_git/repo/pullrequest/12");
    // No comment count in the base payload ⇒ 0 (OQ-A2).
    assert_eq!(pr.comments, 0);
}

#[test]
fn parse_pr_list_empty_envelope() {
    assert!(parse_pr_list(r#"{ "count": 0, "value": [] }"#, WEB_BASE).unwrap().is_empty());
    // Missing `value` also parses (defaulted empty).
    assert!(parse_pr_list(r#"{ "count": 0 }"#, WEB_BASE).unwrap().is_empty());
}

#[test]
fn map_states_cover_all_arms() {
    assert_eq!(map_pr_state("active"), PrState::Open);
    assert_eq!(map_pr_state("completed"), PrState::Merged);
    assert_eq!(map_pr_state("abandoned"), PrState::Closed);
    // notSet / unknown default to Open.
    assert_eq!(map_pr_state("notSet"), PrState::Open);
}

#[test]
fn parse_pr_detail_maps_body_labels_and_mergeable() {
    let body = r#"{
        "pullRequestId": 7, "title": "Done", "status": "completed",
        "isDraft": false,
        "createdBy": { "displayName": "Ada L" },
        "sourceRefName": "refs/heads/feature",
        "targetRefName": "refs/heads/main",
        "creationDate": "2026-01-01T00:00:00Z",
        "closedDate": "2026-01-02T00:00:00Z",
        "lastMergeSourceCommit": { "commitId": "abc" },
        "description": "The body",
        "mergeStatus": "succeeded",
        "labels": [ { "name": "bug" }, { "name": "urgent" } ]
    }"#;
    let d = parse_pr_detail(body, WEB_BASE).unwrap();
    assert_eq!(d.summary.state, PrState::Merged);
    assert_eq!(d.body, "The body");
    assert_eq!(d.mergeable, Some(true));
    assert_eq!(d.labels, vec!["bug".to_string(), "urgent".to_string()]);
    // closedDate becomes the neutral updated_at when present.
    assert_eq!(d.summary.updated_at, "2026-01-02T00:00:00Z");
    // OQ-A2: diff stats stay 0 in v1.
    assert_eq!(d.additions, 0);
    assert_eq!(d.changed_files, 0);
}

#[test]
fn mergeable_conflicts_and_queued() {
    assert_eq!(map_mergeable(Some("succeeded")), Some(true));
    assert_eq!(map_mergeable(Some("conflicts")), Some(false));
    assert_eq!(map_mergeable(Some("queued")), None);
    assert_eq!(map_mergeable(Some("notSet")), None);
    assert_eq!(map_mergeable(None), None);
}

#[test]
fn parse_threads_splits_review_from_conversation_skips_system_and_deleted() {
    let body = r#"{
        "count": 3,
        "value": [
            {
                "id": 1,
                "threadContext": null,
                "comments": [
                    { "id": 1, "content": "top-level", "author": { "displayName": "a" },
                      "publishedDate": "2026-01-01T00:00:00Z", "commentType": "text" }
                ]
            },
            {
                "id": 2,
                "threadContext": {
                    "filePath": "/src/x.rs",
                    "rightFileStart": { "line": 42, "offset": 1 }
                },
                "comments": [
                    { "id": 1, "content": "on a line", "author": { "displayName": "b" },
                      "publishedDate": "2026-01-02T00:00:00Z", "commentType": "text" },
                    { "id": 2, "content": "gone", "author": { "displayName": "c" },
                      "publishedDate": "2026-01-02T01:00:00Z", "commentType": "text",
                      "isDeleted": true }
                ]
            },
            {
                "id": 3,
                "comments": [
                    { "id": 1, "content": "auto event", "author": { "displayName": "sys" },
                      "publishedDate": "2026-01-03T00:00:00Z", "commentType": "system" }
                ]
            }
        ]
    }"#;
    let comments = parse_threads(body, "https://dev.azure.com/org/proj/_git/repo/pullrequest/9").unwrap();
    // The system comment and the deleted comment are dropped ⇒ 2 remain.
    assert_eq!(comments.len(), 2);

    // Thread 1: a conversation comment; id = 1*1000 + 1.
    let conv = &comments[0];
    assert_eq!(conv.kind, CommentKind::Conversation);
    assert_eq!(conv.id, 1001);
    assert_eq!(conv.path, None);
    assert_eq!(conv.body, "top-level");
    assert_eq!(conv.url, "https://dev.azure.com/org/proj/_git/repo/pullrequest/9?discussionId=1");

    // Thread 2: a review comment carrying the file path + right line; id = 2*1000 + 1.
    let review = &comments[1];
    assert_eq!(review.kind, CommentKind::Review);
    assert_eq!(review.id, 2001);
    assert_eq!(review.path.as_deref(), Some("/src/x.rs"));
    assert_eq!(review.line, Some(42));
}

#[test]
fn parse_threads_line_falls_back_to_left_side() {
    let body = r#"{ "value": [
        { "id": 5, "threadContext": { "filePath": "a.rs", "leftFileStart": { "line": 7 } },
          "comments": [ { "id": 1, "content": "old side", "author": { "displayName": "a" },
                          "publishedDate": "t", "commentType": "text" } ] }
    ] }"#;
    let comments = parse_threads(body, "https://x/pullrequest/1").unwrap();
    assert_eq!(comments.len(), 1);
    assert_eq!(comments[0].kind, CommentKind::Review);
    assert_eq!(comments[0].line, Some(7), "falls back to leftFileStart.line");
}

#[test]
fn parse_viewer_uses_display_name_no_avatar() {
    let body = r#"{ "displayName": "Ada Lovelace", "emailAddress": "ada@example.com",
                    "id": "guid" }"#;
    let v = parse_viewer(body).unwrap();
    assert_eq!(v.login, "Ada Lovelace", "login is displayName");
    assert_eq!(v.avatar_url, None, "Azure profile carries no avatar (contract)");
}

#[test]
fn create_pr_body_readds_refs_and_sets_is_draft() {
    let input = CreatePrInput {
        title: "T".into(),
        body: "B".into(),
        source_branch: "feature".into(),
        target_branch: "main".into(),
        draft: true,
        maintainer_can_modify: true,
    };
    let json = create_pr_body(&input).unwrap();
    let v: serde_json::Value = serde_json::from_str(&json).unwrap();
    // refs/heads/ re-added; camelCase keys; isDraft honored.
    assert_eq!(v["sourceRefName"], "refs/heads/feature");
    assert_eq!(v["targetRefName"], "refs/heads/main");
    assert_eq!(v["title"], "T");
    assert_eq!(v["description"], "B");
    assert_eq!(v["isDraft"], true);
    // No GitHub/GitLab/Bitbucket keys leak onto the Azure body.
    assert!(v.get("head").is_none());
    assert!(v.get("source_branch").is_none());
    assert!(v.get("source").is_none());
}

#[test]
fn create_pr_body_preserves_already_qualified_ref() {
    let input = CreatePrInput {
        title: "T".into(),
        body: "".into(),
        source_branch: "refs/heads/already".into(),
        target_branch: "main".into(),
        draft: false,
        maintainer_can_modify: true,
    };
    let v: serde_json::Value = serde_json::from_str(&create_pr_body(&input).unwrap()).unwrap();
    assert_eq!(v["sourceRefName"], "refs/heads/already", "no double prefix");
    assert_eq!(v["isDraft"], false);
}

#[test]
fn build_commit_status_maps_vocabulary_and_rolls_up() {
    let body = r#"{
        "count": 3,
        "value": [
            { "state": "succeeded", "description": "ok",
              "targetUrl": "https://x/1",
              "context": { "name": "ci-build", "genre": "continuous-integration" } },
            { "state": "pending", "context": { "genre": "cd" } },
            { "state": "notApplicable", "context": { "name": "optional" } }
        ]
    }"#;
    let status = build_commit_status("sha1", body).unwrap();
    assert_eq!(status.sha, "sha1");
    // succeeded + pending + notApplicable(neutral) ⇒ overall Pending.
    assert_eq!(status.state, CheckRollup::Pending);
    assert_eq!(status.total, 3);
    assert_eq!(status.passed, 1);
    assert_eq!(status.pending, 1);
    assert_eq!(status.contexts.len(), 3);
    // name prefers `name`, else falls back to `genre`.
    assert_eq!(status.contexts[0].name, "ci-build");
    assert_eq!(status.contexts[1].name, "cd");
}

#[test]
fn status_state_vocabulary() {
    assert_eq!(normalize_status_state("succeeded"), CheckRollup::Success);
    assert_eq!(normalize_status_state("pending"), CheckRollup::Pending);
    assert_eq!(normalize_status_state("failed"), CheckRollup::Failure);
    assert_eq!(normalize_status_state("error"), CheckRollup::Failure);
    assert_eq!(normalize_status_state("notApplicable"), CheckRollup::Neutral);
    assert_eq!(normalize_status_state("notSet"), CheckRollup::Neutral);
    assert_eq!(normalize_status_state("mystery"), CheckRollup::Error);
}

#[test]
fn malformed_body_is_forge_api_error() {
    let err = parse_pr_list("not json", WEB_BASE).unwrap_err();
    assert!(matches!(err, AppError::ForgeApi(_)), "got {err:?}");
}
