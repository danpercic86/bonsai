//! Path-traversal (symlink-escape) guard tests for the git write/delete/read
//! paths. Kept in their own module so the already-large `stage_partial` and
//! `conflict` files are not bloated further (file-size discipline).
//!
//! Each test exercises [`ensure_within_workdir`] — directly, or through the
//! public `discard_paths_force` / `stage_partial` / `resolve_conflict_text` —
//! proving a path that escapes the workdir via a symlinked ANCESTOR is rejected
//! and the external file is left untouched, while legitimate paths pass
//! unchanged. Symlink creation needs privilege on Windows;
//! [`crate::testutil::make_dir_symlink_or_skip`] falls back to an NTFS junction
//! (no privilege) and otherwise signals a skip, so the guard is always exercised
//! on unix (CI) and usually on Windows too.

use crate::error::AppError;
use crate::git::conflict::{list_conflicts, resolve_conflict_text};
use crate::git::diff::LineKind;
use crate::git::discard::discard_paths_force;
use crate::git::stage::ensure_within_workdir;
use crate::git::stage_partial::{stage_partial, LineSelection};
use crate::testutil::{make_dir_symlink_or_skip, scratch_dir};

/// A scratch workdir plus a sibling "outside" dir holding `secret.txt`, with
/// `<work>/link` a directory symlink -> the outside dir. `None` -> the platform
/// refused the symlink and the caller should skip. `link/secret.txt` therefore
/// resolves OUTSIDE the workdir through the symlinked ancestor `link`.
fn escape_fixture() -> Option<(tempfile::TempDir, tempfile::TempDir)> {
    let work = scratch_dir();
    let outside = scratch_dir();
    std::fs::write(outside.path().join("secret.txt"), b"SECRET").expect("write secret");
    if !make_dir_symlink_or_skip(outside.path(), &work.path().join("link")) {
        eprintln!("skipping: dir symlink creation not permitted");
        return None;
    }
    Some((work, outside))
}

// ------------------------------------------------- ensure_within_workdir (unit)

/// Legitimate paths (existing nested, not-yet-created nested, workdir leaf, deep
/// not-yet-created) all pass and return the plain `workdir.join(rel)` — no
/// symlinks needed, so this runs on every platform.
#[test]
fn ensure_within_accepts_legit_paths() {
    let dir = scratch_dir();
    let wd = dir.path();
    std::fs::create_dir_all(wd.join("a/b")).expect("mkdir");
    std::fs::write(wd.join("a/b/c.txt"), b"x").expect("write");

    for rel in ["a/b/c.txt", "a/b/new.txt", "top.txt", "x/y/z.txt"] {
        let got =
            ensure_within_workdir(wd, rel).unwrap_or_else(|e| panic!("must accept {rel:?}: {e:?}"));
        assert_eq!(got, wd.join(rel), "returns the plain joined path for {rel:?}");
    }
}

/// A LEAF symlink (even one pointing outside) is returned AS the link path,
/// unresolved: the guard only inspects the parent (the real workdir), so the
/// caller's op acts on the link itself, not its target.
#[test]
fn ensure_within_returns_leaf_symlink_unresolved() {
    let work = scratch_dir();
    let outside = scratch_dir();
    if !make_dir_symlink_or_skip(outside.path(), &work.path().join("leaf")) {
        eprintln!("skipping: dir symlink creation not permitted");
        return;
    }
    let got = ensure_within_workdir(work.path(), "leaf").expect("leaf symlink allowed as a link");
    assert_eq!(got, work.path().join("leaf"));
}

/// A path whose ANCESTOR is a symlink escaping the workdir is rejected — whether
/// or not the leaf currently exists at the target — and the guard performs no
/// writes, so the external file is untouched.
#[test]
fn ensure_within_rejects_symlinked_ancestor_escape() {
    let Some((work, outside)) = escape_fixture() else {
        return;
    };
    for rel in ["link/secret.txt", "link/new.txt"] {
        let err = ensure_within_workdir(work.path(), rel)
            .expect_err("escaping ancestor must be rejected");
        assert!(
            matches!(err, AppError::Other(ref m) if m.contains("resolves outside")),
            "{rel:?} -> {err:?}"
        );
    }
    assert_eq!(
        std::fs::read(outside.path().join("secret.txt")).expect("read"),
        b"SECRET",
        "external file must be untouched by the guard"
    );
}

