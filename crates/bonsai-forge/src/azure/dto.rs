//! Azure DevOps REST 7.1 wire structs + mappers to provider-neutral
//! [`crate::types`].
//!
//! Azure JSON NEVER escapes this file: every function here takes a raw body
//! string (or a `CreatePrInput`) and returns a neutral DTO, so the `Az*` structs
//! stay private and the rest of the crate speaks only `types`. The commit-status
//! → [`crate::types::CommitStatus`] mapping delegates the precedence + counts +
//! cap to the shared [`crate::rollup::build_commit_status`].
//!
//! Two Azure specifics live here (contract §3c):
//!   * Branch refs arrive as `refs/heads/x` on the wire ⇒ stripped to `x` for the
//!     neutral head/base names, and re-added on `create_pr`.
//!   * Collections come in a `{ count, value: [...] }` envelope; `has_next` is
//!     computed by the caller from the `$top`/returned-count comparison.

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use bonsai_core::error::AppError;

use crate::types::{
    CheckRollup, CommentKind, CommitStatus, CreatePrInput, ForgeViewer, PrDetail, PrState,
    PrSummary, ReviewComment, StatusContext,
};

/// The `refs/heads/` prefix Azure uses for branch source/target refs.
const HEADS_PREFIX: &str = "refs/heads/";

/// Parse an Azure body into `T`; a malformed body ⇒ `ForgeApi` (never a token).
fn from_json<T: DeserializeOwned>(body: &str) -> Result<T, AppError> {
    serde_json::from_str(body)
        .map_err(|e| AppError::ForgeApi(format!("malformed Azure DevOps response: {e}")))
}

/// Strip the `refs/heads/` prefix for a neutral branch name (contract §3c).
fn strip_ref(refname: &str) -> String {
    refname.strip_prefix(HEADS_PREFIX).unwrap_or(refname).to_string()
}

/// Re-add the `refs/heads/` prefix for a create-PR ref, unless the caller already
/// passed a fully-qualified `refs/...` ref.
fn qualify_ref(branch: &str) -> String {
    if branch.starts_with("refs/") {
        branch.to_string()
    } else {
        format!("{HEADS_PREFIX}{branch}")
    }
}

// ---- shared wire structs (private; camelCase exactly as Azure sends) ----

/// The `{ count, value: [...] }` envelope every Azure collection returns.
#[derive(Deserialize)]
struct AzList<T> {
    #[serde(default = "Vec::new")]
    value: Vec<T>,
}

/// An Azure identity (PR `createdBy`, comment `author`).
#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct AzIdentity {
    #[serde(default)]
    display_name: Option<String>,
    #[serde(default)]
    image_url: Option<String>,
}

/// Author name + avatar for a PR/comment.
fn author_of(id: &Option<AzIdentity>) -> (String, Option<String>) {
    match id {
        Some(i) => (i.display_name.clone().unwrap_or_default(), i.image_url.clone()),
        None => (String::new(), None),
    }
}

// ---- pull-request wire structs ----

/// A commit reference (`lastMergeSourceCommit`).
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AzCommitRef {
    #[serde(default)]
    commit_id: String,
}

/// A PR label (`labels[].name`).
#[derive(Deserialize)]
struct AzLabel {
    #[serde(default)]
    name: String,
}

/// A pull request. The list and detail endpoints return the SAME object; the
/// detail-only fields (`description`, `mergeStatus`, `labels`) are absent in list
/// responses ⇒ defaulted.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AzPullRequest {
    pull_request_id: u64,
    title: String,
    /// `active` | `completed` | `abandoned` | `notSet`.
    status: String,
    #[serde(default)]
    is_draft: bool,
    #[serde(default)]
    created_by: Option<AzIdentity>,
    #[serde(default)]
    source_ref_name: String,
    #[serde(default)]
    target_ref_name: String,
    #[serde(default)]
    creation_date: String,
    #[serde(default)]
    closed_date: Option<String>,
    #[serde(default)]
    last_merge_source_commit: Option<AzCommitRef>,
    // ---- detail-only ----
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    merge_status: Option<String>,
    #[serde(default)]
    labels: Vec<AzLabel>,
}

