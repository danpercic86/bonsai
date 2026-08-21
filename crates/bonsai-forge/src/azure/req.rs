//! Azure DevOps PR complete/abandon request-body builders (split from `dto.rs`
//! to keep it under the size limit). Azure JSON stays private here; the boundary
//! is `MergePrInput` in, a JSON `String` out.

use serde::Serialize;

use bonsai_core::error::AppError;

use crate::types::{MergeMethod, MergePrInput};

// ---- complete / abandon request bodies (camelCase exactly as Azure expects) ----

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CompleteWire<'a> {
    status: &'static str,
    last_merge_source_commit: CommitIdWire<'a>,
    completion_options: CompletionOptionsWire,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CommitIdWire<'a> {
    commit_id: &'a str,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CompletionOptionsWire {
    merge_strategy: &'static str,
    delete_source_branch: bool,
}

/// Azure `mergeStrategy`: `Merge→noFastForward`, `Squash→squash`,
/// `Rebase→rebase`; `FastForward` is not a completion strategy ⇒ `ForgeApi`.
fn azure_merge_strategy(method: MergeMethod) -> Result<&'static str, AppError> {
    match method {
        MergeMethod::Merge => Ok("noFastForward"),
        MergeMethod::Squash => Ok("squash"),
        MergeMethod::Rebase => Ok("rebase"),
        MergeMethod::FastForward => Err(AppError::ForgeApi(
            "fast-forward completion is not available on Azure DevOps".to_string(),
        )),
    }
}

/// Serialize a [`MergePrInput`] into the Azure PR-completion JSON body. Errors for
/// an unsupported method (nothing sent). `head_sha` is REQUIRED
/// (`lastMergeSourceCommit.commitId`, filled command-side); absent ⇒ `ForgeApi`.
pub fn complete_body(input: &MergePrInput) -> Result<String, AppError> {
    let merge_strategy = azure_merge_strategy(input.method)?;
    let commit_id = input.head_sha.as_deref().ok_or_else(|| {
        AppError::ForgeApi(
            "Azure DevOps requires the PR head commit to complete a merge; none was provided"
                .to_string(),
        )
    })?;
    let wire = CompleteWire {
        status: "completed",
        last_merge_source_commit: CommitIdWire { commit_id },
        completion_options: CompletionOptionsWire {
            merge_strategy,
            delete_source_branch: input.delete_source_branch,
        },
    };
    serde_json::to_string(&wire)
        .map_err(|e| AppError::Other(format!("failed to encode completion body: {e}")))
}

/// The Azure abandon body: `{ "status": "abandoned" }`.
pub fn abandon_body() -> String {
    "{\"status\":\"abandoned\"}".to_string()
}
