//! Reflog read path (P38 contract §4).
//!
//! Pure git2 logic, no Tauri types. `read_reflog` is BLOCKING (wrapped in
//! `spawn_blocking` at the command layer). This module is **READ-ONLY**: it
//! contains ZERO mutation code. The two P38 restore actions ("Create branch
//! here" / "Reset current branch to this") dispatch the already-shipped
//! `create_branch_here` / `reset_branch` commands — no mutation primitive lives
//! here (contract §2 invariant).

use std::path::Path;

use crate::error::AppError;
use crate::git::stage::open_workdir_repo;

/// Hard cap on reflog entries returned (newest-first). A deeper log is truncated
/// rather than streamed — streaming reflog over a channel is a later item.
pub const MAX_REFLOG_ENTRIES: usize = 2000;

/// One reflog entry (contract §4.2). Serialize camelCase.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReflogEntry {
    /// The N in `<ref>@{N}` (0 == newest / current tip).
    pub index: u32,
    /// 40-hex of the ref position BEFORE this entry.
    pub old_oid: String,
    /// 40-hex of the ref position AFTER this entry (restore actions target this).
    pub new_oid: String,
    /// Committer display name (lossy UTF-8).
    pub committer_name: String,
    /// Committer email (lossy UTF-8).
    pub committer_email: String,
    /// Committer time, seconds since the Unix epoch (UTC).
    pub committer_ts: i64,
    /// Reflog message, e.g. "commit: ...", "reset: moving to ...",
    /// "rebase (finish): ...", "commit (amend): ..." (lossy UTF-8; may be empty).
    pub message: String,
}

