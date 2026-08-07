//! App settings persistence (P1 contract §3.1).
//!
//! Hand-rolled `settings.json` under the app config dir — no `tauri-plugin-store`
//! (one tiny struct, no new capability surface). All file functions are
//! path-parameterized so they stay unit-testable without an `AppHandle`; only
//! [`settings_file`] touches Tauri.

use std::path::{Path, PathBuf};

use bonsai_core::error::AppError;

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

/// Dark or light chrome (P2 contract §2.1). Lane colors are theme-invariant —
/// only chrome (`--bg-*`/`--text-*`/etc.) differs; this enum is purely a UI
/// preference with no effect on Git logic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ThemeChoice {
    #[default]
    Dark,
    Light,
}

/// Flat vs tree-grouped list rendering for sidebar refs and file lists
/// (P3b contract §2). Pure UI preference; display-only, no Git effect.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ListView {
    #[default]
    Tree,
    Flat,
}

/// AI conflict-resolution autonomy (P13). ProposeReview = user accepts before
/// anything is written/staged (default); AutoResolve = write+stage immediately,
/// user reviews the staged diff before commit_merge.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AiAutonomy {
    #[default]
    ProposeReview,
    AutoResolve,
}

/// Persisted sidebar/right-panel widths in px (P2 contract §2.1). Clamped to
/// documented sane bounds on BOTH read (`load_from`) and write (setter
/// commands) — this is the "persisted sanity" bound; the frontend additionally
/// applies a dynamic live-drag clamp against the current window width and the
/// graph pane's 480px floor, which is a deliberately separate check (contract
/// §2.5) not duplicated here.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PaneWidths {
    pub sidebar: u32,
    pub right_panel: u32,
}

impl Default for PaneWidths {
    fn default() -> Self {
        PaneWidths {
            sidebar: 240,
            right_panel: 380,
        }
    }
}

pub const SIDEBAR_MIN: u32 = 180;
pub const SIDEBAR_MAX: u32 = 480;
pub const RIGHT_PANEL_MIN: u32 = 280;
pub const RIGHT_PANEL_MAX: u32 = 640;

/// Clamps to the documented ranges; called by both `load_from` (defend
/// against a hand-edited file) and the setter commands (defend against a
/// future UI bug).
pub fn clamp_pane_widths(w: PaneWidths) -> PaneWidths {
    PaneWidths {
        sidebar: w.sidebar.clamp(SIDEBAR_MIN, SIDEBAR_MAX),
        right_panel: w.right_panel.clamp(RIGHT_PANEL_MIN, RIGHT_PANEL_MAX),
    }
}

/// Auto-fetch preference (P11). OFF by default; interval in minutes.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AutoFetch {
    pub enabled: bool,
    pub interval_minutes: u32,
}

impl Default for AutoFetch {
    fn default() -> Self {
        AutoFetch {
            enabled: false,
            interval_minutes: 5,
        }
    }
}

/// Health-refresh background job preference (P30 D7/D12). OFF by default;
/// interval in minutes. A pure `repo-changed` signal job — no git work.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct HealthRefresh {
    pub enabled: bool,
    pub interval_minutes: u32,
}

impl Default for HealthRefresh {
    fn default() -> Self {
        HealthRefresh {
            enabled: false,
            interval_minutes: 30,
        }
    }
}

pub const HEALTH_REFRESH_INTERVAL_MIN: u32 = 1;
pub const HEALTH_REFRESH_INTERVAL_MAX: u32 = 240;

/// Clamps the health-refresh interval to its documented range; called by both
/// `load_from` (defend a hand-edited file) and the setter command (P30 D12,
/// mirrors [`clamp_auto_fetch`]).
pub fn clamp_health_refresh(h: HealthRefresh) -> HealthRefresh {
    HealthRefresh {
        enabled: h.enabled,
        interval_minutes: h
            .interval_minutes
            .clamp(HEALTH_REFRESH_INTERVAL_MIN, HEALTH_REFRESH_INTERVAL_MAX),
    }
}

/// Which timestamp the graph's date column + relative/absolute date use (P51).
/// Pure UI preference; no Git effect. `Author` is the M2 baseline behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GraphDateBasis {
    #[default]
    Author,
    Committer,
}

/// Graph geometry knobs + per-row detail toggles (P11/P51). Geometry defaults
/// EQUAL the frontend METRICS defaults (avatar 10 / row 32 / lane 16) — the
/// "no override" baseline. Every P51 toggle is `#[serde(default)]` (via the
/// container-level `default`) so an OLD settings.json without them still
/// deserializes, falling back to the sensible defaults below. `dot_radius` was
/// removed (P51 D7 — a dead no-op field); an old file carrying `dotRadius` is
/// ignored (serde has no `deny_unknown_fields`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct GraphPrefs {
    pub avatar_radius: u32,
    pub row_height: u32,
    pub lane_width: u32,
    /// P51: show the short-SHA column (+ verified-badge slot). Default true.
    pub show_sha: bool,
    /// P51: show the optional full author-NAME text column. Default false
    /// (the avatar already conveys author; the name is the clutter-iest column).
    pub show_author: bool,
    /// P51: show the date column. Default true (M2 baseline showed it always).
    pub show_date: bool,
    /// P51: which timestamp the date column/tooltip use. Default Author.
    pub date_basis: GraphDateBasis,
    /// P51: ahead/behind chip on local-branch-tip pills. Default true (renders
    /// only on diverged branches — low clutter, high value).
    pub show_ahead_behind: bool,
    /// P51: compact (denser) rows preset. Default false.
    pub compact: bool,
}

impl Default for GraphPrefs {
    fn default() -> Self {
        GraphPrefs {
            avatar_radius: 10,
            row_height: 32,
            lane_width: 16,
            show_sha: true,
            show_author: false,
            show_date: true,
            date_basis: GraphDateBasis::Author,
            show_ahead_behind: true,
            compact: false,
        }
    }
}

