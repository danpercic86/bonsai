//! App settings persistence (P1 contract §3.1).
//!
//! Hand-rolled `settings.json` under the app config dir — no `tauri-plugin-store`
//! (one tiny struct, no new capability surface). All file functions are
//! path-parameterized so they stay unit-testable without an `AppHandle`; only
//! [`settings_file`] touches Tauri.

// `settings_tests.rs` reaches `Path` / `PathBuf` through `use super::*`; the
// imports moved to `settings/io.rs` with the I/O functions, so keep them
// visible to the test children here.
#[cfg(test)]
use std::path::{Path, PathBuf};

mod clamp;
mod external_tools;
mod forge_accounts;
mod forge_hosts;
mod identity;
mod io;
mod prefs;
pub mod prune;
mod recent;

pub use clamp::*;
pub use external_tools::*;
pub use forge_accounts::*;
pub use forge_hosts::*;
pub use identity::*;
pub use io::*;
pub use prefs::*;
pub use prune::{classify_all, collect_paths, note_prune, prune_stale_paths};
pub use prune::{resolve_candidate_hosts, HostResolver, PathFacts, PathState, PruneReport};
pub use recent::*;

// Settings file I/O (`load_from` / `update` / `update_if` / `save_to` /
// `settings_file`, plus the `SETTINGS_IO` lock) lives in `settings/io.rs`;
// `RecentRepo`, `MAX_RECENT_REPOS`, `record_recent` and the hooks-ack helpers
// live in `settings/recent.rs` (size split), re-exported above.

pub const SETTINGS_VERSION: u32 = 1;

// P44/P82 identity-profile types (`ProfileColor`, `IdentityProfile`) live in
// `settings/identity.rs` (spec-006 size split), re-exported above.

