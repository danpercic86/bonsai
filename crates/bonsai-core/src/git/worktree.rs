//! Worktree support (P27 contract §2). P27a: read-only list.
//!
//! Pure git2 logic, no Tauri types (runtime-free core → unit/CLI-testable
//! without the Tauri "test" feature, same rule as submodule/stash/remote).
//! `list_worktrees` synthesizes the MAIN working tree as the first row so the
//! list is complete, then appends every linked worktree from
//! `Repository::worktrees()`.
//!
//! The `sanitize_slug` + `derive_worktree` path-derivation scaffolding lands
//! here (P27a) but is consumed by `add_worktree` in P27b; it is asserted to keep
//! every derived leaf inside a fixed `.worktrees/` container so no `..` /
//! separator injection reaches libgit2 or the filesystem.

use std::path::{Path, PathBuf};

use crate::error::AppError;
use crate::git::stage::open_workdir_repo;

/// One worktree row (main or linked). Wire: camelCase. `head_oid` is full
/// 40-hex or null; the frontend shortens for display.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorktreeInfo {
    /// Worktree NAME (stable key for remove/lock/unlock). For linked worktrees
    /// this is `Worktree::name()`; for the synthesized main row it is the main
    /// workdir's directory basename. Destructive actions key off `is_main`/
    /// `is_current`, NOT the name (§2.3).
    pub name: String,
    /// ABSOLUTE path to the worktree's working directory, forward slashes on the
    /// wire. Fed verbatim to the open-repo/tab flow (§6.6).
    pub abs_path: String,
    /// Repo-relative path (forward slashes) IF the worktree lives under the main
    /// workdir, else None (worktrees are usually siblings → usually None).
    pub rel_path: Option<String>,
    /// Short branch name checked out in this worktree (e.g. "feature/x"), or None
    /// when HEAD is detached / the worktree is invalid.
    pub branch: Option<String>,
    /// Full HEAD commit oid, or None when the worktree is invalid/unreadable.
    pub head_oid: Option<String>,
    /// True if `Worktree::is_locked()` == Locked. Always false for the main row.
    pub locked: bool,
    /// The optional lock reason from `WorktreeLockStatus::Locked(Some(reason))`.
    pub lock_reason: Option<String>,
    /// True for the synthesized main working tree.
    pub is_main: bool,
    /// True for the worktree whose workdir == the repo the app currently has open.
    pub is_current: bool,
    /// True when `Worktree::is_prunable(None)` — i.e. stale (its working dir is
    /// gone / administratively removable). Always false for the main row.
    pub prunable: bool,
    /// `Worktree::validate().is_ok()` — the working tree + admin files are intact.
    /// Always true for the main row.
    pub valid: bool,
}

/// Blocking. List the main worktree (synthesized, first) followed by every
/// linked worktree in `Repository::worktrees()` order. Never empty (always ≥ the
/// main row for a non-bare repo). Non-UTF-8 linked names are skipped (logged),
/// exactly like the branch/ref listers.
pub fn list_worktrees(workdir: &Path) -> Result<Vec<WorktreeInfo>, AppError> {
    let repo = open_workdir_repo(workdir)?; // rejects bare
    let cur = repo
        .workdir()
        .map(canonical)
        .ok_or_else(|| AppError::Git("repository has no working directory".to_string()))?;
    let main_dir = main_workdir(&repo)?;

    let mut out = Vec::new();
    out.push(build_main_row(&main_dir, &cur)?); // synthesized main row, FIRST

    for name in repo.worktrees()?.iter() {
        let name = match name {
            Some(n) => n,
            None => {
                eprintln!("bonsai: skipping worktree with non-UTF-8 name");
                continue;
            }
        };
        let wt = repo.find_worktree(name)?;
        out.push(build_linked_row(&wt, &main_dir, &cur)?);
    }
    Ok(out)
}

/// Path of the MAIN worktree's workdir, regardless of which worktree the app has
/// open. If the current repo is itself a linked worktree, derive it from the
/// shared common dir (`<main>/.git` → parent). Non-bare assumed
/// (`open_workdir_repo`).
fn main_workdir(repo: &git2::Repository) -> Result<PathBuf, AppError> {
    if repo.is_worktree() {
        // commondir == "<main>/.git" (possibly trailing sep) → parent is <main>.
        strip_dotgit_parent(repo.commondir())
            .ok_or_else(|| AppError::Git("cannot locate main worktree".to_string()))
    } else {
        repo.workdir()
            .map(Path::to_path_buf)
            .ok_or_else(|| AppError::Git("repository has no working directory".to_string()))
    }
}

/// `<main>/.git[/]` → `<main>`. Trailing separators are ignored by `Path`'s
/// component iteration, so a single `.parent()` strips the `.git` component.
fn strip_dotgit_parent(commondir: &Path) -> Option<PathBuf> {
    commondir.parent().map(Path::to_path_buf)
}

