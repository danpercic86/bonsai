//! AI branch/range summary. Given a base ref and a target ref, gathers the
//! commits unique to target (base..target) plus the net diffstat, renders a
//! compact payload, and asks the CLI to summarize what the branch/range
//! introduces. Read-only prose out; WRITES NOTHING. Pure git2 + crate::ai. (P15)

use std::path::Path;

use crate::ai::payload;
use crate::ai::{self, RunOpts};
use crate::error::AppError;
use crate::git::diff::{apply_find_similar, build_diff_options, collect_headers};
use crate::git::stage::open_workdir_repo;

/// System prompt (via `--append-system-prompt`) for the range summary (contract
/// §4.7, verbatim). SINGLE line — on Windows the `claude` CLI is a `.cmd` shim
/// and Rust's `Command` REFUSES an argv arg containing a newline. Multi-line
/// content only ever flows through the stdin payload. (P15)
const SUMMARY_SYSTEM_PROMPT: &str = "You are summarizing the difference between two Git points for a teammate. Given a list of commits and a diffstat on standard input, summarize what this branch or range introduces: the main themes, the notable changes grouped sensibly, and anything risky or incomplete. Be concise. Output prose only — no markdown code fences.";

/// The `-p` positional prompt (contract §4.7, verbatim single line). (P15)
const SUMMARY_PROMPT: &str = "Summarize the branch or range described on standard input.";

/// Cap on commits listed in the payload (keeps the call bounded). Beyond it the
/// list is truncated with a "(+N more commits)" note.
pub const AI_SUMMARY_MAX_COMMITS: usize = 200;

/// Prose summary + echoed context. Serialized camelCase (mirrored in TS).
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiSummary {
    pub text: String,
    pub base: String,      // resolved base ref shorthand, echoed for the UI header
    pub target: String,    // resolved target ref shorthand
    pub commit_count: u32, // commits listed (capped at AI_SUMMARY_MAX_COMMITS)
    pub cost_usd: Option<f64>,
}

