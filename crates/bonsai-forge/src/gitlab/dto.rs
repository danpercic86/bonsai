//! GitLab REST v4 wire structs + mappers to provider-neutral [`crate::types`].
//!
//! GitLab JSON NEVER escapes this file: every function here takes a raw body
//! string (or a `CreatePrInput`) and returns a neutral DTO, so the `Gl*` structs
//! stay private and the rest of the crate speaks only `types`. The pipeline →
//! [`crate::types::CommitStatus`] mapping delegates the precedence + counts +
//! cap to the shared [`crate::rollup::build_commit_status`].

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use bonsai_core::error::AppError;

use crate::types::{
    CheckRollup, CommentKind, CommitStatus, CreatePrInput, ForgeViewer, PrDetail, PrState,
    PrSummary, ReviewComment, StatusContext,
};

/// Parse a GitLab body into `T`; a malformed body ⇒ `ForgeApi` (never a token).
fn from_json<T: DeserializeOwned>(body: &str) -> Result<T, AppError> {
    serde_json::from_str(body)
        .map_err(|e| AppError::ForgeApi(format!("malformed GitLab response: {e}")))
}

// ---- wire structs (private; snake_case exactly as GitLab sends) ----

#[derive(Deserialize)]
struct GlUser {
    username: String,
    #[serde(default)]
    avatar_url: Option<String>,
}

fn author_of(user: &Option<GlUser>) -> (String, Option<String>) {
    match user {
        Some(u) => (u.username.clone(), u.avatar_url.clone()),
        None => (String::new(), None),
    }
}

/// A merge request. The list and detail endpoints return the SAME object; the
/// detail-only fields (`description`, `detailed_merge_status`, `labels`) are
/// absent in list responses ⇒ defaulted.
#[derive(Deserialize)]
struct GlMergeRequest {
    /// Project-scoped id (what appears in the MR URL) — NOT the global `id`.
    iid: u64,
    title: String,
    /// `opened` | `closed` | `merged` | `locked`.
    state: String,
    #[serde(default)]
    draft: bool,
    /// Legacy draft flag (pre-`draft`); either ⇒ draft.
    #[serde(default)]
    work_in_progress: bool,
    #[serde(default)]
    author: Option<GlUser>,
    source_branch: String,
    target_branch: String,
    #[serde(default)]
    user_notes_count: u32,
    created_at: String,
    updated_at: String,
    web_url: String,
    /// Head sha of the source branch (P63 needs it); occasionally null.
    #[serde(default)]
    sha: Option<String>,
    // ---- detail-only ----
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    detailed_merge_status: Option<String>,
    #[serde(default)]
    labels: Vec<String>,
}

impl GlMergeRequest {
    fn to_summary(&self) -> PrSummary {
        let (author, author_avatar_url) = author_of(&self.author);
        PrSummary {
            number: self.iid,
            title: self.title.clone(),
            state: map_mr_state(&self.state),
            is_draft: self.draft || self.work_in_progress,
            author,
            author_avatar_url,
            source_branch: self.source_branch.clone(),
            target_branch: self.target_branch.clone(),
            comments: self.user_notes_count,
            created_at: self.created_at.clone(),
            updated_at: self.updated_at.clone(),
            url: self.web_url.clone(),
            head_sha: self.sha.clone().unwrap_or_default(),
        }
    }

    fn into_detail(self) -> PrDetail {
        let summary = self.to_summary();
        PrDetail {
            summary,
            body: self.description.unwrap_or_default(),
            mergeable: map_mergeable(self.detailed_merge_status.as_deref()),
            // OQ-A2: v1 leaves diff stats at 0 rather than firing a heavy
            // `/changes` call per MR.
            additions: 0,
            deletions: 0,
            changed_files: 0,
            labels: self.labels,
        }
    }
}

/// GitLab MR state → neutral. `locked` maps to `Closed` (no neutral equivalent).
fn map_mr_state(state: &str) -> PrState {
    match state {
        "merged" => PrState::Merged,
        "closed" | "locked" => PrState::Closed,
        _ => PrState::Open, // "opened"
    }
}

/// `detailed_merge_status` → `mergeable`. Only a definite answer becomes
/// `Some`; anything still computing / blocked for a non-conflict reason is
/// `None` (unknown), matching GitHub's "still computing" semantics (contract).
fn map_mergeable(status: Option<&str>) -> Option<bool> {
    match status {
        Some("mergeable") => Some(true),
        Some("conflict") | Some("broken_status") => Some(false),
        _ => None,
    }
}

/// A note on an MR (from `/notes` or inside a `/discussions` entry). Diff notes
/// carry a `position`; conversation notes do not. `system` notes (auto events
/// like "changed the title") are dropped.
#[derive(Deserialize)]
struct GlNote {
    id: u64,
    body: String,
    #[serde(default)]
    author: Option<GlUser>,
    created_at: String,
    #[serde(default)]
    system: bool,
    #[serde(default)]
    position: Option<GlPosition>,
}

#[derive(Deserialize)]
struct GlPosition {
    #[serde(default)]
    new_path: Option<String>,
    #[serde(default)]
    old_path: Option<String>,
    #[serde(default)]
    new_line: Option<u32>,
    #[serde(default)]
    old_line: Option<u32>,
}

#[derive(Deserialize)]
struct GlDiscussion {
    #[serde(default)]
    notes: Vec<GlNote>,
}

