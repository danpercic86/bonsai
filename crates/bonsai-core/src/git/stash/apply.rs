//! Stash apply / pop / drop and the Windows-reserved-path safety machinery
//! (P9 §2, T2.6 hardening).

use std::path::Path;

use crate::error::AppError;
use crate::git::bisect::require_no_bisect;
use crate::git::conflict::list_conflicts;
use crate::git::stage::open_workdir_repo;

use super::{require_clean, ApplyStashOutcome};

/// True if `component` is a Windows reserved device name that NTFS cannot use
/// as a path component: `CON`, `PRN`, `AUX`, `NUL`, `COM1..=COM9`, `LPT1..=LPT9`.
/// Windows strips trailing dots/spaces and resolves the STEM before the first
/// `.`, case-insensitively — so `NUL`, `nul`, `NUL.txt`, `NUL.`, and `NUL `
/// (trailing space) all count, while `NULl`, `NULL2`, `COM`/`COM0`/`COM10`, and
/// `LPT0` do not.
pub(crate) fn is_windows_reserved(component: &str) -> bool {
    let trimmed = component.trim_end_matches(['.', ' ']);
    let stem = trimmed.split('.').next().unwrap_or(trimmed);
    let upper = stem.to_ascii_uppercase();
    match upper.as_str() {
        "CON" | "PRN" | "AUX" | "NUL" => true,
        _ => {
            // COM1..=COM9 / LPT1..=LPT9 — a 3-letter prefix + exactly one 1-9 digit.
            let prefix = if let Some(rest) = upper.strip_prefix("COM") {
                Some(rest)
            } else {
                upper.strip_prefix("LPT")
            };
            match prefix {
                Some(rest) if rest.len() == 1 => {
                    matches!(rest.as_bytes()[0], b'1'..=b'9')
                }
                _ => false,
            }
        }
    }
}

/// Resolve the stash commit oid for `index` the same way `list_stashes` does:
/// the stash stack IS the `refs/stash` reflog, and entry `index`'s `id_new()`
/// is the stash commit oid. Missing entry → AppError::Git.
pub(crate) fn stash_commit_oid(repo: &git2::Repository, index: usize) -> Result<git2::Oid, AppError> {
    let reflog = repo.reflog("refs/stash")?;
    let entry = reflog
        .get(index)
        .ok_or_else(|| AppError::Git(format!("stash reflog entry {index} missing")))?;
    Ok(entry.id_new())
}

