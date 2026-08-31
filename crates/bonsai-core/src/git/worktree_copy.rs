//! Copy selected uncommitted / gitignored changes into a new worktree
//! (P27 contract, P32 extension Part B).
//!
//! Pure git2 + `std::fs`; runtime-free (`&Path`/`&str`/wire structs only). The
//! crate has no `Repository::apply`/patch primitive and its diff text is lossy,
//! so the transfer is a **raw file-content copy** (source workdir bytes →
//! worktree path). "Conflict" is a *content* decision (base blob vs target
//! blob), never a failed patch apply.
//!
//! Three public entry points:
//!   * [`list_copy_candidates`] — enumerate copy-eligible files (staged /
//!     unstaged / untracked / ignored, deletions excluded).
//!   * [`classify_copy`] — per-path clean-vs-conflict verdict against a target
//!     branch (three-way content compare, before the worktree exists).
//!   * [`add_worktree_with_changes`] — create the worktree then copy the
//!     selected source bytes into it.
//!
//! Reuses `status::read_status`, `worktree::{add_worktree, ensure_contained}`.

use std::path::Path;
use std::path::PathBuf;

use crate::error::AppError;
use crate::git::stage::open_workdir_repo;
use crate::git::status::{self, FileStatus};
use crate::git::worktree::{self, ensure_contained, WorktreeInfo};

/// Which status list a copy candidate came from. Wire: lowercase
/// ("staged" | "unstaged" | "untracked" | "ignored"), matching `FileStatus`'s repr.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum CopyGroup {
    Staged,
    Unstaged,
    Untracked,
    Ignored,
}

/// One file the user may copy into the new worktree.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CopyCandidate {
    /// Repo-relative path, forward slashes (StatusEntry.path convention).
    pub path: String,
    pub group: CopyGroup,
}

/// Conflict verdict for a selected path against the target branch. Wire:
/// lowercase ("clean" | "conflict").
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum CopyVerdict {
    Clean,
    Conflict,
}

/// Result of `classify_copy` for one path.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CopyPlanEntry {
    pub path: String,
    pub verdict: CopyVerdict,
}

/// What to do with one selected path at create time. Wire: lowercase
/// ("copy" | "skip"). "Overwrite" in the UI == `copy` on a conflict.
/// Deserialized (command input) → also derives `Deserialize`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CopyAction {
    Copy,
    Skip,
}

/// One user decision, sent to `add_worktree_with_changes`.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CopySelection {
    pub path: String,
    pub action: CopyAction,
}

/// Blocking. Enumerate every uncommitted or gitignored file eligible to copy
/// into a new worktree. Deletions are EXCLUDED (v1). Groups:
///   staged     = read_status().staged   (index vs HEAD, non-deleted)
///   unstaged   = read_status().unstaged (workdir vs index, non-deleted)
///   untracked  = read_status().untracked
///   ignored    = a SECOND status pass with `include_ignored(true)` (read_status
///                uses `include_ignored(false)`) → entries where `status.is_ignored()`
/// Renames contribute the NEW path only (`orig_path` ignored, B5).
/// Returns candidates ordered staged, unstaged, untracked, ignored; each group
/// byte-sorted by path.
pub fn list_copy_candidates(workdir: &Path) -> Result<Vec<CopyCandidate>, AppError> {
    let snap = status::read_status(workdir)?;
    let mut out = Vec::new();

    // staged / unstaged: skip deletions (nothing to copy). read_status already
    // byte-sorted each list, so pushing in order preserves per-group ordering.
    for e in &snap.staged {
        if e.status != FileStatus::Deleted {
            out.push(CopyCandidate {
                path: e.path.clone(),
                group: CopyGroup::Staged,
            });
        }
    }
    for e in &snap.unstaged {
        if e.status != FileStatus::Deleted {
            out.push(CopyCandidate {
                path: e.path.clone(),
                group: CopyGroup::Unstaged,
            });
        }
    }
    for e in &snap.untracked {
        out.push(CopyCandidate {
            path: e.path.clone(),
            group: CopyGroup::Untracked,
        });
    }

    // ignored: a dedicated status pass. Only `is_ignored()` entries are kept;
    // those are disjoint from the tracked/untracked lists above, so no cross-group
    // dedupe is needed.
    let repo = open_workdir_repo(workdir)?;
    let mut opts = git2::StatusOptions::new();
    opts.include_untracked(true)
        .recurse_untracked_dirs(true)
        .include_ignored(true)
        .include_unmodified(false);
    let mut ignored = Vec::new();
    for e in repo.statuses(Some(&mut opts))?.iter() {
        if e.status().is_ignored() {
            ignored.push(CopyCandidate {
                path: lossy_fwd(e.path_bytes()),
                group: CopyGroup::Ignored,
            });
        }
    }
    ignored.sort_by(|a, b| a.path.cmp(&b.path));
    out.extend(ignored);

    Ok(out)
}

