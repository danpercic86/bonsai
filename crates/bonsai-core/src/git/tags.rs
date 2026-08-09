//! Tag operations core (P22 contract §2).
//!
//! create (lightweight AND annotated), delete (local only), and push a single
//! tag to a remote. Pure git2 logic, no Tauri types — testable against the git
//! CLI oracle over local bare-repo remotes (see `tests/tags_cli.rs`). All
//! functions blocking; the command layer wraps them in `spawn_blocking`.
//!
//! `push_tag` reuses the M6 credential chain (`acquire_cred` / `CredAttempts` /
//! `map_remote_err`, already `pub(crate)` in `remote.rs`): Git's configured
//! credential helper → SSH agent → default. NEVER prompts, NEVER stores
//! passwords, NEVER force-pushes unless explicitly asked (§OPEN-4).

use std::cell::RefCell;
use std::path::Path;

use crate::error::AppError;
use crate::git::commit::resolve_signature;
use crate::git::remote::{acquire_cred, map_remote_err, CredAttempts};

/// Opens the repo at `workdir` with `NO_SEARCH` (same as every git/ module).
fn open_repo_at(workdir: &Path) -> Result<git2::Repository, AppError> {
    Ok(git2::Repository::open_ext(
        workdir,
        git2::RepositoryOpenFlags::NO_SEARCH,
        std::iter::empty::<&std::ffi::OsStr>(),
    )?)
}

/// Rejects blank / leading-`-` names, then defers to libgit2's ref-name
/// validation for `refs/tags/<name>` (mirrors `branches::validate_branch_name`).
fn validate_tag_name(name: &str) -> Result<(), AppError> {
    let invalid = || AppError::InvalidName(format!("invalid tag name: '{name}'"));
    if name.trim().is_empty() || name.starts_with('-') {
        return Err(invalid());
    }
    if !git2::Reference::is_valid_name(&format!("refs/tags/{name}")) {
        return Err(invalid());
    }
    Ok(())
}

/// Blocking. Create a tag `name` pointing at commit `target_oid` (P22 §2.2).
/// - `message: Some(_)` → ANNOTATED tag (`Repository::tag`, needs a tagger from
///   `resolve_signature` — `ConfigMissing` if user.name/email unset, exactly
///   like commit).
/// - `message: None`    → LIGHTWEIGHT tag (`Repository::tag_lightweight`).
///
/// `force` overwrites an existing tag of the same name; the v1 UI always passes
/// `false` (§OPEN-4).
///
/// Errors: invalid/blank name → `InvalidName`; bad/unknown `target_oid` →
/// `Git`; duplicate (Exists, !force) → `Git("tag '<name>' already exists")`;
/// missing identity (annotated) → `ConfigMissing`.
pub fn create_tag(
    workdir: &Path,
    name: &str,
    target_oid: &str,
    message: Option<String>,
    force: bool,
) -> Result<(), AppError> {
    validate_tag_name(name)?;

    let repo = open_repo_at(workdir)?;
    let oid = git2::Oid::from_str(target_oid).map_err(|_| {
        AppError::Git(format!(
            "cannot create tag: '{target_oid}' is not a valid commit id"
        ))
    })?;
    let target = repo.find_object(oid, None).map_err(|_| {
        AppError::Git(format!("cannot create tag: commit '{target_oid}' not found"))
    })?;

    let result = match &message {
        Some(msg) => {
            let sig = resolve_signature(&repo.config()?.snapshot()?)?;
            repo.tag(name, &target, &sig, msg, force)
        }
        None => repo.tag_lightweight(name, &target, force),
    };
    match result {
        Ok(_oid) => Ok(()),
        Err(e) if e.code() == git2::ErrorCode::Exists => {
            Err(AppError::Git(format!("tag '{name}' already exists")))
        }
        Err(e) => Err(e.into()),
    }
}

