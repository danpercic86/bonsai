//! Tag sync management core (P77 contract).
//!
//! Live remote-truth tag reconciliation: one `ls-remote` against a chosen
//! remote, joined with the local tags, classified per tag against the remote's
//! current target. Plus the two single-tag resolve ops (`force_refresh_tag`,
//! `delete_remote_tag`). Force-move-remote reuses `tags::push_tag(force=true)`,
//! push-unpushed reuses `tags::push_tag(force=false)`, delete-local reuses
//! `tags::delete_tag` — this module does NOT duplicate them.
//!
//! Pure git2 logic, no Tauri types — testable against local bare-repo remotes.
//! All functions blocking; the command layer wraps them in `spawn_blocking`.
//!
//! Credentials reuse the M6 chain (`acquire_cred` / `CredAttempts` /
//! `map_remote_err` / `evict_fresh_on_auth_fail`, already `pub(crate)`): Git's
//! configured credential helper → SSH agent → default. NEVER prompts, NEVER
//! stores passwords.
//!
//! **Annotated-tag correctness (the crux):** an annotated tag's `refs/tags/X`
//! points at a *tag object*, while ls-remote also advertises the *peeled
//! committish* as `refs/tags/X^{}`. We compare the PEELED committish on both
//! sides — never the annotated-tag-object oid — so an annotated tag whose target
//! matches is `InSync`, not a false `Stale`. `annotated` is a display flag only.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::path::Path;

use crate::error::AppError;
use crate::git::cred::evict_fresh_on_auth_fail;
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
/// validation for `refs/tags/<name>` (mirrors `tags::validate_tag_name`, which
/// is private to that module).
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

/// One tag's reconciliation state against a chosen remote (P77). Compact,
/// fully precomputed — the frontend renders it verbatim, no per-tag round-trips.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TagSyncEntry {
    /// Short tag name (no `refs/tags/` prefix).
    pub name: String,
    pub status: TagSyncStatus,
    /// Peeled committish the LOCAL tag resolves to (40-hex). None => remote-only.
    pub local_oid: Option<String>,
    /// Peeled committish the REMOTE tag resolves to (40-hex). None => local-only.
    pub remote_oid: Option<String>,
    /// True if the tag is an annotated tag object on EITHER side.
    pub annotated: bool,
}

/// serde => kebab-case: "in-sync" | "local-only" | "stale" | "remote-only"
/// | "deleted-on-remote".
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum TagSyncStatus {
    /// Both sides present, peeled committish equal.
    InSync,
    /// Present locally, absent on remote. In v1 this SUBSUMES deleted-on-remote
    /// (contract decision D1); `DeletedOnRemote` is not emitted unless a
    /// pushed-set is available.
    LocalOnly,
    /// Both sides present, peeled committish differ (the moved-tag case).
    Stale,
    /// Present on remote, absent locally.
    RemoteOnly,
    /// Reserved: present locally, previously pushed to this remote, now gone
    /// upstream. NOT produced in v1 (folded into `LocalOnly`) — kept in the enum
    /// so the UI can label it if D1 flips to the pushed-set option.
    #[allow(dead_code)]
    DeletedOnRemote,
}

/// Result of one live ls-remote reconciliation pass.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TagSyncReport {
    /// The remote actually queried (resolved default when caller passed None).
    pub remote: String,
    /// One row per tag in the local∪remote union, sorted case-insensitively.
    pub entries: Vec<TagSyncEntry>,
}

/// "origin" if configured, else the first entry of `repo.remotes()`; `NoRemote`
/// if none configured. A caller-supplied `remote` short-circuits the default.
fn resolve_default_remote(
    repo: &git2::Repository,
    remote: Option<&str>,
) -> Result<String, AppError> {
    if let Some(name) = remote {
        let name = name.trim();
        if !name.is_empty() {
            return Ok(name.to_string());
        }
    }
    let remotes = repo.remotes()?;
    let names: Vec<String> = remotes
        .iter()
        .map(|n| n.ok().flatten())
        .filter_map(|n| n.map(str::to_string))
        .collect();
    if names.iter().any(|n| n == "origin") {
        return Ok("origin".to_string());
    }
    names.into_iter().next().ok_or_else(|| {
        AppError::NoRemote("no remotes configured to compare tags against".to_string())
    })
}

