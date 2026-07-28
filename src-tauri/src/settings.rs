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

/// On-disk settings wire format:
/// `{ "version": 1, "recentRepos": [ { "path": "...", "lastOpened": 0 } ] }`.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub version: u32,
    pub recent_repos: Vec<RecentRepo>,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            version: SETTINGS_VERSION,
            recent_repos: Vec::new(),
        }
    }
}

/// Loads settings from `file`. Missing file, unreadable file, or unparseable
/// JSON all yield `Settings::default()` — settings are best-effort and this
/// NEVER errors (P1 contract §3.1).
pub fn load_from(file: &Path) -> Settings {
    match std::fs::read_to_string(file) {
        Ok(text) => serde_json::from_str(&text).unwrap_or_default(),
        Err(_) => Settings::default(),
    }
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
}
