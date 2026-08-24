//! Shared imports (re-exported) and the `repo_path` helper for the
//! `commands` module, split from the former monolithic `commands.rs`.

pub(crate) use tauri::Emitter;

// P68b streaming conflict resolve. Only the names the command layer NAMES are
// re-exported (the `compose_apply` convention — avoids an unused-import warning
// under -D warnings): `RunLimits`/`ToolPolicy`/`RunControl` travel inside
// `StreamResolveOpts` / the registry and are reached through `ai::` there.
pub(crate) use bonsai_core::ai::{self, AiAvailability, AiRunEvent, AiRunRegistry, RunOpts};
pub(crate) use bonsai_core::assets::{
    self, AgentAsset, AgentAssetInput, AgentAssetInventory, AgentAssetKind, AiAssetInventory,
    AiGeneratedAsset, AssetContent, ContextProfile, ProfileActivation, ProfilePreviewEntry,
    ProfileStore, WorktreeContextStatus,
};
pub(crate) use bonsai_core::error::AppError;
// P87 git-activity observability. `with_activity` (the command bracket) + the
// activity types the op inners name; the emitter is derived to
// `&dyn GitActivityRecorder` inside each `spawn_blocking`.
pub(crate) use super::activity::with_activity;
pub(crate) use bonsai_core::git::activity::{GitActivityCategory, GitActivityRecorder};
// P62b/P63 forge command layer. Only the DTOs the command signatures NAME are
// re-exported (mirrors the `compose_apply` / `ai_operation` convention below —
// avoids an unused-import warning under -D warnings); the still-nested DTOs
// (`PrSummary`/`PrState`/`PrStateFilter`/`CommentKind`/`StatusContext`/
// `CheckRollup`) travel inside these and are never named here. `CommitStatus`
// is named directly by P63's `forge_commit_statuses`.
pub(crate) use bonsai_forge::{
    AccountSource, CommitStatus, CreatePrInput, ForgeAccount, ForgeKind, ForgeRepoContext,
    ForgeViewer, MergePrInput, PrDetail, PrListQuery, PrPage, ReviewComment,
};
pub(crate) use bonsai_core::git::ai_branch_name::{self, BranchNameProposal, BranchNameSource};
pub(crate) use bonsai_core::git::ai_changelog::{self, AiChangelog, ChangelogRange};
pub(crate) use bonsai_core::git::ai_commit::{self, CommitMessageProposal};
pub(crate) use bonsai_core::git::ai_compose::{self, ComposeProposal};
// P54b apply side. Only the names referenced by the command layer are re-exported
// (avoids an unused-import warning, matching the P54a convention above);
// `ComposeGroup`/`ComposeCommit` travel nested inside `ComposePlan`/`ComposeApplyResult`.
pub(crate) use bonsai_core::git::compose_apply::{self, ComposeApplyResult, ComposePlan};
pub(crate) use bonsai_core::git::ai_explain::{self, AiAnalysis, AiAnalysisMode, AiDiffTarget, AiDigestRange};
pub(crate) use bonsai_core::git::ai_history::{self, HistoryAnswer};
pub(crate) use bonsai_core::git::ai_line;
// P55a NL→safe-op planner. Only the names the command layer NAMES are re-exported
// (avoids an unused-import warning under -D warnings, matching the compose_apply
// convention above); `ProposedOperation`/`SafeOp`/`OperationPreview`/`RefChange`/
// `CommitRef`/`DangerLevel` travel nested inside `PlanOutcome`.
pub(crate) use bonsai_core::git::ai_operation::{self, PlanOutcome};
pub(crate) use bonsai_core::git::ai_pr_description::{self, PrDescription};
pub(crate) use bonsai_core::git::ai_resolve::{self, AiResolveProposal};
// P68b. Only the names the command signatures NAME (the `compose_apply`
// convention): `AiResolveFailure` travels nested inside `AiResolveBatch.failed`.
pub(crate) use bonsai_core::git::ai_resolve_stream::{self, AiResolveBatch, StreamResolveOpts};
pub(crate) use bonsai_core::git::ai_summary::{self, AiSummary};
pub(crate) use bonsai_core::git::bisect::{self, BisectOutcome};
pub(crate) use bonsai_core::git::blame::{self, BlameLine, FileHistoryEntry};
// P57b retrieval side. Only the names the command layer NAMES are re-exported
// (avoids an unused-import warning under -D warnings, matching the compose_apply /
// ai_operation convention above); `HistoryHit` travels nested inside
// `HistorySearchResults.hits`.
pub(crate) use bonsai_core::git::history_index::{
    self, HistoryQuery, HistorySearchResults, IndexProgress, IndexStatus,
};
pub(crate) use bonsai_core::git::reflog::{self, ReflogEntry};
pub(crate) use bonsai_core::git::branches::{
    self, BranchesSnapshot, CheckoutResult, CreateBranchHereResult, RenameBranchResult,
};
pub(crate) use bonsai_core::git::cherrypick::{self, CherrypickOutcome};
pub(crate) use bonsai_core::git::clone::{clone_repo as clone_repo_core, init_repo as init_repo_core, CloneProgress};
pub(crate) use bonsai_core::git::commit::{
    amend_commit_with_activity, create_commit_with_activity, CommitResult,
};
pub(crate) use bonsai_core::git::config::{self, ConfigLevelArg, ConfigView};
pub(crate) use bonsai_core::git::conflict::{self, ConflictEntry, ConflictFile, ConflictResolution};
pub(crate) use bonsai_core::git::diff::{
    commit_diff, commit_file_diff, compare_head_diff, compare_head_file_diff, workdir_file_diff,
    CommitDiff, CompareDiff, FileDiff,
};
pub(crate) use bonsai_core::git::image_diff::{self, ImageDiff, ImageDiffRequest};
pub(crate) use bonsai_core::git::merge::{self, MergeOutcome};
pub(crate) use bonsai_core::git::opstate::{read_op_state, RepoOpState};
pub(crate) use bonsai_core::git::rebase::{self, RebaseOutcome};
pub(crate) use bonsai_core::git::rebase_interactive::{self, RebaseTodoOp};
pub(crate) use bonsai_core::git::discard::{
    discard_paths as discard_paths_core, discard_paths_force as discard_paths_force_core,
};
pub(crate) use bonsai_core::git::discard_partial::discard_partial as discard_partial_core;
pub(crate) use bonsai_core::git::remote::{
    add_remote as add_remote_core, fetch_all_with_activity, force_push_with_lease_with_activity,
    list_remotes as list_remotes_core, pull_ff_with_activity, push_current_with_activity,
    remove_remote as remove_remote_core, rename_remote as rename_remote_core,
    set_remote_url as set_remote_url_core, FetchResult, PullResult, PushResult, RemoteInfo,
};
pub(crate) use bonsai_core::git::repo::{read_repo_info, RepoInfo};
pub(crate) use bonsai_core::git::reset::{reset_branch as reset_branch_core, ResetMode};
pub(crate) use bonsai_core::git::revert::{self, RevertOutcome};
pub(crate) use bonsai_core::git::stage::{stage_paths, unstage_paths};
pub(crate) use bonsai_core::git::stale::{self, BranchDeleteResult, StaleReport};
pub(crate) use bonsai_core::git::stage_partial::{
    stage_partial as stage_partial_core, unstage_partial as unstage_partial_core, LineSelection,
};
pub(crate) use bonsai_core::git::stash::{self, ApplyStashOutcome, CreateStashResult, StashEntry, StashScope};
pub(crate) use bonsai_core::git::status::{read_status_with, StatusSnapshot};
pub(crate) use bonsai_core::git::submodule::{
    self, SubmoduleDeinitOutcome, SubmoduleInfo, SubmoduleRemoveOutcome,
};
pub(crate) use bonsai_core::git::worktree::{self, WorktreeInfo};
pub(crate) use bonsai_core::git::worktree_copy::{self, CopyCandidate, CopyPlanEntry, CopySelection};
pub(crate) use bonsai_core::git::tags;
pub(crate) use bonsai_core::git::tag_sync;
// `stream_graph_core` is reached directly by `graph_cache.rs` (the cache-aware
// stream path), so it is intentionally NOT re-exported here.
pub(crate) use bonsai_core::graph::{compute_graph, GraphChunk, GraphLayout};
pub(crate) use bonsai_core::health::{collect_repo_health, RepoHealth};
pub(crate) use crate::scheduler::{self, JobKind, JobOutcome, SchedulerState};
pub(crate) use crate::settings::{
    self, clamp_ai_settings, clamp_auto_fetch, clamp_graph_prefs, clamp_health_refresh,
    clamp_pane_widths, AiAutonomy, AiConflictTools, AutoFetch, GraphPrefs, HealthRefresh,
    IdentityProfile, ListView, PaneWidths, PanelDensity, PrimaryCommitAction, RecentRepo,
    ThemeChoice,
};
pub(crate) use crate::state::{AppState, RepoEntry};
pub(crate) use crate::watcher::spawn_watcher;

