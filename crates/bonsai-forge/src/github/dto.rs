//! GitHub REST wire structs + mappers to provider-neutral [`crate::types`].
//!
//! GitHub JSON NEVER escapes this file: every function here takes a raw body
//! string (or `CreatePrInput`) and returns a neutral DTO, so the `Gh*` structs
//! stay private and the rest of the crate speaks only `types`. The status
//! rollup (§7) is a PURE function unit-tested for its precedence.

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use bonsai_core::error::AppError;

use crate::types::{
    CommentKind, CommitStatus, CreatePrInput, ForgeViewer, MergeMethod, MergePrInput, PrDetail,
    PrState, PrSummary, ReviewComment, StatusContext,
};

use super::rollup;

/// Parse a GitHub body into `T`; a malformed body ⇒ `ForgeApi` (never a token).
fn from_json<T: DeserializeOwned>(body: &str) -> Result<T, AppError> {
    serde_json::from_str(body)
        .map_err(|e| AppError::ForgeApi(format!("malformed GitHub response: {e}")))
}

// ---- wire structs (private; snake_case exactly as GitHub sends) ----

#[derive(Deserialize)]
struct GhUser {
    login: String,
    avatar_url: Option<String>,
}

#[derive(Deserialize)]
struct GhRef {
    #[serde(rename = "ref")]
    ref_: String,
    sha: String,
}

#[derive(Deserialize)]
struct GhLabel {
    name: String,
}

#[derive(Deserialize)]
struct GhPull {
    number: u64,
    title: String,
    state: String,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    user: Option<GhUser>,
    head: GhRef,
    base: GhRef,
    #[serde(default)]
    comments: u32,
    created_at: String,
    updated_at: String,
    html_url: String,
    #[serde(default)]
    merged: Option<bool>,
    #[serde(default)]
    merged_at: Option<String>,
    // detail-only fields (absent in list responses ⇒ defaulted)
    #[serde(default)]
    body: Option<String>,
    #[serde(default)]
    mergeable: Option<bool>,
    #[serde(default)]
    additions: u32,
    #[serde(default)]
    deletions: u32,
    #[serde(default)]
    changed_files: u32,
    #[serde(default)]
    labels: Vec<GhLabel>,
}

impl GhPull {
    fn to_summary(&self) -> PrSummary {
        let (author, author_avatar_url) = match &self.user {
            Some(u) => (u.login.clone(), u.avatar_url.clone()),
            None => (String::new(), None),
        };
        PrSummary {
            number: self.number,
            title: self.title.clone(),
            state: map_pr_state(&self.state, self.merged, &self.merged_at),
            is_draft: self.draft,
            author,
            author_avatar_url,
            source_branch: self.head.ref_.clone(),
            target_branch: self.base.ref_.clone(),
            comments: self.comments,
            created_at: self.created_at.clone(),
            updated_at: self.updated_at.clone(),
            url: self.html_url.clone(),
            head_sha: self.head.sha.clone(),
        }
    }

    fn into_detail(self) -> PrDetail {
        let summary = self.to_summary();
        PrDetail {
            summary,
            body: self.body.unwrap_or_default(),
            mergeable: self.mergeable,
            additions: self.additions,
            deletions: self.deletions,
            changed_files: self.changed_files,
            labels: self.labels.into_iter().map(|l| l.name).collect(),
        }
    }
}

/// GitHub PR state → neutral. `merged`/`merged_at` win over the raw state.
fn map_pr_state(state: &str, merged: Option<bool>, merged_at: &Option<String>) -> PrState {
    if merged == Some(true) || merged_at.is_some() {
        PrState::Merged
    } else if state == "closed" {
        PrState::Closed
    } else {
        PrState::Open
    }
}

#[derive(Deserialize)]
struct GhReviewComment {
    id: u64,
    #[serde(default)]
    user: Option<GhUser>,
    body: String,
    path: Option<String>,
    #[serde(default)]
    line: Option<u32>,
    #[serde(default)]
    original_line: Option<u32>,
    created_at: String,
    html_url: String,
}

#[derive(Deserialize)]
struct GhIssueComment {
    id: u64,
    #[serde(default)]
    user: Option<GhUser>,
    body: String,
    created_at: String,
    html_url: String,
}

fn author_of(user: &Option<GhUser>) -> (String, Option<String>) {
    match user {
        Some(u) => (u.login.clone(), u.avatar_url.clone()),
        None => (String::new(), None),
    }
}

// ---- status wire structs ----

#[derive(Deserialize)]
struct GhCombinedStatus {
    #[serde(default)]
    statuses: Vec<GhStatusItem>,
}

#[derive(Deserialize)]
struct GhStatusItem {
    state: String,
    context: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    target_url: Option<String>,
}

#[derive(Deserialize)]
struct GhCheckRuns {
    #[serde(default)]
    check_runs: Vec<GhCheckRun>,
}

#[derive(Deserialize)]
struct GhCheckRun {
    name: String,
    status: String,
    #[serde(default)]
    conclusion: Option<String>,
    #[serde(default)]
    details_url: Option<String>,
}

// ---- create-PR request body (snake_case exactly as GitHub expects) ----

#[derive(Serialize)]
struct CreatePullWire<'a> {
    title: &'a str,
    head: &'a str,
    base: &'a str,
    body: &'a str,
    draft: bool,
    maintainer_can_modify: bool,
}

#[derive(Serialize)]
struct MergeWire<'a> {
    merge_method: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    commit_title: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    commit_message: Option<&'a str>,
}

