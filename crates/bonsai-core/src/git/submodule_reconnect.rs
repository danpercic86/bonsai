//! P73 — reconnect an orphaned `.git/modules/<key>` gitdir to an empty,
//! gitlink-less submodule worktree (contract `docs/contracts/P73-submodule-reconnect.md`).
//!
//! libgit2's `git_submodule_update` picks clone-vs-checkout from the single
//! `WD_UNINITIALIZED` bit, and that bit is set purely from "does
//! `<workdir>/<path>/.git` exist" — nothing about `.git/modules`. So a
//! superproject whose submodule worktree was emptied (gitlink deleted) but whose
//! cached module gitdir is complete takes libgit2's clone branch and dies with
//! `attempt to reinitialize '<...>'` (`NO_REINIT`). Upstream `git submodule
//! update` instead REUSES that gitdir and rewrites the worktree gitlink
//! (`connect_work_tree_and_git_dir`); this module is that missing path.
//!
//! Everything here is fail-closed: any doubt yields [`Salvage::NotApplicable`]
//! (libgit2 keeps today's behaviour) or a refusal `AppError::Git` — never a
//! silent fall-through after having modified anything.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use crate::error::AppError;
use crate::git::stage::validate_rel_path;
use crate::git::submodule::validate_modules_name;

/// Upper bound on entries visited by [`workdir_is_empty`] before it gives up and
/// declares the dir non-empty (a huge tree is certainly not the wedged state).
const EMPTY_SCAN_LIMIT: usize = 4096;

/// Outcome of the P73 reconnect attempt. `NotApplicable` means "this is not the
/// wedged state — let libgit2 do its normal thing (fresh clone or plain
/// checkout)". `Reattached` means we rewrote the worktree gitlink so that
/// `git_submodule_update` will now take its open+checkout branch instead of its
/// `NO_REINIT` clone branch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Salvage {
    NotApplicable,
    Reattached,
}

// ---------------------------------------------------------------- user copy
//
// USER-FACING COPY — governed by `docs/contracts/P73-submodule-reconnect-ui.md`
// §5.2: every refusal must reach the toast as a complete, capitalised,
// period-terminated sentence with no libgit2 prose and no internal
// `.git/modules` paths, and must NOT repeat the submodule name (the frontend
// prefixes it with "Couldn't check out <name>. "). Do NOT reword these in a
// drive-by refactor — they are contract text.

/// Step 6: the worktree holds files but has no `.git` link.
fn msg_dirty_workdir(rel_path: &str) -> String {
    format!(
        "The folder already has files in it. Move or delete everything inside \
         '{rel_path}', then try again."
    )
}

/// Step 7: no configured url anywhere (local config nor `.gitmodules`).
const MSG_NO_URL: &str = "No URL is configured for this submodule, so its cached data cannot be \
                          verified. Run Sync on this submodule, then try again.";

/// Step 7: the cached gitdir has no `origin` remote (ownership unprovable).
const MSG_NO_ORIGIN: &str = "Bonsai's cached data for this submodule has no remote URL recorded. \
                             Run Sync on this submodule, then try again.";

/// Step 7: the cached gitdir belongs to a different remote url.
fn msg_url_mismatch(origin: &str, configured: &str) -> String {
    format!(
        "Bonsai has cached data for a different remote URL ('{origin}' instead of \
         '{configured}'). Run Sync on this submodule, then try again."
    )
}

/// Backstop for the one wedge the salvage CANNOT repair: `<commondir>/modules/<key>`
/// exists but is not an openable repository (an aborted clone left a garbage or
/// incomplete directory), so step 4 returns `NotApplicable`, libgit2 takes its
/// clone branch and dies with `attempt to reinitialize '<abs path>'`. Killing that
/// raw libgit2 sentence is the entire point of P73, so `update_submodule` maps it
/// to this text instead. This is the ONE refusal that names an internal
/// `.git/modules` path, deliberately: deleting that folder is the only remedy, and
/// Bonsai must not delete it itself (a valid-but-unopenable repo — permissions, a
/// file lock, AV — must never be destroyed by us). Do NOT reword — contract text.
///
/// Takes the submodule's **path**, not its name: libgit2's clone keys the dir it
/// tries to init on `sm->path` (`submodule.c` `submodule_repo_create`), so the dir
/// that provoked the reinitialize failure is always `<modules>/<path>`. For a
/// renamed submodule (name != path) a name-keyed message would send the user to
/// the wrong folder — or to one that does not exist.
pub(super) fn msg_unusable_module_dir(path: &str) -> String {
    format!(
        "Bonsai has leftover data for this submodule that it cannot reuse. Delete the folder \
         \".git/modules/{path}\" inside this repository, then try again."
    )
}

