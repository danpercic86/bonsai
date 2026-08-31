//! Unit tests for the Bitbucket DTO mappers (extracted from `dto.rs` to keep
//! that file under the ~500-line soft limit). `tests` is a child module of
//! `dto`, so `use super::*;` resolves to `dto`'s items — including the private
//! `Bb*` wire structs and mapper fns.

use super::*;

#[test]
fn parse_pr_list_maps_fields_and_signals_next() {
    let body = r#"{
        "pagelen": 10, "page": 1, "size": 40,
        "next": "https://api.bitbucket.org/2.0/repositories/ws/repo/pullrequests?page=2",
        "values": [
            {
                "id": 12, "title": "Add feature", "state": "OPEN",
                "author": { "display_name": "Ada L", "username": "ada",
                            "links": { "avatar": { "href": "https://a/ada.png" } } },
                "source": { "branch": { "name": "feature" }, "commit": { "hash": "abc123" } },
                "destination": { "branch": { "name": "main" }, "commit": { "hash": "def456" } },
                "comment_count": 4, "created_on": "2026-01-01T00:00:00Z",
                "updated_on": "2026-01-02T00:00:00Z",
                "links": { "html": { "href": "https://bitbucket.org/ws/repo/pull-requests/12" } }
            }
        ]
    }"#;
    let (list, has_next) = parse_pr_list(body).unwrap();
    assert!(has_next, "a `next` URL ⇒ has_next");
    assert_eq!(list.len(), 1);
    let pr = &list[0];
    assert_eq!(pr.number, 12);
    assert_eq!(pr.state, PrState::Open);
    assert!(!pr.is_draft, "Bitbucket read model has no draft");
    assert_eq!(pr.author, "Ada L", "display_name preferred for authors");
    assert_eq!(pr.author_avatar_url.as_deref(), Some("https://a/ada.png"));
    assert_eq!(pr.source_branch, "feature");
    assert_eq!(pr.target_branch, "main");
    assert_eq!(pr.comments, 4);
    assert_eq!(pr.head_sha, "abc123", "source.commit.hash → headSha");
    assert_eq!(pr.url, "https://bitbucket.org/ws/repo/pull-requests/12");
}

#[test]
fn parse_pr_list_no_next_means_last_page() {
    let body = r#"{ "values": [] }"#;
    let (list, has_next) = parse_pr_list(body).unwrap();
    assert!(list.is_empty());
    assert!(!has_next, "absent `next` ⇒ last page");
}

#[test]
fn map_states_cover_all_arms() {
    assert_eq!(map_pr_state("OPEN"), PrState::Open);
    assert_eq!(map_pr_state("MERGED"), PrState::Merged);
    assert_eq!(map_pr_state("DECLINED"), PrState::Closed);
    assert_eq!(map_pr_state("SUPERSEDED"), PrState::Closed);
}

#[test]
fn parse_pr_detail_maps_body_and_unknown_mergeable() {
    let body = r#"{
        "id": 7, "title": "Done", "state": "MERGED",
        "author": { "display_name": "Ada L" },
        "source": { "branch": { "name": "feature" }, "commit": { "hash": "abc" } },
        "destination": { "branch": { "name": "main" } },
        "comment_count": 0, "created_on": "2026-01-01T00:00:00Z",
        "updated_on": "2026-01-02T00:00:00Z",
        "description": "The body",
        "links": { "html": { "href": "https://bitbucket.org/ws/repo/pull-requests/7" } }
    }"#;
    let d = parse_pr_detail(body).unwrap();
    assert_eq!(d.summary.state, PrState::Merged);
    assert_eq!(d.body, "The body");
    assert_eq!(d.mergeable, None, "Bitbucket doesn't expose mergeability");
    assert!(d.labels.is_empty(), "Bitbucket has no PR labels");
    // OQ-A2: diff stats stay 0 in v1.
    assert_eq!(d.additions, 0);
    assert_eq!(d.changed_files, 0);
}

