//! Repo-lifecycle core (P21 contract §2): clone + init.
//!
//! `clone_repo` wraps git2 `RepoBuilder`/`FetchOptions`, reusing the M6
//! credential chain (`crate::git::remote::acquire_cred` / `map_remote_err`)
//! verbatim and adding a `transfer_progress` callback that streams
//! `CloneProgress` ticks to a caller-supplied sink. `init_repo` wraps
//! `Repository::init` (non-bare), opening idempotently if the path is already
//! a repo. Pure git2 logic, NO Tauri types — the command layer bridges the
//! progress sink to a Tauri `Channel` and runs the blocking body under
//! `spawn_blocking`. Testable against the git CLI oracle over local-path
//! remotes (see `tests/lifecycle_cli.rs`).

use std::cell::RefCell;
use std::path::Path;

use crate::error::AppError;
use crate::git::remote::{acquire_cred, map_remote_err, CredAttempts};

/// Streamed clone transfer progress (one per git2 `transfer_progress` tick).
/// Wire: camelCase. Sent over a Tauri `Channel<CloneProgress>` (contract
/// §OPEN-1); carries NO libgit2 handles — five scalar counters only.
#[derive(Debug, Clone, Copy, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CloneProgress {
    /// git2 `Progress::received_objects()` — objects downloaded so far.
    pub received_objects: u32,
    /// git2 `Progress::total_objects()` — total to download (0 until known).
    pub total_objects: u32,
    /// git2 `Progress::indexed_deltas()` — deltas resolved so far.
    pub indexed_deltas: u32,
    /// git2 `Progress::total_deltas()` — total deltas to resolve.
    pub total_deltas: u32,
    /// git2 `Progress::received_bytes()` — bytes over the wire so far.
    pub received_bytes: u64,
}

/// Saturating usize→u32 (counts on the wire; overflow is theoretical).
/// Duplicated privately from `remote.rs:201` (it is `fn`, not `pub(crate)`) to
/// avoid touching remote.rs (contract §2.3).
fn to_u32(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

/// Blocking. Clones `url` into `dest` using git2 `RepoBuilder` + `FetchOptions`
/// with the SHARED M6 credential callbacks (helper → SSH agent → default) and a
/// `transfer_progress` callback that calls `on_progress` once per tick. Returns
/// the ABSOLUTE workdir path of the cloned repo (feed to `openTab`).
///
/// `on_progress` is a plain sink so the command layer can bridge it to a Tauri
/// `Channel` WITHOUT pulling `tauri::` into core (contract §OPEN-1, §2.3).
/// Errors: dest exists & non-empty / dest is a file (`Io`, §OPEN-4); auth
/// exhausted (`AuthFailed` via `map_remote_err`); transport/invalid-URL
/// (`NetworkError`/`Git` via `map_remote_err`).
pub fn clone_repo(
    url: &str,
    dest: &Path,
    on_progress: impl FnMut(CloneProgress) + Send,
) -> Result<String, AppError> {
    // 1. dest pre-check (§OPEN-4): if dest exists AND is a non-empty dir → Io.
    if dest.exists() {
        if dest.is_file() {
            return Err(AppError::Io(format!(
                "destination is a file: {}",
                dest.display()
            )));
        }
        let mut entries = std::fs::read_dir(dest)?; // ? -> AppError::Io via From
        if entries.next().is_some() {
            return Err(AppError::Io(format!(
                "destination directory is not empty: {}",
                dest.display()
            )));
        }
    }

    // 2. Credentials: no repo yet -> `credential_fill(None, url)` falls back to
    // global/system git config with no cwd override, matching what `git clone`
    // itself does before a repo exists (§OPEN-5).
    let attempts = RefCell::new(CredAttempts::default());
    // Shared mutable sink for the FnMut callback. libgit2 invokes the callbacks
    // synchronously on THIS thread, so RefCell (not Mutex) is correct.
    let on_progress = RefCell::new(on_progress);

    // 3. Callbacks mirror remote.rs::fetch_remote, ADDING transfer_progress.
    let mut callbacks = git2::RemoteCallbacks::new();
    callbacks.credentials(|url, username_from_url, allowed| {
        acquire_cred(None, &attempts, url, username_from_url, allowed)
    });
    callbacks.transfer_progress(|stats: git2::Progress| {
        (on_progress.borrow_mut())(CloneProgress {
            received_objects: to_u32(stats.received_objects()),
            total_objects: to_u32(stats.total_objects()),
            indexed_deltas: to_u32(stats.indexed_deltas()),
            total_deltas: to_u32(stats.total_deltas()),
            received_bytes: stats.received_bytes() as u64,
        });
        true // never cancel in v1 (§OPEN-1)
    });

    let mut fo = git2::FetchOptions::new();
    fo.remote_callbacks(callbacks);

    let mut builder = git2::build::RepoBuilder::new();
    builder.fetch_options(fo);

    // 4. Clone. Map transfer/auth errors via the shared M6 mapper (ctx = url).
    let repo = builder
        .clone(url, dest)
        .map_err(|e| map_remote_err(e, url))?;

    // 5. Return the absolute workdir path (non-bare clone always has one).
    let workdir = repo
        .workdir()
        .ok_or_else(|| AppError::Git("cloned repository has no working directory".to_string()))?;
    Ok(workdir.to_string_lossy().into_owned())
}

/// Blocking. Initializes a NON-bare repository at `path` (`Repository::init`),
/// creating the directory if needed. If `path` is ALREADY a repo, opens it
/// instead of reinitializing (idempotent, §OPEN-3). Returns the ABSOLUTE
/// workdir path (feed to `openTab`). Errors: `path` exists as a FILE (`Io`).
pub fn init_repo(path: &Path) -> Result<String, AppError> {
    // path exists as a file -> Io error.
    if path.is_file() {
        return Err(AppError::Io(format!(
            "path is a file, not a directory: {}",
            path.display()
        )));
    }
    // Already a repo? Open it (idempotent, §OPEN-3) rather than reinitializing.
    let repo = match git2::Repository::open_ext(
        path,
        git2::RepositoryOpenFlags::NO_SEARCH,
        std::iter::empty::<&std::ffi::OsStr>(),
    ) {
        Ok(r) => r,
        Err(e) if e.code() == git2::ErrorCode::NotFound => git2::Repository::init(path)?, // non-bare
        Err(e) => return Err(e.into()),
    };
    let workdir = repo.workdir().ok_or_else(|| {
        AppError::Git("initialized repository has no working directory".to_string())
    })?;
    Ok(workdir.to_string_lossy().into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// §5.2 #7: `CloneProgress` serializes to the exact camelCase key set the
    /// TS `CloneProgress` mirrors.
    #[test]
    fn clone_progress_wire_shape_is_camel_case() {
        let v = serde_json::to_value(CloneProgress {
            received_objects: 3,
            total_objects: 10,
            indexed_deltas: 1,
            total_deltas: 5,
            received_bytes: 4096,
        })
        .expect("json");
        assert_eq!(
            v,
            serde_json::json!({
                "receivedObjects": 3,
                "totalObjects": 10,
                "indexedDeltas": 1,
                "totalDeltas": 5,
                "receivedBytes": 4096,
            })
        );
    }
}
