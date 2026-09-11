//! MCP-only preconditions for the mutation tools (audit 2026-09-11).
//!
//! Several `bonsai_core` write primitives are documented as trusting their
//! caller, because their caller was the Bonsai UI: the UI only offers paths it
//! already listed in `StatusSnapshot`, and it gates its conflict-editor Save
//! button on `hasUnresolvedMarkers`. **A model is not the UI.** These guards
//! re-establish exactly those preconditions on the MCP path — deliberately HERE
//! and not in the shared primitive, so UI behaviour (which legitimately relies
//! on the current semantics) is untouched.
//!
//! Everything is expressed as a PURE decision function over already-fetched data
//! plus a thin blocking wrapper that fetches it, so the decisions are unit
//! testable without a scratch repo (`write_guards::tests`). The wrappers are
//! blocking (git2 + fs) and are called INSIDE the tool bodies' `run_blocking`
//! closure — same worker thread, same open-repo moment as the mutation they
//! guard, so there is no extra `spawn_blocking` and no widened TOCTOU window.

use std::collections::HashSet;
use std::path::Path;

use bonsai_core::error::AppError;
use bonsai_core::git::status::StatusSnapshot;

// ---------------------------------------------------------------------------
// bonsai_stage: every path must appear in `bonsai_get_status` output
// (audit MEDIUM). `index.add_path` has `git add -f` semantics, so without this
// a model could stage — and then commit — a gitignored `.env` / `secrets.json`
// that no status list ever offered. This is also exactly what the tool
// description promises, so the guard makes the contract true rather than
// adding a new rule.
// ---------------------------------------------------------------------------

/// Every path the snapshot offers as stageable: the `path` of every entry in all
/// four lists PLUS each entry's `orig_path` (staging the OLD side of a rename is
/// legitimate — it is how a rename's deletion half gets staged).
///
/// `read_status` uses `recurse_untracked_dirs(true)`, so untracked rows are
/// individual files (never a `newdir/` summary row) and exact matching is right;
/// it uses `include_ignored(false)`, which is what makes an ignored path absent
/// here and therefore refused.
fn stageable_paths(snap: &StatusSnapshot) -> HashSet<&str> {
    let lists = [
        &snap.staged,
        &snap.unstaged,
        &snap.untracked,
        &snap.conflicted,
    ];
    let mut out = HashSet::new();
    for list in lists {
        for entry in list.iter() {
            out.insert(entry.path.as_str());
            if let Some(orig) = entry.orig_path.as_deref() {
                out.insert(orig);
            }
        }
    }
    out
}

/// PURE. Refuse the FIRST path absent from `snap` (`invalidName`), naming it.
/// All-or-nothing: the caller runs this before `stage_paths`, so one bad path
/// stages nothing — matching the tool's documented atomicity.
pub(crate) fn ensure_paths_in_snapshot(
    snap: &StatusSnapshot,
    paths: &[String],
) -> Result<(), AppError> {
    let allowed = stageable_paths(snap);
    for p in paths {
        if !allowed.contains(p.as_str()) {
            return Err(AppError::InvalidName(format!(
                "path '{p}' is not in bonsai_get_status output; only paths it reports as \
                 staged/unstaged/untracked/conflicted can be staged (ignored and \
                 unchanged files cannot). Nothing was staged."
            )));
        }
    }
    Ok(())
}

/// PURE. The subset of `paths` that status reports as CONFLICTED (either side of
/// a rename). Staging such a path is `markResolved` by another name, so it gets
/// the same marker gate — see [`ensure_paths_in_status`].
fn conflicted_subset<'a>(snap: &StatusSnapshot, paths: &'a [String]) -> Vec<&'a str> {
    paths
        .iter()
        .map(String::as_str)
        .filter(|p| {
            snap.conflicted
                .iter()
                .any(|e| e.path == *p || e.orig_path.as_deref() == Some(*p))
        })
        .collect()
}