#[test]
fn comments_split_inline_review_from_general_conversation() {
    let body = r#"{
        "values": [
            { "id": 1, "content": { "raw": "top-level" },
              "user": { "display_name": "a" },
              "created_on": "2026-01-01T00:00:00Z",
              "links": { "html": { "href": "https://x/1" } } },
            { "id": 2, "content": { "raw": "on a line" },
              "user": { "display_name": "b" },
              "created_on": "2026-01-02T00:00:00Z",
              "inline": { "path": "src/x.rs", "to": 42 },
              "links": { "html": { "href": "https://x/2" } } },
            { "id": 3, "content": { "raw": "gone" },
              "user": { "display_name": "c" },
              "created_on": "2026-01-03T00:00:00Z", "deleted": true }
        ]
    }"#;
    let comments = parse_comments(body).unwrap();
    assert_eq!(comments.len(), 2, "deleted comment dropped");
    // General comment ⇒ Conversation, no path.
    assert_eq!(comments[0].id, 1);
    assert_eq!(comments[0].kind, CommentKind::Conversation);
    assert_eq!(comments[0].path, None);
    assert_eq!(comments[0].body, "top-level");
    assert_eq!(comments[0].url, "https://x/1");
    // Inline comment ⇒ Review, carries path + line (`to`).
    assert_eq!(comments[1].id, 2);
    assert_eq!(comments[1].kind, CommentKind::Review);
    assert_eq!(comments[1].path.as_deref(), Some("src/x.rs"));
    assert_eq!(comments[1].line, Some(42));
}

#[test]
fn inline_line_falls_back_to_from_when_no_to() {
    let body = r#"{ "values": [
        { "id": 9, "content": { "raw": "old side" }, "user": { "display_name": "a" },
          "created_on": "t", "inline": { "path": "a.rs", "from": 7 } }
    ] }"#;
    let comments = parse_comments(body).unwrap();
    assert_eq!(comments[0].kind, CommentKind::Review);
    assert_eq!(comments[0].line, Some(7), "falls back to inline.from");
}

#[test]
fn parse_viewer_prefers_username() {
    let body = r#"{ "username": "ada", "display_name": "Ada L",
                    "links": { "avatar": { "href": "https://a/ada.png" } } }"#;
    let v = parse_viewer(body).unwrap();
    assert_eq!(v.login, "ada", "viewer login is username");
    assert_eq!(v.avatar_url.as_deref(), Some("https://a/ada.png"));
}

#[test]
fn create_pr_body_uses_source_destination_branch_shape() {
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
    assert_eq!(v["title"], "T");
    assert_eq!(v["description"], "B");
    assert_eq!(v["source"]["branch"]["name"], "feature");
    assert_eq!(v["destination"]["branch"]["name"], "main");
    assert_eq!(v["draft"], true);
    // No GitHub/GitLab keys leak onto the Bitbucket body.
    assert!(v.get("head").is_none());
    assert!(v.get("source_branch").is_none());
}

#[test]
fn build_commit_status_maps_vocabulary_and_rolls_up() {
    let body = r#"{
        "values": [
            { "key": "BUILD", "name": "build", "state": "SUCCESSFUL",
              "description": "ok", "url": "https://x/1" },
            { "key": "TEST", "state": "INPROGRESS", "url": null },
            { "key": "DEPLOY", "name": "deploy", "state": "STOPPED" }
        ]
    }"#;
    let status = build_commit_status("sha1", body).unwrap();
    assert_eq!(status.sha, "sha1");
    // success + inprogress(pending) + stopped(neutral) ⇒ overall Pending.
    assert_eq!(status.state, CheckRollup::Pending);
    assert_eq!(status.total, 3);
    assert_eq!(status.passed, 1);
    assert_eq!(status.pending, 1);
    assert_eq!(status.contexts.len(), 3);
    // name falls back to key when absent.
    assert_eq!(status.contexts[1].name, "TEST");
}

#[test]
fn build_state_vocabulary() {
    assert_eq!(normalize_build_state("SUCCESSFUL"), CheckRollup::Success);
    assert_eq!(normalize_build_state("INPROGRESS"), CheckRollup::Pending);
    assert_eq!(normalize_build_state("FAILED"), CheckRollup::Failure);
    assert_eq!(normalize_build_state("STOPPED"), CheckRollup::Neutral);
    assert_eq!(normalize_build_state("mystery"), CheckRollup::Error);
}

#[test]
fn malformed_body_is_forge_api_error() {
    let err = parse_pr_list("not json").unwrap_err();
    assert!(matches!(err, AppError::ForgeApi(_)), "got {err:?}");
}
