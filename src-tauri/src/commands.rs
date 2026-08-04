use tauri::Emitter;

use bonsai_core::ai::{self, AiAvailability, RunOpts};
use bonsai_core::assets::{
    self, AgentAsset, AgentAssetInput, AgentAssetInventory, AgentAssetKind, AiAssetInventory,
    AiGeneratedAsset, AssetContent, ContextProfile, ProfileActivation, ProfilePreviewEntry,
    ProfileStore, WorktreeContextStatus,
};
use bonsai_core::error::AppError;
use bonsai_core::git::ai_commit::{self, CommitMessageProposal};
use bonsai_core::git::ai_explain::{self, AiAnalysis, AiAnalysisMode, AiDiffTarget, AiDigestRange};
use bonsai_core::git::ai_resolve::{self, AiResolveProposal};
use bonsai_core::git::ai_summary::{self, AiSummary};
use bonsai_core::git::blame::{self, BlameLine, FileHistoryEntry};
use bonsai_core::git::branches::{self, BranchesSnapshot, CheckoutResult, CreateBranchHereResult};
use bonsai_core::git::cherrypick::{self, CherrypickOutcome};
use bonsai_core::git::clone::{clone_repo as clone_repo_core, init_repo as init_repo_core, CloneProgress};
use bonsai_core::git::commit::{amend_commit, create_commit, CommitResult};
use bonsai_core::git::conflict::{self, ConflictEntry, ConflictFile, ConflictResolution};
use bonsai_core::git::diff::{
    commit_diff, commit_file_diff, compare_head_diff, compare_head_file_diff, workdir_file_diff,
    CommitDiff, CompareDiff, FileDiff,
};
use bonsai_core::git::merge::{self, MergeOutcome};
use bonsai_core::git::opstate::{read_op_state, RepoOpState};
use bonsai_core::git::rebase::{self, RebaseOutcome};
use bonsai_core::git::rebase_interactive::{self, RebaseTodoOp};
use bonsai_core::git::discard::{
    discard_paths as discard_paths_core, discard_paths_force as discard_paths_force_core,
};
use bonsai_core::git::discard_partial::discard_partial as discard_partial_core;
use bonsai_core::git::remote::{
    add_remote as add_remote_core, fetch_all, list_remotes as list_remotes_core, pull_ff,
    push_current, remove_remote as remove_remote_core, rename_remote as rename_remote_core,
    set_remote_url as set_remote_url_core, FetchResult, PullResult, PushResult, RemoteInfo,
};
use bonsai_core::git::repo::{read_repo_info, RepoInfo};
use bonsai_core::git::reset::{reset_branch as reset_branch_core, ResetMode};
use bonsai_core::git::revert::{self, RevertOutcome};
use bonsai_core::git::stage::{stage_paths, unstage_paths};
use bonsai_core::git::stale::{self, BranchDeleteResult, StaleReport};
use bonsai_core::git::stage_partial::{
    stage_partial as stage_partial_core, unstage_partial as unstage_partial_core, LineSelection,
};
use bonsai_core::git::stash::{self, ApplyStashOutcome, CreateStashResult, StashEntry, StashScope};
use bonsai_core::git::status::{read_status, StatusSnapshot};
use bonsai_core::git::submodule::{self, SubmoduleInfo};
use bonsai_core::git::worktree::{self, WorktreeInfo};
use bonsai_core::git::worktree_copy::{self, CopyCandidate, CopyPlanEntry, CopySelection};
use bonsai_core::git::tags;
use bonsai_core::graph::{compute_graph, GraphLayout};
use bonsai_core::health::{collect_repo_health, RepoHealth};
use crate::scheduler::{self, JobKind, JobOutcome, SchedulerState};
use crate::settings::{
    self, clamp_auto_fetch, clamp_graph_prefs, clamp_health_refresh, clamp_pane_widths,
    AiAutonomy, AutoFetch, GraphPrefs, HealthRefresh, ListView, PaneWidths, RecentRepo,
    ThemeChoice,
};
use crate::state::{AppState, RepoEntry};
use crate::watcher::spawn_watcher;

/// Payload of the `"repo-changed"` event. `reason` is `"fs"` in M1; future
/// reasons (e.g. `"op"` after a commit) reuse this event. `repo_id` identifies
/// which open repo's watcher fired so the frontend can route it to the right
/// tab (P3e contract §4.1).
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepoChangedPayload {
    pub repo_id: String,
    pub reason: String,
}

/// Result of `open_repo`: the resolved `repoId` plus the repo info (P3e
/// contract §4.2).
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenRepoResult {
    /// Canonical workdir path (P3e contract §2). Meaningful (a map entry
    /// exists) only when `info` is a usable repo; still returned for
    /// non-usable opens so the frontend can key its error UI, but no
    /// entry/watcher is created.
    pub repo_id: String,
    pub info: RepoInfo,
}

/// Opens the folder at `path` as a repository and reports its state.
///
/// Bare repositories are reported (`bare: true`) but NOT stored in state and
/// get no watcher — Bonsai v1 is a working-copy client (M1 contract §3.3).
/// The frontend treats `bare: true` like `isRepo: false`.
///
/// For usable (non-bare) repos this inserts a `RepoEntry` keyed by the
/// canonical workdir path (`repoId`, P3e contract §2) and (re)arms that
/// entry's file watcher: re-invoking on the same path is idempotent and
/// self-heals a dead watcher (this is what the refresh button relies on).
/// Opening an already-open path (case-insensitive match) FOCUSES the existing
/// entry — its id is returned, no duplicate is created.
///
/// A non-usable open (non-repo or bare) inserts NO entry and touches no other
/// entry — there is no single "current repo" anymore, so other tabs are
/// unaffected.
#[tauri::command]
pub async fn open_repo(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    path: String,
) -> Result<OpenRepoResult, AppError> {
    let emit_app = app.clone();
    let result = open_repo_inner(
        state.inner(),
        path,
        move |repo_id: String| {
            let emit_app = emit_app.clone();
            Box::new(move || {
                let _ = emit_app.emit(
                    "repo-changed",
                    RepoChangedPayload {
                        repo_id: repo_id.clone(),
                        reason: "fs".to_string(),
                    },
                );
            })
        },
    )
    .await?;
    let info = &result.info;

    // Recents hook (P1 contract §3.2): record every successful usable open.
    // Uses `info.path` (canonical workdir root), not the raw argument, so
    // "repo root" vs "subfolder" opens dedupe. Save failure is NON-FATAL —
    // the open itself succeeded.
    if info.is_repo && !info.bare {
        match settings::settings_file(&app) {
            Ok(file) => {
                let repo_path = info.path.clone();
                let saved = tauri::async_runtime::spawn_blocking(move || {
                    let mut s = settings::load_from(&file);
                    let now = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|d| d.as_secs() as i64)
                        .unwrap_or(0);
                    settings::record_recent(&mut s, &repo_path, now);
                    settings::save_to(&file, &s)
                })
                .await;
                match saved {
                    Ok(Ok(())) => {}
                    Ok(Err(e)) => {
                        eprintln!("bonsai: failed to save recent repos (non-fatal): {e}");
                    }
                    Err(e) => {
                        eprintln!("bonsai: recent-repos task join error (non-fatal): {e}");
                    }
                }
            }
            Err(e) => {
                eprintln!("bonsai: cannot resolve settings file (non-fatal): {e}");
            }
        }

        // Warm-on-open (P35 §16): pre-fill the in-process HTTPS credential
        // cache for this repo's remotes so the first fetch/pull/push does not
        // pay the cold `git credential fill` (e.g. GCM) spawn cost. This is a
        // fire-and-forget, best-effort step: it runs `list_remotes` (blocking
        // git2, hence off the async path), warms only http(s) remotes, is NEVER
        // awaited, and its failure is silently ignored — the open must stay fast
        // and never block on (or fail because of) credential resolution.
        let warm_workdir = info.path.clone();
        tauri::async_runtime::spawn_blocking(move || {
            let workdir = std::path::Path::new(&warm_workdir);
            let Ok(remotes) = bonsai_core::git::remote::list_remotes(workdir) else {
                return;
            };
            for remote in remotes {
                let Some(url) = remote.url else { continue };
                let lower = url.to_ascii_lowercase();
                if lower.starts_with("https://") || lower.starts_with("http://") {
                    bonsai_core::git::cred_cache::warm(Some(workdir), &url);
                }
            }
        });
    }
    Ok(result)
}

/// Recent successfully-opened repos, most recent first, max 10. Never rejects
/// for a missing/corrupt settings file (`load_from` defaults); only
/// settings-path resolution can error (P1 contract §3.2).
#[tauri::command]
pub async fn get_recent_repos(app: tauri::AppHandle) -> Result<Vec<RecentRepo>, AppError> {
    let file = settings::settings_file(&app)?;
    tauri::async_runtime::spawn_blocking(move || settings::load_from(&file).recent_repos)
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))
}

/// Removes one recents entry (case-insensitive path match) and returns the
/// updated list (P1 contract §3.2).
#[tauri::command]
pub async fn remove_recent_repo(
    app: tauri::AppHandle,
    path: String,
) -> Result<Vec<RecentRepo>, AppError> {
    let file = settings::settings_file(&app)?;
    tauri::async_runtime::spawn_blocking(move || {
        let mut s = settings::load_from(&file);
        s.recent_repos
            .retain(|r| !r.path.eq_ignore_ascii_case(&path));
        settings::save_to(&file, &s)?;
        Ok(s.recent_repos)
    })
    .await
    .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Combined UI settings surfaced to the frontend (P2 contract §2.2).
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UiSettings {
    pub theme: ThemeChoice,
    pub pane_widths: PaneWidths,
    pub list_view: ListView,
    pub auto_fetch: AutoFetch,
    /// Health-refresh background job (P30 D7).
    pub health_refresh: HealthRefresh,
    pub graph: GraphPrefs,
    /// AI features master toggle (P13).
    pub ai_enabled: bool,
    /// AI conflict-resolution autonomy (P13).
    pub ai_conflict_autonomy: AiAutonomy,
    /// One-time consent to send repo content to the local Claude CLI (P13).
    pub ai_consented: bool,
    /// One-time consent to expose open repos to an external MCP client for
    /// reading (P16).
    pub mcp_consented: bool,
    /// One-time consent to let an external MCP client modify open repos (P16c).
    pub mcp_write_consented: bool,
}

/// Partial patch for `set_ui_settings` — only `Some(..)` fields are applied
/// (P2 contract §2.2).
#[derive(Debug, Clone, Copy, PartialEq, Default, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UiSettingsPatch {
    pub theme: Option<ThemeChoice>,
    pub pane_widths: Option<PaneWidths>,
    pub list_view: Option<ListView>,
    /// Whole-struct patch (like `pane_widths`): the frontend sends the entire
    /// nested object when any sub-field changes.
    pub auto_fetch: Option<AutoFetch>,
    /// Whole-struct patch, like `auto_fetch` (P30 D7).
    pub health_refresh: Option<HealthRefresh>,
    pub graph: Option<GraphPrefs>,
    /// AI settings (P13); each patches independently.
    pub ai_enabled: Option<bool>,
    pub ai_conflict_autonomy: Option<AiAutonomy>,
    pub ai_consented: Option<bool>,
    /// MCP consent (P16); patches independently.
    pub mcp_consented: Option<bool>,
    /// MCP write consent (P16c); patches independently.
    pub mcp_write_consented: Option<bool>,
}

/// Pure patch application: only `Some(..)` fields of `patch` mutate `s`; pane
/// widths are clamped on write. Extracted from `set_ui_settings` so its
/// partial-update semantics are unit-testable without a Tauri app
/// (P2a contract §3.4.3).
fn apply_patch(s: &mut settings::Settings, patch: UiSettingsPatch) {
    if let Some(theme) = patch.theme {
        s.theme = theme;
    }
    if let Some(pane_widths) = patch.pane_widths {
        s.pane_widths = clamp_pane_widths(pane_widths);
    }
    if let Some(list_view) = patch.list_view {
        s.list_view = list_view;
    }
    if let Some(auto_fetch) = patch.auto_fetch {
        s.auto_fetch = clamp_auto_fetch(auto_fetch);
    }
    if let Some(health_refresh) = patch.health_refresh {
        s.health_refresh = clamp_health_refresh(health_refresh);
    }
    if let Some(graph) = patch.graph {
        s.graph = clamp_graph_prefs(graph);
    }
    if let Some(ai_enabled) = patch.ai_enabled {
        s.ai_enabled = ai_enabled;
    }
    if let Some(ai_conflict_autonomy) = patch.ai_conflict_autonomy {
        s.ai_conflict_autonomy = ai_conflict_autonomy;
    }
    if let Some(ai_consented) = patch.ai_consented {
        s.ai_consented = ai_consented;
    }
    if let Some(mcp_consented) = patch.mcp_consented {
        s.mcp_consented = mcp_consented;
    }
    if let Some(mcp_write_consented) = patch.mcp_write_consented {
        s.mcp_write_consented = mcp_write_consented;
    }
}

/// Current UI settings (theme + pane widths). Never rejects for a
/// missing/corrupt settings file (same as `get_recent_repos`); only
/// settings-path resolution can error.
#[tauri::command]
pub async fn get_ui_settings(app: tauri::AppHandle) -> Result<UiSettings, AppError> {
    let file = settings::settings_file(&app)?;
    tauri::async_runtime::spawn_blocking(move || {
        let s = settings::load_from(&file);
        UiSettings {
            theme: s.theme,
            pane_widths: s.pane_widths,
            list_view: s.list_view,
            auto_fetch: s.auto_fetch,
            health_refresh: s.health_refresh,
            graph: s.graph,
            ai_enabled: s.ai_enabled,
            ai_conflict_autonomy: s.ai_conflict_autonomy,
            ai_consented: s.ai_consented,
            mcp_consented: s.mcp_consented,
            mcp_write_consented: s.mcp_write_consented,
        }
    })
    .await
    .map_err(|e| AppError::Other(format!("task join error: {e}")))
}

/// Applies a partial patch (only `Some(..)` fields) to the persisted UI
/// settings and returns the resulting `UiSettings`. Save failure surfaces as
/// `AppError::Io` (NOT silently swallowed like the recents hook — the user
/// just took an explicit action, e.g. finished a drag or toggled the theme,
/// and silently losing it would be surprising).
#[tauri::command]
pub async fn set_ui_settings(
    app: tauri::AppHandle,
    sched: tauri::State<'_, SchedulerState>,
    patch: UiSettingsPatch,
) -> Result<UiSettings, AppError> {
    let file = settings::settings_file(&app)?;
    let ui = tauri::async_runtime::spawn_blocking(move || -> Result<UiSettings, AppError> {
        let mut s = settings::load_from(&file);
        apply_patch(&mut s, patch);
        settings::save_to(&file, &s)?;
        Ok(UiSettings {
            theme: s.theme,
            pane_widths: s.pane_widths,
            list_view: s.list_view,
            auto_fetch: s.auto_fetch,
            health_refresh: s.health_refresh,
            graph: s.graph,
            ai_enabled: s.ai_enabled,
            ai_conflict_autonomy: s.ai_conflict_autonomy,
            ai_consented: s.ai_consented,
            mcp_consented: s.mcp_consented,
            mcp_write_consented: s.mcp_write_consented,
        })
    })
    .await
    .map_err(|e| AppError::Other(format!("task join error: {e}")))??;
    // P30 D7: push the (already clamped + persisted) job config into the
    // scheduler so interval/enable changes take effect on the next tick.
    scheduler::apply_config(
        &sched,
        scheduler::JobsConfig {
            auto_fetch: ui.auto_fetch,
            health_refresh: ui.health_refresh,
        },
    );
    Ok(ui)
}

/// Persisted multi-tab session (P3e §6.1): the open tabs (in display order,
/// repoIds == canonical workdir paths) and the active tab's repoId. Written as
/// a whole unit — tabs change atomically, not via partial patch.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionState {
    pub open_repos: Vec<String>,
    pub active_repo: Option<String>,
}

/// Current persisted session (open tabs + active tab). Never rejects for a
/// missing/corrupt settings file (same as `get_ui_settings`): defaults to an
/// empty session. Only settings-path resolution can error.
#[tauri::command]
pub async fn get_session(app: tauri::AppHandle) -> Result<SessionState, AppError> {
    let file = settings::settings_file(&app)?;
    tauri::async_runtime::spawn_blocking(move || {
        let s = settings::load_from(&file);
        SessionState {
            open_repos: s.open_repos,
            active_repo: s.active_repo,
        }
    })
    .await
    .map_err(|e| AppError::Other(format!("task join error: {e}")))
}

