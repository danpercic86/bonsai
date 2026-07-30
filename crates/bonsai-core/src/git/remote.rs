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
    WouldNotFastForward { branch: String, ahead: u32, behind: u32 },
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

/// Sentinel in the git2 error raised when every credential source is
/// exhausted — `map_remote_err` keys the `authFailed` mapping off it.
pub(crate) const CRED_EXHAUSTED_MSG: &str = "bonsai: no usable credentials";

/// Which credential source to try next. Pure decision logic — no git2 Cred
/// construction (unit-testable offline, contract §2.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CredMethod {
    Helper,
    SshAgent,
    Default,
}

/// One-shot attempt flags. A fresh `CredAttempts` per remote operation.
#[derive(Debug, Default)]
pub(crate) struct CredAttempts {
    helper: bool,
    agent: bool,
    default_: bool,
}

/// Returns the next untried method compatible with `allowed`, marking it
/// tried. Order: Helper (USER_PASS_PLAINTEXT) -> SshAgent (SSH_KEY) ->
/// Default (DEFAULT). `None` => every compatible method has been tried (or
/// none is compatible). Each method is attempted AT MOST ONCE per operation —
/// libgit2 re-invokes the credentials callback after every auth failure, and
/// this guard is what stops the retry loop.
pub(crate) fn next_cred_method(
    attempts: &mut CredAttempts,
    allowed: git2::CredentialType,
) -> Option<CredMethod> {
    if allowed.contains(git2::CredentialType::USER_PASS_PLAINTEXT) && !attempts.helper {
        attempts.helper = true;
        return Some(CredMethod::Helper);
    }
    if allowed.contains(git2::CredentialType::SSH_KEY) && !attempts.agent {
        attempts.agent = true;
        return Some(CredMethod::SshAgent);
    }
    if allowed.contains(git2::CredentialType::DEFAULT) && !attempts.default_ {
        attempts.default_ = true;
        return Some(CredMethod::Default);
    }
    None
}

/// Credentials callback body (contract §2.2). Construction failure consumes
/// the attempt and falls through to the next method; exhaustion returns an
/// error, which aborts the transport instead of looping. NEVER prompts;
/// NEVER reads or stores passwords.
fn acquire_cred(
    config: &git2::Config,
    attempts: &RefCell<CredAttempts>,
    url: &str,
    username_from_url: Option<&str>,
    allowed: git2::CredentialType,
) -> Result<git2::Cred, git2::Error> {
    loop {
        let method = next_cred_method(&mut attempts.borrow_mut(), allowed);
        match method {
            Some(CredMethod::Helper) => {
                if let Ok(cred) = git2::Cred::credential_helper(config, url, username_from_url) {
                    return Ok(cred);
                }
            }
            Some(CredMethod::SshAgent) => {
                if let Ok(cred) =
                    git2::Cred::ssh_key_from_agent(username_from_url.unwrap_or("git"))
                {
                    return Ok(cred);
                }
            }
            Some(CredMethod::Default) => {
                if let Ok(cred) = git2::Cred::default() {
                    return Ok(cred);
                }
            }
            None => {
                return Err(git2::Error::new(
                    git2::ErrorCode::Auth,
                    git2::ErrorClass::Callback,
                    CRED_EXHAUSTED_MSG,
                ));
            }
        }
    }
}

