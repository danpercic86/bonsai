//! Bitbucket Cloud REST 2.0 wire structs + mappers to provider-neutral
//! [`crate::types`].
//!
//! Bitbucket JSON NEVER escapes this file: every function here takes a raw body
//! string (or a `CreatePrInput`) and returns a neutral DTO, so the `Bb*` structs
//! stay private and the rest of the crate speaks only `types`. The build-status
//! → [`crate::types::CommitStatus`] mapping delegates the precedence + counts +
//! cap to the shared [`crate::rollup::build_commit_status`].
//!
//! Bitbucket paginates in the JSON BODY (a `values[]` array + an absolute `next`
//! URL), NOT via a Link/`X-Next-Page` header — so `parse_pr_list` returns the
//! `has_next` signal alongside the items (contract §3c).

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use bonsai_core::error::AppError;

use crate::types::{
    CheckRollup, CommentKind, CommitStatus, CreatePrInput, ForgeViewer, PrDetail, PrState,
    PrSummary, ReviewComment, StatusContext,
};

/// Parse a Bitbucket body into `T`; a malformed body ⇒ `ForgeApi` (never a token).
fn from_json<T: DeserializeOwned>(body: &str) -> Result<T, AppError> {
    serde_json::from_str(body)
        .map_err(|e| AppError::ForgeApi(format!("malformed Bitbucket response: {e}")))
}

// ---- shared wire structs (private; exactly as Bitbucket sends) ----

/// The paged envelope every Bitbucket 2.0 collection endpoint returns. `next` is
/// an absolute URL when another page follows (contract §3c pagination).
#[derive(Deserialize)]
struct BbPaged<T> {
    #[serde(default = "Vec::new")]
    values: Vec<T>,
    #[serde(default)]
    next: Option<String>,
}

/// A `{ "href": "…" }` link node (used inside a `links` object).
#[derive(Deserialize)]
struct BbLink {
    href: String,
}

/// The `links` object; only the fields Bonsai reads are declared.
#[derive(Deserialize, Default)]
struct BbLinks {
    #[serde(default)]
    html: Option<BbLink>,
    #[serde(default)]
    avatar: Option<BbLink>,
}

impl BbLinks {
    fn html_href(&self) -> String {
        self.html.as_ref().map(|l| l.href.clone()).unwrap_or_default()
    }
    fn avatar_href(&self) -> Option<String> {
        self.avatar.as_ref().map(|l| l.href.clone())
    }
}

/// A Bitbucket account. PR authors surface `display_name`; the identity endpoint
/// (`GET /user`) prefers `username` (contract §3c). All fields are optional so a
/// slimmed-down account object still parses.
#[derive(Deserialize, Default)]
struct BbAccount {
    #[serde(default)]
    display_name: Option<String>,
    #[serde(default)]
    username: Option<String>,
    #[serde(default)]
    nickname: Option<String>,
    #[serde(default)]
    links: BbLinks,
}

/// Author name (`display_name` first, per contract) + avatar for a PR/comment.
fn author_of(account: &Option<BbAccount>) -> (String, Option<String>) {
    match account {
        Some(a) => {
            let name = a
                .display_name
                .clone()
                .or_else(|| a.username.clone())
                .or_else(|| a.nickname.clone())
                .unwrap_or_default();
            (name, a.links.avatar_href())
        }
        None => (String::new(), None),
    }
}

// ---- pull-request wire structs ----

/// The `source`/`destination` endpoint of a PR: a `branch.name` + a `commit.hash`.
#[derive(Deserialize, Default)]
struct BbEndpoint {
    #[serde(default)]
    branch: Option<BbNamed>,
    #[serde(default)]
    commit: Option<BbHash>,
}

#[derive(Deserialize)]
struct BbNamed {
    name: String,
}

#[derive(Deserialize)]
struct BbHash {
    hash: String,
}

impl BbEndpoint {
    fn branch_name(&self) -> String {
        self.branch.as_ref().map(|b| b.name.clone()).unwrap_or_default()
    }
    fn commit_hash(&self) -> String {
        self.commit.as_ref().map(|c| c.hash.clone()).unwrap_or_default()
    }
}

