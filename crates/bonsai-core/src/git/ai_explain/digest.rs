//! P28 "what changed" digest — range resolution + the blocking `digest_changes`
//! entry point. Split out of `ai_explain.rs`; behavior unchanged. Items are
//! re-exported from the `ai_explain` module root so existing paths resolve.

use std::path::Path;

use crate::ai::payload;
use crate::ai::{self, RunOpts};
use crate::error::AppError;
use crate::git::diff::{apply_find_similar, build_diff_options, collect_file_diffs};
use crate::git::stage::open_workdir_repo;
use crate::git::timefmt::epoch_to_ymd;

use super::{
    cap_review_payload, has_analyzable_content, AiAnalysis, AiDigestRange, DIGEST_PROMPT,
    DIGEST_SYSTEM_PROMPT, MAX_DIGEST_COMMITS, MAX_DIGEST_WALK_COMMITS,
};

/// One digest metadata line: `- {short7} {YYYY-MM-DD} {author_name}  {subject}`
/// (P28 §4.3). Lossy UTF-8; date from `commit.time()` (UTC).
pub(crate) fn commit_meta_line(commit: &git2::Commit<'_>) -> String {
    let short7: String = commit.id().to_string().chars().take(7).collect();
    let date = epoch_to_ymd(commit.time().seconds());
    let author = commit.author();
    let name = String::from_utf8_lossy(author.name_bytes()).into_owned();
    let subject = String::from_utf8_lossy(commit.summary_bytes().unwrap_or(b"")).into_owned();
    format!("- {short7} {date} {name}  {subject}")
}

/// Joins metadata lines newest-first, capping at [`MAX_DIGEST_COMMITS`] lines
/// and collapsing the overflow to `... and N more commits` (P28 Decision #5).
/// `pub(crate)` so sibling AI modules (P56 changelog) reuse the digest range
/// helpers rather than duplicating the walk (D2/OQ6).
pub(crate) fn format_commit_meta(lines: &[String]) -> String {
    if lines.len() <= MAX_DIGEST_COMMITS {
        return lines.join("\n");
    }
    let mut out = lines[..MAX_DIGEST_COMMITS].join("\n");
    out.push_str(&format!(
        "\n... and {} more commits",
        lines.len() - MAX_DIGEST_COMMITS
    ));
    out
}

/// Resolves a digest range to `(header_note, commits_newest_first, old_tree,
/// new_tree)` per P28 §3. `old_tree == None` means "empty tree" (unrelated
/// histories, or a lastDays window covering the whole history). `pub(crate)` so
/// P56's `ai_changelog` reuses this single range resolver (D2/OQ6) rather than
/// re-walking raw objects.
pub(crate) fn resolve_digest_range<'r>(
    repo: &'r git2::Repository,
    range: &AiDigestRange,
) -> Result<
    (
        String,
        Vec<git2::Commit<'r>>,
        Option<git2::Tree<'r>>,
        git2::Tree<'r>,
    ),
    AppError,
