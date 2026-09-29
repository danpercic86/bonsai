//! The AppHandle-level glue: building the managed `DepsState` at startup
//! (incl. the startup cache wipe when disabled, §6.4), the production
//! `deps-changed` emitter, and the one-line hooks called from `open_repo`'s
//! watcher callback, `set_active_repo`, `close_repo`, `set_ui_settings` and
//! the scheduler loop. Everything with logic lives in `exec.rs`.

use std::path::PathBuf;
use std::sync::Arc;

use bonsai_deps::remote::{OutboundRequest, RawResponse, ReqwestTransport, TransportError};
use bonsai_deps::Transport;
use tauri::Manager;

use super::host_env::RealHostEnv;
use super::{lock, store, DepsDeps, DepsEmitFn, DepsState};
use crate::settings::DepTracker;
use crate::state::AppState;

/// Stands in when the reqwest client cannot be built: every send is a
/// connect failure (⇒ `offline`), never a panic.
struct Unavailable;

impl Transport for Unavailable {
    fn send(&self, _req: &OutboundRequest, _cap: usize) -> Result<RawResponse, TransportError> {
        Err(TransportError::Connect)
    }
}

fn emitter_for(app: tauri::AppHandle) -> DepsEmitFn {
    Arc::new(move |p| {
        // Scheduler ticks, watcher bursts and detached runs carry no command
        // trace: `root("backend")` is the honest causality.
        crate::obs::emit_logged(
            &app,
            "deps-changed",
            p,
            &crate::obs::TraceMeta::root("backend"),
        );
    })
}

/// Called once from `setup` with the persisted `dep_tracker` setting.
pub fn install(app: &tauri::AppHandle, cfg: DepTracker) {
    let transport: Arc<dyn Transport> = match ReqwestTransport::new() {
        Ok(t) => Arc::new(t),
        Err(_) => {
            eprintln!("bonsai: dependency-tracker HTTP client unavailable (checks report offline)");
            Arc::new(Unavailable)
        }
    };
    // Same base dir as the history index (`commands::shared::app_data_root`).
    let data_root = app.path().app_data_dir().ok();
    if !cfg.enabled {
        // §6.4 startup retry of a disable wipe that failed last session.
        let root = data_root.clone();
        tauri::async_runtime::spawn_blocking(move || store::wipe(root.as_deref()));
    }
    app.manage(DepsState::new(DepsDeps {
        cfg,
        transport,
        env: Arc::new(RealHostEnv::new()),
        data_root,
        emit: emitter_for(app.clone()),
    }));
}

/// The open repo's canonical workdir (lock-and-clone, like `repo_path`).
fn repo_path(app: &tauri::AppHandle, repo_id: &str) -> Option<PathBuf> {
    lock(&app.state::<AppState>().repos)
        .get(repo_id)
        .map(|e| e.path.clone())
}

/// Every open tab's repoId.
pub fn open_repo_ids(app: &tauri::AppHandle) -> Vec<String> {
    lock(&app.state::<AppState>().repos)
        .keys()
        .cloned()
        .collect()
}

/// The focused tab as (repoId, workdir), if it is open.
fn active(app: &tauri::AppHandle) -> Option<(String, PathBuf)> {
    let id = lock(&app.state::<AppState>().active_repo).clone()?;
    let path = repo_path(app, &id)?;
    Some((id, path))
}

/// From `open_repo`'s watcher callback (Worktree and Refs bursts alike).
pub fn on_watcher_burst(app: &tauri::AppHandle, repo_id: &str) {
    let (Some(deps), Some(path)) = (app.try_state::<DepsState>(), repo_path(app, repo_id)) else {
        return;
    };
    super::on_fs_change(&deps, repo_id, path);
}

/// From `set_active_repo`, with the previous focused id.
pub fn on_active_set(app: &tauri::AppHandle, old: Option<&str>, new: Option<&str>) {
    let Some(deps) = app.try_state::<DepsState>() else {
        return;
    };
    let new = new.and_then(|id| Some((id, repo_path(app, id)?)));
    super::on_active_changed(&deps, old, new);
}

/// From `close_repo`.
pub fn on_closed(app: &tauri::AppHandle, repo_id: &str) {
    if let Some(deps) = app.try_state::<DepsState>() {
        super::on_repo_closed(&deps, repo_id);
    }
}

/// From `run_scheduler`'s loop, next to `tick_once` (§6.1).
pub fn scheduler_tick(app: &tauri::AppHandle, now_ms: i64) {
    if let Some(deps) = app.try_state::<DepsState>() {
        super::tick(&deps, active(app), now_ms);
    }
}

/// From `set_ui_settings` after the patch is persisted (§8). Runs the
/// (possibly blocking) `apply_config` off the async executor.
pub async fn apply_settings(app: &tauri::AppHandle, cfg: DepTracker) {
    let Some(deps) = app.try_state::<DepsState>() else {
        return;
    };
    let deps = deps.inner().clone();
    if *lock(&deps.cfg) == cfg {
        return; // nothing changed: no blocking task at all
    }
    let open = open_repo_ids(app);
    let _ =
        tauri::async_runtime::spawn_blocking(move || super::apply_config(&deps, cfg, &open)).await;
}