/// App-data base dir (`%APPDATA%/com.bonsai.app` on Windows), where the history
/// index is persisted keyed by repo (P57 §4). Mirrors `settings::settings_file`
/// but resolves `app_data_dir()` (regenerable derived data, not user config).
pub(crate) fn app_data_root(app: &tauri::AppHandle) -> Result<std::path::PathBuf, AppError> {
    use tauri::Manager;
    app.path()
        .app_data_dir()
        .map_err(|e| AppError::Other(format!("cannot resolve app data dir: {e}")))
}

/// Canonical workdir path for `repo_id`, or `NoRepo` if it isn't open
/// (P3e contract §3).
pub(crate) fn repo_path(state: &AppState, repo_id: &str) -> Result<std::path::PathBuf, AppError> {
    // Poison recovery (audit §3.8): the guarded HashMap is structurally valid
    // at every point (plain insert/remove/read — same argument as the
    // scheduler's `lock_recover`), so a one-off panic under the lock must not
    // permanently fail EVERY later command.
    let repos = state
        .repos
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    repos
        .get(repo_id)
        .map(|e| e.path.clone())
        .ok_or(AppError::NoRepo)
}

/// Canonical workdir path AND the per-repo graph-layout cache handle (P86 B1),
/// cloned out together under ONE brief map-lock acquisition (same pattern as
/// `repo_path`, so the pair can never straddle a concurrent close/re-open).
/// `NoRepo` if the id isn't open.
pub(crate) fn repo_path_and_graph_cache(
    state: &AppState,
    repo_id: &str,
) -> Result<(std::path::PathBuf, std::sync::Arc<crate::graph_cache::GraphCache>), AppError> {
    let repos = state
        .repos
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    repos
        .get(repo_id)
        .map(|e| (e.path.clone(), e.graph_cache.clone()))
        .ok_or(AppError::NoRepo)
}

