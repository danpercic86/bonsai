//! Stash listing: enumerate the stash stack (P9 contract §2).

use std::path::Path;

use crate::error::AppError;
use crate::git::stage::open_workdir_repo;

use super::StashEntry;

/// Blocking. Enumerate the stash stack, index 0 (most recent) first.
/// `stash_foreach` is the ONLY enumeration API; its callback receives
/// (index, message, &oid). Empty stack → Ok(vec![]).
///
/// The closure cannot re-borrow `repo` (it is mutably borrowed for the
/// duration), so we collect (index, message) inside it and resolve
/// oid/base_oid/ts AFTER via the stash reflog (`refs/stash`), where entry `i`'s
/// `id_new()` is the stash commit oid.
pub fn list_stashes(workdir: &Path) -> Result<Vec<StashEntry>, AppError> {
    let mut repo = open_workdir_repo(workdir)?; // rejects bare
    list_stashes_with(&mut repo)
}

/// Blocking. P88b/B2b: [`list_stashes`] from an ALREADY-OPEN handle (round handle
/// cache). The `&Path` entry point above opens (rejecting bare) then delegates
/// here. `&mut` is required because `stash_foreach` needs it; the stash reflog
/// (`refs/stash`) + its commits are re-read on demand ⇒ byte-identical output.
/// The routed command never reaches a bare repo (bare repos are excluded from
/// the open-repos map), so no bare guard is duplicated here.
pub fn list_stashes_with(repo: &mut git2::Repository) -> Result<Vec<StashEntry>, AppError> {
    let mut raw: Vec<(usize, String)> = Vec::new();
    repo.stash_foreach(|index, message, _oid| {
        // NOTE: we intentionally do NOT capture oid here — resolving base/ts
        // needs an immutable repo borrow, impossible inside this &mut closure.
        raw.push((index, message.to_string()));
        true
    })?;

    if raw.is_empty() {
        return Ok(Vec::new());
    }

    // Resolve each entry via the stash reflog (index order; entry 0 == stash@{0}).
    let reflog = repo.reflog("refs/stash")?;
    let mut out = Vec::with_capacity(raw.len());
    for (index, message) in raw {
        let entry = reflog.get(index).ok_or_else(|| {
            AppError::Git(format!("stash reflog entry {index} missing"))
        })?;
        let stash_oid = entry.id_new();
        let commit = repo.find_commit(stash_oid)?;
        let base_oid = commit.parent_id(0)?;
        out.push(StashEntry {
            index,
            message,
            oid: commit.id().to_string(),
            base_oid: base_oid.to_string(),
            ts: commit.author().when().seconds(),
        });
    }
    Ok(out)
}
