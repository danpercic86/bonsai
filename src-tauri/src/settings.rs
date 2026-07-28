//! App settings persistence (P1 contract §3.1).
//!
//! Hand-rolled `settings.json` under the app config dir — no `tauri-plugin-store`
//! (one tiny struct, no new capability surface). All file functions are
//! path-parameterized so they stay unit-testable without an `AppHandle`; only
//! [`settings_file`] touches Tauri.

use std::path::{Path, PathBuf};

use crate::error::AppError;

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

/// On-disk settings wire format:
/// `{ "version": 1, "recentRepos": [ { "path": "...", "lastOpened": 0 } ],
///    "theme": "dark", "paneWidths": { "sidebar": 240, "rightPanel": 380 } }`.
///
/// `SETTINGS_VERSION` stays `1`: both `theme` and `pane_widths` are additive
/// `#[serde(default)]` fields (on the whole struct already, via the
/// container-level `default`) — an old settings.json containing only
/// `recentRepos` deserializes fine, missing fields fall back to their type
/// defaults. No migration code is needed. A future genuine breaking change
/// (e.g. renaming/removing a field with no safe default) IS when a version
/// bump becomes necessary — this precedent documents the bar for that.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub version: u32,
    pub recent_repos: Vec<RecentRepo>,
    pub theme: ThemeChoice,
    pub pane_widths: PaneWidths,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            version: SETTINGS_VERSION,
            recent_repos: Vec::new(),
            theme: ThemeChoice::default(),
            pane_widths: PaneWidths::default(),
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
}