/// Step 5: the submodule path resolves outside the superproject.
const MSG_OUTSIDE_REPO: &str =
    "This submodule resolves to a path outside the repository. Bonsai will not touch it.";

/// Step 9: the reattach was written but git still reports the wedge.
const MSG_REATTACH_INEFFECTIVE: &str =
    "Could not reconnect this submodule to its existing local data. Run \
     \"git submodule update --init\" in a terminal to repair it.";

// ------------------------------------------------------------- pure helpers

/// True when a git2 error is libgit2's "attempt to reinitialize '<path>'" refusal
/// (`repo_init_directory` → `GIT_ERROR_REPOSITORY` / `GIT_EEXISTS` under the
/// `NO_REINIT` clone flag). Two independent discriminators, either sufficient:
/// the message substring (stable libgit2 wording) and the code+class pair, so a
/// future rewording on one side still classifies.
pub(super) fn is_reinitialize_error(e: &git2::Error) -> bool {
    if e.message().to_ascii_lowercase().contains("reinitialize") {
        return true;
    }
    e.code() == git2::ErrorCode::Exists && e.class() == git2::ErrorClass::Repository
}

/// Strip a Windows `\\?\` verbatim prefix and normalize to forward slashes —
/// git cannot read a `\\?\` gitlink or `core.worktree`.
fn strip_verbatim(p: &Path) -> String {
    let s = p.to_string_lossy().replace('\\', "/");
    s.strip_prefix("//?/").map(str::to_string).unwrap_or(s)
}

/// Pure component-diff: the forward-slash relative path from directory
/// `from_dir` to `to`. Both inputs are canonicalized and share a prefix. Empty
/// result ⇒ `"."`. Always forward slashes (git requires them on all platforms).
fn rel_path(from_dir: &Path, to: &Path) -> String {
    let f: Vec<_> = from_dir.components().collect();
    let t: Vec<_> = to.components().collect();
    let mut i = 0;
    while i < f.len() && i < t.len() && f[i] == t[i] {
        i += 1;
    }
    let mut parts: Vec<String> = Vec::new();
    for _ in i..f.len() {
        parts.push("..".to_string());
    }
    for c in &t[i..] {
        parts.push(c.as_os_str().to_string_lossy().into_owned());
    }
    if parts.is_empty() {
        ".".to_string()
    } else {
        parts.join("/")
    }
}

/// Normalize a remote url for the ownership heuristic: trim ASCII whitespace,
/// then strip trailing `/` and ONE trailing `.git` repeatedly until stable.
fn normalize_url(u: &str) -> String {
    let mut s = u.trim();
    loop {
        if let Some(t) = s.strip_suffix('/') {
            s = t;
            continue;
        }
        if let Some(t) = s.strip_suffix(".git") {
            s = t;
            continue;
        }
        break;
    }
    s.to_string()
}

/// Heuristic ownership check for the URL guard (contract OPEN-3, two-tier): an
/// exact byte match after normalization, or an ASCII-case-insensitive match
/// (accepted + logged). NO percent-decoding (`%20` stays `%20`), no scheme/host
/// canonicalization. This is an ownership heuristic, not a security boundary —
/// the security boundary is the name/containment guard in steps 1 and 5.
pub(super) fn urls_equivalent(a: &str, b: &str) -> bool {
    let na = normalize_url(a);
    let nb = normalize_url(b);
    if na == nb {
        return true;
    }
    if na.eq_ignore_ascii_case(&nb) {
        eprintln!("bonsai: submodule url match only case-insensitively: {na} vs {nb}");
        return true;
    }
    false
}

