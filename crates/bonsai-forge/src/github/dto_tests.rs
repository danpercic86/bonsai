//! GitHub dto unit tests (split from dto.rs to keep it under the size limit).

    use super::*;
    use crate::types::CheckRollup;

    #[test]
    fn build_combined_status_merges_both_sources() {
        let combined = r#"{
            "state": "success",
            "statuses": [
                { "state": "success", "context": "ci/lint", "description": "ok", "target_url": "https://x/1" }
            ]
        }"#;
        let checks = r#"{
            "total_count": 2,
            "check_runs": [
                { "name": "build", "status": "completed", "conclusion": "success", "details_url": "https://x/2" },
                { "name": "test", "status": "in_progress", "conclusion": null }
            ]
        }"#;
        let status = build_combined_status("abc123", combined, checks).unwrap();
        assert_eq!(status.sha, "abc123");
        // one pending check ⇒ overall pending (no failures present).
        assert_eq!(status.state, CheckRollup::Pending);
        assert_eq!(status.total, 3);
        assert_eq!(status.passed, 2);
        assert_eq!(status.pending, 1);
        assert_eq!(status.failed, 0);
        assert_eq!(status.contexts.len(), 3);
    }

    #[test]
    fn parse_pr_list_maps_fields() {
        let body = r#"[
            {
                "number": 12, "title": "Add feature", "state": "open", "draft": true,
                "user": { "login": "octocat", "avatar_url": "https://a/o.png" },
                "head": { "ref": "feature", "sha": "abc" },
                "base": { "ref": "main", "sha": "def" },
                "comments": 4, "created_at": "2026-01-01T00:00:00Z",
                "updated_at": "2026-01-02T00:00:00Z",
                "html_url": "https://github.com/o/r/pull/12",
                "merged_at": null
            }
        ]"#;
        let list = parse_pr_list(body).unwrap();
        assert_eq!(list.len(), 1);
        let pr = &list[0];
        assert_eq!(pr.number, 12);
        assert_eq!(pr.state, PrState::Open);
        assert!(pr.is_draft);
        assert_eq!(pr.author, "octocat");
        assert_eq!(pr.source_branch, "feature");
        assert_eq!(pr.target_branch, "main");
        assert_eq!(pr.head_sha, "abc");
        assert_eq!(pr.comments, 4);
    }

    #[test]
    fn parse_pr_detail_derives_merged_and_labels() {
        let body = r#"{
            "number": 7, "title": "Done", "state": "closed",
            "user": { "login": "octocat", "avatar_url": null },
            "head": { "ref": "feature", "sha": "abc" },
            "base": { "ref": "main", "sha": "def" },
            "comments": 0, "created_at": "2026-01-01T00:00:00Z",
            "updated_at": "2026-01-02T00:00:00Z",
            "html_url": "https://github.com/o/r/pull/7",
            "merged": true, "merged_at": "2026-01-03T00:00:00Z",
            "body": "The body", "mergeable": true,
            "additions": 10, "deletions": 3, "changed_files": 2,
            "labels": [ { "name": "bug" }, { "name": "urgent" } ]
        }"#;
        let d = parse_pr_detail(body).unwrap();
        assert_eq!(d.summary.state, PrState::Merged);
        assert_eq!(d.body, "The body");
        assert_eq!(d.mergeable, Some(true));
        assert_eq!(d.additions, 10);
        assert_eq!(d.deletions, 3);
        assert_eq!(d.changed_files, 2);
        assert_eq!(d.labels, vec!["bug".to_string(), "urgent".to_string()]);
    }

    #[test]
    fn parse_comments_tag_their_kind() {
        let review = r#"[
            { "id": 1, "user": { "login": "a", "avatar_url": null }, "body": "nit",
              "path": "src/x.rs", "line": null, "original_line": 42,
              "created_at": "2026-01-01T00:00:00Z", "html_url": "https://x/1" }
        ]"#;
        let issue = r#"[
            { "id": 2, "user": { "login": "b", "avatar_url": null }, "body": "lgtm",
              "created_at": "2026-01-02T00:00:00Z", "html_url": "https://x/2" }
        ]"#;
        let rc = parse_review_comments(review).unwrap();
        assert_eq!(rc.len(), 1);
        assert_eq!(rc[0].kind, CommentKind::Review);
        assert_eq!(rc[0].path.as_deref(), Some("src/x.rs"));
        assert_eq!(rc[0].line, Some(42), "falls back to original_line");

        let ic = parse_issue_comments(issue).unwrap();
        assert_eq!(ic[0].kind, CommentKind::Conversation);
        assert_eq!(ic[0].path, None);
    }

    #[test]
    fn create_pull_body_uses_head_and_base_keys() {
        let input = CreatePrInput {
            title: "T".into(),
            body: "B".into(),
            source_branch: "feature".into(),
            target_branch: "main".into(),
            draft: true,
            maintainer_can_modify: true,
        };
        let json = create_pull_body(&input).unwrap();
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v["title"], "T");
        assert_eq!(v["head"], "feature");
        assert_eq!(v["base"], "main");
        assert_eq!(v["draft"], true);
        assert_eq!(v["maintainer_can_modify"], true);
    }

    #[test]
    fn parse_pr_refs_builds_fetch_plan() {
        let body = r#"{
            "number": 42, "title": "T", "state": "open",
            "head": { "ref": "feature", "sha": "aaa" },
            "base": { "ref": "main", "sha": "bbb" },
            "comments": 0, "created_at": "x", "updated_at": "y",
            "html_url": "https://github.com/o/r/pull/42"
        }"#;
        let refs = parse_pr_refs(body, 42).unwrap();
        assert_eq!(refs.base_oid, "bbb");
        assert_eq!(refs.head_oid, "aaa");
        assert!(refs.base_fetch.url.is_none() && refs.head_fetch.url.is_none());
        assert_eq!(refs.base_fetch.refspec, "+refs/heads/main:refs/bonsai/pr/42/base");
        assert_eq!(refs.head_fetch.refspec, "+refs/pull/42/head:refs/bonsai/pr/42/head");
        assert_eq!(refs.base_fetch.resolve, "bbb");
        assert_eq!(refs.head_fetch.resolve, "aaa");
    }

    #[test]
    fn malformed_body_is_forge_api_error() {
        let err = parse_pr_list("not json").unwrap_err();
        assert!(matches!(err, AppError::ForgeApi(_)), "got {err:?}");
    }
