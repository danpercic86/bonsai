//! Worktree support (P27 contract §2). P27a: read-only list; P27b: the
//! mutating ops (`add_worktree` / `remove_worktree` / `lock_worktree` /
//! `unlock_worktree`).
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

    for name in repo.worktrees()?.iter().map(|name| name.ok().flatten()) {
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
pub(crate) fn main_workdir(repo: &git2::Repository) -> Result<PathBuf, AppError> {
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
        .ok()
        .flatten()
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
                head.shorthand().ok().map(str::to_string)
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
pub(crate) fn canonical(p: &Path) -> PathBuf {
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

/// Derived worktree name/path (§OPEN-1 + P32 Part A): the slug is sourced from
/// the user-editable `name`, and the container gains a per-repo level →
/// `<main_parent>/.worktrees/<repo-name>/<name-slug>` where `<repo-name>` is
/// `dir_basename(main_dir)`. On collision (dir exists OR `find_worktree(name)`
/// succeeds) append `-2`, `-3`, … up to `-99`, else a `Git` error. Because the
/// leaf is a sanitized slug joined onto a fixed container, no separators or `..`
/// reach libgit2.
pub(crate) fn derive_worktree(
    main_dir: &Path,
    repo: &git2::Repository,
    name: &str,
) -> Result<(String, PathBuf), AppError> {
    let base = sanitize_slug(name)?; // Err(InvalidName) if empty / ".."
    let container = main_dir
        .parent()
        .ok_or_else(|| AppError::Git("repo has no parent directory".to_string()))?
        .join(".worktrees")
        .join(dir_basename(main_dir)); // NEW (P32 Part A): per-repo level
    for n in std::iter::once(base.clone()).chain((2..=99).map(|i| format!("{base}-{i}"))) {
        let path = container.join(&n);
        let name_taken = repo.find_worktree(&n).is_ok();
        if !path.exists() && !name_taken {
            // Containment defense (RUNTIME — add_worktree creates this dir):
            // the derived leaf MUST stay inside the `.worktrees/<repo>/` container.
            ensure_contained(&path, &container)?;
            return Ok((n, path));
        }
    }
    Err(AppError::Git(format!(
        "could not derive a free worktree path for '{name}'"
    )))
}

/// RUNTIME path-containment defense: `path` must be lexically inside
/// `container` (the fixed `.worktrees/` dir). The leaf comes from a sanitized
/// slug so this should never fire; it exists so a sanitizer regression can
/// never create a directory outside the container.
pub(crate) fn ensure_contained(path: &Path, container: &Path) -> Result<(), AppError> {
    if path.starts_with(container) && path != container {
        Ok(())
    } else {
        Err(AppError::Git(format!(
            "derived worktree path '{}' escapes the .worktrees container",
            path.display()
        )))
    }
}

/// Blocking. Create a linked worktree checking out the EXISTING local branch
/// `branch` (§2.4/§2.5). The on-disk name/slug is driven by the user-editable
/// `name` (P32 Part A) — decoupled from `branch`; a blank/whitespace `name`
/// defaults to `branch` (callers may pass the branch as the name). The path is
/// derived as `<main_parent>/.worktrees/<repo-name>/<name-slug>`. Returns the
/// created row (§OPEN-2).
/// Errors: `InvalidName` (blank/unsluggable branch or name) | `BranchNotFound` |
/// `Git` (branch already checked out elsewhere / collision exhausted / libgit2)
/// | `Io`.
pub fn add_worktree(workdir: &Path, branch: &str, name: &str) -> Result<WorktreeInfo, AppError> {
    let repo = open_workdir_repo(workdir)?;
    if branch.trim().is_empty() {
        return Err(AppError::InvalidName("branch name is empty".to_string()));
    }
    let br = repo
        .find_branch(branch, git2::BranchType::Local)
        .map_err(|e| match e.code() {
            git2::ErrorCode::NotFound => {
                AppError::BranchNotFound(format!("branch '{branch}' not found"))
            }
            _ => e.into(),
        })?;
    let reference = br.get(); // &Reference (borrows repo)

    let main_dir = main_workdir(&repo)?;
    // A blank name defaults to the branch (the UI auto-syncs name → branch until
    // the user edits it, but a caller may also pass the branch verbatim).
    let name_src = if name.trim().is_empty() { branch } else { name };
    let (name, path) = derive_worktree(&main_dir, &repo, name_src)?;
    // Ensure the `.worktrees/` container exists (Io on failure). derive_worktree
    // guarantees `path` has a parent (it joined a leaf onto the container).
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let mut opts = git2::WorktreeAddOptions::new();
    opts.reference(Some(reference)); // check out THIS existing branch

    // libgit2 refuses a branch already checked out in another worktree → Git,
    // surfaced with its own message ("already checked out ...").
    repo.worktree(&name, &path, Some(&opts))
        .map_err(|e| AppError::Git(e.message().to_string()))?;

    let wt = repo.find_worktree(&name)?;
    let cur = repo
        .workdir()
        .map(canonical)
        .ok_or_else(|| AppError::Git("repository has no working directory".to_string()))?;
    build_linked_row(&wt, &main_dir, &cur) // return the created row (§OPEN-2)
}

/// Blocking. Remove linked worktree `name` (§2.6): refuse main / current /
/// locked / dirty, then prune (admin files + working directory), with a guarded
/// `remove_dir_all` fallback if the directory survives.
/// Errors: `InvalidName` (blank) | `Git` (refusals / not found / libgit2) | `Io`.
pub fn remove_worktree(workdir: &Path, name: &str) -> Result<(), AppError> {
    let repo = open_workdir_repo(workdir)?;
    if name.trim().is_empty() {
        return Err(AppError::InvalidName("worktree name is empty".to_string()));
    }
    let main_dir = main_workdir(&repo)?;
    let cur = repo
        .workdir()
        .map(canonical)
        .ok_or_else(|| AppError::Git("repository has no working directory".to_string()))?;

    // 1. Refuse the MAIN worktree by NAME (its basename is not a real linked
    //    name; also guard by path below in case a linked name collides).
    if name == dir_basename(&main_dir) && repo.find_worktree(name).is_err() {
        return Err(AppError::Git("cannot remove the main worktree".to_string()));
    }
    let wt = repo.find_worktree(name).map_err(|e| match e.code() {
        git2::ErrorCode::NotFound => AppError::Git(format!("worktree '{name}' not found")),
        _ => e.into(),
    })?;
    let wt_path = wt.path().to_path_buf();

    // 2. Refuse current / main by PATH (defense-in-depth).
    if canonical(&wt_path) == cur {
        return Err(AppError::Git(
            "cannot remove the worktree you currently have open".to_string(),
        ));
    }
    if canonical(&wt_path) == canonical(&main_dir) {
        return Err(AppError::Git("cannot remove the main worktree".to_string()));
    }
    // 3. Refuse LOCKED (no force in v1, §OPEN-4).
    if matches!(wt.is_locked()?, git2::WorktreeLockStatus::Locked(_)) {
        return Err(AppError::Git(
            "worktree is locked; unlock it first".to_string(),
        ));
    }
    // 4. Refuse DIRTY (§OPEN-3): data-loss guard — libgit2's prune does not
    //    check dirtiness.
    if wt.validate().is_ok() {
        if let Ok(wt_repo) = git2::Repository::open_from_worktree(&wt) {
            if is_dirty(&wt_repo)? {
                return Err(AppError::Git(
                    "worktree has uncommitted changes; commit or stash them first".to_string(),
                ));
            }
        }
    }
    // 5. Prune: remove admin files AND the working directory recursively.
    //    NOT locked(true): we refused locked above.
    let mut opts = git2::WorktreePruneOptions::new();
    opts.valid(true).working_tree(true);
    wt.prune(Some(&mut opts))?;

    // 6. Guarded fallback: only the exact worktree path we just pruned.
    if wt_path.exists() {
        std::fs::remove_dir_all(&wt_path)?;
    }
    Ok(())
}

/// True when the repo has any status entry that is not CURRENT (staged,
/// unstaged, or untracked — ignored files excluded). Runtime-free; no
/// dependency on `status.rs`.
fn is_dirty(repo: &git2::Repository) -> Result<bool, AppError> {
    let mut opts = git2::StatusOptions::new();
    opts.include_untracked(true).include_ignored(false);
    let statuses = repo.statuses(Some(&mut opts))?;
    Ok(statuses
        .iter()
        .any(|e| e.status() != git2::Status::CURRENT))
}

/// Find linked worktree `name`, mapping blank → `InvalidName` and not-found →
/// a precise `Git` error (shared by lock/unlock).
fn open_linked(repo: &git2::Repository, name: &str) -> Result<git2::Worktree, AppError> {
    if name.trim().is_empty() {
        return Err(AppError::InvalidName("worktree name is empty".to_string()));
    }
    repo.find_worktree(name).map_err(|e| match e.code() {
        git2::ErrorCode::NotFound => AppError::Git(format!("worktree '{name}' not found")),
        _ => e.into(),
    })
}

/// Blocking. Lock linked worktree `name` with an optional reason (§2.7).
/// An empty/blank reason is treated as no reason. The main worktree cannot be
/// locked (it is never a linked worktree → precise `Git` not-found error).
/// Errors: `InvalidName` | `Git` (not found / already locked / libgit2).
pub fn lock_worktree(workdir: &Path, name: &str, reason: Option<&str>) -> Result<(), AppError> {
    let repo = open_workdir_repo(workdir)?;
    let wt = open_linked(&repo, name)?;
    let reason = reason.map(str::trim).filter(|r| !r.is_empty());
    wt.lock(reason)?; // already-locked → libgit2 Git error
    Ok(())
}

/// Blocking. Unlock linked worktree `name` (§2.7).
/// Errors: `InvalidName` | `Git` (not found / not locked / libgit2).
pub fn unlock_worktree(workdir: &Path, name: &str) -> Result<(), AppError> {
    let repo = open_workdir_repo(workdir)?;
    let wt = open_linked(&repo, name)?;
    wt.unlock()?; // not-locked → libgit2 Git error
    Ok(())
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
        let container = main_dir
            .parent()
            .expect("parent")
            .join(".worktrees")
            .join(dir_basename(&main_dir));

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

    /// §2.4 runtime containment: paths outside (or equal to) the container are
    /// rejected with a `Git` error, never silently accepted.
    #[test]
    fn ensure_contained_runtime_check() {
        let container = Path::new("/repo/.worktrees");
        assert!(ensure_contained(Path::new("/repo/.worktrees/feat"), container).is_ok());
        // The container itself is not a valid leaf.
        match ensure_contained(container, container) {
            Err(AppError::Git(_)) => {}
            other => panic!("expected Git error for container itself, got {other:?}"),
        }
        for escapee in ["/repo/elsewhere", "/repo", "/other/.worktrees/feat"] {
            match ensure_contained(Path::new(escapee), container) {
                Err(AppError::Git(_)) => {}
                other => panic!("expected Git error for {escapee:?}, got {other:?}"),
            }
        }
    }

    /// §2.9: blank args are rejected up-front with `InvalidName`.
    #[test]
    fn blank_args_are_invalid_name() {
        let dir = crate::testutil::scratch_dir();
        let repo_dir = dir.path().join("repo");
        git2::Repository::init(&repo_dir).expect("init");
        match add_worktree(&repo_dir, "   ", "   ") {
            Err(AppError::InvalidName(_)) => {}
            other => panic!("expected InvalidName for blank branch, got {other:?}"),
        }
        match remove_worktree(&repo_dir, "") {
            Err(AppError::InvalidName(_)) => {}
            other => panic!("expected InvalidName for blank name, got {other:?}"),
        }
        match lock_worktree(&repo_dir, " ", None) {
            Err(AppError::InvalidName(_)) => {}
            other => panic!("expected InvalidName for blank name, got {other:?}"),
        }
        match unlock_worktree(&repo_dir, "") {
            Err(AppError::InvalidName(_)) => {}
            other => panic!("expected InvalidName for blank name, got {other:?}"),
        }
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