/// A pull request. The list and detail endpoints return the SAME object; the
/// detail-only `description` is absent in list responses ⇒ defaulted.
#[derive(Deserialize)]
struct BbPullRequest {
    id: u64,
    title: String,
    /// `OPEN` | `MERGED` | `DECLINED` | `SUPERSEDED`.
    state: String,
    #[serde(default)]
    author: Option<BbAccount>,
    #[serde(default)]
    source: BbEndpoint,
    #[serde(default)]
    destination: BbEndpoint,
    #[serde(default)]
    comment_count: u32,
    created_on: String,
    #[serde(default)]
    updated_on: String,
    #[serde(default)]
    links: BbLinks,
    // ---- detail-only ----
    #[serde(default)]
    description: Option<String>,
}

impl BbPullRequest {
    fn to_summary(&self) -> PrSummary {
        let (author, author_avatar_url) = author_of(&self.author);
        PrSummary {
            number: self.id,
            title: self.title.clone(),
            state: map_pr_state(&self.state),
            // Bitbucket Cloud exposes no draft flag on the read model (contract
            // §3c) ⇒ never a draft.
            is_draft: false,
            author,
            author_avatar_url,
            source_branch: self.source.branch_name(),
            target_branch: self.destination.branch_name(),
            comments: self.comment_count,
            created_at: self.created_on.clone(),
            updated_at: self.updated_on.clone(),
            url: self.links.html_href(),
            head_sha: self.source.commit_hash(),
        }
    }

    fn into_detail(self) -> PrDetail {
        let summary = self.to_summary();
        PrDetail {
            summary,
            body: self.description.unwrap_or_default(),
            // Bitbucket doesn't directly expose a mergeable flag on the PR object
            // ⇒ unknown (contract §3c).
            mergeable: None,
            // OQ-A2: v1 leaves diff stats at 0 rather than firing a heavy
            // `/diffstat` call per PR. Bitbucket has no PR labels ⇒ empty.
            additions: 0,
            deletions: 0,
            changed_files: 0,
            labels: Vec::new(),
        }
    }
}

/// Bitbucket PR state → neutral. `DECLINED`/`SUPERSEDED` collapse to `Closed`.
fn map_pr_state(state: &str) -> PrState {
    match state {
        "MERGED" => PrState::Merged,
        "DECLINED" | "SUPERSEDED" => PrState::Closed,
        _ => PrState::Open, // "OPEN"
    }
}

// ---- comment wire structs ----

/// A PR comment. An `inline` block (a diff-line anchor) ⇒ a `Review` comment;
/// its absence ⇒ a `Conversation` comment. `deleted` comments are dropped.
#[derive(Deserialize)]
struct BbComment {
    id: u64,
    #[serde(default)]
    content: Option<BbContent>,
    #[serde(default)]
    user: Option<BbAccount>,
    created_on: String,
    #[serde(default)]
    deleted: bool,
    #[serde(default)]
    inline: Option<BbInline>,
    #[serde(default)]
    links: BbLinks,
}

#[derive(Deserialize)]
struct BbContent {
    #[serde(default)]
    raw: Option<String>,
}

/// The diff anchor on an inline comment: a `path` + a line (`to` = new side,
/// `from` = old side).
#[derive(Deserialize)]
struct BbInline {
    #[serde(default)]
    path: Option<String>,
    #[serde(default)]
    to: Option<u32>,
    #[serde(default)]
    from: Option<u32>,
}

impl BbComment {
    fn into_comment(self) -> ReviewComment {
        let (author, author_avatar_url) = author_of(&self.user);
        let (kind, path, line) = match self.inline {
            Some(inl) => (CommentKind::Review, inl.path, inl.to.or(inl.from)),
            None => (CommentKind::Conversation, None, None),
        };
        ReviewComment {
            id: self.id,
            author,
            author_avatar_url,
            body: self.content.and_then(|c| c.raw).unwrap_or_default(),
            path,
            line,
            created_at: self.created_on,
            url: self.links.html_href(),
            kind,
        }
    }
}

// ---- build-status (commit status) wire structs ----

#[derive(Deserialize)]
struct BbBuildStatus {
    #[serde(default)]
    key: Option<String>,
    #[serde(default)]
    name: Option<String>,
    /// `SUCCESSFUL` | `INPROGRESS` | `FAILED` | `STOPPED`.
    state: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    url: Option<String>,
}

