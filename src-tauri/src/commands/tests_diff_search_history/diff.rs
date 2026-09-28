//! Working-dir / commit / compare / image diff command inners.

use super::*;

// ============================================================ working-dir diff

/// Unstaged vs staged diff of the same file: the modification shows as an
/// added/changed line on the unstaged side; once staged, the staged side
/// carries it and the unstaged side is clean.
#[test]
fn workdir_file_diff_unstaged_then_staged() {
    let state = AppState::default();
    let (dir, id, _c0) = fixture_repo(&state);
    std::fs::write(dir.path().join("a.txt"), "base\nADDED\n").expect("write");

    let unstaged = block_on(get_workdir_file_diff_inner(
        &state,
        &id,
        "a.txt".into(),
        None,
        false,
        false,
        false,
    ))
    .expect("unstaged diff");
    assert_eq!(unstaged.path, "a.txt");
    assert!(!unstaged.binary);
    assert!(
        unstaged
            .hunks
            .iter()
            .flat_map(|h| &h.lines)
            .any(|l| l.content == "ADDED"),
        "unstaged diff must contain the added line"
    );

    block_on(stage_inner(&state, &id, vec!["a.txt".into()])).expect("stage");
    let staged = block_on(get_workdir_file_diff_inner(
        &state,
        &id,
        "a.txt".into(),
        None,
        true,
        false,
        false,
    ))
    .expect("staged diff");
    assert!(
        staged
            .hunks
            .iter()
            .flat_map(|h| &h.lines)
            .any(|l| l.content == "ADDED"),
        "staged (HEAD vs index) diff must carry the added line"
    );
}

/// intraline=true annotates the changed sub-range on a modified line with
/// `spans`; intraline=false leaves them empty (wire-invisible).
#[test]
fn workdir_file_diff_intraline_spans() {
    let state = AppState::default();
    let (dir, id, _c0) = fixture_repo(&state);
    write_stage_commit(&state, &id, dir.path(), "i.txt", "hello world\n", "seed i");
    std::fs::write(dir.path().join("i.txt"), "hello brave world\n").expect("write");

    let on = block_on(get_workdir_file_diff_inner(
        &state,
        &id,
        "i.txt".into(),
        None,
        false,
        false,
        true,
    ))
    .expect("intraline on");
    let any_span = on
        .hunks
        .iter()
        .flat_map(|h| &h.lines)
        .any(|l| !l.spans.is_empty());
    assert!(
        any_span,
        "a paired modified line must carry intraline spans"
    );

    let off = block_on(get_workdir_file_diff_inner(
        &state,
        &id,
        "i.txt".into(),
        None,
        false,
        false,
        false,
    ))
    .expect("intraline off");
    assert!(
        off.hunks
            .iter()
            .flat_map(|h| &h.lines)
            .all(|l| l.spans.is_empty()),
        "intraline=false must emit no spans"
    );
}

/// A workdir diff for an unknown repo is NoRepo (failure path).
#[test]
fn workdir_file_diff_no_repo() {
    let state = AppState::default();
    let err = block_on(get_workdir_file_diff_inner(
        &state,
        MISSING_ID,
        "a.txt".into(),
        None,
        false,
        false,
        false,
    ))
    .expect_err("no repo");
    assert!(matches!(err, AppError::NoRepo), "{err:?}");
}

// ============================================================ commit diff

/// get_commit_diff on the ROOT commit (no parent) lists its whole tree as
/// additions; get_commit_file_diff returns that file's hunks. A malformed oid
/// is a clean Git error.
#[test]
fn commit_diff_root_and_file_and_bad_oid() {
    let state = AppState::default();
    let (dir, id, c0) = fixture_repo(&state);

    let cd: CommitDiff =
        block_on(get_commit_diff_inner(&state, &id, c0.clone())).expect("root diff");
    assert_eq!(cd.details.oid, c0);
    assert!(cd.details.parents.is_empty(), "root commit has no parent");
    assert!(
        cd.files.iter().any(|f| f.path == "a.txt"),
        "a.txt is in the root tree"
    );

    let fd: FileDiff = block_on(get_commit_file_diff_inner(
        &state,
        &id,
        c0.clone(),
        "a.txt".into(),
        None,
        false,
        false,
    ))
    .expect("root file diff");
    assert_eq!(fd.path, "a.txt");
    assert!(
        fd.hunks
            .iter()
            .flat_map(|h| &h.lines)
            .any(|l| l.content == "base"),
        "root file diff shows the added content"
    );

    let err = block_on(get_commit_diff_inner(&state, &id, "not-a-valid-oid".into()))
        .expect_err("bad oid");
    assert!(matches!(err, AppError::Git(_)), "{err:?}");
    let _ = dir;
}