/// On-disk settings wire format:
/// `{ "version": 1, "recentRepos": [ { "path": "...", "lastOpened": 0 } ],
///    "theme": "dark", "paneWidths": { "sidebar": 240, "rightPanel": 380 },
///    "listView": "tree", "aiEnabled": true, "aiConflictAutonomy":
///    "proposeReview", "aiConsented": false }`.
///
/// `SETTINGS_VERSION` stays `1`: `theme`, `pane_widths`, `list_view`,
/// `panel_density`, `open_repos`, `active_repo`, `auto_fetch`, `graph`,
/// `ai_enabled`, `ai_conflict_autonomy`, `ai_consented`, and the P68 streaming-AI
/// run knobs (`ai_idle_timeout_secs`, `ai_hard_cap_secs`, `ai_max_turns`,
/// `ai_stream_log`, `ai_include_partial_messages`, `ai_conflict_tools`,
/// `ai_bulk_max_bytes`, `ai_max_budget_usd`, `ai_dock_height`,
/// `ai_dock_collapsed`) are all additive
/// `#[serde(default)]` fields
/// (on the whole struct already, via the container-level `default`) — an old
/// settings.json containing only `recentRepos` deserializes fine, missing
/// fields fall back to their type defaults. No migration code is needed. A
/// future genuine breaking change (e.g. renaming/removing a field with no safe
/// default) IS when a version bump becomes necessary — this precedent documents
/// the bar for that.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub version: u32,
    pub recent_repos: Vec<RecentRepo>,
    pub theme: ThemeChoice,
    pub pane_widths: PaneWidths,
    pub list_view: ListView,
    /// P67: right-panel vertical density. Additive `#[serde(default)]` (via the
    /// container-level `default`); a pre-P67 settings.json without this key loads
    /// `PanelDensity::default()` (Cozy). No version bump — meets the documented
    /// bar above. NOT clamped (no numeric range; `clamp_graph_prefs` untouched).
    pub panel_density: PanelDensity,
    /// P80 D1: which commit button is emphasized in the Working tab. Additive
    /// `#[serde(default)]` (via the container-level `default`); a pre-P80
    /// settings.json without this key loads `PrimaryCommitAction::default()`
    /// (Commit). Pure UI preference; NOT clamped.
    pub primary_commit_action: PrimaryCommitAction,
    /// Spec-002: commit-graph visual style. Additive `#[serde(default)]` (via the
    /// container-level `default`); a pre-spec-002 settings.json without this key
    /// loads `GraphStyle::default()` (Standard). Pure UI preference; NOT clamped.
    pub graph_style: GraphStyle,
    /// Spec-002: seasonal accent for the Bonsai style. Additive `#[serde(default)]`;
    /// a legacy file loads `GraphSeason::default()` (Living). NOT clamped.
    pub graph_season: GraphSeason,
    /// Spec-003: first-parent graph declutter toggle. Additive `#[serde(default)]`
    /// (via the container-level `default`); a legacy file loads `false`. NOT clamped.
    pub graph_first_parent: bool,
    /// Spec-004: fold-linear-runs graph toggle. Additive `#[serde(default)]`
    /// (via the container-level `default`); a legacy file loads `false`. NOT
    /// clamped. Exact `graph_first_parent` precedent.
    pub graph_fold_linear: bool,
    /// Spec-005: always-show overview rail. Additive default; legacy → false.
    pub graph_minimap_always_show: bool,
    /// Spec-006: graph edge/ring coloring. Additive default; legacy → Lane.
    pub graph_color_mode: GraphColorMode,
    /// Spec-003: persisted graph ref-filter INTENT (solo/hide + full ref names).
    /// Opaque to the backend — the frontend derives the wire whitelist. Additive
    /// `#[serde(default)]`; a legacy file loads `None`. NOT clamped.
    pub graph_ref_filter: Option<GraphRefFilter>,
    /// Open tabs, in display order (repoIds == canonical workdir paths).
    /// Additive (P3e §6.1); a legacy file without this key loads as empty.
    pub open_repos: Vec<String>,
    /// The active tab's repoId; `None` ⇒ activate the first still-openable one.
    /// Additive (P3e §6.1); a legacy file without this key loads as `None`.
    pub active_repo: Option<String>,
    /// Auto-fetch preference (P11). Additive `#[serde(default)]`; a legacy file
    /// without this key loads with `AutoFetch::default()`.
    pub auto_fetch: AutoFetch,
    /// Health-refresh job (P30). Additive `#[serde(default)]`; legacy ⇒ default.
    pub health_refresh: HealthRefresh,
    /// Graph geometry knobs (P11). Additive `#[serde(default)]`; a legacy file
    /// without this key loads with `GraphPrefs::default()`.
    pub graph: GraphPrefs,
    /// AI features master toggle (P13). Defaults `true`, but the consent gate
    /// (`ai_consented`) is what actually unlocks the feature. Additive
    /// `#[serde(default)]`; a legacy file without this key loads as `true`.
    pub ai_enabled: bool,
    /// AI conflict-resolution autonomy (P13). Additive `#[serde(default)]`; a
    /// legacy file without this key loads with `AiAutonomy::default()`
    /// (ProposeReview).
    pub ai_conflict_autonomy: AiAutonomy,
    /// One-time consent to send repo content to the local Claude CLI (P13).
    /// Defaults `false`; additive `#[serde(default)]`; a legacy file without
    /// this key loads as `false`.
    pub ai_consented: bool,
    /// Embedded MCP server enabled (P16). Default `false`. Auto-started at
    /// launch ONLY when this persisted flag is true (P44a — the user opted in
    /// previously); still never started without that prior explicit opt-in.
    pub mcp_enabled: bool,
    /// Embedded MCP write-gate (P16). Default `false`. P16b forces the running
    /// server read-only regardless; P16c wires this to (re)register write tools.
    pub mcp_allow_write: bool,
    /// One-time consent to expose open repos to an external MCP client for
    /// READING (P16). Defaults `false`; additive `#[serde(default)]`.
    pub mcp_consented: bool,
    /// One-time consent to let an external MCP client MODIFY open repos (P16c).
    /// A strictly stronger grant than `mcp_consented` (read) — kept as its own
    /// flag so enabling write requires its own explicit confirmation and a
    /// read-only consent never silently implies write. Defaults `false`;
    /// additive `#[serde(default)]`.
    pub mcp_write_consented: bool,
    /// Persisted bound port for the embedded MCP server (P16 §8.5, D-4).
    /// `None` until first enable; preferred on later runs (ephemeral fallback).
    pub mcp_port: Option<u16>,
    /// Persisted bearer token for the embedded MCP server (P16 §8.2, D-4).
    /// Generated on first enable and reused across runs so the user's
    /// `claude mcp add` line keeps working. `None` until first enable.
    pub mcp_token: Option<String>,
    /// P43: first-run onboarding shown+dismissed. Additive `#[serde(default)]`;
    /// a legacy settings.json without this key loads as `false` (⇒ show once).
    pub onboarding_seen: bool,
    /// P42 D4: auto-check for updates on launch. Default `false` (privacy — no
    /// surprise outbound call before opt-in). Additive `#[serde(default)]`; a
    /// legacy file without this key loads as `false`.
    pub auto_check_updates: bool,
    /// P44: named identity profiles (global). Additive `#[serde(default)]`; a
    /// legacy file without this key loads as an empty Vec.
    pub profiles: Vec<IdentityProfile>,
    /// P79: forge hosts with a stored PAT (known-hosts index; the keychain is the
    /// source of record for the token). Additive `#[serde(default)]` ⇒ a pre-P79
    /// file loads `[]`. Stores only host + kind + optional last-known login,
    /// NEVER a token.
    pub forge_hosts: Vec<ForgeHostRecord>,
    /// P80: multi-account forge model. All additive `#[serde(default)]` ⇒ a
    /// pre-P80 file loads `[]` and is populated by `migrate_forge_hosts_to_accounts`
    /// on the next load. NEVER holds a token.
    pub forge_accounts: Vec<ForgeAccountRecord>,
    /// P80: per-host default account (repos inherit it).
    pub forge_host_defaults: Vec<ForgeHostDefault>,
    /// P80: per-repo pinned account overrides (keyed by canonical workdir path).
    pub repo_forge_overrides: Vec<RepoForgeOverride>,
    /// Repos whose one-time git-hook execution disclosure the user has
    /// acknowledged (canonical workdir paths, deduped via
    /// [`crate::commands::same_repo_path`] — mirrors `repo_forge_overrides`).
    /// Additive `#[serde(default)]` (via the container-level `default`) ⇒ a
    /// pre-existing settings.json without this key loads `[]`. NO version bump.
    pub hooks_ack_repos: Vec<String>,
    // P112 §5.1 / §5.4 — the external-tool selection. The coercion rule, the
    // migration and the "unrepresentable, not rejected" rationale all live in
    // `settings/external_tools.rs`.
    /// Selected terminal: `""` ⇒ auto ladder, a catalog id, or the pseudo-id
    /// `"custom"`. Renderer-writable, but only ever as a LOOKUP KEY.
    pub terminal_tool: String,
    /// Selected editor; same rules as [`Self::terminal_tool`].
    pub editor_tool: String,
    /// The browsed terminal program (`""` = none). Written ONLY by
    /// `pick_external_tool`, from a path a native dialog the BACKEND opened
    /// returned; absent from `UiSettings` *and* `UiSettingsPatch`.
    pub custom_terminal_path: String,
    /// The browsed editor program; same rules as [`Self::custom_terminal_path`].
    pub custom_editor_path: String,
    /// LEGACY (P49 … 2026-09-11): P112 §5.3 migration input only, never written
    /// again — `skip_serializing` is what makes the migration idempotent.
    #[serde(default, skip_serializing)]
    pub terminal_command: String,
    /// LEGACY (P49 … 2026-09-11): see [`Self::terminal_command`].
    #[serde(default, skip_serializing)]
    pub editor_command: String,
    // ---- P68 §8.3: streaming AI-run knobs. All additive `#[serde(default)]`
    // (via the container-level `default`), all clamped by `clamp_ai_settings`, NO
    // version bump — a pre-P68 settings.json loads every one of them at its
    // default (asserted by `old_settings_file_without_ai_run_fields_loads_defaults`).
    /// Seconds with NO output from the CLI before a streaming run is killed.
    /// `0` disables the watchdog. Streaming has no wall-clock deadline by design
    /// (the user cancels instead — D3/D7), so this is the only automatic reaper.
    pub ai_idle_timeout_secs: u32,
    /// Absolute cap on one streaming run, in seconds. **`0` = UNBOUNDED, and that
    /// is the shipped default** (the locked user decision: no hard timeout +
    /// Cancel). Paused while the run awaits a human answer (D3).
    pub ai_hard_cap_secs: u32,
    /// Max `result` lines (turns) before a still-questioning model is failed
    /// (P68 §B rule 3). Shares `bonsai_core::ai::DEFAULT_MAX_TURNS` with
    /// `RunLimits::default()` rather than repeating the literal.
    pub ai_max_turns: u32,
    /// Stream the CLI's log lines to the dock. `false` suppresses `Log` events at
    /// the SOURCE, so turning it off costs no IPC (§8.3).
    pub ai_stream_log: bool,
    /// `--include-partial-messages`. Default OFF: the delta shape is unverified
    /// (spike §1.8) and unknown lines only degrade to `log`.
    pub ai_include_partial_messages: bool,
    /// Repo access for a conflict run (D10). Default `ReadOnly`.
    pub ai_conflict_tools: AiConflictTools,
    /// Payload byte cap that triggers batch SPLITTING for a bulk resolve (§6.3).
    /// Never a truncation point — a request too big for one payload is split into
    /// sequential batches, and a single file that alone exceeds it is reported as
    /// an individual failure.
    pub ai_bulk_max_bytes: u32,
    /// `--max-budget-usd`. **`0.0` = NO CAP, the shipped default** (the locked
    /// decision: opt-in, because a surprise mid-run stop is worse than a run the
    /// user can see and cancel). The flag is passed ONLY when > 0.
    pub ai_max_budget_usd: f64,
    /// Persisted px height of the AI activity dock (P68 §E, A8: a top-level
    /// field, NOT a member of `PaneWidths` — that struct is about widths).
    pub ai_dock_height: u32,
    /// Persisted collapsed state of the AI activity dock (P68 §E).
    pub ai_dock_collapsed: bool,
    /// P91 §10: Dev-mode / observability settings. Additive `#[serde(default)]`
    /// (via the container-level `default`) — a pre-P91 settings.json loads
    /// `DevSettings::default()` (Dev mode OFF, strict redaction). NOT clamped
    /// (no numeric range). Whole-struct patch, like `auto_fetch`.
    pub dev: DevSettings,
    /// P125 §2.2: dependency tracker (OFF, 12 h). Additive; legacy ⇒ default.
    pub dep_tracker: DepTracker,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            version: SETTINGS_VERSION,
            recent_repos: Vec::new(),
            theme: ThemeChoice::default(),
            pane_widths: PaneWidths::default(),
            list_view: ListView::default(),
            panel_density: PanelDensity::default(),
            primary_commit_action: PrimaryCommitAction::default(),
            graph_style: GraphStyle::default(),
            graph_season: GraphSeason::default(),
            graph_first_parent: false,
            graph_fold_linear: false,
            graph_minimap_always_show: false,
            graph_color_mode: GraphColorMode::default(),
            graph_ref_filter: None,
            open_repos: Vec::new(),
            active_repo: None,
            auto_fetch: AutoFetch::default(),
            health_refresh: HealthRefresh::default(),
            graph: GraphPrefs::default(),
            ai_enabled: true,
            ai_conflict_autonomy: AiAutonomy::default(),
            ai_consented: false,
            mcp_enabled: false,
            mcp_allow_write: false,
            mcp_consented: false,
            mcp_write_consented: false,
            mcp_port: None,
            mcp_token: None,
            onboarding_seen: false,
            auto_check_updates: false,
            profiles: Vec::new(),
            forge_hosts: Vec::new(),
            forge_accounts: Vec::new(),
            forge_host_defaults: Vec::new(),
            repo_forge_overrides: Vec::new(),
            hooks_ack_repos: Vec::new(),
            terminal_tool: String::new(),
            editor_tool: String::new(),
            custom_terminal_path: String::new(),
            custom_editor_path: String::new(),
            terminal_command: String::new(),
            editor_command: String::new(),
            ai_idle_timeout_secs: AI_IDLE_TIMEOUT_DEFAULT,
            ai_hard_cap_secs: 0, // 0 = unbounded (locked user decision).
            ai_max_turns: bonsai_core::ai::DEFAULT_MAX_TURNS,
            ai_stream_log: true,
            ai_include_partial_messages: false,
            ai_conflict_tools: AiConflictTools::default(),
            ai_bulk_max_bytes: AI_BULK_MAX_BYTES_DEFAULT,
            ai_max_budget_usd: 0.0, // 0.0 = no `--max-budget-usd` flag (locked decision).
            ai_dock_height: AI_DOCK_HEIGHT_DEFAULT,
            ai_dock_collapsed: false,
            dev: DevSettings::default(),
            dep_tracker: DepTracker::default(),
        }
    }
}

#[cfg(test)]
#[path = "settings_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "settings_ui_tests.rs"]
mod ui_tests;

#[cfg(test)]
#[path = "settings_prefs_tests.rs"]
mod prefs_tests;

#[cfg(test)]
#[path = "settings_ai_tests.rs"]
mod ai_tests;
