//! Branch operations core (M5 contract §2).
//!
//! Pure git2 logic, no Tauri types — testable against the git CLI oracle
//! (see `tests/branches_cli.rs`). All functions blocking; the command layer
//! wraps them in `spawn_blocking`.

use std::path::Path;

use crate::error::AppError;
use crate::git::repo::{read_head_info, HeadInfo};

/// One local branch in the sidebar snapshot.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BranchInfo {
    /// Shorthand, e.g. "main", "feature/sidebar".
    pub name: String,
    /// True for the branch HEAD points at (always false when detached/unborn).
    pub is_head: bool,
    /// Upstream shorthand, e.g. "origin/main"; None when no upstream
    /// configured or the upstream ref is gone.
    pub upstream: Option<String>,
    /// Commits ahead of / behind upstream. None whenever `upstream` is None.
    pub ahead: Option<u32>,
    pub behind: Option<u32>,
    /// Full 40-char hex oid of the branch tip.
    pub tip: String,
}

/// One remote-tracking branch (read-only list in M5).
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteBranchInfo {
    /// Shorthand incl. remote, e.g. "origin/main".
    pub name: String,
    /// Full 40-char hex oid of the remote-tracking branch tip.
    pub tip: String,
}

/// One snapshot of everything the sidebar renders.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BranchesSnapshot {
    /// Sorted case-insensitively by name.
    pub local: Vec<BranchInfo>,
    /// Sorted case-insensitively; symbolic "<remote>/HEAD" entries EXCLUDED.
    pub remote: Vec<RemoteBranchInfo>,
    /// Tag names (lightweight + annotated), sorted case-insensitively.
    pub tags: Vec<String>,
    /// Same shape the header already uses — one source of truth for
    /// attached/detached/unborn in the sidebar.
    pub head: HeadInfo,
}

/// Opens the repo at `workdir` with `NO_SEARCH` (same as every git/ module).
fn open_repo_at(workdir: &Path) -> Result<git2::Repository, AppError> {
    Ok(git2::Repository::open_ext(
        workdir,
        git2::RepositoryOpenFlags::NO_SEARCH,
        std::iter::empty::<&std::ffi::OsStr>(),
    )?)
}

/// Case-insensitive name ordering (ties broken case-sensitively so the
/// order is total and stable).
fn ci_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    a.to_lowercase().cmp(&b.to_lowercase()).then_with(|| a.cmp(b))
}