/// Synthesize the main row. Opens the main workdir to read its HEAD.
fn build_main_row(main_dir: &Path, cur: &Path) -> Result<WorktreeInfo, AppError> {
    let repo = git2::Repository::open(main_dir)?;
    let (branch, head_oid) = read_head(&repo);
    Ok(WorktreeInfo {
        name: dir_basename(main_dir),
        abs_path: to_fwd(main_dir),
        rel_path: None,
        branch,
        head_oid,
        locked: false,
        lock_reason: None,
        is_main: true,
        is_current: canonical(main_dir) == *cur,
        prunable: false,
        valid: true,
    })
}

/// Build one linked-worktree row. Tolerates a stale/missing working dir: a
/// worktree that fails `validate()` yields `valid=false` and null branch/oid
/// rather than erroring the whole list.
fn build_linked_row(
    wt: &git2::Worktree,
    main_dir: &Path,
    cur: &Path,
) -> Result<WorktreeInfo, AppError> {
    let name = wt
        .name()
        .ok_or_else(|| AppError::Git("worktree has non-UTF-8 name".to_string()))?;
    let path = wt.path(); // absolute
    let valid = wt.validate().is_ok();
    let prunable = wt.is_prunable(None).unwrap_or(false);
    let (locked, lock_reason) = match wt.is_locked()? {
        // The git CLI writes the reason with a trailing newline; normalize to a
        // clean wire value (trim, drop when empty).
        git2::WorktreeLockStatus::Locked(r) => {
            let reason = r
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty());
            (true, reason)
        }
        git2::WorktreeLockStatus::Unlocked => (false, None),
    };
    // branch/oid: only readable when the working tree is intact.
    let (branch, head_oid) = if valid {
        match git2::Repository::open_from_worktree(wt) {
            Ok(r) => read_head(&r),
            Err(_) => (None, None),
        }
    } else {
        (None, None)
    };
    Ok(WorktreeInfo {
        name: name.to_string(),
        abs_path: to_fwd(path),
        rel_path: path.strip_prefix(main_dir).ok().map(to_fwd),
        branch,
        head_oid,
        locked,
        lock_reason,
        is_main: false,
        is_current: canonical(path) == *cur,
        prunable,
        valid,
    })
}

/// Reads a repo's HEAD: `(short branch name | None when detached/unborn,
/// full HEAD oid | None when unborn/unreadable)`.
fn read_head(repo: &git2::Repository) -> (Option<String>, Option<String>) {
    match repo.head() {
        Ok(head) => {
            let branch = if head.is_branch() {
                head.shorthand().map(str::to_string)
            } else {
                None // detached HEAD
            };
            let oid = head.target().map(|o| o.to_string());
            (branch, oid)
        }
        // Unborn branch / no HEAD: nothing to show.
        Err(_) => (None, None),
    }
}

/// Best-effort canonicalization for path EQUALITY only (both sides go through
/// this, so a `\\?\` prefix or symlink resolution is consistent). Falls back to
/// the lexical path when the target does not exist (e.g. a stale worktree dir).
fn canonical(p: &Path) -> PathBuf {
    std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf())
}

/// `to_string_lossy` with backslashes normalized to forward slashes (the wire
/// format). Used for both absolute and repo-relative paths.
fn to_fwd(p: &Path) -> String {
    p.to_string_lossy().replace('\\', "/")
}

/// Final path component as an owned `String` (falls back to the whole path).
fn dir_basename(p: &Path) -> String {
    p.file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| p.to_string_lossy().into_owned())
}

/// Slug for a branch name: every char outside `[A-Za-z0-9._-]` → `-`, runs of
/// `-` collapsed, leading/trailing `-`/`.` trimmed. Rejects an empty or
/// `..`-containing result with `InvalidName` (git branch names cannot contain
/// `..`, but we reject defensively — mirrors `validate_rel_path`, `stage.rs:33`).
///
/// Consumed by `derive_worktree` (P27b's `add_worktree`); landed in P27a.
#[allow(dead_code)] // wired into add_worktree in P27b; exercised by unit tests now
pub(crate) fn sanitize_slug(branch: &str) -> Result<String, AppError> {
    let replaced: String = branch
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-' {
                c
            } else {
                '-'
            }
        })
        .collect();
    // Collapse runs of '-'.
    let mut collapsed = replaced;
    while collapsed.contains("--") {
        collapsed = collapsed.replace("--", "-");
    }
    let trimmed = collapsed.trim_matches(|c| c == '-' || c == '.');
    if trimmed.is_empty() || trimmed.contains("..") {
        return Err(AppError::InvalidName(format!(
            "cannot derive a worktree name from branch '{branch}'"
        )));
    }
    Ok(trimmed.to_string())
}

