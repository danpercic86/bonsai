//! Stash seeding for the commit-graph walk (split out of `graph.rs`,
//! file-size discipline). Each stash commit `W` is injected as a walk TIP so it
//! renders as its own node; its synthetic parents (`I`/`U`) are skip-emitted.

use crate::error::AppError;

/// One stash resolved for the walk. `stash_oid` (= commit `W`) is pushed as a
/// revwalk TIP so the stash appears as its own node; `hide` = the stash's
/// synthetic parents (index commit `I` = parent 1, untracked commit `U` =
/// parent 2 if present) which are skip-emitted in `layout_walk` so they never
/// become nodes (NOT via `revwalk.hide`, which would also exclude `I`'s parent
/// `B` — see the note there). `W`'s FIRST parent (the base `B`) is left visible
/// and reached naturally, yielding the single `W → B` edge.
pub(super) struct StashSeed {
    pub(super) index: usize,
    pub(super) stash_oid: git2::Oid,
    pub(super) hide: Vec<git2::Oid>,
}

/// O(stashes). Enumerate the stash stack (ascending index, `stash@{0}` first);
/// for each, resolve `W` via the `refs/stash` reflog (entry `i`.`id_new()`), then
/// derive `hide` from `W`'s parents `[1..]` (skip parent 0 = base). A missing
/// `refs/stash` → empty; unresolvable entries are skipped. Requires `&mut` for
/// `stash_foreach`.
///
/// Perf: stashes add O(few) extra tips/hides to the walk. The M2d 20k perf
/// fixture contains no stashes, so this is a no-op there and the criterion
/// benchmark is unaffected.
pub(super) fn collect_stashes(repo: &mut git2::Repository) -> Result<Vec<StashSeed>, AppError> {
    let mut idxs: Vec<usize> = Vec::new();
    repo.stash_foreach(|index, _msg, _oid| {
        idxs.push(index);
        true
    })?;
    let reflog = match repo.reflog("refs/stash") {
        Ok(r) => r,
        Err(_) => return Ok(Vec::new()), // no stash ref → nothing to inject
    };
    let mut out = Vec::with_capacity(idxs.len());
    for &index in &idxs {
        if let Some(entry) = reflog.get(index) {
            let stash_oid = entry.id_new();
            if let Ok(commit) = repo.find_commit(stash_oid) {
                // parents 1.. = the index commit `I` and optional untracked
                // commit `U`; parent 0 = base `B` is left visible.
                let hide: Vec<git2::Oid> = commit.parent_ids().skip(1).collect();
                out.push(StashSeed {
                    index,
                    stash_oid,
                    hide,
                });
            }
        }
    }
    Ok(out) // ascending by index (stash@{0} first)
}

