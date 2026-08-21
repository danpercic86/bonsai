//! App settings persistence (P1 contract §3.1).
//!
//! Hand-rolled `settings.json` under the app config dir — no `tauri-plugin-store`
//! (one tiny struct, no new capability surface). All file functions are
//! path-parameterized so they stay unit-testable without an `AppHandle`; only
//! [`settings_file`] touches Tauri.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

use bonsai_core::error::AppError;

mod clamp;
mod forge_accounts;
mod forge_hosts;
mod prefs;

pub use clamp::*;
pub use forge_accounts::*;
pub use forge_hosts::*;
pub use prefs::*;

/// Serializes every load→mutate→save cycle in this process (audit §2.3).
///
/// `save_to` is atomic on its own, but two concurrent [`update`] cycles on
/// different blocking threads (e.g. a pane-width drag save racing the MCP
/// token persist) would each load, mutate their own field, and rename — the
/// last rename silently reverting the other's field. Pure reads (`load_from`)
/// stay lock-free: they never write, and a torn read is impossible through the
/// atomic rename.
static SETTINGS_IO: Mutex<()> = Mutex::new(());

pub const MAX_RECENT_REPOS: usize = 10;
pub const SETTINGS_VERSION: u32 = 1;

/// One recently-opened repository.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecentRepo {
    /// Absolute workdir path as reported by `read_repo_info` (canonical root).
    pub path: String,
    /// Seconds since epoch (UTC) of the last successful open.
    pub last_opened: i64,
}

/// One named identity profile (P44). Global app setting; applied to a repo's
/// Local git config on demand. `id` is a stable frontend-generated UUID.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IdentityProfile {
    /// Stable id (frontend-generated `crypto.randomUUID()`); never reused.
    pub id: String,
    /// Display label, e.g. "Work". Empty/duplicate allowed but discouraged
    /// (frontend soft-validates non-empty).
    pub label: String,
    pub user_name: String,
    pub user_email: String,
    /// Optional `user.signingkey`. None/empty ⇒ not written on apply.
    pub signing_key: Option<String>,
}

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
    /// Open tabs, in display order (repoIds == canonical workdir paths).
    /// Additive (P3e §6.1); a legacy file without this key loads as empty.
    pub open_repos: Vec<String>,
    /// The active tab's repoId; `None` ⇒ activate the first still-openable one.
    /// Additive (P3e §6.1); a legacy file without this key loads as `None`.
    pub active_repo: Option<String>,
    /// Auto-fetch preference (P11). Additive `#[serde(default)]`; a legacy file
    /// without this key loads with `AutoFetch::default()`.
    pub auto_fetch: AutoFetch,
    /// Health-refresh background job (P30). Additive `#[serde(default)]`; a
    /// legacy file without this key loads with `HealthRefresh::default()`.
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
    /// P49: terminal launch command template (`{path}` placeholder). Empty ⇒
    /// per-OS auto-detect (see `bonsai_core::external`). Additive
    /// `#[serde(default)]` ⇒ a pre-P49 file loads `""`.
    pub terminal_command: String,
    /// P49: editor launch command template (`{path}` placeholder). Empty ⇒
    /// auto-detect the VS Code family. Additive `#[serde(default)]` ⇒ a pre-P49
    /// file loads `""`.
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
            terminal_command: String::new(),
            editor_command: String::new(),
            ai_idle_timeout_secs: AI_IDLE_TIMEOUT_DEFAULT,
            // 0 = unbounded (locked user decision).
            ai_hard_cap_secs: 0,
            ai_max_turns: bonsai_core::ai::DEFAULT_MAX_TURNS,
            ai_stream_log: true,
            ai_include_partial_messages: false,
            ai_conflict_tools: AiConflictTools::default(),
            ai_bulk_max_bytes: AI_BULK_MAX_BYTES_DEFAULT,
            // 0.0 = no `--max-budget-usd` flag at all (locked user decision).
            ai_max_budget_usd: 0.0,
            ai_dock_height: AI_DOCK_HEIGHT_DEFAULT,
            ai_dock_collapsed: false,
        }
    }
}

/// Loads settings from `file`. Missing file, unreadable file, or unparseable
/// JSON all yield `Settings::default()` — settings are best-effort and this
/// NEVER errors (P1 contract §3.1).
pub fn load_from(file: &Path) -> Settings {
    let mut s: Settings = match std::fs::read_to_string(file) {
        Ok(text) => serde_json::from_str(&text).unwrap_or_default(),
        Err(_) => Settings::default(),
    };
    // Defends against a hand-edited or future-version file with out-of-range
    // values (contract §2.1).
    s.pane_widths = clamp_pane_widths(s.pane_widths);
    s.auto_fetch = clamp_auto_fetch(s.auto_fetch);
    s.health_refresh = clamp_health_refresh(s.health_refresh);
    s.graph = clamp_graph_prefs(s.graph);
    clamp_ai_settings(&mut s);
    // P80: lazy P79→P80 migration — pure/in-memory so every read sees the P80
    // shape; the write is deferred to the next `update`/`update_if` (never
    // write-amplify a pure read).
    let _ = migrate_forge_hosts_to_accounts(&mut s);
    s
}