// --------------------------------------------------------- per-call-site wiring

/// discard: an untracked path escaping via a symlinked ancestor is refused and
/// the external file is NOT deleted (highest-impact site — a raw `remove_file`).
#[test]
fn discard_force_rejects_symlinked_ancestor_escape() {
    let Some((work, outside)) = escape_fixture() else {
        return;
    };
    git2::Repository::init(work.path()).expect("init");

    let err = discard_paths_force(work.path(), &["link/secret.txt".to_string()])
        .expect_err("escaping untracked path must be rejected");
    match err {
        AppError::Git(m) => assert!(m.contains("resolves outside"), "got: {m}"),
        other => panic!("expected Git, got {other:?}"),
    }
    assert!(
        outside.path().join("secret.txt").exists(),
        "external file must NOT be deleted"
    );
    assert_eq!(
        std::fs::read(outside.path().join("secret.txt")).expect("read"),
        b"SECRET",
        "external file content must be untouched"
    );
}

/// stage_partial: an escaping path is rejected before any workdir blob read, so
/// no external content leaks into the index.
#[test]
fn stage_partial_rejects_symlinked_ancestor_escape() {
    let Some((work, outside)) = escape_fixture() else {
        return;
    };
    git2::Repository::init(work.path()).expect("init");

    let sel = vec![LineSelection {
        kind: LineKind::Add,
        old_no: None,
        new_no: Some(1),
    }];
    let err = stage_partial(work.path(), "link/secret.txt", None, &sel)
        .expect_err("escaping path must be rejected");
    assert!(
        matches!(err, AppError::Other(ref m) if m.contains("resolves outside")),
        "got: {err:?}"
    );
    assert_eq!(
        std::fs::read(outside.path().join("secret.txt")).expect("read"),
        b"SECRET",
        "external file must be untouched"
    );
}

/// conflict: a crafted index conflict at an escaping path passes `find_conflict`
/// but is then refused by the guard (mapped to `invalidName`), and the external
/// file is NOT overwritten. The conflict is built directly in the index (stage
/// 2 + 3) so control reaches the guard.
#[test]
fn resolve_conflict_text_rejects_symlinked_ancestor_escape() {
    let work = scratch_dir();
    let wd = work.path();
    let repo = git2::Repository::init(wd).expect("init");

    // Craft an index conflict at "link/secret.txt": stage-2 (ours) + stage-3
    // (theirs) entries. The stage is encoded in the high bits of `flags`.
    let our = repo.blob(b"ours\n").expect("our blob");
    let their = repo.blob(b"theirs\n").expect("their blob");
    let mut index = repo.index().expect("index");
    let entry = |stage: u16, id| git2::IndexEntry {
        ctime: git2::IndexTime::new(0, 0),
        mtime: git2::IndexTime::new(0, 0),
        dev: 0,
        ino: 0,
        mode: 0o100644,
        uid: 0,
        gid: 0,
        file_size: 0,
        id,
        flags: stage << 12,
        flags_extended: 0,
        path: b"link/secret.txt".to_vec(),
    };
    index.add(&entry(2, our)).expect("add ours");
    index.add(&entry(3, their)).expect("add theirs");
    index.write().expect("write index");
    drop(index);
    // Crafting sanity (runs on every platform, independent of symlink privilege).
    assert!(
        list_conflicts(wd)
            .expect("list")
            .iter()
            .any(|e| e.path == "link/secret.txt"),
        "crafted conflict must be listed"
    );

    let outside = scratch_dir();
    std::fs::write(outside.path().join("secret.txt"), b"SECRET").expect("write secret");
    if !make_dir_symlink_or_skip(outside.path(), &wd.join("link")) {
        eprintln!("skipping: dir symlink creation not permitted");
        return;
    }

    let err = resolve_conflict_text(wd, "link/secret.txt", "PWNED")
        .expect_err("escaping conflict path must be rejected");
    assert!(matches!(err, AppError::InvalidName(_)), "got: {err:?}");
    assert_eq!(
        std::fs::read(outside.path().join("secret.txt")).expect("read"),
        b"SECRET",
        "external file must NOT be overwritten"
    );
}
