//! Safe file reads (contract §4.5) `[MEDIUM-3]`. Every manifest, lockfile and
//! in-repo registry config goes through [`safe_read`]; nothing here parses.
//!
//! The rules: a clean relative path only, no symlink (checked with
//! `symlink_metadata`, so a symlink to `/dev/zero` or a FIFO is never opened),
//! the canonical target must stay inside the canonical workdir, the opened
//! handle is re-checked (TOCTOU), and the read is `take(cap + 1)` bounded.

use std::fs::{self, File};
use std::io::Read;
use std::path::{Component, Path, PathBuf};
use std::time::UNIX_EPOCH;

/// Per-file size cap for manifests and lockfiles `[MEDIUM-3]`.
pub const MAX_FILE_BYTES: u64 = 32 * 1024 * 1024;
/// Per-file size cap for registry configs (§4.4).
pub const MAX_CONFIG_BYTES: u64 = 1024 * 1024;

/// Result of [`safe_read`].
///
/// `[NEW-2]` The Debug impl is hand-written and prints ONLY the variant:
/// `Content` may be a registry config holding credentials.
pub enum ReadOutcome {
    Absent,
    Content(String),
    /// A symlink, FIFO, device, directory, an unclean path, or a target
    /// outside the workdir.
    NotAFile,
    TooLarge,
    Unreadable,
}

impl std::fmt::Debug for ReadOutcome {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            ReadOutcome::Absent => "Absent",
            ReadOutcome::Content(_) => "Content(..)",
            ReadOutcome::NotAFile => "NotAFile",
            ReadOutcome::TooLarge => "TooLarge",
            ReadOutcome::Unreadable => "Unreadable",
        })
    }
}

/// `[MEDIUM-3]` every component is `Normal`: no `..`, `.`, root, or drive
/// prefix. An empty path is not clean either. Backslashes are rejected too
/// (git paths use `/`, and on Unix a `\` would be a literal file-name byte),
/// and so is `:` (a Windows drive prefix or an NTFS alternate data stream).
pub fn path_is_clean(rel: &str) -> bool {
    if rel.is_empty() || rel.contains(['\\', '\0', ':']) {
        return false;
    }
    // `Path::components` silently normalises `a/./b` and `a//b`; check the
    // raw segments as well so neither slips through.
    if rel
        .split('/')
        .any(|s| s.is_empty() || s == "." || s == "..")
    {
        return false;
    }
    Path::new(rel)
        .components()
        .all(|c| matches!(c, Component::Normal(_)))
}

/// The canonical absolute path for `rel`, or the `ReadOutcome` that stops
/// the read. Shared by [`safe_read`] and [`safe_stat`].
fn resolve(workdir_canon: &Path, rel: &str) -> Result<PathBuf, ReadOutcome> {
    if !path_is_clean(rel) {
        return Err(ReadOutcome::NotAFile);
    }
    let joined = workdir_canon.join(rel);
    let md = match fs::symlink_metadata(&joined) {
        Ok(md) => md,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Err(ReadOutcome::Absent),
        Err(_) => return Err(ReadOutcome::Unreadable),
    };
    if md.file_type().is_symlink() || !md.is_file() {
        return Err(ReadOutcome::NotAFile);
    }
    // A symlinked PARENT directory is caught here: the canonical target
    // leaves the workdir (or, for an in-repo dir symlink, is still inside it
    // and is then just another in-repo path).
    let canon = fs::canonicalize(&joined).map_err(|_| ReadOutcome::Unreadable)?;
    if !canon.starts_with(workdir_canon) {
        return Err(ReadOutcome::NotAFile);
    }
    Ok(canon)
}

/// §4.5. `workdir_canon` MUST be `fs::canonicalize` output (on Windows both
/// sides of the containment check are then `\\?\` paths).
pub fn safe_read(workdir_canon: &Path, rel: &str, cap: u64) -> ReadOutcome {
    if let Err(o) = resolve(workdir_canon, rel) {
        return o;
    }
    // Every component no-follow, re-verified after open [MEDIUM-3].
    read_nofollow_under(workdir_canon, rel, cap)
}

