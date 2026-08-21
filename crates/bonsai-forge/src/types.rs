//! Provider-neutral forge DTOs (P62 contract §3 / overview §F4).
//!
//! These are the ONLY types that cross the [`crate::provider::ForgeProvider`]
//! trait boundary — never a `serde_json::Value`, never a GitHub wire struct.
//! Every type is `#[serde(rename_all = "camelCase")]` so the IPC wire shape
//! matches the TS mirror in `src/ipc/types.ts`; each has a
//! `*_wire_shape_is_camel_case` test below.

use serde::{Deserialize, Serialize};

/// Which forge backs `origin`. Unit variants ⇒ plain camelCase string on the
/// wire (`"gitHub"` | `"gitLab"` | `"bitbucket"` | `"azureDevOps"` | `"unknown"`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ForgeKind {
    GitHub,
    GitLab,
    Bitbucket,
    AzureDevOps,
    Unknown,
}

/// The authenticated user (GitHub `GET /user`). `avatar_url` may be absent.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ForgeViewer {
    pub login: String,
    pub avatar_url: Option<String>,
}

/// Identity of the forge repo derived from `origin` + keychain presence. No
/// network is required to build this.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ForgeRepoContext {
    pub provider: ForgeKind,
    pub host: String,
    pub owner: String,
    pub repo: String,
    /// Azure DevOps needs a 3-part org/project/repo identity; `owner` carries the
    /// org and this carries the project. `None` for GitHub/GitLab/Bitbucket.
    pub project: Option<String>,
    pub remote_name: String,
    pub web_url: String,
    /// A token is present in the keychain for `host` (NO network check).
    pub authenticated: bool,
    /// `Some` only when a validated viewer is cache-warm (after set-token).
    pub viewer: Option<ForgeViewer>,
    /// P80: the account resolved for this repo (`accountId`), or `None` when no
    /// account exists on the host. Filled by the command layer's
    /// `resolve_account`; the crate leaves it `None`.
    pub resolved_account_id: Option<String>,
    /// P80: HOW the resolved account was chosen (override / owner match / host
    /// default / single / none). Filled by the command layer.
    pub account_source: AccountSource,
}

/// P80: how the account backing a repo was resolved (see `resolve_account`).
/// The crate always emits `None`; the command layer overwrites it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AccountSource {
    Override,
    OwnerMatch,
    HostDefault,
    Single,
    None,
}

/// P79: one connected (or previously-connected) forge account for the global
/// Accounts settings section. `login`/`avatar_url` are best-effort display hints
/// from the process viewer cache + the persisted known-hosts index; this type
/// NEVER carries a token.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ForgeAccount {
    /// P80: stable identity "kind:host:login" (or "kind:host" for a legacy
    /// login-unknown account).
    pub account_id: String,
    pub host: String,
    pub kind: ForgeKind,
    /// Cache-warm or last-known login; `None` if never validated this install.
    pub login: Option<String>,
    /// Cache-warm avatar; `None` when the viewer isn't warm.
    pub avatar_url: Option<String>,
    /// A token is currently present in the keychain for `host` (no network).
    pub connected: bool,
    /// P80: whether this account is the host's default (repos inherit it).
    pub is_host_default: bool,
}

/// PR lifecycle state. `Merged` is derived from GitHub's `merged`/`merged_at`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PrState {
    Open,
    Closed,
    Merged,
}

/// List-query filter (maps to GitHub's `?state=`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PrStateFilter {
    Open,
    Closed,
    All,
}

/// One row in a PR list.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrSummary {
    pub number: u64,
    pub title: String,
    pub state: PrState,
    pub is_draft: bool,
    pub author: String,
    pub author_avatar_url: Option<String>,
    /// head ref (branch name only).
    pub source_branch: String,
    /// base ref (branch name only).
    pub target_branch: String,
    pub comments: u32,
    pub created_at: String,
    pub updated_at: String,
    /// `html_url` for opening in a browser.
    pub url: String,
    /// head sha, for the P63 status lookup.
    pub head_sha: String,
}

/// A single PR with its body + diff stats.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrDetail {
    pub summary: PrSummary,
    /// Markdown body; may be empty.
    pub body: String,
    /// `None` while GitHub is still computing mergeability.
    pub mergeable: Option<bool>,
    pub additions: u32,
    pub deletions: u32,
    pub changed_files: u32,
    pub labels: Vec<String>,
}

/// PR list request. `per_page` is capped `<= 50` by the provider.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrListQuery {
    pub state: PrStateFilter,
    pub page: u32,
    pub per_page: u32,
}

/// One page of PR summaries. `has_next` derives from the `Link` header.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrPage {
    pub items: Vec<PrSummary>,
    pub page: u32,
    pub has_next: bool,
}

/// Inputs for creating a PR. `maintainer_can_modify` defaults to true in the UI.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatePrInput {
    pub title: String,
    pub body: String,
    pub source_branch: String,
    pub target_branch: String,
    pub draft: bool,
    pub maintainer_can_modify: bool,
}

/// Whether a comment is a diff-line review comment or a PR conversation comment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CommentKind {
    Review,
    Conversation,
}

/// A merged review/conversation comment.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewComment {
    pub id: u64,
    pub author: String,
    pub author_avatar_url: Option<String>,
    pub body: String,
    pub path: Option<String>,
    pub line: Option<u32>,
    pub created_at: String,
    pub url: String,
    pub kind: CommentKind,
}

/// Normalized CI/commit-status rollup value (overview §F4). Defined + populated
/// in P62; rendered as graph badges in P63.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CheckRollup {
    Success,
    Pending,
    Failure,
    Error,
    Neutral,
    None,
}

/// One check/status context inside a [`CommitStatus`].
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusContext {
    pub name: String,
    pub state: CheckRollup,
    pub description: Option<String>,
    pub target_url: Option<String>,
}

/// The merged legacy-status + check-runs rollup for one commit. `contexts` is
/// capped at 50 individual checks.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommitStatus {
    pub sha: String,
    pub state: CheckRollup,
    pub total: u32,
    pub passed: u32,
    pub failed: u32,
    pub pending: u32,
    pub contexts: Vec<StatusContext>,
}

#[cfg(test)]
mod tests;