/// Canonical workdir path AND the current handle-cache generation for `repo_id`
/// (P88b/B2b), for routing a READ command through `repo_handle::with_repo`.
/// `NoRepo` if the id isn't open. Two brief, independent map locks (path, then
/// generation), each released before any git work — same lock-and-clone
/// discipline as `repo_path`.
pub(crate) fn repo_path_and_gen(
    state: &AppState,
    repo_id: &str,
) -> Result<(std::path::PathBuf, u64), AppError> {
    let path = repo_path(state, repo_id)?;
    let generation = repo_generation(state, repo_id);
    Ok((path, generation))
}

/// Current handle-cache generation for `repo_id` (P88b/B2b); 0 when the id has
/// never been armed (an open repo always bumps to ≥ 1 on open). Poison-recovered
/// like `repo_path`.
pub(crate) fn repo_generation(state: &AppState, repo_id: &str) -> u64 {
    let gens = state
        .repo_generations
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    gens.get(repo_id).copied().unwrap_or(0)
}

/// Bumps `repo_id`'s handle-cache generation (P88b/B2b) so every blocking-pool
/// thread's cached `git2::Repository` for the id is evicted on its next
/// `with_repo` call. Called on the `open_repo` re-arm and on `close_repo`.
/// `wrapping_add` avoids a theoretical overflow panic — the generation only ever
/// needs to DIFFER from the previous value to force eviction.
pub(crate) fn bump_repo_generation(state: &AppState, repo_id: &str) {
    let mut gens = state
        .repo_generations
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let g = gens.entry(repo_id.to_string()).or_insert(0);
    *g = g.wrapping_add(1);
}