/// The full changed+untracked LEAF path set of stash `index`, split into
/// `(reserved, allowed)`. Sources:
///
/// - tracked: `commit.parent(0)` tree vs `commit` tree diff — each delta's
///   path bytes (non-UTF-8 → hard `AppError`, never silently dropped, F-A6-E);
/// - untracked: if `parent_count() >= 3`, an empty-tree → `commit.parent(2)`
///   tree diff, whose deltas are exactly the leaf blob paths (same UTF-8 rule).
///
/// Only leaf blob paths are collected — never a directory prefix. This matters
/// because `git_stash_apply`'s untracked-checkout phase reassigns
/// `checkout_strategy = SAFE | DONT_UPDATE_INDEX`, silently dropping our
/// `disable_pathspec_match(true)`, so `.path()` entries are treated as pathspec
/// PATTERNS there; listing only leaves means no allowed prefix can glob-match a
/// reserved leaf like `.../NUL`. The deduped-sorted union is split by whether
/// ANY `/`-component is `is_windows_reserved`: `reserved` drives detection,
/// `allowed` is the checkout allowlist for a skip-apply.
pub(crate) fn stash_path_sets(
    repo: &git2::Repository,
    index: usize,
) -> Result<(Vec<String>, Vec<String>), AppError> {
    let oid = stash_commit_oid(repo, index)?;
    let commit = repo.find_commit(oid)?;

    let mut paths: Vec<String> = Vec::new();

    // A path we cannot represent must be a HARD error, never a silent drop
    // (F-A6-E): a silently-missing entry in `allowed` means a skip-reserved
    // apply would silently fail to restore that file. Uses `path_bytes()`
    // (never `path()`, whose bytes→Path conversion can panic on Windows for
    // non-UTF-8 bytes) + explicit UTF-8 validation.
    let push_utf8 = |bytes: &[u8], paths: &mut Vec<String>| -> Result<(), AppError> {
        match std::str::from_utf8(bytes) {
            Ok(p) => {
                paths.push(p.to_string());
                Ok(())
            }
            Err(_) => Err(AppError::Git(format!(
                "stash contains a non-unicode path ({}); cannot compute the \
                 reserved-path safety sets",
                String::from_utf8_lossy(bytes)
            ))),
        }
    };

    // Tracked changes: parent(0) tree → stash tree.
    let stash_tree = commit.tree()?;
    let base_tree = commit.parent(0)?.tree()?;
    let diff = repo.diff_tree_to_tree(Some(&base_tree), Some(&stash_tree), None)?;
    for delta in diff.deltas() {
        if let Some(bytes) = delta
            .new_file()
            .path_bytes()
            .or_else(|| delta.old_file().path_bytes())
        {
            push_utf8(bytes, &mut paths)?;
        }
    }

    // Untracked commit (stash@{N}^3) — present only when created with
    // INCLUDE_UNTRACKED. Diff empty→tree instead of Tree::walk: the deltas
    // yield exactly the leaf blob paths, and `path_bytes()` stays available for
    // non-UTF-8 names (Tree::walk's `&str` root would panic on those before we
    // ever saw them).
    if commit.parent_count() >= 3 {
        let untracked_tree = commit.parent(2)?.tree()?;
        let udiff = repo.diff_tree_to_tree(None, Some(&untracked_tree), None)?;
        for delta in udiff.deltas() {
            if let Some(bytes) = delta
                .new_file()
                .path_bytes()
                .or_else(|| delta.old_file().path_bytes())
            {
                push_utf8(bytes, &mut paths)?;
            }
        }
    }

    paths.sort();
    paths.dedup();

    let (reserved, allowed): (Vec<String>, Vec<String>) = paths
        .into_iter()
        .partition(|p| p.split('/').any(is_windows_reserved));
    Ok((reserved, allowed))
}

/// Escape fnmatch/pathspec metacharacters (`\ * ? [ ]`) so `p` self-matches
/// when a checkout phase treats the allowlist entries as PATTERNS. Returns
/// `None` when the path contains no metacharacter (escaped == raw).
pub(crate) fn escape_pathspec(p: &str) -> Option<String> {
    const META: [char; 5] = ['\\', '*', '?', '[', ']'];
    if !p.contains(META) {
        return None;
    }
    let mut out = String::with_capacity(p.len() + 4);
    for c in p.chars() {
        if META.contains(&c) {
            out.push('\\');
        }
        out.push(c);
    }
    Some(out)
}

/// Build the SAFE `StashApplyOptions` for a skip-reserved apply: a checkout
/// allowlist of exactly the `allowed` paths (pathspec match disabled so only the
/// listed paths are written). The reserved paths are simply left out, so both
/// libgit2 checkout phases skip them.
///
/// DUAL-FORM entries (F-A6-F): the tracked-checkout phase honors
/// `disable_pathspec_match` (entries are LITERAL paths → the raw form matches),
/// but `git_stash_apply`'s untracked-checkout phase reassigns the checkout
/// strategy and silently drops that flag (entries become PATTERNS → a raw
/// `foo[1].txt` would NOT self-match and the file would silently not be
/// restored). So every metachar-bearing path is added twice: raw (literal
/// phase) + glob-escaped (pattern phase). An escaped entry matches nothing in
/// the literal phase and exactly its raw path in the pattern phase; the
/// post-apply reserved-path guard in apply/pop still backstops any pattern
/// over-match.
fn skip_reserved_opts(allowed: &[String]) -> git2::StashApplyOptions<'_> {
    let mut co = git2::build::CheckoutBuilder::new();
    co.disable_pathspec_match(true);
    for p in allowed {
        co.path(p);
        if let Some(escaped) = escape_pathspec(p) {
            co.path(escaped);
        }
    }
    let mut opts = git2::StashApplyOptions::new();
    opts.checkout_options(co);
    opts
}

