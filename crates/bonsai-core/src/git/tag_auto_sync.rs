//! P84 automatic tag reconciliation — the non-interactive fetch-time pass split
//! out of `tag_sync.rs` to keep each file focused. Reuses the parent module's
//! repo/name/validation helpers (`super::*`) and the M6 credential chain.
//!
//! Behaviour: fetch the remote's tag OBJECTS into a private namespace
//! (`refs/bonsai-tagsync/*`) so both committishes exist locally for ancestry
//! checks, then: adopt remote-only tags, fast-forward a stale local tag ONLY
//! when the remote committish strictly descends from the local one, and skip
//! (record) diverged / local-ahead tags. NEVER fails the fetch — no remote,
//! remote-not-found, or auth/network failure yields an empty Ok report. The temp
//! namespace is always cleaned up before returning.

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::Path;

use crate::error::AppError;
use crate::git::cred::evict_fresh_on_auth_fail;
use crate::git::remote::{acquire_cred, map_remote_err, CredAttempts};

use super::{collect_local_tags, open_repo_at, resolve_default_remote, validate_tag_name};

/// Private namespace glob used to stage the remote's tag objects locally for
/// ancestry checks without touching `refs/tags/*`. Always cleaned up.
pub(crate) const TAGSYNC_GLOB: &str = "refs/bonsai-tagsync/*";

/// Result of one non-interactive auto-sync pass. Compact, camelCase on the wire.
#[derive(Debug, Clone, Default, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TagAutoSyncReport {
    /// The remote actually reconciled ("" when none configured / skipped).
    pub remote: String,
    /// Tag names newly created locally from a remote-only tag.
    pub adopted: Vec<String>,
    /// Tag names whose local ref was fast-forwarded onto the remote target.
    pub moved: Vec<String>,
    /// Stale tags left untouched (local ahead or diverged — not a strict FF).
    pub skipped_diverged: Vec<String>,
}

/// Deletes every ref under `refs/bonsai-tagsync/*`. Best-effort: a ref that
/// cannot be read/deleted is skipped rather than failing the whole cleanup, so
/// this is safe to call on any exit path (success or error).
pub(crate) fn cleanup_temp_refs(repo: &git2::Repository) {
    let refs = match repo.references_glob(TAGSYNC_GLOB) {
        Ok(r) => r,
        Err(_) => return,
    };
    for r in refs.flatten() {
        let mut r = r;
        let _ = r.delete();
    }
}

/// Fetched side of the join: tag name -> (raw target oid, peeled committish oid).
/// The RAW oid is what we write into `refs/tags/<name>` (keeps annotated tags
/// annotated); the peeled committish is used ONLY for ancestry comparison.
fn collect_temp_tags(
    repo: &git2::Repository,
) -> Result<HashMap<String, (git2::Oid, git2::Oid)>, AppError> {
    let mut out = HashMap::new();
    let refs = repo.references_glob(TAGSYNC_GLOB)?;
    for r in refs {
        let r = match r {
            Ok(r) => r,
            Err(_) => continue,
        };
        let full = match r.name() {
            Ok(n) => n,
            Err(_) => continue,
        };
        let name = match full.strip_prefix("refs/bonsai-tagsync/") {
            Some(n) => n.to_string(),
            None => continue,
        };
        let raw = match r.target() {
            Some(oid) => oid,
            None => continue, // symbolic ref — skip defensively
        };
        let peeled = match r.peel(git2::ObjectType::Any) {
            Ok(obj) => obj.id(),
            Err(_) => continue,
        };
        out.insert(name, (raw, peeled));
    }
    Ok(out)
}

/// Fetch the remote's tag objects into `refs/bonsai-tagsync/*` (force, no
/// auto-tag). Auth/network errors are mapped but the CALLER decides they are
/// best-effort (returns an empty report rather than propagating).
fn fetch_temp_tags(repo: &git2::Repository, remote_name: &str) -> Result<(), AppError> {
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
    let mut callbacks = git2::RemoteCallbacks::new();
    callbacks.credentials(|url, username_from_url, allowed| {
        acquire_cred(repo.workdir(), &attempts, url, username_from_url, allowed)
    });

    let mut opts = git2::FetchOptions::new();
    opts.remote_callbacks(callbacks);
    opts.download_tags(git2::AutotagOption::None);

    let refspec = "+refs/tags/*:refs/bonsai-tagsync/*";
    remote
        .fetch(&[refspec], Some(&mut opts), None)
        .map_err(|e| evict_fresh_on_auth_fail(repo, &attempts, map_remote_err(e, remote_name)))
}