/// Blocking. Read status once, then apply [`ensure_paths_in_snapshot`] — and, for
/// any path status reports as conflicted, the marker gate below. An empty batch
/// skips the read entirely (`stage_paths` is a no-op for it).
///
/// The conflicted case is one site beyond the audit's list, and closing it is
/// what makes the audit's fix real: `bonsai_stage` on a conflicted path is
/// `index.add_path` on the worktree file exactly as it stands — i.e. the
/// `markResolved` operation, reached through a different tool. Guarding only
/// `bonsai_resolve_conflict{,_text}` would leave `bonsai_stage(["c.txt"])`
/// followed by `bonsai_commit` as a one-call bypass. Capability is preserved
/// (a genuinely cleaned conflicted file still stages); only marker text is
/// refused. Runs before `stage_paths`, so the batch stays all-or-nothing.
pub(crate) fn ensure_paths_in_status(workdir: &Path, paths: &[String]) -> Result<(), AppError> {
    if paths.is_empty() {
        return Ok(());
    }
    let snap = bonsai_core::git::status::read_status(workdir)?;
    ensure_paths_in_snapshot(&snap, paths)?;
    for path in conflicted_subset(&snap, paths) {
        ensure_worktree_has_no_markers(workdir, path)?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Conflict resolution: no leftover conflict markers (audit LOW).
// `resolve_conflict_text` and `resolve_conflict(markResolved)` both document
// "Trust the caller" and point at the frontend's `hasUnresolvedMarkers` Save
// gate. There is no such gate over MCP, so a model could stage — and commit — a
// file that still contains `<<<<<<<`.
//
// The predicate used here is `bonsai_core::git::conflict::has_conflict_markers`,
// the SAME function the core AI path uses — so the two Rust gates cannot drift.
// The frontend's gate is an INDEPENDENT TypeScript implementation of the same
// rule (`src/utils/conflictRegions.ts`: `/^(<{7}|={7}|>{7})/`), which Rust
// cannot call; it is semantically equivalent today and kept so by symmetric
// known-answer tests on both sides, not by sharing code (review 2026-09-11
// corrected an earlier claim of a shared definition here).
// ---------------------------------------------------------------------------

/// PURE. Refuse `text` if it still holds a conflict-marker line
/// (`unresolvedConflicts` — the kind the model already knows from
/// `bonsai_commit_merge` / `bonsai_rebase_continue`).
pub(crate) fn ensure_no_conflict_markers(path: &str, text: &str) -> Result<(), AppError> {
    if bonsai_core::git::conflict::has_conflict_markers(text) {
        return Err(AppError::UnresolvedConflicts(format!(
            "'{path}' still contains conflict markers (<<<<<<< / ======= / >>>>>>>); \
             a resolution must be the final merged text with the markers removed. \
             Nothing was written or staged."
        )));
    }
    Ok(())
}

/// Blocking. The `markResolved` variant stages the worktree file AS IT STANDS,
/// so the marker check has to read that file: `get_conflict` is the safe,
/// public way (it re-validates the path and reports the worktree text).
///
/// Residual, stated rather than hidden: a `tooLarge` conflict returns
/// `text: ""`, so markers cannot be inspected. We REFUSE that case over MCP
/// instead of waving it through — a model cannot have authored an informed
/// `markResolved` for a file it was never able to read.
///
/// `binary` and `missing` also carry no text and DO pass. Precisely (review
/// 2026-09-11 — an earlier version of this note claimed markers were
/// "impossible" there, which overstates it): `missing` is a
/// resolved-by-deletion path with no worktree file to hold markers, while
/// `binary` means `get_conflict` found a NUL byte in the first 8000 bytes and
/// stopped reading — so a file that is binary by that test is waved through
/// unchecked. It would have to embed a NUL early AND carry marker lines, and
/// `index.add_path` would stage those bytes exactly as they are; the impact is
/// nil because such a file is not a text merge a model could have resolved.
pub(crate) fn ensure_worktree_has_no_markers(workdir: &Path, path: &str) -> Result<(), AppError> {
    let view = bonsai_core::git::conflict::get_conflict(workdir, path)?;
    if view.too_large {
        return Err(AppError::UnresolvedConflicts(format!(
            "'{path}' is too large for Bonsai to read, so its conflict markers cannot be \
             checked; resolve it in an editor and stage it from the Bonsai app instead. \
             Nothing was staged."
        )));
    }
    ensure_no_conflict_markers(path, &view.text)
}

// ---------------------------------------------------------------------------
// Commit-PRODUCING tools: repository hooks must not run undisclosed (audit
// LOW). All THREE of them — `bonsai_commit`, `bonsai_commit_merge` and
// `bonsai_merge_branch`'s clean auto-merge (review 2026-09-11: that third one
// ran `commit-msg` with no gate while the instructions said otherwise).
// `bonsai.runHooks` defaults true, and the disclosure gate lives in the
// FRONTEND (`src-tauri/src/commands/hooks.rs`: "The gate itself lives in the
// frontend (useHookDisclosure)"). A standalone `bonsai-mcp --repo X
// --allow-write` has no frontend, so a repo whose hooks the user was never
// shown would execute code on the agent's commit. Refuse there; `--allow-hooks`
// is the explicit consent. The EMBEDDED server is untouched: its repos are open
// Bonsai tabs, where the app's disclosure has already run.
// ---------------------------------------------------------------------------

/// PURE. Given the commit hooks that WOULD run (from
/// [`bonsai_core::git::hooks::commit_hooks_that_would_run`]), refuse when there
/// is at least one, naming them and both ways forward.
///
/// `hooksNotPermitted`, not `other` (review 2026-09-11): the caller is a model
/// branching on the typed kind, and a refusal indistinguishable from a generic
/// failure invites a blind retry loop. The kind says "no hook ran, nothing
/// changed, get consent or disable hooks" — unlike `hookRejected`, where a hook
/// did run and vetoed the operation.
pub(crate) fn hooks_refusal(hooks: &[&str]) -> Result<(), AppError> {
    refusal(
        hooks,
        "on the commit",
        "so the commit was refused and nothing was committed",
    )
}

/// [`hooks_refusal`] for the clean auto-merge commit, whose hook set is
/// `commit-msg` alone — see [`merge_hooks_gate`]. Separate wording because the
/// operation is a merge, not a commit, and because naming the commit hooks here
/// would name hooks this path never fires.
pub(crate) fn merge_hooks_refusal(hooks: &[&str]) -> Result<(), AppError> {
    refusal(
        hooks,
        "to commit the merge",
        "so the merge was refused; nothing was merged, committed or stashed and the \
         repository is untouched",
    )
}

/// Shared body of the two refusals above: `Ok` for an empty hook set, else a
/// `hooksNotPermitted` naming the hooks, what was NOT done, and both ways
/// forward.
fn refusal(hooks: &[&str], when: &str, consequence: &str) -> Result<(), AppError> {
    if hooks.is_empty() {
        return Ok(());
    }
    Err(AppError::HooksNotPermitted(format!(
        "this repository would run git hooks {when} ({}), which is arbitrary code from the \
         repository. bonsai-mcp has no way to disclose that to the user, {consequence}. Ask \
         the user to restart the server with --allow-hooks to permit them, or to set \
         `git config bonsai.runHooks false` in this repository to proceed without running \
         them.",
        hooks.join(", ")
    )))
}

/// Blocking. [`hooks_refusal`] over the repo's actual commit-time hooks
/// (present + executable + `bonsai.runHooks` enabled).
pub(crate) fn ensure_commit_hooks_disclosed(workdir: &Path) -> Result<(), AppError> {
    hooks_refusal(&bonsai_core::git::hooks::commit_hooks_that_would_run(
        workdir,
    ))
}

/// The [`MergeHookGate`](bonsai_core::git::merge::MergeHookGate) for
/// `bonsai_merge_branch` (review 2026-09-11 MUST-FIX): a clean auto-merge runs
/// the repository's `commit-msg` hook, which used to happen here with NO gate
/// at all while the server's own instructions claimed commits were refused.
///
/// The gate cannot be applied BEFORE the call the way `bonsai_commit`'s is:
/// fast-forward and up-to-date merges return before any hook is selected, so a
/// pre-call refusal would refuse merges that execute nothing, and the commit
/// probe would name `pre-commit` / `post-commit`, which a merge never fires.
/// Instead core consults this function at the one point that knows the merge is
/// a real (non-FF) merge and before anything has mutated — see
/// `merge_branch_gated`'s doc for the exact placement and its one stated
/// residual (a would-conflict merge is refused too, because concluding it needs
/// `commit_merge`, whose hook set is a superset).
///
/// Plain `MergeHookGate::Run` when this deployment MAY run hooks
/// (`--allow-hooks`, or the embedded server): no probe, no refusal, and the
/// merge behaves exactly as it did before this guard existed.
pub(crate) fn merge_hooks_gate(need_disclosure: bool) -> bonsai_core::git::merge::MergeHookGate {
    use bonsai_core::git::merge::MergeHookGate;
    if need_disclosure {
        MergeHookGate::Gate(merge_hooks_refusal)
    } else {
        MergeHookGate::Run
    }
}

#[cfg(test)]
mod tests;