/// Blocking. Delete LOCAL tag `name` (`Repository::tag_delete`). Local-only —
/// does NOT contact any remote (§OPEN-3). Errors: not-found →
/// `Git("tag '<name>' not found")`; blank/invalid name (same validation as
/// create — leading `-`, bad ref chars) → `InvalidName`.
pub fn delete_tag(workdir: &Path, name: &str) -> Result<(), AppError> {
    validate_tag_name(name)?;
    let repo = open_repo_at(workdir)?;
    match repo.tag_delete(name) {
        Ok(()) => Ok(()),
        Err(e) if e.code() == git2::ErrorCode::NotFound => {
            Err(AppError::Git(format!("tag '{name}' not found")))
        }
        Err(e) => Err(e.into()),
    }
}

/// Blocking. Push `refs/tags/<tag_name>` to `remote_name` over the M6
/// credential path (P22 §2.4). Refspec `refs/tags/<n>:refs/tags/<n>` — NO
/// leading `+` unless `force` (the v1 UI always passes `false`).
///
/// Errors: remote not found → `NoRemote`; tag not found locally → `Git`; auth
/// exhausted / auth code → `AuthFailed`; Net/Http/Ssh → `NetworkError`; server
/// rejection (`push_update_reference` status / NotFastForward) → `PushRejected`;
/// other → `Git`.
pub fn push_tag(
    workdir: &Path,
    remote_name: &str,
    tag_name: &str,
    force: bool,
) -> Result<(), AppError> {
    let repo = open_repo_at(workdir)?;

    // Confirm the local tag exists (a clearer error than a server-side failure).
    if repo
        .find_reference(&format!("refs/tags/{tag_name}"))
        .is_err()
    {
        return Err(AppError::Git(format!("tag '{tag_name}' not found")));
    }

    let mut remote = match repo.find_remote(remote_name) {
        Ok(r) => r,
        Err(e) if e.code() == git2::ErrorCode::NotFound => {
            return Err(AppError::NoRemote(format!(
                "remote '{remote_name}' not found"
            )));
        }
        Err(e) => return Err(e.into()),
    };

    let attempts = RefCell::new(CredAttempts::default());
    let rejected: RefCell<Option<String>> = RefCell::new(None);
    {
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

        // NO leading '+' unless force (v1 UI: force == false).
        let plus = if force { "+" } else { "" };
        let refspec = format!("{plus}refs/tags/{tag_name}:refs/tags/{tag_name}");
        remote
            .push(&[refspec.as_str()], Some(&mut opts))
            .map_err(|e| map_remote_err(e, remote_name))?;
    }

    if let Some(msg) = rejected.into_inner() {
        return Err(AppError::PushRejected(format!(
            "push rejected by remote: {msg}. Bonsai v1 never force-pushes — fetch/pull first."
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Name-validation table (§8.3): blank / leading `-` / control chars reject
    /// with `InvalidName`; ordinary names pass validation.
    #[test]
    fn tag_name_validation_table() {
        for bad in ["", "   ", "-x", "-", "foo bar", "foo\tbar", "with\ncontrol"] {
            match validate_tag_name(bad) {
                Err(AppError::InvalidName(_)) => {}
                other => panic!("name {bad:?} must be InvalidName, got {other:?}"),
            }
        }
        for good in ["v1.0.0", "release-1", "feature/x", "a"] {
            validate_tag_name(good).unwrap_or_else(|e| panic!("name {good:?} must pass: {e:?}"));
        }
    }

    /// NIT (T2.7): `delete_tag` runs the SAME name validation as create —
    /// blank / leading-`-` names are `InvalidName` before touching libgit2.
    #[test]
    fn delete_tag_validates_name() {
        let dir = crate::testutil::scratch_dir();
        for bad in ["", "   ", "-x", "-"] {
            match delete_tag(dir.path(), bad) {
                Err(AppError::InvalidName(_)) => {}
                other => panic!("delete of {bad:?} must be InvalidName, got {other:?}"),
            }
        }
    }
}