impl AzPullRequest {
    /// `web_url_base` is the repo's browser URL (`…/_git/{repo}`); the PR page is
    /// `{base}/pullrequest/{id}` (Azure PR objects carry no browser URL).
    fn to_summary(&self, web_url_base: &str) -> PrSummary {
        let (author, author_avatar_url) = author_of(&self.created_by);
        let head_sha = self
            .last_merge_source_commit
            .as_ref()
            .map(|c| c.commit_id.clone())
            .unwrap_or_default();
        PrSummary {
            number: self.pull_request_id,
            title: self.title.clone(),
            state: map_pr_state(&self.status),
            is_draft: self.is_draft,
            author,
            author_avatar_url,
            source_branch: strip_ref(&self.source_ref_name),
            target_branch: strip_ref(&self.target_ref_name),
            // Azure PR objects carry no comment count in the base payload ⇒ 0
            // (matches the OQ-A2 "no extra call in v1" stance).
            comments: 0,
            created_at: self.creation_date.clone(),
            // No "last updated" field; the close date is the best proxy, else the
            // creation date.
            updated_at: self
                .closed_date
                .clone()
                .unwrap_or_else(|| self.creation_date.clone()),
            url: format!("{web_url_base}/pullrequest/{}", self.pull_request_id),
            head_sha,
        }
    }

    fn into_detail(self, web_url_base: &str) -> PrDetail {
        let summary = self.to_summary(web_url_base);
        let labels = self.labels.into_iter().map(|l| l.name).collect();
        PrDetail {
            summary,
            body: self.description.unwrap_or_default(),
            mergeable: map_mergeable(self.merge_status.as_deref()),
            // OQ-A2: v1 leaves diff stats at 0 rather than firing a heavy
            // `/iterations/{i}/changes` call per PR.
            additions: 0,
            deletions: 0,
            changed_files: 0,
            labels,
        }
    }
}

/// Azure PR status → neutral (contract §3c): `active→Open`, `completed→Merged`,
/// `abandoned→Closed`; anything else (e.g. `notSet`) treated as `Open`.
fn map_pr_state(status: &str) -> PrState {
    match status {
        "completed" => PrState::Merged,
        "abandoned" => PrState::Closed,
        _ => PrState::Open, // "active" | "notSet"
    }
}

/// `mergeStatus` → `mergeable` (contract §3c): `succeeded→Some(true)`,
/// `conflicts→Some(false)`, `queued`/`notSet`/absent/other → `None` (unknown).
fn map_mergeable(status: Option<&str>) -> Option<bool> {
    match status {
        Some("succeeded") => Some(true),
        Some("conflicts") => Some(false),
        _ => None,
    }
}

// ---- comment (thread) wire structs ----

/// A PR comment thread. Each thread's `comments[]` are flattened into neutral
/// comments; a `threadContext.filePath` marks the whole thread as a `Review`
/// (diff-anchored) thread, else its comments are `Conversation`.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AzThread {
    #[serde(default)]
    id: u64,
    #[serde(default)]
    comments: Vec<AzComment>,
    #[serde(default)]
    thread_context: Option<AzThreadContext>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AzComment {
    #[serde(default)]
    id: u64,
    #[serde(default)]
    content: Option<String>,
    #[serde(default)]
    author: Option<AzIdentity>,
    #[serde(default)]
    published_date: String,
    /// `text` | `system` | `codeChange`; `system` comments are dropped.
    #[serde(default)]
    comment_type: Option<String>,
    #[serde(default)]
    is_deleted: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AzThreadContext {
    #[serde(default)]
    file_path: Option<String>,
    #[serde(default)]
    right_file_start: Option<AzFileStart>,
    #[serde(default)]
    left_file_start: Option<AzFileStart>,
}

#[derive(Deserialize)]
struct AzFileStart {
    #[serde(default)]
    line: Option<u32>,
}

impl AzThreadContext {
    /// The diff anchor: prefer the right (new) side's line, else the left (old).
    fn line(&self) -> Option<u32> {
        self.right_file_start
            .as_ref()
            .and_then(|p| p.line)
            .or_else(|| self.left_file_start.as_ref().and_then(|p| p.line))
    }
}

// ---- commit-status wire structs ----

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AzCommitStatus {
    /// `succeeded|pending|failed|error|notApplicable|notSet`.
    state: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    target_url: Option<String>,
    #[serde(default)]
    context: Option<AzStatusContext>,
}

#[derive(Deserialize)]
struct AzStatusContext {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    genre: Option<String>,
}

/// Azure commit-status state → neutral [`CheckRollup`] (contract §3c):
/// `succeeded→Success`, `pending→Pending`, `failed|error→Failure`,
/// `notApplicable|notSet→Neutral`, else `Error`.
fn normalize_status_state(state: &str) -> CheckRollup {
    match state {
        "succeeded" => CheckRollup::Success,
        "pending" => CheckRollup::Pending,
        "failed" | "error" => CheckRollup::Failure,
        "notApplicable" | "notSet" => CheckRollup::Neutral,
        _ => CheckRollup::Error,
    }
}

// ---- profile (viewer) wire struct ----

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AzProfile {
    #[serde(default)]
    display_name: Option<String>,
    #[serde(default)]
    email_address: Option<String>,
}

// ---- create-PR request body (camelCase exactly as Azure expects) ----

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CreatePrWire<'a> {
    source_ref_name: String,
    target_ref_name: String,
    title: &'a str,
    description: &'a str,
    is_draft: bool,
}