/// F-A6-B guard: apply/pop/drop are index-addressed into a MUTATING stack; a
/// shift between the UI render and the confirm (external `git stash`, or an
/// in-app autostash retained on conflict) would silently target the WRONG,
/// unrecoverable entry. When the caller supplies the stash commit oid it saw,
/// verify it still lives at `index` before acting; mismatch → error, nothing
/// touched. `None` (legacy/internal callers) preserves the old behavior.
fn verify_expected_oid(
    repo: &git2::Repository,
    index: usize,
    expected_oid: Option<&str>,
) -> Result<(), AppError> {
    let Some(expected) = expected_oid else {
        return Ok(());
    };
    let actual = stash_commit_oid(repo, index)?;
    if !actual.to_string().eq_ignore_ascii_case(expected.trim()) {
        return Err(AppError::Git(
            "stash list changed; refresh and retry".to_string(),
        ));
    }
    Ok(())
}

/// Blocking. Apply stash `index` WITHOUT dropping it. Precondition: state Clean
/// else OperationInProgress. Conflicts → Ok(Conflicts{paths}) (stash retained).
///
/// `skip_reserved == false`: if the stash holds Windows-reserved paths, return
/// `ReservedPaths` WITHOUT applying anything (nothing mutated); otherwise apply
/// exactly as before. `skip_reserved == true`: apply with a checkout allowlist
/// that skips the reserved paths and return `AppliedSkippingReserved` on clean
/// success.
///
/// `expected_oid` (F-A6-B): the stash commit oid the caller saw for `index`;
/// `Some` + mismatch → "stash list changed" error before anything is applied.
pub fn apply_stash(
    workdir: &Path,
    index: usize,
    skip_reserved: bool,
    expected_oid: Option<&str>,
) -> Result<ApplyStashOutcome, AppError> {
    let mut repo = open_workdir_repo(workdir)?;
    require_clean(&repo)?;
    // A clean detached-HEAD bisect is invisible to `require_clean` — refuse.
    require_no_bisect(&repo)?;
    verify_expected_oid(&repo, index, expected_oid)?;

    if !skip_reserved {
        // Preflight: block (mutate nothing) if reserved paths are present.
        let (reserved, _allowed) = stash_path_sets(&repo, index)?;
        if !reserved.is_empty() {
            return Ok(ApplyStashOutcome::ReservedPaths { paths: reserved });
        }
        let mut opts = git2::StashApplyOptions::new(); // SAFE default; NO REINSTATE_INDEX (OPEN Q#4)
        return match repo.stash_apply(index, Some(&mut opts)) {
            Ok(()) => {
                // This libgit2 may report a CONTENT conflict as Ok(()) with markers
                // in the worktree + conflict entries in the index. Inspect the index
                // rather than trusting the return code (P8 lesson).
                if repo.index()?.has_conflicts() {
                    Ok(ApplyStashOutcome::Conflicts {
                        paths: conflict_paths(workdir)?,
                    })
                } else {
                    Ok(ApplyStashOutcome::Applied)
                }
            }
            Err(e) if e.code() == git2::ErrorCode::Conflict => {
                checkout_conflict_outcome(workdir, &e)
            }
            Err(e) => Err(e.into()),
        };
    }

    // skip_reserved: apply with an allowlist that omits the reserved paths.
    let (reserved, allowed) = stash_path_sets(&repo, index)?;
    if allowed.is_empty() {
        // EVERY path is reserved. libgit2 treats a zero-length checkout pathspec
        // as "match everything", so passing an empty allowlist would re-attempt
        // the reserved checkout and surface a raw error. Nothing to apply →
        // short-circuit. Apply never drops, so the stash is retained regardless.
        return Ok(ApplyStashOutcome::AppliedSkippingReserved { skipped: reserved });
    }
    let mut opts = skip_reserved_opts(&allowed);
    match repo.stash_apply(index, Some(&mut opts)) {
        Ok(()) => {
            if repo.index()?.has_conflicts() {
                Ok(ApplyStashOutcome::Conflicts {
                    paths: conflict_paths(workdir)?,
                })
            } else {
                // POST-APPLY GUARD: the untracked-checkout phase reassigns the
                // checkout strategy and silently drops `disable_pathspec_match`,
                // so verify no reserved path actually materialized on disk. If
                // one did (e.g. a sibling filename with pathspec metacharacters
                // glob-matched it, or rename detection produced an unexpected
                // path), fail loudly. The stash was never dropped → still lossless.
                for p in &reserved {
                    if workdir.join(p).exists() {
                        return Err(AppError::Git(format!(
                            "stash apply could not skip reserved path {p} cleanly; stash retained"
                        )));
                    }
                }
                Ok(ApplyStashOutcome::AppliedSkippingReserved { skipped: reserved })
            }
        }
        Err(e) if e.code() == git2::ErrorCode::Conflict => checkout_conflict_outcome(workdir, &e),
        Err(e) => Err(e.into()),
    }
}

