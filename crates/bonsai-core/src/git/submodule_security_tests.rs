//! Security tests for [`crate::git::submodule`] — the audit 2026-09-03
//! `.gitmodules` path-hostility cases, split out of `submodule_tests.rs`
//! when that file reached exactly the ~500-line limit (CLAUDE.md).
//! Declared from `submodule.rs` alongside `tests`, so `use super::*`
//! still names the submodule module.

use super::*;
use std::path::Path;

// ---------------------------------------------------------------------------
// Security audit 2026-09-03 — `.gitmodules` path hostility reaching `abs_path`.
//
// `list_submodules` builds `abs_path` from an attacker-controlled `.gitmodules`
// `path`. These cases are DRIVEN BY MEASUREMENT (git2 on this host, quoted
// config values so `;`/`#` are not config comments):
//
//   * ADMITTED and ESCAPING via `Path::join` — MUST become `abs_path: None`:
//       `/Windows/System32` (rooted → drive root), `//host/share` (UNC → base
//       discarded), and on Windows both `\\host\\share` (a REAL UNC prefix)
//       and `\host\share` (NOT UNC — `RootDir`, a rooted escape; the first
//       version of these tests mislabelled it, reviewer 2026-09-03).
//   * ADMITTED but CONTAINED — MUST keep a `Some(..)` abs_path:
//       `vendor/lib` (control), `a;b`, `a b`, `payload.exe`.
//   * REJECTED BY libgit2 before ever reaching us (NOT tested here — they never
//       produce a row): `..`/`../`/`vendor/../..` traversal, `C:/Windows`,
//       `C:\Windows`, a leading-`\t` path. libgit2 clears these upstream, so a
//       test would assert vacuously; `contained_abs_path`'s own unit tests
//       cover the defense-in-depth rejection of `..` and drive paths directly.

/// Write a `.gitmodules` with one entry `name`→`raw_path` (quoted, so `;`/`#`
/// are not comments), returning the opened superproject dir. A committed HEAD
/// exists so `list_submodules` opens a normal workdir repo.
fn super_with_gitmodules(raw_path: &str) -> tempfile::TempDir {
    let dir = crate::testutil::scratch_dir();
    let repo = git2::Repository::init(dir.path()).expect("init superproject");
    super::tests::seed_commit(&repo);
    let esc = raw_path.replace('\\', "\\\\").replace('"', "\\\"");
    std::fs::write(
        dir.path().join(".gitmodules"),
        format!("[submodule \"sm\"]\n\tpath = \"{esc}\"\n\turl = ./sub.git\n"),
    )
    .expect("write .gitmodules");
    dir
}

/// The one row `list_submodules` produced (there is exactly one entry), or a
/// panic if libgit2 dropped it entirely (which would make the case vacuous).
fn only_submodule_row(dir: &Path) -> SubmoduleInfo {
    let rows = list_submodules(dir).expect("list_submodules");
    assert_eq!(
        rows.len(),
        1,
        "expected exactly one admitted submodule row: {rows:?}"
    );
    rows.into_iter().next().expect("one row")
}

/// POSITIVE CONTROL (audit step 2): an ordinary relative submodule path still
/// yields a `Some` abs_path that is the workdir-contained join — the row stays
/// launchable. Without this, a validator that rejected everything would look
/// identical to one that works.
#[test]
fn list_submodules_admits_plain_relative_path() {
    let dir = super_with_gitmodules("vendor/lib");
    let row = only_submodule_row(dir.path());
    let abs = row
        .abs_path
        .expect("plain relative path must keep a launchable abs_path");
    let wd = dir.path().to_string_lossy().replace('\\', "/");
    assert!(
        abs.replace('\\', "/").starts_with(&wd),
        "contained under workdir: {abs}"
    );
    assert!(abs.replace('\\', "/").ends_with("vendor/lib"), "{abs}");
}

