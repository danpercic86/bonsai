//! [`super::resolve_program`] ordering + `PATH`-hygiene tests (P112 follow-up).
//!
//! Windows-only by nature: the non-Windows branch hands the bare name to
//! `Command` unchanged, so there is no resolution of ours to assert there (the
//! cross-platform hit/miss contract is covered by
//! `tests/misc/external_spawn.rs::resolve_program_hit_and_miss`).
//!
//! Everything here goes through [`super::resolve_in`], whose `PATH`/`PATHEXT`
//! are parameters — so no test mutates process-global environment state, which
//! would race every other test in the binary.
#![cfg(windows)]

use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

use super::{resolve_in, resolve_program};
use crate::testutil::scratch_dir;

/// `PATH` with one directory in it.
fn path_var(dir: &Path) -> OsString {
    dir.as_os_str().to_owned()
}

/// The default `PATHEXT` shape, with the real `.COM;.EXE;.BAT;.CMD` order.
const PATHEXT: &str = ".COM;.EXE;.BAT;.CMD";

/// Assert WHICH FILE a resolution chose, case-insensitively.
///
/// `PATHEXT` is conventionally UPPERCASE and Windows paths are
/// case-insensitive, so a resolution built from it carries the `PATHEXT`
/// spelling (`code.CMD`) while the on-disk name is `code.cmd`. Both name the
/// same file and both spawn — and this is pre-existing behaviour, unchanged by
/// the reorder — so the test pins the choice, not the casing.
fn assert_chose(got: Option<PathBuf>, want: PathBuf) {
    let got = got.expect("the program must resolve");
    assert!(got.is_file(), "{got:?} must be an existing file");
    assert_eq!(
        got.to_string_lossy().to_lowercase(),
        want.to_string_lossy().to_lowercase()
    );
}

#[test]
fn a_pathext_match_wins_over_an_extension_less_shim_in_the_same_directory() {
    // The measured VS Code shape: a `#!/usr/bin/env sh` shim named `code`
    // sitting beside `code.cmd` in `…\Microsoft VS Code\bin`. Bare-first
    // returned the shim, whose spawn fails with `os error 193`, which is what
    // broke "Open in editor" on a standard Windows install.
    let scratch = scratch_dir();
    let dir = scratch.path();
    std::fs::write(dir.join("code"), b"#!/usr/bin/env sh\n").expect("write the POSIX shim");
    std::fs::write(dir.join("code.cmd"), b"@echo off\r\n").expect("write the cmd shim");
    assert_chose(
        resolve_in("code", &path_var(dir), PATHEXT),
        dir.join("code.cmd"),
    );
}

#[test]
fn a_cmd_shim_with_no_bare_sibling_still_resolves() {
    // The npm case the bare-first order was believed to need: `claude.cmd` with
    // no extension-less `claude` beside it. The PATHEXT loop finds it either
    // way — which is why the reorder is safe for `ai::resolve_bin`.
    let scratch = scratch_dir();
    let dir = scratch.path();
    std::fs::write(dir.join("claude.cmd"), b"@echo off\r\n").expect("write the npm shim");
    assert_chose(
        resolve_in("claude", &path_var(dir), PATHEXT),
        dir.join("claude.cmd"),
    );
}

#[test]
fn an_extension_less_program_still_resolves_as_the_last_resort() {
    // The bare name is kept as a FALLBACK, not deleted: `CreateProcess`
    // validates the image header rather than the name, so an extension-less PE
    // does run. Nothing else in the directory, so only that branch can fire.
    let scratch = scratch_dir();
    let dir = scratch.path();
    std::fs::write(dir.join("tool"), b"MZ").expect("write the extension-less program");
    assert_chose(
        resolve_in("tool", &path_var(dir), PATHEXT),
        dir.join("tool"),
    );
}

