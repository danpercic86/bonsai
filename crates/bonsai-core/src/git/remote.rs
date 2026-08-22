//! Remote operations core (M6 contract §2).
//!
//! fetch (all remotes, sequential, fail-fast); pull (fetch the upstream's
//! remote, then fast-forward ONLY); push (current branch, never force).
//! Pure git2 logic, no Tauri types — testable against the git CLI oracle
//! over local bare-repo remotes (see `tests/remote_cli.rs`). All functions
//! blocking; the command layer wraps them in `spawn_blocking`.
//!
//! Credentials (USER-CONFIRMED strategy, locked): Git's configured credential
//! helper → SSH agent → default. NEVER prompt for or store passwords.

use std::cell::{Cell, RefCell};
use std::path::Path;

use crate::error::AppError;
use crate::git::cred::evict_fresh_on_auth_fail;
// The credential ladder lives in `cred.rs`; re-exported here so the
// `crate::git::remote::…` paths its callers already use keep resolving.
pub(crate) use crate::git::cred::{
    acquire_cred, credential_fill, map_remote_err, CredAttempts, FillOutcome,
};
use crate::git::exec::GitExec;
use crate::git::hooks::{hooks_enabled, run_hook, HookName};
use crate::git::repo::read_head_info;

/// Per-remote fetch outcome (contract §2.1).
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteFetchResult {
    /// Remote name, e.g. "origin".
    pub remote: String,
    /// stats().received_objects() after the fetch.
    pub received_objects: u32,
    /// Number of update_tips callback invocations where old != new
    /// (includes newly created remote-tracking refs).
    pub updated_refs: u32,
}

/// Outcome of fetching every configured remote.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FetchResult {
    /// One entry per configured remote, in remote-list order.
    pub remotes: Vec<RemoteFetchResult>,
    /// P84: best-effort automatic tag reconciliation performed after EVERY fetch
    /// (not gated on refs advancing — the point is to pull down missing/moved tags
    /// even when branches are already up to date). `None` only when auto-sync could
    /// not run at all (e.g. no remote configured, or it was swallowed on error); a
    /// populated report otherwise (which may itself have empty buckets).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tag_auto_sync: Option<crate::git::tag_sync::TagAutoSyncReport>,
}

/// Outcome of a fast-forward-only pull. `WouldNotFastForward` is a RESULT,
/// not an error — nothing failed; the fetch DID land (contract §2.1).
#[derive(Debug, Clone, serde::Serialize)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum PullResult {
    /// behind == 0 (local is equal to or ahead of upstream): nothing to pull.
    UpToDate,
    /// Branch ref + worktree moved from `from` to `to` (full 40-char oids).
    FastForwarded { branch: String, from: String, to: String },
    /// ahead > 0 && behind > 0. NOTHING was changed (fetch already happened —
    /// remote-tracking refs updated — but branch/worktree untouched).
    WouldNotFastForward {
        branch: String,
        ahead: u32,
        behind: u32,
        /// Upstream tracking shorthand ("origin/main") resolved AFTER the fetch —
        /// the exact `name` the frontend hands to `merge_branch`/`rebase_branch`
        /// (P60b). Reuse only; the backend never merges/rebases here.
        upstream: String,
    },
}

/// Outcome of pushing the current branch.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum PushResult {
    /// Remote-tracking ref already equalled the local tip before the push.
    UpToDate { remote: String, branch: String },
    /// `set_upstream` true when the branch had no upstream and we configured
    /// `origin/<branch>` as part of this push (§2.6).
    Pushed {
        remote: String,
        branch: String,
        set_upstream: bool,
    },
}

/// One configured remote (P22 contract §3.1). Wire: camelCase.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteInfo {
    /// Remote name, e.g. "origin".
    pub name: String,
    /// Fetch URL from config. None if unreadable/non-UTF-8. (Push-URL not
    /// surfaced in v1.)
    pub url: Option<String>,
}

/// Opens the repo at `workdir` with `NO_SEARCH` (same as every git/ module).
fn open_repo_at(workdir: &Path) -> Result<git2::Repository, AppError> {
    Ok(git2::Repository::open_ext(
        workdir,
        git2::RepositoryOpenFlags::NO_SEARCH,
        std::iter::empty::<&std::ffi::OsStr>(),
    )?)
}

