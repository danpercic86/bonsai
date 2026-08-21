//! GitLab dto unit tests (split from dto.rs to keep it under the size limit).

    use super::*;

    #[test]
    fn parse_mr_list_maps_fields_and_iid_is_number() {
        let body = r#"[
            {
                "id": 99999, "iid": 12, "title": "Add feature", "state": "opened",
                "draft": true, "work_in_progress": true,
                "author": { "username": "tanuki", "avatar_url": "https://a/t.png" },
                "source_branch": "feature", "target_branch": "main",
                "user_notes_count": 4, "created_at": "2026-01-01T00:00:00Z",
                "updated_at": "2026-01-02T00:00:00Z",
                "web_url": "https://gitlab.com/o/r/-/merge_requests/12",
                "sha": "abc123"
            }
        ]"#;
        let list = parse_mr_list(body).unwrap();
        assert_eq!(list.len(), 1);
        let mr = &list[0];
        // number is `iid` (12), NOT the global `id` (99999).
        assert_eq!(mr.number, 12);
        assert_eq!(mr.state, PrState::Open);
        assert!(mr.is_draft);
        assert_eq!(mr.author, "tanuki");
        assert_eq!(mr.author_avatar_url.as_deref(), Some("https://a/t.png"));
        assert_eq!(mr.source_branch, "feature");
        assert_eq!(mr.target_branch, "main");
        assert_eq!(mr.comments, 4);
        assert_eq!(mr.head_sha, "abc123");
        assert_eq!(mr.url, "https://gitlab.com/o/r/-/merge_requests/12");
    }

    #[test]
    fn draft_via_legacy_work_in_progress_flag() {
        let body = r#"[
            { "iid": 1, "title": "WIP thing", "state": "opened", "draft": false,
              "work_in_progress": true, "source_branch": "wip", "target_branch": "main",
              "created_at": "t", "updated_at": "t",
              "web_url": "https://gitlab.com/o/r/-/merge_requests/1", "sha": "s" }
        ]"#;
        assert!(parse_mr_list(body).unwrap()[0].is_draft);
    }

    #[test]
    fn map_states_cover_all_arms() {
        assert_eq!(map_mr_state("opened"), PrState::Open);
        assert_eq!(map_mr_state("merged"), PrState::Merged);
        assert_eq!(map_mr_state("closed"), PrState::Closed);
        assert_eq!(map_mr_state("locked"), PrState::Closed);
    }

    #[test]
    fn parse_mr_detail_maps_body_labels_and_mergeable() {
        let body = r#"{
            "iid": 7, "title": "Done", "state": "merged", "draft": false,
            "author": { "username": "tanuki", "avatar_url": null },
            "source_branch": "feature", "target_branch": "main",
            "user_notes_count": 0, "created_at": "2026-01-01T00:00:00Z",
            "updated_at": "2026-01-02T00:00:00Z",
            "web_url": "https://gitlab.com/o/r/-/merge_requests/7",
            "sha": "abc", "description": "The body",
            "detailed_merge_status": "mergeable",
            "labels": ["bug", "urgent"]
        }"#;
        let d = parse_mr_detail(body).unwrap();
        assert_eq!(d.summary.state, PrState::Merged);
        assert_eq!(d.body, "The body");
        assert_eq!(d.mergeable, Some(true));
        assert_eq!(d.labels, vec!["bug".to_string(), "urgent".to_string()]);
        // OQ-A2: diff stats stay 0 in v1.
        assert_eq!(d.additions, 0);
        assert_eq!(d.changed_files, 0);
    }

    #[test]
    fn mergeable_conflict_and_checking() {
        assert_eq!(map_mergeable(Some("mergeable")), Some(true));
        assert_eq!(map_mergeable(Some("conflict")), Some(false));
        assert_eq!(map_mergeable(Some("checking")), None);
        assert_eq!(map_mergeable(Some("ci_must_pass")), None);
        assert_eq!(map_mergeable(None), None);
    }

    #[test]
    fn create_mr_body_uses_source_target_and_draft_prefix() {
        let base = CreatePrInput {
            title: "T".into(),
            body: "B".into(),
            source_branch: "feature".into(),
            target_branch: "main".into(),
            draft: false,
            maintainer_can_modify: true,
        };
        let json = create_mr_body(&base).unwrap();
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v["source_branch"], "feature");
        assert_eq!(v["target_branch"], "main");
        assert_eq!(v["title"], "T");
        assert_eq!(v["description"], "B");
        // No `draft`/`head`/`base` keys on the GitLab body.
        assert!(v.get("draft").is_none());

        let draft = CreatePrInput { draft: true, ..base };
        let vj: serde_json::Value = serde_json::from_str(&create_mr_body(&draft).unwrap()).unwrap();
        assert_eq!(vj["title"], "Draft: T", "draft prefixes the title");
    }

    #[test]
    fn notes_are_conversation_discussions_are_review_no_duplicates() {
        // `/notes` carries a conversation note, a diff note (has position), and a
        // system note. Only the conversation note survives here.
        let notes = r#"[
            { "id": 1, "body": "top-level", "author": { "username": "a" },
              "created_at": "2026-01-01T00:00:00Z", "system": false },
            { "id": 2, "body": "on a line", "author": { "username": "b" },
              "created_at": "2026-01-02T00:00:00Z", "system": false,
              "position": { "new_path": "src/x.rs", "new_line": 42 } },
            { "id": 3, "body": "changed the title", "author": { "username": "sys" },
              "created_at": "2026-01-03T00:00:00Z", "system": true }
        ]"#;
        let mr_url = "https://gitlab.com/o/r/-/merge_requests/9";
        let conv = parse_notes(notes, mr_url).unwrap();
        assert_eq!(conv.len(), 1, "only the non-system, non-diff note");
        assert_eq!(conv[0].id, 1);
        assert_eq!(conv[0].kind, CommentKind::Conversation);
        assert_eq!(conv[0].path, None);
        assert_eq!(conv[0].url, format!("{mr_url}#note_1"));

        // `/discussions` yields ONLY the diff note as a Review comment.
        let discussions = r#"[
            { "id": "d1", "notes": [
                { "id": 2, "body": "on a line", "author": { "username": "b" },
                  "created_at": "2026-01-02T00:00:00Z", "system": false,
                  "position": { "new_path": "src/x.rs", "new_line": 42 } }
            ] },
            { "id": "d2", "notes": [
                { "id": 4, "body": "resolved thread note", "author": { "username": "c" },
                  "created_at": "2026-01-04T00:00:00Z", "system": true }
            ] }
        ]"#;
        let review = parse_discussions(discussions, mr_url).unwrap();
        assert_eq!(review.len(), 1, "only the diff note, no system note");
        assert_eq!(review[0].id, 2);
        assert_eq!(review[0].kind, CommentKind::Review);
        assert_eq!(review[0].path.as_deref(), Some("src/x.rs"));
        assert_eq!(review[0].line, Some(42));
    }

    #[test]
    fn build_pipeline_status_maps_vocabulary_and_rolls_up() {
        let body = r#"[
            { "name": "build", "status": "success", "description": "ok",
              "target_url": "https://x/1" },
            { "name": "test", "status": "running", "target_url": null },
            { "name": "lint", "status": "skipped" }
        ]"#;
        let status = build_pipeline_status("sha1", body).unwrap();
        assert_eq!(status.sha, "sha1");
        // success + running(pending) + skipped(neutral) ⇒ overall Pending.
        assert_eq!(status.state, CheckRollup::Pending);
        assert_eq!(status.total, 3);
        assert_eq!(status.passed, 1);
        assert_eq!(status.pending, 1);
        assert_eq!(status.contexts.len(), 3);
    }

    #[test]
    fn pipeline_state_vocabulary() {
        assert_eq!(normalize_pipeline_state("success"), CheckRollup::Success);
        for s in ["running", "pending", "created"] {
            assert_eq!(normalize_pipeline_state(s), CheckRollup::Pending, "{s}");
        }
        assert_eq!(normalize_pipeline_state("failed"), CheckRollup::Failure);
        for s in ["canceled", "skipped", "manual"] {
            assert_eq!(normalize_pipeline_state(s), CheckRollup::Neutral, "{s}");
        }
        assert_eq!(normalize_pipeline_state("mystery"), CheckRollup::Error);
    }

    #[test]
    fn malformed_body_is_forge_api_error() {
        let err = parse_mr_list("not json").unwrap_err();
        assert!(matches!(err, AppError::ForgeApi(_)), "got {err:?}");
    }
