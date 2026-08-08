//! AI explain/review of typed diff data. `analyze_diff` selects a diff source
//! (a commit, a working-dir file, or the whole staged set), renders a payload,
//! and asks the CLI to either EXPLAIN (plain English) or REVIEW (risks/bugs/
//! style) it. Read-only prose out; WRITES NOTHING. Pure git2 + crate::ai. (P15)

use std::path::Path;

use crate::ai::payload;
use crate::ai::{self, RunOpts};
use crate::error::AppError;
use crate::git::diff::{
    apply_find_similar, build_diff_options, collect_file_diffs, commit_diff, commit_file_diff,
    head_tree, workdir_file_diff, FileDiff, LineKind,
};
use crate::git::stage::{open_workdir_repo, validate_rel_path};
use crate::git::status::read_status;
use crate::git::timefmt::epoch_to_ymd;

/// Hard byte-cap on the assembled review payload (P25 §2.4). Belt-and-suspenders
/// for pathological long-line `Worktree`/`Branch` diffs; the small
/// commit/file/staged targets never trip it. `run_claude` streams arbitrarily
/// large stdin without deadlock, but an unbounded payload wastes tokens/latency.
pub const MAX_REVIEW_PAYLOAD_BYTES: usize = 256 * 1024;

/// Model-visible marker appended when a payload is truncated at the byte-cap.
const TRUNCATION_NOTE: &str = "\n... (payload truncated at 256 KiB for review) ...\n";

/// System prompt (via `--append-system-prompt`) for EXPLAIN mode (contract
/// §4.2, verbatim). SINGLE line — on Windows the `claude` CLI is a `.cmd` shim
/// and Rust's `Command` REFUSES an argv arg containing a newline. Multi-line
/// content only ever flows through the stdin payload. (P15)
const EXPLAIN_SYSTEM_PROMPT: &str = "You are a senior engineer explaining a code change to a teammate. Given a diff on standard input, explain in clear plain English what the change does and, where inferable, why — a one or two sentence high-level summary first, then the key specifics grouped by file. Be concise and concrete. Output prose only — no markdown code fences.";

/// System prompt for REVIEW mode (contract §4.2, verbatim single line). (P15)
const REVIEW_SYSTEM_PROMPT: &str = "You are a meticulous senior code reviewer. Given a diff on standard input, review it for likely bugs, correctness and edge-case risks, security issues, and notable style or maintainability problems. Be concise and specific and cite file names. If you find nothing significant, say so briefly. Output prose only — no markdown code fences.";

/// The `-p` positional prompt for EXPLAIN mode (contract §4.2, verbatim). (P15)
const EXPLAIN_PROMPT: &str = "Explain the change provided on standard input.";

/// The `-p` positional prompt for REVIEW mode (contract §4.2, verbatim). (P15)
const REVIEW_PROMPT: &str = "Review the change provided on standard input.";

/// Max commits listed in the digest metadata header (P28 Decision #5). Lines
/// beyond the cap collapse to "... and N more commits"; the diff still spans
/// the WHOLE range (byte-capped separately by `cap_review_payload`).
pub const MAX_DIGEST_COMMITS: usize = 200;

/// System prompt for the "what changed" digest (P28 §2, verbatim). SINGLE line
/// — Windows `claude.cmd` argv constraint, same rule as the P15 prompts.
const DIGEST_SYSTEM_PROMPT: &str = "You are a senior engineer writing a change digest for a teammate returning to a repository. Standard input contains a commit list (short hash, date, author, subject) followed by the corresponding combined diff. Write a clear plain-English digest of what changed over this range: a two or three sentence executive summary first, then the main themes or workstreams as short groups, citing file or area names and mentioning authors when several people contributed. Prefer narrative over per-commit listing; skip trivial churn. Output prose only — no markdown code fences.";

/// The `-p` positional prompt for the digest (P28 §2, verbatim single line).
const DIGEST_PROMPT: &str = "Summarize what changed in the range provided on standard input.";

/// Which diff to analyze. `#[serde(tag="kind", rename_all="camelCase")]` — this
/// is a COMMAND INPUT (Deserialize); TS mirror is a discriminated union (§5).
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum AiDiffTarget {
    /// Commit vs its first parent (root => vs empty tree). `oid` = 40-hex.
    Commit { oid: String },
    /// One working-dir file. `staged=false` => index vs workdir; `staged=true`
    /// => HEAD vs index. `orig_path` for renames.
    WorkdirFile {
        path: String,
        // The enum's `rename_all = "camelCase"` renames VARIANTS, not
        // struct-variant FIELDS, so name the wire key explicitly to match the
        // TS union (§6.1 sends `origPath`). `default` accepts a missing key too.
        #[serde(default, rename = "origPath")]
        orig_path: Option<String>,
        staged: bool,
    },
    /// The whole staged set (HEAD tree vs index) — the natural Review target.
    Staged,
    /// The whole working-tree change set: HEAD tree vs working directory,
    /// index-aware, including untracked additions. The natural pre-commit
    /// Review target (P25 B1).
    Worktree,
    /// A branch (or any ref/oid) vs the merge-base with `base`. `base=None`
    /// => auto-resolve (§2.3). The natural pre-push Review target (P25 B1).
    Branch {
        name: String,
        #[serde(default)]
        base: Option<String>,
    },
}

/// Which range to digest (P28 §2). COMMAND INPUT (Deserialize); the TS mirror
/// is a discriminated union (§5.2).
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum AiDigestRange {
    /// Commits in `to` but not `from` (merge-base range, `from...to` narrative).
    /// Both accept any revparse-able ref/oid (branch, remote-tracking, tag, hex).
    BetweenRefs { from: String, to: String },
    /// First-parent commits on the current branch (HEAD) with committer time
    /// within the last `days` days. days >= 1 (0 => InvalidName).
    LastDays { days: u32 },
    /// Commits in HEAD but not `oid` — sugar for BetweenRefs{from: oid, to: "HEAD"}.
    SinceCommit { oid: String },
}

