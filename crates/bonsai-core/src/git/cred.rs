//! Credential acquisition for remote operations: the libgit2 credentials
//! callback ladder, its one-shot attempt bookkeeping, and the git2 -> AppError
//! mapping every remote op must route through (contract §2.2/§2.3, P35 §9,
//! P70 §3.1).
//!
//! Moved verbatim out of `remote.rs` (which had grown to ~4× the soft file
//! limit) so the ladder reads on its own. The surface is unchanged: `remote.rs`
//! re-exports the items other modules import, so `crate::git::remote::…` paths
//! still resolve.
//!
//! Credentials (USER-CONFIRMED strategy, locked): Git's configured credential
//! helper → SSH agent → default. NEVER prompt for or store passwords.

use std::cell::RefCell;
use std::io::Write;
use std::path::Path;
use std::process::Stdio;

use crate::error::AppError;
use crate::git::cred_cache::{self, CredResolve, Resolved};
use crate::gitbin;

/// Sentinel in the git2 error raised when every credential source is
/// exhausted — `map_remote_err` keys the `authFailed` mapping off it.
pub(crate) const CRED_EXHAUSTED_MSG: &str = "bonsai: no usable credentials";

/// Sentinel threaded through git2's callback error when the `git` executable
/// itself cannot be launched (P70 §3.1) — exactly the [`CRED_EXHAUSTED_MSG`]
/// mechanism, but `map_remote_err` maps it to `GitNotFound` rather than
/// `AuthFailed`, so a launch failure is NEVER reported as an auth problem.
pub(crate) const GIT_MISSING_MSG: &str = "bonsai: git executable not found";

/// Outcome of one `git credential fill` attempt (P70 §3.1). Distinguishes "git
/// could not be launched" from "the helper had nothing" — the pre-P70 `None`
/// conflated them, which is the root of the misleading auth toast.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum FillOutcome {
    Filled { username: String, password: String },
    /// git ran and exited, but produced no usable username+password (cache
    /// miss, non-zero exit, unparseable output). The pre-P70 `None` meaning.
    NoCredentials,
    /// The `git` child could NOT be launched at all. Carries the io error text
    /// for the log; the user-facing string comes from
    /// [`gitbin::git_not_found_message`].
    GitUnavailable(String),
}

/// Which credential source to try next. Pure decision logic — no git2 Cred
/// construction (unit-testable offline, contract §2.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CredMethod {
    Helper,
    SshAgent,
    Default,
}

/// Helper-arm state across libgit2 callback re-invocations within ONE
/// operation (P35 §9). `acquire_cred` owns all transitions; `next_cred_method`
/// only READS it for eligibility.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum HelperState {
    #[default]
    Untried,
    /// First Helper attempt returned a CACHED entry; ONE cache-bypassing re-fill
    /// is still permitted (the invalidation-on-rejection path).
    RetryAllowed,
    /// Helper exhausted: a fresh fill was attempted (miss or bypass), a cache-hit
    /// cred failed to construct, or resolve returned None.
    Done,
}

/// One-shot attempt flags. A fresh `CredAttempts` per remote operation.
#[derive(Debug, Default)]
pub(crate) struct CredAttempts {
    helper: HelperState,
    agent: bool,
    default_: bool,
    /// The url whose FRESH-fill credential we handed to libgit2 this operation
    /// (a cache miss or a post-rejection bypass re-fill — NOT a first cache
    /// hit). On an operation-level auth failure the caller evicts this key so
    /// the next op re-fills instead of re-serving the known-bad cred (F-A5-b).
    fresh_fill_url: Option<String>,
    /// P70: set when the Helper rung failed because `git` itself could not be
    /// launched (unresolvable, or a spawn error at fill time) rather than
    /// because the helper had no credentials. Read ONLY once the ladder is
    /// exhausted, to pick the honest verdict — it never short-circuits the
    /// remaining rungs, so SSH-agent auth still works with git absent.
    helper_git_unavailable: Option<String>,
}

