//! Bitbucket Cloud PR ref-mapping (P89): parse a PR payload into a neutral
//! [`PrRefs`] fetch plan. Kept out of `dto.rs` to isolate the P89 fork concern.
//!
//! Mapping (contract §3):
//!   * base: `+refs/heads/<dest>:refs/bonsai/pr/<n>/base` from origin, resolve
//!     `destination.commit.hash`.
//!   * head (same repo): `+refs/heads/<src>:refs/bonsai/pr/<n>/head` from origin,
//!     resolve `source.commit.hash`.
//!   * head (fork): same refspec but fetched from the source repo clone URL
//!     (`source.repository.links.clone`, preferring the `https` entry).
//!
//! Fork is detected by a differing `full_name` between the source and
//! destination repositories. `resolve` is always the tip SHA.

use serde::de::DeserializeOwned;
use serde::Deserialize;

use bonsai_core::error::AppError;
use bonsai_core::git::pr_diff::FetchTarget;

use crate::types::PrRefs;

/// Parse a Bitbucket body into `T`; a malformed body ⇒ `ForgeApi` (never a token).
fn from_json<T: DeserializeOwned>(body: &str) -> Result<T, AppError> {
    serde_json::from_str(body)
        .map_err(|e| AppError::ForgeApi(format!("malformed Bitbucket response: {e}")))
}

#[derive(Deserialize)]
struct BbHash {
    hash: String,
}

#[derive(Deserialize)]
struct BbNamed {
    name: String,
}

#[derive(Deserialize)]
struct BbCloneLink {
    name: String,
    href: String,
}

#[derive(Deserialize, Default)]
struct BbRepoLinks {
    #[serde(default)]
    clone: Vec<BbCloneLink>,
}

#[derive(Deserialize, Default)]
struct BbRepo {
    #[serde(default)]
    full_name: Option<String>,
    #[serde(default)]
    links: BbRepoLinks,
}

impl BbRepo {
    /// The clone URL, preferring the `https` entry, else the first available.
    fn clone_url(self) -> Option<String> {
        self.links
            .clone
            .iter()
            .find(|c| c.name == "https")
            .or_else(|| self.links.clone.first())
            .map(|c| c.href.clone())
    }
}

/// A source/destination endpoint: branch name, commit hash, and owning repo.
#[derive(Deserialize, Default)]
struct BbEndpoint {
    #[serde(default)]
    branch: Option<BbNamed>,
    #[serde(default)]
    commit: Option<BbHash>,
    #[serde(default)]
    repository: Option<BbRepo>,
}

/// The subset of a Bitbucket PR payload the P89 ref plan needs.
#[derive(Deserialize)]
struct BbPrRefs {
    #[serde(default)]
    source: BbEndpoint,
    #[serde(default)]
    destination: BbEndpoint,
}

/// `GET …/pullrequests/{id}` ⇒ [`PrRefs`] (P89).
pub fn parse_pr_refs(body: &str, number: u64) -> Result<PrRefs, AppError> {
    let pr: BbPrRefs = from_json(body)?;
    let src = pr.source;
    let dest = pr.destination;

    let head_oid = src.commit.map(|c| c.hash).unwrap_or_default();
    let base_oid = dest.commit.map(|c| c.hash).unwrap_or_default();
    let src_branch = src.branch.map(|b| b.name).unwrap_or_default();
    let dest_branch = dest.branch.map(|b| b.name).unwrap_or_default();

    let src_full = src.repository.as_ref().and_then(|r| r.full_name.clone());
    let dest_full = dest.repository.as_ref().and_then(|r| r.full_name.clone());
    let is_fork = matches!((&src_full, &dest_full), (Some(s), Some(d)) if s != d);
    let head_url = if is_fork {
        src.repository.and_then(|r| r.clone_url())
    } else {
        None
    };

    Ok(PrRefs {
        base_oid: base_oid.clone(),
        head_oid: head_oid.clone(),
        base_fetch: FetchTarget {
            url: None,
            refspec: format!("+refs/heads/{dest_branch}:refs/bonsai/pr/{number}/base"),
            resolve: base_oid,
        },
        head_fetch: FetchTarget {
            url: head_url,
            refspec: format!("+refs/heads/{src_branch}:refs/bonsai/pr/{number}/head"),
            resolve: head_oid,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_pr_refs_same_repo_fetches_both_from_origin() {
        let body = r#"{
            "id": 42,
            "source": { "branch": { "name": "feature" }, "commit": { "hash": "aaa" },
                        "repository": { "full_name": "team/repo" } },
            "destination": { "branch": { "name": "main" }, "commit": { "hash": "bbb" },
                             "repository": { "full_name": "team/repo" } }
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
            "+refs/heads/feature:refs/bonsai/pr/42/head"
        );
        assert_eq!(refs.head_fetch.resolve, "aaa");
        assert_eq!(refs.base_fetch.resolve, "bbb");
    }

    #[test]
    fn parse_pr_refs_fork_uses_source_clone_url() {
        let body = r#"{
            "id": 7,
            "source": { "branch": { "name": "feature" }, "commit": { "hash": "aaa" },
                        "repository": { "full_name": "fork/repo", "links": { "clone": [
                            { "name": "ssh", "href": "git@bitbucket.org:fork/repo.git" },
                            { "name": "https", "href": "https://bitbucket.org/fork/repo.git" }
                        ] } } },
            "destination": { "branch": { "name": "main" }, "commit": { "hash": "bbb" },
                             "repository": { "full_name": "team/repo" } }
        }"#;
        let refs = parse_pr_refs(body, 7).unwrap();
        assert_eq!(refs.head_oid, "aaa");
        assert_eq!(
            refs.head_fetch.url.as_deref(),
            Some("https://bitbucket.org/fork/repo.git"),
            "prefers the https clone link"
        );
        assert_eq!(
            refs.head_fetch.refspec,
            "+refs/heads/feature:refs/bonsai/pr/7/head"
        );
        assert!(refs.base_fetch.url.is_none());
    }

    #[test]
    fn malformed_body_is_forge_api_error() {
        let err = parse_pr_refs("not json", 1).unwrap_err();
        assert!(matches!(err, AppError::ForgeApi(_)), "got {err:?}");
    }
}