/// Blocking. One snapshot of local branches, remote-tracking branches, tags,
/// HEAD. Unborn repo: empty lists (or whatever exists), `head.unborn == true`
/// — `Ok`, not `Err`. Non-UTF-8 ref names are skipped with an eprintln,
/// never an error.
pub fn list_refs(workdir: &Path) -> Result<BranchesSnapshot, AppError> {
    let repo = open_repo_at(workdir)?;
    let head = read_head_info(&repo)?;

    let mut local = Vec::new();
    for item in repo.branches(Some(git2::BranchType::Local))? {
        let (branch, _) = item?;
        let name = match branch.name()? {
            Some(n) => n.to_string(),
            None => {
                eprintln!("bonsai: skipping local branch with non-UTF-8 name");
                continue;
            }
        };
        let is_head = branch.is_head();

        // Tip oid; direct local branches always have a target — the `continue`
        // is a defensive skip, consistent with the non-UTF-8 skip above.
        let local_oid = match branch.get().target() {
            Some(oid) => oid,
            None => {
                eprintln!("bonsai: skipping symbolic/targetless local branch");
                continue;
            }
        };
        let tip = local_oid.to_string();

        // Upstream shorthand; None when unset or the upstream ref is gone.
        let upstream_branch = branch.upstream().ok();
        let upstream = upstream_branch
            .as_ref()
            .and_then(|u| u.name().ok().flatten().map(str::to_string));

        // Ahead/behind is best-effort (contract §2.1): any lookup error
        // degrades to None — never fail the whole snapshot for it. Reuse
        // `local_oid` (already read above) rather than calling target() twice.
        let (ahead, behind) = match &upstream {
            Some(_) => {
                let upstream_oid = upstream_branch.as_ref().and_then(|u| u.get().target());
                match upstream_oid.map(|u| repo.graph_ahead_behind(local_oid, u)) {
                    Some(Ok((a, b))) => (u32::try_from(a).ok(), u32::try_from(b).ok()),
                    _ => (None, None),
                }
            }
            None => (None, None),
        };

        local.push(BranchInfo {
            name,
            is_head,
            upstream,
            ahead,
            behind,
            tip,
        });
    }
    local.sort_by(|a, b| ci_cmp(&a.name, &b.name));

    let mut remote = Vec::new();
    for item in repo.branches(Some(git2::BranchType::Remote))? {
        let (branch, _) = item?;
        // Skip symbolic entries — that is "<remote>/HEAD".
        if branch.get().symbolic_target().is_some() {
            continue;
        }
        let name = match branch.name()? {
            Some(n) => n.to_string(),
            None => {
                eprintln!("bonsai: skipping remote branch with non-UTF-8 name");
                continue;
            }
        };
        let tip = match branch.get().target() {
            Some(oid) => oid.to_string(),
            None => {
                eprintln!("bonsai: skipping targetless remote branch");
                continue;
            }
        };
        remote.push(RemoteBranchInfo { name, tip });
    }
    remote.sort_by(|a, b| ci_cmp(&a.name, &b.name));

    let mut tags: Vec<String> = repo
        .tag_names(None)?
        .iter()
        .flatten()
        .map(str::to_string)
        .collect();
    tags.sort_by(|a, b| ci_cmp(a, b));

    Ok(BranchesSnapshot {
        local,
        remote,
        tags,
        head,
    })
}

/// Backend-authoritative branch-name validation (mirrors
/// `git check-ref-format --branch`): trimmed-empty and leading `-` are our
/// stricter pre-checks (libgit2 accepts `refs/heads/-x` as a valid ref name;
/// the git CLI refuses `-x` as a branch name), the rest is
/// `git2::Branch::name_is_valid`.
fn validate_branch_name(name: &str) -> Result<(), AppError> {
    let invalid = || AppError::InvalidName(format!("invalid branch name: '{name}'"));
    if name.trim().is_empty() || name.starts_with('-') {
        return Err(invalid());
    }
    if !git2::Branch::name_is_valid(name)? {
        return Err(invalid());
    }
    Ok(())
}

/// Blocking. Creates local branch `name` at the current HEAD commit.
/// Does NOT check out.
pub fn create_branch(workdir: &Path, name: &str) -> Result<(), AppError> {
    validate_branch_name(name)?;
    let repo = open_repo_at(workdir)?;

    let head_commit = match repo.head().and_then(|h| h.peel_to_commit()) {
        Ok(commit) => commit,
        Err(e)
            if matches!(
                e.code(),
                git2::ErrorCode::UnbornBranch | git2::ErrorCode::NotFound
            ) =>
        {
            return Err(AppError::Git(
                "cannot create a branch: the repository has no commits yet".to_string(),
            ));
        }
        Err(e) => return Err(e.into()),
    };

    if let Err(e) = repo.branch(name, &head_commit, /* force */ false) {
        if e.code() == git2::ErrorCode::Exists {
            return Err(AppError::BranchExists(format!(
                "branch '{name}' already exists"
            )));
        }
        return Err(e.into());
    }
    Ok(())
}

