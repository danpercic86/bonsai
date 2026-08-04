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
