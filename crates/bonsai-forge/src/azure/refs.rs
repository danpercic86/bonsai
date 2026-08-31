//! Azure DevOps PR ref-mapping (P89): parse a PR payload into a neutral
//! [`PrRefs`] fetch plan. Kept out of `dto.rs` to stay under the size limit and
//! to isolate the P89 fork / server-merge-ref concern.
//!
//! Mapping (contract §3):
//!   * base: `+<targetRefName>:refs/bonsai/pr/<n>/base` from origin, resolve the
//!     `lastMergeTargetCommit` SHA.
//!   * head (same repo): `+refs/pull/<n>/merge:refs/bonsai/pr/<n>/merge` from
//!     origin, resolve the `lastMergeSourceCommit` SHA (the raw source tip, not
//!     the server merge commit — OQ-2).
//!   * head (fork): fetch `<sourceRefName>` from the fork clone URL
//!     (`forkSource.repository.remoteUrl`); still resolve by the source SHA.
//!
//! `resolve` is always the tip SHA, so the diff engine resolves by oid
//! regardless of ref naming.

use serde::de::DeserializeOwned;
use serde::Deserialize;

use bonsai_core::error::AppError;
use bonsai_core::git::pr_diff::FetchTarget;

use crate::types::PrRefs;

/// Parse an Azure body into `T`; a malformed body ⇒ `ForgeApi` (never a token).
fn from_json<T: DeserializeOwned>(body: &str) -> Result<T, AppError> {
    serde_json::from_str(body)
        .map_err(|e| AppError::ForgeApi(format!("malformed Azure DevOps response: {e}")))
}

/// A commit reference (`lastMergeSourceCommit`/`lastMergeTargetCommit`).
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AzCommitRef {
    #[serde(default)]
    commit_id: String,
}

/// A source/target repository object; only its clone URL is read here.
#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct AzRepository {
    #[serde(default)]
    remote_url: Option<String>,
}

/// `forkSource` — present only when the PR source lives in a fork.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AzForkSource {
    #[serde(default)]
    repository: Option<AzRepository>,
}

/// The subset of an Azure PR payload the P89 ref plan needs (branch refs,
/// merge-tip SHAs, fork source). `sourceRefName`/`targetRefName` are the raw
/// `refs/...` names Azure sends, used verbatim in the refspec.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AzPrRefs {
    #[serde(default)]
    source_ref_name: String,
    #[serde(default)]
    target_ref_name: String,
    #[serde(default)]
    last_merge_source_commit: Option<AzCommitRef>,
    #[serde(default)]
    last_merge_target_commit: Option<AzCommitRef>,
    #[serde(default)]
    fork_source: Option<AzForkSource>,
}