/// Blocking. Conflict-classify `paths` against the target `branch`, BEFORE the
/// worktree exists (the target tree is readable from the shared ODB). For each
/// path a three-way CONTENT comparison:
///   base   = blob bytes at `path` in the SOURCE repo's HEAD tree   (None if absent)
///   target = blob bytes at `path` in `branch`'s commit tree        (None if absent)
/// Verdict:
///   Clean    if `target.is_none()`  OR  `target == base`
///   Conflict otherwise (target diverged from base → copying would overwrite it)
/// Untracked / gitignored files (base None, usually target None) are Clean.
/// Order/length mirror `paths`.
pub fn classify_copy(
    workdir: &Path,
    branch: &str,
    paths: &[String],
) -> Result<Vec<CopyPlanEntry>, AppError> {
    let repo = open_workdir_repo(workdir)?;
    // base tree: source HEAD (None-tolerant on unborn HEAD → all base treated None).
    let base_tree = repo.head().ok().and_then(|h| h.peel_to_tree().ok());
    // target tree: the selected local branch's commit tree.
    let br = repo
        .find_branch(branch, git2::BranchType::Local)
        .map_err(|e| match e.code() {
            git2::ErrorCode::NotFound => {
                AppError::BranchNotFound(format!("branch '{branch}' not found"))
            }
            _ => e.into(),
        })?;
    let target_tree = br.get().peel_to_commit()?.tree()?;

    let mut out = Vec::with_capacity(paths.len());
    for p in paths {
        let base = match &base_tree {
            Some(t) => blob_bytes(&repo, t, p)?,
            None => None,
        };
        let target = blob_bytes(&repo, &target_tree, p)?;
        let verdict = if target.is_none() || target == base {
            CopyVerdict::Clean
        } else {
            CopyVerdict::Conflict
        };
        out.push(CopyPlanEntry {
            path: p.clone(),
            verdict,
        });
    }
    Ok(out)
}

/// Blocking. Create the worktree via `add_worktree(workdir, branch, name)`, then
/// copy each `action == Copy` selection's SOURCE workdir bytes into the worktree
/// at the same relative path. `Skip` selections (incl. Skip-on-conflict) are not
/// written. Empty `selections` == a plain `add_worktree`.
///
/// Each destination is guarded by `ensure_contained(&dest, &wt_root)` so no path
/// escapes the worktree LEXICALLY, and by [`ensure_no_symlink_dest`] +
/// [`ensure_canon_contained`] so it cannot escape THROUGH A SYMLINK that the
/// just-checked-out branch planted in the worktree (audit §3.1); parents are
/// created before write; binary files copy as raw bytes. A selection whose source
/// file no longer exists is silently skipped (untracked file removed mid-flow);
/// only a real IO failure returns an error.
///
/// **Non-transactional:** a mid-copy IO error returns the error with the worktree
/// ALREADY created — the row exists and the user sees the error. Acceptable in v1.
///
/// Errors: everything `add_worktree` can return, plus `Io` (read source /
/// create_dir / write) and `Git` (a selection path escaping the worktree root).
pub fn add_worktree_with_changes(
    workdir: &Path,
    branch: &str,
    name: &str,
    selections: &[CopySelection],
) -> Result<WorktreeInfo, AppError> {
    let info = worktree::add_worktree(workdir, branch, name)?;
    let wt_root = PathBuf::from(&info.abs_path);
    // Canonicalized root for the symlink-aware containment check below; the
    // worktree was just created by `add_worktree`, so this resolves.
    let canon_root = wt_root.canonicalize()?;
    let src_root = workdir;

    for sel in selections {
        if sel.action != CopyAction::Copy {
            continue;
        }
        // Defense: candidates are repo-relative. Reject absolute / parent-escaping
        // paths before joining (ensure_contained is the second line of defense).
        if is_unsafe_rel(&sel.path) {
            return Err(AppError::Git(format!(
                "copy selection path '{}' is not a safe relative path",
                sel.path
            )));
        }
        let dest = wt_root.join(&sel.path);
        ensure_contained(&dest, &wt_root)?;
        // The target branch is ALREADY checked out here, so its tree may have
        // planted a symlink at the leaf or at a directory component of the
        // destination; create_dir_all/write both FOLLOW symlinks. Lexical
        // containment cannot see that - probe the filesystem before touching it.
        ensure_no_symlink_dest(&dest, &wt_root)?;

        let src = src_root.join(&sel.path);
        let bytes = match std::fs::read(&src) {
            Ok(b) => b,
            // Source vanished mid-flow (e.g. untracked file deleted) → skip, not error.
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(e) => return Err(e.into()),
        };
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)?;
            // Defence in depth (same pattern as
            // `submodule::remove_cached_git_dir`, F-A7-2): once the dirs exist, the
            // CANONICALIZED parent must still be inside the canonicalized worktree
            // root - catching anything the lexical + per-component checks could
            // have missed (incl. a racing symlink swap).
            ensure_canon_contained(parent, &canon_root)?;
        }
        std::fs::write(&dest, bytes)?;
    }

    Ok(info)
}