/// Blocking. Apply stash `index` and drop it on clean success only.
/// Precondition: state Clean else OperationInProgress. Conflicts →
/// Ok(Conflicts{paths}) and the entry is RETAINED (libgit2 does not drop).
///
/// Mirrors `merge::pop_after_success`: uses `stash_apply` + inspect index +
/// conditional `stash_drop` so WE control dropping. A naive `stash_pop` would
/// silently drop the entry on this libgit2's false-clean content conflict —
/// data loss.
///
/// `skip_reserved == false`: if the stash holds Windows-reserved paths, return
/// `ReservedPaths` WITHOUT applying/dropping anything. Otherwise apply + drop on
/// clean success as before. `skip_reserved == true`: apply skipping the reserved
/// paths; because those blobs live ONLY in the stash, a skip-apply must stay
/// lossless — return `AppliedSkippingReserved` and RETAIN the stash (do NOT drop).
///
/// `expected_oid` (F-A6-B): the stash commit oid the caller saw for `index`;
/// `Some` + mismatch → "stash list changed" error before anything is applied
/// or dropped.
pub fn pop_stash(
    workdir: &Path,
    index: usize,
    skip_reserved: bool,
    expected_oid: Option<&str>,
) -> Result<ApplyStashOutcome, AppError> {
    let mut repo = open_workdir_repo(workdir)?;
    pop_stash_with(&mut repo, workdir, index, skip_reserved, expected_oid)
}

