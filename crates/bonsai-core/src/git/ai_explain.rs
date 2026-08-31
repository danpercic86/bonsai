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

/// Hard bound on commits MATERIALIZED by a digest-range walk (audit §3.15).
/// Comfortably above [`MAX_DIGEST_COMMITS`] (so the meta cap still sees its
/// overflow count for mid-sized ranges) but bounded — a `BetweenRefs` over an
/// unrelated-history or ancient base would otherwise collect the entire
/// history in memory. Past the cap the walk STOPS and the header records the
/// truncation; the range DIFF is tree-to-tree and still spans the whole range.
pub const MAX_DIGEST_WALK_COMMITS: usize = 400;

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
//
// The digest range resolution + `digest_changes` entry point live in the
// `digest` submodule; re-exported here so `crate::git::ai_explain::<item>`
// paths (and the sibling AI modules that reuse the range resolver) are
// unchanged.
mod digest;

pub use digest::digest_changes;
pub(crate) use digest::resolve_digest_range;
// `format_commit_meta` / `commit_meta_line` are exercised only by the digest
// test module (via `super::*`), so their re-exports are test-only.
#[cfg(test)]
pub(crate) use digest::{commit_meta_line, format_commit_meta};

#[cfg(test)]
mod test_support;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod digest_tests;