/// Blocking. `base`/`target` are ref shorthands/oids (revparse_single).
/// Uses the merge-base of the two (§7.3 decision) so the summary reflects what
/// TARGET introduces since divergence; for unrelated histories (no merge base)
/// it falls back to the empty tree / `base` and notes it in the payload header.
/// Empty range (no unique commits) => `AiFailed("nothing to summarize: <target>
/// has no commits beyond <base>")` BEFORE any CLI call. Errors: `aiFailed` |
/// `git` (bad ref) | (`aiUnavailable` via gate).
pub fn summarize_range(
    workdir: &Path,
    base: &str,
    target: &str,
    opts: RunOpts,
) -> Result<AiSummary, AppError> {
    let repo = open_workdir_repo(workdir)?;

    // 1. Resolve both refs to commits. A bad ref => `Git` (via revparse/peel).
    let base_commit = repo.revparse_single(base)?.peel_to_commit()?;
    let base_oid = base_commit.id();
    let target_commit = repo.revparse_single(target)?.peel_to_commit()?;
    let target_oid = target_commit.id();
    let target_tree = target_commit.tree()?;

    // 2. Merge base of the two. No merge base => unrelated histories: compare
    //    against the EMPTY tree and hide `base` directly (noted in the header).
    let mb_oid = repo.merge_base(base_oid, target_oid).ok();
    let unrelated = mb_oid.is_none();
    let mb_tree = match mb_oid {
        Some(oid) => repo.find_commit(oid)?.tree()?,
        None => {
            let empty = repo.treebuilder(None)?.write()?;
            repo.find_tree(empty)?
        }
    };

    // 3. Revwalk the commits unique to target (mb..target, or base..target when
    //    unrelated). Collect up to the cap; track the pre-truncation total.
    let mut walk = repo.revwalk()?;
    walk.set_sorting(git2::Sort::TIME)?;
    walk.push(target_oid)?;
    match mb_oid {
        Some(oid) => walk.hide(oid)?,
        None => walk.hide(base_oid)?,
    }

    let mut commit_lines: Vec<payload::CommitLine> = Vec::new();
    let mut total = 0usize;
    for oid_res in walk {
        let oid = oid_res?;
        total += 1;
        if commit_lines.len() < AI_SUMMARY_MAX_COMMITS {
            let commit = repo.find_commit(oid)?;
            let short_oid: String = oid.to_string().chars().take(7).collect();
            let summary = commit
                .summary_bytes()
                .map(|b| String::from_utf8_lossy(b).into_owned())
                .unwrap_or_default();
            let author = String::from_utf8_lossy(commit.author().name_bytes()).into_owned();
            commit_lines.push(payload::CommitLine {
                short_oid,
                summary,
                author,
            });
        }
    }

    // 3b. Empty range => nothing to summarize, no CLI call.
    if total == 0 {
        return Err(AppError::AiFailed(format!(
            "nothing to summarize: {target} has no commits beyond {base}"
        )));
    }
    // `commit_count` = commits listed (capped display, §4.5). The pre-truncation
    // total drives the "(+N more commits)" note below.
    let commit_count = u32::try_from(commit_lines.len()).unwrap_or(u32::MAX);

    // 4. Aggregate net diffstat: mb_tree (or empty) vs target_tree, headers only.
    let mut diff_opts = build_diff_options(&[], false);
    let mut diff =
        repo.diff_tree_to_tree(Some(&mb_tree), Some(&target_tree), Some(&mut diff_opts))?;
    apply_find_similar(&mut diff)?;
    let headers = collect_headers(&diff)?;
    let diffstat = payload::render_headers(&headers);

    // 5. Assemble the labeled payload (multi-line => stdin ONLY, never argv).
    let mut commits_section = payload::render_commit_list(&commit_lines);
    if total > commit_lines.len() {
        commits_section.push_str(&format!("(+{} more commits)\n", total - commit_lines.len()));
    }
    let mut payload_text = String::new();
    if unrelated {
        payload_text.push_str(
            "NOTE: base and target have no common ancestor (unrelated histories); \
             the diffstat is the full contents of target versus an empty base.\n\n",
        );
    }
    payload_text.push_str(&format!(
        "COMMITS (target since base):\n{commits_section}\n\nNET CHANGES (diffstat):\n{}",
        diffstat.text
    ));

    // 6. Ask the CLI (system prompt set here; caller's `opts` carry model/timeout).
    let result = ai::run_claude(
        workdir,
        SUMMARY_PROMPT,
        Some(&payload_text),
        RunOpts {
            system_prompt: Some(SUMMARY_SYSTEM_PROMPT.to_string()),
            ..opts
        },
    )?;

    Ok(AiSummary {
        text: result.text,
        base: base.to_string(),
        target: target.to_string(),
        commit_count,
        cost_usd: result.cost_usd,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The prompt/system-prompt consts MUST be single-line (Windows argv
    /// constraint): a newline in either would make `claude.cmd` reject the
    /// argument.
    #[test]
    fn prompts_are_single_line() {
        for s in [SUMMARY_SYSTEM_PROMPT, SUMMARY_PROMPT] {
            assert!(!s.contains('\n'), "prompt must be single-line: {s:?}");
            assert!(!s.contains('\r'), "prompt must be single-line: {s:?}");
        }
    }

    /// §8.4-adjacent: serde casing must match the TS `AiSummary` type
    /// (`text` / `base` / `target` / `commitCount` / `costUsd`); `None` cost
    /// serializes as `null`.
    #[test]
    fn summary_wire_shape_is_camel_case() {
        let v = serde_json::to_value(AiSummary {
            text: "introduces the AI features".to_string(),
            base: "main".to_string(),
            target: "feature/ai".to_string(),
            commit_count: 3,
            cost_usd: Some(0.008),
        })
        .expect("json");
        assert_eq!(
            v,
            serde_json::json!({
                "text": "introduces the AI features",
                "base": "main",
                "target": "feature/ai",
                "commitCount": 3,
                "costUsd": 0.008
            })
        );

        let v = serde_json::to_value(AiSummary {
            text: "no cost".to_string(),
            base: "master".to_string(),
            target: "topic".to_string(),
            commit_count: 0,
            cost_usd: None,
        })
        .expect("json");
        assert_eq!(
            v,
            serde_json::json!({
                "text": "no cost",
                "base": "master",
                "target": "topic",
                "commitCount": 0,
                "costUsd": null
            })
        );
    }
}
