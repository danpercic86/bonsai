use super::*;
use crate::git::commit::create_commit;
use crate::git::stage::stage_paths;

// Distinct "image" byte blobs. Not required to be valid PNGs — these tests
// only move bytes and compare identity — but they use the PNG signature so
// they read like real fixtures.
const RED: &[u8] = b"\x89PNG\r\n\x1a\nRED-image-bytes-v1";
const GREEN: &[u8] = b"\x89PNG\r\n\x1a\nGREEN-image-bytes-v2-longer";

/// Test-only RFC 4648 decoder, to prove the encoder round-trips.
fn base64_decode(s: &str) -> Vec<u8> {
    fn val(c: u8) -> Option<u32> {
        match c {
            b'A'..=b'Z' => Some((c - b'A') as u32),
            b'a'..=b'z' => Some((c - b'a' + 26) as u32),
            b'0'..=b'9' => Some((c - b'0' + 52) as u32),
            b'+' => Some(62),
            b'/' => Some(63),
            _ => None,
        }
    }
    let bytes: Vec<u8> = s.bytes().collect();
    let mut out = Vec::new();
    for group in bytes.chunks(4) {
        let pad = group.iter().filter(|&&c| c == b'=').count();
        let mut n = 0u32;
        for &c in group {
            n = (n << 6) | val(c).unwrap_or(0);
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

fn init_scratch() -> tempfile::TempDir {
    let dir = crate::testutil::scratch_dir();
    let repo = git2::Repository::init(dir.path()).expect("init repo");
    let mut cfg = repo.config().expect("config");
    cfg.set_str("user.name", "Test User").expect("name");
    cfg.set_str("user.email", "test@example.com").expect("email");
    cfg.set_bool("core.autocrlf", false).expect("autocrlf");
    dir
}

/// Write `bytes` to `path`, stage it, and commit; returns the commit oid.
fn commit_bytes(p: &Path, path: &str, bytes: &[u8], msg: &str) -> String {
    std::fs::write(p.join(path), bytes).expect("write");
    stage_paths(p, &[path.to_string()]).expect("stage");
    create_commit(p, msg, None, false).expect("commit").oid
}

#[test]
fn base64_rfc4648_vectors() {
    assert_eq!(base64_encode(b""), "");
    assert_eq!(base64_encode(b"f"), "Zg==");
    assert_eq!(base64_encode(b"fo"), "Zm8=");
    assert_eq!(base64_encode(b"foo"), "Zm9v");
    assert_eq!(base64_encode(b"foob"), "Zm9vYg==");
    assert_eq!(base64_encode(b"fooba"), "Zm9vYmE=");
    assert_eq!(base64_encode(b"foobar"), "Zm9vYmFy");
}

#[test]
fn base64_roundtrips_all_byte_values() {
    // Every byte value 0..=255, plus a few extra to exercise all 3 tail
    // lengths (258 % 3 == 0, 259 % 3 == 1, 260 % 3 == 2).
    for extra in 0..3usize {
        let data: Vec<u8> = (0..=255u8).chain(std::iter::repeat_n(7u8, extra)).collect();
        let encoded = base64_encode(&data);
        assert_eq!(encoded.len() % 4, 0, "base64 length is a multiple of 4");
        assert_eq!(base64_decode(&encoded), data, "roundtrip (extra={extra})");
    }
}

#[test]
fn mime_from_extension_map() {
    assert_eq!(mime_from_ext("a/logo.png"), "image/png");
    assert_eq!(mime_from_ext("a.JPG"), "image/jpeg");
    assert_eq!(mime_from_ext("a.jpeg"), "image/jpeg");
    assert_eq!(mime_from_ext("a.gif"), "image/gif");
    assert_eq!(mime_from_ext("a.webp"), "image/webp");
    assert_eq!(mime_from_ext("a.bmp"), "image/bmp");
    assert_eq!(mime_from_ext("a.ico"), "image/x-icon");
    assert_eq!(mime_from_ext("a.avif"), "image/avif");
    assert_eq!(mime_from_ext("a.txt"), "application/octet-stream");
    assert_eq!(mime_from_ext("noext"), "application/octet-stream");
}

#[test]
fn make_side_flags_over_cap_and_encodes() {
    // Absent -> (None, false).
    assert_eq!(make_side(None, "image/png"), (None, false));
    // Empty (0-byte) -> (None, false): absent, NOT too_large (F-A9-13).
    assert_eq!(make_side(Some(Vec::new()), "image/png"), (None, false));
    // Over cap -> (None, true).
    let big = vec![0u8; MAX_IMAGE_BYTES + 1];
    assert_eq!(make_side(Some(big), "image/png"), (None, true));
    // Present -> encoded side with correct byte_len + mime.
    let (side, too_large) = make_side(Some(RED.to_vec()), "image/png");
    assert!(!too_large);
    let side = side.expect("side present");
    assert_eq!(side.byte_len as usize, RED.len());
    assert_eq!(side.mime, "image/png");
    assert_eq!(base64_decode(&side.base64), RED);
}

/// Commit variant: root (add) -> old None; a later modify -> both sides with
/// the correct bytes.
#[test]
fn commit_variant_add_then_modify() {
    let dir = init_scratch();
    let p = dir.path();
    let c1 = commit_bytes(p, "img.png", RED, "add image");
    let c2 = commit_bytes(p, "img.png", GREEN, "modify image");

    // Root commit: added -> old None, new = RED.
    let add = get_image_diff(
        p,
        &ImageDiffRequest::Commit {
            oid: c1.clone(),
            path: "img.png".into(),
            orig_path: None,
        },
    )
    .expect("commit add diff");
    assert!(add.old.is_none(), "root add -> old None");
    assert!(!add.old_too_large && !add.new_too_large);
    let new = add.new.expect("new side present");
    assert_eq!(base64_decode(&new.base64), RED);
    assert_eq!(new.mime, "image/png");

    // Modify: old = RED, new = GREEN.
    let modify = get_image_diff(
        p,
        &ImageDiffRequest::Commit {
            oid: c2,
            path: "img.png".into(),
            orig_path: None,
        },
    )
    .expect("commit modify diff");
    assert_eq!(base64_decode(&modify.old.expect("old").base64), RED);
    assert_eq!(base64_decode(&modify.new.expect("new").base64), GREEN);
}

/// Commit variant: a delete commit -> new None, old present.
#[test]
fn commit_variant_delete_gives_new_none() {
    let dir = init_scratch();
    let p = dir.path();
    commit_bytes(p, "img.png", RED, "add image");
    std::fs::remove_file(p.join("img.png")).expect("rm");
    stage_paths(p, &["img.png".into()]).expect("stage deletion");
    let del = create_commit(p, "delete image", None, false).expect("commit").oid;

    let diff = get_image_diff(
        p,
        &ImageDiffRequest::Commit {
            oid: del,
            path: "img.png".into(),
            orig_path: None,
        },
    )
    .expect("commit delete diff");
    assert_eq!(base64_decode(&diff.old.expect("old").base64), RED);
    assert!(diff.new.is_none(), "deleted -> new None");
}

/// Commit variant: a rename resolves the OLD side via `orig_path`; without
/// it the old side is absent (the new name is not in the parent tree).
#[test]
fn commit_variant_rename_uses_orig_path() {
    let dir = init_scratch();
    let p = dir.path();
    commit_bytes(p, "a.png", RED, "add a.png");
    // Rename a.png -> b.png (same bytes).
    std::fs::remove_file(p.join("a.png")).expect("rm a");
    std::fs::write(p.join("b.png"), RED).expect("write b");
    stage_paths(p, &["a.png".into(), "b.png".into()]).expect("stage rename");
    let c = create_commit(p, "rename a->b", None, false).expect("commit").oid;

    // With orig_path -> old resolved from a.png in the parent tree.
    let with = get_image_diff(
        p,
        &ImageDiffRequest::Commit {
            oid: c.clone(),
            path: "b.png".into(),
            orig_path: Some("a.png".into()),
        },
    )
    .expect("rename diff");
    assert_eq!(base64_decode(&with.old.expect("old via orig_path").base64), RED);
    assert!(with.new.is_some(), "new = b.png present");

    // Without orig_path -> old None (b.png absent from the parent tree).
    let without = get_image_diff(
        p,
        &ImageDiffRequest::Commit {
            oid: c,
            path: "b.png".into(),
            orig_path: None,
        },
    )
    .expect("rename diff no orig");
    assert!(without.old.is_none(), "no orig_path -> old None");
}

/// Workdir variant (unstaged): old = index (committed) blob, new = the
/// edited workdir file.
#[test]
fn workdir_variant_unstaged() {
    let dir = init_scratch();
    let p = dir.path();
    commit_bytes(p, "img.png", RED, "add image"); // index + HEAD = RED
    std::fs::write(p.join("img.png"), GREEN).expect("edit workdir"); // workdir = GREEN

    let diff = get_image_diff(
        p,
        &ImageDiffRequest::Workdir {
            path: "img.png".into(),
            orig_path: None,
            staged: false,
        },
    )
    .expect("workdir diff");
    assert_eq!(base64_decode(&diff.old.expect("old = index").base64), RED);
    assert_eq!(base64_decode(&diff.new.expect("new = workdir").base64), GREEN);
}

/// Workdir variant (staged): old = HEAD blob, new = index blob.
#[test]
fn workdir_variant_staged() {
    let dir = init_scratch();
    let p = dir.path();
    commit_bytes(p, "img.png", RED, "add image"); // HEAD = RED
    std::fs::write(p.join("img.png"), GREEN).expect("edit");
    stage_paths(p, &["img.png".into()]).expect("stage"); // index = GREEN

    let diff = get_image_diff(
        p,
        &ImageDiffRequest::Workdir {
            path: "img.png".into(),
            orig_path: None,
            staged: true,
        },
    )
    .expect("staged diff");
    assert_eq!(base64_decode(&diff.old.expect("old = HEAD").base64), RED);
    assert_eq!(base64_decode(&diff.new.expect("new = index").base64), GREEN);
}

/// Over-cap side -> None + `*_too_large`. Exercised on the workdir-new side.
#[test]
fn over_cap_side_is_none_and_flagged() {
    let dir = init_scratch();
    let p = dir.path();
    commit_bytes(p, "img.png", RED, "add small image"); // index = small RED
    // Workdir file over the cap.
    std::fs::write(p.join("img.png"), vec![0u8; MAX_IMAGE_BYTES + 1]).expect("write big");

    let diff = get_image_diff(
        p,
        &ImageDiffRequest::Workdir {
            path: "img.png".into(),
            orig_path: None,
            staged: false,
        },
    )
    .expect("over-cap diff");
    assert!(diff.old.is_some(), "small old side present");
    assert!(!diff.old_too_large);
    assert!(diff.new.is_none(), "over-cap new -> None");
    assert!(diff.new_too_large, "over-cap new -> flag true");
}

/// A 0-byte side comes back `None` (absent), not an empty-base64 side that
/// would render as a broken `data:` URL (F-A9-13). Exercised end-to-end on
/// the workdir-new side: index holds RED, the workdir file is truncated to 0.
#[test]
fn zero_byte_side_is_absent_not_empty_base64() {
    let dir = init_scratch();
    let p = dir.path();
    commit_bytes(p, "img.png", RED, "add image"); // index = RED
    std::fs::write(p.join("img.png"), b"").expect("truncate to 0 bytes"); // workdir = empty

    let diff = get_image_diff(
        p,
        &ImageDiffRequest::Workdir {
            path: "img.png".into(),
            orig_path: None,
            staged: false,
        },
    )
    .expect("zero-byte diff");
    assert!(diff.old.is_some(), "small old side present");
    assert!(diff.new.is_none(), "0-byte new -> None (absent)");
    assert!(!diff.new_too_large, "0-byte is absent, NOT flagged too_large");
}

/// Compare variant: HEAD (old) vs the to-commit (new).
#[test]
fn compare_variant_head_vs_to() {
    let dir = init_scratch();
    let p = dir.path();
    let c1 = commit_bytes(p, "img.png", RED, "A"); // to-commit
    commit_bytes(p, "img.png", GREEN, "B"); // HEAD

    let diff = get_image_diff(
        p,
        &ImageDiffRequest::Compare {
            to_oid: c1,
            path: "img.png".into(),
            orig_path: None,
        },
    )
    .expect("compare diff");
    // old = HEAD (GREEN), new = to-commit (RED).
    assert_eq!(base64_decode(&diff.old.expect("old = HEAD").base64), GREEN);
    assert_eq!(base64_decode(&diff.new.expect("new = to").base64), RED);
}

/// Bad / unknown oid -> `AppError::Git`.
#[test]
fn bad_oid_is_git_error() {
    let dir = init_scratch();
    let p = dir.path();
    commit_bytes(p, "img.png", RED, "add");

    let malformed = get_image_diff(
        p,
        &ImageDiffRequest::Commit {
            oid: "notahexoid".into(),
            path: "img.png".into(),
            orig_path: None,
        },
    )
    .expect_err("malformed oid");
    assert!(matches!(malformed, AppError::Git(_)));

    let unknown = get_image_diff(
        p,
        &ImageDiffRequest::Compare {
            to_oid: "0123456789abcdef0123456789abcdef01234567".into(),
            path: "img.png".into(),
            orig_path: None,
        },
    )
    .expect_err("unknown oid");
    assert!(matches!(unknown, AppError::Git(_)));
}

/// Invalid paths (empty / escaping / backslash) are rejected before any
/// git2 work.
#[test]
fn invalid_paths_are_rejected() {
    let dir = init_scratch();
    let p = dir.path();
    for bad in ["", "../escape", "/abs", "a\\b"] {
        let err = get_image_diff(
            p,
            &ImageDiffRequest::Workdir {
                path: bad.into(),
                orig_path: None,
                staged: false,
            },
        )
        .expect_err("must reject bad path");
        assert!(matches!(err, AppError::Other(m) if m.contains("invalid path")));
    }
}

/// Wire shape: camelCase keys incl. `byteLen`, `oldTooLarge`, `newTooLarge`;
/// the request enum deserializes from `{ "kind": "workdir", ... }`.
#[test]
fn wire_serialization_shape() {
    let diff = ImageDiff {
        path: "a.png".into(),
        old: Some(ImageSide {
            base64: "AAAA".into(),
            mime: "image/png".into(),
            byte_len: 3,
        }),
        new: None,
        old_too_large: false,
        new_too_large: true,
    };
    let json = serde_json::to_string(&diff).expect("serialize");
    assert!(json.contains("\"byteLen\":3"), "{json}");
    assert!(json.contains("\"oldTooLarge\":false"), "{json}");
    assert!(json.contains("\"newTooLarge\":true"), "{json}");
    assert!(json.contains("\"new\":null"), "{json}");

    let req: ImageDiffRequest =
        serde_json::from_str(r#"{"kind":"workdir","path":"a.png","origPath":null,"staged":true}"#)
            .expect("deserialize request");
    assert_eq!(
        req,
        ImageDiffRequest::Workdir {
            path: "a.png".into(),
            orig_path: None,
            staged: true,
        }
    );
}
