//! AI commit-message generation. Reads the STAGED diff (HEAD tree vs index),
//! renders a payload, and asks the local `claude` CLI for a concise
//! Conventional-Commits message. WRITES NOTHING — the user edits the returned
//! text in the commit box before committing. Pure git2 + crate::ai. (P15)

use std::path::Path;

use crate::ai::payload;
use crate::ai::{self, RunOpts};
use crate::error::AppError;
use crate::git::diff::workdir_file_diff;
use crate::git::status::read_status;

/// System prompt (via `--append-system-prompt`): role + strict output contract
/// (contract §3.2). The words are verbatim and deliberately collapsed to a
/// SINGLE line — on Windows the `claude` CLI is a `.cmd` shim and Rust's
/// `Command` REFUSES an argv argument containing a newline (batch-file argument
/// mitigation). Multi-line content only ever flows through the stdin payload. (P15)
const COMMIT_SYSTEM_PROMPT: &str = "You are a Git commit-message author. Given a staged diff on standard input, write ONE concise commit message in Conventional Commits style: a short imperative summary line of at most 72 characters (for example 'feat(scope): ...', 'fix: ...', 'refactor: ...'), then, only if warranted, a blank line followed by a brief body of one-line bullet points. Output ONLY the commit message text — no explanations, no preamble, no surrounding quotes, and no markdown code fences.";

/// The `-p` positional prompt (contract §3.2, verbatim single line). (P15)
const COMMIT_PROMPT: &str = "Write a commit message for the staged changes provided on standard input.";

/// The model's proposed commit message. Serialized camelCase (mirrored in TS).
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommitMessageProposal {
    pub message: String, // trimmed; may contain newlines (summary + body)
    pub cost_usd: Option<f64>,
}

/// Blocking. Gathers the staged diff and returns a proposed message.
/// - Empty staged set (index matches HEAD) => `AppError::NothingToCommit`
///   BEFORE any CLI call (§7.1).
/// - Otherwise renders the staged payload (§3.1) and calls run_claude.
///
/// Errors: `aiFailed` (CLI error/empty/timeout) | `nothingToCommit`
///   (empty staged) | `git` (repo open) | (`aiUnavailable` is enforced by the
///   command gate, not here).
pub fn generate_commit_message(
    workdir: &Path,
    opts: RunOpts,
) -> Result<CommitMessageProposal, AppError> {
    // 1. Collect the entries staged in the index (the "staged" list the status
    //    panel already shows). Empty => nothing to commit, no CLI call.
    let staged = read_status(workdir)?.staged;
    if staged.is_empty() {
        return Err(AppError::NothingToCommit);
    }

    // 2. Per staged entry, gather the full HEAD-tree-vs-index diff (staged=true),
    //    respecting the entry's origPath for renames.
    let mut file_diffs = Vec::with_capacity(staged.len());
    for entry in &staged {
        let fd = workdir_file_diff(workdir, &entry.path, entry.orig_path.as_deref(), true)?;
        file_diffs.push(fd);
    }

    // 3. Render the labeled payload with a one-line header.
    let rendered = payload::render_file_diffs(&file_diffs);
    let payload_text = format!("STAGED CHANGES (git diff --cached):\n\n{}", rendered.text);

    // 4. Ask the CLI (system prompt set here; caller's `opts` carry model/timeout).
    let result = ai::run_claude(
        workdir,
        COMMIT_PROMPT,
        Some(&payload_text),
        RunOpts {
            system_prompt: Some(COMMIT_SYSTEM_PROMPT.to_string()),
            ..opts
        },
    )?;

    Ok(CommitMessageProposal {
        message: result.text,
        cost_usd: result.cost_usd,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// §8.1(4): the serde casing must match the TS `CommitMessageProposal` type
    /// exactly (`message` / `costUsd`); `None` cost serializes as `null`.
    #[test]
    fn proposal_wire_shape_is_camel_case() {
        let v = serde_json::to_value(CommitMessageProposal {
            message: "feat: add thing".to_string(),
            cost_usd: Some(0.004),
        })
        .expect("json");
        assert_eq!(
            v,
            serde_json::json!({ "message": "feat: add thing", "costUsd": 0.004 })
        );

        let v = serde_json::to_value(CommitMessageProposal {
            message: "fix: bug".to_string(),
            cost_usd: None,
        })
        .expect("json");
        assert_eq!(
            v,
            serde_json::json!({ "message": "fix: bug", "costUsd": null })
        );
    }
}
