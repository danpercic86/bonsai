//! T2 Area 9 — image-diff HARDENING extensions (split from `image_diff_cli.rs`).
//!
//! End-to-end boundary + adversarial cases through `get_image_diff`: 0-byte side
//! → None (F-A9-13), non-image bytes moved verbatim, the MAX_IMAGE_BYTES cap
//! boundary vs +1, both-over-cap, unicode filename, `.svg` MIME routing,
//! subtree/submodule path → None, missing-both → None, and bare-repo error.
//! Scratch on D:. Skips (passes with a note) w/o `git`.

use bonsai_core::git::commit::create_commit;
use bonsai_core::git::image_diff::{get_image_diff, ImageDiffRequest, MAX_IMAGE_BYTES};
use bonsai_core::git::stage::stage_paths;
use crate::common;
use crate::common::init_repo;

macro_rules! require_git {
    () => {
        if !common::have_git() {
            eprintln!("skipping: `git` CLI not found on PATH");
            return;
        }
    };
}

const RED: &[u8] = b"\x89PNG\r\n\x1a\nRED-image-bytes";

fn commit_blob(p: &std::path::Path, path: &str, bytes: &[u8], msg: &str) -> String {
    if let Some(parent) = std::path::Path::new(path).parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(p.join(parent)).expect("mkdir parent");
        }
    }
    std::fs::write(p.join(path), bytes).expect("write blob");
    stage_paths(p, &[path.to_string()]).expect("stage");
    create_commit(p, msg, None, false).expect("commit").oid
}

fn workdir_unstaged(path: &str) -> ImageDiffRequest {
    ImageDiffRequest::Workdir { path: path.into(), orig_path: None, staged: false }
}

// ------------------------------------------------------ 0-byte side → None

/// A 0-byte workdir file is `None` (absent), NOT an empty-base64 side (F-A9-13).
#[test]
fn zero_byte_workdir_side_is_none() {
    require_git!();
    let dir = init_repo();
    let p = dir.path();
    commit_blob(p, "img.png", RED, "add img");
    std::fs::write(p.join("img.png"), b"").expect("truncate to 0 bytes");

    let diff = get_image_diff(p, &workdir_unstaged("img.png")).expect("diff");
    assert!(diff.new.is_none(), "0-byte new side is None (not empty base64)");
    assert!(!diff.new_too_large, "0-byte is absent, not over-cap");
    assert!(diff.old.is_some(), "old (index RED) still present");
}

// -------------------------------------------------- non-image bytes verbatim

/// The backend moves bytes without decoding: non-image content in a `.png`
/// round-trips verbatim with an image MIME (validity is the renderer's problem).
#[test]
fn non_image_bytes_in_png_moved_verbatim() {
    require_git!();
    let dir = init_repo();
    let p = dir.path();
    let junk = b"this is definitely not a PNG \x00\x01\x02 plain text";
    commit_blob(p, "fake.png", junk, "add fake");

    let diff = get_image_diff(p, &ImageDiffRequest::Workdir {
        path: "fake.png".into(), orig_path: None, staged: true,
    }).expect("diff");
    let side = diff.new.expect("staged==index side present");
    assert_eq!(side.mime, "image/png", "MIME from extension, not content sniffing");
    assert_eq!(side.byte_len as usize, junk.len(), "raw length preserved");
}

// -------------------------------------------------- MAX_IMAGE_BYTES boundary

/// Exactly at the cap → encoded; one byte over → None with `too_large`.
#[test]
fn cap_boundary_at_vs_over() {
    require_git!();
    // At cap: Some, not too_large.
    let dir = init_repo();
    let p = dir.path();
    commit_blob(p, "at.png", RED, "seed");
    std::fs::write(p.join("at.png"), vec![7u8; MAX_IMAGE_BYTES]).expect("write at-cap");
    let at = get_image_diff(p, &workdir_unstaged("at.png")).expect("diff at");
    assert!(at.new.is_some() && !at.new_too_large, "exactly cap → encoded side");
    assert_eq!(at.new.unwrap().byte_len as usize, MAX_IMAGE_BYTES);

    // Over cap: None + too_large.
    std::fs::write(p.join("at.png"), vec![7u8; MAX_IMAGE_BYTES + 1]).expect("write over-cap");
    let over = get_image_diff(p, &workdir_unstaged("at.png")).expect("diff over");
    assert!(over.new.is_none() && over.new_too_large, "cap+1 → None + too_large");
}