// ---- public boundary: raw body ⇒ neutral DTO ----

/// `GET …/profiles/me` ⇒ [`ForgeViewer`] (`displayName`→login; avatar absent,
/// contract §3c).
pub fn parse_viewer(body: &str) -> Result<ForgeViewer, AppError> {
    let p: AzProfile = from_json(body)?;
    Ok(ForgeViewer {
        login: p.display_name.or(p.email_address).unwrap_or_default(),
        avatar_url: None,
    })
}

/// `GET …/pullrequests` ⇒ `Vec<PrSummary>`. `has_next` (returned == `$top`) is
/// computed by the caller, which knows `$top`.
pub fn parse_pr_list(body: &str, web_url_base: &str) -> Result<Vec<PrSummary>, AppError> {
    let page: AzList<AzPullRequest> = from_json(body)?;
    Ok(page
        .value
        .iter()
        .map(|pr| pr.to_summary(web_url_base))
        .collect())
}

/// `GET …/pullrequests/{id}` (or the POST response) ⇒ [`PrDetail`].
pub fn parse_pr_detail(body: &str, web_url_base: &str) -> Result<PrDetail, AppError> {
    let pr: AzPullRequest = from_json(body)?;
    Ok(pr.into_detail(web_url_base))
}

/// `GET …/pullrequests/{id}/threads` ⇒ neutral comments. Each thread's
/// `comments[]` are flattened; a `threadContext.filePath` ⇒ `Review` (carrying
/// path + line), else `Conversation`. System + deleted comments are dropped.
/// Sorting by `created_at` is the caller's job (matching the other providers).
///
/// Azure comment ids are unique only WITHIN a thread, so the neutral id is
/// synthesized as `threadId*1000 + commentId` to stay globally unique (a thread
/// never holds ≥1000 comments) — the UI keys review comments by this id.
pub fn parse_threads(body: &str, pr_web_url: &str) -> Result<Vec<ReviewComment>, AppError> {
    let page: AzList<AzThread> = from_json(body)?;
    let mut out = Vec::new();
    for thread in page.value {
        let (kind, path, line) = match &thread.thread_context {
            Some(ctx) if ctx.file_path.is_some() => {
                (CommentKind::Review, ctx.file_path.clone(), ctx.line())
            }
            _ => (CommentKind::Conversation, None, None),
        };
        let thread_url = format!("{pr_web_url}?discussionId={}", thread.id);
        for c in thread.comments {
            if c.is_deleted || c.comment_type.as_deref() == Some("system") {
                continue;
            }
            let (author, author_avatar_url) = author_of(&c.author);
            out.push(ReviewComment {
                id: thread.id.saturating_mul(1000).saturating_add(c.id),
                author,
                author_avatar_url,
                body: c.content.unwrap_or_default(),
                path: path.clone(),
                line,
                created_at: c.published_date,
                url: thread_url.clone(),
                kind,
            });
        }
    }
    Ok(out)
}

/// Serialize a [`CreatePrInput`] into the Azure create-PR JSON body
/// (`{sourceRefName, targetRefName, title, description, isDraft}`), re-adding the
/// `refs/heads/` prefix to the branch names.
pub fn create_pr_body(input: &CreatePrInput) -> Result<String, AppError> {
    let wire = CreatePrWire {
        source_ref_name: qualify_ref(&input.source_branch),
        target_ref_name: qualify_ref(&input.target_branch),
        title: &input.title,
        description: &input.body,
        is_draft: input.draft,
    };
    serde_json::to_string(&wire)
        .map_err(|e| AppError::Other(format!("failed to encode create-PR body: {e}")))
}

/// Parse the commit-status collection, map each into a neutral [`StatusContext`],
/// and delegate the cap + rollup to [`crate::rollup::build_commit_status`]. Azure
/// wire structs never leave here.
pub fn build_commit_status(sha: &str, body: &str) -> Result<CommitStatus, AppError> {
    let page: AzList<AzCommitStatus> = from_json(body)?;
    let contexts: Vec<StatusContext> = page
        .value
        .into_iter()
        .map(|s| {
            let name = s
                .context
                .as_ref()
                .and_then(|c| c.name.clone().or_else(|| c.genre.clone()))
                .unwrap_or_else(|| "status".to_string());
            StatusContext {
                name,
                state: normalize_status_state(&s.state),
                description: s.description,
                target_url: s.target_url,
            }
        })
        .collect();
    Ok(crate::rollup::build_commit_status(sha, contexts))
}

#[cfg(test)]
mod tests;