/// `GET …/pullrequests/{id}` ⇒ [`PrRefs`] (P89).
pub fn parse_pr_refs(body: &str, number: u64) -> Result<PrRefs, AppError> {
    let pr: AzPrRefs = from_json(body)?;
    let head_oid = pr
        .last_merge_source_commit
        .map(|c| c.commit_id)
        .unwrap_or_default();
    let base_oid = pr
        .last_merge_target_commit
        .map(|c| c.commit_id)
        .unwrap_or_default();

    // A fork PR carries `forkSource`; its repository remoteUrl is the head's
    // clone URL. Same-repo PRs fetch the server merge ref from origin.
    let fork_url = pr
        .fork_source
        .and_then(|f| f.repository)
        .and_then(|r| r.remote_url);

    // OQ-2 legacy-TFS fallback: `lastMergeSourceCommit` can be absent on very old
    // TFS, so we have no head SHA. Rather than silently emptying `head_oid` (which
    // makes the diff fail to resolve), fetch the raw `sourceRefName` into our
    // namespaced head ref and resolve by that ref NAME instead of a fixed SHA.
    // Requires a source branch to resolve against; otherwise surface a clear error.
    let head_ref = format!("refs/bonsai/pr/{number}/head");
    let missing_source_sha = head_oid.is_empty();
    if missing_source_sha && pr.source_ref_name.is_empty() {
        return Err(AppError::ForgeApi(format!(
            "Azure DevOps PR #{number} has no merge commit yet and no source branch to \
             resolve its head from; the diff isn't available until the PR is refreshed."
        )));
    }

    let head_fetch = match fork_url {
        Some(url) => FetchTarget {
            url: Some(url),
            refspec: format!("+{}:{head_ref}", pr.source_ref_name),
            resolve: if missing_source_sha {
                head_ref.clone()
            } else {
                head_oid.clone()
            },
        },
        None if missing_source_sha => FetchTarget {
            // No server merge ref available: fetch the source branch from origin
            // and resolve by our local ref name.
            url: None,
            refspec: format!("+{}:{head_ref}", pr.source_ref_name),
            resolve: head_ref.clone(),
        },
        None => FetchTarget {
            url: None,
            refspec: format!("+refs/pull/{number}/merge:refs/bonsai/pr/{number}/merge"),
            resolve: head_oid.clone(),
        },
    };

    Ok(PrRefs {
        base_oid: base_oid.clone(),
        head_oid,
        base_fetch: FetchTarget {
            url: None,
            refspec: format!("+{}:refs/bonsai/pr/{number}/base", pr.target_ref_name),
            resolve: base_oid,
        },
        head_fetch,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_pr_refs_same_repo_uses_merge_ref_from_origin() {
        let body = r#"{
            "pullRequestId": 42, "title": "T", "status": "active",
            "sourceRefName": "refs/heads/feature", "targetRefName": "refs/heads/main",
            "lastMergeSourceCommit": { "commitId": "aaa" },
            "lastMergeTargetCommit": { "commitId": "bbb" }
        }"#;
        let refs = parse_pr_refs(body, 42).unwrap();
        assert_eq!(refs.head_oid, "aaa");
        assert_eq!(refs.base_oid, "bbb");
        assert!(refs.base_fetch.url.is_none() && refs.head_fetch.url.is_none());
        assert_eq!(
            refs.base_fetch.refspec,
            "+refs/heads/main:refs/bonsai/pr/42/base"
        );
        assert_eq!(
            refs.head_fetch.refspec,
            "+refs/pull/42/merge:refs/bonsai/pr/42/merge"
        );
        assert_eq!(refs.head_fetch.resolve, "aaa");
        assert_eq!(refs.base_fetch.resolve, "bbb");
    }

    #[test]
    fn parse_pr_refs_fork_fetches_source_ref_from_fork_url() {
        let body = r#"{
            "pullRequestId": 7, "title": "T", "status": "active",
            "sourceRefName": "refs/heads/feature", "targetRefName": "refs/heads/main",
            "lastMergeSourceCommit": { "commitId": "aaa" },
            "lastMergeTargetCommit": { "commitId": "bbb" },
            "forkSource": { "repository": {
                "remoteUrl": "https://dev.azure.com/org/proj/_git/fork"
            } }
        }"#;
        let refs = parse_pr_refs(body, 7).unwrap();
        assert_eq!(refs.head_oid, "aaa");
        assert_eq!(
            refs.head_fetch.url.as_deref(),
            Some("https://dev.azure.com/org/proj/_git/fork")
        );
        assert_eq!(
            refs.head_fetch.refspec,
            "+refs/heads/feature:refs/bonsai/pr/7/head"
        );
        assert_eq!(refs.head_fetch.resolve, "aaa");
        // base always fetched from origin.
        assert!(refs.base_fetch.url.is_none());
    }

    #[test]
    fn missing_source_commit_same_repo_resolves_by_ref_name() {
        // Legacy TFS: no `lastMergeSourceCommit`. Fetch the source branch from
        // origin into our head ref and resolve by that ref name, not a SHA.
        let body = r#"{
            "pullRequestId": 5, "title": "T", "status": "active",
            "sourceRefName": "refs/heads/feature", "targetRefName": "refs/heads/main",
            "lastMergeTargetCommit": { "commitId": "bbb" }
        }"#;
        let refs = parse_pr_refs(body, 5).unwrap();
        assert!(refs.head_fetch.url.is_none());
        assert_eq!(
            refs.head_fetch.refspec,
            "+refs/heads/feature:refs/bonsai/pr/5/head"
        );
        assert_eq!(refs.head_fetch.resolve, "refs/bonsai/pr/5/head");
        assert_eq!(refs.base_oid, "bbb");
    }

    #[test]
    fn missing_source_commit_fork_resolves_by_ref_name() {
        let body = r#"{
            "pullRequestId": 9, "title": "T", "status": "active",
            "sourceRefName": "refs/heads/feature", "targetRefName": "refs/heads/main",
            "lastMergeTargetCommit": { "commitId": "bbb" },
            "forkSource": { "repository": {
                "remoteUrl": "https://dev.azure.com/org/proj/_git/fork"
            } }
        }"#;
        let refs = parse_pr_refs(body, 9).unwrap();
        assert_eq!(
            refs.head_fetch.url.as_deref(),
            Some("https://dev.azure.com/org/proj/_git/fork")
        );
        assert_eq!(
            refs.head_fetch.refspec,
            "+refs/heads/feature:refs/bonsai/pr/9/head"
        );
        assert_eq!(refs.head_fetch.resolve, "refs/bonsai/pr/9/head");
    }

    #[test]
    fn missing_source_commit_and_no_source_ref_is_error() {
        let body = r#"{
            "pullRequestId": 3, "title": "T", "status": "active",
            "targetRefName": "refs/heads/main",
            "lastMergeTargetCommit": { "commitId": "bbb" }
        }"#;
        let err = parse_pr_refs(body, 3).unwrap_err();
        assert!(matches!(err, AppError::ForgeApi(_)), "got {err:?}");
    }

    #[test]
    fn malformed_body_is_forge_api_error() {
        let err = parse_pr_refs("not json", 1).unwrap_err();
        assert!(matches!(err, AppError::ForgeApi(_)), "got {err:?}");
    }
}