/// True when `abs` is absent, or is a directory holding no regular file, no
/// symlink and no `.git` entry at any depth (leftover EMPTY directories are
/// tolerated — contract OPEN-4). A present `.git` entry means we are not in the
/// wedged state at all, so this returns false. Bounded: stops and returns false
/// after [`EMPTY_SCAN_LIMIT`] visited entries. Never follows symlinks.
pub(super) fn workdir_is_empty(abs: &Path) -> bool {
    let md = match std::fs::symlink_metadata(abs) {
        Ok(md) => md,
        Err(_) => return true, // absent ⇒ "empty"
    };
    if !md.is_dir() {
        return false; // a file/symlink at the path ⇒ not the wedged state
    }
    let mut stack = vec![abs.to_path_buf()];
    let mut visited = 0usize;
    while let Some(d) = stack.pop() {
        let entries = match std::fs::read_dir(&d) {
            Ok(e) => e,
            Err(_) => return false, // unreadable ⇒ refuse (fail closed)
        };
        for e in entries {
            let e = match e {
                Ok(e) => e,
                Err(_) => return false,
            };
            visited += 1;
            if visited > EMPTY_SCAN_LIMIT {
                return false;
            }
            if e.file_name() == OsStr::new(".git") {
                return false; // already attached ⇒ not the wedged state
            }
            match std::fs::symlink_metadata(e.path()) {
                Ok(t) if t.is_dir() => stack.push(e.path()), // empty dirs tolerated
                Ok(_) => return false,                       // any file or symlink ⇒ not empty
                Err(_) => return false,
            }
        }
    }
    true
}

// ------------------------------------------------------------ module gitdir

/// Locate an EXISTING orphaned module gitdir for this submodule, or `None`.
///
/// Uses `repo.commondir()` (NOT `repo.path()`): inside a linked worktree
/// `repo.path()` is `.git/worktrees/<wt>/`, whose `modules/` subdir does not
/// exist — the shared modules root lives under the commondir.
///
/// libgit2 keys the clone repodir on `sm->path` while Bonsai's
/// `remove_cached_git_dir` keys on `name`; the two diverge for a renamed
/// submodule, so BOTH candidates are probed (`name` first — it is git's
/// canonical key — then `path` when different, contract OPEN-1). Each candidate
/// must canonicalize to a path STRICTLY inside the canonicalized
/// `<commondir>/modules`. Returns the canonicalized dir.
pub(super) fn module_gitdir(
    repo: &git2::Repository,
    name: &str,
    path: &str,
) -> Result<Option<PathBuf>, AppError> {
    let root = repo.commondir().join("modules");
    let canon_root = match root.canonicalize() {
        Ok(r) => r,
        Err(_) => return Ok(None), // no modules dir at all
    };
    let mut candidates: Vec<&str> = Vec::new();
    if validate_modules_name(name).is_ok() {
        candidates.push(name);
    }
    if path != name && validate_rel_path(path).is_ok() && validate_modules_name(path).is_ok() {
        candidates.push(path);
    }
    for key in candidates {
        let dir = root.join(key);
        let canon = match dir.canonicalize() {
            Ok(c) => c,
            Err(_) => continue, // absent → next candidate
        };
        if !canon.starts_with(&canon_root) || canon == canon_root {
            continue; // containment (belt-and-braces after name validation)
        }
        if !canon.is_dir() {
            continue;
        }
        return Ok(Some(canon));
    }
    Ok(None)
}

// --------------------------------------------------------------- gitlink IO

/// Write `contents` to `target` atomically: a sibling temp file + rename, so a
/// torn `.git` never leaves the submodule in a worse state than the wedge. On
/// rename failure, fall back to a direct write.
fn write_atomic(target: &Path, contents: &str) -> Result<(), AppError> {
    let tmp = target.with_file_name(".git.bonsai-tmp");
    if std::fs::write(&tmp, contents).is_ok() && std::fs::rename(&tmp, target).is_ok() {
        return Ok(());
    }
    let _ = std::fs::remove_file(&tmp);
    std::fs::write(target, contents).map_err(|e| {
        AppError::Git(format!(
            "cannot write submodule gitlink '{}': {e}",
            target.display()
        ))
    })
}