// ------------------------------------------------------- both sides over cap

/// Both sides over-cap → both None, both `too_large`.
#[test]
fn both_sides_over_cap() {
    require_git!();
    let dir = init_repo();
    let p = dir.path();
    // Commit an over-cap blob, then modify to another over-cap blob in workdir.
    commit_blob(p, "big.png", &vec![1u8; MAX_IMAGE_BYTES + 1], "big old");
    std::fs::write(p.join("big.png"), vec![2u8; MAX_IMAGE_BYTES + 2]).expect("big new");

    let diff = get_image_diff(p, &ImageDiffRequest::Workdir {
        path: "big.png".into(), orig_path: None, staged: false,
    }).expect("diff");
    assert!(diff.old.is_none() && diff.old_too_large, "old over-cap");
    assert!(diff.new.is_none() && diff.new_too_large, "new over-cap");
}

// ----------------------------------------------------------- unicode filename

/// A unicode image filename resolves normally.
#[test]
fn unicode_filename_resolves() {
    require_git!();
    let dir = init_repo();
    let p = dir.path();
    let name = "café-日本.png";
    commit_blob(p, name, RED, "add unicode img");

    let diff = get_image_diff(p, &ImageDiffRequest::Workdir {
        path: name.into(), orig_path: None, staged: true,
    }).expect("diff");
    assert_eq!(diff.path, name, "path echoed");
    assert!(diff.new.is_some(), "unicode-named blob resolved");
}

// --------------------------------------------------------- svg MIME routing

/// `.svg` is NOT in the raster-image set — `get_image_diff` reports an
/// octet-stream MIME (svg-as-TEXT routing is a command-layer decision, so
/// image-diff never treats svg as a renderable raster).
#[test]
fn svg_gets_octet_stream_mime() {
    require_git!();
    let dir = init_repo();
    let p = dir.path();
    commit_blob(p, "icon.svg", b"<svg></svg>", "add svg");

    let diff = get_image_diff(p, &ImageDiffRequest::Workdir {
        path: "icon.svg".into(), orig_path: None, staged: true,
    }).expect("diff");
    // The bytes still move (image_diff is byte-agnostic), but the MIME marks it
    // as non-raster — the command layer routes svg to the text differ.
    assert_eq!(diff.new.expect("side").mime, "application/octet-stream",
        "svg is not classified as a raster image here");
}

// ------------------------------------------------ subtree / missing → None

/// A path that resolves to a SUBTREE (not a blob) yields None on both sides.
#[test]
fn subtree_path_is_none() {
    require_git!();
    let dir = init_repo();
    let p = dir.path();
    let oid = commit_blob(p, "assets/logo.png", RED, "add nested");

    let diff = get_image_diff(p, &ImageDiffRequest::Commit {
        oid, path: "assets".into(), orig_path: None,
    }).expect("diff");
    assert!(diff.old.is_none() && diff.new.is_none(), "a subtree path is not a blob → None/None");
}

/// A path absent from both index and workdir → both sides None.
#[test]
fn missing_in_index_and_workdir_is_none() {
    require_git!();
    let dir = init_repo();
    let p = dir.path();
    commit_blob(p, "present.png", RED, "seed");

    let diff = get_image_diff(p, &workdir_unstaged("ghost.png")).expect("diff");
    assert!(diff.old.is_none() && diff.new.is_none(), "unknown path → None/None");
}

// ----------------------------------------------------------- bare repo err

/// A bare repo (no workdir) surfaces a clean `AppError`, never a panic.
#[test]
fn bare_repo_errors_cleanly() {
    require_git!();
    let dir = common::scratch_dir();
    common::git(dir.path(), &["init", "--bare", "-b", "main"]);
    match get_image_diff(dir.path(), &workdir_unstaged("x.png")) {
        Err(_) => {}
        Ok(d) => panic!("bare repo must error, got {d:?}"),
    }
}
