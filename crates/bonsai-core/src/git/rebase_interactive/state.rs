//! On-disk interactive-rebase state persistence, message helpers, and plan
//! validation. Extracted verbatim from `rebase_interactive.rs` (file-size
//! discipline); the public command entry points and the replay engine live in
//! the module root and `engine`.

use crate::error::AppError;

use super::{InteractiveState, RebaseAction, RebaseTodoOp};

// ---------------------------------------------------------------- on-disk state

fn bonsai_dir(repo: &git2::Repository) -> std::path::PathBuf {
    repo.path().join("bonsai-rebase")
}

fn state_path(repo: &git2::Repository) -> std::path::PathBuf {
    bonsai_dir(repo).join("state.json")
}

/// True iff a Bonsai interactive rebase is in progress (state file present).
pub(crate) fn interactive_in_progress(repo: &git2::Repository) -> bool {
    state_path(repo).exists()
}

/// Why the state file could not be read (F-A3-2 / F-A3-4 mirror of `bisect`):
/// "no file" (no rebase) vs an io fault (surface the real error) vs "file
/// exists but undecodable" (salvageable corruption).
pub(super) enum StateReadError {
    Missing,
    Io(std::io::Error),
    Corrupt(serde_json::Error),
}

pub(super) fn read_state_raw(
    repo: &git2::Repository,
) -> Result<InteractiveState, StateReadError> {
    let raw = match std::fs::read_to_string(state_path(repo)) {
        Ok(r) => r,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Err(StateReadError::Missing),
        Err(e) => return Err(StateReadError::Io(e)),
    };
    serde_json::from_str(&raw).map_err(StateReadError::Corrupt)
}

/// Reads + parses `.git/bonsai-rebase/state.json`. Missing → "no rebase";
/// unreadable → the REAL io error; corrupt → `Git`.
pub(crate) fn read_state(repo: &git2::Repository) -> Result<InteractiveState, AppError> {
    read_state_raw(repo).map_err(|e| match e {
        StateReadError::Missing => {
            AppError::Git("interactive rebase state is missing".to_string())
        }
        StateReadError::Io(e) => {
            AppError::Git(format!("failed to read interactive rebase state: {e}"))
        }
        StateReadError::Corrupt(e) => {
            AppError::Git(format!("interactive rebase state is corrupt: {e}"))
        }
    })
}

/// Writes the state file (create_dir_all + temp-file rename for atomicity).
pub(super) fn write_state(
    repo: &git2::Repository,
    state: &InteractiveState,
) -> Result<(), AppError> {
    let dir = bonsai_dir(repo);
    std::fs::create_dir_all(&dir)?;
    let json = serde_json::to_string_pretty(state)
        .map_err(|e| AppError::Git(format!("failed to serialize rebase state: {e}")))?;
    let tmp = dir.join("state.json.tmp");
    std::fs::write(&tmp, json.as_bytes())?;
    std::fs::rename(&tmp, dir.join("state.json"))?;
    Ok(())
}

/// Removes `.git/bonsai-rebase/` (best-effort — a leftover dir is harmless and
/// `interactive_in_progress` keys on `state.json` specifically).
pub(super) fn remove_state(repo: &git2::Repository) {
    let _ = std::fs::remove_dir_all(bonsai_dir(repo));
}

/// Number of non-`Drop` todos == the "total steps" the UI shows.
pub(crate) fn effective_total(state: &InteractiveState) -> u32 {
    state
        .todos
        .iter()
        .filter(|t| t.action != RebaseAction::Drop)
        .count() as u32
}

// ---------------------------------------------------------------- message helpers

/// CRLF/CR -> `\n`, trim, single trailing newline (shared with cherrypick.rs /
/// commit.rs). Empty after trim -> empty string.
pub(super) fn normalize_message(raw: &str) -> String {
    let normalized = raw.replace("\r\n", "\n").replace('\r', "\n");
    let trimmed = normalized.trim();
    format!("{trimmed}\n")
}

/// Default squash message when `new_message` is None: `<head>\n\n<pick>`.
pub(super) fn concat_messages(head_msg: &str, pick_msg: &str) -> String {
    format!("{}\n\n{}", head_msg.trim(), pick_msg.trim())
}

/// A git2 error raised while APPLYING a pick: `Conflict` -> friendly
/// CheckoutConflict; else the generic `From` (`AppError::Git`).
pub(super) fn map_pick_err(e: git2::Error) -> AppError {
    if e.code() == git2::ErrorCode::Conflict {
        AppError::CheckoutConflict(
            "cannot rebase: local changes would be overwritten. Commit or discard them first."
                .to_string(),
        )
    } else {
        e.into()
    }
}

// ---------------------------------------------------------------- validation

/// Rejects a plan BEFORE any mutation (contract §2.6). Structural checks first
/// (so a bad shape never depends on oid resolution), then per-oid resolution.
pub(super) fn validate_todos(
    repo: &git2::Repository,
    todos: &[RebaseTodoOp],
) -> Result<(), AppError> {
    let first_kept = todos.iter().find(|t| t.action != RebaseAction::Drop);
    match first_kept {
        None => {
            return Err(AppError::Git(
                "nothing to rebase: the plan drops every commit".to_string(),
            ));
        }
        Some(op) => {
            if !matches!(op.action, RebaseAction::Pick | RebaseAction::Reword) {
                return Err(AppError::Git("a squash/fixup must follow a pick".to_string()));
            }
        }
    }

    for op in todos {
        if op.action == RebaseAction::Reword
            && op
                .new_message
                .as_ref()
                .map(|m| m.trim().is_empty())
                .unwrap_or(true)
        {
            return Err(AppError::Git("reword requires a message".to_string()));
        }
    }

    for op in todos {
        if op.action == RebaseAction::Drop {
            continue;
        }
        let oid = git2::Oid::from_str(&op.oid)
            .map_err(|_| AppError::Git("invalid commit id".to_string()))?;
        repo.find_commit(oid)?;
    }
    Ok(())
}