/// Live ls-remote of only `refs/tags/*`. Returns (full_ref_name, oid) pairs,
/// INCLUDING peeled `refs/tags/X^{}` entries (authoritative for annotated tags).
/// Auth/network mapped via `map_remote_err`; fresh creds evicted on auth fail.
fn ls_remote_tags(
    repo: &git2::Repository,
    remote_name: &str,
) -> Result<Vec<(String, git2::Oid)>, AppError> {
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

    if let Err(e) = remote.connect_auth(git2::Direction::Fetch, Some(callbacks), None) {
        return Err(evict_fresh_on_auth_fail(
            repo,
            &attempts,
            map_remote_err(e, remote_name),
        ));
    }

    let out = match remote.list() {
        Ok(heads) => heads
            .iter()
            .filter(|h| h.name().starts_with("refs/tags/"))
            .map(|h| (h.name().to_string(), h.oid()))
            .collect::<Vec<_>>(),
        Err(e) => {
            remote.disconnect().ok();
            return Err(map_remote_err(e, remote_name));
        }
    };
    remote.disconnect().ok();
    Ok(out)
}

/// Local side of the join: tag name -> (peeled committish oid, annotated).
/// Tags whose ref cannot be peeled at all are skipped (defensive — a broken ref
/// should not fail the whole reconciliation).
fn collect_local_tags(repo: &git2::Repository) -> Result<HashMap<String, (git2::Oid, bool)>, AppError> {
    let mut local = HashMap::new();
    let refs = repo.references_glob("refs/tags/*")?;
    for r in refs {
        let r = match r {
            Ok(r) => r,
            Err(_) => continue,
        };
        let full = match r.name() {
            Ok(n) => n,
            Err(_) => continue, // non-UTF-8 ref name — skip
        };
        let name = match full.strip_prefix("refs/tags/") {
            Some(n) => n.to_string(),
            None => continue,
        };
        // peel(Any) follows an annotated tag object down to its target
        // (commit, or tree/blob for exotic tags) and stops there — the
        // committish we compare against the remote's peeled entry.
        let peeled = match r.peel(git2::ObjectType::Any) {
            Ok(obj) => obj.id(),
            Err(_) => continue,
        };
        // A ref that peels through a tag OBJECT is annotated (display flag only).
        let annotated = r.peel(git2::ObjectType::Tag).is_ok();
        local.insert(name, (peeled, annotated));
    }
    Ok(local)
}

/// Parse ls-remote output into the remote side of the join: tag name -> peeled
/// committish oid, plus the set of names advertised with a `^{}` peel (annotated
/// on the remote). `^{}` entries are authoritative and OVERWRITE; plain entries
/// only fill a not-yet-seen slot (so a lightweight tag keeps its commit oid and
/// an annotated tag ends up with its peeled committish regardless of emit order).
fn parse_remote_tags(
    pairs: &[(String, git2::Oid)],
) -> (HashMap<String, git2::Oid>, HashSet<String>) {
    let mut remote: HashMap<String, git2::Oid> = HashMap::new();
    let mut annotated: HashSet<String> = HashSet::new();
    for (full, oid) in pairs {
        if let Some(rest) = full.strip_prefix("refs/tags/") {
            if let Some(base) = rest.strip_suffix("^{}") {
                remote.insert(base.to_string(), *oid); // ^{} is authoritative
                annotated.insert(base.to_string());
            } else {
                remote.entry(rest.to_string()).or_insert(*oid);
            }
        }
    }
    (remote, annotated)
}

