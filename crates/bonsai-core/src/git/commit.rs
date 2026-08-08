//! Commit core (M3 contract §2.4).
//!
//! Pure git2 logic, no Tauri types. Author/committer come from git config —
//! clear `ConfigMissing` error naming each missing key, never a fallback
//! identity. Empty commits are rejected (`NothingToCommit`).

use std::path::Path;

use crate::error::AppError;
use crate::git::bisect::require_no_bisect;
use crate::git::exec::SpawnGitExec;
use crate::git::hooks::{hooks_enabled, run_hook, run_hook_nonblocking, HookName};
use crate::git::signing::{self, resolve_signing};
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
///
/// `sign` (P58 D3): `None` ⇒ follow effective `commit.gpgsign` (git default
/// false); `Some(true)` ⇒ force sign; `Some(false)` ⇒ force unsigned. When
/// signing is NOT resolved, the commit is created by git2 EXACTLY as before P58
/// (byte-identical, no `gpgsig` header); signing branches to
/// [`signing::create_signed_commit`] (`git commit-tree -S` + `git update-ref`).
///
/// `skip_hooks` (P59a): `true` ≡ `git commit --no-verify`. Otherwise the effective
/// toggle is `bonsai.runHooks` (default true). When enabled, git's hook order is
/// honoured: `pre-commit` (before `write_tree`, may re-stage) → `commit-msg` (may
/// rewrite the message) → create the commit → `post-commit` (non-blocking). A
/// BLOCKING hook's non-zero exit aborts as [`AppError::HookRejected`] — no commit,
/// no ref move.
// TODO(P59): fold `sign` + `skip_hooks` into a `CommitOpts` struct (2nd per-flag
// fan-out) instead of growing positional bools.
pub fn create_commit(
    workdir: &Path,
    message: &str,
    sign: Option<bool>,
    skip_hooks: bool,
) -> Result<CommitResult, AppError> {
    let repo = open_workdir_repo(workdir)?;

    // A Bonsai bisect runs on a clean detached HEAD, so `state()` below can't
    // see it — refuse a commit while one is active (would move the branch ref).
    require_no_bisect(&repo)?;

    // P3c contract §4.5 backend guard: a plain commit mid-merge would create
    // a 1-parent commit and silently drop MERGE_HEAD ancestry.
    if repo.state() != git2::RepositoryState::Clean {
        return Err(AppError::OperationInProgress(
            "an operation is in progress — use 'Commit merge' or abort it".to_string(),
        ));
    }

    // One config snapshot drives the hook toggle, identity, and signing.
    let cfg = repo.config()?.snapshot()?;
    let hooks = hooks_enabled(&cfg, skip_hooks);

    // pre-commit runs BEFORE write_tree (git order); a non-zero exit aborts with
    // HookRejected before anything is written or any ref moves.
    if hooks {
        run_hook(&SpawnGitExec, workdir, HookName::PreCommit, &[], None)?;
    }

    let mut index = repo.index()?;
    if hooks {
        // Reload from disk so a hook that re-staged (formatter, generator) is
        // included in the committed tree.
        index.read(true)?;
    }

    if index.has_conflicts() {
        return Err(AppError::Git(
            "cannot commit: unresolved conflicts".to_string(),
        ));
    }

    let mut msg = normalize_message(message);
    if msg.is_empty() {
        return Err(AppError::EmptyMessage);
    }

    // commit-msg may REWRITE the message file (trailer/template); re-read after.
    if hooks {
        msg = run_commit_msg_hook(&repo, workdir, &msg)?;
    }

    // Identity: ConfigMissing surfaces before any object/index write.
    let sig = resolve_signature(&cfg)?;

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
    let summary = msg.lines().next().unwrap_or(&msg).to_string();

    let signing = resolve_signing(&cfg, sign);
    let oid = if !signing.sign {
        // Unsigned path: byte-identical to pre-P58.
        let tree = repo.find_tree(tree_oid)?;
        let parents: Vec<&git2::Commit> = head.iter().collect();
        repo.commit(Some("HEAD"), &sig, &sig, &full, &tree, &parents)?
    } else {
        let parent_oids: Vec<git2::Oid> = head.iter().map(git2::Commit::id).collect();
        let old_head = head.as_ref().map(git2::Commit::id);
        signing::create_signed_commit(
            &SpawnGitExec,
            workdir,
            tree_oid,
            &parent_oids,
            &sig,
            &sig,
            &full,
            old_head,
            &format!("commit: {summary}"),
        )?
    };

    // post-commit is best-effort: the commit already landed — never block on it.
    if hooks {
        let _ = run_hook_nonblocking(&SpawnGitExec, workdir, HookName::PostCommit, &[]);
    }

    let branch = branch_shorthand_after(&repo, workdir, signing.sign)?;

    Ok(CommitResult {
        oid: oid.to_string(),
        summary,
        branch,
    })
}

