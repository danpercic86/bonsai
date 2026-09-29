//! Settings file I/O: load / serialized update / atomic save, plus the
//! Tauri-resolved settings path. Split out of `settings.rs` (size split);
//! re-exported from there so every `crate::settings::X` path is unchanged.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

use bonsai_core::error::AppError;

use super::{
    clamp_ai_settings, clamp_auto_fetch, clamp_dep_tracker, clamp_graph_prefs,
    clamp_health_refresh, clamp_pane_widths, migrate_external_tools,
    migrate_forge_hosts_to_accounts, Settings,
};

/// Serializes every load→mutate→save cycle in this process (audit §2.3).
///
/// `save_to` is atomic on its own, but two concurrent [`update`] cycles on
/// different blocking threads (e.g. a pane-width drag save racing the MCP
/// token persist) would each load, mutate their own field, and rename — the
/// last rename silently reverting the other's field. Pure reads (`load_from`)
/// stay lock-free: they never write, and a torn read is impossible through the
/// atomic rename.
static SETTINGS_IO: Mutex<()> = Mutex::new(());

/// Loads settings from `file`. Missing file, unreadable file, or unparseable
/// JSON all yield `Settings::default()` — settings are best-effort and this
/// NEVER errors (P1 contract §3.1).
pub fn load_from(file: &Path) -> Settings {
    let mut s: Settings = match std::fs::read_to_string(file) {
        Ok(text) => serde_json::from_str(&text).unwrap_or_default(),
        Err(_) => Settings::default(),
    };
    // Defend against a hand-edited / future-version file's out-of-range values (§2.1).
    s.pane_widths = clamp_pane_widths(s.pane_widths);
    s.auto_fetch = clamp_auto_fetch(s.auto_fetch);
    s.health_refresh = clamp_health_refresh(s.health_refresh);
    s.dep_tracker = clamp_dep_tracker(s.dep_tracker);
    s.graph = clamp_graph_prefs(s.graph);
    clamp_ai_settings(&mut s);
    // P80: lazy P79→P80 migration — pure/in-memory so every read sees the P80
    // shape; the write is deferred to the next `update`/`update_if` (never
    // write-amplify a pure read).
    let _ = migrate_forge_hosts_to_accounts(&mut s);
    // P112 §5.3: one-shot, pure/in-memory migration of the deleted free-text
    // `terminalCommand` / `editorCommand` (see `settings/external_tools.rs`).
    migrate_external_tools(&mut s);
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