/// Blocking. Checks out LOCAL branch `name` (v1: local branch names only —
/// no tags, no oids, no remote-tracking checkout; contract §9).
///
/// SAFE checkout only — NEVER force. `checkout_tree` runs before `set_head`,
/// so a conflict leaves both the worktree and HEAD untouched.
pub fn checkout_branch(workdir: &Path, name: &str) -> Result<(), AppError> {
    let repo = open_repo_at(workdir)?;

    let branch = match repo.find_branch(name, git2::BranchType::Local) {
        Ok(b) => b,
        Err(e) if e.code() == git2::ErrorCode::NotFound => {
            return Err(AppError::BranchNotFound(format!("branch '{name}' not found")));
        }
        Err(e) => return Err(e.into()),
    };

    // No-op when already checked out (UI hides the action; guard the race).
    if branch.is_head() {
        return Ok(());
    }

    let target_oid = branch
        .get()
        .target()
        .ok_or_else(|| AppError::Git(format!("branch '{name}' has no target commit")))?;
    let obj = repo.find_object(target_oid, None)?;

    let mut opts = git2::build::CheckoutBuilder::new();
    opts.safe(); // DEFAULT SAFE MODE — never .force()
    match repo.checkout_tree(&obj, Some(&mut opts)) {
        Ok(()) => {}
        Err(e) if e.code() == git2::ErrorCode::Conflict => {
            return Err(AppError::CheckoutConflict(format!(
                "cannot switch to '{name}': local changes would be overwritten. \
                 Commit or discard them first."
            )));
        }
        Err(e) => return Err(e.into()),
    }

    repo.set_head(&format!("refs/heads/{name}"))?;
    Ok(())
}