/// Bitbucket build-status state → neutral [`CheckRollup`] (contract §3c):
/// `SUCCESSFUL→Success`, `INPROGRESS→Pending`, `FAILED→Failure`,
/// `STOPPED→Neutral`, else `Error`.
fn normalize_build_state(state: &str) -> CheckRollup {
    match state {
        "SUCCESSFUL" => CheckRollup::Success,
        "INPROGRESS" => CheckRollup::Pending,
        "FAILED" => CheckRollup::Failure,
        "STOPPED" => CheckRollup::Neutral,
        _ => CheckRollup::Error,
    }
}

// ---- create-PR request body (exactly as Bitbucket expects) ----

#[derive(Serialize)]
struct CreatePrWire<'a> {
    title: &'a str,
    description: &'a str,
    source: BranchRef<'a>,
    destination: BranchRef<'a>,
    draft: bool,
}

#[derive(Serialize)]
struct BranchRef<'a> {
    branch: BranchName<'a>,
}

#[derive(Serialize)]
struct BranchName<'a> {
    name: &'a str,
}

// ---- public boundary: raw body ⇒ neutral DTO ----

/// `GET /user` ⇒ [`ForgeViewer`] (`username`→login, `links.avatar.href`→avatarUrl).
pub fn parse_viewer(body: &str) -> Result<ForgeViewer, AppError> {
    let acc: BbAccount = from_json(body)?;
    let login = acc
        .username
        .or(acc.nickname)
        .or(acc.display_name)
        .unwrap_or_default();
    let avatar_url = acc.links.avatar_href();
    Ok(ForgeViewer { login, avatar_url })
}

/// `GET …/pullrequests` ⇒ (`Vec<PrSummary>`, `has_next`). `has_next` is `true`
/// when the paged envelope carries a `next` URL (contract §3c).
pub fn parse_pr_list(body: &str) -> Result<(Vec<PrSummary>, bool), AppError> {
    let page: BbPaged<BbPullRequest> = from_json(body)?;
    let has_next = page.next.is_some();
    let items = page.values.iter().map(BbPullRequest::to_summary).collect();
    Ok((items, has_next))
}

/// `GET …/pullrequests/{id}` (or the POST response) ⇒ [`PrDetail`].
pub fn parse_pr_detail(body: &str) -> Result<PrDetail, AppError> {
    let pr: BbPullRequest = from_json(body)?;
    Ok(pr.into_detail())
}

/// `GET …/pullrequests/{id}/comments` ⇒ neutral comments (inline⇒`Review`,
/// else `Conversation`), dropping deleted comments. Sorting by `created_at` is
/// the caller's job (matching the GitLab provider).
pub fn parse_comments(body: &str) -> Result<Vec<ReviewComment>, AppError> {
    let page: BbPaged<BbComment> = from_json(body)?;
    Ok(page
        .values
        .into_iter()
        .filter(|c| !c.deleted)
        .map(BbComment::into_comment)
        .collect())
}

/// Serialize a [`CreatePrInput`] into the Bitbucket create-PR JSON body
/// (`{title, description, source:{branch:{name}}, destination:{branch:{name}},
/// draft}`).
pub fn create_pr_body(input: &CreatePrInput) -> Result<String, AppError> {
    let wire = CreatePrWire {
        title: &input.title,
        description: &input.body,
        source: BranchRef {
            branch: BranchName {
                name: &input.source_branch,
            },
        },
        destination: BranchRef {
            branch: BranchName {
                name: &input.target_branch,
            },
        },
        draft: input.draft,
    };
    serde_json::to_string(&wire)
        .map_err(|e| AppError::Other(format!("failed to encode create-PR body: {e}")))
}

/// Parse the build-status collection, map each into a neutral [`StatusContext`],
/// and delegate the cap + rollup to [`crate::rollup::build_commit_status`].
/// Bitbucket wire structs never leave here.
pub fn build_commit_status(sha: &str, body: &str) -> Result<CommitStatus, AppError> {
    let page: BbPaged<BbBuildStatus> = from_json(body)?;
    let contexts: Vec<StatusContext> = page
        .values
        .into_iter()
        .map(|s| StatusContext {
            name: s.name.or(s.key).unwrap_or_else(|| "status".to_string()),
            state: normalize_build_state(&s.state),
            description: s.description,
            target_url: s.url,
        })
        .collect();
    Ok(crate::rollup::build_commit_status(sha, contexts))
}

#[cfg(test)]
mod tests;