/// Resolve the branch HEAD points at after a commit. The unsigned git2 path
/// updated `repo`'s own refdb, so `repo.head()` is authoritative; the signed
/// path moved HEAD via an EXTERNAL `git update-ref`, so re-open a fresh handle
/// to avoid a stale (possibly still-unborn) refdb view. `None` ⇒ detached HEAD.
fn branch_shorthand_after(
    repo: &git2::Repository,
    workdir: &Path,
    signed: bool,
) -> Result<Option<String>, AppError> {
    let read = |r: &git2::Repository| {
        r.head()
            .ok()
            .filter(|h| h.is_branch())
            .and_then(|h| h.shorthand().ok().map(String::from))
    };
    if signed {
        Ok(read(&open_workdir_repo(workdir)?))
    } else {
        Ok(read(repo))
    }
}

/// Normalize a message for the commit object: CRLF / lone-CR → `\n`, then trim.
/// `git commit -m` (cleanup=whitespace) strips a trailing `\r` on every line, so
/// interior CRLF/CR must not survive into the object (stray ^M in other clients).
/// Shared by create/amend and the `commit-msg` re-read.
pub(crate) fn normalize_message(message: &str) -> String {
    message
        .replace("\r\n", "\n")
        .replace('\r', "\n")
        .trim()
        .to_string()
}

/// Write `msg` to the repo's `COMMIT_EDITMSG`, run the `commit-msg` hook with
/// that file as `$1` (git's contract), then re-read + re-normalize the (possibly
/// rewritten) message. Empty after the hook ⇒ [`AppError::EmptyMessage`]. Shared
/// by create/amend (P59a) and the merge-commit finalize.
pub(crate) fn run_commit_msg_hook(
    repo: &git2::Repository,
    workdir: &Path,
    msg: &str,
) -> Result<String, AppError> {
    let msg_file = repo.path().join("COMMIT_EDITMSG");
    std::fs::write(&msg_file, format!("{msg}\n"))?;
    let arg = msg_file.to_string_lossy().into_owned();
    run_hook(
        &SpawnGitExec,
        workdir,
        HookName::CommitMsg,
        std::slice::from_ref(&arg),
        None,
    )?;
    let rewritten = std::fs::read_to_string(&msg_file)?;
    let out = normalize_message(&rewritten);
    if out.is_empty() {
        return Err(AppError::EmptyMessage);
    }
    Ok(out)
}