/// Blocking, best-effort, NEVER-fail-the-fetch tag reconciliation (P84). See the
/// module docs. `remote`: None => default ("origin" else the first configured
/// remote).
pub fn auto_sync_tags(
    workdir: &Path,
    remote: Option<&str>,
) -> Result<TagAutoSyncReport, AppError> {
    let repo = open_repo_at(workdir)?;

    // No remote configured => empty Ok (not an error).
    let remote_name = match resolve_default_remote(&repo, remote) {
        Ok(n) => n,
        Err(AppError::NoRemote(_)) => return Ok(TagAutoSyncReport::default()),
        Err(e) => return Err(e),
    };

    // Clear any temp refs left behind by a previous run that crashed before its
    // cleanup — otherwise a stale `refs/bonsai-tagsync/*` (whose remote tag was
    // since deleted, so the force refspec won't overwrite it) could be re-adopted.
    cleanup_temp_refs(&repo);

    // Best-effort fetch into the private namespace. Auth/network/not-found =>
    // empty report (naming the remote), NEVER propagate.
    if fetch_temp_tags(&repo, &remote_name).is_err() {
        cleanup_temp_refs(&repo);
        return Ok(TagAutoSyncReport {
            remote: remote_name,
            ..Default::default()
        });
    }

    // From here on always clean up the temp namespace before returning.
    let result = reconcile_temp_tags(&repo, &remote_name);
    cleanup_temp_refs(&repo);
    result
}

/// The classify+apply core of `auto_sync_tags`, factored out so the caller can
/// guarantee `cleanup_temp_refs` runs on every path.
fn reconcile_temp_tags(
    repo: &git2::Repository,
    remote_name: &str,
) -> Result<TagAutoSyncReport, AppError> {
    let mut report = TagAutoSyncReport {
        remote: remote_name.to_string(),
        ..Default::default()
    };

    let local = collect_local_tags(repo)?; // name -> (peeled_oid, annotated)
    let temp = collect_temp_tags(repo)?; // name -> (raw_oid, peeled_oid)

    for (name, (raw_oid, remote_peeled)) in &temp {
        match local.get(name) {
            None => {
                // RemoteOnly => ADOPT at the raw target (keeps annotated tags annotated).
                if validate_tag_name(name).is_err() {
                    continue;
                }
                if repo
                    .reference(
                        &format!("refs/tags/{name}"),
                        *raw_oid,
                        false,
                        "bonsai auto-sync: adopt",
                    )
                    .is_ok()
                {
                    report.adopted.push(name.clone());
                }
            }
            Some((local_peeled, _)) => {
                if local_peeled == remote_peeled {
                    continue; // InSync
                }
                // Stale: move ONLY if the remote committish strictly descends.
                let ff = matches!(
                    repo.graph_descendant_of(*remote_peeled, *local_peeled),
                    Ok(true)
                );
                if ff {
                    if repo
                        .reference(
                            &format!("refs/tags/{name}"),
                            *raw_oid,
                            true,
                            "bonsai auto-sync: move",
                        )
                        .is_ok()
                    {
                        report.moved.push(name.clone());
                    }
                } else {
                    report.skipped_diverged.push(name.clone());
                }
            }
        }
    }

    let ci = |v: &mut Vec<String>| {
        v.sort_by(|a, b| a.to_lowercase().cmp(&b.to_lowercase()).then(a.cmp(b)));
    };
    ci(&mut report.adopted);
    ci(&mut report.moved);
    ci(&mut report.skipped_diverged);
    Ok(report)
}

#[cfg(test)]
#[path = "tag_auto_sync_tests.rs"]
mod tests;