/// Explain (teammate-friendly summary) vs Review (risks/bugs/style).
#[derive(Debug, Clone, Copy, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AiAnalysisMode {
    Explain,
    Review,
}

/// Prose result. Serialized camelCase (mirrored in TS).
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiAnalysis {
    pub text: String,
    pub cost_usd: Option<f64>,
}

/// True when the gathered diffs carry anything worth analyzing: any add/del
/// line, or a binary/too-large placeholder (which still describes a real
/// change). Contract §4.1 says "zero add/del lines => AiFailed"; we extend it so
/// a binary-only change — which produces a non-empty payload placeholder but no
/// textual add/del lines — is NOT misreported as "no changes to analyze".
/// `pub(crate)` so `ai_branch_name` reuses the same "is there anything to work
/// with" gate over a gathered change set (P53c) rather than duplicating it.
pub(crate) fn has_analyzable_content(files: &[FileDiff]) -> bool {
    files.iter().any(|f| {
        f.binary
            || f.too_large
            || f.hunks.iter().any(|h| {
                h.lines
                    .iter()
                    .any(|l| matches!(l.kind, LineKind::Add | LineKind::Del))
            })
    })
}

/// Gathers the staged file diffs (HEAD tree vs index), mirroring P15a §3.1
/// steps 1–2 without depending on `ai_commit.rs` internals. An empty staged set
/// (index matches HEAD) => `NothingToCommit` (§7.1). Kept tiny + private.
fn gather_staged(workdir: &Path) -> Result<Vec<FileDiff>, AppError> {
    let staged = read_status(workdir)?.staged;
    if staged.is_empty() {
        return Err(AppError::NothingToCommit);
    }
    let mut file_diffs = Vec::with_capacity(staged.len());
    for entry in &staged {
        let fd = workdir_file_diff(workdir, &entry.path, entry.orig_path.as_deref(), true, false, false)?;
        file_diffs.push(fd);
    }
    Ok(file_diffs)
}

/// Gathers the ENTIRE working-tree change set (P25 §2.2): HEAD tree vs the
/// working directory, index-aware, including untracked additions — the single
/// "everything since my last commit" diff in one pass. Unborn HEAD => diff vs
/// the empty tree (all Added). Empty diff => empty Vec (=> `AiFailed` in
/// `analyze_diff`). `pub(crate)` so `ai_branch_name` reuses the SAME index-aware
/// worktree gather for the `Working` naming source (P53c) — no duplication.
pub(crate) fn gather_worktree(workdir: &Path) -> Result<Vec<FileDiff>, AppError> {
    let repo = open_workdir_repo(workdir)?;
    let head = head_tree(&repo)?;
    let mut opts = build_diff_options(&[], false);
    opts.include_untracked(true).recurse_untracked_dirs(true);
    let mut diff = repo.diff_tree_to_workdir_with_index(head.as_ref(), Some(&mut opts))?;
    apply_find_similar(&mut diff)?;
    collect_file_diffs(&diff)
}

/// Resolves the comparison base for a branch review (P25 §2.3). Returns
/// `(shorthand, commit)`. Precedence: explicit `base` (revparse) → the branch's
/// configured upstream (only when `name` is a local branch) → `origin/HEAD`
/// target → local `main` → local `master` → `Git` error.
fn resolve_branch_base<'r>(
    repo: &'r git2::Repository,
    name: &str,
    base: Option<&str>,
) -> Result<(String, git2::Commit<'r>), AppError> {
    // 1. Explicit base wins (any ref/oid).
    if let Some(b) = base {
        let commit = repo.revparse_single(b)?.peel_to_commit()?;
        return Ok((b.to_string(), commit));
    }

    // 2. The branch's configured upstream — only when `name` is a local branch.
    if let Ok(branch) = repo.find_branch(name, git2::BranchType::Local) {
        if let Ok(up) = branch.upstream() {
            let shorthand = up
                .name()
                .ok()
                .flatten()
                .map(str::to_string)
                .unwrap_or_else(|| "upstream".to_string());
            if let Ok(commit) = up.get().peel_to_commit() {
                return Ok((shorthand, commit));
            }
        }
    }

    // 3. origin/HEAD's target (e.g. `origin/main`).
    if let Ok(r) = repo.find_reference("refs/remotes/origin/HEAD") {
        if let Ok(resolved) = r.resolve() {
            if let Ok(commit) = resolved.peel_to_commit() {
                let shorthand = resolved.shorthand().unwrap_or("origin/HEAD").to_string();
                return Ok((shorthand, commit));
            }
        }
    }

    // 4. Local `main`, then `master`.
    for candidate in ["main", "master"] {
        if let Ok(branch) = repo.find_branch(candidate, git2::BranchType::Local) {
            if let Ok(commit) = branch.get().peel_to_commit() {
                return Ok((candidate.to_string(), commit));
            }
        }
    }

    Err(AppError::Git(
        "cannot determine a base branch to review against; specify one explicitly".to_string(),
    ))
}