/// Maps a git2 error from a remote operation to an AppError. `context` is the
/// remote name or URL for message interpolation (contract §2.3 table,
/// evaluated top-down, first match wins).
pub(crate) fn map_remote_err(e: git2::Error, context: &str) -> AppError {
    let auth_msg = || {
        format!(
            "authentication failed for '{context}': no usable credentials. \
             Configure a Git credential helper (e.g. Git Credential Manager) for \
             HTTPS remotes, or run an SSH agent for SSH remotes."
        )
    };
    if e.class() == git2::ErrorClass::Callback && e.message().contains(CRED_EXHAUSTED_MSG) {
        return AppError::AuthFailed(auth_msg());
    }
    if e.code() == git2::ErrorCode::Auth {
        return AppError::AuthFailed(auth_msg());
    }
    if e.code() == git2::ErrorCode::NotFastForward {
        return AppError::PushRejected(
            "push rejected: the remote contains commits you do not have. \
             Fetch/pull first — Bonsai v1 never force-pushes."
                .to_string(),
        );
    }
    match e.class() {
        git2::ErrorClass::Net | git2::ErrorClass::Http | git2::ErrorClass::Ssh => {
            AppError::NetworkError(format!(
                "network error talking to '{context}': {}",
                e.message()
            ))
        }
        _ => AppError::Git(e.message().to_string()),
    }
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

    let config = repo.config()?;
    let attempts = RefCell::new(CredAttempts::default());
    let updated = Cell::new(0u32);

    let received = {
        let mut callbacks = git2::RemoteCallbacks::new();
        callbacks.credentials(|url, username_from_url, allowed| {
            acquire_cred(&config, &attempts, url, username_from_url, allowed)
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

        remote
            .fetch(&[] as &[&str], Some(&mut opts), None)
            .map_err(|e| map_remote_err(e, name))?;
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
    Ok(FetchResult { remotes })
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
pub fn push_current(workdir: &Path) -> Result<PushResult, AppError> {
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
            acquire_cred(&config, &attempts, url, username_from_url, allowed)
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
        remote
            .push(&[refspec.as_str()], Some(&mut opts))
            .map_err(|e| map_remote_err(e, &remote_name))?;
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
        })
        .expect("json");
        assert_eq!(
            v,
            serde_json::json!({ "kind": "wouldNotFastForward", "branch": "main", "ahead": 2, "behind": 1 })
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
        })
        .expect("json");
        assert_eq!(
            v,
            serde_json::json!({ "remotes": [{ "remote": "origin", "receivedObjects": 12, "updatedRefs": 1 }] })
        );
    }

    // ------------------------------------------------ §6.5 credential guard

    /// All three sources allowed → Helper, SshAgent, Default, then None
    /// forever (idempotent exhaustion).
    #[test]
    fn cred_guard_full_sequence() {
        let allowed = git2::CredentialType::USER_PASS_PLAINTEXT
            | git2::CredentialType::SSH_KEY
            | git2::CredentialType::DEFAULT;
        let mut attempts = CredAttempts::default();
        assert_eq!(next_cred_method(&mut attempts, allowed), Some(CredMethod::Helper));
        assert_eq!(next_cred_method(&mut attempts, allowed), Some(CredMethod::SshAgent));
        assert_eq!(next_cred_method(&mut attempts, allowed), Some(CredMethod::Default));
        assert_eq!(next_cred_method(&mut attempts, allowed), None);
        assert_eq!(next_cred_method(&mut attempts, allowed), None);
    }

    /// SSH_KEY only → SshAgent once, then None.
    #[test]
    fn cred_guard_ssh_only() {
        let allowed = git2::CredentialType::SSH_KEY;
        let mut attempts = CredAttempts::default();
        assert_eq!(next_cred_method(&mut attempts, allowed), Some(CredMethod::SshAgent));
        assert_eq!(next_cred_method(&mut attempts, allowed), None);
    }

    /// Nothing allowed → None immediately.
    #[test]
    fn cred_guard_empty_allowed() {
        let allowed = git2::CredentialType::empty();
        let mut attempts = CredAttempts::default();
        assert_eq!(next_cred_method(&mut attempts, allowed), None);
    }

    /// Each method at most once, even across repeat calls with a single
    /// allowed type.
    #[test]
    fn cred_guard_single_method_once() {
        let allowed = git2::CredentialType::USER_PASS_PLAINTEXT;
        let mut attempts = CredAttempts::default();
        assert_eq!(next_cred_method(&mut attempts, allowed), Some(CredMethod::Helper));
        assert_eq!(next_cred_method(&mut attempts, allowed), None);
        assert_eq!(next_cred_method(&mut attempts, allowed), None);
    }

    // -------------------------------------------------- §6.6 map_remote_err

    fn g2err(code: git2::ErrorCode, class: git2::ErrorClass, msg: &str) -> git2::Error {
        git2::Error::new(code, class, msg)
    }

    /// Callback class + exhaustion sentinel → authFailed (row 1; matches by
    /// class+message regardless of code).
    #[test]
    fn map_cred_exhausted_to_auth_failed() {
        for code in [git2::ErrorCode::Auth, git2::ErrorCode::GenericError] {
            let e = g2err(code, git2::ErrorClass::Callback, CRED_EXHAUSTED_MSG);
            let mapped = map_remote_err(e, "origin");
            match mapped {
                AppError::AuthFailed(m) => {
                    assert!(m.contains("'origin'"), "context missing: {m}");
                    assert!(m.contains("credential helper"), "guidance missing: {m}");
                }
                other => panic!("expected AuthFailed, got {other:?}"),
            }
        }
    }

    /// Auth code under any class (Http, Ssh, Net) → authFailed (row 2).
    #[test]
    fn map_auth_code_to_auth_failed() {
        for class in [
            git2::ErrorClass::Http,
            git2::ErrorClass::Ssh,
            git2::ErrorClass::Net,
        ] {
            let e = g2err(git2::ErrorCode::Auth, class, "401");
            match map_remote_err(e, "origin") {
                AppError::AuthFailed(m) => assert!(m.contains("'origin'")),
                other => panic!("expected AuthFailed for class {class:?}, got {other:?}"),
            }
        }
    }

    /// NotFastForward (push negotiation) → pushRejected (row 3).
    #[test]
    fn map_not_fast_forward_to_push_rejected() {
        let e = g2err(
            git2::ErrorCode::NotFastForward,
            git2::ErrorClass::Reference,
            "cannot push non-fastforwardable reference",
        );
        match map_remote_err(e, "origin") {
            AppError::PushRejected(m) => assert!(m.contains("never force-pushes"), "{m}"),
            other => panic!("expected PushRejected, got {other:?}"),
        }
    }

    /// Non-auth Net / Http / Ssh class → networkError with context +
    /// underlying message (rows 4-6).
    #[test]
    fn map_transport_classes_to_network_error() {
        for class in [
            git2::ErrorClass::Net,
            git2::ErrorClass::Http,
            git2::ErrorClass::Ssh,
        ] {
            let e = g2err(git2::ErrorCode::GenericError, class, "failed to resolve address");
            match map_remote_err(e, "origin") {
                AppError::NetworkError(m) => {
                    assert!(m.contains("'origin'"), "context missing: {m}");
                    assert!(m.contains("failed to resolve address"), "cause missing: {m}");
                }
                other => panic!("expected NetworkError for class {class:?}, got {other:?}"),
            }
        }
    }

    /// Anything else → plain git error with the original message (row 7).
    #[test]
    fn map_other_to_git() {
        let e = g2err(
            git2::ErrorCode::GenericError,
            git2::ErrorClass::None,
            "something odd",
        );
        match map_remote_err(e, "origin") {
            AppError::Git(m) => assert_eq!(m, "something odd"),
            other => panic!("expected Git, got {other:?}"),
        }
    }
}