/// Returns the next untried method compatible with `allowed`, marking
/// `agent`/`default_` tried. Order: Helper (USER_PASS_PLAINTEXT) -> SshAgent
/// (SSH_KEY) -> Default (DEFAULT). `None` => every compatible method has been
/// tried (or none is compatible). SshAgent/Default are each attempted AT MOST
/// ONCE per operation. Helper eligibility is driven by `HelperState`; this fn
/// does NOT mutate `helper` — `acquire_cred` sets it (RetryAllowed on a first
/// cache-hit, else Done) after every Helper attempt, which is what stops the
/// retry loop.
pub(crate) fn next_cred_method(
    attempts: &mut CredAttempts,
    allowed: git2::CredentialType,
) -> Option<CredMethod> {
    if allowed.contains(git2::CredentialType::USER_PASS_PLAINTEXT)
        && attempts.helper != HelperState::Done
    {
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

/// Resolves HTTPS credentials via the user's REAL configured credential
/// helper by shelling out to `git credential fill` — NOT libgit2's own
/// reimplementation (see addendum preamble). `repo_path`: cwd for the child
/// process when `Some`, so repo-local `credential.helper` config resolves
/// exactly like the `git` CLI does (it also reads cwd's repo config); `None`
/// when no repo exists yet (clone, §A.3) — global/system config still
/// resolves without a cwd, matching what `git clone` itself does before a
/// repo exists.
///
/// NEVER prompts: `GIT_TERMINAL_PROMPT=0` gates the terminal prompt and
/// `-c core.askpass=` + `env_remove` of GIT_ASKPASS/SSH_ASKPASS neutralize the
/// askpass GUI path, so a cache miss fails fast instead of blocking on an
/// interactive prompt — this preserves the locked never-prompt policy, §2.2). NEVER
/// panics.
///
/// P70: the return distinguishes "the helper ran and had nothing"
/// ([`FillOutcome::NoCredentials`] — non-zero exit, I/O error writing stdin,
/// non-UTF-8 stdout, missing/empty fields) from "`git` could not be LAUNCHED at
/// all" ([`FillOutcome::GitUnavailable`]). Collapsing the latter into the
/// former is what produced the misleading "no cached credentials" toast when
/// the app inherited a PATH without git.
pub(crate) fn credential_fill(repo_path: Option<&Path>, url: &str) -> FillOutcome {
    let mut cmd = gitbin::git_command();
    // Never block on an interactive prompt. GIT_TERMINAL_PROMPT=0 only gates the
    // *terminal* prompt; git also has an askpass path (a GUI dialog on Git for
    // Windows — "Username for '<url>'") reached via the GIT_ASKPASS / SSH_ASKPASS
    // env vars or `core.askpass` config. Neutralize all of them so a cache miss
    // fails fast instead of popping a window: `-c core.askpass=` overrides any
    // configured askpass, and env_remove clears the env-level ones (Git for
    // Windows sets GIT_ASKPASS). env_remove affects ONLY this child, so parallel
    // tests stay hermetic. With no askpass and no terminal prompt, git returns an
    // error and we fall through to `NoCredentials` — the locked never-prompt
    // policy (§2.2). ("git ran and had nothing", NOT "git could not run".)
    cmd.args(["-c", "core.askpass=", "credential", "fill"])
        .env("GIT_TERMINAL_PROMPT", "0") // REQUIRED — never block on a prompt
        .env_remove("GIT_ASKPASS")
        .env_remove("SSH_ASKPASS")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null()); // discarded — never logged, never in an error path
    if let Some(p) = repo_path {
        cmd.current_dir(p);
    }
    // (The Windows console-window suppression that used to live here now comes
    // from `gitbin::git_command()`, which applies it to EVERY git spawn.)

    // ANY spawn io error — NotFound, PermissionDenied, … — means "not
    // launchable", which is emphatically NOT "the helper had no credentials".
    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => return FillOutcome::GitUnavailable(e.to_string()),
    };
    let write_ok = child
        .stdin
        .as_mut()
        .is_some_and(|stdin| stdin.write_all(format!("url={url}\n\n").as_bytes()).is_ok());
    if !write_ok {
        let _ = child.wait(); // reap the child instead of leaving it a zombie
        return FillOutcome::NoCredentials;
    }

    // wait_with_output closes stdin (EOF) before waiting, so the child sees
    // the full request even though we haven't dropped our handle explicitly.
    let Ok(output) = child.wait_with_output() else {
        return FillOutcome::NoCredentials;
    };
    if !output.status.success() {
        return FillOutcome::NoCredentials;
    }
    let Ok(stdout) = String::from_utf8(output.stdout) else {
        return FillOutcome::NoCredentials;
    };

    let (mut username, mut password) = (None, None);
    for line in stdout.lines() {
        let Some((key, value)) = line.split_once('=') else { continue };
        match key {
            "username" => username = Some(value.to_string()),
            "password" => password = Some(value.to_string()),
            _ => {} // ignore unknown keys (protocol/host/path/url echo, etc.)
        }
    }
    match (username, password) {
        (Some(u), Some(p)) if !u.is_empty() && !p.is_empty() => FillOutcome::Filled {
            username: u,
            password: p,
        },
        _ => FillOutcome::NoCredentials,
    }
}