/// The bounded `take(cap + 1)` read of an already-verified handle.
fn read_handle(f: File, cap: u64) -> ReadOutcome {
    let mut bytes = Vec::new();
    if f.take(cap.saturating_add(1))
        .read_to_end(&mut bytes)
        .is_err()
    {
        return ReadOutcome::Unreadable;
    }
    if bytes.len() as u64 > cap {
        return ReadOutcome::TooLarge;
    }
    ReadOutcome::Content(String::from_utf8_lossy(&bytes).into_owned())
}

/// `dir/rel` with every intermediate component a real directory and the
/// final one a regular file — none of them a symlink / junction — plus
/// the final `symlink_metadata`.
fn nofollow_components(dir: &Path, rel: &str) -> Result<(PathBuf, fs::Metadata), ReadOutcome> {
    if !path_is_clean(rel) {
        return Err(ReadOutcome::NotAFile);
    }
    let mut p = dir.to_path_buf();
    let mut parts = rel.split('/').peekable();
    while let Some(part) = parts.next() {
        if cfg!(windows) && is_dos_device_name(part) {
            // [Q9-4] `NUL`, `CON`, `COM1`, … name a device, not a file.
            return Err(ReadOutcome::NotAFile);
        }
        p.push(part);
        let md = match fs::symlink_metadata(&p) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Err(ReadOutcome::Absent),
            Err(_) => return Err(ReadOutcome::Unreadable),
            Ok(md) => md,
        };
        let last = parts.peek().is_none();
        let kind_ok = if last { md.is_file() } else { md.is_dir() };
        if md.file_type().is_symlink() || !kind_ok {
            return Err(ReadOutcome::NotAFile);
        }
        if last {
            return Ok((p, md));
        }
    }
    Err(ReadOutcome::NotAFile)
}

/// `[MEDIUM-3]` NO-FOLLOW read of `dir/rel`, where `rel` may have several
/// `/`-separated components (`.cargo/config.toml`): every component is
/// checked with `symlink_metadata` (a symlinked / junctioned `.cargo` dir
/// is `NotAFile`), the handle is re-checked after open, and the component
/// checks are RE-RUN after open and compared with the handle, so a swap
/// between check and open is caught.
pub(crate) fn read_nofollow_under(dir: &Path, rel: &str, cap: u64) -> ReadOutcome {
    let (path, before) = match nofollow_components(dir, rel) {
        Ok(v) => v,
        Err(o) => return o,
    };
    // `[Q9-5]` the last component is opened with O_NOFOLLOW /
    // FILE_FLAG_OPEN_REPARSE_POINT.
    let Ok(f) = open_nofollow(&path) else {
        return ReadOutcome::Unreadable;
    };
    let handle = match f.metadata() {
        Ok(md) if md.is_file() => md,
        Ok(_) => return ReadOutcome::NotAFile,
        Err(_) => return ReadOutcome::Unreadable,
    };
    match nofollow_components(dir, rel) {
        Ok((_, after)) if same_file(&before, &after) && same_file(&after, &handle) => {}
        Ok(_) => return ReadOutcome::NotAFile,
        Err(o) => return o,
    }
    read_handle(f, cap)
}

/// §4.5: the same checks as [`safe_read`]; returns `(len, mtime_ms)`.
pub fn safe_stat(workdir_canon: &Path, rel: &str) -> Option<(u64, i64)> {
    let canon = resolve(workdir_canon, rel).ok()?;
    let md = fs::metadata(&canon).ok()?;
    if !md.is_file() {
        return None;
    }
    let mtime_ms = md
        .modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX));
    Some((md.len(), mtime_ms))
}

mod follow;
mod open_nofollow;
pub use follow::{
    is_dos_device_name, is_network_path, read_above_workdir_config, read_config_follow,
};
use open_nofollow::{open_nofollow, same_file};

#[cfg(test)]
#[path = "fsread_tests.rs"]
pub(crate) mod tests;
