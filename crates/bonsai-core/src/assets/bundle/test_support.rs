//! Shared filesystem helper for the `bundle` test modules.

use std::path::Path;

pub(super) fn write(root: &Path, rel: &str, bytes: &[u8]) {
    let full = root.join(rel);
    if let Some(parent) = full.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(full, bytes).unwrap();
}