/// Gathers a branch's diff vs its merge-base with `base` (P25 §2.2). `name` is
/// any ref/oid (`revparse_single`); `base=None` auto-resolves (§2.3). No merge
/// base (unrelated histories) => diff vs the empty tree with a noted prefix
/// (mirrors `ai_summary`). Returns `(prefix, files)`. Empty diff => empty Vec.
fn gather_branch(
    workdir: &Path,
    name: &str,
    base: Option<&str>,
) -> Result<(String, Vec<FileDiff>), AppError> {
    let repo = open_workdir_repo(workdir)?;
    let head = repo.revparse_single(name)?.peel_to_commit()?;
    let (base_name, base_commit) = resolve_branch_base(&repo, name, base)?;

    let mb = repo.merge_base(base_commit.id(), head.id()).ok();
    let unrelated = mb.is_none();
    let mb_tree = match mb {
        Some(oid) => repo.find_commit(oid)?.tree()?,
        None => {
            let empty = repo.treebuilder(None)?.write()?;
            repo.find_tree(empty)?
        }
    };
    let head_tree = head.tree()?;

    let mut opts = build_diff_options(&[], false);
    let mut diff = repo.diff_tree_to_tree(Some(&mb_tree), Some(&head_tree), Some(&mut opts))?;
    apply_find_similar(&mut diff)?;
    let files = collect_file_diffs(&diff)?;

    let mut prefix = String::new();
    if unrelated {
        prefix.push_str(
            "NOTE: this branch and its base have no common ancestor (unrelated histories); \
             the diff is the full branch contents versus an empty base.\n\n",
        );
    }
    prefix.push_str(&format!("BRANCH {name} vs {base_name} (merge-base)\n\n"));
    Ok((prefix, files))
}

/// Truncates `text` to at most [`MAX_REVIEW_PAYLOAD_BYTES`] (P25 §2.4). When
/// over the cap, cuts on the largest char boundary that leaves room for
/// [`TRUNCATION_NOTE`] and appends the note, so the RESULT stays `<= cap` and
/// the model is told the diff was clipped. Under the cap, returns `text` as-is.
pub(crate) fn cap_review_payload(text: String) -> String {
    if text.len() <= MAX_REVIEW_PAYLOAD_BYTES {
        return text;
    }
    // Reserve room for the note so the final string never exceeds the cap.
    let budget = MAX_REVIEW_PAYLOAD_BYTES - TRUNCATION_NOTE.len();
    let mut end = budget;
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    let mut out = text[..end].to_string();
    out.push_str(TRUNCATION_NOTE);
    out
}

/// Gathers `target`'s file diffs and the payload text prefix (empty for
/// non-commit targets). Reuses the existing public diff fns; no new plumbing.
fn build_payload(workdir: &Path, target: &AiDiffTarget) -> Result<(String, Vec<FileDiff>), AppError> {
    match target {
        AiDiffTarget::Commit { oid } => {
            let cd = commit_diff(workdir, oid)?;
            let short7: String = cd.details.oid.chars().take(7).collect();
            // (D2) Ground on the author's stated intent — the full commit
            // MESSAGE, not just the diff — inserted after COMMIT/AUTHOR and
            // before the per-file blocks. `cd.details.message` is the full
            // lossy message with trailing whitespace already trimmed
            // (diff::commit_details), so no trailing blank line leaks in.
            let prefix = format!(
                "COMMIT {}  {}\nAUTHOR {}\nMESSAGE:\n{}\n\n",
                short7, cd.details.summary, cd.details.author_name, cd.details.message
            );
            let mut file_diffs = Vec::with_capacity(cd.files.len());
            for h in &cd.files {
                let fd = commit_file_diff(workdir, oid, &h.path, h.orig_path.as_deref(), false, false)?;
                file_diffs.push(fd);
            }
            Ok((prefix, file_diffs))
        }
        AiDiffTarget::WorkdirFile {
            path,
            orig_path,
            staged,
        } => {
            // Reject traversal/absolute paths up front and map to `InvalidName`
            // (same guard + mapping as `ai_resolve.rs`), so the wire error kind
            // matches the documented IPC contract rather than the bare `Other`
            // that `validate_rel_path` yields — before any git tree access.
            validate_rel_path(path)
                .map_err(|_| AppError::InvalidName(format!("invalid path: {path}")))?;
            let fd = workdir_file_diff(workdir, path, orig_path.as_deref(), *staged, false, false)?;
            Ok((String::new(), vec![fd]))
        }
        AiDiffTarget::Staged => Ok((String::new(), gather_staged(workdir)?)),
        AiDiffTarget::Worktree => Ok((String::new(), gather_worktree(workdir)?)),
        AiDiffTarget::Branch { name, base } => gather_branch(workdir, name, base.as_deref()),
    }
}

/// Blocking. Gathers `target`'s diff, renders a payload, calls run_claude with
/// the `mode` system prompt. An EMPTY target diff (no changes) => `AiFailed(
/// "no changes to analyze")` before any CLI call (§7.1). Errors: `aiFailed`
/// | `git` (bad oid) | `invalidName` (bad path) | `nothingToCommit` (empty
/// staged set) | (`aiUnavailable` via gate).
pub fn analyze_diff(
    workdir: &Path,
    target: AiDiffTarget,
    mode: AiAnalysisMode,
    opts: RunOpts,
) -> Result<AiAnalysis, AppError> {
    // 1. Gather the typed diffs (+ optional commit header prefix).
    let (prefix, file_diffs) = build_payload(workdir, &target)?;

    // 2. No textual/binary content => nothing to analyze, no CLI call.
    if !has_analyzable_content(&file_diffs) {
        return Err(AppError::AiFailed("no changes to analyze".to_string()));
    }

    // 3. Render the labeled payload (prefix carries commit metadata for Commit).
    //    A pathological long-line Worktree/Branch diff is byte-capped so the CLI
    //    call stays bounded (§2.4); small targets never trip the cap.
    let rendered = payload::render_file_diffs(&file_diffs);
    let payload_text = cap_review_payload(format!("{}{}", prefix, rendered.text));

    // 4. Select the (system prompt, prompt) pair from the mode.
    let (system_prompt, prompt) = match mode {
        AiAnalysisMode::Explain => (EXPLAIN_SYSTEM_PROMPT, EXPLAIN_PROMPT),
        AiAnalysisMode::Review => (REVIEW_SYSTEM_PROMPT, REVIEW_PROMPT),
    };

    // 5. Ask the CLI (system prompt set here; caller's `opts` carry model/timeout).
    let result = ai::run_claude(
        workdir,
        prompt,
        Some(&payload_text),
        RunOpts {
            system_prompt: Some(system_prompt.to_string()),
            ..opts
        },
    )?;

    Ok(AiAnalysis {
        text: result.text,
        cost_usd: result.cost_usd,
    })
}