impl GlNote {
    /// Build a neutral comment. `url` anchors the note within the MR page.
    fn to_comment(&self, mr_web_url: &str, kind: CommentKind) -> ReviewComment {
        let (author, author_avatar_url) = author_of(&self.author);
        let (path, line) = match (&self.position, kind) {
            (Some(p), CommentKind::Review) => (
                p.new_path.clone().or_else(|| p.old_path.clone()),
                p.new_line.or(p.old_line),
            ),
            _ => (None, None),
        };
        ReviewComment {
            id: self.id,
            author,
            author_avatar_url,
            body: self.body.clone(),
            path,
            line,
            created_at: self.created_at.clone(),
            url: format!("{mr_web_url}#note_{}", self.id),
            kind,
        }
    }
}

// ---- pipeline / commit-status wire structs ----

#[derive(Deserialize)]
struct GlCommitStatus {
    #[serde(default)]
    name: Option<String>,
    /// `success|failed|running|pending|created|canceled|skipped|manual`.
    status: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    target_url: Option<String>,
}

/// GitLab pipeline/commit-status state → neutral [`CheckRollup`] (contract §3c):
/// `success→Success`, `running|pending|created→Pending`, `failed→Failure`,
/// `canceled|skipped|manual→Neutral`, else `Error`.
fn normalize_pipeline_state(state: &str) -> CheckRollup {
    match state {
        "success" => CheckRollup::Success,
        "running" | "pending" | "created" => CheckRollup::Pending,
        "failed" => CheckRollup::Failure,
        "canceled" | "skipped" | "manual" => CheckRollup::Neutral,
        _ => CheckRollup::Error,
    }
}

// ---- create-MR request body (snake_case exactly as GitLab expects) ----

#[derive(Serialize)]
struct CreateMrWire<'a> {
    source_branch: &'a str,
    target_branch: &'a str,
    /// GitLab has no `draft` param on create — the "Draft: " title prefix is the
    /// convention (contract §3c).
    title: String,
    description: &'a str,
}

// ---- public boundary: raw body ⇒ neutral DTO ----

/// `GET /user` ⇒ [`ForgeViewer`] (`username`→login, `avatar_url`→avatarUrl).
pub fn parse_viewer(body: &str) -> Result<ForgeViewer, AppError> {
    let u: GlUser = from_json(body)?;
    Ok(ForgeViewer {
        login: u.username,
        avatar_url: u.avatar_url,
    })
}

/// `GET …/merge_requests` ⇒ `Vec<PrSummary>`.
pub fn parse_mr_list(body: &str) -> Result<Vec<PrSummary>, AppError> {
    let mrs: Vec<GlMergeRequest> = from_json(body)?;
    Ok(mrs.iter().map(GlMergeRequest::to_summary).collect())
}

/// `GET …/merge_requests/{iid}` (or the POST response) ⇒ [`PrDetail`].
pub fn parse_mr_detail(body: &str) -> Result<PrDetail, AppError> {
    let mr: GlMergeRequest = from_json(body)?;
    Ok(mr.into_detail())
}

/// `GET …/merge_requests/{iid}/notes` ⇒ conversation comments. Diff notes (they
/// carry a `position`) and system notes are dropped here — the diff notes come
/// from [`parse_discussions`] as `Review`, so nothing is double-counted.
pub fn parse_notes(body: &str, mr_web_url: &str) -> Result<Vec<ReviewComment>, AppError> {
    let notes: Vec<GlNote> = from_json(body)?;
    Ok(notes
        .into_iter()
        .filter(|n| !n.system && n.position.is_none())
        .map(|n| n.to_comment(mr_web_url, CommentKind::Conversation))
        .collect())
}

/// `GET …/merge_requests/{iid}/discussions` ⇒ diff (`Review`) comments only:
/// the notes carrying a `position` (`new_path`/`new_line`). System notes and
/// non-diff notes are dropped (the latter are covered by [`parse_notes`]).
pub fn parse_discussions(body: &str, mr_web_url: &str) -> Result<Vec<ReviewComment>, AppError> {
    let discussions: Vec<GlDiscussion> = from_json(body)?;
    Ok(discussions
        .into_iter()
        .flat_map(|d| d.notes)
        .filter(|n| !n.system && n.position.is_some())
        .map(|n| n.to_comment(mr_web_url, CommentKind::Review))
        .collect())
}

/// Serialize a [`CreatePrInput`] into the GitLab create-MR JSON body. A draft MR
/// gets a `"Draft: "` title prefix (GitLab convention; no `draft` param).
pub fn create_mr_body(input: &CreatePrInput) -> Result<String, AppError> {
    let title = if input.draft {
        format!("Draft: {}", input.title)
    } else {
        input.title.clone()
    };
    let wire = CreateMrWire {
        source_branch: &input.source_branch,
        target_branch: &input.target_branch,
        title,
        description: &input.body,
    };
    serde_json::to_string(&wire)
        .map_err(|e| AppError::Other(format!("failed to encode create-MR body: {e}")))
}

/// Parse the commit-status list, map each into a neutral [`StatusContext`], and
/// delegate the cap + rollup to [`crate::rollup::build_commit_status`]. GitLab
/// wire structs never leave here.
pub fn build_pipeline_status(sha: &str, body: &str) -> Result<CommitStatus, AppError> {
    let statuses: Vec<GlCommitStatus> = from_json(body)?;
    let contexts: Vec<StatusContext> = statuses
        .into_iter()
        .map(|s| StatusContext {
            name: s.name.unwrap_or_else(|| "status".to_string()),
            state: normalize_pipeline_state(&s.status),
            description: s.description,
            target_url: s.target_url,
        })
        .collect();
    Ok(crate::rollup::build_commit_status(sha, contexts))
}

#[cfg(test)]
mod tests {
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
}