/// Persists the WHOLE session (tabs change as a unit — no partial patch).
/// Loads the current settings, overwrites the session fields, and saves. Save
/// failure surfaces as `AppError::Io` (NOT swallowed — mirrors
/// `set_ui_settings`; the user just opened/closed/switched a tab and silently
/// losing it would be surprising).
#[tauri::command]
pub async fn set_session(app: tauri::AppHandle, session: SessionState) -> Result<(), AppError> {
    let file = settings::settings_file(&app)?;
    tauri::async_runtime::spawn_blocking(move || {
        let mut s = settings::load_from(&file);
        s.open_repos = session.open_repos;
        s.active_repo = session.active_repo;
        settings::save_to(&file, &s)
    })
    .await
    .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Records the frontend's focused-tab repoId (or `None` when no repo is
/// focused). Lock-and-clone discipline like `repo_path`; poisoned lock →
/// `Other`. This seeds a new embedded-MCP session's initial repo (P16 §5) — it
/// does NOT change any already-connected AI session's selection.
#[tauri::command]
pub async fn set_active_repo(
    state: tauri::State<'_, AppState>,
    repo_id: Option<String>,
) -> Result<(), AppError> {
    *state
        .active_repo
        .lock()
        .map_err(|_| AppError::Other("state lock poisoned".to_string()))? = repo_id;
    Ok(())
}

/// Current embedded-MCP server status for the Settings panel (P16 §10.1).
#[tauri::command]
pub async fn get_mcp_status(
    mcp_state: tauri::State<'_, crate::mcp::McpServerState>,
) -> Result<crate::mcp::McpStatus, AppError> {
    Ok(crate::mcp::status_of(&mcp_state))
}

/// Starts or stops the embedded MCP server (P16 §6). Read-only in P16b (the
/// write-gate is P16c). Returns the resulting status; emits `mcp-server-changed`.
#[tauri::command]
pub async fn set_mcp_enabled(
    app: tauri::AppHandle,
    mcp_state: tauri::State<'_, crate::mcp::McpServerState>,
    enabled: bool,
) -> Result<crate::mcp::McpStatus, AppError> {
    crate::mcp::set_enabled(&app, &mcp_state, enabled).await
}

/// Flips the embedded-MCP write-gate (P16c §9). Persists `mcp_allow_write` and,
/// if the server is running, BOUNCES it (stop + restart on the same token/port)
/// so the 20 mutation tools (de)register and live sessions re-negotiate.
/// Returns the resulting status; emits `mcp-server-changed`.
#[tauri::command]
pub async fn set_mcp_allow_write(
    app: tauri::AppHandle,
    mcp_state: tauri::State<'_, crate::mcp::McpServerState>,
    allow_write: bool,
) -> Result<crate::mcp::McpStatus, AppError> {
    crate::mcp::set_allow_write(&app, &mcp_state, allow_write).await
}

/// Registers Bonsai's running embedded MCP server with the local `claude` CLI
/// (P16). Reads the live `url` + `token` from the running server (errors if the
/// server is not enabled). `scope` is `"user"` (register globally) or `"local"`
/// (register in the open repo, private/not committed). cwd = `repo_path` when
/// given (required for a meaningful `local` registration), else the process cwd.
/// The `claude mcp add` argv is built in `bonsai-core` as an argument list, so
/// the variadic `--header` cannot swallow the URL. Errors:
/// `aiUnavailable` | `aiFailed` | `other`.
#[tauri::command]
pub async fn register_mcp_with_claude(
    scope: String,
    repo_path: Option<String>,
    mcp_state: tauri::State<'_, crate::mcp::McpServerState>,
) -> Result<(), AppError> {
    let status = crate::mcp::status_of(&mcp_state);
    let (url, token) = match (status.url, status.token) {
        (Some(u), Some(t)) => (u, t),
        _ => return Err(AppError::Other("MCP server is not running".to_string())),
    };
    let cwd = match repo_path {
        Some(p) => std::path::PathBuf::from(p),
        None => std::env::current_dir()
            .map_err(|e| AppError::Other(format!("could not resolve current dir: {e}")))?,
    };
    tauri::async_runtime::spawn_blocking(move || {
        ai::register_with_claude(&url, &token, &scope, &cwd)
    })
    .await
    .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Runtime-free core of `open_repo` (unit-testable without a Tauri app).
/// `make_on_change` is given the resolved `repo_id` and returns the watcher
/// callback for that repo; the command wires it to an app-wide
/// `"repo-changed"` emit carrying that id. Tests pass `|_id| Box::new(|| {})`
/// (no Tauri runtime).
async fn open_repo_inner<F>(
    state: &AppState,
    path: String,
    make_on_change: F,
) -> Result<OpenRepoResult, AppError>
where
    F: FnOnce(String) -> Box<dyn Fn() + Send + 'static>,
{
    let path_buf = std::path::PathBuf::from(&path);
    let info = tauri::async_runtime::spawn_blocking(move || read_repo_info(&path_buf))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))??;

    // repoId == canonical workdir path string (P3e contract §2).
    let mut repo_id = info.path.clone();

    if info.is_repo && !info.bare {
        let workdir = std::path::PathBuf::from(&info.path);

        // Dedupe scan (P3e contract §2): if a case-insensitive match is already
        // open, reuse its exact key so we FOCUS it instead of inserting a
        // duplicate. Only compute the callback once we know the final id.
        {
            let repos = state
                .repos
                .lock()
                .map_err(|_| AppError::Other("state lock poisoned".to_string()))?;
            if let Some(existing) = repos
                .keys()
                .find(|k| k.eq_ignore_ascii_case(&repo_id))
                .cloned()
            {
                repo_id = existing;
            }
        }

        let on_change = make_on_change(repo_id.clone());

        // Watch failure is non-fatal (M1 contract §4): manual refresh + focus
        // rescan keep the app correct even without filesystem events. Build the
        // handle OUTSIDE the map lock (the initial watch registration is
        // synchronous), then insert/replace the entry under the lock. Replacing
        // an existing entry drops its old watcher here (self-heal); to keep
        // that drop-join off the map lock we take the old entry out first.
        let watcher = match spawn_watcher(&workdir, on_change) {
            Ok(handle) => Some(handle),
            Err(e) => {
                eprintln!("bonsai: file watcher failed to start (falling back to manual refresh): {e}");
                None
            }
        };

        let previous = {
            let mut repos = state
                .repos
                .lock()
                .map_err(|_| AppError::Other("state lock poisoned".to_string()))?;
            repos.insert(
                repo_id.clone(),
                RepoEntry {
                    path: workdir,
                    watcher,
                },
            )
        };
        // Drop the replaced entry (its watcher's debounce thread joins) off the
        // map lock.
        drop(previous);
    }
    // Non-usable open (non-repo or bare): insert nothing, touch no other entry.

    Ok(OpenRepoResult { repo_id, info })
}

/// Closes the tab identified by `repo_id`, stopping just that repo's watcher.
/// Idempotent: closing an unknown/already-closed id is `Ok(())` (P3e contract
/// §4.3).
#[tauri::command]
pub async fn close_repo(
    state: tauri::State<'_, AppState>,
    repo_id: String,
) -> Result<(), AppError> {
    close_repo_inner(state.inner(), &repo_id).await
}

/// Runtime-free core of `close_repo` (unit-testable without a Tauri app).
async fn close_repo_inner(state: &AppState, repo_id: &str) -> Result<(), AppError> {
    // Take the entry out UNDER the lock, then drop it OUTSIDE the lock so the
    // WatcherHandle's debounce-thread join (≤ ~300 ms) doesn't hold the map
    // lock.
    let entry = {
        let mut repos = state
            .repos
            .lock()
            .map_err(|_| AppError::Other("state lock poisoned".to_string()))?;
        repos.remove(repo_id)
    };
    drop(entry); // watcher stops, debounce thread joins here
    Ok(())
}

/// Computes the working-directory status of `repo_id`.
#[tauri::command]
pub async fn get_status(
    state: tauri::State<'_, AppState>,
    repo_id: String,
) -> Result<StatusSnapshot, AppError> {
    get_status_inner(state.inner(), &repo_id).await
}

/// Runtime-free core of `get_status` (unit-testable without a Tauri app).
async fn get_status_inner(state: &AppState, repo_id: &str) -> Result<StatusSnapshot, AppError> {
    let path = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || read_status(&path))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Computes the full commit-graph layout of `repo_id`.
///
/// Unborn-HEAD / zero-ref repos yield an empty layout (M2 contract §2.1),
/// not an error; `NoRepo` when nothing is open under that id.
#[tauri::command]
pub async fn get_graph(
    state: tauri::State<'_, AppState>,
    repo_id: String,
) -> Result<GraphLayout, AppError> {
    get_graph_inner(state.inner(), &repo_id).await
}

/// Runtime-free core of `get_graph` (unit-testable without a Tauri app).
async fn get_graph_inner(state: &AppState, repo_id: &str) -> Result<GraphLayout, AppError> {
    let path = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || compute_graph(&path))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Canonical workdir path for `repo_id`, or `NoRepo` if it isn't open
/// (P3e contract §3).
fn repo_path(state: &AppState, repo_id: &str) -> Result<std::path::PathBuf, AppError> {
    let repos = state
        .repos
        .lock()
        .map_err(|_| AppError::Other("state lock poisoned".to_string()))?;
    repos
        .get(repo_id)
        .map(|e| e.path.clone())
        .ok_or(AppError::NoRepo)
}

/// Stages the given worktree-relative paths (atomic batch, M3 contract §2.2).
/// Does NOT emit `repo-changed` — the frontend refetches imperatively after
/// every successful mutation (§2.7).
#[tauri::command]
pub async fn stage(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    paths: Vec<String>,
) -> Result<(), AppError> {
    stage_inner(state.inner(), &repo_id, paths).await
}

/// Runtime-free core of `stage` (unit-testable without a Tauri app).
async fn stage_inner(state: &AppState, repo_id: &str, paths: Vec<String>) -> Result<(), AppError> {
    let path = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || stage_paths(&path, &paths))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Unstages the given worktree-relative paths (atomic batch). Safe: the
/// worktree is never touched.
#[tauri::command]
pub async fn unstage(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    paths: Vec<String>,
) -> Result<(), AppError> {
    unstage_inner(state.inner(), &repo_id, paths).await
}

/// Runtime-free core of `unstage` (unit-testable without a Tauri app).
async fn unstage_inner(
    state: &AppState,
    repo_id: &str,
    paths: Vec<String>,
) -> Result<(), AppError> {
    let path = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || unstage_paths(&path, &paths))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Creates a commit from the current index. Errors:
/// `emptyMessage` | `configMissing` | `nothingToCommit` | `git` | `noRepo`.
#[tauri::command]
pub async fn commit(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    message: String,
) -> Result<CommitResult, AppError> {
    commit_inner(state.inner(), &repo_id, message).await
}

/// Runtime-free core of `commit` (unit-testable without a Tauri app).
async fn commit_inner(
    state: &AppState,
    repo_id: &str,
    message: String,
) -> Result<CommitResult, AppError> {
    let path = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || create_commit(&path, &message))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Diff of one working-dir file (M4 contract §2.2/§2.8).
/// `staged == false`: index vs workdir; `staged == true`: HEAD vs index.
/// `orig_path`: pass `StatusEntry.origPath` for renames.
#[tauri::command]
pub async fn get_workdir_file_diff(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    path: String,
    orig_path: Option<String>,
    staged: bool,
    full_context: bool,
) -> Result<FileDiff, AppError> {
    get_workdir_file_diff_inner(state.inner(), &repo_id, path, orig_path, staged, full_context).await
}