/// Saturating usize→u32 (counts on the wire; overflow is theoretical).
fn to_u32(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

/// Fetches ONE remote by name: default refspecs, `AutotagOption::Auto`, no
/// prune, fresh `CredAttempts`. Fetch errors mapped via `map_remote_err`.
fn fetch_remote(repo: &git2::Repository, name: &str) -> Result<RemoteFetchResult, AppError> {
    let mut remote = match repo.find_remote(name) {
        Ok(r) => r,
        Err(e) if e.code() == git2::ErrorCode::NotFound => {
            return Err(AppError::NoRemote(format!("remote '{name}' not found")));
        }
        Err(e) => return Err(e.into()),
    };

    let attempts = RefCell::new(CredAttempts::default());
    let updated = Cell::new(0u32);

    let received = {
        let mut callbacks = git2::RemoteCallbacks::new();
        callbacks.credentials(|url, username_from_url, allowed| {
            acquire_cred(repo.workdir(), &attempts, url, username_from_url, allowed)
        });
        callbacks.update_tips(|_refname, old, new| {
            if old != new {
                updated.set(updated.get().saturating_add(1));
            }
            true
        });

        let mut opts = git2::FetchOptions::new();
        opts.remote_callbacks(callbacks);
        opts.download_tags(git2::AutotagOption::Auto);

        if let Err(e) = remote.fetch(&[] as &[&str], Some(&mut opts), None) {
            return Err(evict_fresh_on_auth_fail(
                repo,
                &attempts,
                map_remote_err(e, name),
            ));
        }
        to_u32(remote.stats().received_objects())
    };

    Ok(RemoteFetchResult {
        remote: name.to_string(),
        received_objects: received,
        updated_refs: updated.get(),
    })
}

/// Blocking. Fetches every configured remote, sequentially, in
/// `repo.remotes()` order; fail-fast on the first failing remote (named in
/// the error — contract §9). Works fine with detached or unborn HEAD.
pub fn fetch_all(workdir: &Path) -> Result<FetchResult, AppError> {
    let repo = open_repo_at(workdir)?;

    let names: Vec<String> = repo
        .remotes()?
        .iter()
        .map(|n| n.ok().flatten())
        .filter_map(|n| match n {
            Some(n) => Some(n.to_string()),
            None => {
                eprintln!("bonsai: skipping remote with non-UTF-8 name");
                None
            }
        })
        .collect();
    if names.is_empty() {
        return Err(AppError::NoRemote("no remotes configured".to_string()));
    }

    let mut remotes = Vec::with_capacity(names.len());
    for name in &names {
        remotes.push(fetch_remote(&repo, name)?);
    }
    Ok(FetchResult {
        remotes,
        tag_auto_sync: None,
    })
}

/// Blocking. Fetches the current branch's upstream remote, then fast-forwards
/// ONLY (contract §2.5). Not fast-forwardable → `WouldNotFastForward`,
/// changing nothing. SAFE checkout only — never force; `checkout_tree` runs
/// BEFORE `set_target`, so a conflict leaves ref AND worktree untouched.
pub fn pull_ff(workdir: &Path) -> Result<PullResult, AppError> {
    let repo = open_repo_at(workdir)?;

    let head = read_head_info(&repo)?;
    if head.unborn {
        return Err(AppError::Git(
            "cannot pull: the repository has no commits yet".to_string(),
        ));
    }
    if head.detached {
        return Err(AppError::Git("cannot pull: HEAD is detached".to_string()));
    }
    let name = head
        .branch_name
        .ok_or_else(|| AppError::Git("cannot pull: HEAD has no branch name".to_string()))?;
    let refname = format!("refs/heads/{name}");

    // Upstream must be configured before we bother fetching.
    let branch = repo.find_branch(&name, git2::BranchType::Local)?;
    match branch.upstream() {
        Ok(_) => {}
        Err(e) if e.code() == git2::ErrorCode::NotFound => {
            return Err(AppError::NoUpstream(format!(
                "cannot pull: branch '{name}' has no upstream configured"
            )));
        }
        Err(e) => return Err(e.into()),
    }

    let remote_buf = repo.branch_upstream_remote(&refname)?;
    let remote_name = remote_buf
        .as_str()
        .ok()
        .ok_or_else(|| AppError::Git("upstream remote name is not valid UTF-8".to_string()))?
        .to_string();

    // Fetch ONLY the upstream's remote (the Fetch button covers the rest).
    fetch_remote(&repo, &remote_name)?;

    // RE-RESOLVE the upstream after the fetch — it may have moved.
    let branch = repo.find_branch(&name, git2::BranchType::Local)?;
    let upstream = branch.upstream()?;
    let upstream_oid = upstream.get().target().ok_or_else(|| {
        AppError::Git(format!("upstream of '{name}' has no target commit"))
    })?;
    // Resolved tracking shorthand ("origin/main") from the ALREADY-resolved
    // upstream branch (post-fetch) — the exact name the frontend passes back to
    // merge_branch/rebase_branch on a non-FF result. NOT recomputed from config;
    // falls back to "{remote}/{local}" only if the ref name is unreadable.
    let upstream_shorthand = upstream
        .name()
        .ok()
        .flatten()
        .map(str::to_string)
        .unwrap_or_else(|| format!("{remote_name}/{name}"));
    let local_oid = branch
        .get()
        .target()
        .ok_or_else(|| AppError::Git(format!("branch '{name}' has no target commit")))?;

    let (ahead, behind) = repo.graph_ahead_behind(local_oid, upstream_oid)?;
    if behind == 0 {
        // Equal or ahead-only: nothing to pull.
        return Ok(PullResult::UpToDate);
    }
    if ahead > 0 {
        // Diverged: change NOTHING (the fetch already landed — that's fine).
        return Ok(PullResult::WouldNotFastForward {
            branch: name,
            ahead: to_u32(ahead),
            behind: to_u32(behind),
            upstream: upstream_shorthand,
        });
    }

    // Fast-forward (ahead == 0 && behind > 0).
    let obj = repo.find_object(upstream_oid, None)?;
    let mut opts = git2::build::CheckoutBuilder::new();
    opts.safe(); // DEFAULT SAFE MODE — never .force()
    match repo.checkout_tree(&obj, Some(&mut opts)) {
        Ok(()) => {}
        Err(e) if e.code() == git2::ErrorCode::Conflict => {
            return Err(AppError::CheckoutConflict(
                "cannot pull: local changes would be overwritten by the update. \
                 Commit or discard them first."
                    .to_string(),
            ));
        }
        Err(e) => return Err(e.into()),
    }
    repo.find_reference(&refname)?
        .set_target(upstream_oid, &format!("pull: fast-forward to {upstream_oid}"))?;

    Ok(PullResult::FastForwarded {
        branch: name,
        from: local_oid.to_string(),
        to: upstream_oid.to_string(),
    })
}

/// Blocking. Pushes the current branch to its upstream — or, when no upstream
/// is configured, to `origin/<branch>` AND sets the upstream (contract §2.6,
/// locked decision §9). NEVER force: the refspec has NO leading '+'.
///
/// `skip_hooks` (P59a-2): `true` ≡ `git push --no-verify`; otherwise the
/// effective toggle is `bonsai.runHooks` (default true). When enabled, the
/// `pre-push` hook runs through `runner` BEFORE the libgit2 push (git2 fires no
/// hooks) — a non-zero exit aborts as [`AppError::HookRejected`] with nothing
/// pushed. The push itself still goes through libgit2 + the in-process
/// credential cache; only the hook uses `runner`.
pub fn push_current(
    workdir: &Path,
    runner: &dyn GitExec,
    skip_hooks: bool,
) -> Result<PushResult, AppError> {
    let repo = open_repo_at(workdir)?;

    let head = read_head_info(&repo)?;
    if head.unborn {
        return Err(AppError::Git(
            "cannot push: the repository has no commits yet".to_string(),
        ));
    }
    if head.detached {
        return Err(AppError::Git("cannot push: HEAD is detached".to_string()));
    }
    let name = head
        .branch_name
        .ok_or_else(|| AppError::Git("cannot push: HEAD has no branch name".to_string()))?;
    let refname = format!("refs/heads/{name}");

    let mut branch = repo.find_branch(&name, git2::BranchType::Local)?;
    let local_tip = branch
        .get()
        .target()
        .ok_or_else(|| AppError::Git(format!("branch '{name}' has no target commit")))?;

    // Resolve target: configured upstream, else origin/<branch> + set upstream.
    let config = repo.config()?;
    let (remote_name, remote_ref, set_upstream_after) = if repo
        .branch_upstream_remote(&refname)
        .is_ok()
        && repo.branch_upstream_name(&refname).is_ok()
    {
        let remote_buf = repo.branch_upstream_remote(&refname)?;
        let remote_name = remote_buf
            .as_str()
            .ok()
            .ok_or_else(|| AppError::Git("upstream remote name is not valid UTF-8".to_string()))?
            .to_string();
        // branch.<name>.merge already IS "refs/heads/<x>" (contract §2.6).
        let remote_ref = config.get_string(&format!("branch.{name}.merge"))?;
        (remote_name, remote_ref, false)
    } else {
        match repo.find_remote("origin") {
            Ok(_) => {}
            Err(e) if e.code() == git2::ErrorCode::NotFound => {
                return Err(AppError::NoRemote(format!(
                    "cannot push: branch '{name}' has no upstream and no 'origin' remote exists"
                )));
            }
            Err(e) => return Err(e.into()),
        }
        ("origin".to_string(), refname.clone(), true)
    };

    // Local up-to-date short-circuit: no network round-trip (decision §9).
    let remote_branch = remote_ref
        .strip_prefix("refs/heads/")
        .unwrap_or(remote_ref.as_str());
    let prev_remote_tip = repo
        .find_reference(&format!("refs/remotes/{remote_name}/{remote_branch}"))
        .ok()
        .and_then(|r| r.target());
    if prev_remote_tip == Some(local_tip) {
        return Ok(PushResult::UpToDate {
            remote: remote_name,
            branch: name,
        });
    }

    // P59a-2: run the `pre-push` hook BEFORE the libgit2 push. Args =
    // `<remote-name> <remote-url>`; stdin = one line per pushed ref (git's
    // contract). A non-zero exit aborts with HookRejected — nothing is pushed.
    // Placed AFTER the up-to-date short-circuit (nothing to push ⇒ git runs no
    // pre-push either) and BEFORE any credential/network work or ref update.
    if hooks_enabled(&config, skip_hooks) {
        let remote_url = remote_url_of(&repo, &remote_name);
        let stdin = build_pre_push_stdin(&refname, local_tip, &remote_ref, prev_remote_tip);
        run_hook(
            runner,
            workdir,
            HookName::PrePush,
            &[remote_name.clone(), remote_url],
            Some(stdin.as_bytes()),
        )?;
    }

    let attempts = RefCell::new(CredAttempts::default());
    let rejected: RefCell<Option<String>> = RefCell::new(None);
    {
        let mut remote = match repo.find_remote(&remote_name) {
            Ok(r) => r,
            Err(e) if e.code() == git2::ErrorCode::NotFound => {
                return Err(AppError::NoRemote(format!(
                    "remote '{remote_name}' not found"
                )));
            }
            Err(e) => return Err(e.into()),
        };

        let mut callbacks = git2::RemoteCallbacks::new();
        callbacks.credentials(|url, username_from_url, allowed| {
            acquire_cred(repo.workdir(), &attempts, url, username_from_url, allowed)
        });
        callbacks.push_update_reference(|_refname, status| {
            if let Some(msg) = status {
                *rejected.borrow_mut() = Some(msg.to_string());
            }
            Ok(())
        });

        let mut opts = git2::PushOptions::new();
        opts.remote_callbacks(callbacks);

        // NO leading '+' — never force.
        let refspec = format!("{refname}:{remote_ref}");
        if let Err(e) = remote.push(&[refspec.as_str()], Some(&mut opts)) {
            return Err(evict_fresh_on_auth_fail(
                &repo,
                &attempts,
                map_remote_err(e, &remote_name),
            ));
        }
    }

    if let Some(msg) = rejected.into_inner() {
        return Err(AppError::PushRejected(format!(
            "push rejected by remote: {msg}. Bonsai v1 never force-pushes — fetch/pull first."
        )));
    }

    if set_upstream_after {
        // libgit2's push already updated refs/remotes/<remote>/<name> via the
        // default tracking refspec, so set_upstream finds the ref.
        branch.set_upstream(Some(&format!("{remote_name}/{name}")))?;
    }

    Ok(PushResult::Pushed {
        remote: remote_name,
        branch: name,
        set_upstream: set_upstream_after,
    })
}

/// Lease failure: the remote advanced (or its ref was deleted) since the last
/// fetch (P37 §3.2).
fn lease_moved_msg(remote: &str, branch: &str) -> String {
    format!(
        "force-push refused: '{remote}/{branch}' has moved on the remote since you last \
         fetched — someone may have pushed. Fetch and review before force-pushing again."
    )
}

/// Lease failure: no remote-tracking ref to lease against (P37 §3.2).
fn lease_no_baseline_msg(remote: &str, branch: &str) -> String {
    format!(
        "cannot force-push with lease: no remote-tracking ref for '{remote}/{branch}'. \
         Fetch first so Bonsai knows the remote's current tip."
    )
}

/// Blocking. Force-push the current branch to its configured upstream WITH A
/// LEASE: refuse if the remote branch moved past the oid we last fetched
/// (someone else pushed), otherwise force-update it. For republishing a
/// rewritten history (amend / interactive rebase). NEVER a bare force.
///
/// Requires a configured upstream (unlike `push_current`, which can create
/// origin/<branch>). Lease baseline = the remote-tracking ref
/// `refs/remotes/<remote>/<branch>` (git's default --force-with-lease).
///
/// P59b hardening: the branch/upstream/baseline resolution stays in git2, but
/// the PUSH runs through the git binary (`runner`) as
/// `git push --force-with-lease=<remote_ref>:<expected> --force-if-includes …`.
/// git performs the expected-old-value compare-and-swap at push-negotiation
/// time (atomic on capable servers), eliminating P37's client-side
/// ls-remote→push TOCTOU window. `SpawnGitExec` never prompts and relies on
/// git's own configured credential helper for this one op (OQ-B2) — the same
/// never-prompt helper used for reads.
///
/// Errors: unborn/detached/no-name -> `Git`; no upstream -> `NoUpstream`;
/// no remote-tracking baseline -> `PushRejected` (fetch first); baseline already
/// equals the local tip -> `UpToDate` (NO git spawned); a non-zero `git push`
/// is mapped by `classify_push_stderr` — git's atomic lease refusal
/// (stale info / [rejected] / force-with-lease) -> `PushRejected` (wrapped with
/// the contextual `lease_moved_msg`); auth -> `AuthFailed`; connect/DNS/TLS ->
/// `NetworkError`; anything else -> `Git`.
pub fn force_push_with_lease(
    workdir: &Path,
    runner: &dyn GitExec,
    skip_hooks: bool,
) -> Result<PushResult, AppError> {
    let repo = open_repo_at(workdir)?;

    let head = read_head_info(&repo)?;
    if head.unborn {
        return Err(AppError::Git(
            "cannot force-push: repository has no commits yet".to_string(),
        ));
    }
    if head.detached {
        return Err(AppError::Git(
            "cannot force-push: HEAD is detached".to_string(),
        ));
    }
    let name = head
        .branch_name
        .ok_or_else(|| AppError::Git("cannot force-push: HEAD has no branch name".to_string()))?;
    let refname = format!("refs/heads/{name}");

    let branch = repo.find_branch(&name, git2::BranchType::Local)?;
    let local_tip = branch
        .get()
        .target()
        .ok_or_else(|| AppError::Git(format!("branch '{name}' has no target commit")))?;

    // Upstream is REQUIRED (force-with-lease republishes an existing upstream).
    // Determine it from CONFIG (branch.<name>.remote + .merge), NOT
    // `branch.upstream()`: the latter also requires the remote-tracking ref to
    // exist, which would conflate "no upstream configured" (-> NoUpstream) with
    // "configured but never fetched" (the no-baseline PushRejected below).
    let no_upstream = || {
        AppError::NoUpstream(format!(
            "cannot force-push: '{name}' has no upstream; use a normal push"
        ))
    };
    let remote_buf = match repo.branch_upstream_remote(&refname) {
        Ok(b) => b,
        Err(e) if e.code() == git2::ErrorCode::NotFound => return Err(no_upstream()),
        Err(e) => return Err(e.into()),
    };
    let remote_name = remote_buf
        .as_str()
        .ok()
        .ok_or_else(|| AppError::Git("upstream remote name is not valid UTF-8".to_string()))?
        .to_string();
    // branch.<name>.merge already IS "refs/heads/<x>".
    let config = repo.config()?;
    let remote_ref = match config.get_string(&format!("branch.{name}.merge")) {
        Ok(s) => s,
        Err(e) if e.code() == git2::ErrorCode::NotFound => return Err(no_upstream()),
        Err(e) => return Err(e.into()),
    };
    let remote_branch = remote_ref
        .strip_prefix("refs/heads/")
        .unwrap_or(remote_ref.as_str())
        .to_string();

    // --- lease baseline: the remote-tracking ref we last fetched ---
    let tracking = format!("refs/remotes/{remote_name}/{remote_branch}");
    let expected = repo
        .find_reference(&tracking)
        .ok()
        .and_then(|r| r.target());
    let expected = match expected {
        Some(oid) => oid,
        None => {
            return Err(AppError::PushRejected(lease_no_baseline_msg(
                &remote_name,
                &remote_branch,
            )));
        }
    };

    if expected == local_tip {
        // Baseline already equals the local tip: nothing to force.
        return Ok(PushResult::UpToDate {
            remote: remote_name,
            branch: name,
        });
    }

    // P59a-2: run the `pre-push` hook BEFORE the git-binary force-push (same
    // `runner`). Args = `<remote-name> <remote-url>`; stdin = the single pushed
    // ref, its remote-oid = the resolved lease baseline `expected` (never zero
    // here — a missing baseline already returned PushRejected above). A non-zero
    // exit aborts with HookRejected — nothing is pushed. Placed AFTER the
    // up-to-date short-circuit so an up-to-date force-push spawns no git at all.
    if hooks_enabled(&config, skip_hooks) {
        let remote_url = remote_url_of(&repo, &remote_name);
        let stdin = build_pre_push_stdin(&refname, local_tip, &remote_ref, Some(expected));
        run_hook(
            runner,
            workdir,
            HookName::PrePush,
            &[remote_name.clone(), remote_url],
            Some(stdin.as_bytes()),
        )?;
    }

    // Hand the resolved baseline to the git binary so IT performs the atomic
    // `--force-with-lease` expected-old-value check at push-negotiation time
    // (P59b B-D2). This collapses P37's two-step ls-remote→push compare into
    // git's single negotiated push, closing OUR TOCTOU window. `--force-if-
    // includes` additionally refuses a rewrite made without having seen the
    // remote tip. `SpawnGitExec` never prompts and uses git's configured
    // credential helper for this one op (OQ-B2).
    let expected_hex = expected.to_string();
    let args = build_force_push_args(&remote_name, &name, &remote_ref, &expected_hex);
    let arg_refs: Vec<&str> = args.iter().map(String::as_str).collect();
    let out = runner.exec(&arg_refs, workdir, None, &[])?;
    if out.success {
        return Ok(PushResult::Pushed {
            remote: remote_name,
            branch: name,
            set_upstream: false,
        });
    }
    // Non-zero exit: classify git's stderr. A lease/lease-includes refusal is a
    // `PushRejected`; wrap it with the contextual `lease_moved_msg` (which names
    // the ref that moved) followed by git's own stderr tail so the UI shows both
    // the actionable hint and git's verbatim reason.
    Err(match classify_push_stderr(&out.stderr) {
        AppError::PushRejected(git_tail) => AppError::PushRejected(format!(
            "{}\n{git_tail}",
            lease_moved_msg(&remote_name, &remote_branch)
        )),
        other => other,
    })
}

/// The fetch URL of remote `name`, or an empty string when it is unset /
/// unreadable / non-UTF-8. Used as the `pre-push` hook's 2nd arg (git passes
/// the remote URL there); an empty string is a harmless placeholder git itself
/// also tolerates when a remote has no URL.
fn remote_url_of(repo: &git2::Repository, name: &str) -> String {
    repo.find_remote(name)
        .ok()
        .and_then(|r| r.url().ok().map(str::to_string))
        .unwrap_or_default()
}

/// Build the `pre-push` hook stdin (git's contract, `githooks(5)`): one line per
/// pushed ref, `<local-ref> SP <local-oid> SP <remote-ref> SP <remote-oid> LF`.
/// `remote_oid` is the baseline/expected remote tip, or 40 zeros when the remote
/// ref is new (no baseline). Pure / unit-tested — Bonsai only ever pushes ONE
/// ref per op, so this is a single line.
fn build_pre_push_stdin(
    local_ref: &str,
    local_oid: git2::Oid,
    remote_ref: &str,
    remote_oid: Option<git2::Oid>,
) -> String {
    const ZERO_OID: &str = "0000000000000000000000000000000000000000";
    let remote = remote_oid.map_or_else(|| ZERO_OID.to_string(), |o| o.to_string());
    format!("{local_ref} {local_oid} {remote_ref} {remote}\n")
}

/// Assemble the atomic-lease force-push argv (P59b B2). Pure / unit-tested.
///
/// `git push --force-with-lease=<remote_ref>:<expected_hex> --force-if-includes
///  <remote> refs/heads/<branch>:<remote_ref>` — git does the expected-old-
/// value compare at negotiation time (atomic), so there is no client-side
/// ls-remote→push race. `remote_ref` is the upstream's `branch.<b>.merge`
/// (already `refs/heads/<x>`).
///
/// The refspec deliberately has **no leading `+`**: `--force-with-lease` itself
/// supplies the (CONDITIONAL) force. A `+` is an UNCONDITIONAL force that git
/// applies BEFORE the lease and which therefore OVERRIDES `--force-with-lease`
/// entirely — verified empirically — defeating P59b's atomic guarantee. (The
/// old git2 path needed `+` because it did the lease compare client-side; the
/// git binary must not.) This diverges from the literal B2 pseudocode, which
/// carried the `+` over from the git2 code.
///
/// `--no-verify` (P59a-2): the git binary would fire the `pre-push` hook ITSELF
/// on this push. Bonsai runs `pre-push` explicitly BEFORE this (via
/// [`run_hook`], so a non-zero exit is a structured [`AppError::HookRejected`]
/// and `skip_hooks` / `bonsai.runHooks` are honored) — so we suppress git's OWN
/// run here to avoid a double execution and to make `skip_hooks` actually skip
/// (git otherwise ignores our toggle). Unlike `push_current` (libgit2, which
/// fires no hooks), the force-push MUST pass `--no-verify` for the toggle to work.
fn build_force_push_args(
    remote: &str,
    branch: &str,
    remote_ref: &str,
    expected_hex: &str,
) -> Vec<String> {
    vec![
        "push".to_string(),
        format!("--force-with-lease={remote_ref}:{expected_hex}"),
        "--force-if-includes".to_string(),
        "--no-verify".to_string(),
        // F-A5-d: `--` end-of-options so the positional <remote> <refspec> can
        // never be reinterpreted as flags (defense-in-depth against a remote
        // name / branch that begins with `-`; config-write-level arg-injection).
        "--".to_string(),
        remote.to_string(),
        format!("refs/heads/{branch}:{remote_ref}"),
    ]
}

/// Compact tail (last few non-empty lines) of `git push` stderr, for a readable
/// error message without dumping the whole progress stream.
fn push_stderr_tail(stderr: &str) -> String {
    let lines: Vec<&str> = stderr.lines().filter(|l| !l.trim().is_empty()).collect();
    if lines.is_empty() {
        return "git push failed".to_string();
    }
    let start = lines.len().saturating_sub(6);
    lines[start..].join("\n")
}

/// Map a non-zero `git push` stderr to a typed [`AppError`] (P59b B-D3). Pure /
/// unit-tested. git's atomic lease / lease-includes refusal
/// (`stale info` / `[rejected]` / `force-with-lease` / `remote ref updated since
/// checkout`) => `PushRejected` (the caller prepends the contextual
/// `lease_moved_msg`); credential failures => `AuthFailed`; connect / DNS / TLS
/// => `NetworkError`; anything else => `Git` (stderr tail).
fn classify_push_stderr(stderr: &str) -> AppError {
    let low = stderr.to_lowercase();
    let tail = push_stderr_tail(stderr);
    // git's atomic --force-with-lease / --force-if-includes refusal.
    if low.contains("stale info")
        || low.contains("[rejected]")
        || low.contains("force-with-lease")
        || low.contains("remote ref updated since checkout")
    {
        return AppError::PushRejected(tail);
    }
    // Never-prompt policy => git fails fast with one of these when creds are
    // needed but the configured helper can't supply them non-interactively.
    if low.contains("authentication failed")
        || low.contains("could not read username")
        || low.contains("could not read password")
        || low.contains("terminal prompts disabled")
    {
        return AppError::AuthFailed(tail);
    }
    // Connectivity / transport.
    if low.contains("could not resolve host")
        || low.contains("could not connect")
        || low.contains("connection refused")
        || low.contains("connection timed out")
        || low.contains("failed to connect")
        || low.contains("network is unreachable")
        || low.contains("ssl")
        || low.contains("tls")
    {
        return AppError::NetworkError(tail);
    }
    AppError::Git(tail)
}

// ============================================================ P22 §3 remotes
// management (add / remove / rename / set-url / list). All LOCAL config ops —
// no network, no credentials.

/// Blocking. Enumerate configured remotes (name + fetch URL), sorted
/// case-insensitively by name (P22 §3.2/§3.3). Empty repo / no remotes →
/// `Ok(vec![])` (NOT an error — unlike `fetch_all`).
pub fn list_remotes(workdir: &Path) -> Result<Vec<RemoteInfo>, AppError> {
    let repo = open_repo_at(workdir)?;
    let mut out = Vec::new();
    for n in repo.remotes()?.iter().map(|n| n.ok().flatten()) {
        let name = match n {
            Some(n) => n.to_string(),
            None => {
                eprintln!("bonsai: skipping remote with non-UTF-8 name");
                continue;
            }
        };
        let url = repo
            .find_remote(&name)
            .ok()
            .and_then(|r| r.url().ok().map(str::to_string));
        out.push(RemoteInfo { name, url });
    }
    out.sort_by(|a, b| {
        a.name
            .to_lowercase()
            .cmp(&b.name.to_lowercase())
            .then_with(|| a.name.cmp(&b.name))
    });
    Ok(out)
}

/// Blocking. Add remote `name` → `url` (`Repository::remote`, P22 §3.4).
/// Errors: invalid name (`git2::Remote::is_valid_name`) → `InvalidName`;
/// duplicate (Exists) → `Git("remote '<name>' already exists")`.
pub fn add_remote(workdir: &Path, name: &str, url: &str) -> Result<(), AppError> {
    if !git2::Remote::is_valid_name(name) {
        return Err(AppError::InvalidName(format!("invalid remote name: '{name}'")));
    }
    let repo = open_repo_at(workdir)?;
    let result = repo.remote(name, url).map(|_remote| ());
    match result {
        Ok(()) => Ok(()),
        Err(e) if e.code() == git2::ErrorCode::Exists => {
            Err(AppError::Git(format!("remote '{name}' already exists")))
        }
        Err(e) => Err(e.into()),
    }
}

/// Blocking. Remove remote `name` (`Repository::remote_delete` — also drops its
/// remote-tracking refs + config, P22 §3.4). Errors: not found → `NoRemote`.
pub fn remove_remote(workdir: &Path, name: &str) -> Result<(), AppError> {
    let repo = open_repo_at(workdir)?;
    match repo.remote_delete(name) {
        Ok(()) => Ok(()),
        Err(e) if e.code() == git2::ErrorCode::NotFound => {
            Err(AppError::NoRemote(format!("remote '{name}' not found")))
        }
        Err(e) => Err(e.into()),
    }
}

/// Blocking. Rename remote `name` → `new_name` (`Repository::remote_rename` —
/// moves `refs/remotes/<name>/*` and rewrites config, P22 §3.4). The returned
/// non-default-refspec "problem" list is logged (`eprintln`) and ignored — a
/// non-standard refspec that could not be auto-rewritten is not fatal to the
/// rename. Errors: not found → `NoRemote`; invalid new name → `InvalidName`;
/// new name exists → `Git("remote '<new_name>' already exists")`.
pub fn rename_remote(workdir: &Path, name: &str, new_name: &str) -> Result<(), AppError> {
    if !git2::Remote::is_valid_name(new_name) {
        return Err(AppError::InvalidName(format!(
            "invalid remote name: '{new_name}'"
        )));
    }
    let repo = open_repo_at(workdir)?;
    match repo.remote_rename(name, new_name) {
        Ok(problems) => {
            if !problems.is_empty() {
                let listed: Vec<&str> = problems.iter().filter_map(Result::ok).flatten().collect();
                eprintln!(
                    "bonsai: rename_remote('{name}' -> '{new_name}') left \
                     non-default refspecs unmodified: {listed:?}"
                );
            }
            Ok(())
        }
        Err(e) if e.code() == git2::ErrorCode::NotFound => {
            Err(AppError::NoRemote(format!("remote '{name}' not found")))
        }
        Err(e) if e.code() == git2::ErrorCode::Exists => {
            Err(AppError::Git(format!("remote '{new_name}' already exists")))
        }
        Err(e) => Err(e.into()),
    }
}

/// Blocking. Set the FETCH url of remote `name` (`Repository::remote_set_url`,
/// push=false, P22 §3.4). Errors: not found → `NoRemote`; invalid url → `Git`.
pub fn set_remote_url(workdir: &Path, name: &str, url: &str) -> Result<(), AppError> {
    let repo = open_repo_at(workdir)?;
    // libgit2's `remote_set_url` writes the config key unconditionally (it does
    // NOT error on a missing remote, unlike the `git` CLI), so pre-check
    // existence to honor the contract's NoRemote mapping + CLI parity.
    match repo.find_remote(name) {
        Ok(_) => {}
        Err(e) if e.code() == git2::ErrorCode::NotFound => {
            return Err(AppError::NoRemote(format!("remote '{name}' not found")));
        }
        Err(e) => return Err(e.into()),
    }
    match repo.remote_set_url(name, url) {
        Ok(()) => Ok(()),
        Err(e) if e.code() == git2::ErrorCode::NotFound => {
            Err(AppError::NoRemote(format!("remote '{name}' not found")))
        }
        Err(e) => Err(e.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---------------------------------------- §2.1 wire shape (TS mirrors)

    /// The serde tag/casing must match the TS types exactly:
    /// `{ "kind": "upToDate" } | { "kind": "pushed", ..., "setUpstream": ... }`.
    #[test]
    fn wire_shapes_are_camel_case_tagged() {
        let v = serde_json::to_value(PullResult::UpToDate).expect("json");
        assert_eq!(v, serde_json::json!({ "kind": "upToDate" }));

        let v = serde_json::to_value(PullResult::WouldNotFastForward {
            branch: "main".to_string(),
            ahead: 2,
            behind: 1,
            upstream: "origin/main".to_string(),
        })
        .expect("json");
        assert_eq!(
            v,
            serde_json::json!({ "kind": "wouldNotFastForward", "branch": "main", "ahead": 2, "behind": 1, "upstream": "origin/main" })
        );

        let v = serde_json::to_value(PushResult::Pushed {
            remote: "origin".to_string(),
            branch: "topic".to_string(),
            set_upstream: true,
        })
        .expect("json");
        assert_eq!(
            v,
            serde_json::json!({ "kind": "pushed", "remote": "origin", "branch": "topic", "setUpstream": true })
        );

        let v = serde_json::to_value(FetchResult {
            remotes: vec![RemoteFetchResult {
                remote: "origin".to_string(),
                received_objects: 12,
                updated_refs: 1,
            }],
            tag_auto_sync: None,
        })
        .expect("json");
        assert_eq!(
            v,
            serde_json::json!({ "remotes": [{ "remote": "origin", "receivedObjects": 12, "updatedRefs": 1 }] })
        );
    }

    // -------------------------------------- P37 §3.2 lease message helpers

    /// The lease messages interpolate the remote and branch names.
    #[test]
    fn lease_messages_interpolate_remote_and_branch() {
        let moved = lease_moved_msg("origin", "main");
        assert!(moved.contains("'origin/main'"), "{moved}");
        assert!(moved.contains("moved"), "{moved}");
        assert!(moved.contains("Fetch"), "{moved}");

        let no_baseline = lease_no_baseline_msg("upstream", "topic");
        assert!(no_baseline.contains("'upstream/topic'"), "{no_baseline}");
        assert!(no_baseline.contains("Fetch first"), "{no_baseline}");
    }

    // ------------------------------------- P59b build_force_push_args (pure)

    /// The atomic-lease argv: `push`, force-with-lease keyed on
    /// `<remote_ref>:<baseline>`, `--force-if-includes`, remote, then a PLAIN
    /// (no `+`) refspec — the lease itself supplies the conditional force; a `+`
    /// would be an unconditional force that overrides the lease (P59b).
    #[test]
    fn force_push_args_exact_vec() {
        let args = build_force_push_args(
            "origin",
            "main",
            "refs/heads/main",
            "1111111111111111111111111111111111111111",
        );
        assert_eq!(
            args,
            vec![
                "push".to_string(),
                "--force-with-lease=refs/heads/main:1111111111111111111111111111111111111111"
                    .to_string(),
                "--force-if-includes".to_string(),
                "--no-verify".to_string(),
                "--".to_string(),
                "origin".to_string(),
                "refs/heads/main:refs/heads/main".to_string(),
            ]
        );
        // No leading '+': an unconditional force would defeat --force-with-lease.
        assert!(!args[6].starts_with('+'), "refspec must not force unconditionally");
        // --no-verify present so git does not re-run the pre-push hook we ran.
        assert!(args.contains(&"--no-verify".to_string()), "must suppress git's own pre-push");
        // F-A5-d: `--` immediately precedes the positional remote + refspec.
        assert_eq!(args[4], "--", "end-of-options guards the positionals");
    }

    /// A slashed branch name flows verbatim into both the lease ref and the
    /// refspec (guards nested-ref interpolation).
    #[test]
    fn force_push_args_nested_branch() {
        let args = build_force_push_args(
            "upstream",
            "feature/x",
            "refs/heads/feature/x",
            "2222222222222222222222222222222222222222",
        );
        assert_eq!(
            args[1],
            "--force-with-lease=refs/heads/feature/x:2222222222222222222222222222222222222222"
        );
        assert_eq!(args[4], "--");
        assert_eq!(args[5], "upstream");
        assert_eq!(args[6], "refs/heads/feature/x:refs/heads/feature/x");
    }

    // ------------------------------------- P59a-2 build_pre_push_stdin (pure)

    /// An existing remote ref: the baseline oid appears as the 4th field, and the
    /// line is `<local-ref> <local-oid> <remote-ref> <remote-oid>\n`.
    #[test]
    fn pre_push_stdin_existing_ref() {
        let local = git2::Oid::from_str("1111111111111111111111111111111111111111").expect("oid");
        let remote = git2::Oid::from_str("2222222222222222222222222222222222222222").expect("oid");
        let line = build_pre_push_stdin("refs/heads/main", local, "refs/heads/main", Some(remote));
        assert_eq!(
            line,
            "refs/heads/main 1111111111111111111111111111111111111111 \
             refs/heads/main 2222222222222222222222222222222222222222\n"
        );
    }

    /// A NEW remote ref (no baseline): the remote-oid field is 40 zeros, exactly
    /// as git synthesizes it for a create.
    #[test]
    fn pre_push_stdin_new_ref_is_zeros() {
        let local = git2::Oid::from_str("3333333333333333333333333333333333333333").expect("oid");
        let line = build_pre_push_stdin("refs/heads/feature/x", local, "refs/heads/feature/x", None);
        assert_eq!(
            line,
            "refs/heads/feature/x 3333333333333333333333333333333333333333 \
             refs/heads/feature/x 0000000000000000000000000000000000000000\n"
        );
        // Trailing LF so `read` in a `while read` hook loop terminates the line.
        assert!(line.ends_with('\n'));
    }

    // ------------------------------------- P59b classify_push_stderr (pure)

    /// git's atomic --force-with-lease / --force-if-includes refusal maps to
    /// `PushRejected` (the caller then prepends the contextual lease_moved_msg).
    #[test]
    fn classify_lease_refusal_is_push_rejected() {
        for s in [
            " ! [rejected]        main -> main (stale info)",
            "error: remote ref updated since checkout",
            "! refusing to lose commits: force-with-lease",
        ] {
            assert!(
                matches!(classify_push_stderr(s), AppError::PushRejected(_)),
                "stderr should map to PushRejected: {s:?}"
            );
        }
    }

    /// Never-prompt credential failures map to `AuthFailed`.
    #[test]
    fn classify_auth_failure_is_auth_failed() {
        for s in [
            "fatal: Authentication failed for 'https://example.com/x.git/'",
            "fatal: could not read Username for 'https://example.com': terminal prompts disabled",
        ] {
            assert!(
                matches!(classify_push_stderr(s), AppError::AuthFailed(_)),
                "stderr should map to AuthFailed: {s:?}"
            );
        }
    }

    /// Connect / DNS / TLS failures map to `NetworkError`.
    #[test]
    fn classify_network_failure_is_network_error() {
        for s in [
            "fatal: unable to access 'https://x/': Could not resolve host: x",
            "fatal: unable to access 'https://x/': SSL certificate problem",
        ] {
            assert!(
                matches!(classify_push_stderr(s), AppError::NetworkError(_)),
                "stderr should map to NetworkError: {s:?}"
            );
        }
    }

    /// Anything unrecognized falls through to a generic `Git` error carrying the
    /// stderr tail.
    #[test]
    fn classify_unknown_is_git() {
        match classify_push_stderr("fatal: something entirely unexpected happened") {
            AppError::Git(m) => assert!(m.contains("unexpected"), "{m}"),
            other => panic!("expected Git, got {other:?}"),
        }
    }

    /// The stderr tail keeps only the last few non-empty lines and never panics
    /// on empty input.
    #[test]
    fn push_stderr_tail_is_compact() {
        assert_eq!(push_stderr_tail("   \n  \n"), "git push failed");
        let many = (0..20).map(|i| format!("line{i}")).collect::<Vec<_>>().join("\n");
        let tail = push_stderr_tail(&many);
        assert_eq!(tail.lines().count(), 6);
        assert!(tail.contains("line19"));
        assert!(!tail.contains("line13"));
    }

    // ------------------------------------------------ §8.3 RemoteInfo shape

    /// `RemoteInfo` serializes camelCase with `url: null` when absent.
    #[test]
    fn remote_info_wire_shape() {
        let v = serde_json::to_value(RemoteInfo {
            name: "origin".to_string(),
            url: Some("https://example.com/repo.git".to_string()),
        })
        .expect("json");
        assert_eq!(
            v,
            serde_json::json!({ "name": "origin", "url": "https://example.com/repo.git" })
        );

        let v = serde_json::to_value(RemoteInfo {
            name: "origin".to_string(),
            url: None,
        })
        .expect("json");
        assert_eq!(v, serde_json::json!({ "name": "origin", "url": null }));
    }

    /// `list_remotes` sort: case-insensitive primary, exact tie-break.
    #[test]
    fn remote_info_sort_order() {
        let mut v = [
            RemoteInfo { name: "Zeta".to_string(), url: None },
            RemoteInfo { name: "alpha".to_string(), url: None },
            RemoteInfo { name: "Beta".to_string(), url: None },
            RemoteInfo { name: "beta".to_string(), url: None },
        ];
        v.sort_by(|a, b| {
            a.name
                .to_lowercase()
                .cmp(&b.name.to_lowercase())
                .then_with(|| a.name.cmp(&b.name))
        });
        let names: Vec<&str> = v.iter().map(|r| r.name.as_str()).collect();
        // case-insensitive order: alpha, Beta/beta (tie → 'B' < 'b'), Zeta.
        assert_eq!(names, vec!["alpha", "Beta", "beta", "Zeta"]);
    }
}