pub const AUTO_FETCH_INTERVAL_MIN: u32 = 1;
pub const AUTO_FETCH_INTERVAL_MAX: u32 = 120;
pub const AVATAR_RADIUS_MIN: u32 = 6;
pub const AVATAR_RADIUS_MAX: u32 = 16;
pub const ROW_HEIGHT_MIN: u32 = 24;
pub const ROW_HEIGHT_MAX: u32 = 48;
pub const LANE_WIDTH_MIN: u32 = 10;
pub const LANE_WIDTH_MAX: u32 = 28;

/// Clamps the auto-fetch interval to its documented range; called by both
/// `load_from` (defend a hand-edited file) and the setter command.
pub fn clamp_auto_fetch(a: AutoFetch) -> AutoFetch {
    AutoFetch {
        enabled: a.enabled,
        interval_minutes: a
            .interval_minutes
            .clamp(AUTO_FETCH_INTERVAL_MIN, AUTO_FETCH_INTERVAL_MAX),
    }
}

/// Clamps each graph geometry knob to its documented range; called by both
/// `load_from` (defend a hand-edited file) and the setter command. The P51
/// detail toggles + `date_basis` have no numeric range — they pass through
/// unclamped via struct-update (`..g`). Keep the `..g`: dropping it would
/// silently reset every toggle to its field default on every load/save.
pub fn clamp_graph_prefs(g: GraphPrefs) -> GraphPrefs {
    GraphPrefs {
        avatar_radius: g.avatar_radius.clamp(AVATAR_RADIUS_MIN, AVATAR_RADIUS_MAX),
        row_height: g.row_height.clamp(ROW_HEIGHT_MIN, ROW_HEIGHT_MAX),
        lane_width: g.lane_width.clamp(LANE_WIDTH_MIN, LANE_WIDTH_MAX),
        ..g // toggles + date_basis pass through unclamped
    }
}

/// On-disk settings wire format:
/// `{ "version": 1, "recentRepos": [ { "path": "...", "lastOpened": 0 } ],
///    "theme": "dark", "paneWidths": { "sidebar": 240, "rightPanel": 380 },
///    "listView": "tree", "aiEnabled": true, "aiConflictAutonomy":
///    "proposeReview", "aiConsented": false }`.
///
/// `SETTINGS_VERSION` stays `1`: `theme`, `pane_widths`, `list_view`,
/// `open_repos`, `active_repo`, `auto_fetch`, `graph`, `ai_enabled`,
/// `ai_conflict_autonomy`, and `ai_consented` are all additive
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
    /// P49: terminal launch command template (`{path}` placeholder). Empty ⇒
    /// per-OS auto-detect (see `bonsai_core::external`). Additive
    /// `#[serde(default)]` ⇒ a pre-P49 file loads `""`.
    pub terminal_command: String,
    /// P49: editor launch command template (`{path}` placeholder). Empty ⇒
    /// auto-detect the VS Code family. Additive `#[serde(default)]` ⇒ a pre-P49
    /// file loads `""`.
    pub editor_command: String,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            version: SETTINGS_VERSION,
            recent_repos: Vec::new(),
            theme: ThemeChoice::default(),
            pane_widths: PaneWidths::default(),
            list_view: ListView::default(),
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
            terminal_command: String::new(),
            editor_command: String::new(),
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
    s
}

