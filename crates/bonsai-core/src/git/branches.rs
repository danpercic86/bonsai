//! Branch operations core (M5 contract §2).
//!
//! Pure git2 logic, no Tauri types — testable against the git CLI oracle
//! (see `tests/branches_cli.rs`). All functions blocking; the command layer
//! wraps them in `spawn_blocking`.

use std::path::Path;

use crate::error::AppError;
use crate::git::repo::{read_head_info, HeadInfo};
use crate::git::stash;

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

/// Result of `create_branch_here`. Wire: camelCase.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateBranchHereResult {
    /// true when uncommitted work was auto-stashed and carried across.
    pub stashed: bool,
    /// Present only when `stashed`; the outcome of re-applying the stash on the
    /// new branch (`Applied` = clean carry-over, `Conflicts{paths}` = carried
    /// with markers, stash retained). `None` when the worktree was clean.
    pub apply: Option<stash::ApplyStashOutcome>,
}

/// Blocking. Create local branch `name` at commit `oid`, then check it out,
/// carrying any uncommitted work across via auto-stash. Composes existing
/// primitives; NEVER lossy (working changes are recovered on every failure path).
///
/// Ordered algorithm (P11 §1.1). Errors: `invalidName` | `branchExists` |
/// `operationInProgress` (via `create_stash`) | `configMissing` (via
/// `create_stash`) | `checkoutConflict` (defensive, via `checkout_branch`) |
/// `git` (bad/unknown oid, or any other libgit2 error).
pub fn create_branch_here(
    workdir: &Path,
    name: &str,
    oid: &str,
) -> Result<CreateBranchHereResult, AppError> {
    // 1. Validate & resolve FIRST — zero side effects on any failure here.
    validate_branch_name(name)?;
    let repo = open_repo_at(workdir)?;

    let target_oid = git2::Oid::from_str(oid).map_err(|_| {
        AppError::Git(format!(
            "cannot create branch: '{oid}' is not a valid commit id"
        ))
    })?;
    let target = repo.find_commit(target_oid).map_err(|_| {
        AppError::Git(format!("cannot create branch: commit '{oid}' not found"))
    })?;

    // 2. Pre-check branch existence BEFORE any side effect, so a `BranchExists`
    //    can never strand a stash.
    if repo
        .find_branch(name, git2::BranchType::Local)
        .is_ok()
    {
        return Err(AppError::BranchExists(format!(
            "branch '{name}' already exists"
        )));
    }

    // 3. Auto-stash. `create_stash` owns the dirty-vs-clean decision (clean tree
    //    → created:false) AND the mid-merge/rebase guard (OperationInProgress).
    //    `configMissing` may surface here (stash authors a commit) — let it
    //    propagate. `stashed == true` means work must be re-applied afterwards.
    let stashed = stash::create_stash(workdir, None, /* include_untracked */ true)?.created;

    // 4. Create the branch ref at the resolved commit. On failure, restore the
    //    stashed work onto the original branch (best-effort) before returning.
    if let Err(e) = repo.branch(name, &target, /* force */ false) {
        if stashed {
            let _ = stash::pop_stash(workdir, 0);
        }
        if e.code() == git2::ErrorCode::Exists {
            return Err(AppError::BranchExists(format!(
                "branch '{name}' already exists"
            )));
        }
        return Err(e.into());
    }

    // 5. SAFE checkout the new branch. On failure, roll back so nothing is
    //    stranded: delete the just-created ref and restore stashed work (both
    //    best-effort). Post-stash the worktree is clean, so this is defensive.
    if let Err(e) = checkout_branch(workdir, name) {
        let _ = delete_branch(workdir, name);
        if stashed {
            let _ = stash::pop_stash(workdir, 0);
        }
        return Err(e);
    }

    // 6. Re-apply the carried work iff stashed. `pop_stash` drops on clean apply
    //    and RETAINS on conflict (never lossy). A `Conflicts` outcome is a
    //    SUCCESS return (branch created & checked out; changes present w/ markers).
    if stashed {
        let outcome = stash::pop_stash(workdir, 0)?;
        return Ok(CreateBranchHereResult {
            stashed: true,
            apply: Some(outcome),
        });
    }

    // 7. Clean case.
    Ok(CreateBranchHereResult {
        stashed: false,
        apply: None,
    })
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

#[cfg(test)]
mod create_branch_here_tests {
    //! P11f (contract §1.1 algorithm, §1.5/§7 acceptance): `create_branch_here`
    //! must carry uncommitted work across a checkout via auto-stash and NEVER be
    //! lossy. Every test asserts the observable git state (HEAD ref/target,
    //! worktree file contents, stash stack length) — not just the return value.
    //!
    //! Fixtures are built with git2 in a scratch `TempDir` (deterministic, no
    //! network, no CLI), mirroring the style in `stash.rs`.

    use super::*;
    use crate::git::stash::{list_stashes, ApplyStashOutcome};

    /// Init a scratch repo with a deterministic identity + autocrlf off
    /// (== stash.rs `s9_init`).
    fn cbh_init(dir: &Path) -> git2::Repository {
        let repo = git2::Repository::init(dir).expect("init repo");
        let mut cfg = repo.config().expect("config");
        cfg.set_str("user.name", "Test User").expect("name");
        cfg.set_str("user.email", "test@example.com").expect("email");
        cfg.set_bool("core.autocrlf", false).expect("autocrlf");
        drop(cfg);
        repo
    }

    /// Stage + commit `files` on the CURRENT branch (moves HEAD + worktree).
    fn cbh_commit(dir: &Path, msg: &str, files: &[(&str, &str)]) {
        use crate::git::stage::stage_paths;
        for (name, content) in files {
            std::fs::write(dir.join(name), content).expect("write file");
        }
        stage_paths(
            dir,
            &files.iter().map(|(n, _)| n.to_string()).collect::<Vec<_>>(),
        )
        .expect("stage");
        crate::git::commit::create_commit(dir, msg).expect("commit");
    }

    /// Build a commit on `refname` from `parent`'s tree WITHOUT moving HEAD or the
    /// worktree (== stash.rs `s9_commit_on_ref`). Used to build a divergent tip.
    fn cbh_commit_on_ref(
        repo: &git2::Repository,
        refname: &str,
        parent: &git2::Commit,
        files: &[(&str, &str)],
        msg: &str,
    ) -> git2::Oid {
        let sig = git2::Signature::now("Test User", "test@example.com").expect("sig");
        let mut tb = repo
            .treebuilder(Some(&parent.tree().expect("parent tree")))
            .expect("treebuilder");
        for (name, content) in files {
            let blob = repo.blob(content.as_bytes()).expect("blob");
            tb.insert(name, blob, 0o100644).expect("insert");
        }
        let tree = repo.find_tree(tb.write().expect("tree oid")).expect("tree");
        repo.commit(Some(refname), &sig, &sig, &format!("{msg}\n"), &tree, &[parent])
            .expect("commit on ref")
    }

    /// Full 40-hex oid of the current HEAD commit.
    fn cbh_head_oid(dir: &Path) -> String {
        let repo = git2::Repository::open(dir).expect("open");
        let oid = repo
            .head()
            .expect("HEAD")
            .peel_to_commit()
            .expect("peel")
            .id();
        oid.to_string()
    }

    /// The short branch name HEAD points at, or None when detached/unborn.
    fn cbh_head_branch(dir: &Path) -> Option<String> {
        let repo = git2::Repository::open(dir).expect("open");
        let head = repo.head().ok()?;
        if !head.is_branch() {
            return None;
        }
        head.shorthand().map(str::to_string)
    }

    fn cbh_read(dir: &Path, name: &str) -> String {
        std::fs::read_to_string(dir.join(name)).expect("read file")
    }

    /// True when local branch `name` does not exist in the repo at `dir`.
    fn cbh_branch_absent(dir: &Path, name: &str) -> bool {
        let repo = git2::Repository::open(dir).expect("open");
        let absent = repo.find_branch(name, git2::BranchType::Local).is_err();
        absent
    }

    // ------------------------------------------------------- Scenario 1: clean

    /// §1.5/§7: clean worktree → `{ stashed:false, apply:None }`; HEAD is the new
    /// branch pointing at the requested (older) commit.
    #[test]
    fn cbh_1_clean_worktree_creates_and_checks_out() {
        let dir = crate::testutil::scratch_dir();
        let d = dir.path();
        cbh_init(d);
        cbh_commit(d, "C0", &[("a.txt", "base\n")]);
        let c0 = cbh_head_oid(d);
        cbh_commit(d, "C1", &[("b.txt", "b1\n")]);
        cbh_commit(d, "C2", &[("c.txt", "c1\n")]);

        // Clean worktree; create branch at the OLDER commit C0.
        let res = create_branch_here(d, "feat", &c0).expect("create_branch_here");
        assert_eq!(
            res,
            CreateBranchHereResult {
                stashed: false,
                apply: None
            },
            "clean worktree must not stash"
        );

        // HEAD now on the new branch, at C0.
        assert_eq!(cbh_head_branch(d).as_deref(), Some("feat"), "HEAD is 'feat'");
        assert_eq!(cbh_head_oid(d), c0, "'feat' points at C0");

        // Checkout to C0 removed the files introduced by C1/C2.
        assert_eq!(cbh_read(d, "a.txt"), "base\n");
        assert!(!d.join("b.txt").exists(), "b.txt (C1) gone at C0");
        assert!(!d.join("c.txt").exists(), "c.txt (C2) gone at C0");

        assert_eq!(list_stashes(d).expect("list").len(), 0, "no stash created");
    }

    // ---------------------------------------------- Scenario 2: dirty, applies

    /// §1.5/§7: dirty worktree, branch created at an OLDER commit, changes apply
    /// cleanly → `{ stashed:true, apply:Some(Applied) }`; carried change present
    /// on the new branch; the stash stack is EMPTY (clean pop dropped).
    #[test]
    fn cbh_2_dirty_clean_carry_over() {
        let dir = crate::testutil::scratch_dir();
        let d = dir.path();
        cbh_init(d);
        // a.txt is added at C0 and NEVER changes through C2, so the stashed edit
        // (base a.txt == C2 a.txt) re-applies cleanly onto C0.
        cbh_commit(d, "C0", &[("a.txt", "base\n")]);
        let c0 = cbh_head_oid(d);
        cbh_commit(d, "C1", &[("b.txt", "b1\n")]);
        cbh_commit(d, "C2", &[("c.txt", "c1\n")]);

        // Dirty: unstaged edit to a.txt.
        std::fs::write(d.join("a.txt"), "edited\n").expect("edit a.txt");

        let res = create_branch_here(d, "feat", &c0).expect("create_branch_here");
        assert_eq!(
            res,
            CreateBranchHereResult {
                stashed: true,
                apply: Some(ApplyStashOutcome::Applied)
            },
            "dirty tree carries cleanly across → Applied"
        );

        // HEAD on 'feat' at C0, carrying the edit.
        assert_eq!(cbh_head_branch(d).as_deref(), Some("feat"));
        assert_eq!(cbh_head_oid(d), c0, "'feat' points at C0");
        assert_eq!(
            cbh_read(d, "a.txt"),
            "edited\n",
            "carried edit present on the new branch"
        );

        assert_eq!(
            list_stashes(d).expect("list").len(),
            0,
            "clean pop dropped the stash; stack empty"
        );
    }

    // ------------------------------------------- Scenario 3: dirty, conflicts

    /// §1.5/§7: dirty edit to a file whose content differs at the target commit
    /// → `{ stashed:true, apply:Some(Conflicts{paths}) }`; index has conflicts;
    /// the stash is RETAINED (never lossy).
    #[test]
    fn cbh_3_dirty_conflict_carry_over_retains_stash() {
        let dir = crate::testutil::scratch_dir();
        let d = dir.path();
        cbh_init(d);
        // a.txt changes on every commit so the 3-way apply of the stash onto C0
        // conflicts (ancestor=C2, ours=C0, theirs=dirty all differ).
        cbh_commit(d, "C0", &[("a.txt", "base\n")]);
        let c0 = cbh_head_oid(d);
        cbh_commit(d, "C1", &[("a.txt", "c1\n")]);
        cbh_commit(d, "C2", &[("a.txt", "c2\n")]);

        // Dirty edit to a.txt (base of stash == C2's "c2\n").
        std::fs::write(d.join("a.txt"), "dirty\n").expect("edit a.txt");

        let res = create_branch_here(d, "feat", &c0).expect("create_branch_here");
        assert_eq!(
            res,
            CreateBranchHereResult {
                stashed: true,
                apply: Some(ApplyStashOutcome::Conflicts {
                    paths: vec!["a.txt".to_string()]
                })
            },
            "carry-over onto a divergent file must report Conflicts on a.txt"
        );

        // Branch was created & checked out (Conflicts is a SUCCESS return).
        assert_eq!(cbh_head_branch(d).as_deref(), Some("feat"));
        assert_eq!(cbh_head_oid(d), c0, "'feat' points at C0");

        // Index carries conflict entries; markers present in the worktree.
        let repo = git2::Repository::open(d).expect("reopen");
        assert!(
            repo.index().expect("index").has_conflicts(),
            "index must carry conflict entries"
        );
        assert!(
            cbh_read(d, "a.txt").contains("<<<<<<<"),
            "worktree a.txt must carry conflict markers"
        );

        // DATA SAFETY: conflicting pop retains the stash.
        assert_eq!(
            list_stashes(d).expect("list").len(),
            1,
            "conflicting carry-over must RETAIN the stash (never lossy)"
        );
    }

    // ----------------------------------------- Scenario 4: name already exists

    /// §1.5/§7: name already exists → `Err(BranchExists)` with NOTHING stashed
    /// (stack unchanged) and HEAD unchanged (pre-check runs before any side
    /// effect, even with a dirty tree).
    #[test]
    fn cbh_4_existing_name_no_side_effects() {
        let dir = crate::testutil::scratch_dir();
        let d = dir.path();
        cbh_init(d);
        cbh_commit(d, "C0", &[("a.txt", "base\n")]);
        let c0 = cbh_head_oid(d);

        // A local branch that already exists.
        create_branch(d, "existing").expect("seed branch");

        let head_before = cbh_head_branch(d);
        let oid_before = cbh_head_oid(d);

        // Dirty tree, to prove the pre-check runs BEFORE the auto-stash.
        std::fs::write(d.join("a.txt"), "edited\n").expect("edit a.txt");

        match create_branch_here(d, "existing", &c0) {
            Err(AppError::BranchExists(_)) => {}
            other => panic!("expected BranchExists, got {other:?}"),
        }

        // No stash was created; HEAD + worktree untouched.
        assert_eq!(
            list_stashes(d).expect("list").len(),
            0,
            "BranchExists must NOT strand a stash"
        );
        assert_eq!(cbh_head_branch(d), head_before, "HEAD branch unchanged");
        assert_eq!(cbh_head_oid(d), oid_before, "HEAD oid unchanged");
        assert_eq!(cbh_read(d, "a.txt"), "edited\n", "dirty edit still present");
    }

    // ------------------------------------------------ Scenario 5: bad/unknown oid

    /// §1.5/§7: bad oid → `Err(Git)` before ANY side effect. Covers both a
    /// malformed string and a well-formed but non-existent 40-hex oid; a dirty
    /// tree proves nothing gets stashed.
    #[test]
    fn cbh_5_bad_oid_errors_before_side_effects() {
        let dir = crate::testutil::scratch_dir();
        let d = dir.path();
        cbh_init(d);
        cbh_commit(d, "C0", &[("a.txt", "base\n")]);

        let head_before = cbh_head_branch(d);
        let oid_before = cbh_head_oid(d);

        // Dirty, so a stray auto-stash would be observable.
        std::fs::write(d.join("a.txt"), "edited\n").expect("edit a.txt");

        // (a) malformed oid string.
        match create_branch_here(d, "feat", "not-a-valid-oid") {
            Err(AppError::Git(_)) => {}
            other => panic!("malformed oid: expected Git error, got {other:?}"),
        }

        // (b) well-formed hex but no such commit.
        let missing = "0".repeat(40);
        match create_branch_here(d, "feat", &missing) {
            Err(AppError::Git(_)) => {}
            other => panic!("unknown oid: expected Git error, got {other:?}"),
        }

        assert_eq!(
            list_stashes(d).expect("list").len(),
            0,
            "a bad oid must error before the auto-stash"
        );
        assert!(
            cbh_branch_absent(d, "feat"),
            "no branch should have been created"
        );
        assert_eq!(cbh_head_branch(d), head_before, "HEAD branch unchanged");
        assert_eq!(cbh_head_oid(d), oid_before, "HEAD oid unchanged");
        assert_eq!(cbh_read(d, "a.txt"), "edited\n", "dirty edit still present");
    }

    // ---------------------------------------------- Scenario 6: mid-operation

    /// §1.5/§7: mid-merge (an operation in progress) → `Err(OperationInProgress)`
    /// (via `create_stash`'s `require_clean` gate) and no branch created.
    ///
    /// The mid-op state is produced deterministically with a conflicting
    /// auto-stashing merge (== stash.rs `s9_7`), which pauses the repo in Merge
    /// state — so this scenario is exercised, not skipped.
    #[test]
    fn cbh_6_mid_merge_operation_in_progress() {
        let dir = crate::testutil::scratch_dir();
        let d = dir.path();
        let repo = cbh_init(d);

        cbh_commit(d, "base", &[("x.txt", "base\n"), ("y.txt", "y-base\n")]);
        let base = repo
            .find_commit(repo.head().expect("HEAD").target().expect("oid"))
            .expect("base");
        // topic diverges on x.txt.
        cbh_commit_on_ref(
            &repo,
            "refs/heads/topic",
            &base,
            &[("x.txt", "topic\n")],
            "topic edits x",
        );
        // main diverges on x.txt (guaranteed conflict on merge).
        cbh_commit(d, "main edits x", &[("x.txt", "main\n")]);

        // Dirty an unrelated file so the merge auto-stashes then pauses in Merge.
        std::fs::write(d.join("y.txt"), "y-edited\n").expect("edit y");
        crate::git::merge::merge_branch(d, "topic").expect("merge");

        let repo = git2::Repository::open(d).expect("reopen");
        assert_eq!(
            repo.state(),
            git2::RepositoryState::Merge,
            "conflicting merge over a dirty tree must pause in Merge state"
        );

        let target = cbh_head_oid(d);
        match create_branch_here(d, "feat", &target) {
            Err(AppError::OperationInProgress(_)) => {}
            other => panic!("expected OperationInProgress mid-merge, got {other:?}"),
        }

        assert!(
            cbh_branch_absent(d, "feat"),
            "no branch created while an operation is in progress"
        );
    }
}