/// Reconnect `sub_workdir` to `module_dir`, mirroring upstream git's
/// `connect_work_tree_and_git_dir`: write `<sub_workdir>/.git` =
/// `gitdir: <p>\n`, set the module's `core.worktree` back to the worktree, and
/// `core.bare = false`. Paths are RELATIVE (forward slashes) when both dirs live
/// under the superproject root, else absolute with any `\\?\` prefix stripped
/// (contract OPEN-5). NEVER touches the superproject repo, its config, its index
/// or `.gitmodules`.
pub(super) fn write_gitlink(
    sub_workdir: &Path,
    module_dir: &Path,
    super_workdir: &Path,
) -> Result<(), AppError> {
    let canon = |p: &Path| -> Result<PathBuf, AppError> {
        p.canonicalize()
            .map_err(|e| AppError::Git(format!("cannot resolve '{}': {e}", p.display())))
    };
    let c_sub = canon(sub_workdir)?;
    let c_mod = canon(module_dir)?;
    let c_root = canon(super_workdir)?;

    // Canonical forms are used ONLY for the containment test and the hop
    // computation; the absolute fallback is emitted from the (already absolute)
    // un-canonicalized inputs so no `\\?\` prefix can leak into the gitlink.
    let contained = c_mod.starts_with(&c_root) && c_sub.starts_with(&c_root);
    let gitdir_value = if contained {
        rel_path(&c_sub, &c_mod)
    } else {
        strip_verbatim(module_dir)
    };
    let worktree_value = if contained {
        rel_path(&c_mod, &c_sub)
    } else {
        strip_verbatim(sub_workdir)
    };

    let gitlink = sub_workdir.join(".git");
    write_atomic(&gitlink, &format!("gitdir: {gitdir_value}\n"))?;

    // If the config half fails, UNDO the gitlink: leaving it behind would clear
    // `WD_UNINITIALIZED` while `core.worktree` still points somewhere stale, so
    // the next Update would hand libgit2 an inconsistent pair instead of the
    // recoverable wedge. Keeps "a refusal leaves disk unchanged" total.
    let configure = (|| -> Result<(), AppError> {
        // NO_SEARCH: a bogus dir must never resolve upward to the superproject.
        let sub_repo = git2::Repository::open_ext(
            module_dir,
            git2::RepositoryOpenFlags::NO_SEARCH,
            &[] as &[&OsStr],
        )?;
        let mut cfg = sub_repo.config()?;
        cfg.set_str("core.worktree", &worktree_value)?;
        cfg.set_bool("core.bare", false)?;
        Ok(())
    })();
    if let Err(e) = configure {
        let _ = std::fs::remove_file(&gitlink);
        return Err(e);
    }
    Ok(())
}

// -------------------------------------------------------------- the salvage

