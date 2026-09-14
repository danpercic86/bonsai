//! `ui_settings` commands — split from the former monolithic `commands.rs`.

use super::shared::*;

use bonsai_core::tools::{self, ToolKind};

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
    /// P80 D1: which commit button is emphasized in the Working tab.
    pub primary_commit_action: PrimaryCommitAction,
    /// Spec-002: commit-graph visual style (default Standard).
    pub graph_style: GraphStyle,
    /// Spec-002: seasonal accent for the Bonsai style (default Living).
    pub graph_season: GraphSeason,
    /// Spec-003: first-parent graph declutter toggle (default false).
    pub graph_first_parent: bool,
    /// Spec-004: fold-linear-runs graph toggle (default false).
    pub graph_fold_linear: bool,
    /// Spec-005: always-show overview rail (default false).
    pub graph_minimap_always_show: bool,
    /// Spec-006: graph edge/ring coloring (default Lane).
    pub graph_color_mode: GraphColorMode,
    /// Spec-003: persisted graph ref-filter INTENT (default None). Opaque to
    /// the backend; the frontend derives the wire whitelist from it.
    pub graph_ref_filter: Option<GraphRefFilter>,
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
    /// P112 §5.1: the selected terminal — `""` (auto ladder), a catalog id, or
    /// the pseudo-id `"custom"`. **There is deliberately no `customTerminalPath`
    /// / `customEditorPath` here or on [`UiSettingsPatch`]:** the browsed path
    /// travels outbound only, once, as `DetectedTool.detail`, so a
    /// renderer-written program path is unrepresentable rather than rejected
    /// (§5.4 — and `set_ui_settings` has no field that could carry one).
    pub terminal_tool: String,
    /// P112 §5.1: the selected editor; same rules as [`Self::terminal_tool`].
    pub editor_tool: String,
    // ---- P68 §8.3: streaming AI-run knobs. Each patches independently; see
    // `settings::Settings` for the per-field semantics and the two LOCKED
    // defaults (`aiHardCapSecs = 0` unbounded, `aiMaxBudgetUsd = 0.0` no cap).
    pub ai_idle_timeout_secs: u32,
    pub ai_hard_cap_secs: u32,
    pub ai_max_turns: u32,
    pub ai_stream_log: bool,
    pub ai_include_partial_messages: bool,
    pub ai_conflict_tools: AiConflictTools,
    pub ai_bulk_max_bytes: u32,
    pub ai_max_budget_usd: f64,
    pub ai_dock_height: u32,
    pub ai_dock_collapsed: bool,
    /// P91 §10: Dev-mode / observability settings (whole-struct, like
    /// `auto_fetch`).
    pub dev: DevSettings,
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
    /// P80 D1: primary commit action; patches independently.
    pub primary_commit_action: Option<PrimaryCommitAction>,
    /// Spec-002: commit-graph style + season; each patches independently.
    pub graph_style: Option<GraphStyle>,
    pub graph_season: Option<GraphSeason>,
    /// Spec-003: first-parent declutter toggle; patches independently.
    pub graph_first_parent: Option<bool>,
    /// Spec-004: fold-linear-runs toggle; patches independently.
    pub graph_fold_linear: Option<bool>,
    /// Spec-005: always-show overview rail; patches independently.
    pub graph_minimap_always_show: Option<bool>,
    /// Spec-006: graph edge/ring coloring; patches independently.
    pub graph_color_mode: Option<GraphColorMode>,
    /// Spec-003: graph ref-filter intent. Double-option so an explicit `null`
    /// (clear the filter) is distinguishable from an ABSENT key (leave
    /// unchanged): missing → `None`, `null` → `Some(None)`, a value →
    /// `Some(Some(v))`. The field-level `default` is mandatory: with a custom
    /// `deserialize_with`, a missing key would otherwise be a hard error.
    #[serde(default, deserialize_with = "double_option")]
    pub graph_ref_filter: Option<Option<GraphRefFilter>>,
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
    /// P112 §5.1: the selected terminal id; patches independently and is
    /// COERCED on write (§5.2) — anything that is not a catalog id, or
    /// `"custom"` without a stored browsed path, becomes `""`.
    pub terminal_tool: Option<String>,
    /// P112 §5.1: the selected editor id; same coercion.
    pub editor_tool: Option<String>,
    /// P68 §8.3: the ten streaming AI-run knobs, each patching independently of
    /// `graph` / `listView` / `panelDensity` and clamped on write by
    /// `clamp_ai_settings`.
    pub ai_idle_timeout_secs: Option<u32>,
    pub ai_hard_cap_secs: Option<u32>,
    pub ai_max_turns: Option<u32>,
    pub ai_stream_log: Option<bool>,
    pub ai_include_partial_messages: Option<bool>,
    pub ai_conflict_tools: Option<AiConflictTools>,
    pub ai_bulk_max_bytes: Option<u32>,
    pub ai_max_budget_usd: Option<f64>,
    pub ai_dock_height: Option<u32>,
    pub ai_dock_collapsed: Option<bool>,
    /// P91 §10: whole-struct patch — the frontend sends the entire `dev` object
    /// when any sub-field changes (the `auto_fetch` / `health_refresh`
    /// precedent). NOT clamped: no field has a numeric range.
    pub dev: Option<DevSettings>,
}