fn lossy(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

/// Blocking. Reads the reflog for `ref_name` ("HEAD" or a plain local branch
/// name like "main"), newest-first, capped at `MAX_REFLOG_ENTRIES`.
///
/// `ref_name`:
///   - "HEAD"          -> reads `.git/logs/HEAD`.
///   - "refs/..."      -> used verbatim (already a full ref name).
///   - "<branch>"      -> reads `refs/heads/<branch>` (the fn prepends the
///     `refs/heads/` prefix).
///
/// A ref that has never been updated (no reflog on disk) yields `Ok(vec![])`,
/// NOT an error (contract §2). The 40-zero `old_oid` of a freshly-created ref's
/// first entry is kept as-is; the frontend renders it as `(root)`.
///
/// Errors: `NoRepo` (open) | `Git` (unexpected libgit2 failure).
pub fn read_reflog(workdir: &Path, ref_name: &str) -> Result<Vec<ReflogEntry>, AppError> {
    let repo = open_workdir_repo(workdir)?;

    let full = if ref_name == "HEAD" || ref_name.starts_with("refs/") {
        ref_name.to_string()
    } else {
        format!("refs/heads/{ref_name}")
    };

    let reflog = match repo.reflog(&full) {
        Ok(r) => r,
        // A valid-but-never-updated ref has no reflog on disk -> empty vec.
        Err(e) if e.code() == git2::ErrorCode::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e.into()),
    };

    let cap = reflog.len().min(MAX_REFLOG_ENTRIES);
    let mut out: Vec<ReflogEntry> = Vec::with_capacity(cap);

    for (i, entry) in reflog.iter().enumerate() {
        if out.len() >= MAX_REFLOG_ENTRIES {
            break;
        }
        let committer = entry.committer();
        out.push(ReflogEntry {
            index: i as u32,
            old_oid: entry.id_old().to_string(),
            new_oid: entry.id_new().to_string(),
            committer_name: lossy(committer.name_bytes()),
            committer_email: lossy(committer.email_bytes()),
            committer_ts: committer.when().seconds(),
            message: entry.message_bytes().map(lossy).unwrap_or_default(),
        });
    }

    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Inits a repo at `dir` on branch `main` with a deterministic committer
    /// identity (`initial_head` pins the branch so the prefixing test is stable
    /// regardless of the host's `init.defaultBranch`).
    fn init_repo(dir: &Path) -> git2::Repository {
        let mut opts = git2::RepositoryInitOptions::new();
        opts.initial_head("main");
        let repo = git2::Repository::init_opts(dir, &opts).expect("init repo");
        {
            let mut cfg = repo.config().expect("open config");
            cfg.set_str("user.name", "Test User").expect("set name");
            cfg.set_str("user.email", "test@example.com").expect("set email");
        }
        repo
    }

    /// Stages `name` (created with `content`) and commits it via the shipped
    /// commit path, which writes a HEAD reflog entry.
    fn commit_file(dir: &Path, name: &str, content: &str, msg: &str) -> String {
        std::fs::write(dir.join(name), content).expect("write file");
        crate::git::stage::stage_paths(dir, &[name.to_string()]).expect("stage");
        crate::git::commit::create_commit(dir, msg, None, false).expect("commit").oid
    }

    /// `ReflogEntry` serializes with EXACTLY the camelCase keys the TS wire type
    /// declares (contract §4.5 test 1). Guards the TS wire type.
    #[test]
    fn reflog_entry_wire_shape_is_camel_case() {
        let v = serde_json::to_value(ReflogEntry {
            index: 0,
            old_oid: "0000000000000000000000000000000000000000".to_string(),
            new_oid: "abc".to_string(),
            committer_name: "Ada".to_string(),
            committer_email: "ada@example.com".to_string(),
            committer_ts: 1_700_000_000,
            message: "commit: base".to_string(),
        })
        .expect("json");
        assert_eq!(
            v,
            serde_json::json!({
                "index": 0,
                "oldOid": "0000000000000000000000000000000000000000",
                "newOid": "abc",
                "committerName": "Ada",
                "committerEmail": "ada@example.com",
                "committerTs": 1_700_000_000i64,
                "message": "commit: base"
            })
        );
    }

    /// HEAD reflog after two commits: >=2 entries, newest-first, index 0 == the
    /// current HEAD oid, and the newest message mentions "commit" (contract
    /// §4.5 test 2).
    #[test]
    fn read_reflog_head_after_commits() {
        let dir = crate::testutil::scratch_dir();
        let path = dir.path();
        init_repo(path);

        commit_file(path, "a.txt", "one\n", "c1: base");
        let head2 = commit_file(path, "a.txt", "two\n", "c2: edit");

        let entries = read_reflog(path, "HEAD").expect("reflog ok");
        assert!(entries.len() >= 2, "at least two reflog entries, got {}", entries.len());
        assert_eq!(entries[0].index, 0, "index 0 is newest");
        assert_eq!(entries[0].new_oid, head2, "newest new_oid == current HEAD");
        assert!(
            entries[0].committer_ts >= entries[1].committer_ts,
            "newest-first by committer time"
        );
        assert!(
            entries[0].message.contains("commit"),
            "newest message mentions commit, got {:?}",
            entries[0].message
        );
    }

    /// A valid-but-never-updated ref name yields `Ok(vec![])`, NOT an error
    /// (contract §4.5 test 3).
    #[test]
    fn read_reflog_missing_ref_is_empty() {
        let dir = crate::testutil::scratch_dir();
        let path = dir.path();
        init_repo(path);

        let entries = read_reflog(path, "nonexistent").expect("missing ref is Ok");
        assert!(entries.is_empty(), "never-updated ref -> empty vec");
    }

    /// Calling on a plain temp dir (no repo) errors (contract §4.5 test 4).
    /// The shared `open_workdir_repo` surfaces libgit2's "could not find
    /// repository" as `Git` at the core level; the command layer maps an unknown
    /// repo id to `NoRepo` earlier (see `read_reflog_requires_an_open_repo`).
    #[test]
    fn read_reflog_no_repo_errors() {
        let dir = crate::testutil::scratch_dir();
        let err = read_reflog(dir.path(), "HEAD").expect_err("no repo must error");
        assert!(matches!(err, AppError::Git(_)), "got {err:?}");
    }

    /// The `refs/heads/` prefix path: a branch reflog's newest new_oid matches
    /// HEAD's newest new_oid (contract §4.5 test 5).
    #[test]
    fn read_reflog_branch_prefixing() {
        let dir = crate::testutil::scratch_dir();
        let path = dir.path();
        init_repo(path);

        commit_file(path, "a.txt", "one\n", "c1: base");
        let head_tip = commit_file(path, "a.txt", "two\n", "c2: edit");

        let head = read_reflog(path, "HEAD").expect("head reflog");
        let main = read_reflog(path, "main").expect("branch reflog");
        assert!(!main.is_empty(), "main has a reflog");
        assert_eq!(main[0].new_oid, head[0].new_oid, "branch tip == HEAD tip");
        assert_eq!(main[0].new_oid, head_tip);
    }
}