/// Handle-reusing twin of [`pop_stash`] (P88b/B2a): runs against an already-open
/// `&mut Repository` so a composite mutation opens the repo once. `workdir` is
/// still threaded for the conflict-path reader. Byte-identical to `pop_stash`
/// minus the `open_workdir_repo` call (the bare-repo guard is preserved by the
/// composite's own open path).
pub fn pop_stash_with(
    repo: &mut git2::Repository,
    workdir: &Path,
    index: usize,
    skip_reserved: bool,
    expected_oid: Option<&str>,
) -> Result<ApplyStashOutcome, AppError> {
    require_clean(&*repo)?;
    // A clean detached-HEAD bisect is invisible to `require_clean` — refuse.
    require_no_bisect(&*repo)?;
    verify_expected_oid(&*repo, index, expected_oid)?;

    if !skip_reserved {
        // Preflight: block (mutate nothing, never drop) on reserved paths.
        let (reserved, _allowed) = stash_path_sets(&*repo, index)?;
        if !reserved.is_empty() {
            return Ok(ApplyStashOutcome::ReservedPaths { paths: reserved });
        }
        let mut opts = git2::StashApplyOptions::new(); // SAFE default; NO REINSTATE_INDEX (OPEN Q#4)
        return match repo.stash_apply(index, Some(&mut opts)) {
            Ok(()) => {
                if repo.index()?.has_conflicts() {
                    // Conflicted re-apply: LEAVE the stash on the stack (do NOT
                    // drop) → retained for the user to resolve.
                    Ok(ApplyStashOutcome::Conflicts {
                        paths: conflict_paths(workdir)?,
                    })
                } else {
                    // Clean apply → now drop, equivalent to a clean pop.
                    repo.stash_drop(index)?;
                    Ok(ApplyStashOutcome::Applied)
                }
            }
            // A checkout-level conflict means nothing droppable was applied →
            // stash retained.
            Err(e) if e.code() == git2::ErrorCode::Conflict => {
                checkout_conflict_outcome(workdir, &e)
            }
            Err(e) => Err(e.into()),
        };
    }

    // skip_reserved: apply the non-reserved paths. Because the skipped blobs
    // survive ONLY in the stash, we must NOT drop — pop stays lossless here.
    let (reserved, allowed) = stash_path_sets(&*repo, index)?;
    if allowed.is_empty() {
        // EVERY path is reserved. libgit2 treats a zero-length checkout pathspec
        // as "match everything", so passing an empty allowlist would re-attempt
        // the reserved checkout and surface a raw error. Nothing to apply →
        // short-circuit. Do NOT drop: the reserved blobs live only in the stash,
        // so retaining it keeps pop lossless.
        return Ok(ApplyStashOutcome::AppliedSkippingReserved { skipped: reserved });
    }
    let mut opts = skip_reserved_opts(&allowed);
    match repo.stash_apply(index, Some(&mut opts)) {
        Ok(()) => {
            if repo.index()?.has_conflicts() {
                Ok(ApplyStashOutcome::Conflicts {
                    paths: conflict_paths(workdir)?,
                })
            } else {
                // POST-APPLY GUARD (see apply_stash): reserved paths must not exist.
                for p in &reserved {
                    if workdir.join(p).exists() {
                        return Err(AppError::Git(format!(
                            "stash apply could not skip reserved path {p} cleanly; stash retained"
                        )));
                    }
                }
                // RETAIN the stash: the reserved blobs are unrecoverable if dropped.
                Ok(ApplyStashOutcome::AppliedSkippingReserved { skipped: reserved })
            }
        }
        Err(e) if e.code() == git2::ErrorCode::Conflict => checkout_conflict_outcome(workdir, &e),
        Err(e) => Err(e.into()),
    }
}

/// Blocking. Permanently discard stash `index`. Allowed in ANY repo state
/// (touches only the stash reflog). UI confirms first (destructive). An
/// out-of-range index surfaces as the underlying git2 error → AppError::Git.
///
/// `expected_oid` (F-A6-B): the stash commit oid the caller saw for `index`;
/// `Some` + mismatch → "stash list changed" error and NOTHING is dropped —
/// this is the wrong-target-destructive guard (a dropped stash is
/// unrecoverable).
pub fn drop_stash(workdir: &Path, index: usize, expected_oid: Option<&str>) -> Result<(), AppError> {
    let mut repo = open_workdir_repo(workdir)?;
    verify_expected_oid(&repo, index, expected_oid)?;
    repo.stash_drop(index)?;
    Ok(())
}

/// Maps a `GIT_ECONFLICT` from `stash_apply` to an outcome. A MERGE-level
/// conflict leaves conflict entries in the index → `Conflicts { paths }`. A
/// CHECKOUT-level conflict (a dirty/untracked file in the way) means NOTHING
/// was applied and the index has no conflict entries — an empty `Conflicts`
/// would render as "conflicts" with no paths, so surface libgit2's message
/// (it names the blocking file) as a hard error instead. The stash is
/// retained either way (apply never drops).
fn checkout_conflict_outcome(
    workdir: &Path,
    e: &git2::Error,
) -> Result<ApplyStashOutcome, AppError> {
    let paths = conflict_paths(workdir)?;
    if paths.is_empty() {
        return Err(AppError::Git(format!(
            "stash apply was blocked at checkout: {}. Stash retained.",
            e.message()
        )));
    }
    Ok(ApplyStashOutcome::Conflicts { paths })
}

/// Sorted conflicted paths — the exact set `list_conflicts` returns (P8 reuse).
fn conflict_paths(workdir: &Path) -> Result<Vec<String>, AppError> {
    Ok(list_conflicts(workdir)?
        .into_iter()
        .map(|c| c.path)
        .collect())
}