/// Distinguishes an ABSENT patch key (don't touch) from an explicit `null`
/// (clear): missing → `None` (via the field default), `null` → `Some(None)`,
/// a value → `Some(Some(v))`. A plain `Option<Option<T>>` can't — serde folds
/// `null` and missing together at the outer level.
fn double_option<'de, T, D>(de: D) -> Result<Option<Option<T>>, D::Error>
where
    T: serde::Deserialize<'de>,
    D: serde::Deserializer<'de>,
{
    serde::Deserialize::deserialize(de).map(Some)
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
    if let Some(primary_commit_action) = patch.primary_commit_action {
        s.primary_commit_action = primary_commit_action;
    }
    if let Some(graph_style) = patch.graph_style {
        s.graph_style = graph_style;
    }
    if let Some(graph_season) = patch.graph_season {
        s.graph_season = graph_season;
    }
    if let Some(graph_first_parent) = patch.graph_first_parent {
        s.graph_first_parent = graph_first_parent;
    }
    if let Some(graph_fold_linear) = patch.graph_fold_linear {
        s.graph_fold_linear = graph_fold_linear;
    }
    if let Some(graph_minimap_always_show) = patch.graph_minimap_always_show {
        s.graph_minimap_always_show = graph_minimap_always_show;
    }
    if let Some(graph_color_mode) = patch.graph_color_mode {
        s.graph_color_mode = graph_color_mode;
    }
    // Double-option: `Some(None)` (an explicit wire `null`) CLEARS the filter.
    if let Some(graph_ref_filter) = patch.graph_ref_filter {
        s.graph_ref_filter = graph_ref_filter;
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
    // P112 §5.2 — write-time COERCION, not validation: the settings writer
    // merges pending keys into one patch and re-queues on failure, so a
    // rejection here would wedge every later settings write. A pure catalog
    // lookup cannot fail, so "the renderer wrote garbage" degrades to "nothing
    // is selected" (⇒ the auto ladder). `"custom"` selects only a path the user
    // already browsed to, which is why the stored path decides.
    if let Some(v) = patch.terminal_tool {
        let has = !s.custom_terminal_path.is_empty();
        s.terminal_tool = tools::coerce_tool_id(&v, ToolKind::Terminal, has);
    }
    if let Some(v) = patch.editor_tool {
        let has = !s.custom_editor_path.is_empty();
        s.editor_tool = tools::coerce_tool_id(&v, ToolKind::Editor, has);
    }
    // P68 §8.3. Assigned raw, then clamped ONCE at the end (mirrors
    // `clamp_pane_widths` on write, but for ten top-level scalars): the clamp is
    // idempotent, so running it unconditionally cannot disturb an untouched field.
    if let Some(v) = patch.ai_idle_timeout_secs {
        s.ai_idle_timeout_secs = v;
    }
    if let Some(v) = patch.ai_hard_cap_secs {
        s.ai_hard_cap_secs = v;
    }
    if let Some(v) = patch.ai_max_turns {
        s.ai_max_turns = v;
    }
    if let Some(v) = patch.ai_stream_log {
        s.ai_stream_log = v;
    }
    if let Some(v) = patch.ai_include_partial_messages {
        s.ai_include_partial_messages = v;
    }
    if let Some(v) = patch.ai_conflict_tools {
        s.ai_conflict_tools = v;
    }
    if let Some(v) = patch.ai_bulk_max_bytes {
        s.ai_bulk_max_bytes = v;
    }
    if let Some(v) = patch.ai_max_budget_usd {
        s.ai_max_budget_usd = v;
    }
    if let Some(v) = patch.ai_dock_height {
        s.ai_dock_height = v;
    }
    if let Some(v) = patch.ai_dock_collapsed {
        s.ai_dock_collapsed = v;
    }
    // P91 §10: whole-struct, unclamped. The sink is (re)started by the caller
    // AFTER the save, never here — `apply_patch` is pure by contract.
    if let Some(dev) = patch.dev {
        s.dev = dev;
    }
    clamp_ai_settings(s);
}

/// The `Settings` → `UiSettings` projection, in ONE place.
///
/// Extracted in P68b because it was duplicated verbatim in `get_ui_settings` and
/// `set_ui_settings`: with 27 fields, adding one to a single copy compiles fine and
/// then returns a stale value from exactly one of the two commands. One builder
/// makes that class of bug impossible.
///
/// `pub(crate)` (P69 §3.2) so the defaults-parity test in
/// `settings_defaults_parity_tests.rs` can serialise the default projection: there
/// is no `UiSettings::default()`, and this is the ONE place that builds one.
pub(crate) fn ui_settings_of(s: &settings::Settings) -> UiSettings {
    UiSettings {
        theme: s.theme,
        pane_widths: s.pane_widths,
        list_view: s.list_view,
        panel_density: s.panel_density,
        primary_commit_action: s.primary_commit_action,
        graph_style: s.graph_style,
        graph_season: s.graph_season,
        graph_first_parent: s.graph_first_parent,
        graph_fold_linear: s.graph_fold_linear,
        graph_minimap_always_show: s.graph_minimap_always_show,
        graph_color_mode: s.graph_color_mode,
        graph_ref_filter: s.graph_ref_filter.clone(),
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
        terminal_tool: s.terminal_tool.clone(),
        editor_tool: s.editor_tool.clone(),
        ai_idle_timeout_secs: s.ai_idle_timeout_secs,
        ai_hard_cap_secs: s.ai_hard_cap_secs,
        ai_max_turns: s.ai_max_turns,
        ai_stream_log: s.ai_stream_log,
        ai_include_partial_messages: s.ai_include_partial_messages,
        ai_conflict_tools: s.ai_conflict_tools,
        ai_bulk_max_bytes: s.ai_bulk_max_bytes,
        ai_max_budget_usd: s.ai_max_budget_usd,
        ai_dock_height: s.ai_dock_height,
        ai_dock_collapsed: s.ai_dock_collapsed,
        dev: s.dev,
    }
}

/// Current UI settings (theme + pane widths). Never rejects for a
/// missing/corrupt settings file (same as `get_recent_repos`); only
/// settings-path resolution can error.
#[tauri::command]
pub async fn get_ui_settings(app: tauri::AppHandle) -> Result<UiSettings, AppError> {
    let file = settings::settings_file(&app)?;
    tauri::async_runtime::spawn_blocking(move || ui_settings_of(&settings::load_from(&file)))
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
        Ok(ui_settings_of(&s))
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
    // P91 §10: Dev mode takes effect IMMEDIATELY (no restart) — start, stop or
    // restart the sink to match what was just persisted. `spawn_blocking`
    // because it opens a file and prunes the folder; the `AppHandle` (not a
    // `State` borrow) is what lets that work off-thread. A failure here is
    // NON-FATAL and must not undo a saved setting: the user's preference is
    // stored, the UI reports the state via `log_session_info`, and the next
    // toggle retries.
    let obs_handle = app.clone();
    let dev = ui.dev;
    if let Err(e) = tauri::async_runtime::spawn_blocking(move || {
        use tauri::Manager;
        let obs_state = obs_handle.state::<crate::obs::ObsState>();
        crate::obs::apply_dev_settings(&obs_handle, &obs_state, &dev)
    })
    .await
    .map_err(|e| AppError::Other(format!("task join error: {e}")))?
    {
        eprintln!("bonsai: cannot apply dev-mode logging settings (non-fatal): {e}");
    }
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
