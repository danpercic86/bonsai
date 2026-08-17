//! `ui_settings` commands — split from the former monolithic `commands.rs`.

use super::shared::*;

/// Combined UI settings surfaced to the frontend (P2 contract §2.2).
///
/// NOT `Copy` since P44 added the `profiles` `Vec` — clone the Vec into the
/// returned value in `get`/`set_ui_settings`.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UiSettings {
    pub theme: ThemeChoice,
    pub pane_widths: PaneWidths,
    pub list_view: ListView,
    /// P67: right-panel vertical density; display-only, patches independently.
    pub panel_density: PanelDensity,
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
    /// P43: first-run onboarding has been shown+dismissed.
    pub onboarding_seen: bool,
    /// P42 D4: auto-check for updates on launch (default false).
    pub auto_check_updates: bool,
    /// P44: named identity profiles (global).
    pub profiles: Vec<IdentityProfile>,
    /// P49: terminal launch command template (`{path}` placeholder). Empty ⇒
    /// per-OS auto-detect.
    pub terminal_command: String,
    /// P49: editor launch command template. Empty ⇒ auto-detect VS Code.
    pub editor_command: String,
}

/// Partial patch for `set_ui_settings` — only `Some(..)` fields are applied
/// (P2 contract §2.2).
///
/// NOT `Copy` since P44 added the `profiles` `Vec`. `apply_patch` already takes
/// this by value.
#[derive(Debug, Clone, PartialEq, Default, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UiSettingsPatch {
    pub theme: Option<ThemeChoice>,
    pub pane_widths: Option<PaneWidths>,
    pub list_view: Option<ListView>,
    /// P67: right-panel density (P67c). Patches independently of `list_view`
    /// and `graph`; NOT clamped (no numeric range).
    pub panel_density: Option<PanelDensity>,
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
    /// P43: first-run onboarding seen flag; patches independently.
    pub onboarding_seen: Option<bool>,
    /// P42 D4: auto-check-updates-on-launch flag; patches independently.
    pub auto_check_updates: Option<bool>,
    /// P44: identity profiles — whole-array replace (like `pane_widths`); the
    /// frontend sends the entire list when any profile changes.
    pub profiles: Option<Vec<IdentityProfile>>,
    /// P49: terminal launch command template; patches independently.
    pub terminal_command: Option<String>,
    /// P49: editor launch command template; patches independently.
    pub editor_command: Option<String>,
}

/// Pure patch application: only `Some(..)` fields of `patch` mutate `s`; pane
/// widths are clamped on write. Extracted from `set_ui_settings` so its
/// partial-update semantics are unit-testable without a Tauri app
/// (P2a contract §3.4.3).
pub(crate) fn apply_patch(s: &mut settings::Settings, patch: UiSettingsPatch) {
    if let Some(theme) = patch.theme {
        s.theme = theme;
    }
    if let Some(pane_widths) = patch.pane_widths {
        s.pane_widths = clamp_pane_widths(pane_widths);
    }
    if let Some(list_view) = patch.list_view {
        s.list_view = list_view;
    }
    if let Some(panel_density) = patch.panel_density {
        s.panel_density = panel_density;
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
    if let Some(onboarding_seen) = patch.onboarding_seen {
        s.onboarding_seen = onboarding_seen;
    }
    if let Some(auto_check_updates) = patch.auto_check_updates {
        s.auto_check_updates = auto_check_updates;
    }
    if let Some(profiles) = patch.profiles {
        s.profiles = profiles;
    }
    if let Some(terminal_command) = patch.terminal_command {
        s.terminal_command = terminal_command;
    }
    if let Some(editor_command) = patch.editor_command {
        s.editor_command = editor_command;
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
            panel_density: s.panel_density,
            auto_fetch: s.auto_fetch,
            health_refresh: s.health_refresh,
            graph: s.graph,
            ai_enabled: s.ai_enabled,
            ai_conflict_autonomy: s.ai_conflict_autonomy,
            ai_consented: s.ai_consented,
            mcp_consented: s.mcp_consented,
            mcp_write_consented: s.mcp_write_consented,
            onboarding_seen: s.onboarding_seen,
            auto_check_updates: s.auto_check_updates,
            profiles: s.profiles.clone(),
            terminal_command: s.terminal_command.clone(),
            editor_command: s.editor_command.clone(),
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
        // Serialized load→mutate→save (audit §2.3) — never a bare load+save pair.
        let s = settings::update(&file, |s| apply_patch(s, patch))?;
        Ok(UiSettings {
            theme: s.theme,
            pane_widths: s.pane_widths,
            list_view: s.list_view,
            panel_density: s.panel_density,
            auto_fetch: s.auto_fetch,
            health_refresh: s.health_refresh,
            graph: s.graph,
            ai_enabled: s.ai_enabled,
            ai_conflict_autonomy: s.ai_conflict_autonomy,
            ai_consented: s.ai_consented,
            mcp_consented: s.mcp_consented,
            mcp_write_consented: s.mcp_write_consented,
            onboarding_seen: s.onboarding_seen,
            auto_check_updates: s.auto_check_updates,
            profiles: s.profiles.clone(),
            terminal_command: s.terminal_command.clone(),
            editor_command: s.editor_command.clone(),
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
        // Serialized load→mutate→save (audit §2.3).
        settings::update(&file, |s| {
            s.open_repos = session.open_repos;
            s.active_repo = session.active_repo;
        })
        .map(|_| ())
    })
    .await
    .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}