/// Runtime-free core of `get_workdir_file_diff` (unit-testable without a Tauri app).
async fn get_workdir_file_diff_inner(
    state: &AppState,
    repo_id: &str,
    path: String,
    orig_path: Option<String>,
    staged: bool,
    full_context: bool,
) -> Result<FileDiff, AppError> {
    let workdir = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || {
        workdir_file_diff(&workdir, &path, orig_path.as_deref(), staged, full_context)
    })
    .await
    .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Stages only the selected changed lines of one working-dir file (index moves
/// toward the workdir; P17 §2.7). Empty selection is a no-op. Does NOT emit
/// `repo-changed` — the frontend refetches imperatively.
/// Errors: `noRepo` | `git` | `other` (stale/unsupported/invalid path).
#[tauri::command]
pub async fn stage_partial(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    path: String,
    orig_path: Option<String>,
    selection: Vec<LineSelection>,
) -> Result<(), AppError> {
    stage_partial_inner(state.inner(), &repo_id, path, orig_path, selection).await
}

/// Runtime-free core of `stage_partial` (unit-testable without a Tauri app).
async fn stage_partial_inner(
    state: &AppState,
    repo_id: &str,
    path: String,
    orig_path: Option<String>,
    selection: Vec<LineSelection>,
) -> Result<(), AppError> {
    let workdir = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || {
        stage_partial_core(&workdir, &path, orig_path.as_deref(), &selection)
    })
    .await
    .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Unstages only the selected changed lines of one staged file (index moves
/// toward HEAD; P17 §2.7). Empty selection is a no-op. Does NOT emit
/// `repo-changed` — the frontend refetches imperatively.
/// Errors: `noRepo` | `git` | `other` (stale/unsupported/invalid path).
#[tauri::command]
pub async fn unstage_partial(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    path: String,
    orig_path: Option<String>,
    selection: Vec<LineSelection>,
) -> Result<(), AppError> {
    unstage_partial_inner(state.inner(), &repo_id, path, orig_path, selection).await
}

/// Runtime-free core of `unstage_partial` (unit-testable without a Tauri app).
async fn unstage_partial_inner(
    state: &AppState,
    repo_id: &str,
    path: String,
    orig_path: Option<String>,
    selection: Vec<LineSelection>,
) -> Result<(), AppError> {
    let workdir = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || {
        unstage_partial_core(&workdir, &path, orig_path.as_deref(), &selection)
    })
    .await
    .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Discards the selected changed lines of one tracked working-dir file: the
/// WORKTREE moves toward the INDEX; the index is never modified (P28 §2.1).
/// DESTRUCTIVE — the UI confirms first. Empty selection is a no-op. Does NOT
/// emit `repo-changed` — the frontend refetches imperatively.
/// Errors: `noRepo` | `git` (untracked) | `other` (stale/unsupported/invalid path).
#[tauri::command]
pub async fn discard_partial(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    path: String,
    orig_path: Option<String>,
    selection: Vec<LineSelection>,
) -> Result<(), AppError> {
    discard_partial_inner(state.inner(), &repo_id, path, orig_path, selection).await
}

/// Runtime-free core of `discard_partial` (unit-testable without a Tauri app).
async fn discard_partial_inner(
    state: &AppState,
    repo_id: &str,
    path: String,
    orig_path: Option<String>,
    selection: Vec<LineSelection>,
) -> Result<(), AppError> {
    let workdir = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || {
        discard_partial_core(&workdir, &path, orig_path.as_deref(), &selection)
    })
    .await
    .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Commit details + per-file headers for `oid` vs its first parent
/// (M4 contract §2.2/§2.8). Errors: `noRepo` | `git`.
#[tauri::command]
pub async fn get_commit_diff(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    oid: String,
) -> Result<CommitDiff, AppError> {
    get_commit_diff_inner(state.inner(), &repo_id, oid).await
}

/// Runtime-free core of `get_commit_diff` (unit-testable without a Tauri app).
async fn get_commit_diff_inner(
    state: &AppState,
    repo_id: &str,
    oid: String,
) -> Result<CommitDiff, AppError> {
    let workdir = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || commit_diff(&workdir, &oid))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Hunks for ONE file of a commit's first-parent diff (M4 contract §2.2/§2.8).
#[tauri::command]
pub async fn get_commit_file_diff(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    oid: String,
    path: String,
    orig_path: Option<String>,
    full_context: bool,
) -> Result<FileDiff, AppError> {
    get_commit_file_diff_inner(state.inner(), &repo_id, oid, path, orig_path, full_context).await
}

/// Runtime-free core of `get_commit_file_diff` (unit-testable without a Tauri app).
async fn get_commit_file_diff_inner(
    state: &AppState,
    repo_id: &str,
    oid: String,
    path: String,
    orig_path: Option<String>,
    full_context: bool,
) -> Result<FileDiff, AppError> {
    let workdir = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || {
        commit_file_diff(&workdir, &oid, &path, orig_path.as_deref(), full_context)
    })
    .await
    .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// HEAD → `oid` tree comparison (P5 §1.2). Errors: `noRepo` | `git`.
#[tauri::command]
pub async fn compare_with_head(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    oid: String,
) -> Result<CompareDiff, AppError> {
    compare_with_head_inner(state.inner(), &repo_id, oid).await
}

/// Runtime-free core of `compare_with_head` (unit-testable without a Tauri app).
async fn compare_with_head_inner(
    state: &AppState,
    repo_id: &str,
    oid: String,
) -> Result<CompareDiff, AppError> {
    let workdir = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || compare_head_diff(&workdir, &oid))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Hunks for one file of the HEAD → `oid` comparison. Errors: `noRepo` | `git`.
#[tauri::command]
pub async fn compare_with_head_file_diff(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    oid: String,
    path: String,
    orig_path: Option<String>,
    full_context: bool,
) -> Result<FileDiff, AppError> {
    compare_with_head_file_diff_inner(state.inner(), &repo_id, oid, path, orig_path, full_context)
        .await
}

/// Runtime-free core of `compare_with_head_file_diff`.
async fn compare_with_head_file_diff_inner(
    state: &AppState,
    repo_id: &str,
    oid: String,
    path: String,
    orig_path: Option<String>,
    full_context: bool,
) -> Result<FileDiff, AppError> {
    let workdir = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || {
        compare_head_file_diff(&workdir, &oid, &path, orig_path.as_deref(), full_context)
    })
    .await
    .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// One snapshot of local branches + remote-tracking branches + tags + HEAD
/// (M5 contract §2.2/§2.8). Errors: `noRepo` | `git`.
#[tauri::command]
pub async fn list_branches(
    state: tauri::State<'_, AppState>,
    repo_id: String,
) -> Result<BranchesSnapshot, AppError> {
    list_branches_inner(state.inner(), &repo_id).await
}

/// Runtime-free core of `list_branches` (unit-testable without a Tauri app).
async fn list_branches_inner(
    state: &AppState,
    repo_id: &str,
) -> Result<BranchesSnapshot, AppError> {
    let path = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || branches::list_refs(&path))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Creates a local branch at the current HEAD commit — does NOT check out
/// (M5 contract §2.4). Errors: `invalidName` | `branchExists` | `git` | `noRepo`.
/// Does NOT emit `repo-changed` — the frontend refetches imperatively.
#[tauri::command]
pub async fn create_branch(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    name: String,
) -> Result<(), AppError> {
    create_branch_inner(state.inner(), &repo_id, name).await
}

/// Runtime-free core of `create_branch` (unit-testable without a Tauri app).
async fn create_branch_inner(
    state: &AppState,
    repo_id: &str,
    name: String,
) -> Result<(), AppError> {
    let path = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || branches::create_branch(&path, &name))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Creates local branch `name` at commit `oid`, auto-stashing/re-applying
/// uncommitted work across the checkout (P11 §1). Errors: `invalidName` |
/// `branchExists` | `operationInProgress` | `configMissing` | `checkoutConflict`
/// | `git` | `noRepo`. Does NOT emit `repo-changed`.
#[tauri::command]
pub async fn create_branch_here(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    name: String,
    oid: String,
) -> Result<CreateBranchHereResult, AppError> {
    create_branch_here_inner(state.inner(), &repo_id, name, oid).await
}

/// Runtime-free core of `create_branch_here` (unit-testable without a Tauri app).
async fn create_branch_here_inner(
    state: &AppState,
    repo_id: &str,
    name: String,
    oid: String,
) -> Result<CreateBranchHereResult, AppError> {
    let path = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || branches::create_branch_here(&path, &name, &oid))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Dirty-safe checkout of a LOCAL branch (P33): auto-stash -> switch -> auto FF
/// to upstream (no fetch) -> re-apply stash. A conflicted re-apply is a SUCCESS
/// carrying `apply: Some(conflicts)` (stash retained). Errors: `branchNotFound`
/// | `operationInProgress` | `configMissing` | `checkoutConflict` | `git` |
/// `noRepo`. Does NOT emit `repo-changed` (frontend calls refreshAll).
#[tauri::command]
pub async fn checkout_branch(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    name: String,
) -> Result<CheckoutResult, AppError> {
    checkout_branch_inner(state.inner(), &repo_id, name).await
}

/// Runtime-free core of `checkout_branch` (unit-testable without a Tauri app).
async fn checkout_branch_inner(
    state: &AppState,
    repo_id: &str,
    name: String,
) -> Result<CheckoutResult, AppError> {
    let path = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || branches::checkout_branch_autostash(&path, &name))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Deletes a LOCAL, fully merged, non-current branch (M5 contract §2.6 —
/// unmerged deletion is blocked; no force-delete in v1).
/// Errors: `branchNotFound` | `unmergedBranch` | `git` | `noRepo`.
#[tauri::command]
pub async fn delete_branch(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    name: String,
) -> Result<(), AppError> {
    delete_branch_inner(state.inner(), &repo_id, name).await
}

/// Runtime-free core of `delete_branch` (unit-testable without a Tauri app).
async fn delete_branch_inner(
    state: &AppState,
    repo_id: &str,
    name: String,
) -> Result<(), AppError> {
    let path = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || branches::delete_branch(&path, &name))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// GitKraken-style remote checkout: create/reuse a local tracking branch for
/// `name` ("<remote>/<branch>") and safe-checkout it (P6 §2.2).
/// Errors: `invalidName` | `branchNotFound` | `checkoutConflict` | `git` | `noRepo`.
#[tauri::command]
pub async fn checkout_remote(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    name: String,
) -> Result<(), AppError> {
    checkout_remote_inner(state.inner(), &repo_id, name).await
}

/// Runtime-free core of `checkout_remote` (unit-testable without a Tauri app).
async fn checkout_remote_inner(
    state: &AppState,
    repo_id: &str,
    name: String,
) -> Result<(), AppError> {
    let path = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || branches::checkout_remote(&path, &name))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Deletes the LOCAL remote-tracking ref `name` — does NOT touch the server
/// (P6 §2.3). Errors: `branchNotFound` | `git` | `noRepo`.
#[tauri::command]
pub async fn delete_remote_tracking(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    name: String,
) -> Result<(), AppError> {
    delete_remote_tracking_inner(state.inner(), &repo_id, name).await
}

/// Runtime-free core of `delete_remote_tracking` (unit-testable without a Tauri app).
async fn delete_remote_tracking_inner(
    state: &AppState,
    repo_id: &str,
    name: String,
) -> Result<(), AppError> {
    let path = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || branches::delete_remote_tracking(&path, &name))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Classifies local branches safe to delete (merged into `base` OR
/// upstream-gone) — read-only, touches nothing (P25 §4.1). `base` auto-resolves
/// when omitted. Pure git; NO consent gate. Errors: `git` | `noRepo`.
#[tauri::command]
pub async fn list_stale_branches(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    base: Option<String>,
) -> Result<StaleReport, AppError> {
    list_stale_branches_inner(state.inner(), &repo_id, base).await
}

/// Runtime-free core of `list_stale_branches` (unit-testable without a Tauri app).
async fn list_stale_branches_inner(
    state: &AppState,
    repo_id: &str,
    base: Option<String>,
) -> Result<StaleReport, AppError> {
    let path = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || {
        stale::find_stale_branches(&path, base.as_deref())
    })
    .await
    .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Batch-deletes the caller-supplied branch names that are STILL safe against a
/// freshly-recomputed stale set — refusing the current branch, the base, and
/// anything not re-verified as stale (P25 §4.3). Per-branch outcomes are DATA,
/// never thrown; a partial batch returns `Ok(results)`. Pure git; NO consent
/// gate. Does NOT emit `repo-changed` — the frontend refetches imperatively.
/// Errors (whole-call): `git` (bad base) | `noRepo`.
#[tauri::command]
pub async fn delete_branches(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    names: Vec<String>,
    base: Option<String>,
) -> Result<Vec<BranchDeleteResult>, AppError> {
    delete_branches_inner(state.inner(), &repo_id, names, base).await
}

/// Runtime-free core of `delete_branches` (unit-testable without a Tauri app).
async fn delete_branches_inner(
    state: &AppState,
    repo_id: &str,
    names: Vec<String>,
    base: Option<String>,
) -> Result<Vec<BranchDeleteResult>, AppError> {
    let path = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || {
        stale::delete_branches(&path, &names, base.as_deref())
    })
    .await
    .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Fetches every configured remote, sequentially, fail-fast (M6 contract
/// §2.4/§9). Errors: `noRemote` | `authFailed` | `networkError` | `git`
/// | `noRepo`. Does NOT emit `repo-changed` — the frontend refetches
/// imperatively (the watcher also fires and is absorbed by request-id guards).
#[tauri::command]
pub async fn fetch(
    state: tauri::State<'_, AppState>,
    repo_id: String,
) -> Result<FetchResult, AppError> {
    fetch_inner(state.inner(), &repo_id).await
}

/// Runtime-free core of `fetch` (unit-testable without a Tauri app).
async fn fetch_inner(state: &AppState, repo_id: &str) -> Result<FetchResult, AppError> {
    let path = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || fetch_all(&path))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Fetches the upstream's remote + fast-forwards ONLY (M6 contract §2.5).
/// Errors: `noUpstream` | `authFailed` | `networkError` | `checkoutConflict`
/// | `git` | `noRepo`.
#[tauri::command]
pub async fn pull(
    state: tauri::State<'_, AppState>,
    repo_id: String,
) -> Result<PullResult, AppError> {
    pull_inner(state.inner(), &repo_id).await
}

/// Runtime-free core of `pull` (unit-testable without a Tauri app).
async fn pull_inner(state: &AppState, repo_id: &str) -> Result<PullResult, AppError> {
    let path = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || pull_ff(&path))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Pushes the current branch to its upstream — or origin/<branch> + set
/// upstream when none (M6 contract §2.6). Never force. Errors: `noRemote`
/// | `authFailed` | `networkError` | `pushRejected` | `git` | `noRepo`.
#[tauri::command]
pub async fn push(
    state: tauri::State<'_, AppState>,
    repo_id: String,
) -> Result<PushResult, AppError> {
    push_inner(state.inner(), &repo_id).await
}

/// Runtime-free core of `push` (unit-testable without a Tauri app).
async fn push_inner(state: &AppState, repo_id: &str) -> Result<PushResult, AppError> {
    let path = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || push_current(&path))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Current operation state (merge / rebase / cherry-pick / revert / none).
/// Part of the frontend refresh batch (P3c contract §6). Errors: `noRepo`
/// | `git`.
#[tauri::command]
pub async fn get_op_state(
    state: tauri::State<'_, AppState>,
    repo_id: String,
) -> Result<RepoOpState, AppError> {
    get_op_state_inner(state.inner(), &repo_id).await
}

/// Runtime-free core of `get_op_state` (unit-testable without a Tauri app).
async fn get_op_state_inner(state: &AppState, repo_id: &str) -> Result<RepoOpState, AppError> {
    let path = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || read_op_state(&path))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// One background job's status for the UI readout (P30 contract §3).
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JobStatus {
    pub job: JobKind,
    pub enabled: bool,
    pub last_run_ms: Option<i64>,
    pub last_outcome: Option<JobOutcome>,
    pub last_error: Option<String>,
    pub consecutive_failures: u32,
    pub in_backoff: bool,
    /// Estimate; `None` when disabled (or never seen by the loop yet).
    pub next_run_ms: Option<i64>,
}

/// Background-job status for one open repo — exactly 2 entries (autoFetch,
/// healthRefresh). Errors: `noRepo` for an unknown repoId (P30 §3).
#[tauri::command]
pub async fn get_job_status(
    state: tauri::State<'_, AppState>,
    sched: tauri::State<'_, SchedulerState>,
    repo_id: String,
) -> Result<Vec<JobStatus>, AppError> {
    get_job_status_inner(state.inner(), &sched, &repo_id)
}

/// Runtime-free core of `get_job_status` (unit-testable without a Tauri app).
fn get_job_status_inner(
    state: &AppState,
    sched: &SchedulerState,
    repo_id: &str,
) -> Result<Vec<JobStatus>, AppError> {
    repo_path(state, repo_id)?; // NoRepo gate only
    // Recover from poison like the scheduler loop itself does (scheduler.rs
    // lock_recover rationale) — a single panicked job must not make this
    // command fail forever.
    let cfg = *crate::scheduler::lock_recover(&sched.cfg);
    let jobs = crate::scheduler::lock_recover(&sched.jobs);
    Ok([JobKind::AutoFetch, JobKind::HealthRefresh]
        .into_iter()
        .map(|job| {
            let (enabled, base_ms) = cfg.job_params(job);
            let rt = jobs
                .get(&(repo_id.to_string(), job))
                .cloned()
                .unwrap_or_default();
            JobStatus {
                job,
                enabled,
                last_run_ms: rt.last_run_ms,
                last_outcome: rt.last_outcome,
                last_error: rt.last_error,
                consecutive_failures: rt.consecutive_failures,
                in_backoff: rt.consecutive_failures >= scheduler::BACKOFF_THRESHOLD,
                next_run_ms: scheduler::next_run_estimate_ms(
                    enabled,
                    base_ms,
                    rt.last_run_ms,
                    rt.consecutive_failures,
                ),
            }
        })
        .collect())
}

/// Manual "run now" (P30 D10): fire-and-forget — `Ok(())` once the job is
/// started; the result arrives via `job-status-changed`. Ignores backoff
/// delay; suppression + backoff-reset rules apply as for a scheduled run.
/// Errors: `noRepo` | `Other("job already running")`.
#[tauri::command]
pub async fn run_job_now(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    sched: tauri::State<'_, SchedulerState>,
    repo_id: String,
    job: JobKind,
) -> Result<(), AppError> {
    let path = repo_path(state.inner(), &repo_id)?;
    scheduler::start_job_now(
        &sched,
        &repo_id,
        path,
        job,
        scheduler::unix_now_ms(),
        scheduler::emitter_for(app),
    )
    .map(|_handle| ()) // detached (fire-and-forget)
}

/// Merges a local or remote-tracking branch into the current branch (P3c
/// contract §4). Errors: `operationInProgress` | `branchNotFound`
/// | `checkoutConflict` | `configMissing` | `git` | `noRepo`. Does NOT emit
/// `repo-changed` — the frontend refetches imperatively.
#[tauri::command]
pub async fn merge_branch(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    name: String,
) -> Result<MergeOutcome, AppError> {
    merge_branch_inner(state.inner(), &repo_id, name).await
}

/// Runtime-free core of `merge_branch` (unit-testable without a Tauri app).
async fn merge_branch_inner(
    state: &AppState,
    repo_id: &str,
    name: String,
) -> Result<MergeOutcome, AppError> {
    let path = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || merge::merge_branch(&path, &name))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Finalizes a paused merge as a 2(+)-parent commit (P3c contract §4.4).
/// Errors: `noOperationInProgress` | `unresolvedConflicts` | `emptyMessage`
/// | `configMissing` | `git` | `noRepo`.
#[tauri::command]
pub async fn commit_merge(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    message: String,
) -> Result<CommitResult, AppError> {
    commit_merge_inner(state.inner(), &repo_id, message).await
}

/// Runtime-free core of `commit_merge` (unit-testable without a Tauri app).
async fn commit_merge_inner(
    state: &AppState,
    repo_id: &str,
    message: String,
) -> Result<CommitResult, AppError> {
    let path = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || merge::commit_merge(&path, &message))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Aborts a paused merge (worktree-destructive for merge-touched files —
/// the UI confirms first; P3c contract §4.5). Errors: `noOperationInProgress`
/// | `git` | `noRepo`.
#[tauri::command]
pub async fn abort_merge(
    state: tauri::State<'_, AppState>,
    repo_id: String,
) -> Result<(), AppError> {
    abort_merge_inner(state.inner(), &repo_id).await
}

/// Runtime-free core of `abort_merge` (unit-testable without a Tauri app).
async fn abort_merge_inner(state: &AppState, repo_id: &str) -> Result<(), AppError> {
    let path = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || merge::abort_merge(&path))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// All current index conflicts, path-ascending (P3c contract §3).
/// Errors: `noRepo` | `git`.
#[tauri::command]
pub async fn list_conflicts(
    state: tauri::State<'_, AppState>,
    repo_id: String,
) -> Result<Vec<ConflictEntry>, AppError> {
    list_conflicts_inner(state.inner(), &repo_id).await
}

/// Runtime-free core of `list_conflicts` (unit-testable without a Tauri app).
async fn list_conflicts_inner(
    state: &AppState,
    repo_id: &str,
) -> Result<Vec<ConflictEntry>, AppError> {
    let path = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || conflict::list_conflicts(&path))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Read-only marker view of one conflicted file (P3c contract §3).
/// Errors: `noRepo` | `git`.
#[tauri::command]
pub async fn get_conflict(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    path: String,
) -> Result<ConflictFile, AppError> {
    get_conflict_inner(state.inner(), &repo_id, path).await
}

/// Runtime-free core of `get_conflict` (unit-testable without a Tauri app).
async fn get_conflict_inner(
    state: &AppState,
    repo_id: &str,
    path: String,
) -> Result<ConflictFile, AppError> {
    let workdir = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || conflict::get_conflict(&workdir, &path))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Resolves one conflicted path per the P3c contract §3.2 matrix.
/// Errors: `noRepo` | `git` | `invalidName` (validate_rel_path).
#[tauri::command]
pub async fn resolve_conflict(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    path: String,
    resolution: ConflictResolution,
) -> Result<(), AppError> {
    resolve_conflict_inner(state.inner(), &repo_id, path, resolution).await
}

/// Runtime-free core of `resolve_conflict` (unit-testable without a Tauri app).
async fn resolve_conflict_inner(
    state: &AppState,
    repo_id: &str,
    path: String,
    resolution: ConflictResolution,
) -> Result<(), AppError> {
    let workdir = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || {
        conflict::resolve_conflict(&workdir, &path, resolution)
    })
    .await
    .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Stages user-authored resolved text for one conflicted path (P12 §1.2).
/// Errors: `noRepo` | `git` | `invalidName`. Does NOT emit `repo-changed` —
/// the frontend refetches imperatively.
#[tauri::command]
pub async fn resolve_conflict_text(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    path: String,
    content: String,
) -> Result<(), AppError> {
    resolve_conflict_text_inner(state.inner(), &repo_id, path, content).await
}

/// Runtime-free core of `resolve_conflict_text` (unit-testable without a Tauri app).
async fn resolve_conflict_text_inner(
    state: &AppState,
    repo_id: &str,
    path: String,
    content: String,
) -> Result<(), AppError> {
    let workdir = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || {
        conflict::resolve_conflict_text(&workdir, &path, &content)
    })
    .await
    .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Cheap Claude Code CLI health probe (P13 §6). No repo, no state; NEVER
/// rejects for CLI state — a missing/broken CLI yields `{ installed:false, .. }`.
/// Only a task-join error can `Err`.
#[tauri::command]
pub async fn check_ai_availability() -> Result<AiAvailability, AppError> {
    tauri::async_runtime::spawn_blocking(ai::check_availability)
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))
}

/// Proposes an AI resolution for one conflicted path (P13 §6). Loads settings
/// and REFUSES with `AiUnavailable` unless `ai_enabled && ai_consented` (§9.6 —
/// the authoritative backend gate; the frontend also gates for UX). WRITES
/// NOTHING — applying is the separate `resolve_conflict_text` command. Errors:
/// `aiUnavailable` | `aiFailed` | `git` | `invalidName` | `noRepo`.
#[tauri::command]
pub async fn ai_resolve_conflict(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    repo_id: String,
    path: String,
) -> Result<AiResolveProposal, AppError> {
    // Resolve the settings-file path at the AppHandle boundary so the inner
    // stays runtime-free and unit-testable (mirrors `settings.rs`'s
    // path-parameterized design), then delegate.
    let file = settings::settings_file(&app)?;
    ai_resolve_conflict_inner(state.inner(), &file, &repo_id, path).await
}

/// Runtime-free core of `ai_resolve_conflict` (unit-testable without a Tauri
/// app). The consent gate is enforced HERE, BEFORE `repo_path`, per §9.6.
async fn ai_resolve_conflict_inner(
    state: &AppState,
    settings_file: &std::path::Path,
    repo_id: &str,
    path: String,
) -> Result<AiResolveProposal, AppError> {
    let s = settings::load_from(settings_file);
    if !(s.ai_enabled && s.ai_consented) {
        return Err(AppError::AiUnavailable(
            "AI features are disabled or not yet consented to".to_string(),
        ));
    }
    let workdir = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || {
        ai_resolve::ai_resolve_conflict(&workdir, &path, RunOpts::default())
    })
    .await
    .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Generates a commit message from the staged diff (P15a §5). Loads settings and
/// REFUSES with `AiUnavailable` unless `ai_enabled && ai_consented` (the
/// authoritative backend gate; the frontend also gates for UX). WRITES NOTHING —
/// the user edits the returned text in the commit box and commits separately.
/// Errors: `aiUnavailable` | `aiFailed` | `nothingToCommit` | `git` | `noRepo`.
#[tauri::command]
pub async fn generate_commit_message(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    repo_id: String,
) -> Result<CommitMessageProposal, AppError> {
    // Resolve the settings-file path at the AppHandle boundary so the inner stays
    // runtime-free and unit-testable (mirrors `ai_resolve_conflict`), then delegate.
    let file = settings::settings_file(&app)?;
    generate_commit_message_inner(state.inner(), &file, &repo_id).await
}

/// Runtime-free core of `generate_commit_message` (unit-testable without a Tauri
/// app). The consent gate is enforced HERE, BEFORE `repo_path`.
async fn generate_commit_message_inner(
    state: &AppState,
    settings_file: &std::path::Path,
    repo_id: &str,
) -> Result<CommitMessageProposal, AppError> {
    let s = settings::load_from(settings_file);
    if !(s.ai_enabled && s.ai_consented) {
        return Err(AppError::AiUnavailable(
            "AI features are disabled or not yet consented to".to_string(),
        ));
    }
    let workdir = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || {
        ai_commit::generate_commit_message(&workdir, RunOpts::default())
    })
    .await
    .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Explains or reviews a diff target (P15b §5). Loads settings and REFUSES with
/// `AiUnavailable` unless `ai_enabled && ai_consented` (the authoritative backend
/// gate; the frontend also gates for UX). Read-only prose out — WRITES NOTHING.
/// Errors: `aiUnavailable` | `aiFailed` | `nothingToCommit` | `git` | `invalidName` | `noRepo`.
#[tauri::command]
pub async fn ai_analyze_diff(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    repo_id: String,
    target: AiDiffTarget,
    mode: AiAnalysisMode,
) -> Result<AiAnalysis, AppError> {
    // Resolve the settings-file path at the AppHandle boundary so the inner stays
    // runtime-free and unit-testable (mirrors `generate_commit_message`), then delegate.
    let file = settings::settings_file(&app)?;
    ai_analyze_diff_inner(state.inner(), &file, &repo_id, target, mode).await
}

/// Runtime-free core of `ai_analyze_diff` (unit-testable without a Tauri app).
/// The consent gate is enforced HERE, BEFORE `repo_path`.
async fn ai_analyze_diff_inner(
    state: &AppState,
    settings_file: &std::path::Path,
    repo_id: &str,
    target: AiDiffTarget,
    mode: AiAnalysisMode,
) -> Result<AiAnalysis, AppError> {
    let s = settings::load_from(settings_file);
    if !(s.ai_enabled && s.ai_consented) {
        return Err(AppError::AiUnavailable(
            "AI features are disabled or not yet consented to".to_string(),
        ));
    }
    let workdir = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || {
        ai_explain::analyze_diff(&workdir, target, mode, RunOpts::default())
    })
    .await
    .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Summarizes the commits/diff unique to `target` vs `base` (P15c §5). Loads
/// settings and REFUSES with `AiUnavailable` unless `ai_enabled && ai_consented`
/// (the authoritative backend gate; the frontend also gates for UX). Read-only
/// prose out — WRITES NOTHING. Errors: `aiUnavailable` | `aiFailed` | `git` |
/// `noRepo`.
#[tauri::command]
pub async fn ai_summarize_range(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    repo_id: String,
    base: String,
    target: String,
) -> Result<AiSummary, AppError> {
    // Resolve the settings-file path at the AppHandle boundary so the inner stays
    // runtime-free and unit-testable (mirrors `ai_analyze_diff`), then delegate.
    let file = settings::settings_file(&app)?;
    ai_summarize_range_inner(state.inner(), &file, &repo_id, base, target).await
}

/// Runtime-free core of `ai_summarize_range` (unit-testable without a Tauri app).
/// The consent gate is enforced HERE, BEFORE `repo_path`.
async fn ai_summarize_range_inner(
    state: &AppState,
    settings_file: &std::path::Path,
    repo_id: &str,
    base: String,
    target: String,
) -> Result<AiSummary, AppError> {
    let s = settings::load_from(settings_file);
    if !(s.ai_enabled && s.ai_consented) {
        return Err(AppError::AiUnavailable(
            "AI features are disabled or not yet consented to".to_string(),
        ));
    }
    let workdir = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || {
        ai_summary::summarize_range(&workdir, &base, &target, RunOpts::default())
    })
    .await
    .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// AI "what changed" digest over a selectable range (P28 §5). Loads settings
/// and REFUSES with `AiUnavailable` unless `ai_enabled && ai_consented` (the
/// authoritative backend gate; the frontend also gates for UX). Read-only prose
/// out — WRITES NOTHING. Errors: `aiUnavailable` | `aiFailed` | `git` |
/// `invalidName` | `noRepo`.
#[tauri::command]
pub async fn ai_digest(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    repo_id: String,
    range: AiDigestRange,
) -> Result<AiAnalysis, AppError> {
    // Resolve the settings-file path at the AppHandle boundary so the inner stays
    // runtime-free and unit-testable (mirrors `ai_analyze_diff`), then delegate.
    let file = settings::settings_file(&app)?;
    ai_digest_inner(state.inner(), &file, &repo_id, range).await
}

/// Runtime-free core of `ai_digest` (unit-testable without a Tauri app).
/// The consent gate is enforced HERE, BEFORE `repo_path`.
async fn ai_digest_inner(
    state: &AppState,
    settings_file: &std::path::Path,
    repo_id: &str,
    range: AiDigestRange,
) -> Result<AiAnalysis, AppError> {
    let s = settings::load_from(settings_file);
    if !(s.ai_enabled && s.ai_consented) {
        return Err(AppError::AiUnavailable(
            "AI features are disabled or not yet consented to".to_string(),
        ));
    }
    let workdir = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || {
        ai_explain::digest_changes(&workdir, range, RunOpts::default())
    })
    .await
    .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Starts a rebase of the current branch onto `onto` (local or remote-tracking
/// shorthand; P3d contract §3). Errors: `operationInProgress` | `branchNotFound`
/// | `checkoutConflict` | `configMissing` | `git` | `noRepo`. Does NOT emit
/// `repo-changed` — the frontend refetches imperatively.
#[tauri::command]
pub async fn rebase_branch(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    onto: String,
) -> Result<RebaseOutcome, AppError> {
    rebase_branch_inner(state.inner(), &repo_id, onto).await
}

/// Runtime-free core of `rebase_branch` (unit-testable without a Tauri app).
async fn rebase_branch_inner(
    state: &AppState,
    repo_id: &str,
    onto: String,
) -> Result<RebaseOutcome, AppError> {
    let path = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || rebase::rebase_branch(&path, &onto))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Resumes a paused rebase — commits the resolved op, then replays on (P3d
/// contract §3.7). Errors: `noOperationInProgress` | `unresolvedConflicts`
/// | `configMissing` | `git` | `noRepo`.
#[tauri::command]
pub async fn rebase_continue(
    state: tauri::State<'_, AppState>,
    repo_id: String,
) -> Result<RebaseOutcome, AppError> {
    rebase_continue_inner(state.inner(), &repo_id).await
}

/// Runtime-free core of `rebase_continue` (unit-testable without a Tauri app).
async fn rebase_continue_inner(state: &AppState, repo_id: &str) -> Result<RebaseOutcome, AppError> {
    let path = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || rebase::rebase_continue(&path))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Skips the current operation and resumes (P3d contract §3.8). Errors:
/// `noOperationInProgress` | `configMissing` | `git` | `noRepo`.
#[tauri::command]
pub async fn rebase_skip(
    state: tauri::State<'_, AppState>,
    repo_id: String,
) -> Result<RebaseOutcome, AppError> {
    rebase_skip_inner(state.inner(), &repo_id).await
}

/// Runtime-free core of `rebase_skip` (unit-testable without a Tauri app).
async fn rebase_skip_inner(state: &AppState, repo_id: &str) -> Result<RebaseOutcome, AppError> {
    let path = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || rebase::rebase_skip(&path))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Aborts a paused rebase (worktree-destructive — the UI confirms first; P3d
/// contract §3.10). Errors: `noOperationInProgress` | `git` | `noRepo`.
#[tauri::command]
pub async fn rebase_abort(
    state: tauri::State<'_, AppState>,
    repo_id: String,
) -> Result<(), AppError> {
    rebase_abort_inner(state.inner(), &repo_id).await
}

/// Runtime-free core of `rebase_abort` (unit-testable without a Tauri app).
async fn rebase_abort_inner(state: &AppState, repo_id: &str) -> Result<(), AppError> {
    let path = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || rebase::rebase_abort(&path))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Returns the DEFAULT interactive-rebase plan (every commit `pick`, oldest-
/// first) for `base..HEAD`, seeding the plan editor (P23 contract §7). Errors:
/// `git` | `noRepo`. Does NOT emit `repo-changed`.
#[tauri::command]
pub async fn get_interactive_plan(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    base_oid: String,
) -> Result<Vec<RebaseTodoOp>, AppError> {
    get_interactive_plan_inner(state.inner(), &repo_id, base_oid).await
}

/// Runtime-free core of `get_interactive_plan` (unit-testable without a Tauri app).
async fn get_interactive_plan_inner(
    state: &AppState,
    repo_id: &str,
    base_oid: String,
) -> Result<Vec<RebaseTodoOp>, AppError> {
    let path = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || {
        rebase_interactive::get_interactive_plan(&path, &base_oid)
    })
    .await
    .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Starts an interactive rebase of the current branch onto `onto_oid`, replaying
/// `todos` in order (P23 contract §7). Continue/Skip/Abort reuse the existing
/// `rebase_{continue,skip,abort}` commands via the core delegation. Errors:
/// `operationInProgress` | `checkoutConflict` | `configMissing` | `git` |
/// `noRepo`. Does NOT emit `repo-changed` — the frontend refetches imperatively.
#[tauri::command]
pub async fn start_interactive_rebase(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    onto_oid: String,
    todos: Vec<RebaseTodoOp>,
) -> Result<RebaseOutcome, AppError> {
    start_interactive_rebase_inner(state.inner(), &repo_id, onto_oid, todos).await
}

/// Runtime-free core of `start_interactive_rebase` (unit-testable without a Tauri app).
async fn start_interactive_rebase_inner(
    state: &AppState,
    repo_id: &str,
    onto_oid: String,
    todos: Vec<RebaseTodoOp>,
) -> Result<RebaseOutcome, AppError> {
    let path = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || {
        rebase_interactive::start_interactive_rebase(&path, &onto_oid, todos)
    })
    .await
    .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Per-line blame of `path` as of `at_oid` (`null`/omitted -> HEAD, P23
/// contract §9.1/§10). Errors: `other` (bad path) | `git` | `noRepo`. Does NOT
/// emit `repo-changed`.
#[tauri::command]
pub async fn blame_file(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    path: String,
    at_oid: Option<String>,
) -> Result<Vec<BlameLine>, AppError> {
    blame_file_inner(state.inner(), &repo_id, path, at_oid).await
}

/// Runtime-free core of `blame_file` (unit-testable without a Tauri app).
async fn blame_file_inner(
    state: &AppState,
    repo_id: &str,
    path: String,
    at_oid: Option<String>,
) -> Result<Vec<BlameLine>, AppError> {
    let workdir = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || blame::blame_file(&workdir, &path, at_oid.as_deref()))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Commits that modified `path`, newest-first, best-effort following a single
/// rename (P23 contract §9.2/§10). `limit == 0` -> the built-in `MAX_HISTORY`
/// cap. Errors: `other` (bad path) | `git` | `noRepo`. Does NOT emit
/// `repo-changed`.
#[tauri::command]
pub async fn file_history(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    path: String,
    limit: u32,
) -> Result<Vec<FileHistoryEntry>, AppError> {
    file_history_inner(state.inner(), &repo_id, path, limit).await
}

/// Runtime-free core of `file_history` (unit-testable without a Tauri app).
async fn file_history_inner(
    state: &AppState,
    repo_id: &str,
    path: String,
    limit: u32,
) -> Result<Vec<FileHistoryEntry>, AppError> {
    let workdir = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || blame::file_history(&workdir, &path, limit))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Enumerates the stash stack, index 0 (most recent) first (P9 contract §3).
/// Errors: `git` | `noRepo`. Does NOT emit `repo-changed`.
#[tauri::command]
pub async fn list_stashes(
    state: tauri::State<'_, AppState>,
    repo_id: String,
) -> Result<Vec<StashEntry>, AppError> {
    list_stashes_inner(state.inner(), &repo_id).await
}

/// Runtime-free core of `list_stashes` (unit-testable without a Tauri app).
async fn list_stashes_inner(
    state: &AppState,
    repo_id: &str,
) -> Result<Vec<StashEntry>, AppError> {
    let path = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || stash::list_stashes(&path))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Stashes the dirty worktree (P9 contract §3). `message: None` → git default.
/// `created:false` == nothing to stash (NOT an error). Errors:
/// `operationInProgress` | `configMissing` | `git` | `noRepo`. Does NOT emit
/// `repo-changed`.
#[tauri::command]
pub async fn create_stash(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    message: Option<String>,
    scope: StashScope,
) -> Result<CreateStashResult, AppError> {
    create_stash_inner(state.inner(), &repo_id, message, scope).await
}

/// Runtime-free core of `create_stash` (unit-testable without a Tauri app).
async fn create_stash_inner(
    state: &AppState,
    repo_id: &str,
    message: Option<String>,
    scope: StashScope,
) -> Result<CreateStashResult, AppError> {
    let path = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || {
        stash::create_stash(&path, message.as_deref(), scope)
    })
    .await
    .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Applies stash `index` WITHOUT dropping it (P9 contract §3). Conflicts →
/// `Conflicts{paths}` (stash retained). Errors: `operationInProgress` | `git`
/// | `noRepo`. Does NOT emit `repo-changed`.
#[tauri::command]
pub async fn apply_stash(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    index: usize,
    skip_reserved: bool,
) -> Result<ApplyStashOutcome, AppError> {
    apply_stash_inner(state.inner(), &repo_id, index, skip_reserved).await
}

/// Runtime-free core of `apply_stash` (unit-testable without a Tauri app).
async fn apply_stash_inner(
    state: &AppState,
    repo_id: &str,
    index: usize,
    skip_reserved: bool,
) -> Result<ApplyStashOutcome, AppError> {
    let path = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || stash::apply_stash(&path, index, skip_reserved))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Applies stash `index` and drops it on clean success only (P9 contract §3).
/// Conflicts → `Conflicts{paths}` and the entry is RETAINED. Errors:
/// `operationInProgress` | `git` | `noRepo`. Does NOT emit `repo-changed`.
#[tauri::command]
pub async fn pop_stash(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    index: usize,
    skip_reserved: bool,
) -> Result<ApplyStashOutcome, AppError> {
    pop_stash_inner(state.inner(), &repo_id, index, skip_reserved).await
}

/// Runtime-free core of `pop_stash` (unit-testable without a Tauri app).
async fn pop_stash_inner(
    state: &AppState,
    repo_id: &str,
    index: usize,
    skip_reserved: bool,
) -> Result<ApplyStashOutcome, AppError> {
    let path = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || stash::pop_stash(&path, index, skip_reserved))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Permanently discards stash `index` (P9 contract §3). Allowed in any repo
/// state (the UI confirms first — destructive). Errors: `git` | `noRepo`. Does
/// NOT emit `repo-changed`.
#[tauri::command]
pub async fn drop_stash(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    index: usize,
) -> Result<(), AppError> {
    drop_stash_inner(state.inner(), &repo_id, index).await
}

/// Runtime-free core of `drop_stash` (unit-testable without a Tauri app).
async fn drop_stash_inner(
    state: &AppState,
    repo_id: &str,
    index: usize,
) -> Result<(), AppError> {
    let path = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || stash::drop_stash(&path, index))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Amends the current HEAD commit with a new message + the current index
/// (P20 contract §2). Preserves HEAD's parents + original author. Errors:
/// `operationInProgress` | `git` | `emptyMessage` | `configMissing` | `noRepo`.
/// Does NOT emit `repo-changed` — the frontend refetches.
#[tauri::command]
pub async fn commit_amend(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    message: String,
) -> Result<CommitResult, AppError> {
    commit_amend_inner(state.inner(), &repo_id, message).await
}

/// Runtime-free core of `commit_amend` (unit-testable without a Tauri app).
async fn commit_amend_inner(
    state: &AppState,
    repo_id: &str,
    message: String,
) -> Result<CommitResult, AppError> {
    let path = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || amend_commit(&path, &message))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Moves the current branch (HEAD) to `oid` in the given `mode` (P20 contract
/// §3). Hard is destructive — the UI confirms first. Errors:
/// `operationInProgress` | `git` | `noRepo`. Does NOT emit `repo-changed`.
#[tauri::command]
pub async fn reset_branch(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    oid: String,
    mode: ResetMode,
) -> Result<(), AppError> {
    reset_branch_command_inner(state.inner(), &repo_id, oid, mode).await
}

/// Runtime-free core of `reset_branch` (unit-testable without a Tauri app).
async fn reset_branch_command_inner(
    state: &AppState,
    repo_id: &str,
    oid: String,
    mode: ResetMode,
) -> Result<(), AppError> {
    let path = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || reset_branch_core(&path, &oid, mode))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Restores each tracked path's worktree content to the index version,
/// discarding unstaged edits (P20 contract §4). Destructive — the UI confirms
/// first. Errors: `other` (invalid path) | `git` | `noRepo`. Does NOT emit
/// `repo-changed`.
#[tauri::command]
pub async fn discard_paths(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    paths: Vec<String>,
) -> Result<(), AppError> {
    discard_paths_inner(state.inner(), &repo_id, paths).await
}

/// Runtime-free core of `discard_paths` (unit-testable without a Tauri app).
async fn discard_paths_inner(
    state: &AppState,
    repo_id: &str,
    paths: Vec<String>,
) -> Result<(), AppError> {
    let path = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || discard_paths_core(&path, &paths))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Force-discards a mixed set: tracked paths restored to index, untracked paths
/// deleted from disk. Destructive — the UI confirms first. Errors: `other`
/// (invalid path) | `io` | `git` | `noRepo`. Does NOT emit `repo-changed`.
#[tauri::command]
pub async fn discard_paths_force(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    paths: Vec<String>,
) -> Result<(), AppError> {
    discard_paths_force_inner(state.inner(), &repo_id, paths).await
}

/// Runtime-free core of `discard_paths_force` (unit-testable without a Tauri app).
async fn discard_paths_force_inner(
    state: &AppState,
    repo_id: &str,
    paths: Vec<String>,
) -> Result<(), AppError> {
    let path = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || discard_paths_force_core(&path, &paths))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Cherry-picks a single commit onto the current branch (P20 contract §5).
/// Clean → auto-commits; conflict → pauses into RepoOpState::CherryPick.
/// Errors: `operationInProgress` | `git` | `checkoutConflict` | `configMissing`
/// | `nothingToCommit` | `noRepo`. Does NOT emit `repo-changed`.
#[tauri::command]
pub async fn cherrypick_commit(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    oid: String,
) -> Result<CherrypickOutcome, AppError> {
    cherrypick_commit_inner(state.inner(), &repo_id, oid).await
}

/// Runtime-free core of `cherrypick_commit` (unit-testable without a Tauri app).
async fn cherrypick_commit_inner(
    state: &AppState,
    repo_id: &str,
    oid: String,
) -> Result<CherrypickOutcome, AppError> {
    let path = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || cherrypick::cherrypick_commit(&path, &oid))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Finalizes a paused (resolved) cherry-pick (P20 contract §5). Errors:
/// `noOperationInProgress` | `unresolvedConflicts` | `configMissing`
/// | `nothingToCommit` | `git` | `noRepo`. Does NOT emit `repo-changed`.
#[tauri::command]
pub async fn cherrypick_continue(
    state: tauri::State<'_, AppState>,
    repo_id: String,
) -> Result<CherrypickOutcome, AppError> {
    cherrypick_continue_inner(state.inner(), &repo_id).await
}

/// Runtime-free core of `cherrypick_continue`.
async fn cherrypick_continue_inner(
    state: &AppState,
    repo_id: &str,
) -> Result<CherrypickOutcome, AppError> {
    let path = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || cherrypick::cherrypick_continue(&path))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Aborts a paused cherry-pick (reset --hard to HEAD; destructive — the UI
/// confirms first; P20 contract §5). Errors: `noOperationInProgress` | `git`
/// | `noRepo`. Does NOT emit `repo-changed`.
#[tauri::command]
pub async fn cherrypick_abort(
    state: tauri::State<'_, AppState>,
    repo_id: String,
) -> Result<(), AppError> {
    cherrypick_abort_inner(state.inner(), &repo_id).await
}

/// Runtime-free core of `cherrypick_abort`.
async fn cherrypick_abort_inner(state: &AppState, repo_id: &str) -> Result<(), AppError> {
    let path = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || cherrypick::cherrypick_abort(&path))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Reverts a single commit on the current branch (P20 contract §6). Clean →
/// auto-commits; conflict → pauses into RepoOpState::Revert. Errors:
/// `operationInProgress` | `git` | `checkoutConflict` | `configMissing`
/// | `nothingToCommit` | `noRepo`. Does NOT emit `repo-changed`.
#[tauri::command]
pub async fn revert_commit(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    oid: String,
) -> Result<RevertOutcome, AppError> {
    revert_commit_inner(state.inner(), &repo_id, oid).await
}

/// Runtime-free core of `revert_commit` (unit-testable without a Tauri app).
async fn revert_commit_inner(
    state: &AppState,
    repo_id: &str,
    oid: String,
) -> Result<RevertOutcome, AppError> {
    let path = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || revert::revert_commit(&path, &oid))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Finalizes a paused (resolved) revert (P20 contract §6). Errors:
/// `noOperationInProgress` | `unresolvedConflicts` | `configMissing`
/// | `nothingToCommit` | `git` | `noRepo`. Does NOT emit `repo-changed`.
#[tauri::command]
pub async fn revert_continue(
    state: tauri::State<'_, AppState>,
    repo_id: String,
) -> Result<RevertOutcome, AppError> {
    revert_continue_inner(state.inner(), &repo_id).await
}

/// Runtime-free core of `revert_continue`.
async fn revert_continue_inner(
    state: &AppState,
    repo_id: &str,
) -> Result<RevertOutcome, AppError> {
    let path = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || revert::revert_continue(&path))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Aborts a paused revert (reset --hard to HEAD; destructive — the UI confirms
/// first; P20 contract §6). Errors: `noOperationInProgress` | `git` | `noRepo`.
/// Does NOT emit `repo-changed`.
#[tauri::command]
pub async fn revert_abort(
    state: tauri::State<'_, AppState>,
    repo_id: String,
) -> Result<(), AppError> {
    revert_abort_inner(state.inner(), &repo_id).await
}

/// Runtime-free core of `revert_abort`.
async fn revert_abort_inner(state: &AppState, repo_id: &str) -> Result<(), AppError> {
    let path = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || revert::revert_abort(&path))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Lists every submodule with its classified status (P19 contract §3). Errors:
/// `git` | `noRepo`. Does NOT emit `repo-changed` — the frontend refetches.
#[tauri::command]
pub async fn list_submodules(
    state: tauri::State<'_, AppState>,
    repo_id: String,
) -> Result<Vec<SubmoduleInfo>, AppError> {
    list_submodules_inner(state.inner(), &repo_id).await
}

/// Runtime-free core of `list_submodules` (unit-testable without a Tauri app).
async fn list_submodules_inner(
    state: &AppState,
    repo_id: &str,
) -> Result<Vec<SubmoduleInfo>, AppError> {
    let path = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || submodule::list_submodules(&path))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Registers submodule `name` in .git/config — no worktree change (P19 contract
/// §3). Errors: `invalidName` | `git` | `noRepo`. Does NOT emit `repo-changed`.
#[tauri::command]
pub async fn init_submodule(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    name: String,
) -> Result<(), AppError> {
    init_submodule_inner(state.inner(), &repo_id, name).await
}

/// Runtime-free core of `init_submodule` (unit-testable without a Tauri app).
async fn init_submodule_inner(
    state: &AppState,
    repo_id: &str,
    name: String,
) -> Result<(), AppError> {
    let path = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || submodule::init_submodule(&path, &name))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Init-if-needed + fetch + checkout the pinned commit for submodule `name`
/// (P19 contract §3). Reuses the M6 credential chain. Errors: `invalidName` |
/// `authFailed` | `networkError` | `git` | `noRepo`. Does NOT emit `repo-changed`.
#[tauri::command]
pub async fn update_submodule(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    name: String,
) -> Result<(), AppError> {
    update_submodule_inner(state.inner(), &repo_id, name).await
}

/// Runtime-free core of `update_submodule` (unit-testable without a Tauri app).
async fn update_submodule_inner(
    state: &AppState,
    repo_id: &str,
    name: String,
) -> Result<(), AppError> {
    let path = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || submodule::update_submodule(&path, &name))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Propagates the .gitmodules URL into config + the submodule remote for
/// submodule `name` (P19 contract §3). No worktree change. Errors:
/// `invalidName` | `git` | `noRepo`. Does NOT emit `repo-changed`.
#[tauri::command]
pub async fn sync_submodule(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    name: String,
) -> Result<(), AppError> {
    sync_submodule_inner(state.inner(), &repo_id, name).await
}

/// Runtime-free core of `sync_submodule` (unit-testable without a Tauri app).
async fn sync_submodule_inner(
    state: &AppState,
    repo_id: &str,
    name: String,
) -> Result<(), AppError> {
    let path = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || submodule::sync_submodule(&path, &name))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Lists every worktree — the synthesized main row first, then each linked
/// worktree — with resolved branch/oid/badges (P27 contract §3). Errors:
/// `git` | `noRepo`. Does NOT emit `repo-changed` — the frontend refetches.
#[tauri::command]
pub async fn list_worktrees(
    state: tauri::State<'_, AppState>,
    repo_id: String,
) -> Result<Vec<WorktreeInfo>, AppError> {
    list_worktrees_inner(state.inner(), &repo_id).await
}

/// Runtime-free core of `list_worktrees` (unit-testable without a Tauri app).
async fn list_worktrees_inner(
    state: &AppState,
    repo_id: &str,
) -> Result<Vec<WorktreeInfo>, AppError> {
    let path = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || worktree::list_worktrees(&path))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Creates a linked worktree checking out the EXISTING local branch `branch` at
/// a derived `<parent>/.worktrees/<repo-name>/<name-slug>` path; the on-disk
/// `name` is user-editable and decoupled from `branch` (P32 Part A — a blank
/// `name` defaults to `branch`). Returns the created row (P27 contract §3).
/// Errors: `noRepo` | `invalidName` | `branchNotFound` | `git` | `io`. Does NOT
/// emit `repo-changed` — the frontend refetches.
#[tauri::command]
pub async fn add_worktree(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    branch: String,
    name: String,
) -> Result<WorktreeInfo, AppError> {
    add_worktree_inner(state.inner(), &repo_id, branch, name).await
}

/// Runtime-free core of `add_worktree` (unit-testable without a Tauri app).
async fn add_worktree_inner(
    state: &AppState,
    repo_id: &str,
    branch: String,
    name: String,
) -> Result<WorktreeInfo, AppError> {
    let path = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || worktree::add_worktree(&path, &branch, &name))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Removes linked worktree `name` — refuses main/current/locked/dirty, then
/// prunes admin files + working directory (P27 contract §3). Errors: `noRepo`
/// | `invalidName` | `git` | `io`. Does NOT emit `repo-changed`.
#[tauri::command]
pub async fn remove_worktree(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    name: String,
) -> Result<(), AppError> {
    remove_worktree_inner(state.inner(), &repo_id, name).await
}

/// Runtime-free core of `remove_worktree` (unit-testable without a Tauri app).
async fn remove_worktree_inner(
    state: &AppState,
    repo_id: &str,
    name: String,
) -> Result<(), AppError> {
    let path = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || worktree::remove_worktree(&path, &name))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Locks linked worktree `name` with an optional reason (P27 contract §3).
/// Errors: `noRepo` | `invalidName` | `git`. Does NOT emit `repo-changed`.
#[tauri::command]
pub async fn lock_worktree(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    name: String,
    reason: Option<String>,
) -> Result<(), AppError> {
    lock_worktree_inner(state.inner(), &repo_id, name, reason).await
}

/// Runtime-free core of `lock_worktree` (unit-testable without a Tauri app).
async fn lock_worktree_inner(
    state: &AppState,
    repo_id: &str,
    name: String,
    reason: Option<String>,
) -> Result<(), AppError> {
    let path = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || {
        worktree::lock_worktree(&path, &name, reason.as_deref())
    })
    .await
    .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Unlocks linked worktree `name` (P27 contract §3). Errors: `noRepo` |
/// `invalidName` | `git`. Does NOT emit `repo-changed`.
#[tauri::command]
pub async fn unlock_worktree(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    name: String,
) -> Result<(), AppError> {
    unlock_worktree_inner(state.inner(), &repo_id, name).await
}

/// Runtime-free core of `unlock_worktree` (unit-testable without a Tauri app).
async fn unlock_worktree_inner(
    state: &AppState,
    repo_id: &str,
    name: String,
) -> Result<(), AppError> {
    let path = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || worktree::unlock_worktree(&path, &name))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Lists uncommitted + gitignored files eligible to copy into a new worktree
/// (P32 Part B). Groups: staged / unstaged / untracked / ignored; deletions
/// excluded. Errors: `noRepo` | `git`. Does NOT emit `repo-changed`.
#[tauri::command]
pub async fn list_copy_candidates(
    state: tauri::State<'_, AppState>,
    repo_id: String,
) -> Result<Vec<CopyCandidate>, AppError> {
    list_copy_candidates_inner(state.inner(), &repo_id).await
}

/// Runs the blocking core of `list_copy_candidates` under the async runtime
/// (testable directly under a tokio runtime, no Tauri app required).
async fn list_copy_candidates_inner(
    state: &AppState,
    repo_id: &str,
) -> Result<Vec<CopyCandidate>, AppError> {
    let path = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || worktree_copy::list_copy_candidates(&path))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Classifies `paths` against target `branch` (clean/conflict) BEFORE creating
/// the worktree (P32 Part B). Errors: `noRepo` | `branchNotFound` | `git`. Does
/// NOT emit `repo-changed`.
#[tauri::command]
pub async fn preview_worktree_copy(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    branch: String,
    paths: Vec<String>,
) -> Result<Vec<CopyPlanEntry>, AppError> {
    preview_worktree_copy_inner(state.inner(), &repo_id, branch, paths).await
}

/// Runs the blocking core of `preview_worktree_copy` under the async runtime
/// (testable directly under a tokio runtime, no Tauri app required).
async fn preview_worktree_copy_inner(
    state: &AppState,
    repo_id: &str,
    branch: String,
    paths: Vec<String>,
) -> Result<Vec<CopyPlanEntry>, AppError> {
    let path = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || {
        worktree_copy::classify_copy(&path, &branch, &paths)
    })
    .await
    .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Creates the worktree (Part A branch/name), then copies each `copy` selection's
/// source bytes into it; `skip` selections are not written; empty behaves like a
/// plain `add_worktree` (P32 Part B). Errors: `noRepo` | `invalidName` |
/// `branchNotFound` | `git` | `io`. Does NOT emit `repo-changed`.
#[tauri::command]
pub async fn add_worktree_with_changes(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    branch: String,
    name: String,
    selections: Vec<CopySelection>,
) -> Result<WorktreeInfo, AppError> {
    add_worktree_with_changes_inner(state.inner(), &repo_id, branch, name, selections).await
}

/// Runs the blocking core of `add_worktree_with_changes` under the async runtime
/// (testable directly under a tokio runtime, no Tauri app required).
async fn add_worktree_with_changes_inner(
    state: &AppState,
    repo_id: &str,
    branch: String,
    name: String,
    selections: Vec<CopySelection>,
) -> Result<WorktreeInfo, AppError> {
    let path = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || {
        worktree_copy::add_worktree_with_changes(&path, &branch, &name, &selections)
    })
    .await
    .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Collects all four repo-health sections in ONE round-trip (P29 contract
/// §D2/§D4). Per-section failures are folded into `Section.error` inside the
/// payload; the command itself errors only for `noRepo` (unknown id) or a
/// join failure. READ-ONLY — never emits `repo-changed`.
#[tauri::command]
pub async fn get_repo_health(
    state: tauri::State<'_, AppState>,
    repo_id: String,
) -> Result<RepoHealth, AppError> {
    get_repo_health_inner(state.inner(), &repo_id).await
}

/// Runtime-free core of `get_repo_health` (unit-testable without a Tauri app).
async fn get_repo_health_inner(state: &AppState, repo_id: &str) -> Result<RepoHealth, AppError> {
    let path = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || collect_repo_health(&path))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))
}

/// Clones `url` into `dest`, streaming `CloneProgress` over `on_progress`.
/// Returns the absolute workdir path of the clone (frontend then calls
/// `open_repo`/openTab). NOT repo-scoped — it CREATES a repo (P21 §OPEN-2).
/// Rejects io | authFailed | networkError | git.
#[tauri::command]
pub async fn clone_repo(
    url: String,
    dest: String,
    on_progress: tauri::ipc::Channel<CloneProgress>,
) -> Result<String, AppError> {
    tauri::async_runtime::spawn_blocking(move || {
        clone_repo_core(&url, std::path::Path::new(&dest), move |p| {
            // Channel is Clone+Send+Sync+'static; a send failure means the
            // frontend dropped the channel — ignore it, the clone completes.
            let _ = on_progress.send(p);
        })
    })
    .await
    .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Initializes (or opens, if already a repo) a repository at `path`. Returns
/// the absolute workdir path. NOT repo-scoped (P21 §OPEN-2/§OPEN-3).
/// Rejects io | git.
#[tauri::command]
pub async fn init_repo(path: String) -> Result<String, AppError> {
    tauri::async_runtime::spawn_blocking(move || init_repo_core(std::path::Path::new(&path)))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Creates a tag at `target_oid` (P22 contract §2.2). `message: Some(_)` →
/// annotated (needs a git identity); `message: None` → lightweight. `force`
/// overwrites an existing tag (the v1 UI passes `false`). Errors:
/// `noRepo` | `invalidName` | `configMissing` | `git`. Does NOT emit
/// `repo-changed` — the frontend refetches.
#[tauri::command]
pub async fn create_tag(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    name: String,
    target_oid: String,
    message: Option<String>,
    force: bool,
) -> Result<(), AppError> {
    create_tag_inner(state.inner(), &repo_id, name, target_oid, message, force).await
}

/// Runtime-free core of `create_tag` (unit-testable without a Tauri app).
async fn create_tag_inner(
    state: &AppState,
    repo_id: &str,
    name: String,
    target_oid: String,
    message: Option<String>,
    force: bool,
) -> Result<(), AppError> {
    let path = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || {
        tags::create_tag(&path, &name, &target_oid, message, force)
    })
    .await
    .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Deletes a LOCAL tag (P22 contract §2.3). Does NOT contact any remote.
/// Errors: `noRepo` | `invalidName` | `git`. Does NOT emit `repo-changed`.
#[tauri::command]
pub async fn delete_tag(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    name: String,
) -> Result<(), AppError> {
    delete_tag_inner(state.inner(), &repo_id, name).await
}

/// Runtime-free core of `delete_tag` (unit-testable without a Tauri app).
async fn delete_tag_inner(
    state: &AppState,
    repo_id: &str,
    name: String,
) -> Result<(), AppError> {
    let path = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || tags::delete_tag(&path, &name))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Pushes `refs/tags/<tag_name>` to `remote` over the M6 credential path
/// (P22 contract §2.4). `force` is `false` in the v1 UI. Errors:
/// `noRepo` | `noRemote` | `authFailed` | `networkError` | `pushRejected` |
/// `git`. Does NOT emit `repo-changed`.
#[tauri::command]
pub async fn push_tag(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    remote: String,
    tag_name: String,
    force: bool,
) -> Result<(), AppError> {
    push_tag_inner(state.inner(), &repo_id, remote, tag_name, force).await
}

/// Runtime-free core of `push_tag` (unit-testable without a Tauri app).
async fn push_tag_inner(
    state: &AppState,
    repo_id: &str,
    remote: String,
    tag_name: String,
    force: bool,
) -> Result<(), AppError> {
    let path = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || tags::push_tag(&path, &remote, &tag_name, force))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

// ============================================================ P22 §3 remotes
// management (list / add / remove / rename / set-url). Local-only config ops —
// none emit `repo-changed`; the frontend refetches imperatively.

/// Lists configured remotes (name + fetch URL, P22 contract §3.2). Errors:
/// `noRepo` | `git`.
#[tauri::command]
pub async fn list_remotes(
    state: tauri::State<'_, AppState>,
    repo_id: String,
) -> Result<Vec<RemoteInfo>, AppError> {
    list_remotes_inner(state.inner(), &repo_id).await
}

/// Runtime-free core of `list_remotes` (unit-testable without a Tauri app).
async fn list_remotes_inner(state: &AppState, repo_id: &str) -> Result<Vec<RemoteInfo>, AppError> {
    let path = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || list_remotes_core(&path))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Adds a remote (P22 contract §3.2). Errors: `noRepo` | `invalidName` | `git`.
/// Does NOT emit `repo-changed`.
#[tauri::command]
pub async fn add_remote(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    name: String,
    url: String,
) -> Result<(), AppError> {
    add_remote_inner(state.inner(), &repo_id, name, url).await
}

/// Runtime-free core of `add_remote` (unit-testable without a Tauri app).
async fn add_remote_inner(
    state: &AppState,
    repo_id: &str,
    name: String,
    url: String,
) -> Result<(), AppError> {
    let path = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || add_remote_core(&path, &name, &url))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Removes a remote and its remote-tracking refs (P22 contract §3.2). Errors:
/// `noRepo` | `noRemote` | `git`. Does NOT emit `repo-changed`.
#[tauri::command]
pub async fn remove_remote(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    name: String,
) -> Result<(), AppError> {
    remove_remote_inner(state.inner(), &repo_id, name).await
}

/// Runtime-free core of `remove_remote` (unit-testable without a Tauri app).
async fn remove_remote_inner(
    state: &AppState,
    repo_id: &str,
    name: String,
) -> Result<(), AppError> {
    let path = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || remove_remote_core(&path, &name))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Renames a remote (P22 contract §3.2). Errors:
/// `noRepo` | `noRemote` | `invalidName` | `git`. Does NOT emit `repo-changed`.
#[tauri::command]
pub async fn rename_remote(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    name: String,
    new_name: String,
) -> Result<(), AppError> {
    rename_remote_inner(state.inner(), &repo_id, name, new_name).await
}

/// Runtime-free core of `rename_remote` (unit-testable without a Tauri app).
async fn rename_remote_inner(
    state: &AppState,
    repo_id: &str,
    name: String,
    new_name: String,
) -> Result<(), AppError> {
    let path = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || rename_remote_core(&path, &name, &new_name))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Sets a remote's fetch URL (P22 contract §3.2). Errors:
/// `noRepo` | `noRemote` | `git`. Does NOT emit `repo-changed`.
#[tauri::command]
pub async fn set_remote_url(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    name: String,
    url: String,
) -> Result<(), AppError> {
    set_remote_url_inner(state.inner(), &repo_id, name, url).await
}

/// Runtime-free core of `set_remote_url` (unit-testable without a Tauri app).
async fn set_remote_url_inner(
    state: &AppState,
    repo_id: &str,
    name: String,
    url: String,
) -> Result<(), AppError> {
    let path = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || set_remote_url_core(&path, &name, &url))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Full AI-asset inventory + drift for `repo_id` (P24 contract §6.1). Optional
/// `canonical` overrides the drift reference asset id. No events, no channels —
/// the frontend refetches imperatively (and on the existing `repo-changed`).
#[tauri::command]
pub async fn list_ai_assets(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    canonical: Option<String>,
) -> Result<AiAssetInventory, AppError> {
    list_ai_assets_inner(state.inner(), &repo_id, canonical).await
}

/// Runtime-free core of `list_ai_assets` (unit-testable without a Tauri app).
async fn list_ai_assets_inner(
    state: &AppState,
    repo_id: &str,
    canonical: Option<String>,
) -> Result<AiAssetInventory, AppError> {
    let workdir = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || {
        assets::scan_inventory(&workdir, canonical.as_deref())
    })
    .await
    .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Raw content of one AI-asset file under `repo_id` (P24 §3, read path). The
/// path is validated to stay inside the workdir; a missing file yields
/// `exists:false` (not an error).
#[tauri::command]
pub async fn read_ai_asset(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    path: String,
) -> Result<AssetContent, AppError> {
    read_ai_asset_inner(state.inner(), &repo_id, path).await
}

/// Runtime-free core of `read_ai_asset` (unit-testable without a Tauri app).
async fn read_ai_asset_inner(
    state: &AppState,
    repo_id: &str,
    path: String,
) -> Result<AssetContent, AppError> {
    let workdir = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || assets::read_asset(&workdir, &path))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Managed inventory of the three `.claude/` agent-asset kinds (skills /
/// subagents / slash commands) under `repo_id` (P26 §5, read path). Parses +
/// validates each; a missing `.claude/` yields an empty inventory. No events,
/// no channels — the frontend refetches imperatively (and on `repo-changed`).
#[tauri::command]
pub async fn list_agent_assets(
    state: tauri::State<'_, AppState>,
    repo_id: String,
) -> Result<AgentAssetInventory, AppError> {
    list_agent_assets_inner(state.inner(), &repo_id).await
}

/// Runtime-free core of `list_agent_assets` (unit-testable without a Tauri app).
async fn list_agent_assets_inner(
    state: &AppState,
    repo_id: &str,
) -> Result<AgentAssetInventory, AppError> {
    let workdir = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || assets::scan_agent_assets(&workdir))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// One parsed agent asset by `(kind, name)` under `repo_id` (P26 §5, read path).
/// The name is validated for filesystem safety; a missing file resolves to an
/// `exists:false` shell (not an error).
#[tauri::command]
pub async fn read_agent_asset(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    kind: AgentAssetKind,
    name: String,
) -> Result<AgentAsset, AppError> {
    read_agent_asset_inner(state.inner(), &repo_id, kind, name).await
}

/// Runtime-free core of `read_agent_asset` (unit-testable without a Tauri app).
async fn read_agent_asset_inner(
    state: &AppState,
    repo_id: &str,
    kind: AgentAssetKind,
    name: String,
) -> Result<AgentAsset, AppError> {
    let workdir = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || assets::read_agent_asset(&workdir, kind, &name))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Create or overwrite an agent asset under `repo_id` (P26 §5, write path).
/// Validates the name + computed path; atomic temp+rename with parent-dir
/// creation (incl. the skill's `<name>/` dir). Returns the refreshed inventory.
/// Missing required fields do NOT block the write — they surface as `valid:false`
/// in the returned inventory. No consent gate; no events/channels — the frontend
/// refetches (and the watcher fires `repo-changed` on the `.claude/` write).
#[tauri::command]
pub async fn save_agent_asset(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    asset: AgentAssetInput,
) -> Result<AgentAssetInventory, AppError> {
    save_agent_asset_inner(state.inner(), &repo_id, asset).await
}

/// Runtime-free core of `save_agent_asset`.
async fn save_agent_asset_inner(
    state: &AppState,
    repo_id: &str,
    asset: AgentAssetInput,
) -> Result<AgentAssetInventory, AppError> {
    let workdir = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || assets::save_agent_asset(&workdir, asset))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Delete one agent asset by `(kind, name)` under `repo_id` (P26 §5). A **skill**
/// removes the whole `.claude/skills/<name>/` directory recursively (the UI
/// confirm spells this out); an agent/command removes the single `.md`. A missing
/// target is a no-op. Returns the refreshed inventory. No events/channels.
#[tauri::command]
pub async fn delete_agent_asset(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    kind: AgentAssetKind,
    name: String,
) -> Result<AgentAssetInventory, AppError> {
    delete_agent_asset_inner(state.inner(), &repo_id, kind, name).await
}

/// Runtime-free core of `delete_agent_asset`.
async fn delete_agent_asset_inner(
    state: &AppState,
    repo_id: &str,
    kind: AgentAssetKind,
    name: String,
) -> Result<AgentAssetInventory, AppError> {
    let workdir = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || assets::delete_agent_asset(&workdir, kind, &name))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// The context-profile store for `repo_id` (P24 §6). Lazy default when absent.
#[tauri::command]
pub async fn list_profiles(
    state: tauri::State<'_, AppState>,
    repo_id: String,
) -> Result<ProfileStore, AppError> {
    list_profiles_inner(state.inner(), &repo_id).await
}

/// Runtime-free core of `list_profiles` (unit-testable without a Tauri app).
async fn list_profiles_inner(state: &AppState, repo_id: &str) -> Result<ProfileStore, AppError> {
    let workdir = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || assets::list_profiles(&workdir))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Insert-or-replace a profile keyed by name, then persist (P24 §5.2). Rejects
/// invalid names / non-single-file targets with `invalidName`.
#[tauri::command]
pub async fn save_profile(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    profile: ContextProfile,
) -> Result<ProfileStore, AppError> {
    save_profile_inner(state.inner(), &repo_id, profile).await
}

/// Runtime-free core of `save_profile`.
async fn save_profile_inner(
    state: &AppState,
    repo_id: &str,
    profile: ContextProfile,
) -> Result<ProfileStore, AppError> {
    let workdir = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || assets::save_profile(&workdir, profile))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Remove a profile (no-op if absent), clearing `activeProfile` if it matched
/// (P24 §5.2). Returns the updated store.
#[tauri::command]
pub async fn delete_profile(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    name: String,
) -> Result<ProfileStore, AppError> {
    delete_profile_inner(state.inner(), &repo_id, name).await
}

/// Runtime-free core of `delete_profile`.
async fn delete_profile_inner(
    state: &AppState,
    repo_id: &str,
    name: String,
) -> Result<ProfileStore, AppError> {
    let workdir = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || assets::delete_profile(&workdir, &name))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Per-target before/after preview for a profile's activation (P24 §5.2).
/// Writes nothing — the UI's diff-preview safety gate.
#[tauri::command]
pub async fn preview_profile(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    name: String,
) -> Result<Vec<ProfilePreviewEntry>, AppError> {
    preview_profile_inner(state.inner(), &repo_id, name).await
}

/// Runtime-free core of `preview_profile`.
async fn preview_profile_inner(
    state: &AppState,
    repo_id: &str,
    name: String,
) -> Result<Vec<ProfilePreviewEntry>, AppError> {
    let workdir = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || assets::preview_profile(&workdir, &name))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Activate a profile: write each target's content to its mapped file (atomic
/// temp+rename), set `activeProfile`, persist (P24 §5.2). The one write path;
/// UI-gated behind confirm + preview.
#[tauri::command]
pub async fn activate_profile(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    name: String,
) -> Result<ProfileActivation, AppError> {
    activate_profile_inner(state.inner(), &repo_id, name).await
}

/// Runtime-free core of `activate_profile`.
async fn activate_profile_inner(
    state: &AppState,
    repo_id: &str,
    name: String,
) -> Result<ProfileActivation, AppError> {
    let workdir = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || assets::activate_profile(&workdir, &name))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Per-worktree AI-context matrix: every worktree row joined with its active
/// profile + drift/missing counts (P31 §5). Read-only. Errors: `noRepo` |
/// `git` | `other` | `io`.
#[tauri::command]
pub async fn list_worktree_contexts(
    state: tauri::State<'_, AppState>,
    repo_id: String,
) -> Result<Vec<WorktreeContextStatus>, AppError> {
    list_worktree_contexts_inner(state.inner(), &repo_id).await
}

/// Runtime-free core of `list_worktree_contexts` (unit-testable without a Tauri app).
async fn list_worktree_contexts_inner(
    state: &AppState,
    repo_id: &str,
) -> Result<Vec<WorktreeContextStatus>, AppError> {
    let workdir = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || assets::list_worktree_contexts(&workdir))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Per-target before/after preview for activating profile `name` onto
/// WORKTREE `worktree_key` (P31 §5). Writes nothing — the UI's diff-preview
/// safety gate. Enforces D6 eligibility (locked/invalid/prunable → `git`).
/// Errors: `noRepo` | `git` | `other` | `io`.
#[tauri::command]
pub async fn preview_worktree_profile(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    worktree_key: String,
    name: String,
) -> Result<Vec<ProfilePreviewEntry>, AppError> {
    preview_worktree_profile_inner(state.inner(), &repo_id, worktree_key, name).await
}

/// Runtime-free core of `preview_worktree_profile`.
async fn preview_worktree_profile_inner(
    state: &AppState,
    repo_id: &str,
    worktree_key: String,
    name: String,
) -> Result<Vec<ProfilePreviewEntry>, AppError> {
    let workdir = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || {
        assets::preview_profile_for_worktree(&workdir, &worktree_key, &name)
    })
    .await
    .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Activate profile `name` onto WORKTREE `worktree_key` — THE one write path
/// (P31 §4), UI-gated behind confirm + preview like `activate_profile`. The
/// core enforces D6 eligibility and the D7 dirty-target guard (all targets
/// checked before any write). Errors: `noRepo` | `invalidName` | `git` |
/// `other` | `io`.
#[tauri::command]
pub async fn activate_worktree_profile(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    worktree_key: String,
    name: String,
) -> Result<ProfileActivation, AppError> {
    activate_worktree_profile_inner(state.inner(), &repo_id, worktree_key, name).await
}

/// Runtime-free core of `activate_worktree_profile`.
async fn activate_worktree_profile_inner(
    state: &AppState,
    repo_id: &str,
    worktree_key: String,
    name: String,
) -> Result<ProfileActivation, AppError> {
    let workdir = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || {
        assets::activate_profile_for_worktree(&workdir, &worktree_key, &name)
    })
    .await
    .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Optional AI helper (P24e §6.8): translate the `source_asset_id` instruction
/// file into `target_agent`'s flavor via the local `claude` CLI. Enforces the
/// consent gate FIRST (before `repo_path`), exactly like `generate_commit_message`.
/// WRITES NOTHING — returns proposed text the user reviews and saves into a
/// profile target. Errors: `aiUnavailable` | `aiFailed` | `other` | `io` | `noRepo`.
#[tauri::command]
pub async fn ai_generate_asset(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    repo_id: String,
    source_asset_id: String,
    target_agent: String,
    guidance: Option<String>,
) -> Result<AiGeneratedAsset, AppError> {
    // Resolve the settings-file path at the AppHandle boundary so the inner stays
    // runtime-free (mirrors `generate_commit_message`), then delegate.
    let file = settings::settings_file(&app)?;
    ai_generate_asset_inner(state.inner(), &file, &repo_id, source_asset_id, target_agent, guidance)
        .await
}

/// Runtime-free core of `ai_generate_asset`. The consent gate is enforced HERE,
/// BEFORE `repo_path` (§6.8).
async fn ai_generate_asset_inner(
    state: &AppState,
    settings_file: &std::path::Path,
    repo_id: &str,
    source_asset_id: String,
    target_agent: String,
    guidance: Option<String>,
) -> Result<AiGeneratedAsset, AppError> {
    let s = settings::load_from(settings_file);
    if !(s.ai_enabled && s.ai_consented) {
        return Err(AppError::AiUnavailable(
            "AI features are disabled — enable them in Settings".to_string(),
        ));
    }
    let workdir = repo_path(state, repo_id)?;
    // Resolve the source asset id to its mapped file, then read its content. A
    // missing/empty source is an error (nothing to translate) → `Other`.
    let descriptor = assets::descriptor(&source_asset_id)
        .ok_or_else(|| AppError::Other(format!("unknown asset id: '{source_asset_id}'")))?;
    let src_path = descriptor.path.to_string();
    let source_content = {
        let workdir = workdir.clone();
        tauri::async_runtime::spawn_blocking(move || assets::read_asset(&workdir, &src_path))
            .await
            .map_err(|e| AppError::Other(format!("task join error: {e}")))??
    };
    let content = match source_content.content {
        Some(c) if !c.trim().is_empty() => c,
        _ => {
            return Err(AppError::Other(format!(
                "source asset '{source_asset_id}' has no content to translate"
            )))
        }
    };
    tauri::async_runtime::spawn_blocking(move || {
        assets::generate_asset(&workdir, &content, &target_agent, guidance.as_deref(), RunOpts::default())
    })
    .await
    .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

#[cfg(test)]
mod tests {
    use super::*;

    const MISSING_ID: &str = "missing";

    fn path_string(p: &std::path::Path) -> String {
        p.to_string_lossy().into_owned()
    }

    /// Opens `path` runtime-free with a no-op watcher factory (P3e contract
    /// §9.1: `open_repo_inner(state, path, |_id| Box::new(|| {}))`).
    fn open(state: &AppState, path: &std::path::Path) -> Result<OpenRepoResult, AppError> {
        tauri::async_runtime::block_on(open_repo_inner(
            state,
            path_string(path),
            |_id| Box::new(|| {}),
        ))
    }

    /// git2-init a repo with a committable identity; returns the temp dir.
    fn init_repo_with_identity() -> tempfile::TempDir {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let repo = git2::Repository::init(dir.path()).expect("init repo");
        let mut cfg = repo.config().expect("open config");
        cfg.set_str("user.name", "Test User").expect("set user.name");
        cfg.set_str("user.email", "test@example.com")
            .expect("set user.email");
        dir
    }

    /// Writes `rel` under the workdir, stages it, and commits — via the command
    /// inners, so the whole round-trip is keyed by `repo_id`.
    fn write_stage_commit(
        state: &AppState,
        repo_id: &str,
        workdir: &std::path::Path,
        rel: &str,
        contents: &str,
        message: &str,
    ) -> CommitResult {
        std::fs::write(workdir.join(rel), contents).expect("write file");
        tauri::async_runtime::block_on(stage_inner(state, repo_id, vec![rel.to_string()]))
            .expect("stage");
        tauri::async_runtime::block_on(commit_inner(state, repo_id, message.to_string()))
            .expect("commit")
    }

    fn repo_count(state: &AppState) -> usize {
        state.repos.lock().expect("repos lock").len()
    }

    /// Opening a non-repo path inserts NO entry and touches no other open tab
    /// (P3e contract §4.2 — there is no single "current repo" to clear).
    #[test]
    fn failed_open_leaves_other_entries_untouched() {
        let state = AppState::default();

        // Open a real (empty, unborn-HEAD) repo first.
        let repo_dir = tempfile::TempDir::new().expect("create temp dir");
        git2::Repository::init(repo_dir.path()).expect("init repo");
        let a = open(&state, repo_dir.path()).expect("open repo A");
        assert!(a.info.is_repo && !a.info.bare);
        tauri::async_runtime::block_on(get_status_inner(&state, &a.repo_id))
            .expect("status of repo A");

        // Now open a plain directory: not a repo. No entry is created for it…
        let non_repo_dir = tempfile::TempDir::new().expect("create temp dir");
        let n = open(&state, non_repo_dir.path()).expect("open non-repo dir");
        assert!(!n.info.is_repo);
        let err = tauri::async_runtime::block_on(get_status_inner(&state, &n.repo_id))
            .expect_err("a non-repo id must not be open");
        assert!(matches!(err, AppError::NoRepo));

        // …and repo A is still open and usable.
        tauri::async_runtime::block_on(get_status_inner(&state, &a.repo_id))
            .expect("repo A still open after a failed open");
        assert_eq!(repo_count(&state), 1);
    }

    /// `get_repo_health` errors only for an unknown id (`NoRepo`, P29 §D4);
    /// on an open repo it resolves with all four sections carrying data —
    /// section-level failures never reject the command.
    #[test]
    fn get_repo_health_requires_open_repo() {
        let state = AppState::default();
        let err = tauri::async_runtime::block_on(get_repo_health_inner(&state, MISSING_ID))
            .expect_err("unknown id must be NoRepo");
        assert!(matches!(err, AppError::NoRepo));

        let dir = init_repo_with_identity();
        let opened = open(&state, dir.path()).expect("open repo");
        write_stage_commit(&state, &opened.repo_id, dir.path(), "a.txt", "a\n", "C0");
        let health =
            tauri::async_runtime::block_on(get_repo_health_inner(&state, &opened.repo_id))
                .expect("health never errors for an open repo");
        assert!(health.stats.data.is_some(), "{:?}", health.stats.error);
        assert!(health.branches.data.is_some(), "{:?}", health.branches.error);
        assert!(
            health.working_state.data.is_some(),
            "{:?}",
            health.working_state.error
        );
        assert!(health.structure.data.is_some(), "{:?}", health.structure.error);
        assert_eq!(
            health.stats.data.as_ref().map(|s| s.commit_count),
            Some(1)
        );
        assert!(health.generated_at > 0);
    }

    /// `get_graph` with an unknown id returns `NoRepo`; after opening an
    /// unborn-HEAD repo it returns an empty layout (not an error).
    #[test]
    fn get_graph_no_repo_and_unborn() {
        let state = AppState::default();

        let err = tauri::async_runtime::block_on(get_graph_inner(&state, MISSING_ID))
            .expect_err("no repo open must be NoRepo");
        assert!(matches!(err, AppError::NoRepo));

        let repo_dir = tempfile::TempDir::new().expect("create temp dir");
        git2::Repository::init(repo_dir.path()).expect("init repo");
        let id = open(&state, repo_dir.path()).expect("open unborn repo").repo_id;

        let layout = tauri::async_runtime::block_on(get_graph_inner(&state, &id))
            .expect("empty layout for unborn repo");
        assert!(layout.nodes.is_empty());
        assert_eq!(layout.head_index, None);
    }

    /// Bare repos are reported but not kept open; other entries are untouched.
    #[test]
    fn bare_open_leaves_other_entries_untouched() {
        let state = AppState::default();

        let repo_dir = tempfile::TempDir::new().expect("create temp dir");
        git2::Repository::init(repo_dir.path()).expect("init repo");
        let a = open(&state, repo_dir.path()).expect("open repo A");

        let bare_dir = tempfile::TempDir::new().expect("create temp dir");
        git2::Repository::init_bare(bare_dir.path()).expect("init bare repo");
        let b = open(&state, bare_dir.path()).expect("open bare repo");
        assert!(b.info.is_repo && b.info.bare);

        let err = tauri::async_runtime::block_on(get_status_inner(&state, &b.repo_id))
            .expect_err("bare repo must not be open");
        assert!(matches!(err, AppError::NoRepo));

        tauri::async_runtime::block_on(get_status_inner(&state, &a.repo_id))
            .expect("repo A still open after opening a bare repo");
        assert_eq!(repo_count(&state), 1);
    }

    /// The M3 mutation commands all return `NoRepo` for an unknown id
    /// (empty map + dummy id).
    #[test]
    fn mutation_commands_require_an_open_repo() {
        let state = AppState::default();
        let paths = vec!["file.txt".to_string()];

        let err = tauri::async_runtime::block_on(stage_inner(&state, MISSING_ID, paths.clone()))
            .expect_err("stage with no repo");
        assert!(matches!(err, AppError::NoRepo));

        let err = tauri::async_runtime::block_on(unstage_inner(&state, MISSING_ID, paths))
            .expect_err("unstage with no repo");
        assert!(matches!(err, AppError::NoRepo));

        let err =
            tauri::async_runtime::block_on(commit_inner(&state, MISSING_ID, "msg".to_string()))
                .expect_err("commit with no repo");
        assert!(matches!(err, AppError::NoRepo));
    }

    /// The P22 tag commands all return `NoRepo` for an unknown id (§8.4).
    #[test]
    fn tag_commands_require_an_open_repo() {
        let state = AppState::default();

        let err = tauri::async_runtime::block_on(create_tag_inner(
            &state,
            MISSING_ID,
            "v1".to_string(),
            "0".repeat(40),
            None,
            false,
        ))
        .expect_err("create_tag with no repo");
        assert!(matches!(err, AppError::NoRepo));

        let err = tauri::async_runtime::block_on(delete_tag_inner(
            &state,
            MISSING_ID,
            "v1".to_string(),
        ))
        .expect_err("delete_tag with no repo");
        assert!(matches!(err, AppError::NoRepo));

        let err = tauri::async_runtime::block_on(push_tag_inner(
            &state,
            MISSING_ID,
            "origin".to_string(),
            "v1".to_string(),
            false,
        ))
        .expect_err("push_tag with no repo");
        assert!(matches!(err, AppError::NoRepo));
    }

    /// The P22 remote-management commands all return `NoRepo` for an unknown
    /// id (§8.4).
    #[test]
    fn remote_mgmt_commands_require_an_open_repo() {
        let state = AppState::default();

        let err = tauri::async_runtime::block_on(list_remotes_inner(&state, MISSING_ID))
            .expect_err("list_remotes with no repo");
        assert!(matches!(err, AppError::NoRepo));

        let err = tauri::async_runtime::block_on(add_remote_inner(
            &state,
            MISSING_ID,
            "backup".to_string(),
            "https://example.com/repo.git".to_string(),
        ))
        .expect_err("add_remote with no repo");
        assert!(matches!(err, AppError::NoRepo));

        let err = tauri::async_runtime::block_on(remove_remote_inner(
            &state,
            MISSING_ID,
            "origin".to_string(),
        ))
        .expect_err("remove_remote with no repo");
        assert!(matches!(err, AppError::NoRepo));

        let err = tauri::async_runtime::block_on(rename_remote_inner(
            &state,
            MISSING_ID,
            "origin".to_string(),
            "upstream".to_string(),
        ))
        .expect_err("rename_remote with no repo");
        assert!(matches!(err, AppError::NoRepo));

        let err = tauri::async_runtime::block_on(set_remote_url_inner(
            &state,
            MISSING_ID,
            "origin".to_string(),
            "https://example.com/other.git".to_string(),
        ))
        .expect_err("set_remote_url with no repo");
        assert!(matches!(err, AppError::NoRepo));
    }

    /// The M4 diff commands all return `NoRepo` for an unknown id
    /// (contract §6.2 scenario 17).
    #[test]
    fn diff_commands_require_an_open_repo() {
        let state = AppState::default();

        let err = tauri::async_runtime::block_on(get_workdir_file_diff_inner(
            &state,
            MISSING_ID,
            "file.txt".to_string(),
            None,
            false,
            false,
        ))
        .expect_err("get_workdir_file_diff with no repo");
        assert!(matches!(err, AppError::NoRepo));

        let oid = "0123456789abcdef0123456789abcdef01234567".to_string();
        let err =
            tauri::async_runtime::block_on(get_commit_diff_inner(&state, MISSING_ID, oid.clone()))
                .expect_err("get_commit_diff with no repo");
        assert!(matches!(err, AppError::NoRepo));

        let err = tauri::async_runtime::block_on(get_commit_file_diff_inner(
            &state,
            MISSING_ID,
            oid,
            "file.txt".to_string(),
            None,
            false,
        ))
        .expect_err("get_commit_file_diff with no repo");
        assert!(matches!(err, AppError::NoRepo));
    }

    /// The P23c blame + file-history commands return `NoRepo` for an unknown id.
    #[test]
    fn blame_commands_require_an_open_repo() {
        let state = AppState::default();

        let err = tauri::async_runtime::block_on(blame_file_inner(
            &state,
            MISSING_ID,
            "src/app.ts".to_string(),
            None,
        ))
        .expect_err("blame_file with no repo");
        assert!(matches!(err, AppError::NoRepo));

        let err = tauri::async_runtime::block_on(file_history_inner(
            &state,
            MISSING_ID,
            "src/app.ts".to_string(),
            200,
        ))
        .expect_err("file_history with no repo");
        assert!(matches!(err, AppError::NoRepo));
    }

    /// The P17 partial-staging commands return `NoRepo` for an unknown id
    /// (contract §6.2 scenario 15) — the gate is `repo_path` before any git2.
    #[test]
    fn partial_staging_commands_require_an_open_repo() {
        let state = AppState::default();
        let selection = vec![LineSelection {
            kind: bonsai_core::git::diff::LineKind::Add,
            old_no: None,
            new_no: Some(1),
        }];

        let err = tauri::async_runtime::block_on(stage_partial_inner(
            &state,
            MISSING_ID,
            "file.txt".to_string(),
            None,
            selection.clone(),
        ))
        .expect_err("stage_partial with no repo");
        assert!(matches!(err, AppError::NoRepo));

        let err = tauri::async_runtime::block_on(unstage_partial_inner(
            &state,
            MISSING_ID,
            "file.txt".to_string(),
            None,
            selection,
        ))
        .expect_err("unstage_partial with no repo");
        assert!(matches!(err, AppError::NoRepo));
    }

    /// The P28 partial-discard command returns `NoRepo` for an unknown id —
    /// the gate is `repo_path` before any git2 work.
    #[test]
    fn discard_partial_command_requires_an_open_repo() {
        let state = AppState::default();
        let selection = vec![LineSelection {
            kind: bonsai_core::git::diff::LineKind::Add,
            old_no: None,
            new_no: Some(1),
        }];
        let err = tauri::async_runtime::block_on(discard_partial_inner(
            &state,
            MISSING_ID,
            "file.txt".to_string(),
            None,
            selection,
        ))
        .expect_err("discard_partial with no repo");
        assert!(matches!(err, AppError::NoRepo));
    }

    /// The P5 compare commands also return `NoRepo` for an unknown id
    /// (contract §6.2).
    #[test]
    fn compare_commands_require_an_open_repo() {
        let state = AppState::default();
        let oid = "0123456789abcdef0123456789abcdef01234567".to_string();

        let err = tauri::async_runtime::block_on(compare_with_head_inner(
            &state,
            MISSING_ID,
            oid.clone(),
        ))
        .expect_err("compare_with_head with no repo");
        assert!(matches!(err, AppError::NoRepo));

        let err = tauri::async_runtime::block_on(compare_with_head_file_diff_inner(
            &state,
            MISSING_ID,
            oid,
            "file.txt".to_string(),
            None,
            false,
        ))
        .expect_err("compare_with_head_file_diff with no repo");
        assert!(matches!(err, AppError::NoRepo));
    }

    /// The M5 branch commands all return `NoRepo` for an unknown id
    /// (contract §6.5).
    #[test]
    fn branch_commands_require_an_open_repo() {
        let state = AppState::default();

        let err = tauri::async_runtime::block_on(list_branches_inner(&state, MISSING_ID))
            .expect_err("list_branches with no repo");
        assert!(matches!(err, AppError::NoRepo));

        let err = tauri::async_runtime::block_on(create_branch_inner(
            &state,
            MISSING_ID,
            "topic".to_string(),
        ))
        .expect_err("create_branch with no repo");
        assert!(matches!(err, AppError::NoRepo));

        let err = tauri::async_runtime::block_on(checkout_branch_inner(
            &state,
            MISSING_ID,
            "topic".to_string(),
        ))
        .expect_err("checkout_branch with no repo");
        assert!(matches!(err, AppError::NoRepo));

        let err = tauri::async_runtime::block_on(delete_branch_inner(
            &state,
            MISSING_ID,
            "topic".to_string(),
        ))
        .expect_err("delete_branch with no repo");
        assert!(matches!(err, AppError::NoRepo));

        let err = tauri::async_runtime::block_on(checkout_remote_inner(
            &state,
            MISSING_ID,
            "origin/topic".to_string(),
        ))
        .expect_err("checkout_remote with no repo");
        assert!(matches!(err, AppError::NoRepo));

        let err = tauri::async_runtime::block_on(delete_remote_tracking_inner(
            &state,
            MISSING_ID,
            "origin/topic".to_string(),
        ))
        .expect_err("delete_remote_tracking with no repo");
        assert!(matches!(err, AppError::NoRepo));
    }

    /// The M6 remote commands all return `NoRepo` for an unknown id
    /// (contract §6.7).
    #[test]
    fn remote_commands_require_an_open_repo() {
        let state = AppState::default();

        let err = tauri::async_runtime::block_on(fetch_inner(&state, MISSING_ID))
            .expect_err("fetch with no repo");
        assert!(matches!(err, AppError::NoRepo));

        let err = tauri::async_runtime::block_on(pull_inner(&state, MISSING_ID))
            .expect_err("pull with no repo");
        assert!(matches!(err, AppError::NoRepo));

        let err = tauri::async_runtime::block_on(push_inner(&state, MISSING_ID))
            .expect_err("push with no repo");
        assert!(matches!(err, AppError::NoRepo));
    }

    /// The P3c merge/conflict commands all return `NoRepo` for an unknown id
    /// (contract §6).
    #[test]
    fn merge_commands_require_an_open_repo() {
        let state = AppState::default();

        let err = tauri::async_runtime::block_on(get_op_state_inner(&state, MISSING_ID))
            .expect_err("get_op_state with no repo");
        assert!(matches!(err, AppError::NoRepo));

        let err = tauri::async_runtime::block_on(merge_branch_inner(
            &state,
            MISSING_ID,
            "topic".to_string(),
        ))
        .expect_err("merge_branch with no repo");
        assert!(matches!(err, AppError::NoRepo));

        let err = tauri::async_runtime::block_on(commit_merge_inner(
            &state,
            MISSING_ID,
            "msg".to_string(),
        ))
        .expect_err("commit_merge with no repo");
        assert!(matches!(err, AppError::NoRepo));

        let err = tauri::async_runtime::block_on(abort_merge_inner(&state, MISSING_ID))
            .expect_err("abort_merge with no repo");
        assert!(matches!(err, AppError::NoRepo));

        let err = tauri::async_runtime::block_on(list_conflicts_inner(&state, MISSING_ID))
            .expect_err("list_conflicts with no repo");
        assert!(matches!(err, AppError::NoRepo));

        let err = tauri::async_runtime::block_on(get_conflict_inner(
            &state,
            MISSING_ID,
            "file.txt".to_string(),
        ))
        .expect_err("get_conflict with no repo");
        assert!(matches!(err, AppError::NoRepo));

        let err = tauri::async_runtime::block_on(resolve_conflict_inner(
            &state,
            MISSING_ID,
            "file.txt".to_string(),
            ConflictResolution::Ours,
        ))
        .expect_err("resolve_conflict with no repo");
        assert!(matches!(err, AppError::NoRepo));

        let err = tauri::async_runtime::block_on(resolve_conflict_text_inner(
            &state,
            MISSING_ID,
            "file.txt".to_string(),
            "resolved\n".to_string(),
        ))
        .expect_err("resolve_conflict_text with no repo");
        assert!(matches!(err, AppError::NoRepo));
    }

    /// The P3d rebase commands all return `NoRepo` for an unknown id
    /// (contract §4).
    #[test]
    fn rebase_commands_require_an_open_repo() {
        let state = AppState::default();

        let err = tauri::async_runtime::block_on(rebase_branch_inner(
            &state,
            MISSING_ID,
            "main".to_string(),
        ))
        .expect_err("rebase_branch with no repo");
        assert!(matches!(err, AppError::NoRepo));

        let err = tauri::async_runtime::block_on(rebase_continue_inner(&state, MISSING_ID))
            .expect_err("rebase_continue with no repo");
        assert!(matches!(err, AppError::NoRepo));

        let err = tauri::async_runtime::block_on(rebase_skip_inner(&state, MISSING_ID))
            .expect_err("rebase_skip with no repo");
        assert!(matches!(err, AppError::NoRepo));

        let err = tauri::async_runtime::block_on(rebase_abort_inner(&state, MISSING_ID))
            .expect_err("rebase_abort with no repo");
        assert!(matches!(err, AppError::NoRepo));
    }

    /// The P23 interactive-rebase commands return `NoRepo` for an unknown id.
    #[test]
    fn interactive_rebase_commands_require_an_open_repo() {
        let state = AppState::default();

        let err = tauri::async_runtime::block_on(get_interactive_plan_inner(
            &state,
            MISSING_ID,
            "a".repeat(40),
        ))
        .expect_err("get_interactive_plan with no repo");
        assert!(matches!(err, AppError::NoRepo));

        let err = tauri::async_runtime::block_on(start_interactive_rebase_inner(
            &state,
            MISSING_ID,
            "a".repeat(40),
            Vec::new(),
        ))
        .expect_err("start_interactive_rebase with no repo");
        assert!(matches!(err, AppError::NoRepo));
    }

    // ---- P3e-a two-repo isolation (contract §9.1) --------------------------

    /// Committing in A leaves B's status/graph unaffected and A reflects the
    /// change.
    #[test]
    fn isolation_independent_status_and_commit() {
        let state = AppState::default();

        let dir_a = init_repo_with_identity();
        let dir_b = init_repo_with_identity();
        let id_a = open(&state, dir_a.path()).expect("open A").repo_id;
        let id_b = open(&state, dir_b.path()).expect("open B").repo_id;
        assert_ne!(id_a, id_b);

        write_stage_commit(&state, &id_a, dir_a.path(), "a.txt", "hello", "first in A");

        // A now has one commit; its status is clean.
        let graph_a = tauri::async_runtime::block_on(get_graph_inner(&state, &id_a))
            .expect("graph A");
        assert_eq!(graph_a.nodes.len(), 1, "A should have exactly one commit");
        let status_a = tauri::async_runtime::block_on(get_status_inner(&state, &id_a))
            .expect("status A");
        assert!(status_a.staged.is_empty() && status_a.unstaged.is_empty());

        // B is untouched: still unborn, empty graph, no files.
        let graph_b = tauri::async_runtime::block_on(get_graph_inner(&state, &id_b))
            .expect("graph B");
        assert!(graph_b.nodes.is_empty(), "B must be unaffected by a commit in A");
        let status_b = tauri::async_runtime::block_on(get_status_inner(&state, &id_b))
            .expect("status B");
        assert!(
            status_b.staged.is_empty()
                && status_b.unstaged.is_empty()
                && status_b.untracked.is_empty(),
            "B working dir must be empty"
        );
    }

    /// A branch created in A does not appear in B; B's op-state stays `None`.
    #[test]
    fn isolation_independent_branches_and_op_state() {
        let state = AppState::default();

        let dir_a = init_repo_with_identity();
        let dir_b = init_repo_with_identity();
        let id_a = open(&state, dir_a.path()).expect("open A").repo_id;
        let id_b = open(&state, dir_b.path()).expect("open B").repo_id;

        // Need a commit before a branch can be created at HEAD.
        write_stage_commit(&state, &id_a, dir_a.path(), "a.txt", "hello", "first in A");
        tauri::async_runtime::block_on(create_branch_inner(&state, &id_a, "x".to_string()))
            .expect("create branch x in A");

        let branches_a = tauri::async_runtime::block_on(list_branches_inner(&state, &id_a))
            .expect("branches A");
        assert!(
            branches_a.local.iter().any(|b| b.name == "x"),
            "A must have branch x"
        );

        let branches_b = tauri::async_runtime::block_on(list_branches_inner(&state, &id_b))
            .expect("branches B");
        assert!(
            !branches_b.local.iter().any(|b| b.name == "x"),
            "B must NOT have branch x"
        );

        let op_b = tauri::async_runtime::block_on(get_op_state_inner(&state, &id_b))
            .expect("op-state B");
        assert_eq!(op_b, RepoOpState::None, "B op-state must stay None");
    }

    /// Closing A makes A's commands `NoRepo` while B keeps working; the map
    /// then holds exactly one entry.
    #[test]
    fn isolation_close_only_affects_target() {
        let state = AppState::default();

        let dir_a = init_repo_with_identity();
        let dir_b = init_repo_with_identity();
        let id_a = open(&state, dir_a.path()).expect("open A").repo_id;
        let id_b = open(&state, dir_b.path()).expect("open B").repo_id;
        assert_eq!(repo_count(&state), 2);

        tauri::async_runtime::block_on(close_repo_inner(&state, &id_a)).expect("close A");

        let err = tauri::async_runtime::block_on(get_status_inner(&state, &id_a))
            .expect_err("A must be closed");
        assert!(matches!(err, AppError::NoRepo));

        tauri::async_runtime::block_on(get_status_inner(&state, &id_b)).expect("B still open");
        assert_eq!(repo_count(&state), 1, "exactly one entry after closing A");
    }

    /// Opening A's path twice (including a case-variant) focuses the same
    /// entry: same `repo_id`, one map entry.
    #[test]
    fn isolation_focus_dedupe_on_reopen() {
        let state = AppState::default();

        let dir_a = init_repo_with_identity();
        let first = open(&state, dir_a.path()).expect("open A").repo_id;
        assert_eq!(repo_count(&state), 1);

        // Re-open the exact same path.
        let again = open(&state, dir_a.path()).expect("re-open A").repo_id;
        assert_eq!(first, again, "re-opening the same path must reuse the id");
        assert_eq!(repo_count(&state), 1, "no duplicate entry on re-open");

        // Re-open via an ASCII case-variant of the path (Windows is
        // case-insensitive): still the same entry.
        let variant = path_string(dir_a.path()).to_uppercase();
        let cased = tauri::async_runtime::block_on(open_repo_inner(
            &state,
            variant,
            |_id| Box::new(|| {}),
        ))
        .expect("re-open A (case-variant)")
        .repo_id;
        assert_eq!(
            first, cased,
            "a case-variant path must dedupe to the same id"
        );
        assert_eq!(repo_count(&state), 1, "case-variant must not add an entry");
    }

    /// Closing an unknown id is a no-op `Ok(())` (idempotent).
    #[test]
    fn isolation_idempotent_close_of_unknown_id() {
        let state = AppState::default();
        tauri::async_runtime::block_on(close_repo_inner(&state, "does-not-exist"))
            .expect("closing an unknown id must be Ok(())");
        assert_eq!(repo_count(&state), 0);
    }

    /// Drives the repo `id` (workdir `dir`) into a PAUSED merge with a conflict
    /// on `a.txt`, entirely through the command inners + git2 checkout, and
    /// returns the base branch name. Post-condition: `merge_branch_inner`
    /// returned `Conflicts` and the repo is in `RepoOpState::Merge`.
    ///
    /// Recipe mirrors `merge_cli::script_conflict`: same middle line edited on
    /// both the base branch and `topic`, so the true merge is guaranteed to
    /// conflict.
    fn start_conflicting_merge(state: &AppState, id: &str, dir: &std::path::Path) -> String {
        // Base commit on the default branch.
        write_stage_commit(state, id, dir, "a.txt", "line1\nbase\nline3\n", "base");
        let base_branch = tauri::async_runtime::block_on(list_branches_inner(state, id))
            .expect("branches after base commit")
            .head
            .branch_name
            .expect("HEAD has a branch name after the first commit");

        // topic diverges: edits the middle line differently.
        tauri::async_runtime::block_on(create_branch_inner(state, id, "topic".to_string()))
            .expect("create topic");
        tauri::async_runtime::block_on(checkout_branch_inner(state, id, "topic".to_string()))
            .expect("checkout topic");
        write_stage_commit(state, id, dir, "a.txt", "line1\ntopic\nline3\n", "topic change");

        // Back on the base branch: a conflicting edit to the same line.
        tauri::async_runtime::block_on(checkout_branch_inner(state, id, base_branch.clone()))
            .expect("checkout base branch");
        write_stage_commit(state, id, dir, "a.txt", "line1\nmain\nline3\n", "main change");

        // Merge topic → guaranteed conflict, repo pauses in Merge state.
        let outcome =
            tauri::async_runtime::block_on(merge_branch_inner(state, id, "topic".to_string()))
                .expect("merge_branch");
        match outcome {
            MergeOutcome::Conflicts { paths, .. } => {
                assert!(
                    paths.iter().any(|p| p == "a.txt"),
                    "expected a.txt to be conflicted, got {paths:?}"
                );
            }
            other => panic!("expected a conflicting merge, got {other:?}"),
        }
        base_branch
    }

    /// An in-progress MERGE in repo A must NOT leak into repo B: B's op-state
    /// stays `None`, and B's status/branches are untouched, while A genuinely
    /// reflects the paused merge. This strengthens
    /// `isolation_independent_branches_and_op_state` (whose op-state half was
    /// tautological — it never started an operation). Contract §9.1.
    #[test]
    fn isolation_in_progress_merge_does_not_leak() {
        let state = AppState::default();

        let dir_a = init_repo_with_identity();
        let dir_b = init_repo_with_identity();
        let id_a = open(&state, dir_a.path()).expect("open A").repo_id;
        let id_b = open(&state, dir_b.path()).expect("open B").repo_id;
        assert_ne!(id_a, id_b);

        // Give B a real (but independent) history so "unaffected" is a
        // meaningful assertion rather than "both are empty".
        write_stage_commit(&state, &id_b, dir_b.path(), "b.txt", "b-only\n", "b base");
        tauri::async_runtime::block_on(create_branch_inner(&state, &id_b, "keep".to_string()))
            .expect("create branch keep in B");

        // Snapshot B before the merge storm in A.
        let branches_b_before = tauri::async_runtime::block_on(list_branches_inner(&state, &id_b))
            .expect("branches B before");
        let status_b_before = tauri::async_runtime::block_on(get_status_inner(&state, &id_b))
            .expect("status B before");
        let graph_b_before = tauri::async_runtime::block_on(get_graph_inner(&state, &id_b))
            .expect("graph B before");

        // Now drive A into a paused merge.
        start_conflicting_merge(&state, &id_a, dir_a.path());

        // A genuinely reflects the in-progress merge.
        let op_a = tauri::async_runtime::block_on(get_op_state_inner(&state, &id_a))
            .expect("op-state A");
        assert!(
            matches!(op_a, RepoOpState::Merge { .. }),
            "A must be paused in a merge, got {op_a:?}"
        );
        let conflicts_a = tauri::async_runtime::block_on(list_conflicts_inner(&state, &id_a))
            .expect("conflicts A");
        assert!(
            conflicts_a.iter().any(|c| c.path == "a.txt"),
            "A must list a.txt as conflicted, got {conflicts_a:?}"
        );

        // B is entirely unaffected: op-state None, and its branches/status/graph
        // are byte-identical to the pre-merge snapshot.
        let op_b = tauri::async_runtime::block_on(get_op_state_inner(&state, &id_b))
            .expect("op-state B");
        assert_eq!(op_b, RepoOpState::None, "B op-state must stay None");

        let branches_b_after = tauri::async_runtime::block_on(list_branches_inner(&state, &id_b))
            .expect("branches B after");
        assert_eq!(
            branches_b_after.local.iter().map(|b| b.name.clone()).collect::<Vec<_>>(),
            branches_b_before.local.iter().map(|b| b.name.clone()).collect::<Vec<_>>(),
            "B's branch set must be unchanged"
        );
        assert!(
            !branches_b_after.local.iter().any(|b| b.name == "topic"),
            "B must not have gained A's 'topic' branch"
        );

        let status_b_after = tauri::async_runtime::block_on(get_status_inner(&state, &id_b))
            .expect("status B after");
        assert_eq!(
            (status_b_after.staged.len(), status_b_after.unstaged.len(), status_b_after.untracked.len()),
            (status_b_before.staged.len(), status_b_before.unstaged.len(), status_b_before.untracked.len()),
            "B's working-dir status must be unchanged"
        );

        let graph_b_after = tauri::async_runtime::block_on(get_graph_inner(&state, &id_b))
            .expect("graph B after");
        assert_eq!(
            graph_b_after.nodes.len(),
            graph_b_before.nodes.len(),
            "B's commit graph must be unchanged"
        );
    }

    /// Closing repo B while repo A has a PAUSED merge must not disturb A's
    /// on-disk operation: A's op-state is still `Merge` and its conflicts are
    /// still readable, and the map holds exactly A. Contract §9.1 (close edge).
    #[test]
    fn isolation_close_preserves_other_repos_in_progress_op() {
        let state = AppState::default();

        let dir_a = init_repo_with_identity();
        let dir_b = init_repo_with_identity();
        let id_a = open(&state, dir_a.path()).expect("open A").repo_id;
        let id_b = open(&state, dir_b.path()).expect("open B").repo_id;

        start_conflicting_merge(&state, &id_a, dir_a.path());

        // Close B while A is mid-merge.
        tauri::async_runtime::block_on(close_repo_inner(&state, &id_b)).expect("close B");
        assert_eq!(repo_count(&state), 1, "only A remains open");

        // A's in-progress merge survives the close of B, fully readable.
        let op_a = tauri::async_runtime::block_on(get_op_state_inner(&state, &id_a))
            .expect("op-state A after closing B");
        assert!(
            matches!(op_a, RepoOpState::Merge { .. }),
            "A must still be paused in a merge after B is closed, got {op_a:?}"
        );
        let conflicts_a = tauri::async_runtime::block_on(list_conflicts_inner(&state, &id_a))
            .expect("conflicts A after closing B");
        assert!(
            conflicts_a.iter().any(|c| c.path == "a.txt"),
            "A must still list a.txt as conflicted after closing B, got {conflicts_a:?}"
        );
    }

    /// Patching only `theme` leaves `pane_widths`/`list_view` untouched, and
    /// each other single-field patch is equally partial (P2a contract §3.4.3;
    /// P3b contract §2.1).
    #[test]
    fn set_ui_settings_patch_is_partial() {
        let mut s = settings::Settings::default();
        let original_widths = s.pane_widths;

        apply_patch(
            &mut s,
            UiSettingsPatch {
                theme: Some(ThemeChoice::Light),
                pane_widths: None,
                list_view: None,
                ..Default::default()
            },
        );
        assert_eq!(s.theme, ThemeChoice::Light);
        assert_eq!(s.pane_widths, original_widths);
        assert_eq!(s.list_view, settings::ListView::Tree);

        apply_patch(
            &mut s,
            UiSettingsPatch {
                theme: None,
                pane_widths: Some(PaneWidths {
                    sidebar: 300,
                    right_panel: 400,
                }),
                list_view: None,
                ..Default::default()
            },
        );
        assert_eq!(s.theme, ThemeChoice::Light); // untouched by the second patch
        assert_eq!(s.list_view, settings::ListView::Tree);
        assert_eq!(
            s.pane_widths,
            PaneWidths {
                sidebar: 300,
                right_panel: 400,
            }
        );

        // Patching only `list_view` leaves theme + pane widths untouched.
        apply_patch(
            &mut s,
            UiSettingsPatch {
                theme: None,
                pane_widths: None,
                list_view: Some(settings::ListView::Flat),
                ..Default::default()
            },
        );
        assert_eq!(s.list_view, settings::ListView::Flat);
        assert_eq!(s.theme, ThemeChoice::Light);
        assert_eq!(
            s.pane_widths,
            PaneWidths {
                sidebar: 300,
                right_panel: 400,
            }
        );

        // Out-of-range pane widths in a patch get clamped on write.
        apply_patch(
            &mut s,
            UiSettingsPatch {
                theme: None,
                pane_widths: Some(PaneWidths {
                    sidebar: 5,
                    right_panel: 5000,
                }),
                list_view: None,
                ..Default::default()
            },
        );
        assert_eq!(s.pane_widths.sidebar, settings::SIDEBAR_MIN);
        assert_eq!(s.pane_widths.right_panel, settings::RIGHT_PANEL_MAX);
    }

    /// `auto_fetch` and `graph` patch independently, leave the other fields
    /// unchanged when `None`, and are clamped on write (P11 §2.4).
    #[test]
    fn set_ui_settings_patch_auto_fetch_and_graph() {
        let mut s = settings::Settings::default();
        let original_af = s.auto_fetch;
        let original_graph = s.graph;

        // Only `auto_fetch` changes auto-fetch; everything else untouched.
        apply_patch(
            &mut s,
            UiSettingsPatch {
                auto_fetch: Some(AutoFetch {
                    enabled: true,
                    interval_minutes: 20,
                }),
                ..Default::default()
            },
        );
        assert_eq!(
            s.auto_fetch,
            AutoFetch {
                enabled: true,
                interval_minutes: 20,
            }
        );
        assert_eq!(s.graph, original_graph);
        assert_eq!(s.theme, ThemeChoice::default());

        // Only `graph` changes graph; auto-fetch preserved from the prior patch.
        apply_patch(
            &mut s,
            UiSettingsPatch {
                graph: Some(GraphPrefs {
                    dot_radius: 5,
                    avatar_radius: 12,
                    row_height: 36,
                    lane_width: 20,
                }),
                ..Default::default()
            },
        );
        assert_eq!(
            s.graph,
            GraphPrefs {
                dot_radius: 5,
                avatar_radius: 12,
                row_height: 36,
                lane_width: 20,
            }
        );
        assert_eq!(
            s.auto_fetch,
            AutoFetch {
                enabled: true,
                interval_minutes: 20,
            }
        );

        // An empty patch leaves both new fields unchanged.
        apply_patch(&mut s, UiSettingsPatch::default());
        assert_eq!(
            s.auto_fetch,
            AutoFetch {
                enabled: true,
                interval_minutes: 20,
            }
        );
        assert_eq!(
            s.graph,
            GraphPrefs {
                dot_radius: 5,
                avatar_radius: 12,
                row_height: 36,
                lane_width: 20,
            }
        );

        // Out-of-range interval (0) clamps to the min on write.
        apply_patch(
            &mut s,
            UiSettingsPatch {
                auto_fetch: Some(AutoFetch {
                    enabled: true,
                    interval_minutes: 0,
                }),
                ..Default::default()
            },
        );
        assert_eq!(s.auto_fetch.interval_minutes, settings::AUTO_FETCH_INTERVAL_MIN);

        // Out-of-range interval (999) clamps to the max on write.
        apply_patch(
            &mut s,
            UiSettingsPatch {
                auto_fetch: Some(AutoFetch {
                    enabled: false,
                    interval_minutes: 999,
                }),
                ..Default::default()
            },
        );
        assert_eq!(s.auto_fetch.interval_minutes, settings::AUTO_FETCH_INTERVAL_MAX);

        // Below-min / above-max graph knobs clamp to their bounds on write.
        apply_patch(
            &mut s,
            UiSettingsPatch {
                graph: Some(GraphPrefs {
                    dot_radius: 0,
                    avatar_radius: 9999,
                    row_height: 0,
                    lane_width: 9999,
                }),
                ..Default::default()
            },
        );
        assert_eq!(s.graph.dot_radius, settings::DOT_RADIUS_MIN);
        assert_eq!(s.graph.avatar_radius, settings::AVATAR_RADIUS_MAX);
        assert_eq!(s.graph.row_height, settings::ROW_HEIGHT_MIN);
        assert_eq!(s.graph.lane_width, settings::LANE_WIDTH_MAX);

        // Sanity: the `original_*` snapshots were genuinely the defaults.
        assert_eq!(original_af, AutoFetch::default());
        assert_eq!(original_graph, GraphPrefs::default());
    }

    /// The three AI fields patch independently: patching only `ai_enabled`
    /// leaves autonomy + consent untouched (and vice versa), and an empty
    /// patch mutates nothing (P13 §4.2).
    #[test]
    fn set_ui_settings_patch_ai_is_partial() {
        let mut s = settings::Settings::default();
        // Defaults sanity: enabled true, ProposeReview, not consented.
        assert!(s.ai_enabled);
        assert_eq!(s.ai_conflict_autonomy, AiAutonomy::ProposeReview);
        assert!(!s.ai_consented);

        // Only `ai_enabled` changes; autonomy + consent untouched.
        apply_patch(
            &mut s,
            UiSettingsPatch {
                ai_enabled: Some(false),
                ..Default::default()
            },
        );
        assert!(!s.ai_enabled);
        assert_eq!(s.ai_conflict_autonomy, AiAutonomy::ProposeReview);
        assert!(!s.ai_consented);
        // Unrelated fields untouched too.
        assert_eq!(s.theme, ThemeChoice::default());

        // Only `ai_consented` changes; enabled + autonomy preserved.
        apply_patch(
            &mut s,
            UiSettingsPatch {
                ai_consented: Some(true),
                ..Default::default()
            },
        );
        assert!(s.ai_consented);
        assert!(!s.ai_enabled);
        assert_eq!(s.ai_conflict_autonomy, AiAutonomy::ProposeReview);

        // Only `ai_conflict_autonomy` changes; enabled + consent preserved.
        apply_patch(
            &mut s,
            UiSettingsPatch {
                ai_conflict_autonomy: Some(AiAutonomy::AutoResolve),
                ..Default::default()
            },
        );
        assert_eq!(s.ai_conflict_autonomy, AiAutonomy::AutoResolve);
        assert!(!s.ai_enabled);
        assert!(s.ai_consented);

        // An empty patch leaves all three AI fields unchanged.
        apply_patch(&mut s, UiSettingsPatch::default());
        assert!(!s.ai_enabled);
        assert_eq!(s.ai_conflict_autonomy, AiAutonomy::AutoResolve);
        assert!(s.ai_consented);
    }

    /// `ai_resolve_conflict` enforces the backend consent gate (§9.6) BEFORE
    /// touching the repo: default settings (`ai_consented=false`) → `AiUnavailable`
    /// even with no repo open; once enabled+consented, an unknown repo id →
    /// `NoRepo` (the gate passed, `repo_path` then fails). Covers the
    /// AppHandle-free part of the command via its inner (P13 §6).
    #[test]
    fn ai_resolve_conflict_enforces_consent_gate_then_no_repo() {
        let state = AppState::default();
        let dir = tempfile::TempDir::new().expect("temp dir");
        let file = dir.path().join("settings.json");

        // No settings file → defaults → not consented → the gate refuses.
        let err = tauri::async_runtime::block_on(ai_resolve_conflict_inner(
            &state,
            &file,
            MISSING_ID,
            "a.txt".to_string(),
        ))
        .expect_err("disabled gate must refuse");
        assert!(matches!(err, AppError::AiUnavailable(_)), "got {err:?}");

        // P13 tester: the gate is `ai_enabled && ai_consented` — the OTHER OR-half.
        // Consented but DISABLED must still refuse (proves it is AND, not OR).
        let s = settings::Settings {
            ai_enabled: false,
            ai_consented: true,
            ..settings::Settings::default()
        };
        settings::save_to(&file, &s).expect("save settings");
        let err = tauri::async_runtime::block_on(ai_resolve_conflict_inner(
            &state,
            &file,
            MISSING_ID,
            "a.txt".to_string(),
        ))
        .expect_err("enabled=false must refuse even when consented");
        assert!(matches!(err, AppError::AiUnavailable(_)), "got {err:?}");

        // Enable + consent; now the gate passes and the missing repo → NoRepo.
        let s = settings::Settings {
            ai_enabled: true,
            ai_consented: true,
            ..settings::Settings::default()
        };
        settings::save_to(&file, &s).expect("save settings");
        let err = tauri::async_runtime::block_on(ai_resolve_conflict_inner(
            &state,
            &file,
            MISSING_ID,
            "a.txt".to_string(),
        ))
        .expect_err("no repo open must be NoRepo");
        assert!(matches!(err, AppError::NoRepo), "got {err:?}");
    }

    /// P15a §8.5: `generate_commit_message` enforces the same backend consent
    /// gate BEFORE touching the repo: default settings (`ai_consented=false`) →
    /// `AiUnavailable`; once enabled+consented, an unknown repo id → `NoRepo`
    /// (the gate passed, `repo_path` then fails). No CLI needed.
    #[test]
    fn generate_commit_message_enforces_consent_gate_then_no_repo() {
        let state = AppState::default();
        let dir = tempfile::TempDir::new().expect("temp dir");
        let file = dir.path().join("settings.json");

        // No settings file → defaults → not consented → the gate refuses.
        let err = tauri::async_runtime::block_on(generate_commit_message_inner(
            &state, &file, MISSING_ID,
        ))
        .expect_err("disabled gate must refuse");
        assert!(matches!(err, AppError::AiUnavailable(_)), "got {err:?}");

        // Enable + consent; now the gate passes and the missing repo → NoRepo.
        let s = settings::Settings {
            ai_enabled: true,
            ai_consented: true,
            ..settings::Settings::default()
        };
        settings::save_to(&file, &s).expect("save settings");
        let err = tauri::async_runtime::block_on(generate_commit_message_inner(
            &state, &file, MISSING_ID,
        ))
        .expect_err("no repo open must be NoRepo");
        assert!(matches!(err, AppError::NoRepo), "got {err:?}");
    }

    /// P28 §5: `ai_digest` enforces the same backend consent gate BEFORE
    /// touching the repo: default settings (`ai_consented=false`) →
    /// `AiUnavailable`; once enabled+consented, an unknown repo id → `NoRepo`
    /// (the gate passed, `repo_path` then fails). No CLI needed.
    #[test]
    fn ai_digest_enforces_consent_gate_then_no_repo() {
        let state = AppState::default();
        let dir = tempfile::TempDir::new().expect("temp dir");
        let file = dir.path().join("settings.json");
        let range = || AiDigestRange::LastDays { days: 7 };

        // No settings file → defaults → not consented → the gate refuses.
        let err =
            tauri::async_runtime::block_on(ai_digest_inner(&state, &file, MISSING_ID, range()))
                .expect_err("disabled gate must refuse");
        assert!(matches!(err, AppError::AiUnavailable(_)), "got {err:?}");

        // Enable + consent; now the gate passes and the missing repo → NoRepo.
        let s = settings::Settings {
            ai_enabled: true,
            ai_consented: true,
            ..settings::Settings::default()
        };
        settings::save_to(&file, &s).expect("save settings");
        let err =
            tauri::async_runtime::block_on(ai_digest_inner(&state, &file, MISSING_ID, range()))
                .expect_err("no repo open must be NoRepo");
        assert!(matches!(err, AppError::NoRepo), "got {err:?}");
    }

    /// P15b §5/§8.5: `ai_analyze_diff` enforces the same backend consent gate
    /// BEFORE touching the repo: default settings (`ai_consented=false`) →
    /// `AiUnavailable`; once enabled+consented, an unknown repo id → `NoRepo`
    /// (the gate passed, `repo_path` then fails). No CLI needed.
    #[test]
    fn ai_analyze_diff_enforces_consent_gate_then_no_repo() {
        let state = AppState::default();
        let dir = tempfile::TempDir::new().expect("temp dir");
        let file = dir.path().join("settings.json");

        // No settings file → defaults → not consented → the gate refuses.
        let err = tauri::async_runtime::block_on(ai_analyze_diff_inner(
            &state,
            &file,
            MISSING_ID,
            AiDiffTarget::Staged,
            AiAnalysisMode::Review,
        ))
        .expect_err("disabled gate must refuse");
        assert!(matches!(err, AppError::AiUnavailable(_)), "got {err:?}");

        // Enable + consent; now the gate passes and the missing repo → NoRepo.
        let s = settings::Settings {
            ai_enabled: true,
            ai_consented: true,
            ..settings::Settings::default()
        };
        settings::save_to(&file, &s).expect("save settings");
        let err = tauri::async_runtime::block_on(ai_analyze_diff_inner(
            &state,
            &file,
            MISSING_ID,
            AiDiffTarget::Commit {
                oid: "0123456789abcdef0123456789abcdef01234567".to_string(),
            },
            AiAnalysisMode::Explain,
        ))
        .expect_err("no repo open must be NoRepo");
        assert!(matches!(err, AppError::NoRepo), "got {err:?}");
    }

    /// P15c §5/§8.5: `ai_summarize_range` enforces the same backend consent gate
    /// BEFORE touching the repo: default settings (`ai_consented=false`) →
    /// `AiUnavailable`; once enabled+consented, an unknown repo id → `NoRepo`
    /// (the gate passed, `repo_path` then fails). No CLI needed.
    #[test]
    fn ai_summarize_range_enforces_consent_gate_then_no_repo() {
        let state = AppState::default();
        let dir = tempfile::TempDir::new().expect("temp dir");
        let file = dir.path().join("settings.json");

        // No settings file → defaults → not consented → the gate refuses.
        let err = tauri::async_runtime::block_on(ai_summarize_range_inner(
            &state,
            &file,
            MISSING_ID,
            "main".to_string(),
            "feature".to_string(),
        ))
        .expect_err("disabled gate must refuse");
        assert!(matches!(err, AppError::AiUnavailable(_)), "got {err:?}");

        // Enable + consent; now the gate passes and the missing repo → NoRepo.
        let s = settings::Settings {
            ai_enabled: true,
            ai_consented: true,
            ..settings::Settings::default()
        };
        settings::save_to(&file, &s).expect("save settings");
        let err = tauri::async_runtime::block_on(ai_summarize_range_inner(
            &state,
            &file,
            MISSING_ID,
            "main".to_string(),
            "feature".to_string(),
        ))
        .expect_err("no repo open must be NoRepo");
        assert!(matches!(err, AppError::NoRepo), "got {err:?}");
    }

    /// P31 §5: the three worktree-context commands resolve the repo by id
    /// (`NoRepo` for unknown ids) and round-trip against the core: the matrix
    /// carries the `@main` row, preview is read-only, and activation writes
    /// the target file + records the activation.
    #[test]
    fn worktree_context_commands_round_trip() {
        let state = AppState::default();

        for res in [
            tauri::async_runtime::block_on(list_worktree_contexts_inner(&state, MISSING_ID))
                .map(|_| ()),
            tauri::async_runtime::block_on(preview_worktree_profile_inner(
                &state,
                MISSING_ID,
                "@main".to_string(),
                "p".to_string(),
            ))
            .map(|_| ()),
            tauri::async_runtime::block_on(activate_worktree_profile_inner(
                &state,
                MISSING_ID,
                "@main".to_string(),
                "p".to_string(),
            ))
            .map(|_| ()),
        ] {
            assert!(matches!(res.expect_err("unknown id"), AppError::NoRepo));
        }

        let dir = init_repo_with_identity();
        let opened = open(&state, dir.path()).expect("open repo");
        let id = &opened.repo_id;
        write_stage_commit(&state, id, dir.path(), "a.txt", "a\n", "C0");
        bonsai_core::assets::save_profile(
            dir.path(),
            bonsai_core::assets::ContextProfile {
                name: "p".to_string(),
                description: None,
                model: None,
                targets: vec![bonsai_core::assets::ProfileTarget {
                    asset_id: "claude".to_string(),
                    content: "# from command\n".to_string(),
                }],
            },
        )
        .expect("save profile");

        // Matrix: single main row, keyed "@main", activatable, no activation yet.
        let rows = tauri::async_runtime::block_on(list_worktree_contexts_inner(&state, id))
            .expect("matrix");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].worktree_key, "@main");
        assert!(rows[0].is_main && rows[0].activatable);
        assert_eq!(rows[0].active_profile, None);

        // Preview writes nothing.
        let preview = tauri::async_runtime::block_on(preview_worktree_profile_inner(
            &state,
            id,
            "@main".to_string(),
            "p".to_string(),
        ))
        .expect("preview");
        assert_eq!(preview.len(), 1);
        assert!(preview[0].changed);
        assert!(!dir.path().join("CLAUDE.md").exists());

        // Activate writes the target + records the "@main" activation.
        let act = tauri::async_runtime::block_on(activate_worktree_profile_inner(
            &state,
            id,
            "@main".to_string(),
            "p".to_string(),
        ))
        .expect("activate");
        assert_eq!(act.profile, "p");
        assert_eq!(
            std::fs::read(dir.path().join("CLAUDE.md")).expect("read CLAUDE.md"),
            b"# from command\n"
        );
        let rows = tauri::async_runtime::block_on(list_worktree_contexts_inner(&state, id))
            .expect("matrix after activation");
        assert_eq!(rows[0].active_profile.as_deref(), Some("p"));

        // Unknown worktree key surfaces the core's Git error.
        let err = tauri::async_runtime::block_on(activate_worktree_profile_inner(
            &state,
            id,
            "nope".to_string(),
            "p".to_string(),
        ))
        .expect_err("unknown worktree key");
        assert!(matches!(err, AppError::Git(m) if m.contains("not found")));
    }
}