// ============================================================ P28 digest

/// One digest metadata line: `- {short7} {YYYY-MM-DD} {author_name}  {subject}`
/// (P28 §4.3). Lossy UTF-8; date from `commit.time()` (UTC).
fn commit_meta_line(commit: &git2::Commit<'_>) -> String {
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
            let mut commits = Vec::new();
            for oid in walk {
                commits.push(repo.find_commit(oid?)?);
            }

            let old_tree = match mb {
                Some(oid) => Some(repo.find_commit(oid)?.tree()?),
                None => None,
            };
            let new_tree = to_c.tree()?;

            let mut header = format!("RANGE {from}..{to} ({} commits)", commits.len());
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
            for oid in walk {
                let commit = repo.find_commit(oid?)?;
                if commit.time().seconds() >= cutoff {
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
            let header = format!(
                "RANGE last {days} day(s) on {branch} ({} commits)",
                commits.len()
            );
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

#[cfg(test)]
mod tests {
    use super::*;

    /// The prompt/system-prompt consts MUST be single-line (Windows argv
    /// constraint): a newline in any of them would make `claude.cmd` reject the
    /// argument.
    #[test]
    fn prompts_are_single_line() {
        for s in [
            EXPLAIN_SYSTEM_PROMPT,
            REVIEW_SYSTEM_PROMPT,
            EXPLAIN_PROMPT,
            REVIEW_PROMPT,
            DIGEST_SYSTEM_PROMPT,
            DIGEST_PROMPT,
        ] {
            assert!(!s.contains('\n'), "prompt must be single-line: {s:?}");
            assert!(!s.contains('\r'), "prompt must be single-line: {s:?}");
        }
    }

    /// Serde casing must match the TS `AiAnalysis` type (`text` / `costUsd`);
    /// `None` cost serializes as `null`.
    #[test]
    fn analysis_wire_shape_is_camel_case() {
        let v = serde_json::to_value(AiAnalysis {
            text: "does a thing".to_string(),
            cost_usd: Some(0.006),
        })
        .expect("json");
        assert_eq!(
            v,
            serde_json::json!({ "text": "does a thing", "costUsd": 0.006 })
        );

        let v = serde_json::to_value(AiAnalysis {
            text: "no cost".to_string(),
            cost_usd: None,
        })
        .expect("json");
        assert_eq!(v, serde_json::json!({ "text": "no cost", "costUsd": null }));
    }

    /// `AiDiffTarget` deserializes from the EXACT JSON the TS discriminated
    /// union sends for each variant — locking the IPC contract without a CLI.
    #[test]
    fn diff_target_deserializes_each_variant() {
        let commit: AiDiffTarget =
            serde_json::from_str(r#"{"kind":"commit","oid":"deadbeef"}"#).expect("commit");
        match commit {
            AiDiffTarget::Commit { oid } => assert_eq!(oid, "deadbeef"),
            other => panic!("expected Commit, got {other:?}"),
        }

        let wf: AiDiffTarget = serde_json::from_str(
            r#"{"kind":"workdirFile","path":"src/a.rs","origPath":null,"staged":true}"#,
        )
        .expect("workdirFile");
        match wf {
            AiDiffTarget::WorkdirFile {
                path,
                orig_path,
                staged,
            } => {
                assert_eq!(path, "src/a.rs");
                assert_eq!(orig_path, None);
                assert!(staged);
            }
            other => panic!("expected WorkdirFile, got {other:?}"),
        }

        // origPath may also be a string, and (via #[serde(default)]) omitted.
        let wf_renamed: AiDiffTarget = serde_json::from_str(
            r#"{"kind":"workdirFile","path":"src/new.rs","origPath":"src/old.rs","staged":false}"#,
        )
        .expect("workdirFile renamed");
        match wf_renamed {
            AiDiffTarget::WorkdirFile {
                orig_path, staged, ..
            } => {
                assert_eq!(orig_path.as_deref(), Some("src/old.rs"));
                assert!(!staged);
            }
            other => panic!("expected WorkdirFile, got {other:?}"),
        }

        let staged: AiDiffTarget =
            serde_json::from_str(r#"{"kind":"staged"}"#).expect("staged");
        assert!(matches!(staged, AiDiffTarget::Staged));

        // P25 B1: the two new review scopes deserialize from the exact TS union.
        let worktree: AiDiffTarget =
            serde_json::from_str(r#"{"kind":"worktree"}"#).expect("worktree");
        assert!(matches!(worktree, AiDiffTarget::Worktree));

        // Branch: base may be null, omitted (via #[serde(default)]), or a string.
        let branch_null: AiDiffTarget =
            serde_json::from_str(r#"{"kind":"branch","name":"feature","base":null}"#)
                .expect("branch base null");
        match branch_null {
            AiDiffTarget::Branch { name, base } => {
                assert_eq!(name, "feature");
                assert_eq!(base, None);
            }
            other => panic!("expected Branch, got {other:?}"),
        }

        let branch_omitted: AiDiffTarget =
            serde_json::from_str(r#"{"kind":"branch","name":"feature"}"#)
                .expect("branch base omitted");
        match branch_omitted {
            AiDiffTarget::Branch { name, base } => {
                assert_eq!(name, "feature");
                assert_eq!(base, None);
            }
            other => panic!("expected Branch, got {other:?}"),
        }

        let branch_base: AiDiffTarget =
            serde_json::from_str(r#"{"kind":"branch","name":"f","base":"main"}"#)
                .expect("branch base string");
        match branch_base {
            AiDiffTarget::Branch { name, base } => {
                assert_eq!(name, "f");
                assert_eq!(base.as_deref(), Some("main"));
            }
            other => panic!("expected Branch, got {other:?}"),
        }
    }

    // ---- P25 B1 scratch-repo + cap tests -------------------------------------

    use crate::git::commit::create_commit;
    use crate::git::stage::stage_paths;

    /// git2-init a scratch repo with identity + autocrlf off (mirrors `diff.rs`).
    fn init_scratch() -> tempfile::TempDir {
        let dir = crate::testutil::scratch_dir();
        let repo = git2::Repository::init(dir.path()).expect("init repo");
        let mut cfg = repo.config().expect("config");
        cfg.set_str("user.name", "Test User").expect("name");
        cfg.set_str("user.email", "test@example.com").expect("email");
        cfg.set_bool("core.autocrlf", false).expect("autocrlf");
        dir
    }

    fn commit_of<'r>(repo: &'r git2::Repository, oid: &str) -> git2::Commit<'r> {
        repo.find_commit(git2::Oid::from_str(oid).expect("oid"))
            .expect("commit")
    }

    /// §9.1(3): `gather_worktree` covers staged + unstaged + untracked in one
    /// pass; a clean worktree gathers empty (→ `AiFailed` in `analyze_diff`).
    #[test]
    fn gather_worktree_covers_all_change_kinds() {
        let dir = init_scratch();
        let p = dir.path();

        std::fs::write(p.join("staged.txt"), "s1\n").expect("write");
        std::fs::write(p.join("unstaged.txt"), "u1\n").expect("write");
        stage_paths(p, &["staged.txt".into(), "unstaged.txt".into()]).expect("stage");
        create_commit(p, "base", None, false).expect("commit");

        // Clean worktree => empty gather.
        assert!(
            gather_worktree(p).expect("gather clean").is_empty(),
            "a clean worktree must gather no files"
        );

        // Stage a modification to staged.txt.
        std::fs::write(p.join("staged.txt"), "s1 changed\n").expect("write");
        stage_paths(p, &["staged.txt".into()]).expect("stage");
        // Modify unstaged.txt WITHOUT staging.
        std::fs::write(p.join("unstaged.txt"), "u1 changed\n").expect("write");
        // Add an untracked file.
        std::fs::write(p.join("untracked.txt"), "new\n").expect("write");

        let files = gather_worktree(p).expect("gather dirty");
        let mut paths: Vec<&str> = files.iter().map(|f| f.path.as_str()).collect();
        paths.sort_unstable();
        assert_eq!(
            paths,
            vec!["staged.txt", "unstaged.txt", "untracked.txt"],
            "worktree gather must cover staged + unstaged + untracked"
        );
        assert!(has_analyzable_content(&files));
    }

    /// §7.10 (P53b/D2): the Commit-target grounding prefix now carries the FULL
    /// commit MESSAGE (author intent = the strongest "why" signal), not just the
    /// summary — inserted after AUTHOR and before the per-file blocks.
    #[test]
    fn commit_payload_prefix_carries_full_message() {
        let dir = init_scratch();
        let p = dir.path();

        std::fs::write(p.join("f.txt"), "hello\n").expect("write");
        stage_paths(p, &["f.txt".into()]).expect("stage");
        // Summary line + a body line whose text lives ONLY in the body — proving
        // the whole message body (not merely the summary) reaches the grounding.
        let msg = "Add greeting\n\nExplains WHY: users needed a friendly hello.";
        let oid = create_commit(p, msg, None, false).expect("commit").oid;

        let (prefix, files) =
            build_payload(p, &AiDiffTarget::Commit { oid }).expect("build payload");

        assert!(
            prefix.contains("MESSAGE:\n"),
            "prefix must carry a MESSAGE section: {prefix:?}"
        );
        assert!(
            prefix.contains("Explains WHY: users needed a friendly hello."),
            "prefix must carry the full message BODY, not just the summary: {prefix:?}"
        );
        // Ordering: COMMIT → AUTHOR → MESSAGE, before the (separately rendered)
        // per-file blocks.
        let author_idx = prefix.find("\nAUTHOR ").expect("AUTHOR line");
        let msg_idx = prefix.find("\nMESSAGE:").expect("MESSAGE line");
        assert!(msg_idx > author_idx, "MESSAGE must follow AUTHOR: {prefix:?}");
        assert!(!files.is_empty(), "the commit changed a file");
    }

    /// §9.1(4): `resolve_branch_base` — explicit base wins over everything.
    #[test]
    fn resolve_base_explicit_wins() {
        let dir = init_scratch();
        let p = dir.path();
        std::fs::write(p.join("a.txt"), "a\n").expect("write");
        stage_paths(p, &["a.txt".into()]).expect("stage");
        let head = create_commit(p, "A", None, false).expect("commit").oid;

        let repo = git2::Repository::open(p).expect("open");
        let head_commit = commit_of(&repo, &head);
        // A distinct base branch pointing at the same commit is enough to verify
        // the returned shorthand + commit come from the explicit ref.
        repo.branch("some-base", &head_commit, true).expect("branch");

        let (name, commit) = resolve_branch_base(&repo, "feature", Some("some-base")).expect("base");
        assert_eq!(name, "some-base");
        assert_eq!(commit.id().to_string(), head);
    }

    /// §9.1(4): configured upstream is used when no explicit base is given.
    #[test]
    fn resolve_base_uses_upstream() {
        let dir = init_scratch();
        let p = dir.path();
        std::fs::write(p.join("a.txt"), "a\n").expect("write");
        stage_paths(p, &["a.txt".into()]).expect("stage");
        let head = create_commit(p, "A", None, false).expect("commit").oid;

        let repo = git2::Repository::open(p).expect("open");
        let head_commit = commit_of(&repo, &head);
        repo.branch("feature", &head_commit, true).expect("local branch");
        repo.remote_with_fetch(
            "origin",
            "https://example.invalid/x.git",
            "+refs/heads/*:refs/remotes/origin/*",
        )
        .expect("remote");
        repo.reference(
            "refs/remotes/origin/feature",
            head_commit.id(),
            true,
            "test upstream",
        )
        .expect("remote-tracking ref");
        {
            let mut cfg = repo.config().expect("config");
            cfg.set_str("branch.feature.remote", "origin").expect("remote cfg");
            cfg.set_str("branch.feature.merge", "refs/heads/feature")
                .expect("merge cfg");
        }

        let (name, commit) = resolve_branch_base(&repo, "feature", None).expect("base");
        assert_eq!(name, "origin/feature");
        assert_eq!(commit.id().to_string(), head);
    }

    /// §9.1(4): with no upstream, `origin/HEAD`'s target is used (before local
    /// `main`/`master`).
    #[test]
    fn resolve_base_uses_origin_head() {
        let dir = init_scratch();
        let p = dir.path();
        std::fs::write(p.join("a.txt"), "a\n").expect("write");
        stage_paths(p, &["a.txt".into()]).expect("stage");
        let head = create_commit(p, "A", None, false).expect("commit").oid;

        let repo = git2::Repository::open(p).expect("open");
        let head_commit = commit_of(&repo, &head);
        repo.reference("refs/remotes/origin/main", head_commit.id(), true, "test")
            .expect("origin/main");
        repo.reference_symbolic(
            "refs/remotes/origin/HEAD",
            "refs/remotes/origin/main",
            true,
            "test",
        )
        .expect("origin/HEAD symbolic");

        let (name, commit) = resolve_branch_base(&repo, "no-upstream", None).expect("base");
        assert_eq!(name, "origin/main");
        assert_eq!(commit.id().to_string(), head);
    }

    /// §9.1(4): with no upstream and no `origin/HEAD`, local `main` is preferred
    /// over `master`; and with neither `main` nor any base, an error results.
    #[test]
    fn resolve_base_main_then_master_then_error() {
        let dir = init_scratch();
        let p = dir.path();
        std::fs::write(p.join("a.txt"), "a\n").expect("write");
        stage_paths(p, &["a.txt".into()]).expect("stage");
        let head = create_commit(p, "A", None, false).expect("commit").oid;

        let repo = git2::Repository::open(p).expect("open");
        let head_commit = commit_of(&repo, &head);

        // Ensure both main + master exist at head without force-updating whichever
        // one is the current HEAD (libgit2's default branch varies by host config).
        let head_name = repo
            .head()
            .expect("head")
            .shorthand()
            .expect("head shorthand")
            .to_string();
        for b in ["main", "master"] {
            if b != head_name {
                repo.branch(b, &head_commit, true).expect("branch");
            }
        }
        // main wins over master.
        let (name, _) = resolve_branch_base(&repo, "topic", None).expect("main base");
        assert_eq!(name, "main");

        // Move HEAD to a lone `topic`, then drop main → master wins.
        repo.branch("topic", &head_commit, true).expect("topic");
        repo.set_head("refs/heads/topic").expect("head->topic");
        repo.find_branch("main", git2::BranchType::Local)
            .expect("find main")
            .delete()
            .expect("delete main");
        let (name, _) = resolve_branch_base(&repo, "topic", None).expect("master base");
        assert_eq!(name, "master");

        // Drop master too → no base resolves.
        repo.find_branch("master", git2::BranchType::Local)
            .expect("find master")
            .delete()
            .expect("delete master");
        let err = resolve_branch_base(&repo, "topic", None).expect_err("no base");
        assert!(matches!(err, AppError::Git(_)), "got {err:?}");
    }

    /// §9.1(5): the byte-cap truncates an oversize payload with the marker, keeps
    /// the result ≤ cap, and cuts on a valid char boundary.
    #[test]
    fn review_payload_byte_cap() {
        // Under the cap: returned unchanged.
        let small = "small payload".to_string();
        assert_eq!(cap_review_payload(small.clone()), small);

        // Multi-byte content well over the cap → truncated, still valid UTF-8.
        let big = "é".repeat(MAX_REVIEW_PAYLOAD_BYTES); // 2 bytes each
        let capped = cap_review_payload(big);
        assert!(
            capped.len() <= MAX_REVIEW_PAYLOAD_BYTES,
            "capped payload ({} bytes) must stay <= cap",
            capped.len()
        );
        assert!(
            capped.ends_with(TRUNCATION_NOTE),
            "capped payload must carry the truncation marker"
        );
        // Round-trips as UTF-8 (a bad char boundary would corrupt it) — the
        // string already IS valid UTF-8, so just assert no replacement char crept
        // in at the cut and the prefix is all 'é'.
        let body = &capped[..capped.len() - TRUNCATION_NOTE.len()];
        assert!(body.chars().all(|c| c == 'é'), "cut must be on a char boundary");
    }

    /// `AiAnalysisMode` deserializes from the exact `"explain"`/`"review"`
    /// literals the TS `AiAnalysisMode` union sends.
    #[test]
    fn analysis_mode_deserializes_literals() {
        let explain: AiAnalysisMode = serde_json::from_str(r#""explain""#).expect("explain");
        assert!(matches!(explain, AiAnalysisMode::Explain));
        let review: AiAnalysisMode = serde_json::from_str(r#""review""#).expect("review");
        assert!(matches!(review, AiAnalysisMode::Review));
    }

    // ---- P28 digest tests -----------------------------------------------------

    /// §10.1(1): `AiDigestRange` deserializes the exact TS JSON per variant.
    #[test]
    fn digest_range_deserializes_each_variant() {
        let br: AiDigestRange =
            serde_json::from_str(r#"{"kind":"betweenRefs","from":"main","to":"feature"}"#)
                .expect("betweenRefs");
        match br {
            AiDigestRange::BetweenRefs { from, to } => {
                assert_eq!(from, "main");
                assert_eq!(to, "feature");
            }
            other => panic!("expected BetweenRefs, got {other:?}"),
        }

        let ld: AiDigestRange =
            serde_json::from_str(r#"{"kind":"lastDays","days":7}"#).expect("lastDays");
        match ld {
            AiDigestRange::LastDays { days } => assert_eq!(days, 7),
            other => panic!("expected LastDays, got {other:?}"),
        }

        let sc: AiDigestRange =
            serde_json::from_str(r#"{"kind":"sinceCommit","oid":"deadbeef"}"#).expect("sinceCommit");
        match sc {
            AiDigestRange::SinceCommit { oid } => assert_eq!(oid, "deadbeef"),
            other => panic!("expected SinceCommit, got {other:?}"),
        }
    }

    /// §10.1(6): 250 synthetic metas → 200 lines + "... and 50 more commits".
    #[test]
    fn format_commit_meta_caps_at_200() {
        let lines: Vec<String> = (0..250).map(|i| format!("- {i:07} line")).collect();
        let out = format_commit_meta(&lines);
        assert_eq!(out.lines().count(), MAX_DIGEST_COMMITS + 1);
        assert!(out.ends_with("... and 50 more commits"), "got tail: {out:?}");
        assert!(out.starts_with("- 0000000 line"));
        // Under the cap: joined verbatim, no overflow note.
        let small = format_commit_meta(&lines[..3]);
        assert_eq!(small.lines().count(), 3);
        assert!(!small.contains("more commits"));
    }

    /// git2-only fixture helpers: commits with controlled committer times and
    /// per-commit unique trees (no workdir writes needed).
    fn tree_with<'r>(repo: &'r git2::Repository, key: &str) -> git2::Tree<'r> {
        let blob = repo.blob(key.as_bytes()).expect("blob");
        let mut tb = repo.treebuilder(None).expect("treebuilder");
        tb.insert("f.txt", blob, 0o100644).expect("insert");
        let oid = tb.write().expect("tree write");
        repo.find_tree(oid).expect("find tree")
    }

    fn commit_at(
        repo: &git2::Repository,
        update_ref: Option<&str>,
        msg: &str,
        secs: i64,
        parents: &[&git2::Commit<'_>],
    ) -> git2::Oid {
        let sig = git2::Signature::new("Test User", "test@example.com", &git2::Time::new(secs, 0))
            .expect("signature");
        let tree = tree_with(repo, msg);
        repo.commit(update_ref, &sig, &sig, msg, &tree, parents)
            .expect("commit")
    }

    /// Builds the §10.1(2) fixture: `main` = A→B, `feature` = B→C→D, HEAD on
    /// feature. Returns (dir, [a, b, c, d]).
    fn digest_fixture() -> (tempfile::TempDir, [git2::Oid; 4]) {
        let dir = init_scratch();
        let repo = git2::Repository::open(dir.path()).expect("open");
        let t = 1_700_000_000i64;
        let a = commit_at(&repo, None, "A", t, &[]);
        let a_c = repo.find_commit(a).expect("A");
        let b = commit_at(&repo, None, "B", t + 10, &[&a_c]);
        let b_c = repo.find_commit(b).expect("B");
        let c = commit_at(&repo, None, "C", t + 20, &[&b_c]);
        let c_c = repo.find_commit(c).expect("C");
        let d = commit_at(&repo, None, "D", t + 30, &[&c_c]);
        let d_c = repo.find_commit(d).expect("D");
        repo.branch("main", &b_c, true).expect("main");
        repo.branch("feature", &d_c, true).expect("feature");
        repo.set_head("refs/heads/feature").expect("head");
        drop((a_c, b_c, c_c, d_c));
        (dir, [a, b, c, d])
    }

    /// §10.1(2): BetweenRefs{main, feature} → exactly [D, C] newest-first,
    /// old_tree = B's tree; header carries the count.
    #[test]
    fn between_refs_walk_yields_range_commits_and_merge_base_tree() {
        let (dir, [_a, b, c, d]) = digest_fixture();
        let repo = git2::Repository::open(dir.path()).expect("open");
        let range = AiDigestRange::BetweenRefs {
            from: "main".to_string(),
            to: "feature".to_string(),
        };
        let (header, commits, old_tree, new_tree) =
            resolve_digest_range(&repo, &range).expect("resolve");
        let ids: Vec<git2::Oid> = commits.iter().map(|c| c.id()).collect();
        assert_eq!(ids, vec![d, c], "newest-first D then C");
        let b_tree = repo.find_commit(b).expect("B").tree().expect("tree").id();
        assert_eq!(old_tree.expect("old tree").id(), b_tree);
        assert_eq!(new_tree.id(), repo.find_commit(d).expect("D").tree().expect("t").id());
        assert!(header.contains("RANGE main..feature (2 commits)"), "got {header}");
        assert!(!header.contains("no common ancestor"));
    }

    /// §10.1(2): `from == to` → zero commits → `digest_changes` returns
    /// `AiFailed("no changes in the selected range")` BEFORE any CLI call.
    #[test]
    fn empty_range_fails_before_cli() {
        let (dir, _) = digest_fixture();
        let err = digest_changes(
            dir.path(),
            AiDigestRange::BetweenRefs {
                from: "feature".to_string(),
                to: "feature".to_string(),
            },
            RunOpts::default(),
        )
        .expect_err("empty range must fail");
        match err {
            AppError::AiFailed(m) => assert_eq!(m, "no changes in the selected range"),
            other => panic!("expected AiFailed, got {other:?}"),
        }
    }

    /// §10.1(3): SinceCommit{B} ≡ BetweenRefs{B, HEAD} → [D, C].
    #[test]
    fn since_commit_is_between_refs_to_head() {
        let (dir, [_a, b, c, d]) = digest_fixture();
        let repo = git2::Repository::open(dir.path()).expect("open");
        let range = AiDigestRange::SinceCommit { oid: b.to_string() };
        let (_h, commits, old_tree, _new) = resolve_digest_range(&repo, &range).expect("resolve");
        let ids: Vec<git2::Oid> = commits.iter().map(|cm| cm.id()).collect();
        assert_eq!(ids, vec![d, c]);
        assert_eq!(
            old_tree.expect("old tree").id(),
            repo.find_commit(b).expect("B").tree().expect("t").id()
        );
    }

    /// §10.1(4): unrelated histories → no hide (full `to` history), old_tree
    /// None (empty tree), header carries the no-common-ancestor note.
    #[test]
    fn unrelated_histories_diff_vs_empty_tree_with_note() {
        let (dir, [a, b, c, d]) = digest_fixture();
        let repo = git2::Repository::open(dir.path()).expect("open");
        let r = commit_at(&repo, None, "ROOT2", 1_700_000_100, &[]);
        let r_c = repo.find_commit(r).expect("R");
        repo.branch("other", &r_c, true).expect("other");

        let range = AiDigestRange::BetweenRefs {
            from: "other".to_string(),
            to: "feature".to_string(),
        };
        let (header, commits, old_tree, _new) = resolve_digest_range(&repo, &range).expect("resolve");
        let ids: Vec<git2::Oid> = commits.iter().map(|cm| cm.id()).collect();
        assert_eq!(ids, vec![d, c, b, a], "full feature history, newest first");
        assert!(old_tree.is_none(), "unrelated → empty-tree base");
        assert!(header.contains("no common ancestor"), "got {header}");
    }

    /// §10.1(5): lastDays first-parent walk with controlled committer times —
    /// commits at now−1d/−2d/−10d; days=7 collects the two recent, boundary =
    /// the 10-day-old commit; days=0 → InvalidName; all-in-window → old_tree None.
    #[test]
    fn last_days_walk_cutoff_and_boundary() {
        let dir = init_scratch();
        let repo = git2::Repository::open(dir.path()).expect("open");
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_secs() as i64;
        let day = 86_400i64;
        let old = commit_at(&repo, Some("HEAD"), "old-10d", now - 10 * day, &[]);
        let old_c = repo.find_commit(old).expect("old");
        let mid = commit_at(&repo, Some("HEAD"), "mid-2d", now - 2 * day, &[&old_c]);
        let mid_c = repo.find_commit(mid).expect("mid");
        let new = commit_at(&repo, Some("HEAD"), "new-1d", now - day, &[&mid_c]);

        let (header, commits, old_tree, new_tree) =
            resolve_digest_range(&repo, &AiDigestRange::LastDays { days: 7 }).expect("resolve");
        let ids: Vec<git2::Oid> = commits.iter().map(|cm| cm.id()).collect();
        assert_eq!(ids, vec![new, mid], "two in-window commits, newest first");
        assert_eq!(
            old_tree.expect("boundary tree").id(),
            old_c.tree().expect("t").id(),
            "boundary = the 10-day-old commit's tree"
        );
        assert_eq!(new_tree.id(), repo.find_commit(new).expect("n").tree().expect("t").id());
        assert!(header.contains("last 7 day(s)"), "got {header}");
        assert!(header.contains("(2 commits)"), "got {header}");

        // days=0 → InvalidName, before any repo access matters.
        let err = resolve_digest_range(&repo, &AiDigestRange::LastDays { days: 0 })
            .expect_err("days=0 must fail");
        assert!(matches!(err, AppError::InvalidName(_)), "got {err:?}");

        // Whole history inside the window → old_tree None (diff vs empty tree).
        let (_h, commits, old_tree, _n) =
            resolve_digest_range(&repo, &AiDigestRange::LastDays { days: 30 }).expect("resolve");
        assert_eq!(commits.len(), 3);
        assert!(old_tree.is_none(), "all-in-window → empty-tree base");
    }

    /// The metadata line format: `- {short7} {YYYY-MM-DD} {author}  {subject}`.
    #[test]
    fn commit_meta_line_format() {
        let (dir, [_a, _b, _c, d]) = digest_fixture();
        let repo = git2::Repository::open(dir.path()).expect("open");
        let d_c = repo.find_commit(d).expect("D");
        let line = commit_meta_line(&d_c);
        let short7: String = d.to_string().chars().take(7).collect();
        assert_eq!(line, format!("- {short7} 2023-11-14 Test User  D"));
    }
}