/// Blocking. Replaces HEAD with a new commit built from the current index, on
/// HEAD's EXISTING parents (preserves merge parents), reusing HEAD's ORIGINAL
/// author and stamping a fresh committer. `message` is the final message (the
/// frontend prefills + lets the user edit HEAD's message). Mirrors
/// `git commit --amend -m <message>` (P20 contract §2.1).
///
/// `sign` (P58 D3): as [`create_commit`]. The unsigned path is byte-identical
/// to pre-P58 (`Commit::amend`); the signed path rebuilds on HEAD's ORIGINAL
/// parents via [`signing::create_signed_commit`] (preserving the original
/// author + author date, re-stamping the committer).
///
/// `skip_hooks` (P59a): as [`create_commit`]. git runs the commit hooks on an
/// amend too — `pre-commit` → `commit-msg` → (amend) → `post-commit`.
pub fn amend_commit(
    workdir: &Path,
    message: &str,
    sign: Option<bool>,
    skip_hooks: bool,
) -> Result<CommitResult, AppError> {
    let repo = open_workdir_repo(workdir)?;

    // A clean detached-HEAD bisect is invisible to `state()` below — refuse.
    require_no_bisect(&repo)?;

    // Amending mid-merge/rebase/pick is nonsense — refuse before any read.
    if repo.state() != git2::RepositoryState::Clean {
        return Err(AppError::OperationInProgress(
            "an operation is in progress — finish or abort it first".to_string(),
        ));
    }

    // HEAD commit to amend. Unborn / missing HEAD → nothing to amend.
    let head_commit = match repo.head() {
        Ok(h) => h.peel_to_commit()?,
        Err(e)
            if matches!(
                e.code(),
                git2::ErrorCode::UnbornBranch | git2::ErrorCode::NotFound
            ) =>
        {
            return Err(AppError::Git(
                "nothing to amend: the repository has no commits yet".to_string(),
            ));
        }
        Err(e) => return Err(e.into()),
    };

    let cfg = repo.config()?.snapshot()?;
    let hooks = hooks_enabled(&cfg, skip_hooks);

    // pre-commit BEFORE write_tree (may re-stage); non-zero ⇒ HookRejected, abort.
    if hooks {
        run_hook(&SpawnGitExec, workdir, HookName::PreCommit, &[], None)?;
    }

    // Tree from the current index. NO NothingToCommit guard — a message-only
    // amend (tree == HEAD's tree, 0 staged) is valid.
    let mut index = repo.index()?;
    if hooks {
        index.read(true)?; // pick up any hook re-staging
    }

    // Normalize line endings before trim, identical to `create_commit`.
    let mut msg = normalize_message(message);
    if msg.is_empty() {
        return Err(AppError::EmptyMessage);
    }
    if hooks {
        msg = run_commit_msg_hook(&repo, workdir, &msg)?;
    }

    // Fresh committer (ConfigMissing before any write); original author preserved.
    let committer = resolve_signature(&cfg)?;
    let author = head_commit.author().to_owned();

    let tree_oid = index.write_tree()?;
    let tree = repo.find_tree(tree_oid)?;

    let full = format!("{msg}\n");
    let summary = msg.lines().next().unwrap_or(&msg).to_string();

    let signing = resolve_signing(&cfg, sign);
    let oid = if !signing.sign {
        // Unsigned path: byte-identical to pre-P58. `Commit::amend` REPLACES the
        // current commit onto its EXISTING parents (preserving merge parents) and
        // moves HEAD — unlike `repo.commit(Some("HEAD"), …)`, which would reject
        // the amend because the new first parent (HEAD^) is not the current tip.
        head_commit.amend(
            Some("HEAD"),
            Some(&author),
            Some(&committer),
            None,
            Some(&full),
            Some(&tree),
        )?
    } else {
        // Signed amend: rebuild on HEAD's ORIGINAL parents (not HEAD itself), with
        // the CAS old-oid = current HEAD so update-ref replaces the tip.
        let parent_oids: Vec<git2::Oid> = head_commit.parent_ids().collect();
        signing::create_signed_commit(
            &SpawnGitExec,
            workdir,
            tree_oid,
            &parent_oids,
            &author,
            &committer,
            &full,
            Some(head_commit.id()),
            &format!("commit (amend): {summary}"),
        )?
    };

    if hooks {
        let _ = run_hook_nonblocking(&SpawnGitExec, workdir, HookName::PostCommit, &[]);
    }

    let branch = branch_shorthand_after(&repo, workdir, signing.sign)?;

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
            assert_eq!(commit.message().ok(), Some(expect));
        };

        // CRLF input (Windows textarea), incl. a trailing lone \r.
        std::fs::write(dir.path().join("a.txt"), "one\n").expect("write a.txt");
        crate::git::stage::stage_paths(dir.path(), &["a.txt".to_string()]).expect("stage");
        let res = create_commit(dir.path(), "subject line\r\nsecond line\r", None, false).expect("commit");
        assert_eq!(res.summary, "subject line");
        head_message("subject line\nsecond line\n");

        // Lone-CR interior endings.
        std::fs::write(dir.path().join("a.txt"), "two\n").expect("modify a.txt");
        crate::git::stage::stage_paths(dir.path(), &["a.txt".to_string()]).expect("stage");
        let res = create_commit(dir.path(), "first\rsecond\rthird", None, false).expect("commit");
        assert_eq!(res.summary, "first");
        head_message("first\nsecond\nthird\n");

        // A CR-only message is whitespace after normalization -> EmptyMessage.
        std::fs::write(dir.path().join("a.txt"), "three\n").expect("modify a.txt");
        crate::git::stage::stage_paths(dir.path(), &["a.txt".to_string()]).expect("stage");
        let err = create_commit(dir.path(), "\r\n\r", None, false).expect_err("CR-only message");
        assert!(matches!(err, AppError::EmptyMessage), "got: {err:?}");
    }

    /// Amending an unborn HEAD refuses with a Git error before any write.
    #[test]
    fn amend_on_unborn_head_errors() {
        let dir = crate::testutil::scratch_dir();
        let repo = git2::Repository::init(dir.path()).expect("init repo");
        {
            let mut cfg = repo.config().expect("config");
            cfg.set_str("user.name", "Test User").expect("name");
            cfg.set_str("user.email", "test@example.com").expect("email");
        }
        let err = amend_commit(dir.path(), "msg", None, false).expect_err("unborn");
        match err {
            AppError::Git(m) => assert!(m.contains("no commits yet"), "got: {m}"),
            other => panic!("expected Git, got {other:?}"),
        }
    }

    /// A message-only amend (0 staged) succeeds, keeps the original author, and
    /// preserves the parent set; an empty message is rejected.
    #[test]
    fn amend_message_only_preserves_author_and_parents() {
        let dir = crate::testutil::scratch_dir();
        let repo = git2::Repository::init(dir.path()).expect("init repo");
        {
            let mut cfg = repo.config().expect("config");
            cfg.set_str("user.name", "Orig Author").expect("name");
            cfg.set_str("user.email", "orig@example.com").expect("email");
        }
        std::fs::write(dir.path().join("a.txt"), "one\n").expect("write");
        crate::git::stage::stage_paths(dir.path(), &["a.txt".to_string()]).expect("stage");
        create_commit(dir.path(), "original subject", None, false).expect("commit");
        let orig_tree = repo
            .head()
            .expect("head")
            .peel_to_commit()
            .expect("peel")
            .tree_id();

        // Empty message rejected.
        let err = amend_commit(dir.path(), "   ", None, false).expect_err("empty");
        assert!(matches!(err, AppError::EmptyMessage), "got: {err:?}");

        // Change committer identity so we can prove author is preserved.
        {
            let mut cfg = repo.config().expect("config");
            cfg.set_str("user.name", "New Committer").expect("name");
            cfg.set_str("user.email", "new@example.com").expect("email");
        }
        let res = amend_commit(dir.path(), "amended subject", None, false).expect("amend");
        assert_eq!(res.summary, "amended subject");

        let head = repo.head().expect("head").peel_to_commit().expect("peel");
        assert_eq!(head.tree_id(), orig_tree, "message-only amend keeps the tree");
        assert_eq!(head.parent_count(), 0, "root commit parents preserved (0)");
        assert_eq!(head.author().email().ok(), Some("orig@example.com"));
        assert_eq!(head.committer().email().ok(), Some("new@example.com"));
        assert_eq!(head.message().ok(), Some("amended subject\n"));
    }

    #[test]
    fn resolve_signature_both_set() {
        let (_dir, cfg) = isolated_config(&[
            ("user.name", "Test User"),
            ("user.email", "test@example.com"),
        ]);
        let sig = resolve_signature(&cfg).expect("signature");
        assert_eq!(sig.name().ok(), Some("Test User"));
        assert_eq!(sig.email().ok(), Some("test@example.com"));
    }
}