/// Bytes at `rel` in `tree`, `None` when the path is absent (or not a blob:
/// directory / submodule → treated as absent, matching the contract).
fn blob_bytes(
    repo: &git2::Repository,
    tree: &git2::Tree,
    rel: &str,
) -> Result<Option<Vec<u8>>, AppError> {
    match tree.get_path(Path::new(rel)) {
        Ok(entry) => match entry.to_object(repo)?.peel_to_blob() {
            Ok(b) => Ok(Some(b.content().to_vec())),
            Err(_) => Ok(None),
        },
        Err(e) if e.code() == git2::ErrorCode::NotFound => Ok(None),
        Err(e) => Err(e.into()),
    }
}

/// Lossy-decode a byte path with backslashes normalized to forward slashes (the
/// wire format; git2 already reports forward slashes but normalize defensively).
fn lossy_fwd(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).replace('\\', "/")
}

/// True when a repo-relative selection path is unsafe to join onto the worktree
/// root: absolute, or containing a `..` / root / prefix component.
fn is_unsafe_rel(rel: &str) -> bool {
    let p = Path::new(rel);
    p.is_absolute()
        || p.components().any(|c| {
            matches!(
                c,
                std::path::Component::ParentDir
                    | std::path::Component::RootDir
                    | std::path::Component::Prefix(_)
            )
        })
}

/// Symlink write-through defense (audit §3.1). `add_worktree_with_changes` copies
/// AFTER the target branch is checked out, so the worktree may ALREADY contain a
/// symlink at the destination's leaf or at one of its directory components - and
/// both `create_dir_all` and `fs::write` follow symlinks. A branch carrying
/// `config -> ~/.gitconfig` would otherwise let an Overwrite copy clobber a file
/// OUTSIDE the worktree.
///
/// Every component of `dest` below `wt_root` is probed with `symlink_metadata`
/// (which never follows) and an EXISTING symlink is refused. Absent components are
/// fine - they get created inside the worktree.
fn ensure_no_symlink_dest(dest: &Path, wt_root: &Path) -> Result<(), AppError> {
    let Ok(rel) = dest.strip_prefix(wt_root) else {
        return Err(AppError::Git(format!(
            "copy destination '{}' escapes the worktree root",
            dest.display()
        )));
    };
    let mut cur = wt_root.to_path_buf();
    for comp in rel.components() {
        cur.push(comp);
        match std::fs::symlink_metadata(&cur) {
            Ok(md) if md.file_type().is_symlink() => {
                return Err(AppError::Git(format!(
                    "refusing to copy to '{}': '{}' is a symlink",
                    dest.display(),
                    cur.display()
                )));
            }
            Ok(_) => {}
            // Absent (or unreadable): nothing below it can exist either.
            Err(_) => break,
        }
    }
    Ok(())
}

