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
use std::sync::Mutex;

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

/// Serializes the whole commit sequence below.
///
/// `save` runs OUTSIDE the `MetricsState` mutex (it must — it does file IO while
/// counters keep being bumped), so two overlapping savers are reachable in
/// practice: the 60 s flush timer vs `metrics_reset`, or the exit flush vs the
/// timer. Without this lock they share the single `usage.json.tmp` name (the
/// second `File::create` truncates the first's bytes) AND, worse, their two
/// renames interleave: A's `rename(tmp, primary)` can land between B's
/// `rename(primary, bak)` and B's own commit, leaving `.bak` newer than the
/// primary. A per-save unique temp name would fix only the first half — the
/// rename PAIR still needs to be atomic with respect to another saver, so the
/// mutex is the fix and the temp name stays fixed (and therefore self-cleaning:
/// the next save truncates any file a crash left behind).
///
/// Cross-process concurrency is deliberately out of scope: two app instances
/// sharing a config dir already conflict at the in-memory level (each holds its
/// own `MetricsFile`), which no temp-file scheme can repair.
static SAVE_LOCK: Mutex<()> = Mutex::new(());

/// Persists `file` atomically: temp + fsync → rotate primary to `.bak` → rename
/// temp onto primary → fsync the parent directory. `std::fs::rename` replaces the
/// destination on all three targets, so a crash between the two renames leaves a
/// good `.bak` that `load` recovers.
///
/// Serialized process-wide by [`SAVE_LOCK`]; see its doc for the race it closes.
pub fn save(path: &Path, file: &MetricsFile) -> Result<(), AppError> {
    // A poisoned lock means a previous saver panicked mid-sequence; the on-disk
    // state is still one of the two consistent files (`load` recovers from
    // `.bak`), and metrics must never block or fail the app, so we take the guard
    // anyway rather than propagating the poison.
    let _guard = SAVE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(parent) = path.parent() {
        super::fs_perm::create_dir_private(parent)
            .map_err(|e| AppError::Io(format!("cannot create metrics dir: {e}")))?;
    }
    let json = serde_json::to_vec_pretty(file)
        .map_err(|e| AppError::Other(format!("cannot serialize metrics: {e}")))?;

    let tmp = tmp_path(path);
    {
        // Owner-only (`0600` on unix). The temp is what gets renamed onto
        // `usage.json`, so the primary (and later its `.bak`) inherits the mode.
        let mut f = super::fs_perm::create_file_private(&tmp)
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
    // Durability of the rename itself: the file's own bytes are fsynced above,
    // but on a crash the DIRECTORY ENTRY that points at them can still be lost.
    sync_parent_dir(path);
    Ok(())
}

/// Best-effort fsync of the directory holding `path`, so the renames committed
/// above survive power loss. Never fails a save — metrics must not block the app,
/// and a lost rename costs at most the previous good primary, which `load`
/// already recovers.
///
/// PLATFORM LIMITATION (deliberate, not a silent no-op): directory fsync is only
/// meaningful on POSIX. On Windows a directory handle needs
/// `FILE_FLAG_BACKUP_SEMANTICS` to open at all, and `FlushFileBuffers` on it
/// returns `ERROR_ACCESS_DENIED` on NTFS — there is no supported way to flush a
/// directory. We still attempt it (it is harmless and a future filesystem may
/// honour it) and otherwise rely on NTFS's own metadata journal, which orders the
/// rename against the already-fsynced temp contents.
fn sync_parent_dir(path: &Path) {
    let Some(parent) = path.parent() else {
        return;
    };
    #[cfg(windows)]
    let opened = {
        use std::os::windows::fs::OpenOptionsExt;
        /// `FILE_FLAG_BACKUP_SEMANTICS` — required to obtain a directory handle.
        const BACKUP_SEMANTICS: u32 = 0x0200_0000;
        std::fs::OpenOptions::new()
            .read(true)
            .custom_flags(BACKUP_SEMANTICS)
            .open(parent)
    };
    #[cfg(not(windows))]
    let opened = std::fs::File::open(parent);

    if let Ok(dir) = opened {
        let _ = dir.sync_all();
    }
}

#[cfg(test)]
#[path = "tests_metrics_file.rs"]
mod tests_metrics_file;
