//! Commit core (M3 contract §2.4).
//!
//! Pure git2 logic, no Tauri types. Author/committer come from git config —
//! clear `ConfigMissing` error naming each missing key, never a fallback
//! identity. Empty commits are rejected (`NothingToCommit`).

use std::path::Path;

use crate::error::AppError;
use crate::git::stage::open_workdir_repo;

/// Result of a successful commit.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommitResult {
    /// Full 40-char hex oid of the new commit.
    pub oid: String,
    /// First line of the cleaned message.
    pub summary: String,
    /// Branch HEAD points at after the commit ("main"); `None` when detached.
    pub branch: Option<String>,
}

/// Example value shown in the `git config` hint for a missing identity key.
fn key_example(key: &str) -> &'static str {
    if key == "user.name" {
        "\"Your Name\""
    } else {
        "\"you@example.com\""
    }
}

/// Reads `user.name` / `user.email` from `cfg` (a repo config snapshot —
/// includes local, global, system levels). Missing or empty value(s) ->
/// `AppError::ConfigMissing` with a message that NAMES each missing key, e.g.
/// `git identity not configured: user.email is not set. Run: git config
/// --global user.email "you@example.com"` (both missing -> both keys named in
/// one message). Never falls back to a default identity.
pub fn resolve_signature(cfg: &git2::Config) -> Result<git2::Signature<'static>, AppError> {
    let read = |key: &str| {
        cfg.get_string(key)
            .ok()
            .filter(|v| !v.trim().is_empty())
    };
    let name = read("user.name");
    let email = read("user.email");

    if let (Some(name), Some(email)) = (&name, &email) {
        return Ok(git2::Signature::now(name, email)?);
    }

    let mut missing: Vec<&str> = Vec::new();
    if name.is_none() {
        missing.push("user.name");
    }
    if email.is_none() {
        missing.push("user.email");
    }
    let keys = missing.join(" and ");
    let verb = if missing.len() > 1 { "are" } else { "is" };
    let commands = missing
        .iter()
        .map(|k| format!("git config --global {k} {}", key_example(k)))
        .collect::<Vec<_>>()
        .join(" and ");
    Err(AppError::ConfigMissing(format!(
        "git identity not configured: {keys} {verb} not set. Run: {commands}"
    )))
}

