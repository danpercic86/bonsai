//! Test-only helpers (compiled under `#[cfg(test)]` only).
//!
//! HARD RULE (M3 contract §6.0): C: is critically full — all scratch repos
//! and temp dirs live under `D:\Temp\bonsai-scratch`, never the system temp.
//! Integration tests have their own copy in `tests/common/mod.rs` (a
//! `#[cfg(test)]` lib module is not linkable from integration binaries).

/// Creates a scratch temp dir under `D:\Temp\bonsai-scratch` (created if
/// absent). Use this — never `TempDir::new()` — for every fixture.
pub fn scratch_dir() -> tempfile::TempDir {
    let root = std::path::Path::new("D:\\Temp\\bonsai-scratch");
    std::fs::create_dir_all(root).expect("create scratch root");
    tempfile::Builder::new()
        .prefix("bonsai-")
        .tempdir_in(root)
        .expect("scratch dir")
}
