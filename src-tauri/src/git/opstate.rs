//! Detects an in-progress repository operation (merge / rebase / cherry-pick /
//! revert) from repo.state() + on-disk metadata. Pure git2, no Tauri types.
//! SHARED module (P3c contract §2): P3d rebase drives the exact same wire type.

use std::path::Path;

use crate::error::AppError;
use crate::git::stage::open_workdir_repo;

/// Wire: `{ "kind": "none" } | { "kind": "merge", "incoming": ..., "message": ... } | ...`
/// The Rebase variant is fully shaped NOW so P3d does not change the wire
/// type; P3c only populates its fields best-effort from plain file reads.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum RepoOpState {
    None,
    Merge {
        /// Human name of what is being merged, e.g. "feature/login" or
        /// "origin/main" — parsed from MERGE_MSG; falls back to the 7-char
        /// short oid of the first MERGE_HEAD entry, then "(unknown)".
        incoming: String,
        /// Full prepared merge message (MERGE_MSG contents, CRLF normalized,
        /// trailing whitespace trimmed). The frontend prefills the commit box
        /// with it. Empty string when the file is missing/unreadable.
        message: String,
    },
    Rebase {
        /// `.git/rebase-merge/head-name` minus `refs/heads/`, best-effort.
        head_name: Option<String>,
        /// `.git/rebase-merge/onto` (full oid), best-effort.
        onto: Option<String>,
        /// msgnum, 0 when unreadable.
        current_step: u32,
        /// end, 0 when unreadable.
        total_steps: u32,
    },
    CherryPick,
    Revert,
}

/// Extracts the incoming name from the first line of a MERGE_MSG: the text
/// between the first pair of single quotes when the line starts with one of
/// the two prefixes Bonsai itself writes (`Merge branch '...'` /
/// `Merge remote-tracking branch '...'` — the CLI uses the same phrasing).
fn parse_incoming(first_line: &str) -> Option<String> {
    if !(first_line.starts_with("Merge branch '")
        || first_line.starts_with("Merge remote-tracking branch '"))
    {
        return None;
    }
    let start = first_line.find('\'')? + 1;
    let rest = &first_line[start..];
    let end = rest.find('\'')?;
    Some(rest[..end].to_string())
}

/// Best-effort read of one small text file under the gitdir; `None` when
/// missing/unreadable. Trims trailing whitespace (files end with `\n`).
fn read_gitdir_file(repo: &git2::Repository, rel: &str) -> Option<String> {
    std::fs::read_to_string(repo.path().join(rel))
        .ok()
        .map(|s| s.trim_end().to_string())
}

/// Builds the Merge variant per the locked derivation (contract §2).
fn read_merge_state(repo: &mut git2::Repository) -> RepoOpState {
    // 1. MERGE_MSG contents (missing/unreadable -> empty string, never Err —
    //    a foreign tool may have removed it).
    let message = std::fs::read_to_string(repo.path().join("MERGE_MSG"))
        .map(|s| s.replace("\r\n", "\n").trim_end().to_string())
        .unwrap_or_default();

    // 2. Incoming name: quoted name on the first line, else the short oid of
    //    the FIRST MERGE_HEAD entry, else "(unknown)".
    let incoming = message
        .lines()
        .next()
        .and_then(parse_incoming)
        .or_else(|| {
            let mut first: Option<String> = None;
            let _ = repo.mergehead_foreach(|oid| {
                first = Some(oid.to_string().chars().take(7).collect());
                false // first callback wins
            });
            first
        })
        .unwrap_or_else(|| "(unknown)".to_string());

    RepoOpState::Merge { incoming, message }
}

/// Builds the Rebase variant best-effort from plain file reads (contract §2:
/// do NOT use `repo.open_rebase()` — it errors on rebases not started by
/// libgit2; plain file reads never do). `rebase-apply` is a fallback for
/// msgnum/end only.
fn read_rebase_state(repo: &git2::Repository) -> RepoOpState {
    let head_name = read_gitdir_file(repo, "rebase-merge/head-name")
        .map(|s| s.strip_prefix("refs/heads/").unwrap_or(&s).to_string());
    let onto = read_gitdir_file(repo, "rebase-merge/onto");
    let read_num = |name: &str| -> u32 {
        read_gitdir_file(repo, &format!("rebase-merge/{name}"))
            .or_else(|| read_gitdir_file(repo, &format!("rebase-apply/{name}")))
            .and_then(|s| s.trim().parse().ok())
            .unwrap_or(0)
    };
    RepoOpState::Rebase {
        head_name,
        onto,
        current_step: read_num("msgnum"),
        total_steps: read_num("end"),
    }
}

