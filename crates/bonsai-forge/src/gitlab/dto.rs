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
use bonsai_core::git::pr_diff::FetchTarget;

use crate::types::{
    CheckRollup, CommentKind, CommitStatus, CreatePrInput, ForgeViewer, MergeMethod, MergePrInput,
    PrDetail, PrRefs, PrState, PrSummary, ReviewComment, StatusContext,
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
    /// Base/head SHAs of the MR diff (present on the detail endpoint). The
    /// `base_sha` is the merge-base tip we diff against; `head_sha` mirrors `sha`.
    #[serde(default)]
    diff_refs: Option<GlDiffRefs>,
    // ---- detail-only ----
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    detailed_merge_status: Option<String>,
    #[serde(default)]
    labels: Vec<String>,
}

#[derive(Deserialize)]
struct GlDiffRefs {
    #[serde(default)]
    base_sha: Option<String>,
    #[serde(default)]
    head_sha: Option<String>,
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
/// `GET …/merge_requests/{iid}` ⇒ [`PrRefs`] (P89). The head is fetched via
/// `refs/merge-requests/<iid>/head`, which covers fork MRs, so both endpoints
/// fetch from the origin remote (`url: None`). `base_sha`/`head_sha` come from
/// `diff_refs` (falling back to `sha` for the head); an absent `base_sha` leaves
/// the base resolve empty — the diff engine then errors clearly on a bad oid.
pub fn parse_mr_refs(body: &str, iid: u64) -> Result<PrRefs, AppError> {
    let mr: GlMergeRequest = from_json(body)?;
    let diff_refs = mr.diff_refs.unwrap_or(GlDiffRefs {
        base_sha: None,
        head_sha: None,
    });
    let head_oid = diff_refs
        .head_sha
        .or(mr.sha)
        .unwrap_or_default();
    let base_oid = diff_refs.base_sha.unwrap_or_default();
    Ok(PrRefs {
        base_oid: base_oid.clone(),
        head_oid: head_oid.clone(),
        base_fetch: FetchTarget {
            url: None,
            refspec: format!("+refs/heads/{}:refs/bonsai/pr/{iid}/base", mr.target_branch),
            resolve: base_oid,
        },
        head_fetch: FetchTarget {
            url: None,
            refspec: format!("+refs/merge-requests/{iid}/head:refs/bonsai/pr/{iid}/head"),
            resolve: head_oid,
        },
    })
}

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

// ---- merge / close request bodies (snake_case exactly as GitLab expects) ----

#[derive(Serialize)]
struct MergeMrWire {
    /// GitLab merges with a merge commit unless `squash` is set (contract §3c).
    squash: bool,
    should_remove_source_branch: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    merge_commit_message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    squash_commit_message: Option<String>,
}

/// GitLab `squash` flag for a neutral [`MergeMethod`]. GitLab has no rebase- or
/// fast-forward-merge API param on this endpoint ⇒ `ForgeApi` (nothing sent).
fn gitlab_squash(method: MergeMethod) -> Result<bool, AppError> {
    match method {
        MergeMethod::Merge => Ok(false),
        MergeMethod::Squash => Ok(true),
        MergeMethod::Rebase => Err(AppError::ForgeApi(
            "rebase merge is not available on GitLab".to_string(),
        )),
        MergeMethod::FastForward => Err(AppError::ForgeApi(
            "fast-forward merge is not available on GitLab".to_string(),
        )),
    }
}

/// Serialize a [`MergePrInput`] into the GitLab merge JSON body. Returns an error
/// for an unsupported method (nothing should be sent). The optional commit
/// message is routed to `squash_commit_message` for a squash, else
/// `merge_commit_message`.
pub fn merge_body(input: &MergePrInput) -> Result<String, AppError> {
    let squash = gitlab_squash(input.method)?;
    let (merge_commit_message, squash_commit_message) = if squash {
        (None, input.commit_message.clone())
    } else {
        (input.commit_message.clone(), None)
    };
    let wire = MergeMrWire {
        squash,
        should_remove_source_branch: input.delete_source_branch,
        merge_commit_message,
        squash_commit_message,
    };
    serde_json::to_string(&wire)
        .map_err(|e| AppError::Other(format!("failed to encode merge body: {e}")))
}

/// The GitLab close body: `{ "state_event": "close" }`.
pub fn close_body() -> String {
    "{\"state_event\":\"close\"}".to_string()
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
#[path = "dto_tests.rs"]
mod dto_tests;
