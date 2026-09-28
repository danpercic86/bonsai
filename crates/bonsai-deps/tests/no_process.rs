//! AC11 / contract §7.4 `[MEDIUM-4]` and §7.3 item 4: a source scan backing
//! the clippy process ban.
//!
//! 1. No process-spawning token appears in the tracker's sources.
//! 2. Every `bonsai_core` path is on the allowlist (`use bonsai_core::{…}`
//!    groups are expanded first, aliases and globs included).
//! 3. Nothing under `src/remote/` names adapter types.
//! 4. Hardening (P125b): whitespace around `::` is collapsed first,
//!    `std::{…}` / `core::{…}` groups are expanded, aliasing `std` / `core`
//!    and the `include!` family are flagged, every `#[path]` must resolve
//!    inside the crate's `src/`, and `Cargo.toml` may not rename a
//!    dependency with `package =`.
//!
//! The src-tauri roots arrive in P125e; a root that does not exist yet is
//! skipped, not failed.

#[path = "no_process/scan.rs"]
mod scan;
#[path = "no_process/self_tests.rs"]
mod self_tests;

use std::path::PathBuf;

use scan::*;

struct Root {
    path: PathBuf,
    tauri_deps: bool,
    /// The crate `src/` that `#[path]` targets must stay inside.
    src_dir: PathBuf,
}

fn roots() -> Vec<Root> {
    let tauri = repo_root().join("src-tauri").join("src");
    vec![
        Root {
            path: crate_dir().join("src"),
            tauri_deps: false,
            src_dir: crate_dir().join("src"),
        },
        Root {
            path: tauri.join("deps"),
            tauri_deps: true,
            src_dir: tauri.clone(),
        },
        Root {
            path: tauri.join("commands").join("deps.rs"),
            tauri_deps: false,
            src_dir: tauri.clone(),
        },
    ]
}

#[test]
fn tracker_sources_contain_no_process_tokens() {
    let mut failures = Vec::new();
    let mut scanned = 0usize;
    for root in roots() {
        for file in rust_files(&root.path) {
            scanned += 1;
            for hit in banned_hits(&read(&file)) {
                failures.push(format!("{}: {hit}", file.display()));
            }
        }
    }
    assert!(scanned > 0, "the scan found no sources at all");
    assert!(
        failures.is_empty(),
        "process tokens found:\n{}",
        failures.join("\n")
    );
}

#[test]
fn bonsai_core_imports_are_allowlisted() {
    let mut failures = Vec::new();
    for root in roots() {
        for file in rust_files(&root.path) {
            for p in core_violations(&read(&file), root.tauri_deps) {
                failures.push(format!("{}: {p}", file.display()));
            }
        }
    }
    assert!(
        failures.is_empty(),
        "non-allowlisted bonsai_core paths:\n{}",
        failures.join("\n")
    );
}

#[test]
fn remote_names_no_adapter_types() {
    let remote = crate_dir().join("src").join("remote");
    let files = rust_files(&remote);
    assert!(!files.is_empty(), "src/remote/ must exist");
    let mut failures = Vec::new();
    for file in files {
        let src = read(&file);
        for t in REMOTE_BANNED.iter().filter(|t| src.contains(*t)) {
            failures.push(format!("{}: {t}", file.display()));
        }
    }
    assert!(
        failures.is_empty(),
        "adapter types under remote/:\n{}",
        failures.join("\n")
    );
}

#[test]
fn path_attributes_stay_inside_src() {
    let mut failures = Vec::new();
    for root in roots() {
        for file in rust_files(&root.path) {
            for p in path_escapes(&file, &read(&file), &root.src_dir) {
                failures.push(format!("{}: #[path = {p:?}]", file.display()));
            }
        }
    }
    assert!(
        failures.is_empty(),
        "#[path] outside src/:\n{}",
        failures.join("\n")
    );
}

#[test]
fn manifest_renames_no_dependency() {
    // A `bonsai-core` dependency renamed via `package = "bonsai-core"` would
    // bypass the `bonsai_core::` allowlist scan.
    let toml = read(&crate_dir().join("Cargo.toml"));
    let hits = package_keys(&toml);
    assert!(hits.is_empty(), "renamed dependencies: {hits:?}");
}
