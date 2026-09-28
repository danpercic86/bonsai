//! `[MEDIUM-3]` fsread tests (E4). Symlink cases run wherever the OS lets
//! the test create a symlink (Windows needs Developer Mode); the special-file
//! cases are unix-gated.

use std::fs;
use std::path::{Path, PathBuf};

use super::*;

fn workdir() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let canon = fs::canonicalize(dir.path()).unwrap();
    (dir, canon)
}

/// Creates a file symlink; false when the platform refuses (unprivileged
/// Windows), in which case the caller skips.
pub(crate) fn try_symlink_file(target: &Path, link: &Path) -> bool {
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(target, link).is_ok()
    }
    #[cfg(windows)]
    {
        std::os::windows::fs::symlink_file(target, link).is_ok()
    }
}

fn try_symlink_dir(target: &Path, link: &Path) -> bool {
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(target, link).is_ok()
    }
    #[cfg(windows)]
    {
        std::os::windows::fs::symlink_dir(target, link).is_ok()
    }
}

fn content(o: ReadOutcome) -> String {
    match o {
        ReadOutcome::Content(s) => s,
        other => panic!("expected Content, got {other:?}"),
    }
}

#[test]
fn reads_a_regular_file() {
    let (_d, w) = workdir();
    fs::create_dir_all(w.join("a")).unwrap();
    fs::write(w.join("a/package.json"), "{}").unwrap();
    assert_eq!(content(safe_read(&w, "a/package.json", 10)), "{}");
    assert_eq!(safe_stat(&w, "a/package.json").map(|s| s.0), Some(2));
}

#[test]
fn missing_is_absent() {
    let (_d, w) = workdir();
    assert!(matches!(
        safe_read(&w, "nope.json", 10),
        ReadOutcome::Absent
    ));
    assert!(safe_stat(&w, "nope.json").is_none());
}

#[test]
fn over_cap_is_too_large_via_take() {
    let (_d, w) = workdir();
    fs::write(w.join("big"), vec![b'x'; 101]).unwrap();
    assert!(matches!(safe_read(&w, "big", 100), ReadOutcome::TooLarge));
    fs::write(w.join("exact"), vec![b'x'; 100]).unwrap();
    assert_eq!(content(safe_read(&w, "exact", 100)).len(), 100);
}

#[test]
fn unclean_paths_are_not_files() {
    let (_d, w) = workdir();
    fs::write(w.join("x"), "1").unwrap();
    for rel in [
        "",
        "..",
        "../x",
        "a/../x",
        "./x",
        "a/./x",
        "a//x",
        "/etc/passwd",
        "C:/Windows/win.ini",
        "C:x",
        "x:ads",
        "a\\..\\x",
        "x/",
        "\0",
    ] {
        assert!(!path_is_clean(rel), "{rel:?}");
        assert!(
            matches!(safe_read(&w, rel, 10), ReadOutcome::NotAFile),
            "{rel:?}"
        );
        assert!(safe_stat(&w, rel).is_none(), "{rel:?}");
    }
    assert!(path_is_clean("a/b/package.json"));
    assert!(path_is_clean(".npmrc"));
}

#[test]
fn directory_is_not_a_file() {
    let (_d, w) = workdir();
    fs::create_dir(w.join("package.json")).unwrap();
    assert!(matches!(
        safe_read(&w, "package.json", 10),
        ReadOutcome::NotAFile
    ));
    assert!(safe_stat(&w, "package.json").is_none());
}

#[test]
fn symlinked_file_is_not_a_file() {
    let (_d, w) = workdir();
    fs::write(w.join("real.json"), "{}").unwrap();
    if !try_symlink_file(&w.join("real.json"), &w.join("package.json")) {
        eprintln!("skipping: cannot create symlinks here");
        return;
    }
    assert!(matches!(
        safe_read(&w, "package.json", 10),
        ReadOutcome::NotAFile
    ));
    assert!(safe_stat(&w, "package.json").is_none());
}

#[test]
fn symlinked_parent_escaping_workdir_is_not_a_file() {
    let (_d, w) = workdir();
    let (_o, outside) = workdir();
    fs::write(outside.join("package.json"), "{\"secret\":1}").unwrap();
    if !try_symlink_dir(&outside, &w.join("sub")) {
        eprintln!("skipping: cannot create symlinks here");
        return;
    }
    assert!(matches!(
        safe_read(&w, "sub/package.json", 100),
        ReadOutcome::NotAFile
    ));
}

#[cfg(unix)]
#[test]
fn symlink_to_dev_zero_is_not_a_file() {
    let (_d, w) = workdir();
    assert!(try_symlink_file(
        Path::new("/dev/zero"),
        &w.join("yarn.lock")
    ));
    assert!(matches!(
        safe_read(&w, "yarn.lock", MAX_FILE_BYTES),
        ReadOutcome::NotAFile
    ));
}

/// A non-regular special file is rejected from `symlink_metadata` alone,
/// without opening it (so a FIFO could not block). A FIFO cannot be created
/// without `libc` or a process (both banned in this crate), so a unix socket
/// exercises the same `!is_file()` branch.
#[cfg(unix)]
#[test]
fn special_file_is_not_a_file_without_opening() {
    let (_d, w) = workdir();
    let _l = std::os::unix::net::UnixListener::bind(w.join("package.json")).unwrap();
    assert!(matches!(
        safe_read(&w, "package.json", 10),
        ReadOutcome::NotAFile
    ));
}

#[test]
fn debug_prints_only_the_variant() {
    let o = ReadOutcome::Content("//h/:_authToken=zz-sentinel-token".into());
    assert_eq!(format!("{o:?}"), "Content(..)");
}

#[test]
fn read_regular_rejects_a_directory_handle() {
    let (_d, w) = workdir();
    // Opening a directory fails on Windows and yields a non-file handle on
    // unix; either way it is not Content.
    assert!(!matches!(read_regular(&w, 10), ReadOutcome::Content(_)));
}