/// Contained-but-metachar paths (`;`, space, `.exe`) are ADMITTED with a
/// `Some` abs_path — the `;` protection is downstream (one argv token; `wt`'s
/// own arg convention), NOT a reason to null the path here.
#[test]
fn list_submodules_keeps_contained_metachar_paths() {
    for raw in ["a;b", "a b", "payload.exe"] {
        let dir = super_with_gitmodules(raw);
        let row = only_submodule_row(dir.path());
        assert!(
            row.abs_path.is_some(),
            "contained path {raw:?} must keep an abs_path, got None"
        );
    }
}

/// The finding: a ROOTED `.gitmodules` path escapes `Path::join` to the drive
/// root. It MUST surface as `abs_path: None` (inert row), never a launchable
/// absolute path outside the superproject.
#[test]
fn list_submodules_nulls_rooted_path() {
    let dir = super_with_gitmodules("/Windows/System32");
    let row = only_submodule_row(dir.path());
    assert_eq!(
        row.abs_path, None,
        "a rooted .gitmodules path must yield an inert abs_path: None"
    );
    // The row is still LISTED (decision (b)) so the malformed submodule is visible.
    assert_eq!(row.path, "/Windows/System32");
}

/// A forward-slash UNC `.gitmodules` path — `Path::join` DISCARDS the base
/// entirely, so an unchecked `abs_path` would dial an attacker host on
/// `p.exists()`. MUST be `None`. Platform-neutral: on POSIX this is absolute
/// (`RootDir`), on Windows a UNC `Prefix` — neither is all-`Normal`, so both
/// are rejected, for different reasons that both hold.
#[test]
fn list_submodules_nulls_forward_slash_unc_path() {
    let dir = super_with_gitmodules("//attacker.example/share");
    let row = only_submodule_row(dir.path());
    assert_eq!(
        row.abs_path, None,
        "a //host/share .gitmodules path must yield abs_path: None"
    );
}

/// The BACKSLASH forms, Windows-only — and the two are NOT the same thing,
/// which the first version of this test got wrong (reviewer, 2026-09-03):
///
///   * `\\host\\share` (two leading) is a REAL UNC path: `Prefix(UNC) + ..`.
///     This is the form the block header calls "base discarded", and it had NO
///     integration coverage at all — only a `cfg(windows)` unit test that never
///     touches libgit2. It is the escape the finding is named after, so it is
///     now driven through `list_submodules` end to end.
///   * `\host\share` (one leading) is NOT UNC. On Windows it parses as
///     `RootDir + Normal + Normal` and joins to `<drive>:\host\share` — a rooted
///     escape, the same class as `/Windows/System32` above.
///
/// **Windows-only by necessity:** on POSIX a lone `\` is an ordinary filename
/// character, so `\host\share` is a single `Normal` component and is
/// legitimately CONTAINED. Asserting `None` there would fail, and CI runs this
/// crate on ubuntu and macos as well as windows.
#[cfg(windows)]
#[test]
fn list_submodules_nulls_windows_backslash_paths() {
    for (raw, why) in [
        (r"\\attacker.example\share", "a real UNC path (Prefix(UNC))"),
        (
            r"\attacker.example\share",
            "a rooted backslash path (RootDir)",
        ),
    ] {
        let dir = super_with_gitmodules(raw);
        let row = only_submodule_row(dir.path());
        assert_eq!(
            row.abs_path, None,
            "{why} must yield abs_path: None — input {raw:?}"
        );
    }
}

/// The POSIX half of the case above: a lone backslash is a legal filename
/// character there, so the same string is ONE `Normal` component and stays
/// contained. Asserting it keeps the platform split honest rather than
/// silently untested — the `cfg` above is a real behavioural difference, not a
/// convenience.
#[cfg(unix)]
#[test]
fn list_submodules_keeps_backslash_filename_contained_on_unix() {
    let dir = super_with_gitmodules(r"\attacker.example\share");
    let row = only_submodule_row(dir.path());
    assert!(
        row.abs_path.is_some(),
        "on unix a backslash is a filename char, so this path is contained: {row:?}"
    );
}
