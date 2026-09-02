//! P91 §6/§8 — owner-only permissions for everything observability writes.
//!
//! Log parts, export zips and `usage.json` carry absolute repo paths, real
//! branch names and (in `raw` mode) argument values. Created through plain
//! `File::create` they land at `0666 & ~umask` — typically `0644` — so on a
//! shared Linux/macOS box any other local account can read them. Every
//! observability file therefore goes through this module, which creates it
//! `0600`, and every observability directory through [`create_dir_private`],
//! which creates it `0700`.
//!
//! PLATFORM (deliberate, documented no-op — same shape as
//! `metrics_file::sync_parent_dir`): the mode bits are POSIX. On Windows there
//! is no umask and no meaningful `0600`; the files live under the per-user
//! `%APPDATA%` tree, which inherits an ACL granting the owning user and
//! administrators only. Faking an ACL rewrite here would add a privileged,
//! untested code path for no gain, so the Windows build simply skips the mode
//! and relies on that inherited ACL.

use std::fs::{File, OpenOptions};
use std::io;
use std::path::Path;

/// Mode for observability FILES (owner read/write only).
#[cfg(unix)]
const FILE_MODE: u32 = 0o600;
/// Mode for observability DIRECTORIES (owner read/write/traverse only).
#[cfg(unix)]
const DIR_MODE: u32 = 0o700;

/// `create_dir_all(path)`, then tighten `path` itself to `0700` on unix.
///
/// Only the leaf is tightened: the ancestors are the app-config tree, which
/// belongs to the platform, not to us.
pub fn create_dir_private(path: &Path) -> io::Result<()> {
    std::fs::create_dir_all(path)?;
    set_dir_mode(path);
    Ok(())
}

/// Creates (or truncates) `path` for writing with owner-only permissions.
pub fn create_file_private(path: &Path) -> io::Result<File> {
    let mut opts = OpenOptions::new();
    opts.write(true).create(true).truncate(true);
    with_file_mode(&mut opts);
    let file = opts.open(path)?;
    // `mode()` applies only when the open CREATES the file; an existing one keeps
    // whatever it had, so re-assert it on the handle we now hold.
    set_file_mode(&file);
    Ok(file)
}

/// Opens `path` for appending, creating it with owner-only permissions.
pub fn append_file_private(path: &Path) -> io::Result<File> {
    let mut opts = OpenOptions::new();
    opts.create(true).append(true);
    with_file_mode(&mut opts);
    let file = opts.open(path)?;
    set_file_mode(&file);
    Ok(file)
}

#[cfg(unix)]
fn with_file_mode(opts: &mut OpenOptions) {
    use std::os::unix::fs::OpenOptionsExt;
    opts.mode(FILE_MODE);
}

#[cfg(not(unix))]
fn with_file_mode(_opts: &mut OpenOptions) {}

/// Best-effort tighten of an already-open file. Never fails a write path:
/// observability must not break the app over a permission bit.
#[cfg(unix)]
fn set_file_mode(file: &File) {
    use std::os::unix::fs::PermissionsExt;
    let _ = file.set_permissions(std::fs::Permissions::from_mode(FILE_MODE));
}

#[cfg(not(unix))]
fn set_file_mode(_file: &File) {}

#[cfg(unix)]
fn set_dir_mode(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(DIR_MODE));
}

#[cfg(not(unix))]
fn set_dir_mode(_path: &Path) {}

/// TEST HELPER — the unix mode bits of `path`, or `None` off unix.
#[cfg(test)]
pub fn mode_of(path: &Path) -> Option<u32> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::metadata(path)
            .ok()
            .map(|m| m.permissions().mode() & 0o777)
    }
    #[cfg(not(unix))]
    {
        let _ = path;
        None
    }
}

#[cfg(test)]
#[path = "tests_fs_perm.rs"]
mod tests_fs_perm;
