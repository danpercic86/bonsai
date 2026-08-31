//! P61b — image diff (side-by-side / onion-skin / swipe).
//!
//! Resolves both sides of an image comparison as base64 over IPC (D2 — the
//! `asset://` protocol cannot be served by the mock browser harness, so the
//! command returns raw blob bytes as base64 plus a MIME type and the frontend
//! builds a `data:` URL for a plain `img` element). No image decoding happens
//! here: natural dimensions are read frontend-side from the rendered image.
//!
//! Blob resolution reuses the `diff.rs` internals `commit_trees` and
//! `head_endpoint`, together with `stage::open_workdir_repo`. A tree side reads
//! the blob at the path, the workdir-new side reads the file bytes, and the
//! index side reads the staged blob. Each side is `None` when absent
//! (add/delete), missing, or empty (0-byte, which has no renderable image); a
//! side over `MAX_IMAGE_BYTES` is `None` with its `*_too_large` flag set. The
//! `orig_path` is used for the OLD side on renames.
//!
//! Error kinds: an invalid `path`/`orig_path` (empty, absolute, `..`-escaping,
//! or backslash-bearing) is rejected up-front by `validate_rel_path` as
//! [`AppError::Other`] ("invalid path"); a bad/malformed oid maps to
//! [`AppError::Git`]; an unknown repo maps to `noRepo` (via `open_workdir_repo`).

use std::path::Path;

use crate::error::AppError;
use crate::git::diff::{commit_trees, head_endpoint};
use crate::git::stage::{open_workdir_repo, validate_rel_path};

/// Per-side raw-byte cap (D3). A side larger than this comes back `None` with
/// its `*_too_large` flag `true` ("image too large to preview"). Bounds the
/// worst-case base64 IPC payload at ~11 MB per side.
pub const MAX_IMAGE_BYTES: usize = 8 * 1024 * 1024;

/// One resolved side of an image comparison (serialized camelCase).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageSide {
    /// Raw blob bytes, standard RFC 4648 base64 (NO `data:` prefix — the
    /// frontend builds `data:${mime};base64,${base64}`).
    pub base64: String,
    /// MIME from the path extension, e.g. "image/png".
    pub mime: String,
    /// Raw byte length pre-base64 (for the "N KB" label).
    pub byte_len: u32,
}

/// Both sides of an image comparison (serialized camelCase). A `None` side is
/// either absent (add/delete) or over-cap; the two `*_too_large` flags
/// disambiguate the over-cap case.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageDiff {
    pub path: String,
    /// OLD side (index / HEAD / parent tree). `None` when added OR missing OR
    /// empty (0-byte) OR over-cap (`old_too_large` then `true`).
    pub old: Option<ImageSide>,
    /// NEW side (workdir / index / commit tree). `None` when deleted OR missing
    /// OR empty (0-byte) OR over-cap.
    pub new: Option<ImageSide>,
    pub old_too_large: bool,
    pub new_too_large: bool,
}

/// Which pair to load — mirrors the three file-diff contexts so the frontend
/// constructs it exactly where it picks a `*_file_diff` command today.
/// (`tag = "kind"`, all keys + field names camelCase.)
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum ImageDiffRequest {
    /// `staged == false`: old = index blob, new = workdir file.
    /// `staged == true`: old = HEAD tree blob, new = index blob.
    Workdir {
        path: String,
        orig_path: Option<String>,
        staged: bool,
    },
    /// old = first-parent tree blob (root commit -> `None`); new = commit tree blob.
    Commit {
        oid: String,
        path: String,
        orig_path: Option<String>,
    },
    /// old = HEAD tree blob (unborn -> `None`); new = to-commit tree blob.
    Compare {
        to_oid: String,
        path: String,
        orig_path: Option<String>,
    },
}

/// The standard RFC 4648 base64 alphabet (index -> output char).
const B64_ALPHABET: &[u8; 64] =
    b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// Hand-rolled standard-alphabet base64 encoder with `=` padding (OQ5 — no
/// `base64` crate dependency). Each 3-byte group becomes 4 output chars; a
/// final 1- or 2-byte group is padded with one or two `=`. `chunks(3)` never
/// yields an empty slice, so `chunk[0]` is always in bounds.
fn base64_encode(data: &[u8]) -> String {
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = *chunk.get(1).unwrap_or(&0) as u32;
        let b2 = *chunk.get(2).unwrap_or(&0) as u32;
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(B64_ALPHABET[((n >> 18) & 63) as usize] as char);
        out.push(B64_ALPHABET[((n >> 12) & 63) as usize] as char);
        out.push(if chunk.len() > 1 {
            B64_ALPHABET[((n >> 6) & 63) as usize] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            B64_ALPHABET[(n & 63) as usize] as char
        } else {
            '='
        });
    }
    out
}

/// MIME type from a path's file extension (D4 image set). Anything else ->
/// `application/octet-stream` (defensive; callers only ask for image paths).
fn mime_from_ext(path: &str) -> String {
    let ext = Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    match ext.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "bmp" => "image/bmp",
        "ico" => "image/x-icon",
        "avif" => "image/avif",
        _ => "application/octet-stream",
    }
    .to_string()
}

