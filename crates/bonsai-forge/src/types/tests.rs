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
    assert_eq!(value_of(&ForgeKind::AzureDevOps), json!("azureDevOps"));
    assert_eq!(value_of(&ForgeKind::Unknown), json!("unknown"));
    // Round-trips from the wire string the TS union sends.
    let got: ForgeKind = serde_json::from_value(json!("gitLab")).unwrap();
    assert_eq!(got, ForgeKind::GitLab);
    let bb: ForgeKind = serde_json::from_value(json!("bitbucket")).unwrap();
    assert_eq!(bb, ForgeKind::Bitbucket);
    let az: ForgeKind = serde_json::from_value(json!("azureDevOps")).unwrap();
    assert_eq!(az, ForgeKind::AzureDevOps);
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
fn forge_account_wire_shape_is_camel_case() {
    let v = value_of(&ForgeAccount {
        account_id: "gitHub:github.com:octocat".into(),
        host: "github.com".into(),
        kind: ForgeKind::GitHub,
        login: Some("octocat".into()),
        avatar_url: Some("https://a/o.png".into()),
        connected: true,
        is_host_default: true,
    });
    assert_keys(
        &v,
        &[
            "accountId",
            "host",
            "kind",
            "login",
            "avatarUrl",
            "connected",
            "isHostDefault",
        ],
    );
}

#[test]
fn account_source_wire_shape_is_camel_case() {
    assert_eq!(value_of(&AccountSource::Override), json!("override"));
    assert_eq!(value_of(&AccountSource::OwnerMatch), json!("ownerMatch"));
    assert_eq!(value_of(&AccountSource::HostDefault), json!("hostDefault"));
    assert_eq!(value_of(&AccountSource::Single), json!("single"));
    assert_eq!(value_of(&AccountSource::None), json!("none"));
    let got: AccountSource = serde_json::from_value(json!("ownerMatch")).unwrap();
    assert_eq!(got, AccountSource::OwnerMatch);
}

#[test]
fn forge_repo_context_wire_shape_is_camel_case() {
    let v = value_of(&ForgeRepoContext {
        provider: ForgeKind::GitHub,
        host: "github.com".into(),
        owner: "o".into(),
        repo: "r".into(),
        project: None,
        remote_name: "origin".into(),
        web_url: "https://github.com/o/r".into(),
        authenticated: true,
        viewer: None,
        resolved_account_id: Some("gitHub:github.com:o".into()),
        account_source: AccountSource::OwnerMatch,
    });
    assert_keys(
        &v,
        &[
            "provider",
            "host",
            "owner",
            "repo",
            "project",
            "remoteName",
            "webUrl",
            "authenticated",
            "viewer",
            "resolvedAccountId",
            "accountSource",
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

