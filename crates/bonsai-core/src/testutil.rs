//! Test-only helpers (compiled under `#[cfg(test)]` only).
//!
//! HARD RULE (M3 contract §6.0): on Windows, C: is critically full — all
//! scratch repos and temp dirs live under `D:\Temp\bonsai-scratch`, never
//! the system temp. On macOS/Linux there is no such constraint, so scratch
//! dirs fall back to `std::env::temp_dir()/bonsai-scratch`.
//! Integration tests have their own copy in `tests/common/mod.rs` (a
//! `#[cfg(test)]` lib module is not linkable from integration binaries).

#[cfg(windows)]
fn scratch_root() -> std::path::PathBuf {
    std::path::PathBuf::from("D:\\Temp\\bonsai-scratch")
}

#[cfg(not(windows))]
fn scratch_root() -> std::path::PathBuf {
    std::env::temp_dir().join("bonsai-scratch")
}

/// Creates a scratch temp dir under the platform scratch root (created if
/// absent). Use this — never `TempDir::new()` — for every fixture.
pub fn scratch_dir() -> tempfile::TempDir {
    let root = scratch_root();
    std::fs::create_dir_all(&root).expect("create scratch root");
    tempfile::Builder::new()
        .prefix("bonsai-")
        .tempdir_in(&root)
        .expect("scratch dir")
}

/// Creates a directory symlink `link` -> `target` for the path-traversal guard
/// tests. On unix it MUST succeed (so CI always exercises the guard); on Windows,
/// where symlink creation needs privilege / Developer Mode, it returns `false`
/// when refused so the caller can skip the test gracefully. Returns `true` once
/// the symlink exists.
#[cfg(unix)]
pub fn make_dir_symlink_or_skip(target: &std::path::Path, link: &std::path::Path) -> bool {
    std::os::unix::fs::symlink(target, link).expect("create unix dir symlink");
    true
}

#[cfg(windows)]
pub fn make_dir_symlink_or_skip(target: &std::path::Path, link: &std::path::Path) -> bool {
    // Prefer a real directory symlink (needs privilege / Developer Mode).
    if std::os::windows::fs::symlink_dir(target, link).is_ok() {
        return true;
    }
    // Otherwise fall back to an NTFS directory JUNCTION: `std::fs::canonicalize`
    // resolves it exactly like a symlink (both are reparse points), but creating
    // one needs NO privilege. `mklink /J <link> <target>` is a cmd.exe builtin.
    // Returns `false` only if even that is unavailable, so the caller can skip.
    std::process::Command::new("cmd")
        .args(["/C", "mklink", "/J"])
        .arg(link)
        .arg(target)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}