/// The P73 salvage: detect the wedged state (empty/gitlink-less worktree +
/// complete `.git/modules/<key>` gitdir) and reattach it so `Submodule::update`
/// reuses the existing gitdir instead of hitting libgit2's `NO_REINIT` clone.
/// Fail-CLOSED at every step. Nothing is written before step 8, so a refusal
/// always leaves the repo byte-identical.
pub(super) fn reattach_module_gitdir(
    repo: &git2::Repository,
    sm: &git2::Submodule<'_>,
    name: &str,
) -> Result<Salvage, AppError> {
    let path = sm.path().to_string_lossy().replace('\\', "/");
    let super_wd = repo
        .workdir()
        .ok_or_else(|| AppError::Git("repository has no working directory".to_string()))?;

    // 1. Hostile-name / traversal guard BEFORE any filesystem decision.
    validate_modules_name(name)?;
    validate_rel_path(&path)?;

    // 2. Only the WD_UNINITIALIZED state can be wedged.
    let flags = repo.submodule_status(name, git2::SubmoduleIgnore::None)?;
    if !flags.contains(git2::SubmoduleStatus::WD_UNINITIALIZED) {
        return Ok(Salvage::NotApplicable);
    }

    // 3. No orphaned gitdir ⇒ a genuine first clone. Let libgit2 clone.
    let module_dir = match module_gitdir(repo, name, &path)? {
        Some(d) => d,
        None => return Ok(Salvage::NotApplicable),
    };

    // 4. The gitdir must be a REAL repo (NO_SEARCH: never resolve upward).
    let sub_repo = match git2::Repository::open_ext(
        &module_dir,
        git2::RepositoryOpenFlags::NO_SEARCH,
        &[] as &[&OsStr],
    ) {
        Ok(r) => r,
        Err(_) => return Ok(Salvage::NotApplicable), // garbage dir → libgit2's problem
    };

    // 5. Workdir containment: the target must be strictly inside the superproject.
    let sub_wd = super_wd.join(&path);
    let parent = sub_wd
        .parent()
        .ok_or_else(|| AppError::Git("submodule path has no parent".to_string()))?;
    let c_parent = parent.canonicalize().map_err(|e| {
        AppError::Git(format!(
            "cannot resolve submodule parent directory '{}': {e}",
            parent.display()
        ))
    })?;
    let c_super = super_wd.canonicalize().map_err(|e| {
        AppError::Git(format!(
            "cannot resolve repository directory '{}': {e}",
            super_wd.display()
        ))
    })?;
    if !c_parent.starts_with(&c_super) {
        return Err(AppError::Git(MSG_OUTSIDE_REPO.to_string()));
    }

    // 6. The workdir must be empty/absent. NEVER clobber user files.
    if !workdir_is_empty(&sub_wd) {
        return Err(AppError::Git(msg_dirty_workdir(&path)));
    }

    // 7. URL guard — prove the orphaned gitdir belongs to THIS submodule. The
    //    configured url comes from the LOCAL config first (a GLOBAL
    //    `submodule.<name>.url` key must not be able to fake registration),
    //    falling back to `.gitmodules` via `sm.url()`.
    let local = repo
        .config()
        .and_then(|c| c.open_level(git2::ConfigLevel::Local))
        .ok()
        .or_else(|| git2::Config::open(&repo.commondir().join("config")).ok());
    let configured = local
        .and_then(|c| c.get_string(&format!("submodule.{name}.url")).ok())
        .or_else(|| sm.url().ok().flatten().map(str::to_string));
    let configured = match configured {
        Some(u) if !u.trim().is_empty() => u,
        _ => return Err(AppError::Git(MSG_NO_URL.to_string())),
    };
    let origin = match sub_repo
        .find_remote("origin")
        .ok()
        .and_then(|r| r.url().ok().map(str::to_string))
    {
        Some(u) => u,
        None => return Err(AppError::Git(MSG_NO_ORIGIN.to_string())), // OPEN-2
    };
    if !urls_equivalent(&configured, &origin) {
        return Err(AppError::Git(msg_url_mismatch(&origin, &configured)));
    }

    // 8. Reattach. Containment is proven, so creating the dir is safe.
    if !sub_wd.exists() {
        std::fs::create_dir_all(&sub_wd).map_err(|e| {
            AppError::Git(format!(
                "cannot create submodule directory '{}': {e}",
                sub_wd.display()
            ))
        })?;
    }
    write_gitlink(&sub_wd, &module_dir, super_wd)?;

    // 9. Verify the wedge actually cleared, from a FRESH handle
    //    (`Submodule::reload` ignores `force`; a stale handle would lie).
    let fresh = repo.find_submodule(name)?;
    let _ = &fresh;
    let flags2 = repo.submodule_status(name, git2::SubmoduleIgnore::None)?;
    if flags2.contains(git2::SubmoduleStatus::WD_UNINITIALIZED) {
        return Err(AppError::Git(MSG_REATTACH_INEFFECTIVE.to_string()));
    }

    // 10. Done.
    eprintln!(
        "bonsai: reconnected submodule '{name}' to existing git dir {}",
        strip_verbatim(&module_dir) // canonicalized ⇒ would print `\\?\D:\...`
    );
    Ok(Salvage::Reattached)
}

#[cfg(test)]
#[path = "submodule_reconnect_tests.rs"]
mod tests;