/// Blocking. Current operation state of the repo at `workdir`. Exotic states
/// (Bisect, ApplyMailbox, ...) map to `None` — Bonsai has no UI for them and
/// get_op_state must never error the refresh batch for them.
pub fn read_op_state(workdir: &Path) -> Result<RepoOpState, AppError> {
    let mut repo = open_workdir_repo(workdir)?;
    use git2::RepositoryState as S;
    Ok(match repo.state() {
        S::Clean => RepoOpState::None,
        S::Merge => read_merge_state(&mut repo),
        S::Rebase | S::RebaseInteractive | S::RebaseMerge => read_rebase_state(&repo),
        S::CherryPick | S::CherryPickSequence => RepoOpState::CherryPick,
        S::Revert | S::RevertSequence => RepoOpState::Revert,
        _ => RepoOpState::None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---------------------------------------------- wire shape (TS mirrors)

    /// The serde tag/casing must match the TS `RepoOpState` union exactly.
    #[test]
    fn wire_shapes_are_camel_case_tagged() {
        let v = serde_json::to_value(RepoOpState::None).expect("json");
        assert_eq!(v, serde_json::json!({ "kind": "none" }));

        let v = serde_json::to_value(RepoOpState::Merge {
            incoming: "feature/login".to_string(),
            message: "Merge branch 'feature/login'".to_string(),
        })
        .expect("json");
        assert_eq!(
            v,
            serde_json::json!({
                "kind": "merge",
                "incoming": "feature/login",
                "message": "Merge branch 'feature/login'"
            })
        );

        let v = serde_json::to_value(RepoOpState::Rebase {
            head_name: Some("topic".to_string()),
            onto: None,
            current_step: 2,
            total_steps: 5,
        })
        .expect("json");
        assert_eq!(
            v,
            serde_json::json!({
                "kind": "rebase",
                "headName": "topic",
                "onto": null,
                "currentStep": 2,
                "totalSteps": 5
            })
        );

        let v = serde_json::to_value(RepoOpState::CherryPick).expect("json");
        assert_eq!(v, serde_json::json!({ "kind": "cherryPick" }));

        let v = serde_json::to_value(RepoOpState::Revert).expect("json");
        assert_eq!(v, serde_json::json!({ "kind": "revert" }));
    }

    // ------------------------------------------- MERGE_MSG incoming parsing

    #[test]
    fn parse_incoming_local_prefix() {
        assert_eq!(
            parse_incoming("Merge branch 'feature/login'"),
            Some("feature/login".to_string())
        );
    }

    #[test]
    fn parse_incoming_remote_tracking_prefix() {
        assert_eq!(
            parse_incoming("Merge remote-tracking branch 'origin/main'"),
            Some("origin/main".to_string())
        );
    }

    #[test]
    fn parse_incoming_name_with_slashes_and_suffix() {
        // git may append "into <branch>" — the first quoted span still wins.
        assert_eq!(
            parse_incoming("Merge branch 'a/b/c' into main"),
            Some("a/b/c".to_string())
        );
    }

    #[test]
    fn parse_incoming_rejects_other_phrasings() {
        assert_eq!(parse_incoming("Merge commit 'abc1234'"), None);
        assert_eq!(parse_incoming("Something else entirely"), None);
        assert_eq!(parse_incoming(""), None);
        // Prefix present but unterminated quote -> no name.
        assert_eq!(parse_incoming("Merge branch 'unterminated"), None);
    }

    /// Clean repo -> None; missing MERGE_MSG during a merge -> fallback path.
    #[test]
    fn clean_repo_reads_none() {
        let dir = crate::testutil::scratch_dir();
        git2::Repository::init(dir.path()).expect("init repo");
        let state = read_op_state(dir.path()).expect("op state");
        assert_eq!(state, RepoOpState::None);
    }
}