> {
    match range {
        // SinceCommit is pure sugar for BetweenRefs{from: oid, to: "HEAD"} (Decision #2).
        AiDigestRange::SinceCommit { oid } => resolve_digest_range(
            repo,
            &AiDigestRange::BetweenRefs {
                from: oid.clone(),
                to: "HEAD".to_string(),
            },
        ),
        AiDigestRange::BetweenRefs { from, to } => {
            let from_c = repo.revparse_single(from)?.peel_to_commit()?;
            let to_c = repo.revparse_single(to)?.peel_to_commit()?;

            let mb = repo.merge_base(from_c.id(), to_c.id()).ok();
            let unrelated = mb.is_none();

            let mut walk = repo.revwalk()?;
            walk.set_sorting(git2::Sort::TOPOLOGICAL | git2::Sort::TIME)?;
            walk.push(to_c.id())?;
            if let Some(oid) = mb {
                walk.hide(oid)?;
            }
            // Bounded collection (audit §3.15, mirrors ai_branch_name's
            // collect-≤cap pattern): stop walking past the cap instead of
            // materializing an arbitrarily large range in memory.
            let mut commits = Vec::new();
            let mut walk_truncated = false;
            for oid in walk {
                if commits.len() >= MAX_DIGEST_WALK_COMMITS {
                    walk_truncated = true;
                    break;
                }
                commits.push(repo.find_commit(oid?)?);
            }

            let old_tree = match mb {
                Some(oid) => Some(repo.find_commit(oid)?.tree()?),
                None => None,
            };
            let new_tree = to_c.tree()?;

            let mut header = format!(
                "RANGE {from}..{to} ({}{} commits)",
                commits.len(),
                if walk_truncated { "+" } else { "" }
            );
            if walk_truncated {
                header.push_str(&format!(
                    "\n(+ more commits — list truncated at the {MAX_DIGEST_WALK_COMMITS} \
                     newest; the diff still spans the whole range)"
                ));
            }
            if unrelated {
                header.push_str(
                    "\nNOTE: this branch and its base have no common ancestor (unrelated \
                     histories); the diff is the full branch contents versus an empty base.",
                );
            }
            Ok((header, commits, old_tree, new_tree))
        }
        AiDigestRange::LastDays { days } => {
            if *days == 0 {
                return Err(AppError::InvalidName("days must be >= 1".to_string()));
            }
            let days = (*days).min(3650); // clamp, no error (P28 §3)
            let head = repo.head()?.peel_to_commit()?;
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs() as i64)
                .unwrap_or(0);
            let cutoff = now - i64::from(days) * 86_400;

            // First-parent walk, newest first; stop at the first commit older
            // than the cutoff — that commit's tree anchors the range diff.
            // First-parent order is monotone enough; a single stale-dated commit
            // INSIDE the window is an accepted edge case (P28 §3 step 4).
            let mut walk = repo.revwalk()?;
            walk.set_sorting(git2::Sort::TOPOLOGICAL)?;
            walk.simplify_first_parent()?;
            walk.push(head.id())?;
            let mut commits = Vec::new();
            let mut boundary: Option<git2::Commit<'r>> = None;
            let mut walk_truncated = false;
            for oid in walk {
                let commit = repo.find_commit(oid?)?;
                if commit.time().seconds() >= cutoff {
                    // Bounded collection (audit §3.15): past the cap the
                    // current in-window commit becomes the boundary, so the
                    // diff anchors exactly to the collected newest commits.
                    if commits.len() >= MAX_DIGEST_WALK_COMMITS {
                        walk_truncated = true;
                        boundary = Some(commit);
                        break;
                    }
                    commits.push(commit);
                } else {
                    boundary = Some(commit);
                    break;
                }
            }

            let old_tree = match &boundary {
                Some(b) => Some(b.tree()?),
                None => None, // whole history in the window → diff vs empty tree
            };
            let new_tree = head.tree()?;

            let branch = repo
                .head()?
                .shorthand()
                .ok()
                .map(str::to_string)
                .unwrap_or_else(|| "HEAD (detached)".to_string());
            let mut header = format!(
                "RANGE last {days} day(s) on {branch} ({}{} commits)",
                commits.len(),
                if walk_truncated { "+" } else { "" }
            );
            if walk_truncated {
                header.push_str(&format!(
                    "\n(+ more commits — list truncated at the {MAX_DIGEST_WALK_COMMITS} \
                     newest; the diff covers exactly these commits)"
                ));
            }
            Ok((header, commits, old_tree, new_tree))
        }
    }
}

/// Blocking. Resolves the range, gathers commit metadata + the range diff,
/// renders the payload, and asks the CLI for a digest (P28 §4). Read-only;
/// WRITES NOTHING. Errors: `aiFailed` (empty range / CLI failure) | `git`
/// (bad ref, unborn HEAD for HEAD-anchored ranges) | `invalidName` (days == 0)
/// | (`aiUnavailable` via the command-layer gate).
pub fn digest_changes(
    workdir: &Path,
    range: AiDigestRange,
    opts: RunOpts,
) -> Result<AiAnalysis, AppError> {
    let repo = open_workdir_repo(workdir)?;
    let (header_note, commits, old_tree, new_tree) = resolve_digest_range(&repo, &range)?;

    // Range diff: exactly the gather_branch pipeline (P28 §3).
    let mut opts_diff = build_diff_options(&[], false);
    let mut diff = repo.diff_tree_to_tree(old_tree.as_ref(), Some(&new_tree), Some(&mut opts_diff))?;
    apply_find_similar(&mut diff)?;
    let files = collect_file_diffs(&diff)?;

    // Empty range (no commits AND no diff content) → no CLI call (Decision #7).
    // Commits with an empty diff (e.g. a revert pair) still digest: the metadata
    // alone is a valid narrative.
    if commits.is_empty() && !has_analyzable_content(&files) {
        return Err(AppError::AiFailed(
            "no changes in the selected range".to_string(),
        ));
    }

    // Metadata header (newest first, capped), then the rendered diff, then ONE
    // combined byte-cap over the whole string — truncation only eats diff tail.
    let meta_lines: Vec<String> = commits.iter().map(commit_meta_line).collect();
    let meta = format_commit_meta(&meta_lines);
    let rendered = payload::render_file_diffs(&files);
    let payload_text = cap_review_payload(format!(
        "{header_note}\n\nCOMMITS\n{meta}\n\nDIFF\n{}",
        rendered.text
    ));

    let result = ai::run_claude(
        workdir,
        DIGEST_PROMPT,
        Some(&payload_text),
        RunOpts {
            system_prompt: Some(DIGEST_SYSTEM_PROMPT.to_string()),
            ..opts
        },
    )?;

    Ok(AiAnalysis {
        text: result.text,
        cost_usd: result.cost_usd,
    })
}