// ---- public boundary: raw body ⇒ neutral DTO ----

/// `GET /user` ⇒ [`ForgeViewer`].
pub fn parse_viewer(body: &str) -> Result<ForgeViewer, AppError> {
    let u: GhUser = from_json(body)?;
    Ok(ForgeViewer {
        login: u.login,
        avatar_url: u.avatar_url,
    })
}

/// `GET …/pulls` ⇒ `Vec<PrSummary>`.
pub fn parse_pr_list(body: &str) -> Result<Vec<PrSummary>, AppError> {
    let pulls: Vec<GhPull> = from_json(body)?;
    Ok(pulls.iter().map(GhPull::to_summary).collect())
}

/// `GET …/pulls/{n}` (or the POST response) ⇒ [`PrDetail`].
pub fn parse_pr_detail(body: &str) -> Result<PrDetail, AppError> {
    let pull: GhPull = from_json(body)?;
    Ok(pull.into_detail())
}

/// `GET …/pulls/{n}/comments` (diff-line) ⇒ `Vec<ReviewComment>` (kind=Review).
pub fn parse_review_comments(body: &str) -> Result<Vec<ReviewComment>, AppError> {
    let items: Vec<GhReviewComment> = from_json(body)?;
    Ok(items
        .into_iter()
        .map(|c| {
            let (author, author_avatar_url) = author_of(&c.user);
            ReviewComment {
                id: c.id,
                author,
                author_avatar_url,
                body: c.body,
                path: c.path,
                line: c.line.or(c.original_line),
                created_at: c.created_at,
                url: c.html_url,
                kind: CommentKind::Review,
            }
        })
        .collect())
}

/// `GET …/issues/{n}/comments` (conversation) ⇒ `Vec<ReviewComment>`.
pub fn parse_issue_comments(body: &str) -> Result<Vec<ReviewComment>, AppError> {
    let items: Vec<GhIssueComment> = from_json(body)?;
    Ok(items
        .into_iter()
        .map(|c| {
            let (author, author_avatar_url) = author_of(&c.user);
            ReviewComment {
                id: c.id,
                author,
                author_avatar_url,
                body: c.body,
                path: None,
                line: None,
                created_at: c.created_at,
                url: c.html_url,
                kind: CommentKind::Conversation,
            }
        })
        .collect())
}

/// Serialize a [`CreatePrInput`] into the GitHub create-PR JSON body.
pub fn create_pull_body(input: &CreatePrInput) -> Result<String, AppError> {
    let wire = CreatePullWire {
        title: &input.title,
        head: &input.source_branch,
        base: &input.target_branch,
        body: &input.body,
        draft: input.draft,
        maintainer_can_modify: input.maintainer_can_modify,
    };
    serde_json::to_string(&wire)
        .map_err(|e| AppError::Other(format!("failed to encode create-PR body: {e}")))
}

/// GitHub `merge_method` wire value for a neutral [`MergeMethod`].
/// `FastForward` is unsupported on GitHub ⇒ `ForgeApi`.
fn github_merge_method(method: MergeMethod) -> Result<&'static str, AppError> {
    match method {
        MergeMethod::Merge => Ok("merge"),
        MergeMethod::Squash => Ok("squash"),
        MergeMethod::Rebase => Ok("rebase"),
        MergeMethod::FastForward => Err(AppError::ForgeApi(
            "fast-forward merge is not available on GitHub".to_string(),
        )),
    }
}

/// Serialize a [`MergePrInput`] into the GitHub merge JSON body. Returns an
/// error for an unsupported method (nothing should be sent). GitHub ignores
/// `delete_source_branch` on merge, so it is omitted from the body.
pub fn merge_body(input: &MergePrInput) -> Result<String, AppError> {
    let merge_method = github_merge_method(input.method)?;
    let wire = MergeWire {
        merge_method,
        commit_title: input.commit_title.as_deref(),
        commit_message: input.commit_message.as_deref(),
    };
    serde_json::to_string(&wire)
        .map_err(|e| AppError::Other(format!("failed to encode merge body: {e}")))
}

/// The GitHub close body: `{ "state": "closed" }`.
pub fn close_body() -> String {
    "{\"state\":\"closed\"}".to_string()
}

/// Parse the legacy combined-status body and the check-runs body, map each into
/// a neutral [`StatusContext`], and delegate the cap + rollup math to
/// [`crate::rollup::build_commit_status`] (§7). GitHub wire structs never leave here.
pub fn build_combined_status(
    sha: &str,
    combined_body: &str,
    checks_body: &str,
) -> Result<CommitStatus, AppError> {
    let combined: GhCombinedStatus = from_json(combined_body)?;
    let checks: GhCheckRuns = from_json(checks_body)?;

    let mut contexts: Vec<StatusContext> = Vec::new();
    for s in combined.statuses {
        contexts.push(StatusContext {
            name: s.context,
            state: rollup::normalize_status_state(&s.state),
            description: s.description,
            target_url: s.target_url,
        });
    }
    for c in checks.check_runs {
        contexts.push(StatusContext {
            name: c.name,
            state: rollup::normalize_check_run(&c.status, c.conclusion.as_deref()),
            description: None,
            target_url: c.details_url,
        });
    }
    Ok(crate::rollup::build_commit_status(sha, contexts))
}

#[cfg(test)]
mod tests {
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
    fn malformed_body_is_forge_api_error() {
        let err = parse_pr_list("not json").unwrap_err();
        assert!(matches!(err, AppError::ForgeApi(_)), "got {err:?}");
    }
}