/// Derived worktree name/path for `branch` (§OPEN-1): `name == slug`,
/// `path == <main_parent>/.worktrees/<slug>`. On collision (dir exists OR
/// `find_worktree(name)` succeeds) append `-2`, `-3`, … up to `-99`, else a
/// `Git` error. Because the leaf is a sanitized slug joined onto a fixed
/// container, no separators or `..` reach libgit2.
///
/// Consumed by `add_worktree` in P27b; landed (with tests) in P27a.
#[allow(dead_code)] // wired into add_worktree in P27b; exercised by unit tests now
pub(crate) fn derive_worktree(
    main_dir: &Path,
    repo: &git2::Repository,
    branch: &str,
) -> Result<(String, PathBuf), AppError> {
    let base = sanitize_slug(branch)?; // Err(InvalidName) if empty / ".."
    let container = main_dir
        .parent()
        .ok_or_else(|| AppError::Git("repo has no parent directory".to_string()))?
        .join(".worktrees");
    for n in std::iter::once(base.clone()).chain((2..=99).map(|i| format!("{base}-{i}"))) {
        let path = container.join(&n);
        let name_taken = repo.find_worktree(&n).is_ok();
        if !path.exists() && !name_taken {
            // Containment defense: `path` MUST stay under `container`.
            debug_assert!(path.starts_with(&container));
            return Ok((n, path));
        }
    }
    Err(AppError::Git(format!(
        "could not derive a free worktree path for '{branch}'"
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// §2.9: the wire shape must match the TS mirror — camelCase keys, full oid.
    #[test]
    fn worktree_info_serializes_camel_case_keys() {
        let info = WorktreeInfo {
            name: "feature-login".to_string(),
            abs_path: "/repo/.worktrees/feature-login".to_string(),
            rel_path: None,
            branch: Some("feature/login".to_string()),
            head_oid: Some("a".repeat(40)),
            locked: true,
            lock_reason: Some("pinned for QA".to_string()),
            is_main: false,
            is_current: false,
            prunable: false,
            valid: true,
        };
        let v = serde_json::to_value(&info).expect("json");
        assert_eq!(
            v,
            serde_json::json!({
                "name": "feature-login",
                "absPath": "/repo/.worktrees/feature-login",
                "relPath": null,
                "branch": "feature/login",
                "headOid": "a".repeat(40),
                "locked": true,
                "lockReason": "pinned for QA",
                "isMain": false,
                "isCurrent": false,
                "prunable": false,
                "valid": true
            })
        );
    }

    /// §2.9: the slug table, including the priority tie-breaks and rejections.
    #[test]
    fn sanitize_slug_table() {
        // Happy cases.
        assert_eq!(sanitize_slug("feature/login").expect("slug"), "feature-login");
        assert_eq!(sanitize_slug("feature/x").expect("slug"), "feature-x");
        assert_eq!(sanitize_slug("feat/x").expect("slug"), "feat-x");
        assert_eq!(sanitize_slug("a//b").expect("slug"), "a-b"); // collapse runs
        assert_eq!(sanitize_slug("--weird--").expect("slug"), "weird"); // trim ends
        assert_eq!(sanitize_slug("release/1.2").expect("slug"), "release-1.2"); // dots kept
        assert_eq!(sanitize_slug("hot fix").expect("slug"), "hot-fix"); // space → '-'

        // Rejections → InvalidName.
        for bad in ["", "   ", "..", "/", "---", "..."] {
            match sanitize_slug(bad) {
                Err(AppError::InvalidName(_)) => {}
                other => panic!("expected InvalidName for {bad:?}, got {other:?}"),
            }
        }
    }

    /// §2.4: derive_worktree yields `<parent>/.worktrees/<slug>`, stays inside
    /// the container, and appends `-2` on a directory collision.
    #[test]
    fn derive_worktree_containment_and_collision() {
        // Init the repo in a subdir so its PARENT is the unique tempdir — the
        // derived `.worktrees/` container then never leaks into the shared
        // scratch root across runs.
        let dir = crate::testutil::scratch_dir();
        let repo_dir = dir.path().join("repo");
        let repo = git2::Repository::init(&repo_dir).expect("init");
        let main_dir = repo.workdir().expect("workdir").to_path_buf();
        let container = main_dir.parent().expect("parent").join(".worktrees");

        // First derivation: exact slug.
        let (name, path) = derive_worktree(&main_dir, &repo, "feature/login").expect("derive");
        assert_eq!(name, "feature-login");
        assert_eq!(path, container.join("feature-login"));
        assert!(path.starts_with(&container), "leaf must stay in container");

        // Force a collision: create the directory, re-derive → "-2".
        std::fs::create_dir_all(&path).expect("mkdir collision");
        let (name2, path2) = derive_worktree(&main_dir, &repo, "feature/login").expect("derive");
        assert_eq!(name2, "feature-login-2");
        assert_eq!(path2, container.join("feature-login-2"));
        assert!(path2.starts_with(&container));
    }

    /// A branch whose sanitized slug is empty is rejected before any path work.
    #[test]
    fn derive_worktree_rejects_empty_slug() {
        let dir = crate::testutil::scratch_dir();
        let repo = git2::Repository::init(dir.path().join("repo")).expect("init");
        let main_dir = repo.workdir().expect("workdir").to_path_buf();
        match derive_worktree(&main_dir, &repo, "///") {
            Err(AppError::InvalidName(_)) => {}
            other => panic!("expected InvalidName, got {other:?}"),
        }
    }
}