/// Blocking. Deletes LOCAL branch `name`. Safety gates in order:
/// not-found → `BranchNotFound`; currently checked out → `Git` (race-only
/// backstop, the UI never offers it); not fully merged into HEAD →
/// `UnmergedBranch` (libgit2's `Branch::delete` has `git branch -D`
/// semantics, so the `-d` merged-check is implemented here).
pub fn delete_branch(workdir: &Path, name: &str) -> Result<(), AppError> {
    let repo = open_repo_at(workdir)?;

    let mut branch = match repo.find_branch(name, git2::BranchType::Local) {
        Ok(b) => b,
        Err(e) if e.code() == git2::ErrorCode::NotFound => {
            return Err(AppError::BranchNotFound(format!("branch '{name}' not found")));
        }
        Err(e) => return Err(e.into()),
    };

    if branch.is_head() {
        return Err(AppError::Git(format!(
            "cannot delete '{name}': it is the currently checked-out branch"
        )));
    }

    let tip = branch
        .get()
        .target()
        .ok_or_else(|| AppError::Git(format!("branch '{name}' has no target commit")))?;

    // Merged = tip reachable from HEAD (strict `git branch -d`-style check
    // against HEAD only). Detached HEAD: the detached commit; unborn HEAD:
    // treat as unmerged.
    let head_oid = match repo.head() {
        Ok(head) => head.target(),
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
    let merged = match head_oid {
        Some(head) => tip == head || repo.graph_descendant_of(head, tip)?,
        None => false,
    };
    if !merged {
        let tip_hex = tip.to_string();
        let short_tip = tip_hex.get(..7).unwrap_or(&tip_hex);
        return Err(AppError::UnmergedBranch(format!(
            "branch '{name}' is not fully merged into HEAD (tip {short_tip}). \
             Bonsai v1 does not force-delete; use `git branch -D {name}` if you are sure."
        )));
    }

    branch.delete()?;
    Ok(())
}

/// Blocking. GitKraken-style remote checkout: create (or reuse) a LOCAL tracking
/// branch for the remote-tracking ref `remote_shorthand` ("<remote>/<branch>")
/// and safe-checkout it. SAFE checkout only — never force (P6 contract §2.2).
///
/// A name collision (a local branch of the same short name already exists) just
/// switches to the existing local branch — it is NOT repointed. Safe checkout
/// runs before any ref mutation, so a conflict leaves HEAD + worktree untouched
/// and creates nothing.
pub fn checkout_remote(workdir: &Path, remote_shorthand: &str) -> Result<(), AppError> {
    let repo = open_repo_at(workdir)?;

    // Split on the FIRST '/': remote names contain no '/'. The remote segment
    // is validated non-empty but not otherwise needed here.
    let local_name = match remote_shorthand.split_once('/') {
        Some((r, l)) if !r.is_empty() && !l.is_empty() => l,
        _ => {
            return Err(AppError::InvalidName(format!(
                "invalid remote branch name: '{remote_shorthand}'"
            )));
        }
    };

    // Find the remote-tracking ref and its tip.
    let remote_branch = match repo.find_branch(remote_shorthand, git2::BranchType::Remote) {
        Ok(b) => b,
        Err(e) if e.code() == git2::ErrorCode::NotFound => {
            return Err(AppError::BranchNotFound(format!(
                "remote-tracking branch '{remote_shorthand}' not found"
            )));
        }
        Err(e) => return Err(e.into()),
    };
    let remote_tip = remote_branch.get().target().ok_or_else(|| {
        AppError::Git(format!(
            "remote-tracking branch '{remote_shorthand}' has no target commit"
        ))
    })?;

    // Decide the checkout target + whether we create — BEFORE touching the
    // worktree, so a conflict leaves everything untouched and creates nothing.
    let (checkout_oid, created) = match repo.find_branch(local_name, git2::BranchType::Local) {
        Ok(existing) => {
            let oid = existing.get().target().ok_or_else(|| {
                AppError::Git(format!("branch '{local_name}' has no target commit"))
            })?;
            (oid, false)
        }
        Err(e) if e.code() == git2::ErrorCode::NotFound => (remote_tip, true),
        Err(e) => return Err(e.into()),
    };

    // SAFE checkout FIRST (matches `checkout_branch`): a conflict leaves HEAD +
    // worktree untouched AND nothing has been created yet.
    let obj = repo.find_object(checkout_oid, None)?;
    let mut opts = git2::build::CheckoutBuilder::new();
    opts.safe(); // DEFAULT SAFE MODE — never .force()
    match repo.checkout_tree(&obj, Some(&mut opts)) {
        Ok(()) => {}
        Err(e) if e.code() == git2::ErrorCode::Conflict => {
            return Err(AppError::CheckoutConflict(format!(
                "cannot switch to '{local_name}': local changes would be overwritten. \
                 Commit or discard them first."
            )));
        }
        Err(e) => return Err(e.into()),
    }

    // Checkout succeeded — only now mutate refs.
    if created {
        let remote_commit = repo.find_commit(remote_tip)?;
        match repo.branch(local_name, &remote_commit, /* force */ false) {
            Ok(mut new_branch) => {
                // Best-effort upstream — a set failure is still a successful
                // checkout; log and continue, do NOT roll back.
                if let Err(e) = new_branch.set_upstream(Some(remote_shorthand)) {
                    eprintln!(
                        "bonsai: checked out '{local_name}' but failed to set upstream \
                         '{remote_shorthand}': {e}"
                    );
                }
            }
            // Race: created between our probe and now — just proceed to set_head.
            Err(e) if e.code() == git2::ErrorCode::Exists => {}
            Err(e) => return Err(e.into()),
        }
    }

    repo.set_head(&format!("refs/heads/{local_name}"))?;
    Ok(())
}

/// Blocking. Deletes the LOCAL remote-tracking ref `name` ("origin/feature").
/// Local-only: does NOT contact the server. No merged-check (a local-branch
/// concept only) (P6 contract §2.3).
pub fn delete_remote_tracking(workdir: &Path, name: &str) -> Result<(), AppError> {
    let repo = open_repo_at(workdir)?;

    let mut branch = match repo.find_branch(name, git2::BranchType::Remote) {
        Ok(b) => b,
        Err(e) if e.code() == git2::ErrorCode::NotFound => {
            return Err(AppError::BranchNotFound(format!(
                "remote-tracking branch '{name}' not found"
            )));
        }
        Err(e) => return Err(e.into()),
    };

    branch.delete()?;
    Ok(())
}
