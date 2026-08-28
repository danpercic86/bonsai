//! P91 §8 — atomic persistence for `metrics/usage.json`.
//!
//! Crash-safety model (§8 decision 4): a write goes to a `usage.json.tmp`, is
//! fsynced, the previous good `usage.json` is rotated to `usage.json.bak`, then
//! the temp is renamed onto `usage.json`. A torn write therefore costs at most
//! the last flush window of counters, and a corrupt primary is detected on load
//! and recovered from `.bak` rather than losing all history. No SQLite, no second
//! vendored C build — `serde_json` is already present (§8 justification 1).

use std::io::Write;
use std::path::Path;

use bonsai_core::error::AppError;

use super::metrics::{MetricsFile, METRICS_SCHEMA_VERSION};

fn bak_path(path: &Path) -> std::path::PathBuf {
    let mut s = path.as_os_str().to_os_string();
    s.push(".bak");
    std::path::PathBuf::from(s)
}

fn tmp_path(path: &Path) -> std::path::PathBuf {
    let mut s = path.as_os_str().to_os_string();
    s.push(".tmp");
    std::path::PathBuf::from(s)
}

/// Loads and parses `path`. On a missing OR corrupt primary, falls back to
/// `path.bak`; if that too is unreadable, returns a fresh empty file. NEVER
/// errors — metrics must never block launch (§8).
pub fn load(path: &Path) -> MetricsFile {
    if let Some(f) = read_valid(path) {
        return f;
    }
    if let Some(f) = read_valid(&bak_path(path)) {
        return f;
    }
    MetricsFile {
        schema: METRICS_SCHEMA_VERSION,
        ..Default::default()
    }
}

/// Reads and JSON-parses a single candidate file, or `None` if absent/corrupt.
fn read_valid(path: &Path) -> Option<MetricsFile> {
    let bytes = std::fs::read(path).ok()?;
    serde_json::from_slice::<MetricsFile>(&bytes).ok()
}

/// Persists `file` atomically: temp + fsync → rotate primary to `.bak` → rename
/// temp onto primary. `std::fs::rename` replaces the destination on all three
/// targets, so a crash between the two renames leaves a good `.bak` that `load`
/// recovers.
pub fn save(path: &Path, file: &MetricsFile) -> Result<(), AppError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| AppError::Io(format!("cannot create metrics dir: {e}")))?;
    }
    let json = serde_json::to_vec_pretty(file)
        .map_err(|e| AppError::Other(format!("cannot serialize metrics: {e}")))?;

    let tmp = tmp_path(path);
    {
        let mut f = std::fs::File::create(&tmp)
            .map_err(|e| AppError::Io(format!("cannot create metrics temp: {e}")))?;
        f.write_all(&json)
            .map_err(|e| AppError::Io(format!("cannot write metrics temp: {e}")))?;
        f.sync_all()
            .map_err(|e| AppError::Io(format!("cannot fsync metrics temp: {e}")))?;
    }

    // Rotate the current good primary aside before overwriting it, so a crash
    // during the final rename still leaves a recoverable copy.
    if path.exists() {
        let _ = std::fs::rename(path, bak_path(path));
    }
    std::fs::rename(&tmp, path)
        .map_err(|e| AppError::Io(format!("cannot commit metrics file: {e}")))?;
    Ok(())
}

#[cfg(test)]
#[path = "tests_metrics_file.rs"]
mod tests_metrics_file;
