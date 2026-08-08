//! P61b — image-diff integration tests exercising `get_image_diff` end-to-end
//! from OUTSIDE the crate against a real scratch repo (contract
//! `docs/contracts/P61-diff-quality.md`, P61b acceptance §1).
//!
//! The bulk of P61b behaviour (all three contexts, add/delete/rename in the
//! commit context, the 8 MiB over-cap path, the MIME map, and base64
//! round-tripping) is already covered by the in-module unit tests in
//! `src/git/image_diff.rs`, which themselves build real git2 repos. This file
//! deliberately does NOT duplicate those; it fills the two genuine gaps those
//! tests leave — both realistic user scenarios whose code branches are
//! otherwise unexercised:
//!   1. a STAGED image add before the first commit (unborn HEAD -> old None);
//!   2. a WORKDIR rename resolving the OLD side via `orig_path`.
//!
//! It also serves as the public-API / integration-boundary smoke for the
//! command layer's call site.
//!
//! The backend moves raw bytes and never decodes images, so (per the same
//! convention as the in-module fixtures) the "image" blobs are distinct byte
//! sequences carrying the PNG signature rather than CRC-valid PNGs — validity
//! only matters to the native `img` renderer, which is a USER CHECKPOINT.
//!
//! HARD RULE: scratch repos live on D: via `common::init_repo()`.

mod common;

use bonsai_core::git::commit::create_commit;
use bonsai_core::git::image_diff::{get_image_diff, ImageDiffRequest};
use bonsai_core::git::stage::stage_paths;
use common::init_repo;

macro_rules! require_git {
    () => {
        if !common::have_git() {
            eprintln!("skipping: `git` CLI not found on PATH");
            return;
        }
    };
}

// Distinct blobs with the PNG signature (see module note: byte-agnostic backend).
const RED: &[u8] = b"\x89PNG\r\n\x1a\nRED-image-bytes-v1";
const GREEN: &[u8] = b"\x89PNG\r\n\x1a\nGREEN-image-bytes-v2-different-length";

/// Minimal RFC 4648 base64 decoder (standard alphabet, `=` padding), to prove
/// the resolved side round-trips to the original blob bytes. Mirrors the
/// in-module test decoder; the crate's encoder is private to `image_diff.rs`.
fn base64_decode(s: &str) -> Vec<u8> {
    fn val(c: u8) -> u32 {
        match c {
            b'A'..=b'Z' => (c - b'A') as u32,
            b'a'..=b'z' => (c - b'a' + 26) as u32,
            b'0'..=b'9' => (c - b'0' + 52) as u32,
            b'+' => 62,
            b'/' => 63,
            _ => 0,
        }
    }
    let bytes: Vec<u8> = s.bytes().collect();
    let mut out = Vec::new();
    for group in bytes.chunks(4) {
        let pad = group.iter().filter(|&&c| c == b'=').count();
        let mut n = 0u32;
        for &c in group {
            n = (n << 6) | val(c);
        }
        out.push((n >> 16) as u8);
        if pad < 2 {
            out.push((n >> 8) as u8);
        }
        if pad < 1 {
            out.push(n as u8);
        }
    }
    out
}

/// Write `bytes` to `path`, stage it, and commit; returns the commit oid.
fn commit_blob(p: &std::path::Path, path: &str, bytes: &[u8], msg: &str) -> String {
    std::fs::write(p.join(path), bytes).expect("write blob");
    stage_paths(p, &[path.to_string()]).expect("stage");
    create_commit(p, msg, None, false).expect("commit").oid
}

/// Gap 1: a STAGED image add on an unborn HEAD (first-commit staging). The
/// staged-workdir branch hits `repo.head()` -> UnbornBranch and must yield
/// `old = None` with `new` = the staged blob. No committed history exists yet.
#[test]
fn workdir_staged_add_on_unborn_head_old_none() {
    require_git!();
    let dir = init_repo(); // git init -b main, no commits => unborn HEAD
    let p = dir.path();

    std::fs::write(p.join("logo.png"), GREEN).expect("write image");
    stage_paths(p, &["logo.png".into()]).expect("stage add");

    let diff = get_image_diff(
        p,
        &ImageDiffRequest::Workdir {
            path: "logo.png".into(),
            orig_path: None,
            staged: true,
        },
    )
    .expect("staged diff on unborn HEAD");

    assert!(diff.old.is_none(), "unborn HEAD => staged old is None");
    assert!(!diff.old_too_large && !diff.new_too_large);
    let new = diff.new.expect("staged new side present");
    assert_eq!(base64_decode(&new.base64), GREEN, "new = staged blob");
    assert_eq!(new.mime, "image/png");
    assert_eq!(new.byte_len as usize, GREEN.len());
}

/// Gap 2: a WORKDIR rename (unstaged) must resolve the OLD side from the index
/// at `orig_path`, not at the new path. Commit `a.png`, then in the workdir
/// remove it and write `b.png` with different bytes without staging: the OLD
/// side comes from the index blob `a.png`, the NEW side from the workdir file
/// `b.png`.
#[test]
fn workdir_unstaged_rename_uses_orig_path() {
    require_git!();
    let dir = init_repo();
    let p = dir.path();
    commit_blob(p, "a.png", RED, "add a.png");

    // Workdir rename a.png -> b.png with an edit; leave the index untouched so
    // `a.png` (RED) is still the stage-0 index blob.
    std::fs::remove_file(p.join("a.png")).expect("rm a.png in workdir");
    std::fs::write(p.join("b.png"), GREEN).expect("write b.png in workdir");

    let diff = get_image_diff(
        p,
        &ImageDiffRequest::Workdir {
            path: "b.png".into(),
            orig_path: Some("a.png".into()),
            staged: false,
        },
    )
    .expect("workdir rename diff");

    assert_eq!(
        base64_decode(&diff.old.expect("old via orig_path").base64),
        RED,
        "old = index blob at orig_path a.png"
    );
    assert_eq!(
        base64_decode(&diff.new.expect("new = workdir b.png").base64),
        GREEN,
        "new = workdir file at b.png"
    );

    // Sanity: without orig_path the old side falls back to the (untracked) new
    // path, which is absent from the index -> old None.
    let no_orig = get_image_diff(
        p,
        &ImageDiffRequest::Workdir {
            path: "b.png".into(),
            orig_path: None,
            staged: false,
        },
    )
    .expect("workdir diff without orig_path");
    assert!(
        no_orig.old.is_none(),
        "no orig_path => old None (b.png absent from index)"
    );
}
