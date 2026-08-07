//! Read-only GROUNDING payload for the NL-operation planner (P55 contract §7).
//!
//! Split out of `ai_operation.rs` (file-size discipline, P55b). Assembles the
//! `USER REQUEST` + `REPO STATE` block that is fed to the model on STDIN (never
//! argv) from existing read fns (`list_refs`, `read_status`, `list_stashes`,
//! `read_op_state`) plus a first-parent HEAD revwalk. Even if any field embeds
//! adversarial text, the safety model (L1–L7) holds: it can only nudge the
//! model toward some ALLOWLISTED intent, which Rust re-validates + previews +
//! confirm-gates. Mutates nothing.

use std::fmt::Write as _;
use std::path::Path;

use crate::error::AppError;
use crate::git::ai_operation::{current_branch_name, head_commit, short7, summary_of};
use crate::git::branches::list_refs;
use crate::git::opstate::{read_op_state, RepoOpState};
use crate::git::stash::list_stashes;
use crate::git::status::read_status;
use crate::git::timefmt::epoch_to_ymd;

/// First-parent HEAD commits sampled into the grounding (mirrors `ai_summary`).
const RECENT_COMMITS: usize = 25;

/// Cap on `CHANGED PATHS` listed in the grounding (rest collapse to a count).
const GROUNDING_MAX_PATHS: usize = 50;

/// Assembles the read-only grounding payload (§7) from existing read fns + a
/// first-parent HEAD revwalk. stdin ONLY (multi-line) — never argv.
pub(crate) fn build_grounding(
    repo: &git2::Repository,
    workdir: &Path,
    request: &str,
) -> Result<String, AppError> {
    let mut s = String::new();
    let _ = writeln!(s, "USER REQUEST:\n{}\n", request.trim());
    let _ = writeln!(s, "REPO STATE:");

    // HEAD line.
    match head_commit(repo)? {
        Some(head) => {
            let detached = repo.head_detached().unwrap_or(false);
            let label = if detached {
                "detached".to_string()
            } else {
                current_branch_name(repo)
            };
            let merge = if head.parent_count() >= 2 { "yes" } else { "no" };
            let _ = writeln!(
                s,
                "HEAD: {label} at {} \"{}\"  (merge commit: {merge})",
                short7(head.id()),
                summary_of(&head)
            );
        }
        None => {
            let _ = writeln!(s, "HEAD: (unborn — no commits yet)");
        }
    }

    // Refs snapshot (upstream + branch lists) via the existing read fn.
    let refs = list_refs(workdir)?;
    let upstream = refs.local.iter().find(|b| b.is_head).and_then(|b| {
        b.upstream.as_ref().map(|u| match (b.ahead, b.behind) {
            (Some(a), Some(bh)) => format!("{u}, ahead {a} behind {bh}"),
            _ => u.clone(),
        })
    });
    let _ = writeln!(s, "UPSTREAM: {}", upstream.unwrap_or_else(|| "none".to_string()));

    // Recent commits (first-parent, newest first).
    let _ = writeln!(s, "RECENT COMMITS (first-parent, newest first):");
    if repo.head().is_ok() {
        if let Ok(mut walk) = repo.revwalk() {
            let _ = walk.set_sorting(git2::Sort::TOPOLOGICAL);
            let _ = walk.simplify_first_parent();
            if walk.push_head().is_ok() {
                for oid in walk.take(RECENT_COMMITS) {
                    let oid = match oid {
                        Ok(o) => o,
                        Err(_) => break,
                    };
                    if let Ok(c) = repo.find_commit(oid) {
                        let date = epoch_to_ymd(c.time().seconds());
                        let author = String::from_utf8_lossy(c.author().name_bytes()).into_owned();
                        let merge = if c.parent_count() >= 2 { "  [merge]" } else { "" };
                        let _ = writeln!(
                            s,
                            "- {} {date} {author}  {}{merge}",
                            short7(oid),
                            summary_of(&c)
                        );
                    }
                }
            }
        }
    }

    // Branch lists.
    let locals: Vec<&str> = refs.local.iter().map(|b| b.name.as_str()).collect();
    let _ = writeln!(
        s,
        "LOCAL BRANCHES: {}",
        if locals.is_empty() { "(none)".to_string() } else { locals.join(", ") }
    );
    let remotes: Vec<&str> = refs.remote.iter().map(|b| b.name.as_str()).collect();
    let _ = writeln!(
        s,
        "REMOTE BRANCHES: {}",
        if remotes.is_empty() { "(none)".to_string() } else { remotes.join(", ") }
    );

    // Working tree + changed (tracked-modified) paths.
    let status = read_status(workdir)?;
    if status.staged.is_empty()
        && status.unstaged.is_empty()
        && status.untracked.is_empty()
        && status.conflicted.is_empty()
    {
        let _ = writeln!(s, "WORKING TREE: clean");
    } else {
        let _ = writeln!(
            s,
            "WORKING TREE: {} staged, {} unstaged, {} untracked",
            status.staged.len(),
            status.unstaged.len(),
            status.untracked.len()
        );
    }
    let mut changed: Vec<String> = Vec::new();
    for e in status.staged.iter().chain(status.unstaged.iter()) {
        if !changed.contains(&e.path) {
            changed.push(e.path.clone());
        }
    }
    if !changed.is_empty() {
        let shown: Vec<&str> = changed
            .iter()
            .take(GROUNDING_MAX_PATHS)
            .map(String::as_str)
            .collect();
        let more = changed.len().saturating_sub(shown.len());
        let more_note = if more > 0 {
            format!(" (+{more} more)")
        } else {
            String::new()
        };
        let _ = writeln!(s, "CHANGED PATHS: {}{}", shown.join(", "), more_note);
    }

    // Stashes.
    let stashes = list_stashes(workdir)?;
    if stashes.is_empty() {
        let _ = writeln!(s, "STASHES: none");
    } else {
        let items: Vec<String> = stashes
            .iter()
            .take(10)
            .map(|e| format!("[{}] \"{}\"", e.index, e.message))
            .collect();
        let _ = writeln!(s, "STASHES: {}", items.join(", "));
    }

    // In-progress op.
    let op = match read_op_state(workdir)? {
        RepoOpState::None => "none",
        RepoOpState::Merge { .. } => "merge",
        RepoOpState::Rebase { .. } => "rebase",
        RepoOpState::CherryPick => "cherryPick",
        RepoOpState::Revert => "revert",
        RepoOpState::Bisect { .. } => "bisect",
    };
    let _ = writeln!(s, "IN-PROGRESS OP: {op}");

    Ok(s)
}