/// Canonicalized containment for a write destination's parent dir, mirroring
/// `submodule::remove_cached_git_dir` (F-A7-2): the resolved parent must be inside
/// the resolved worktree root. Defence in depth behind the lexical
/// [`ensure_contained`] and the per-component [`ensure_no_symlink_dest`], not a
/// replacement for either. Errors when the parent cannot be resolved.
fn ensure_canon_contained(parent: &Path, canon_root: &Path) -> Result<(), AppError> {
    let canon_parent = parent.canonicalize()?;
    if canon_parent.starts_with(canon_root) {
        Ok(())
    } else {
        Err(AppError::Git(format!(
            "copy destination '{}' resolves outside the worktree root",
            parent.display()
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// CopyGroup serializes lowercase (matches FileStatus repr).
    #[test]
    fn copy_group_serializes_lowercase() {
        assert_eq!(
            serde_json::to_value(CopyGroup::Ignored).expect("json"),
            serde_json::json!("ignored")
        );
        assert_eq!(
            serde_json::to_value(CopyVerdict::Conflict).expect("json"),
            serde_json::json!("conflict")
        );
    }

    /// CopySelection round-trips camelCase with a lowercase action (input type).
    #[test]
    fn copy_selection_deserializes_camel_case() {
        let sel: CopySelection =
            serde_json::from_value(serde_json::json!({ "path": "a/b.txt", "action": "copy" }))
                .expect("de");
        assert_eq!(sel.action, CopyAction::Copy);
        assert_eq!(sel.path, "a/b.txt");
    }

    /// Unsafe relative paths are rejected by the guard helper.
    #[test]
    fn is_unsafe_rel_rejects_escapes() {
        assert!(is_unsafe_rel("../escape"));
        assert!(is_unsafe_rel("a/../../b"));
        #[cfg(windows)]
        assert!(is_unsafe_rel("C:/abs"));
        assert!(!is_unsafe_rel("a/b/c.txt"));
        assert!(!is_unsafe_rel("file.txt"));
    }

    /// Empty selections yields a plain worktree identical to add_worktree.
    #[test]
    fn empty_selections_is_plain_worktree() {
        let dir = crate::testutil::scratch_dir();
        let repo_dir = dir.path().join("repo");
        let repo = git2::Repository::init(&repo_dir).expect("init");
        // One commit on a branch so add_worktree has something to check out.
        let mut idx = repo.index().expect("index");
        std::fs::write(repo_dir.join("seed.txt"), b"seed").expect("write seed");
        idx.add_path(Path::new("seed.txt")).expect("add");
        idx.write().expect("write idx");
        let tree = repo.find_tree(idx.write_tree().expect("write tree")).expect("tree");
        let sig = git2::Signature::now("t", "t@e").expect("sig");
        let oid = repo
            .commit(Some("HEAD"), &sig, &sig, "seed", &tree, &[])
            .expect("commit");
        // A distinct branch: libgit2 refuses to check out HEAD's branch twice.
        let commit = repo.find_commit(oid).expect("commit obj");
        repo.branch("feature", &commit, false).expect("branch");

        let info = add_worktree_with_changes(&repo_dir, "feature", "wt-empty", &[]).expect("wt");
        assert!(!info.is_main);
        assert!(PathBuf::from(&info.abs_path).exists());
    }

    /// Audit §3.1: the destination symlink guard. Ordinary paths (existing dir,
    /// absent leaf/dirs) pass everywhere; a dest outside the root is refused; an
    /// EXISTING symlink at the leaf or at a directory component is refused
    /// (symlink creation needs privilege on Windows -> those asserts are unix-only).
    #[test]
    fn symlink_dest_guard_refuses_write_through() {
        let dir = crate::testutil::scratch_dir();
        let root = dir.path().join("wt");
        std::fs::create_dir_all(root.join("sub")).expect("mkdir");
        std::fs::write(root.join("sub/plain.txt"), b"x").expect("write");

        assert!(ensure_no_symlink_dest(&root.join("sub/new.txt"), &root).is_ok());
        assert!(ensure_no_symlink_dest(&root.join("sub/plain.txt"), &root).is_ok());
        assert!(ensure_no_symlink_dest(&root.join("a/b/c.txt"), &root).is_ok());
        assert!(ensure_no_symlink_dest(&dir.path().join("outside.txt"), &root).is_err());

        #[cfg(unix)]
        {
            let outside = dir.path().join("precious.txt");
            std::fs::write(&outside, b"precious").expect("write outside");
            std::os::unix::fs::symlink(&outside, root.join("link.txt")).expect("symlink");
            std::os::unix::fs::symlink(dir.path(), root.join("linkdir")).expect("symlink");
            assert!(ensure_no_symlink_dest(&root.join("link.txt"), &root).is_err());
            assert!(ensure_no_symlink_dest(&root.join("linkdir/x.txt"), &root).is_err());
            // The guard must not have followed anything.
            assert_eq!(std::fs::read(&outside).expect("read"), b"precious");
        }
    }

    /// Audit §3.1 (defence in depth): canonicalized parent containment accepts the
    /// root itself + real subdirs, rejects a sibling dir and an absent path.
    #[test]
    fn canon_contained_accepts_root_rejects_outside() {
        let dir = crate::testutil::scratch_dir();
        let root = dir.path().join("wt");
        std::fs::create_dir_all(root.join("sub")).expect("mkdir");
        let canon_root = root.canonicalize().expect("canonicalize root");
        assert!(ensure_canon_contained(&root, &canon_root).is_ok());
        assert!(ensure_canon_contained(&root.join("sub"), &canon_root).is_ok());
        assert!(ensure_canon_contained(dir.path(), &canon_root).is_err());
        assert!(ensure_canon_contained(&root.join("missing"), &canon_root).is_err());
    }
}