/// Blocking. Creates a commit from the current index (M3 contract §2.4 —
/// exact step order, cheap checks first). On unborn HEAD, git2 creates the
/// branch HEAD symbolically points at (first-commit flow).
pub fn create_commit(workdir: &Path, message: &str) -> Result<CommitResult, AppError> {
    let repo = open_workdir_repo(workdir)?;
    let mut index = repo.index()?;

    if index.has_conflicts() {
        return Err(AppError::Git(
            "cannot commit: unresolved conflicts".to_string(),
        ));
    }

    // Normalize line endings BEFORE trim: `git commit -m` (cleanup=whitespace)
    // strips the trailing `\r` of every line, so interior CRLF/CR must not
    // survive into the commit object (stray ^M in other clients otherwise).
    let normalized = message.replace("\r\n", "\n").replace('\r', "\n");
    let msg = normalized.trim();
    if msg.is_empty() {
        return Err(AppError::EmptyMessage);
    }

    let sig = resolve_signature(&repo.config()?.snapshot()?)?;

    let tree_oid = index.write_tree()?;

    let head = match repo.head() {
        Ok(h) => Some(h.peel_to_commit()?),
        Err(e)
            if matches!(
                e.code(),
                git2::ErrorCode::UnbornBranch | git2::ErrorCode::NotFound
            ) =>
        {
            None
        }
        Err(e) => return Err(e.into()),
    };

    match &head {
        Some(h) if h.tree_id() == tree_oid => return Err(AppError::NothingToCommit),
        None if index.is_empty() => return Err(AppError::NothingToCommit),
        _ => {}
    }

    let full = format!("{msg}\n");
    let tree = repo.find_tree(tree_oid)?;
    let parents: Vec<&git2::Commit> = head.iter().collect();
    let oid = repo.commit(Some("HEAD"), &sig, &sig, &full, &tree, &parents)?;

    let branch = repo
        .head()
        .ok()
        .filter(|h| h.is_branch())
        .and_then(|h| h.shorthand().map(String::from));
    let summary = msg.lines().next().unwrap_or(msg).to_string();

    Ok(CommitResult {
        oid: oid.to_string(),
        summary,
        branch,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Builds a fully isolated `git2::Config` backed by a single scratch file
    /// (no env, no global/system config visible), pre-populated with `entries`.
    fn isolated_config(entries: &[(&str, &str)]) -> (tempfile::TempDir, git2::Config) {
        let dir = crate::testutil::scratch_dir();
        let file = dir.path().join("gitconfig");
        std::fs::write(&file, "").expect("create empty config file");
        let mut cfg = git2::Config::open(&file).expect("open isolated config");
        for (key, value) in entries {
            cfg.set_str(key, value).expect("set config entry");
        }
        (dir, cfg)
    }

    fn expect_config_missing(cfg: &git2::Config, expected_keys: &[&str], absent_keys: &[&str]) {
        let err = match resolve_signature(cfg) {
            Ok(_) => panic!("must be ConfigMissing, got a signature"),
            Err(e) => e,
        };
        match err {
            AppError::ConfigMissing(m) => {
                for key in expected_keys {
                    assert!(m.contains(key), "message must name {key}, got: {m}");
                }
                for key in absent_keys {
                    assert!(!m.contains(key), "message must NOT name {key}, got: {m}");
                }
                assert!(m.contains("git identity not configured"), "got: {m}");
                assert!(m.contains("git config --global"), "got: {m}");
            }
            other => panic!("expected ConfigMissing, got: {other:?}"),
        }
    }

    #[test]
    fn resolve_signature_both_missing() {
        let (_dir, cfg) = isolated_config(&[]);
        expect_config_missing(&cfg, &["user.name", "user.email"], &[]);
    }

    #[test]
    fn resolve_signature_only_name_set() {
        let (_dir, cfg) = isolated_config(&[("user.name", "Test User")]);
        expect_config_missing(&cfg, &["user.email"], &["user.name"]);
    }

    #[test]
    fn resolve_signature_only_email_set() {
        let (_dir, cfg) = isolated_config(&[("user.email", "test@example.com")]);
        expect_config_missing(&cfg, &["user.name"], &["user.email"]);
    }

    #[test]
    fn resolve_signature_empty_values_count_as_missing() {
        let (_dir, cfg) = isolated_config(&[("user.name", ""), ("user.email", "")]);
        expect_config_missing(&cfg, &["user.name", "user.email"], &[]);
    }

    /// CRLF / lone-CR line endings are normalized to `\n` before trim, so no
    /// carriage return ever reaches the commit object (matches `git commit
    /// -m` cleanup=whitespace CR handling).
    #[test]
    fn create_commit_normalizes_crlf_and_lone_cr() {
        let dir = crate::testutil::scratch_dir();
        let repo = git2::Repository::init(dir.path()).expect("init repo");
        {
            let mut cfg = repo.config().expect("open config");
            cfg.set_str("user.name", "Test User").expect("set name");
            cfg.set_str("user.email", "test@example.com").expect("set email");
        }

        let head_message = |expect: &str| {
            let head = repo.head().expect("HEAD");
            let commit = head.peel_to_commit().expect("HEAD commit");
            assert_eq!(commit.message(), Some(expect));
        };

        // CRLF input (Windows textarea), incl. a trailing lone \r.
        std::fs::write(dir.path().join("a.txt"), "one\n").expect("write a.txt");
        crate::git::stage::stage_paths(dir.path(), &["a.txt".to_string()]).expect("stage");
        let res = create_commit(dir.path(), "subject line\r\nsecond line\r").expect("commit");
        assert_eq!(res.summary, "subject line");
        head_message("subject line\nsecond line\n");

        // Lone-CR interior endings.
        std::fs::write(dir.path().join("a.txt"), "two\n").expect("modify a.txt");
        crate::git::stage::stage_paths(dir.path(), &["a.txt".to_string()]).expect("stage");
        let res = create_commit(dir.path(), "first\rsecond\rthird").expect("commit");
        assert_eq!(res.summary, "first");
        head_message("first\nsecond\nthird\n");

        // A CR-only message is whitespace after normalization -> EmptyMessage.
        std::fs::write(dir.path().join("a.txt"), "three\n").expect("modify a.txt");
        crate::git::stage::stage_paths(dir.path(), &["a.txt".to_string()]).expect("stage");
        let err = create_commit(dir.path(), "\r\n\r").expect_err("CR-only message");
        assert!(matches!(err, AppError::EmptyMessage), "got: {err:?}");
    }

    #[test]
    fn resolve_signature_both_set() {
        let (_dir, cfg) = isolated_config(&[
            ("user.name", "Test User"),
            ("user.email", "test@example.com"),
        ]);
        let sig = resolve_signature(&cfg).expect("signature");
        assert_eq!(sig.name(), Some("Test User"));
        assert_eq!(sig.email(), Some("test@example.com"));
    }
}
