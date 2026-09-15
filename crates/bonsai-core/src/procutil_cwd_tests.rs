//! [`super::safe_cwd`] — the launch-neutral working directory (audit LOW-1).
//!
//! Migrated verbatim from `external_cmd_tests.rs` when P112 §7 deleted that
//! module; its own file rather than `procutil_tests.rs` because that one is
//! `#![cfg(windows)]` (the `PATHEXT` ladder has no non-Windows half) and this
//! property holds on every OS.

use std::path::PathBuf;

use super::safe_cwd;

#[test]
fn safe_cwd_is_an_existing_directory_that_is_not_the_repo() {
    let cwd = safe_cwd();
    assert!(cwd.is_dir(), "safe_cwd must exist: {}", cwd.display());
    // The test binary's own directory — never a repo working tree, which is the
    // whole point of LOW-1. `"."` would mean the process cwd leaked back in; the
    // documented fallback is `temp_dir()`, which is also never a repo.
    assert_ne!(cwd, PathBuf::from("."), "the cwd must never be the process cwd");
}
