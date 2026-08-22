//! P87 activity-recording push/force-push cores (split from `remote.rs` for the
//! ~500-line limit; sibling of `remote_activity.rs`, which holds fetch/pull).
//!
//! The `*_with_activity` variants are [`super::remote`]'s `push_current` /
//! `force_push_with_lease` PLUS an optional [`GitActivityRecorder`]: the pre-push
//! hook runs in a `RunningHook` phase, then a `Network` phase (the "feels hung"
//! fix, §5), with force-push's CLI output streamed as lines (§14.5). `activity ==
//! None` is the **byte-for-byte** pre-P87 path; credential/push behaviour is
//! unchanged. The plain names in `remote.rs` are thin `None` wrappers.

use std::cell::RefCell;
use std::path::Path;

use crate::error::AppError;
use crate::git::activity::{GitActivityRecorder, GitPhaseKind, GitStream};
use crate::git::cred::{acquire_cred, evict_fresh_on_auth_fail, map_remote_err, CredAttempts};
use crate::git::exec::{GitExec, LineSink};
use crate::git::hooks::{hooks_enabled, run_hook_streaming, HookName};
use crate::git::remote::{
    build_force_push_args, build_pre_push_stdin, classify_push_stderr, lease_moved_msg,
    lease_no_baseline_msg, open_repo_at, remote_url_of, PushResult,
};
use crate::git::repo::read_head_info;

/// Adapter driving a [`GitActivityRecorder`] from the exec seam's [`LineSink`]
/// (force-push CLI output → activity lines). `line` runs only on the caller
/// thread, so no `Sync` bound is needed.
struct RecorderSink<'a>(&'a dyn GitActivityRecorder);

impl LineSink for RecorderSink<'_> {
    fn line(&self, stream: GitStream, line: &str) {
        self.0.line(stream, line);
    }
}

/// P87: [`super::remote::push_current`] plus an optional recorder (category
/// `Push`). Runs the pre-push hook in a `RunningHook` phase, then transitions to
/// `Network` immediately before the libgit2 push — the "feels hung" fix (§5).
pub fn push_current_with_activity(
    workdir: &Path,
    runner: &dyn GitExec,
    skip_hooks: bool,
    activity: Option<&dyn GitActivityRecorder>,
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
        run_hook_streaming(
            runner,
            workdir,
            HookName::PrePush,
            &[remote_name.clone(), remote_url],
            Some(stdin.as_bytes()),
            activity,
        )?;
    }

    // THE C FIX: the hook is done — the actual network push starts now, so the
    // UI stops showing "Running pre-push hook…" and shows "Pushing…".
    if let Some(a) = activity {
        a.phase(GitPhaseKind::Network, None);
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

/// P87: [`super::remote::force_push_with_lease`] plus an optional recorder
/// (category `ForcePush`). Runs the pre-push hook in a `RunningHook` phase, then
/// transitions to `Network` and streams the git-binary push's output as lines
/// (force-push is CLI-only — §14.5).
pub fn force_push_with_lease_with_activity(
    workdir: &Path,
    runner: &dyn GitExec,
    skip_hooks: bool,
    activity: Option<&dyn GitActivityRecorder>,
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
        run_hook_streaming(
            runner,
            workdir,
            HookName::PrePush,
            &[remote_name.clone(), remote_url],
            Some(stdin.as_bytes()),
            activity,
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
    // The hook is done — the network push starts now (§14.5: force-push is CLI, so
    // its network output streams as exec lines within the Network phase).
    let out = match activity {
        Some(a) => {
            a.phase(GitPhaseKind::Network, None);
            runner.exec_streaming(&arg_refs, workdir, None, &[], &RecorderSink(a))?
        }
        None => runner.exec(&arg_refs, workdir, None, &[])?,
    };
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