#[test]
fn pathext_order_decides_between_two_extension_hits() {
    let scratch = scratch_dir();
    let dir = scratch.path();
    std::fs::write(dir.join("thing.cmd"), b"@echo off\r\n").expect("write .cmd");
    std::fs::write(dir.join("thing.exe"), b"MZ").expect("write .exe");
    // .EXE precedes .CMD in PATHEXT.
    assert_chose(
        resolve_in("thing", &path_var(dir), PATHEXT),
        dir.join("thing.exe"),
    );
}

#[test]
fn path_order_stays_the_primary_precedence() {
    // The extension preference is per-DIRECTORY: an earlier PATH entry wins
    // even when a later one holds a better-ranked extension.
    let first = scratch_dir();
    let second = scratch_dir();
    std::fs::write(first.path().join("thing.cmd"), b"@echo off\r\n").expect("write .cmd");
    std::fs::write(second.path().join("thing.exe"), b"MZ").expect("write .exe");
    let mut var = path_var(first.path());
    var.push(";");
    var.push(second.path().as_os_str());
    assert_chose(
        resolve_in("thing", &var, PATHEXT),
        first.path().join("thing.cmd"),
    );
}

#[test]
fn the_bare_name_in_an_earlier_dir_beats_an_extension_hit_in_a_later_one() {
    // The DISCRIMINATING case the test above cannot rule out. With `.cmd` in
    // dir1 and `.exe` in dir2, "all PATHEXT across all dirs, then bare across
    // all dirs" would ALSO pick dir1 — so that test does not prove the
    // iteration is per-directory, only that it is not extension-major.
    //
    // Bare in dir1 + `.exe` in dir2 separates them: per-directory yields dir1's
    // bare hit; a PATHEXT-major sweep would find dir2's `.exe` first. A hit
    // that is NOT the bare name is also caught, since the bare file is the only
    // thing in dir1.
    let first = scratch_dir();
    let second = scratch_dir();
    std::fs::write(first.path().join("thing"), b"MZ").expect("write bare");
    std::fs::write(second.path().join("thing.exe"), b"MZ").expect("write .exe");
    let mut var = path_var(first.path());
    var.push(";");
    var.push(second.path().as_os_str());
    assert_chose(
        resolve_in("thing", &var, PATHEXT),
        first.path().join("thing"),
    );
}

#[test]
fn an_empty_path_component_never_resolves_against_the_process_cwd() {
    // `cargo test` runs with the cwd at the package root, so `Cargo.toml` IS
    // there: without the guards, `dir.join(program)` for an empty component is
    // the bare RELATIVE name and would "resolve" to whatever the app's cwd
    // happens to hold. `std::env::split_paths` does yield empty components on
    // Windows (a trailing `;` is very common) — verified on this host.
    assert!(
        Path::new("Cargo.toml").is_file(),
        "this test's premise: the cwd is the package root"
    );
    // No PATHEXT, so only the bare-name branch can fire.
    assert_eq!(resolve_in("Cargo.toml", OsStr::new(";"), ""), None);
    assert_eq!(resolve_in("Cargo.toml", OsStr::new("C:\\nope;;"), ""), None);
}

#[test]
fn a_relative_path_entry_never_resolves() {
    // Deliberate, and the same rule as `gitbin`'s unix branch: a relative PATH
    // entry (`.`) can only ever produce a relative candidate, which must never
    // reach `Command`.
    assert_eq!(resolve_in("Cargo.toml", OsStr::new("."), ""), None);
}

#[test]
fn a_name_with_a_separator_is_used_verbatim_without_touching_path() {
    // Unchanged behaviour: the ladders hand absolute catalog paths straight
    // through, and a non-existent one stays an `Ok` so the spawn reports it.
    let literal = r"C:\nope\Code.exe";
    assert_eq!(resolve_program(literal), Ok(PathBuf::from(literal)));
}

#[test]
fn an_unresolvable_bare_name_is_an_error() {
    assert!(resolve_program("bonsai-definitely-not-a-real-tool-xyz123").is_err());
}
