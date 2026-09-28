//! The persisted report (contract §6.5):
//! `<app_data_root>/deps-tracker/<repo_key>.json`.
//!
//! `repo_key` is computed by src-tauri (`history_index::store::repo_key` +
//! `path_ignorecase`) and passed in as a VALUE; this crate never calls
//! `path_ignorecase` (§7.4 allowlist). A key that is not a plain file-name
//! token is refused, so no caller value can steer the path.
//!
//! - `load` ⇒ `None` on a schema or `repo_id` mismatch, a file > 32 MiB
//!   (bounded `take` read), or bad JSON. No display TTL.
//! - `save` writes a sibling tmp file, then renames it over the target.

use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use crate::model::{CachedReport, CACHE_SCHEMA};

/// The cache directory under the app data root.
pub const CACHE_DIR: &str = "deps-tracker";
/// Same cap as a repo file (§4.2).
pub const MAX_CACHE_BYTES: u64 = 32 * 1024 * 1024;

/// Why a save failed. No payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CacheError {
    BadKey,
    Io,
    Serialize,
}

/// `[A-Za-z0-9_-]{1,128}` (a `repo_key` is lowercase hex).
fn key_ok(repo_key: &str) -> bool {
    (1..=128).contains(&repo_key.len())
        && repo_key
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

/// `<root>/deps-tracker/<repo_key>.json`, or None for a bad key.
pub fn cache_path(app_data_root: &Path, repo_key: &str) -> Option<PathBuf> {
    key_ok(repo_key).then(|| {
        app_data_root
            .join(CACHE_DIR)
            .join(format!("{repo_key}.json"))
    })
}

pub fn load(app_data_root: &Path, repo_key: &str, repo_id: &str) -> Option<CachedReport> {
    let path = cache_path(app_data_root, repo_key)?;
    // [P125d review] stat BEFORE open: a FIFO / device / symlink at the path
    // is refused without an `open` that could block.
    let md = fs::symlink_metadata(&path).ok()?;
    if !md.is_file() || md.len() > MAX_CACHE_BYTES {
        return None;
    }
    let file = fs::File::open(path).ok()?;
    if !file.metadata().ok()?.is_file() {
        return None;
    }
    let mut bytes = Vec::new();
    file.take(MAX_CACHE_BYTES + 1)
        .read_to_end(&mut bytes)
        .ok()?;
    if bytes.len() as u64 > MAX_CACHE_BYTES {
        return None;
    }
    let c: CachedReport = serde_json::from_slice(&bytes).ok()?;
    (c.schema == CACHE_SCHEMA && c.repo_id == repo_id && c.report.repo_id == repo_id).then_some(c)
}

pub fn save(app_data_root: &Path, repo_key: &str, report: &CachedReport) -> Result<(), CacheError> {
    let path = cache_path(app_data_root, repo_key).ok_or(CacheError::BadKey)?;
    let bytes = serde_json::to_vec(report).map_err(|_| CacheError::Serialize)?;
    let dir = app_data_root.join(CACHE_DIR);
    fs::create_dir_all(&dir).map_err(|_| CacheError::Io)?;
    let tmp = dir.join(format!("{repo_key}.json.tmp"));
    let write = || -> std::io::Result<()> {
        let mut f = fs::File::create(&tmp)?;
        f.write_all(&bytes)?;
        f.sync_all()?;
        drop(f);
        fs::rename(&tmp, &path)
    };
    write().map_err(|_| {
        let _ = fs::remove_file(&tmp);
        CacheError::Io
    })
}

#[cfg(test)]
#[path = "cache_tests.rs"]
mod tests;
