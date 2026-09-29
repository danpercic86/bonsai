//! Cache glue (contract §6.5): the `repo_key` is computed HERE (git's
//! `core.ignorecase` via `path_ignorecase`) and handed to `bonsai_deps::cache`
//! as a value — the crate never calls `path_ignorecase` (§7.4 allowlist).
//!
//! Saves are serialized by the caller: `exec::finish` saves while holding the
//! `repos` lock, the same lock the disable wipe holds, so two saves of one
//! key (a fixed tmp name) never interleave and no save lands after a wipe.

use std::path::Path;

use bonsai_core::git::history_index::store::repo_key;
use bonsai_core::git::repo::path_ignorecase;
use bonsai_deps::cache::{self, CacheError, CACHE_DIR};
use bonsai_deps::model::CachedReport;

/// Blocking (opens the repo to read `core.ignorecase`).
pub(crate) fn key_for(workdir: &Path) -> String {
    repo_key(workdir, path_ignorecase(workdir))
}

/// Blocking. `None` when there is no data root or no valid cache.
pub(crate) fn load(root: Option<&Path>, key: &str, repo_id: &str) -> Option<CachedReport> {
    cache::load(root?, key, repo_id)
}

/// Blocking; call with the `repos` lock held (see the module doc).
pub(crate) fn save(
    root: Option<&Path>,
    key: &str,
    report: &CachedReport,
) -> Result<(), CacheError> {
    cache::save(root.ok_or(CacheError::Io)?, key, report)
}

/// Blocking. Removes `<root>/deps-tracker`; a missing directory is fine.
/// The error is logged as a FACT only (the path carries the OS account name).
pub(crate) fn wipe(root: Option<&Path>) {
    let Some(root) = root else {
        return;
    };
    match std::fs::remove_dir_all(root.join(CACHE_DIR)) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(_) => eprintln!(
            "bonsai: could not remove the dependency-tracker cache (retried at next launch)"
        ),
    }
}