/// Credentials callback body (contract §2.2). Construction failure consumes
/// the attempt and falls through to the next method; exhaustion returns an
/// error, which aborts the transport instead of looping. NEVER prompts;
/// NEVER reads or stores passwords.
pub(crate) fn acquire_cred(
    repo_path: Option<&Path>,
    attempts: &RefCell<CredAttempts>,
    url: &str,
    username_from_url: Option<&str>,
    allowed: git2::CredentialType,
) -> Result<git2::Cred, git2::Error> {
    acquire_cred_with(
        repo_path,
        attempts,
        url,
        username_from_url,
        allowed,
        gitbin::git_missing(),
    )
}

/// [`acquire_cred`] with the "is there a runnable git?" answer INJECTED, so the
/// git-missing behaviour is unit-testable without touching `std::env` or the
/// process-global resolver cache (P70).
///
/// Note what this deliberately does NOT do: it does not short-circuit the
/// ladder when git is missing. An SSH remote with a running ssh-agent
/// authenticates entirely inside libgit2 and never needs `git.exe` — failing
/// it early would break a user whose setup works fine today. Only the Helper
/// rung needs git; when git is unresolvable that rung fails IMMEDIATELY (no
/// spawn) and records WHY, and the ladder carries on to SshAgent/Default. The
/// recorded reason only matters once the ladder is exhausted: then the verdict
/// is the honest `GitNotFound` instead of a misleading auth failure.
pub(crate) fn acquire_cred_with(
    repo_path: Option<&Path>,
    attempts: &RefCell<CredAttempts>,
    url: &str,
    username_from_url: Option<&str>,
    allowed: git2::CredentialType,
    git_missing: bool,
) -> Result<git2::Cred, git2::Error> {
    loop {
        let method = next_cred_method(&mut attempts.borrow_mut(), allowed);
        match method {
            Some(CredMethod::Helper) => {
                // A second Helper attempt is permitted ONLY after a first
                // cache HIT (RetryAllowed); it evicts + forces a fresh re-fill
                // (invalidation on rejection, §9).
                if git_missing {
                    // No runnable git => `git credential fill` cannot possibly
                    // work. Fail this rung with NO spawn attempt, remember why,
                    // and let the ladder continue (SSH agent may well succeed).
                    //
                    // Still honour the invalidation half of a RetryAllowed
                    // re-entry: the remote just REJECTED the cached credential,
                    // so evict it even though we cannot re-fill. Otherwise the
                    // known-bad entry survives to its TTL and would be served
                    // again the moment git becomes resolvable (Re-check).
                    if attempts.borrow().helper == HelperState::RetryAllowed {
                        cred_cache::evict(repo_path, url);
                    }
                    let mut a = attempts.borrow_mut();
                    a.helper = HelperState::Done;
                    a.helper_git_unavailable = Some("no runnable git executable was resolved".to_string());
                    continue;
                }
                let bypass = attempts.borrow().helper == HelperState::RetryAllowed;
                if bypass {
                    cred_cache::evict(repo_path, url);
                }
                match cred_cache::resolve(repo_path, url, bypass) {
                    CredResolve::Resolved(Resolved {
                        creds: (user, pass),
                        from_cache,
                    }) => {
                        if let Ok(cred) = git2::Cred::userpass_plaintext(&user, &pass) {
                            // Only a FIRST-attempt cache hit earns a retry; a
                            // fresh fill (miss or bypass) is terminal.
                            let fresh = !(from_cache && !bypass);
                            let mut a = attempts.borrow_mut();
                            a.helper = if fresh {
                                HelperState::Done
                            } else {
                                HelperState::RetryAllowed
                            };
                            // Remember a FRESH fill so op-level auth failure can
                            // evict it (F-A5-b); a plain cache hit stays evictable
                            // only through the existing RetryAllowed bypass path.
                            if fresh {
                                a.fresh_fill_url = Some(url.to_string());
                            }
                            return Ok(cred);
                        }
                        // userpass_plaintext failing is theoretical (string-only
                        // validation) — treat as a construction failure: mark
                        // Helper Done so the loop moves on to SshAgent.
                        attempts.borrow_mut().helper = HelperState::Done;
                    }
                    // No cached creds / fill failed (§A.1) -> fall through.
                    CredResolve::NoCredentials => {
                        attempts.borrow_mut().helper = HelperState::Done
                    }
                    // P70: git could NOT be launched (e.g. the resolver's
                    // cached path went stale mid-session). This rung is done,
                    // and we remember that it failed for a launch reason — but
                    // the ladder still continues, because SshAgent/Default may
                    // succeed without git existing at all.
                    CredResolve::GitUnavailable(detail) => {
                        let mut a = attempts.borrow_mut();
                        a.helper = HelperState::Done;
                        a.helper_git_unavailable = Some(detail);
                    }
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
                return Err(exhausted_error(
                    attempts.borrow().helper_git_unavailable.as_deref(),
                ));
            }
        }
    }
}

/// The error raised when EVERY credential rung has been tried and none
/// produced a usable credential. PURE (no git2 state, no spawn) so both
/// verdicts are unit-testable.
///
/// `helper_git_unavailable` = `Some(detail)` iff the Helper rung failed because `git`
/// itself could not be launched. Only then is the honest [`GIT_MISSING_MSG`]
/// sentinel used (→ `AppError::GitNotFound`); a helper that RAN and simply had
/// nothing keeps the pre-P70 [`CRED_EXHAUSTED_MSG`] → `AppError::AuthFailed`
/// path untouched. The io/path detail rides along on the (internal) git2
/// message — `map_remote_err` replaces the user-facing text with the honest
/// copy, so the raw detail is never shown to the user.
fn exhausted_error(helper_git_unavailable: Option<&str>) -> git2::Error {
    let message = match helper_git_unavailable {
        Some(detail) => format!("{GIT_MISSING_MSG}: {detail}"),
        None => CRED_EXHAUSTED_MSG.to_string(),
    };
    git2::Error::new(
        git2::ErrorCode::Auth,
        git2::ErrorClass::Callback,
        &message,
    )
}

/// Maps a git2 error from a remote operation to an AppError. `context` is the
/// remote name or URL for message interpolation (contract §2.3 table,
/// evaluated top-down, first match wins).
///
/// **Every remote operation MUST route its `git2::Error` through here.** The
/// blanket `impl From<git2::Error> for AppError` (`error.rs`) maps to
/// `AppError::Git` with the RAW libgit2 text, which for a `GIT_MISSING_MSG`
/// callback error would leak the internal sentinel plus the raw io detail into
/// the UI *and* lose the `gitNotFound` kind the banner keys off. A `?` on a
/// git2 call inside a remote op is therefore a bug, not a shortcut.
pub(crate) fn map_remote_err(e: git2::Error, context: &str) -> AppError {
    let auth_msg = || {
        let helper_configured = git2::Config::open_default()
            .ok()
            .and_then(|c| c.get_string("credential.helper").ok())
            .map(|s| !s.trim().is_empty())
            .unwrap_or(false); // covers Err(NotFound) (unset) and any other lookup failure

        if helper_configured {
            format!(
                "authentication failed for '{context}': the configured credential helper has no \
                 cached credentials for this remote. Run the equivalent git command in a terminal \
                 once to (re-)authenticate, or run an SSH agent for SSH remotes."
            )
        } else {
            format!(
                "authentication failed for '{context}': no usable credentials and no Git credential \
                 helper is configured. Configure one (e.g. Git Credential Manager) for HTTPS \
                 remotes, or run an SSH agent for SSH remotes."
            )
        }
    };
    // P70: FIRST — a launch failure is never an auth failure. Checked before
    // the CRED_EXHAUSTED_MSG / ErrorCode::Auth arms below, which are otherwise
    // untouched (genuine auth failures keep their existing copy).
    if e.message().contains(GIT_MISSING_MSG) {
        return AppError::GitNotFound(gitbin::git_not_found_message());
    }
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

/// F-A5-b: on an operation-level auth failure, evict the credential this op
/// FRESH-filled (cache miss / post-rejection bypass) so the NEXT op re-fills
/// through the helper instead of re-serving the just-rejected cred for the full
/// TTL. Returns `err` unchanged (identity on the non-auth / no-fresh-fill path).
///
/// We deliberately do NOT invoke `git credential reject` here: a proper reject
/// must feed the helper the exact `username`/`password` it stored, but Bonsai's
/// `cred_cache` keeps the plaintext deliberately walled off from `remote.rs`
/// (it never surfaces the secret back out). Evicting our in-process entry is the
/// safe, sufficient fix — the next op's fresh fill re-consults the helper, which
/// will re-prompt/re-issue as its own policy dictates. (Behavior change:
/// FOR USER REVIEW.)
pub(crate) fn evict_fresh_on_auth_fail(
    repo: &git2::Repository,
    attempts: &RefCell<CredAttempts>,
    err: AppError,
) -> AppError {
    if matches!(err, AppError::AuthFailed(_)) {
        let fresh = attempts.borrow().fresh_fill_url.clone();
        if let Some(url) = fresh {
            cred_cache::evict(repo.workdir(), &url);
        }
    }
    err
}

#[cfg(test)]
#[path = "cred_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "cred_gitbin_tests.rs"]
mod gitbin_tests;