/// Saves settings to `file` atomically: creates parent dirs, writes pretty
/// JSON to `<file>.tmp` (same volume), then renames over `file`. On Windows
/// `std::fs::rename` replaces an existing destination file.
pub fn save_to(file: &Path, s: &Settings) -> Result<(), AppError> {
    if let Some(parent) = file.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| AppError::Io(format!("create settings dir {}: {e}", parent.display())))?;
    }
    let json = serde_json::to_string_pretty(s)
        .map_err(|e| AppError::Io(format!("serialize settings: {e}")))?;

    let mut tmp_name = file.as_os_str().to_owned();
    tmp_name.push(".tmp");
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
/// deduping by case-insensitive path compare, and truncating to
/// [`MAX_RECENT_REPOS`]. Pure — unit-testable.
///
/// Dedupe uses `str::eq_ignore_ascii_case` — correct for Windows drive letters
/// and ASCII paths; non-ASCII case-folding is a documented simplification
/// (P1 contract §12.4).
pub fn record_recent(s: &mut Settings, path: &str, now: i64) {
    s.recent_repos.retain(|r| !r.path.eq_ignore_ascii_case(path));
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
mod tests {
    use super::*;

    fn settings_path(dir: &tempfile::TempDir) -> PathBuf {
        dir.path().join("settings.json")
    }

    /// save_to then load_from round-trips the exact struct (P1 §3.3.1).
    #[test]
    fn roundtrip() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let file = settings_path(&dir);
        let mut s = Settings::default();
        record_recent(&mut s, "D:\\Repos\\x", 1_753_660_800);
        record_recent(&mut s, "D:\\Repos\\y", 1_753_660_900);

        save_to(&file, &s).expect("save settings");
        let loaded = load_from(&file);
        assert_eq!(loaded, s);
        assert_eq!(loaded.version, SETTINGS_VERSION);
        assert_eq!(loaded.recent_repos[0].path, "D:\\Repos\\y");
    }

    /// A missing file degrades to defaults, never errors (P1 §3.3.2).
    #[test]
    fn missing_file_defaults() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let loaded = load_from(&settings_path(&dir));
        assert_eq!(loaded, Settings::default());
        assert!(loaded.recent_repos.is_empty());
    }

    /// Corrupt JSON degrades to defaults, never errors (P1 §3.3.2).
    #[test]
    fn corrupt_json_defaults() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let file = settings_path(&dir);
        std::fs::write(&file, "{nope").expect("write corrupt file");
        assert_eq!(load_from(&file), Settings::default());
    }

    /// Insert 12 distinct paths -> capped at 10, newest first; re-inserting an
    /// existing path in a different case moves it to the front, dedupes, and
    /// updates last_opened (P1 §3.3.3).
    #[test]
    fn record_recent_upserts_and_caps() {
        let mut s = Settings::default();
        for i in 0..12 {
            record_recent(&mut s, &format!("D:\\Repos\\repo-{i}"), 1000 + i);
        }
        assert_eq!(s.recent_repos.len(), MAX_RECENT_REPOS);
        assert_eq!(s.recent_repos[0].path, "D:\\Repos\\repo-11");
        assert_eq!(s.recent_repos[9].path, "D:\\Repos\\repo-2");

        // Re-insert repo-5 with different case: moved to front, deduped,
        // last_opened stamped.
        record_recent(&mut s, "d:\\repos\\REPO-5", 2000);
        assert_eq!(s.recent_repos.len(), MAX_RECENT_REPOS);
        assert_eq!(s.recent_repos[0].path, "d:\\repos\\REPO-5");
        assert_eq!(s.recent_repos[0].last_opened, 2000);
        assert_eq!(
            s.recent_repos
                .iter()
                .filter(|r| r.path.eq_ignore_ascii_case("D:\\Repos\\repo-5"))
                .count(),
            1
        );
    }

    /// The atomic write leaves no `settings.json.tmp` behind (P1 §3.3.4).
    #[test]
    fn atomic_write_leaves_no_tmp() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let file = settings_path(&dir);
        let mut s = Settings::default();
        record_recent(&mut s, "D:\\Repos\\x", 1);
        save_to(&file, &s).expect("save settings");

        assert!(file.exists());
        assert!(!dir.path().join("settings.json.tmp").exists());

        // Overwriting an existing file is also atomic (rename replaces).
        record_recent(&mut s, "D:\\Repos\\y", 2);
        save_to(&file, &s).expect("save settings again");
        assert!(!dir.path().join("settings.json.tmp").exists());
        assert_eq!(load_from(&file), s);
    }

    /// save_to creates missing parent directories.
    #[test]
    fn save_creates_parent_dirs() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let file = dir.path().join("nested").join("deeper").join("settings.json");
        save_to(&file, &Settings::default()).expect("save into nested dir");
        assert_eq!(load_from(&file), Settings::default());
    }

    /// Below-min and above-max on each axis clamp to the documented bounds;
    /// in-range values pass through unchanged (P2a contract §3.4.1).
    #[test]
    fn clamp_pane_widths_clamps_both_axes() {
        assert_eq!(
            clamp_pane_widths(PaneWidths {
                sidebar: 10,
                right_panel: 10,
            }),
            PaneWidths {
                sidebar: SIDEBAR_MIN,
                right_panel: RIGHT_PANEL_MIN,
            }
        );
        assert_eq!(
            clamp_pane_widths(PaneWidths {
                sidebar: 9999,
                right_panel: 9999,
            }),
            PaneWidths {
                sidebar: SIDEBAR_MAX,
                right_panel: RIGHT_PANEL_MAX,
            }
        );
        let in_range = PaneWidths {
            sidebar: 300,
            right_panel: 400,
        };
        assert_eq!(clamp_pane_widths(in_range), in_range);
    }

    /// Save/load a `Settings` with non-default `theme` + `pane_widths`
    /// round-trips exactly (P2a contract §3.4.2).
    #[test]
    fn ui_settings_roundtrip() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let file = settings_path(&dir);
        let s = Settings {
            theme: ThemeChoice::Light,
            pane_widths: PaneWidths {
                sidebar: 300,
                right_panel: 420,
            },
            ..Default::default()
        };

        save_to(&file, &s).expect("save settings");
        let loaded = load_from(&file);
        assert_eq!(loaded, s);
        assert_eq!(loaded.theme, ThemeChoice::Light);
        assert_eq!(loaded.pane_widths.sidebar, 300);
        assert_eq!(loaded.pane_widths.right_panel, 420);
    }

    /// `Dark` also round-trips explicitly (not just the non-default `Light`
    /// case above) — both wire strings ("dark"/"light") deserialize back to
    /// the matching enum variant (P2 contract §2.1/§4.1).
    #[test]
    fn theme_choice_roundtrips_both_variants() {
        let dir = tempfile::TempDir::new().expect("create temp dir");

        let file_dark = settings_path(&dir).with_file_name("dark.json");
        let s_dark = Settings {
            theme: ThemeChoice::Dark,
            ..Default::default()
        };
        save_to(&file_dark, &s_dark).expect("save dark");
        assert_eq!(load_from(&file_dark).theme, ThemeChoice::Dark);
        let raw_dark = std::fs::read_to_string(&file_dark).expect("read dark.json");
        assert!(raw_dark.contains("\"theme\": \"dark\""));

        let file_light = settings_path(&dir).with_file_name("light.json");
        let s_light = Settings {
            theme: ThemeChoice::Light,
            ..Default::default()
        };
        save_to(&file_light, &s_light).expect("save light");
        assert_eq!(load_from(&file_light).theme, ThemeChoice::Light);
        let raw_light = std::fs::read_to_string(&file_light).expect("read light.json");
        assert!(raw_light.contains("\"theme\": \"light\""));
    }

    /// Both `ListView` wire strings ("tree"/"flat") round-trip through
    /// save/load, and the raw JSON uses the documented camelCase key +
    /// lowercase values (P3b contract §2.1).
    #[test]
    fn list_view_roundtrips_both_variants() {
        let dir = tempfile::TempDir::new().expect("create temp dir");

        let file_tree = settings_path(&dir).with_file_name("tree.json");
        let s_tree = Settings {
            list_view: ListView::Tree,
            ..Default::default()
        };
        save_to(&file_tree, &s_tree).expect("save tree");
        assert_eq!(load_from(&file_tree).list_view, ListView::Tree);
        let raw_tree = std::fs::read_to_string(&file_tree).expect("read tree.json");
        assert!(raw_tree.contains("\"listView\": \"tree\""));

        let file_flat = settings_path(&dir).with_file_name("flat.json");
        let s_flat = Settings {
            list_view: ListView::Flat,
            ..Default::default()
        };
        save_to(&file_flat, &s_flat).expect("save flat");
        assert_eq!(load_from(&file_flat).list_view, ListView::Flat);
        let raw_flat = std::fs::read_to_string(&file_flat).expect("read flat.json");
        assert!(raw_flat.contains("\"listView\": \"flat\""));
    }

    /// An old `settings.json` written before P2 (only `version`/`recentRepos`,
    /// no `theme`/`paneWidths` keys at all) still loads without error and
    /// falls back to the type defaults for the new fields — this is the
    /// forward-compat guarantee documented in the `Settings` doc comment
    /// (`#[serde(default)]` on the whole struct), exercised here against a
    /// hand-written legacy JSON string rather than a struct round-trip so a
    /// future accidental removal of `default` would actually fail this test.
    #[test]
    fn old_settings_file_without_new_fields_loads_with_defaults() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let file = settings_path(&dir);
        let legacy_json = r#"{
            "version": 1,
            "recentRepos": [ { "path": "D:\\Repos\\legacy", "lastOpened": 123 } ]
        }"#;
        std::fs::write(&file, legacy_json).expect("write legacy settings.json");

        let loaded = load_from(&file);
        assert_eq!(loaded.recent_repos.len(), 1);
        assert_eq!(loaded.recent_repos[0].path, "D:\\Repos\\legacy");
        assert_eq!(loaded.theme, ThemeChoice::default());
        assert_eq!(loaded.pane_widths, PaneWidths::default());
        assert_eq!(loaded.list_view, ListView::Tree);
    }

    /// Save/load a `Settings` with non-empty `open_repos` + `active_repo`
    /// round-trips exactly (P3e §6.1 / §9.1 "Session (P3e-b)").
    #[test]
    fn session_roundtrip() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let file = settings_path(&dir);
        let s = Settings {
            open_repos: vec![
                "D:\\Repos\\alpha".to_string(),
                "D:\\Repos\\beta".to_string(),
            ],
            active_repo: Some("D:\\Repos\\beta".to_string()),
            ..Default::default()
        };

        save_to(&file, &s).expect("save settings");
        let loaded = load_from(&file);
        assert_eq!(loaded, s);
        assert_eq!(
            loaded.open_repos,
            vec![
                "D:\\Repos\\alpha".to_string(),
                "D:\\Repos\\beta".to_string()
            ]
        );
        assert_eq!(loaded.active_repo.as_deref(), Some("D:\\Repos\\beta"));
    }

    /// An old `settings.json` written before P3e (no `openRepos`/`activeRepo`
    /// keys) loads with empty session defaults and preserves the existing
    /// fields — the additive-field guarantee for the P3e session fields
    /// specifically (P3e §6.1 / §9.1 "Session (P3e-b)").
    #[test]
    fn old_settings_file_without_session_fields_loads_with_defaults() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let file = settings_path(&dir);
        let json = r#"{
            "version": 1,
            "recentRepos": [ { "path": "D:\\Repos\\legacy", "lastOpened": 123 } ],
            "theme": "light",
            "paneWidths": { "sidebar": 300, "rightPanel": 400 },
            "listView": "flat"
        }"#;
        std::fs::write(&file, json).expect("write pre-P3e settings.json");

        let loaded = load_from(&file);
        // New session fields fall back to empty defaults.
        assert!(loaded.open_repos.is_empty());
        assert_eq!(loaded.active_repo, None);
        // Existing fields are preserved untouched.
        assert_eq!(loaded.recent_repos.len(), 1);
        assert_eq!(loaded.recent_repos[0].path, "D:\\Repos\\legacy");
        assert_eq!(loaded.theme, ThemeChoice::Light);
        assert_eq!(loaded.list_view, ListView::Flat);
        assert_eq!(loaded.pane_widths.sidebar, 300);
    }

    /// A pre-P3b `settings.json` that has `theme`/`paneWidths` but no
    /// `listView` key loads with the default (`Tree`) — the additive-field
    /// guarantee for the P3b setting specifically (P3b contract §2.1).
    #[test]
    fn old_settings_file_without_list_view_loads_default() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let file = settings_path(&dir);
        let json = r#"{
            "version": 1,
            "recentRepos": [],
            "theme": "light",
            "paneWidths": { "sidebar": 300, "rightPanel": 400 }
        }"#;
        std::fs::write(&file, json).expect("write pre-P3b settings.json");

        let loaded = load_from(&file);
        assert_eq!(loaded.list_view, ListView::Tree);
        assert_eq!(loaded.theme, ThemeChoice::Light); // other fields untouched
        assert_eq!(loaded.pane_widths.sidebar, 300);
    }

    /// A `settings.json` with in-range `recentRepos` but out-of-range/corrupt
    /// `paneWidths` values (e.g. hand-edited, or written by a future version
    /// with looser bounds) is clamped on load rather than left out-of-range or
    /// rejected wholesale (contract §2.1: "load_from calls clamp_pane_widths
    /// on the deserialized value before returning").
    #[test]
    fn corrupt_pane_widths_on_disk_are_clamped_on_load() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let file = settings_path(&dir);
        let json = r#"{
            "version": 1,
            "recentRepos": [],
            "theme": "dark",
            "paneWidths": { "sidebar": 0, "rightPanel": 999999 }
        }"#;
        std::fs::write(&file, json).expect("write out-of-range settings.json");

        let loaded = load_from(&file);
        assert_eq!(loaded.pane_widths.sidebar, SIDEBAR_MIN);
        assert_eq!(loaded.pane_widths.right_panel, RIGHT_PANEL_MAX);
    }

    /// A `paneWidths.sidebar` field of the wrong JSON type (string instead of
    /// number) makes the whole document fail to parse as `Settings` — per the
    /// documented "never errors" contract this degrades to full defaults
    /// (same as `corrupt_json_defaults`), not a partial/field-level recovery.
    /// This pins that behavior so a future switch to field-level tolerant
    /// parsing is a deliberate, visible change rather than an accidental
    /// regression.
    #[test]
    fn malformed_field_type_falls_back_to_full_defaults() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let file = settings_path(&dir);
        let json = r#"{
            "version": 1,
            "recentRepos": [ { "path": "D:\\Repos\\x", "lastOpened": 1 } ],
            "theme": "dark",
            "paneWidths": { "sidebar": "not-a-number", "rightPanel": 380 }
        }"#;
        std::fs::write(&file, json).expect("write malformed settings.json");

        assert_eq!(load_from(&file), Settings::default());
    }

    /// Below-min and above-max clamp to their bounds on each graph knob and on
    /// the auto-fetch interval; in-range values pass through (P11 §2.1/§2.4).
    #[test]
    fn clamp_auto_fetch_and_graph_prefs_clamp_ranges() {
        // Interval below-min (0) and above-max (999) clamp to 1/120; `enabled`
        // is carried through untouched.
        assert_eq!(
            clamp_auto_fetch(AutoFetch {
                enabled: true,
                interval_minutes: 0,
            }),
            AutoFetch {
                enabled: true,
                interval_minutes: AUTO_FETCH_INTERVAL_MIN,
            }
        );
        assert_eq!(
            clamp_auto_fetch(AutoFetch {
                enabled: false,
                interval_minutes: 999,
            }),
            AutoFetch {
                enabled: false,
                interval_minutes: AUTO_FETCH_INTERVAL_MAX,
            }
        );
        let in_range = AutoFetch {
            enabled: true,
            interval_minutes: 30,
        };
        assert_eq!(clamp_auto_fetch(in_range), in_range);

        // Each graph knob below-min clamps to its min (toggles pass through).
        assert_eq!(
            clamp_graph_prefs(GraphPrefs {
                avatar_radius: 0,
                row_height: 0,
                lane_width: 0,
                ..GraphPrefs::default()
            }),
            GraphPrefs {
                avatar_radius: AVATAR_RADIUS_MIN,
                row_height: ROW_HEIGHT_MIN,
                lane_width: LANE_WIDTH_MIN,
                ..GraphPrefs::default()
            }
        );
        // Each graph knob above-max clamps to its max.
        assert_eq!(
            clamp_graph_prefs(GraphPrefs {
                avatar_radius: 9999,
                row_height: 9999,
                lane_width: 9999,
                ..GraphPrefs::default()
            }),
            GraphPrefs {
                avatar_radius: AVATAR_RADIUS_MAX,
                row_height: ROW_HEIGHT_MAX,
                lane_width: LANE_WIDTH_MAX,
                ..GraphPrefs::default()
            }
        );
        // In-range graph knobs pass through unchanged.
        let g_in = GraphPrefs {
            avatar_radius: 12,
            row_height: 36,
            lane_width: 20,
            ..GraphPrefs::default()
        };
        assert_eq!(clamp_graph_prefs(g_in), g_in);
    }

    /// Save/load a `Settings` with non-default `auto_fetch` + `graph` round-trips
    /// exactly (P11 §2.4).
    #[test]
    fn auto_fetch_and_graph_roundtrip() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let file = settings_path(&dir);
        let s = Settings {
            auto_fetch: AutoFetch {
                enabled: true,
                interval_minutes: 15,
            },
            graph: GraphPrefs {
                avatar_radius: 14,
                row_height: 40,
                lane_width: 24,
                ..GraphPrefs::default()
            },
            ..Default::default()
        };

        save_to(&file, &s).expect("save settings");
        let loaded = load_from(&file);
        assert_eq!(loaded, s);
        assert!(loaded.auto_fetch.enabled);
        assert_eq!(loaded.auto_fetch.interval_minutes, 15);
        assert_eq!(loaded.graph.avatar_radius, 14);
        assert_eq!(loaded.graph.lane_width, 24);
    }

    /// A hand-edited file with out-of-range `autoFetch`/`graph` values is clamped
    /// on load rather than left out-of-range (P11 §2.1 — `load_from` clamps both
    /// new structs on read).
    #[test]
    fn corrupt_auto_fetch_and_graph_on_disk_are_clamped_on_load() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let file = settings_path(&dir);
        let json = r#"{
            "version": 1,
            "recentRepos": [],
            "theme": "dark",
            "paneWidths": { "sidebar": 240, "rightPanel": 380 },
            "autoFetch": { "enabled": true, "intervalMinutes": 0 },
            "graph": { "avatarRadius": 99, "rowHeight": 1, "laneWidth": 999 }
        }"#;
        std::fs::write(&file, json).expect("write out-of-range settings.json");

        let loaded = load_from(&file);
        assert_eq!(loaded.auto_fetch.interval_minutes, AUTO_FETCH_INTERVAL_MIN);
        assert!(loaded.auto_fetch.enabled);
        assert_eq!(loaded.graph.avatar_radius, AVATAR_RADIUS_MAX);
        assert_eq!(loaded.graph.row_height, ROW_HEIGHT_MIN);
        assert_eq!(loaded.graph.lane_width, LANE_WIDTH_MAX);
    }

    /// P51: non-default detail toggles + a `Committer` date basis round-trip
    /// through save/load, and the raw JSON carries the documented camelCase
    /// keys + the lowercase basis value.
    #[test]
    fn graph_prefs_toggles_roundtrip() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let file = settings_path(&dir);
        let s = Settings {
            graph: GraphPrefs {
                avatar_radius: 12,
                row_height: 30,
                lane_width: 18,
                show_sha: false,
                show_author: true,
                show_date: false,
                date_basis: GraphDateBasis::Committer,
                show_ahead_behind: false,
                compact: true,
            },
            ..Default::default()
        };
        save_to(&file, &s).expect("save settings");
        let loaded = load_from(&file);
        assert_eq!(loaded, s);
        assert!(!loaded.graph.show_sha);
        assert!(loaded.graph.show_author);
        assert!(!loaded.graph.show_date);
        assert_eq!(loaded.graph.date_basis, GraphDateBasis::Committer);
        assert!(!loaded.graph.show_ahead_behind);
        assert!(loaded.graph.compact);

        let raw = std::fs::read_to_string(&file).expect("read settings.json");
        assert!(raw.contains("\"showSha\": false"));
        assert!(raw.contains("\"dateBasis\": \"committer\""));
        assert!(raw.contains("\"compact\": true"));
    }

    /// P51 D7 back-compat: a legacy `graph` object that still carries the
    /// removed `dotRadius` field and NONE of the new P51 toggle keys loads
    /// without error — `dotRadius` is silently ignored (serde has no
    /// `deny_unknown_fields`) and every new toggle falls back to its
    /// `#[serde(default)]` default. Pins both the dead-field removal and the
    /// additive-toggle guarantee.
    #[test]
    fn old_graph_prefs_with_dot_radius_ignored() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let file = settings_path(&dir);
        let json = r#"{
            "version": 1,
            "recentRepos": [],
            "graph": { "dotRadius": 4, "avatarRadius": 12, "rowHeight": 30, "laneWidth": 18 }
        }"#;
        std::fs::write(&file, json).expect("write legacy graph settings.json");

        let loaded = load_from(&file);
        // Geometry from the legacy file is preserved (and clamped in-range).
        assert_eq!(loaded.graph.avatar_radius, 12);
        assert_eq!(loaded.graph.row_height, 30);
        assert_eq!(loaded.graph.lane_width, 18);
        // Every new P51 toggle falls back to its default (the `dotRadius` key
        // in the file is ignored, not an error).
        assert!(loaded.graph.show_sha);
        assert!(!loaded.graph.show_author);
        assert!(loaded.graph.show_date);
        assert_eq!(loaded.graph.date_basis, GraphDateBasis::Author);
        assert!(loaded.graph.show_ahead_behind);
        assert!(!loaded.graph.compact);
    }

    /// An old `settings.json` written before P11 (no `autoFetch`/`graph` keys)
    /// loads with the type defaults for the new fields and preserves existing
    /// ones — the additive-field guarantee for the P11 settings specifically
    /// (P11 §2.4).
    #[test]
    fn old_settings_file_without_auto_fetch_graph_loads_defaults() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let file = settings_path(&dir);
        let json = r#"{
            "version": 1,
            "recentRepos": [ { "path": "D:\\Repos\\legacy", "lastOpened": 123 } ],
            "theme": "light",
            "paneWidths": { "sidebar": 300, "rightPanel": 400 },
            "listView": "flat"
        }"#;
        std::fs::write(&file, json).expect("write pre-P11 settings.json");

        let loaded = load_from(&file);
        assert_eq!(loaded.auto_fetch, AutoFetch::default());
        assert_eq!(loaded.graph, GraphPrefs::default());
        // Existing fields untouched.
        assert_eq!(loaded.theme, ThemeChoice::Light);
        assert_eq!(loaded.list_view, ListView::Flat);
        assert_eq!(loaded.recent_repos.len(), 1);
    }

    /// An unrecognized `theme` string (e.g. from a hypothetical future third
    /// theme, or a hand-typo'd file) fails the whole-document parse the same
    /// way a malformed field type does — pinned for the same reason as above.
    #[test]
    fn unknown_theme_string_falls_back_to_full_defaults() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let file = settings_path(&dir);
        let json = r#"{
            "version": 1,
            "recentRepos": [],
            "theme": "solarized",
            "paneWidths": { "sidebar": 240, "rightPanel": 380 }
        }"#;
        std::fs::write(&file, json).expect("write unknown-theme settings.json");

        assert_eq!(load_from(&file), Settings::default());
    }

    /// `clamp_health_refresh` clamps below-min/above-max intervals, passes
    /// in-range through, and carries `enabled` untouched (P30 D12).
    #[test]
    fn clamp_health_refresh_clamps_range() {
        assert_eq!(
            clamp_health_refresh(HealthRefresh {
                enabled: true,
                interval_minutes: 0,
            }),
            HealthRefresh {
                enabled: true,
                interval_minutes: HEALTH_REFRESH_INTERVAL_MIN,
            }
        );
        assert_eq!(
            clamp_health_refresh(HealthRefresh {
                enabled: false,
                interval_minutes: 9999,
            }),
            HealthRefresh {
                enabled: false,
                interval_minutes: HEALTH_REFRESH_INTERVAL_MAX,
            }
        );
        let in_range = HealthRefresh {
            enabled: true,
            interval_minutes: 60,
        };
        assert_eq!(clamp_health_refresh(in_range), in_range);
    }

    /// Non-default `health_refresh` round-trips; a pre-P30 file without the
    /// key loads the default; an out-of-range on-disk value is clamped on
    /// load (P30 §3 — serde back-compat guarantee for the new field).
    #[test]
    fn health_refresh_roundtrip_backcompat_and_clamp() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let file = settings_path(&dir);
        let s = Settings {
            health_refresh: HealthRefresh {
                enabled: true,
                interval_minutes: 45,
            },
            ..Default::default()
        };
        save_to(&file, &s).expect("save settings");
        assert_eq!(load_from(&file), s);
        let raw = std::fs::read_to_string(&file).expect("read settings.json");
        assert!(raw.contains("\"healthRefresh\""));

        // Pre-P30 file (no healthRefresh key) → default, other fields kept.
        let legacy = r#"{
            "version": 1,
            "recentRepos": [],
            "theme": "light",
            "autoFetch": { "enabled": true, "intervalMinutes": 15 }
        }"#;
        std::fs::write(&file, legacy).expect("write legacy settings.json");
        let loaded = load_from(&file);
        assert_eq!(loaded.health_refresh, HealthRefresh::default());
        assert!(loaded.auto_fetch.enabled);
        assert_eq!(loaded.theme, ThemeChoice::Light);

        // Out-of-range on-disk value is clamped on load.
        let corrupt = r#"{
            "version": 1,
            "recentRepos": [],
            "healthRefresh": { "enabled": true, "intervalMinutes": 0 }
        }"#;
        std::fs::write(&file, corrupt).expect("write corrupt settings.json");
        let loaded = load_from(&file);
        assert_eq!(
            loaded.health_refresh.interval_minutes,
            HEALTH_REFRESH_INTERVAL_MIN
        );
        assert!(loaded.health_refresh.enabled);
    }

    /// Save/load a `Settings` with non-default AI fields round-trips exactly
    /// (P13 §4.1). Also asserts the raw JSON uses the documented camelCase keys
    /// + values.
    #[test]
    fn ai_settings_roundtrip() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let file = settings_path(&dir);
        let s = Settings {
            ai_enabled: false,
            ai_conflict_autonomy: AiAutonomy::AutoResolve,
            ai_consented: true,
            ..Default::default()
        };

        save_to(&file, &s).expect("save settings");
        let loaded = load_from(&file);
        assert_eq!(loaded, s);
        assert!(!loaded.ai_enabled);
        assert_eq!(loaded.ai_conflict_autonomy, AiAutonomy::AutoResolve);
        assert!(loaded.ai_consented);

        let raw = std::fs::read_to_string(&file).expect("read settings.json");
        assert!(raw.contains("\"aiEnabled\": false"));
        assert!(raw.contains("\"aiConflictAutonomy\": \"autoResolve\""));
        assert!(raw.contains("\"aiConsented\": true"));
    }

    /// An old `settings.json` written before P13 (no `aiEnabled`/
    /// `aiConflictAutonomy`/`aiConsented` keys) loads with the type defaults
    /// for the new fields (`true` / `ProposeReview` / `false`) and preserves
    /// existing ones — the additive-field guarantee for the P13 settings
    /// specifically (P13 §4.1), mirroring
    /// `old_settings_file_without_list_view_loads_default`.
    #[test]
    fn old_settings_file_without_ai_fields_loads_defaults() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let file = settings_path(&dir);
        let json = r#"{
            "version": 1,
            "recentRepos": [ { "path": "D:\\Repos\\legacy", "lastOpened": 123 } ],
            "theme": "light",
            "paneWidths": { "sidebar": 300, "rightPanel": 400 },
            "listView": "flat"
        }"#;
        std::fs::write(&file, json).expect("write pre-P13 settings.json");

        let loaded = load_from(&file);
        assert!(loaded.ai_enabled);
        assert_eq!(loaded.ai_conflict_autonomy, AiAutonomy::ProposeReview);
        assert!(!loaded.ai_consented);
        // Existing fields untouched.
        assert_eq!(loaded.theme, ThemeChoice::Light);
        assert_eq!(loaded.list_view, ListView::Flat);
        assert_eq!(loaded.recent_repos.len(), 1);
    }

    /// An old `settings.json` written before P43 (no `onboardingSeen` key) loads
    /// with the default (`false` ⇒ show onboarding once) and preserves existing
    /// fields — the additive-field guarantee for the P43 setting specifically,
    /// mirroring `old_settings_file_without_ai_fields_loads_defaults`.
    #[test]
    fn old_settings_file_without_onboarding_seen_loads_default() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let file = settings_path(&dir);
        let json = r#"{
            "version": 1,
            "recentRepos": [ { "path": "D:\\Repos\\legacy", "lastOpened": 123 } ],
            "theme": "light",
            "paneWidths": { "sidebar": 300, "rightPanel": 400 },
            "listView": "flat"
        }"#;
        std::fs::write(&file, json).expect("write pre-P43 settings.json");

        let loaded = load_from(&file);
        assert!(!loaded.onboarding_seen);
        // Existing fields untouched.
        assert_eq!(loaded.theme, ThemeChoice::Light);
        assert_eq!(loaded.list_view, ListView::Flat);
        assert_eq!(loaded.recent_repos.len(), 1);
    }

    /// A non-default `onboarding_seen` round-trips and serializes to the
    /// documented camelCase key (P43 §6).
    #[test]
    fn onboarding_seen_roundtrip() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let file = settings_path(&dir);
        let s = Settings {
            onboarding_seen: true,
            ..Default::default()
        };
        save_to(&file, &s).expect("save settings");
        let loaded = load_from(&file);
        assert_eq!(loaded, s);
        assert!(loaded.onboarding_seen);
        let raw = std::fs::read_to_string(&file).expect("read settings.json");
        assert!(raw.contains("\"onboardingSeen\": true"));
    }

    /// An old `settings.json` written before P42 (no `autoCheckUpdates` key)
    /// loads with the default (`false` ⇒ no auto-check on launch) and preserves
    /// existing fields — the additive-field guarantee for the P42 setting
    /// specifically, mirroring `old_settings_file_without_onboarding_seen_loads_default`.
    #[test]
    fn old_settings_file_without_auto_check_updates_loads_default() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let file = settings_path(&dir);
        let json = r#"{
            "version": 1,
            "recentRepos": [ { "path": "D:\\Repos\\legacy", "lastOpened": 123 } ],
            "theme": "light",
            "paneWidths": { "sidebar": 300, "rightPanel": 400 },
            "listView": "flat"
        }"#;
        std::fs::write(&file, json).expect("write pre-P42 settings.json");

        let loaded = load_from(&file);
        assert!(!loaded.auto_check_updates);
        // Existing fields untouched.
        assert_eq!(loaded.theme, ThemeChoice::Light);
        assert_eq!(loaded.list_view, ListView::Flat);
        assert_eq!(loaded.recent_repos.len(), 1);
    }

    /// A non-default `auto_check_updates` round-trips and serializes to the
    /// documented camelCase key (P42 D4).
    #[test]
    fn auto_check_updates_roundtrip() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let file = settings_path(&dir);
        let s = Settings {
            auto_check_updates: true,
            ..Default::default()
        };
        save_to(&file, &s).expect("save settings");
        let loaded = load_from(&file);
        assert_eq!(loaded, s);
        assert!(loaded.auto_check_updates);
        let raw = std::fs::read_to_string(&file).expect("read settings.json");
        assert!(raw.contains("\"autoCheckUpdates\": true"));
    }

    /// A couple of `profiles` round-trip through save/load intact (incl. the
    /// optional signing key both Some and None); AND a legacy `settings.json`
    /// WITHOUT the `profiles` key loads with `profiles == vec![]` — the
    /// additive-field back-compat guarantee for the P44 setting specifically,
    /// mirroring `old_settings_file_without_auto_check_updates_loads_default`.
    #[test]
    fn profiles_roundtrip_and_backcompat() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let file = settings_path(&dir);
        let s = Settings {
            profiles: vec![
                IdentityProfile {
                    id: "id-work".to_string(),
                    label: "Work".to_string(),
                    user_name: "Ada Lovelace".to_string(),
                    user_email: "work@example.com".to_string(),
                    signing_key: None,
                },
                IdentityProfile {
                    id: "id-personal".to_string(),
                    label: "Personal".to_string(),
                    user_name: "Ada".to_string(),
                    user_email: "me@personal.dev".to_string(),
                    signing_key: Some("KEY123".to_string()),
                },
            ],
            ..Default::default()
        };
        save_to(&file, &s).expect("save settings");
        let loaded = load_from(&file);
        assert_eq!(loaded, s);
        assert_eq!(loaded.profiles.len(), 2);
        assert_eq!(loaded.profiles[0].label, "Work");
        assert_eq!(loaded.profiles[0].signing_key, None);
        assert_eq!(loaded.profiles[1].signing_key.as_deref(), Some("KEY123"));
        let raw = std::fs::read_to_string(&file).expect("read settings.json");
        assert!(raw.contains("\"profiles\""));
        // camelCase wire mirror for the IdentityProfile fields.
        assert!(raw.contains("\"userEmail\""));
        assert!(raw.contains("\"signingKey\""));

        // Back-compat: a pre-P44 file without `profiles` loads as an empty Vec
        // and preserves the existing fields.
        let legacy = r#"{
            "version": 1,
            "recentRepos": [ { "path": "D:\\Repos\\legacy", "lastOpened": 123 } ],
            "theme": "light",
            "paneWidths": { "sidebar": 300, "rightPanel": 400 },
            "listView": "flat"
        }"#;
        std::fs::write(&file, legacy).expect("write pre-P44 settings.json");
        let loaded = load_from(&file);
        assert!(loaded.profiles.is_empty());
        assert_eq!(loaded.theme, ThemeChoice::Light);
        assert_eq!(loaded.list_view, ListView::Flat);
        assert_eq!(loaded.recent_repos.len(), 1);
    }

    /// An old `settings.json` written before P49 (no `terminalCommand`/
    /// `editorCommand` keys) loads both as `""` and preserves the existing
    /// fields — the additive-field back-compat guarantee for the P49 settings.
    /// Also asserts a round-trip through the documented camelCase keys.
    #[test]
    fn external_commands_roundtrip_and_backcompat() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let file = settings_path(&dir);

        // Round-trip non-default values + camelCase wire keys.
        let s = Settings {
            terminal_command: "wt -d {path}".to_string(),
            editor_command: "code {path}".to_string(),
            ..Default::default()
        };
        save_to(&file, &s).expect("save settings");
        let loaded = load_from(&file);
        assert_eq!(loaded, s);
        assert_eq!(loaded.terminal_command, "wt -d {path}");
        assert_eq!(loaded.editor_command, "code {path}");
        let raw = std::fs::read_to_string(&file).expect("read settings.json");
        assert!(raw.contains("\"terminalCommand\""));
        assert!(raw.contains("\"editorCommand\""));

        // Back-compat: a pre-P49 file without the keys loads both as "" and
        // preserves existing fields.
        let legacy = r#"{
            "version": 1,
            "recentRepos": [ { "path": "D:\\Repos\\legacy", "lastOpened": 123 } ],
            "theme": "light",
            "paneWidths": { "sidebar": 300, "rightPanel": 400 },
            "listView": "flat"
        }"#;
        std::fs::write(&file, legacy).expect("write pre-P49 settings.json");
        let loaded = load_from(&file);
        assert_eq!(loaded.terminal_command, "");
        assert_eq!(loaded.editor_command, "");
        assert_eq!(loaded.theme, ThemeChoice::Light);
        assert_eq!(loaded.list_view, ListView::Flat);
        assert_eq!(loaded.recent_repos.len(), 1);
    }
}