/// Loads, mutates, and saves settings as ONE serialized transaction (audit
/// §2.3): the process-wide [`SETTINGS_IO`] lock is held across the whole
/// load→mutate→save cycle so concurrent writers can never revert each other's
/// fields. ALL read-modify-write callers must go through this; `load_from`
/// alone remains fine for pure reads. Returns the saved settings so callers
/// can derive their response without a second (unlocked) read.
pub fn update(file: &Path, mutate: impl FnOnce(&mut Settings)) -> Result<Settings, AppError> {
    // Poison recovery: a panicking mutator leaves the settings FILE untouched
    // (the save never ran), so later updates are safe to proceed.
    let _io = SETTINGS_IO
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let mut s = load_from(file);
    mutate(&mut s);
    save_to(file, &s)?;
    Ok(s)
}

/// Like [`update`], but skips the (temp-write + atomic-rename) save when the
/// mutator reports no change by returning `false`. For hot read paths that only
/// *occasionally* mutate — e.g. the P79 lazy backfill on `forge_repo_context`,
/// which would otherwise rewrite `settings.json` on every panel open. Holds the
/// same [`SETTINGS_IO`] lock across the whole cycle.
pub fn update_if(
    file: &Path,
    mutate: impl FnOnce(&mut Settings) -> bool,
) -> Result<Settings, AppError> {
    let _io = SETTINGS_IO
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let mut s = load_from(file);
    if mutate(&mut s) {
        save_to(file, &s)?;
    }
    Ok(s)
}

/// Saves settings to `file` atomically: creates parent dirs, writes pretty
/// JSON to a uniquely-named `<file>.<pid>.<n>.tmp` (same volume), then renames
/// over `file`. On Windows `std::fs::rename` replaces an existing destination
/// file. The tmp name carries the process id + a process-local counter so
/// concurrent PROCESSES (the in-process serialization is [`update`]'s job)
/// can never collide on one tmp path (audit §2.3).
pub fn save_to(file: &Path, s: &Settings) -> Result<(), AppError> {
    if let Some(parent) = file.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| AppError::Io(format!("create settings dir {}: {e}", parent.display())))?;
    }
    let json = serde_json::to_string_pretty(s)
        .map_err(|e| AppError::Io(format!("serialize settings: {e}")))?;

    static TMP_SEQ: AtomicU64 = AtomicU64::new(0);
    let mut tmp_name = file.as_os_str().to_owned();
    tmp_name.push(format!(
        ".{}.{}.tmp",
        std::process::id(),
        TMP_SEQ.fetch_add(1, Ordering::Relaxed)
    ));
    let tmp = PathBuf::from(tmp_name);

    std::fs::write(&tmp, json)
        .map_err(|e| AppError::Io(format!("write {}: {e}", tmp.display())))?;
    std::fs::rename(&tmp, file).map_err(|e| {
        // Best-effort cleanup so a failed rename doesn't leave the tmp behind.
        let _ = std::fs::remove_file(&tmp);
        AppError::Io(format!(
            "rename {} -> {}: {e}",
            tmp.display(),
            file.display()
        ))
    })?;
    Ok(())
}

/// `<app_config_dir>/settings.json` (resolves under `%APPDATA%/com.bonsai.app`
/// on Windows).
pub fn settings_file(app: &tauri::AppHandle) -> Result<PathBuf, AppError> {
    use tauri::Manager;
    let dir = app
        .path()
        .app_config_dir()
        .map_err(|e| AppError::Other(format!("cannot resolve app config dir: {e}")))?;
    Ok(dir.join("settings.json"))
}

/// Upserts `path` at the front of the recents list, stamping `last_opened`,
/// and truncating to [`MAX_RECENT_REPOS`].
///
/// Dedupe goes through [`crate::commands::same_repo_path`] — the same
/// `fs::canonicalize` compare the open-repo scan uses — so the recents list and
/// the repo-registry agree on what "the same repo" is. The previous
/// `eq_ignore_ascii_case` was wrong in BOTH directions: it missed non-ASCII
/// case variants and 8.3 short names on Windows, and on a case-SENSITIVE
/// filesystem (ext4) it merged `/home/u/Repo` and `/home/u/repo`, which are two
/// genuinely different repositories there.
///
/// `same_repo_path` falls back to the old ASCII-case-insensitive compare
/// whenever either side cannot be canonicalized (a recents entry whose folder
/// was deleted or is on a detached drive), so no entry is ever silently dropped
/// and the previous behaviour is preserved for unresolvable paths. This is the
/// one non-pure step in this module; everything it needs is still injectable in
/// tests because unresolvable temp paths take the string-compare branch.
pub fn record_recent(s: &mut Settings, path: &str, now: i64) {
    s.recent_repos
        .retain(|r| !crate::commands::same_repo_path(&r.path, path));
    s.recent_repos.insert(
        0,
        RecentRepo {
            path: path.to_string(),
            last_opened: now,
        },
    );
    s.recent_repos.truncate(MAX_RECENT_REPOS);
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