/// A second commit diffs against its first parent — only the changed file
/// appears, not the untouched root file.
#[test]
fn commit_diff_against_first_parent() {
    let state = AppState::default();
    let (dir, id, _c0) = fixture_repo(&state);
    let c1 = write_stage_commit(&state, &id, dir.path(), "b.txt", "second\n", "C1").oid;

    let cd = block_on(get_commit_diff_inner(&state, &id, c1.clone())).expect("diff");
    assert_eq!(cd.details.parents.len(), 1);
    let paths: Vec<&str> = cd.files.iter().map(|f| f.path.as_str()).collect();
    assert_eq!(
        paths,
        vec!["b.txt"],
        "only the file introduced by C1: {paths:?}"
    );
}

// ============================================================ compare with HEAD

/// compare_with_head against the root commit reports the net tree delta; the
/// per-file variant returns that file's hunks. Bad oid → Git.
#[test]
fn compare_with_head_happy_and_bad_oid() {
    let state = AppState::default();
    let (dir, id, c0) = fixture_repo(&state);
    write_stage_commit(&state, &id, dir.path(), "c.txt", "extra\n", "C1");

    let cmp: CompareDiff =
        block_on(compare_with_head_inner(&state, &id, c0.clone())).expect("compare");
    // HEAD(new) has c.txt that the root(old) lacks → c.txt appears in the delta.
    assert!(
        cmp.files.iter().any(|f| f.path == "c.txt"),
        "delta must include c.txt"
    );

    let fd = block_on(compare_with_head_file_diff_inner(
        &state,
        &id,
        c0,
        "c.txt".into(),
        None,
        false,
        false,
    ))
    .expect("compare file diff");
    assert_eq!(fd.path, "c.txt");

    let err = block_on(compare_with_head_inner(&state, &id, "zzz".into())).expect_err("bad oid");
    assert!(matches!(err, AppError::Git(_)), "{err:?}");
    let _ = dir;
}

// ============================================================ image diff

const PNG_BYTES: &[u8] = b"\x89PNG\r\n\x1a\nfake-image-payload";

/// get_image_diff over the four side-resolution cases: an untracked image is
/// added (new only); a committed-then-deleted image is deleted (old only); a
/// 0-byte image side is treated as ABSENT (None, not empty base64); a
/// non-image extension resolves to `application/octet-stream`.
#[test]
fn image_diff_added_deleted_zero_byte_and_non_image() {
    let state = AppState::default();
    let (dir, id, _c0) = fixture_repo(&state);

    // ADDED: png in the workdir, not staged/tracked.
    std::fs::write(dir.path().join("logo.png"), PNG_BYTES).expect("write png");
    let added = block_on(get_image_diff_inner(
        &state,
        &id,
        ImageDiffRequest::Workdir {
            path: "logo.png".into(),
            orig_path: None,
            staged: false,
        },
    ))
    .expect("added image");
    assert!(added.old.is_none(), "no old side for an added image");
    let new = added.new.expect("new side present");
    assert_eq!(new.mime, "image/png");
    assert_eq!(new.byte_len as usize, PNG_BYTES.len());

    // DELETED: commit the png, then delete it from the workdir (unstaged).
    write_stage_commit(&state, &id, dir.path(), "logo.png", "PNGDATA", "add png");
    std::fs::remove_file(dir.path().join("logo.png")).expect("rm png");
    let deleted = block_on(get_image_diff_inner(
        &state,
        &id,
        ImageDiffRequest::Workdir {
            path: "logo.png".into(),
            orig_path: None,
            staged: false,
        },
    ))
    .expect("deleted image");
    assert!(deleted.old.is_some(), "old side (index blob) present");
    assert!(deleted.new.is_none(), "deleted → no new side");

    // 0-BYTE → ABSENT: an empty file yields no renderable side.
    std::fs::write(dir.path().join("empty.png"), b"").expect("write empty");
    let zero = block_on(get_image_diff_inner(
        &state,
        &id,
        ImageDiffRequest::Workdir {
            path: "empty.png".into(),
            orig_path: None,
            staged: false,
        },
    ))
    .expect("zero byte");
    assert!(
        zero.old.is_none() && zero.new.is_none(),
        "0-byte side is absent, not empty base64"
    );
    assert!(!zero.new_too_large, "0-byte is not over-cap");

    // NON-IMAGE extension: octet-stream mime (a.txt exists from the fixture).
    std::fs::write(dir.path().join("a.txt"), "changed\n").expect("write txt");
    let non = block_on(get_image_diff_inner(
        &state,
        &id,
        ImageDiffRequest::Workdir {
            path: "a.txt".into(),
            orig_path: None,
            staged: false,
        },
    ))
    .expect("non image");
    if let Some(side) = non.new {
        assert_eq!(side.mime, "application/octet-stream");
    }
}

/// A `..` path in an image request is rejected up front (Other/invalid path),
/// never reaching the filesystem.
#[test]
fn image_diff_rejects_traversal_path() {
    let state = AppState::default();
    let (_dir, id, _c0) = fixture_repo(&state);
    let err = block_on(get_image_diff_inner(
        &state,
        &id,
        ImageDiffRequest::Workdir {
            path: "../evil.png".into(),
            orig_path: None,
            staged: false,
        },
    ))
    .expect_err("traversal rejected");
    assert!(
        matches!(err, AppError::Other(_) | AppError::InvalidName(_)),
        "{err:?}"
    );
}
