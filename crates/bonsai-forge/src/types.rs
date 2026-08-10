//! Provider-neutral forge DTOs (P62 contract §3 / overview §F4).
//!
//! These are the ONLY types that cross the [`crate::provider::ForgeProvider`]
//! trait boundary — never a `serde_json::Value`, never a GitHub wire struct.
//! Every type is `#[serde(rename_all = "camelCase")]` so the IPC wire shape
//! matches the TS mirror in `src/ipc/types.ts`; each has a
//! `*_wire_shape_is_camel_case` test below.

use serde::{Deserialize, Serialize};

/// Which forge backs `origin`. Unit variants ⇒ plain camelCase string on the
/// wire (`"gitHub"` | `"gitLab"` | `"bitbucket"` | `"unknown"`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ForgeKind {
    GitHub,
    GitLab,
    Bitbucket,
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
    pub remote_name: String,
    pub web_url: String,
    /// A token is present in the keychain for `host` (NO network check).
    pub authenticated: bool,
    /// `Some` only when a validated viewer is cache-warm (after set-token).
    pub viewer: Option<ForgeViewer>,
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
mod tests {
    use super::*;
    use serde_json::{json, Value};

    fn value_of<T: Serialize>(v: &T) -> Value {
        serde_json::to_value(v).expect("serialize")
    }

    fn assert_keys(v: &Value, keys: &[&str]) {
        let obj = v.as_object().expect("object");
        for k in keys {
            assert!(obj.contains_key(*k), "missing camelCase key `{k}` in {v}");
        }
    }

    #[test]
    fn forge_kind_wire_shape_is_camel_case() {
        assert_eq!(value_of(&ForgeKind::GitHub), json!("gitHub"));
        assert_eq!(value_of(&ForgeKind::GitLab), json!("gitLab"));
        assert_eq!(value_of(&ForgeKind::Bitbucket), json!("bitbucket"));
        assert_eq!(value_of(&ForgeKind::Unknown), json!("unknown"));
        // Round-trips from the wire string the TS union sends.
        let got: ForgeKind = serde_json::from_value(json!("gitLab")).unwrap();
        assert_eq!(got, ForgeKind::GitLab);
        let bb: ForgeKind = serde_json::from_value(json!("bitbucket")).unwrap();
        assert_eq!(bb, ForgeKind::Bitbucket);
    }

    #[test]
    fn pr_state_wire_shape_is_camel_case() {
        assert_eq!(value_of(&PrState::Open), json!("open"));
        assert_eq!(value_of(&PrState::Closed), json!("closed"));
        assert_eq!(value_of(&PrState::Merged), json!("merged"));
    }

    #[test]
    fn pr_state_filter_wire_shape_is_camel_case() {
        assert_eq!(value_of(&PrStateFilter::Open), json!("open"));
        assert_eq!(value_of(&PrStateFilter::Closed), json!("closed"));
        assert_eq!(value_of(&PrStateFilter::All), json!("all"));
        // Round-trips from the wire (TS sends these strings).
        let got: PrStateFilter = serde_json::from_value(json!("all")).unwrap();
        assert_eq!(got, PrStateFilter::All);
    }

    #[test]
    fn check_rollup_wire_shape_is_camel_case() {
        assert_eq!(value_of(&CheckRollup::Success), json!("success"));
        assert_eq!(value_of(&CheckRollup::Pending), json!("pending"));
        assert_eq!(value_of(&CheckRollup::Failure), json!("failure"));
        assert_eq!(value_of(&CheckRollup::Error), json!("error"));
        assert_eq!(value_of(&CheckRollup::Neutral), json!("neutral"));
        assert_eq!(value_of(&CheckRollup::None), json!("none"));
    }

    #[test]
    fn comment_kind_wire_shape_is_camel_case() {
        assert_eq!(value_of(&CommentKind::Review), json!("review"));
        assert_eq!(value_of(&CommentKind::Conversation), json!("conversation"));
    }

    #[test]
    fn forge_viewer_wire_shape_is_camel_case() {
        let v = value_of(&ForgeViewer {
            login: "octocat".into(),
            avatar_url: Some("https://x/y.png".into()),
        });
        assert_keys(&v, &["login", "avatarUrl"]);
    }