/// Blob bytes at `path` in `tree`, or `None` when the path is absent or is not
/// a blob (a subtree / submodule). Real git2 errors propagate.
fn blob_from_tree(
    repo: &git2::Repository,
    tree: &git2::Tree,
    path: &str,
) -> Result<Option<Vec<u8>>, AppError> {
    match tree.get_path(Path::new(path)) {
        Ok(entry) => {
            let obj = entry.to_object(repo)?;
            Ok(obj.as_blob().map(|b| b.content().to_vec()))
        }
        Err(e) if e.code() == git2::ErrorCode::NotFound => Ok(None),
        Err(e) => Err(e.into()),
    }
}

/// Staged blob bytes at `path` (stage 0), or `None` when unstaged/absent.
fn blob_from_index(repo: &git2::Repository, path: &str) -> Result<Option<Vec<u8>>, AppError> {
    let index = repo.index()?;
    match index.get_path(Path::new(path), 0) {
        Some(entry) => Ok(Some(repo.find_blob(entry.id)?.content().to_vec())),
        None => Ok(None),
    }
}

/// Workdir file bytes at `workdir/path`, or `None` when the file is missing.
fn bytes_from_workdir(workdir: &Path, path: &str) -> Result<Option<Vec<u8>>, AppError> {
    match std::fs::read(workdir.join(path)) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(AppError::Other(format!("read image file {path}: {e}"))),
    }
}

/// Turn resolved bytes into `(side, too_large)`: absent -> `(None, false)`;
/// empty (0-byte) -> `(None, false)`; over-cap -> `(None, true)`; else the
/// base64-encoded side.
fn make_side(bytes: Option<Vec<u8>>, mime: &str) -> (Option<ImageSide>, bool) {
    match bytes {
        None => (None, false),
        // F-A9-13: a 0-byte side (empty/placeholder file) has no renderable
        // image — treat it as absent rather than emit `data:${mime};base64,`
        // with an empty payload, which resolves to a broken image element.
        Some(b) if b.is_empty() => (None, false),
        Some(b) if b.len() > MAX_IMAGE_BYTES => (None, true),
        Some(b) => (
            Some(ImageSide {
                base64: base64_encode(&b),
                mime: mime.to_string(),
                byte_len: b.len() as u32,
            }),
            false,
        ),
    }
}

/// The OLD-side lookup path: `orig_path` on a rename, else `path`.
fn old_path<'a>(path: &'a str, orig_path: Option<&'a str>) -> &'a str {
    orig_path.unwrap_or(path)
}

/// Blocking. Resolves both sides of an image comparison as base64 (D2). Each
/// side is `None` when absent (add/delete) or missing; a side over
/// [`MAX_IMAGE_BYTES`] is `None` with `*_too_large = true`. Uses `orig_path`
/// for the OLD side on renames. Bad oid -> `git`; unknown repo -> `noRepo`.
pub fn get_image_diff(workdir: &Path, req: &ImageDiffRequest) -> Result<ImageDiff, AppError> {
    let (path, orig_path): (&str, Option<&str>) = match req {
        ImageDiffRequest::Workdir {
            path, orig_path, ..
        }
        | ImageDiffRequest::Commit {
            path, orig_path, ..
        }
        | ImageDiffRequest::Compare {
            path, orig_path, ..
        } => (path.as_str(), orig_path.as_deref()),
    };
    validate_rel_path(path)?;
    if let Some(op) = orig_path {
        validate_rel_path(op)?;
    }

    let repo = open_workdir_repo(workdir)?;
    let mime = mime_from_ext(path);
    let old_lookup = old_path(path, orig_path);

    let (old_bytes, new_bytes) = match req {
        ImageDiffRequest::Workdir { staged, .. } => {
            if *staged {
                // old = HEAD tree blob; new = index blob.
                let old = match repo.head() {
                    Ok(h) => blob_from_tree(&repo, &h.peel_to_tree()?, old_lookup)?,
                    Err(e)
                        if matches!(
                            e.code(),
                            git2::ErrorCode::UnbornBranch | git2::ErrorCode::NotFound
                        ) =>
                    {
                        None
                    }
                    Err(e) => return Err(e.into()),
                };
                (old, blob_from_index(&repo, path)?)
            } else {
                // old = index blob; new = workdir file.
                (
                    blob_from_index(&repo, old_lookup)?,
                    bytes_from_workdir(workdir, path)?,
                )
            }
        }
        ImageDiffRequest::Commit { oid, .. } => {
            let commit = repo.find_commit(git2::Oid::from_str(oid)?)?;
            let (old_tree, new_tree) = commit_trees(&commit)?;
            let old = match old_tree {
                Some(t) => blob_from_tree(&repo, &t, old_lookup)?,
                None => None, // root commit
            };
            (old, blob_from_tree(&repo, &new_tree, path)?)
        }
        ImageDiffRequest::Compare { to_oid, .. } => {
            let to_commit = repo.find_commit(git2::Oid::from_str(to_oid)?)?;
            let to_tree = to_commit.tree()?;
            let (_from, old_tree) = head_endpoint(&repo)?;
            let old = match old_tree {
                Some(t) => blob_from_tree(&repo, &t, old_lookup)?,
                None => None, // unborn HEAD
            };
            (old, blob_from_tree(&repo, &to_tree, path)?)
        }
    };

    let (old, old_too_large) = make_side(old_bytes, &mime);
    let (new, new_too_large) = make_side(new_bytes, &mime);
    Ok(ImageDiff {
        path: path.to_string(),
        old,
        new,
        old_too_large,
        new_too_large,
    })
}


#[cfg(test)]
mod tests;