/// Join the local tags with the parsed remote tags and classify each (§4).
fn classify(
    local: &HashMap<String, (git2::Oid, bool)>,
    remote: &HashMap<String, git2::Oid>,
    remote_annotated: &HashSet<String>,
) -> Vec<TagSyncEntry> {
    let mut names: Vec<&String> = local.keys().chain(remote.keys()).collect();
    names.sort_by(|a, b| a.to_lowercase().cmp(&b.to_lowercase()).then(a.cmp(b)));
    names.dedup();

    let mut entries = Vec::with_capacity(names.len());
    for name in names {
        let l = local.get(name);
        let r = remote.get(name);
        let annotated = l.map(|(_, a)| *a).unwrap_or(false) || remote_annotated.contains(name);
        let status = match (l, r) {
            (Some((lo, _)), Some(ro)) => {
                if lo == ro {
                    TagSyncStatus::InSync
                } else {
                    TagSyncStatus::Stale
                }
            }
            (Some(_), None) => TagSyncStatus::LocalOnly, // D1: deleted-on-remote folded in
            (None, Some(_)) => TagSyncStatus::RemoteOnly,
            (None, None) => continue, // unreachable — a name comes from one side
        };
        entries.push(TagSyncEntry {
            name: name.clone(),
            status,
            local_oid: l.map(|(o, _)| o.to_string()),
            remote_oid: r.map(|o| o.to_string()),
            annotated,
        });
    }
    entries
}

/// Blocking. Live tag reconciliation against `remote` (None => default: "origin"
/// else the first configured remote). One ls-remote round-trip.
///
/// Errors: no remotes → `NoRemote`; remote not found → `NoRemote`; auth
/// exhausted / auth code → `AuthFailed`; Net/Http/Ssh → `NetworkError`; other →
/// `Git`. Never panics — the frontend degrades to the plain tags list.
pub fn list_tag_sync(workdir: &Path, remote: Option<&str>) -> Result<TagSyncReport, AppError> {
    let repo = open_repo_at(workdir)?;
    let remote_name = resolve_default_remote(&repo, remote)?;

    let local = collect_local_tags(&repo)?;
    let remote_pairs = ls_remote_tags(&repo, &remote_name)?;
    let (remote_map, remote_annotated) = parse_remote_tags(&remote_pairs);

    let entries = classify(&local, &remote_map, &remote_annotated);
    Ok(TagSyncReport {
        remote: remote_name,
        entries,
    })
}

/// Blocking. Force-update ONE local tag from `remote` (refspec
/// `+refs/tags/<n>:refs/tags/<n>`, `AutotagOption::None`). Resolves the reported
/// stale/moved bug: overwrites the local tag with the remote's current target.
///
/// Errors: invalid/blank name → `InvalidName`; remote not found → `NoRemote`;
/// auth → `AuthFailed`; Net/Http/Ssh → `NetworkError`; other → `Git`.
pub fn force_refresh_tag(workdir: &Path, remote_name: &str, tag_name: &str) -> Result<(), AppError> {
    validate_tag_name(tag_name)?;
    let repo = open_repo_at(workdir)?;

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
    opts.download_tags(git2::AutotagOption::None); // fetch ONLY the named tag

    // Leading '+' = force-update the local ref to the remote's target.
    let refspec = format!("+refs/tags/{tag_name}:refs/tags/{tag_name}");
    remote
        .fetch(&[refspec.as_str()], Some(&mut opts), None)
        .map_err(|e| evict_fresh_on_auth_fail(&repo, &attempts, map_remote_err(e, remote_name)))?;
    Ok(())
}

/// Blocking. Delete a tag ON the remote (refspec `:refs/tags/<n>` — empty
/// source deletes the remote ref). NET-NEW. Destructive — the caller MUST
/// confirm in the UI first.
///
/// Errors: invalid/blank name → `InvalidName`; remote not found → `NoRemote`;
/// auth → `AuthFailed`; Net/Http/Ssh → `NetworkError`; server rejection
/// (`push_update_reference` status / NotFastForward) → `PushRejected`; other →
/// `Git`.
pub fn delete_remote_tag(workdir: &Path, remote_name: &str, tag_name: &str) -> Result<(), AppError> {
    validate_tag_name(tag_name)?;
    let repo = open_repo_at(workdir)?;

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

        // Empty source = delete the remote ref.
        let refspec = format!(":refs/tags/{tag_name}");
        remote
            .push(&[refspec.as_str()], Some(&mut opts))
            .map_err(|e| map_remote_err(e, remote_name))?;
    }

    if let Some(msg) = rejected.into_inner() {
        return Err(AppError::PushRejected(format!(
            "remote rejected deleting tag '{tag_name}': {msg}"
        )));
    }
    Ok(())
}

#[cfg(test)]
#[path = "tag_sync_tests.rs"]
mod tests;