    #[test]
    fn forge_repo_context_wire_shape_is_camel_case() {
        let v = value_of(&ForgeRepoContext {
            provider: ForgeKind::GitHub,
            host: "github.com".into(),
            owner: "o".into(),
            repo: "r".into(),
            remote_name: "origin".into(),
            web_url: "https://github.com/o/r".into(),
            authenticated: true,
            viewer: None,
        });
        assert_keys(
            &v,
            &[
                "provider",
                "host",
                "owner",
                "repo",
                "remoteName",
                "webUrl",
                "authenticated",
                "viewer",
            ],
        );
    }

    fn sample_summary() -> PrSummary {
        PrSummary {
            number: 7,
            title: "t".into(),
            state: PrState::Open,
            is_draft: true,
            author: "a".into(),
            author_avatar_url: None,
            source_branch: "feature".into(),
            target_branch: "main".into(),
            comments: 3,
            created_at: "2026-01-01T00:00:00Z".into(),
            updated_at: "2026-01-02T00:00:00Z".into(),
            url: "https://github.com/o/r/pull/7".into(),
            head_sha: "deadbeef".into(),
        }
    }

    #[test]
    fn pr_summary_wire_shape_is_camel_case() {
        let v = value_of(&sample_summary());
        assert_keys(
            &v,
            &[
                "number",
                "title",
                "state",
                "isDraft",
                "author",
                "authorAvatarUrl",
                "sourceBranch",
                "targetBranch",
                "comments",
                "createdAt",
                "updatedAt",
                "url",
                "headSha",
            ],
        );
    }

    #[test]
    fn pr_detail_wire_shape_is_camel_case() {
        let v = value_of(&PrDetail {
            summary: sample_summary(),
            body: "".into(),
            mergeable: None,
            additions: 1,
            deletions: 2,
            changed_files: 3,
            labels: vec!["bug".into()],
        });
        assert_keys(
            &v,
            &[
                "summary",
                "body",
                "mergeable",
                "additions",
                "deletions",
                "changedFiles",
                "labels",
            ],
        );
    }

    #[test]
    fn pr_list_query_wire_shape_is_camel_case() {
        let v = value_of(&PrListQuery {
            state: PrStateFilter::Open,
            page: 1,
            per_page: 30,
        });
        assert_keys(&v, &["state", "page", "perPage"]);
        // And round-trips from a wire payload.
        let got: PrListQuery =
            serde_json::from_value(json!({ "state": "open", "page": 2, "perPage": 10 })).unwrap();
        assert_eq!(got.page, 2);
        assert_eq!(got.per_page, 10);
    }

    #[test]
    fn pr_page_wire_shape_is_camel_case() {
        let v = value_of(&PrPage {
            items: vec![],
            page: 1,
            has_next: false,
        });
        assert_keys(&v, &["items", "page", "hasNext"]);
    }

    #[test]
    fn create_pr_input_wire_shape_is_camel_case() {
        let v = value_of(&CreatePrInput {
            title: "t".into(),
            body: "b".into(),
            source_branch: "feature".into(),
            target_branch: "main".into(),
            draft: false,
            maintainer_can_modify: true,
        });
        assert_keys(
            &v,
            &[
                "title",
                "body",
                "sourceBranch",
                "targetBranch",
                "draft",
                "maintainerCanModify",
            ],
        );
    }

    #[test]
    fn review_comment_wire_shape_is_camel_case() {
        let v = value_of(&ReviewComment {
            id: 1,
            author: "a".into(),
            author_avatar_url: None,
            body: "b".into(),
            path: Some("src/x.rs".into()),
            line: Some(42),
            created_at: "2026-01-01T00:00:00Z".into(),
            url: "https://x".into(),
            kind: CommentKind::Review,
        });
        assert_keys(
            &v,
            &[
                "id",
                "author",
                "authorAvatarUrl",
                "body",
                "path",
                "line",
                "createdAt",
                "url",
                "kind",
            ],
        );
    }

    #[test]
    fn commit_status_wire_shape_is_camel_case() {
        let v = value_of(&CommitStatus {
            sha: "abc".into(),
            state: CheckRollup::Success,
            total: 2,
            passed: 2,
            failed: 0,
            pending: 0,
            contexts: vec![StatusContext {
                name: "ci".into(),
                state: CheckRollup::Success,
                description: Some("ok".into()),
                target_url: Some("https://x".into()),
            }],
        });
        assert_keys(
            &v,
            &["sha", "state", "total", "passed", "failed", "pending", "contexts"],
        );
        let ctx = &v["contexts"][0];
        assert_keys(ctx, &["name", "state", "description", "targetUrl"]);
    }
}
