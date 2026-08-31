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

use std::path::Path;

use crate::error::AppError;
// The credential ladder lives in `cred.rs`; re-exported here so the
// `crate::git::remote::…` paths its callers already use keep resolving.
pub(crate) use crate::git::cred::{
    acquire_cred, credential_fill, map_remote_err, CredAttempts, FillOutcome,
};
use crate::git::exec::GitExec;
// P87: the activity-recording variants live in `remote_activity` (fetch/pull) +
// `remote_push_activity` (push/force) — file-size splits; re-exported so
// `crate::git::remote::*_with_activity` keeps resolving for the command layer.
// The plain fns below are thin `None` wrappers over them.
pub use crate::git::remote_activity::{fetch_all_with_activity, pull_ff_with_activity};
pub use crate::git::remote_push_activity::{
    force_push_with_lease_with_activity, push_current_with_activity,
};

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
pub(crate) fn open_repo_at(workdir: &Path) -> Result<git2::Repository, AppError> {
    Ok(git2::Repository::open_ext(
        workdir,
        git2::RepositoryOpenFlags::NO_SEARCH,
        std::iter::empty::<&std::ffi::OsStr>(),
    )?)
}

/// Saturating usize→u32 (counts on the wire; overflow is theoretical).
pub(crate) fn to_u32(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

/// Blocking. Fetches every configured remote, sequentially, in
/// `repo.remotes()` order; fail-fast on the first failing remote (named in
/// the error — contract §9). Works fine with detached or unborn HEAD.
pub fn fetch_all(workdir: &Path) -> Result<FetchResult, AppError> {
    fetch_all_with_activity(workdir, None)
}

/// Blocking. Fetches the current branch's upstream remote, then fast-forwards
/// ONLY (contract §2.5). Not fast-forwardable → `WouldNotFastForward`,
/// changing nothing. SAFE checkout only — never force; `checkout_tree` runs
/// BEFORE `set_target`, so a conflict leaves ref AND worktree untouched.
pub fn pull_ff(workdir: &Path) -> Result<PullResult, AppError> {
    pull_ff_with_activity(workdir, None)
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
    push_current_with_activity(workdir, runner, skip_hooks, None)
}

/// Lease failure: the remote advanced (or its ref was deleted) since the last
/// fetch (P37 §3.2).
pub(crate) fn lease_moved_msg(remote: &str, branch: &str) -> String {
    format!(
        "force-push refused: '{remote}/{branch}' has moved on the remote since you last \
         fetched — someone may have pushed. Fetch and review before force-pushing again."
    )
}

/// Lease failure: no remote-tracking ref to lease against (P37 §3.2).
pub(crate) fn lease_no_baseline_msg(remote: &str, branch: &str) -> String {
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
    force_push_with_lease_with_activity(workdir, runner, skip_hooks, None)
}

/// The fetch URL of remote `name`, or an empty string when it is unset /
/// unreadable / non-UTF-8. Used as the `pre-push` hook's 2nd arg (git passes
/// the remote URL there); an empty string is a harmless placeholder git itself
/// also tolerates when a remote has no URL.
pub(crate) fn remote_url_of(repo: &git2::Repository, name: &str) -> String {
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
pub(crate) fn build_pre_push_stdin(
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
pub(crate) fn build_force_push_args(
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
pub(crate) fn classify_push_stderr(stderr: &str) -> AppError {
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
mod tests;
