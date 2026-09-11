//! End-to-end guards from the MCP tool-contract audit (2026-09-11), driven
//! through the real binary over stdio against a real scratch repo.
//!
//! Each test asserts BOTH halves of a refusal: the typed error the model sees,
//! and that the repository did not change (index / worktree / refs). The pure
//! decision logic behind these guards is unit-tested in
//! `src/server/write_guards/tests.rs`; this file proves the wiring.

mod common;

use common::{err_structured, ok_structured, McpClient};
use serde_json::json;

/// `{ kind, message }` of a refused tool call.
fn refusal(resp: &serde_json::Value) -> (String, String) {
    let sc = err_structured(resp);
    let kind = sc
        .get("kind")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string();
    let message = sc
        .get("message")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string();
    (kind, message)
}

/// Write an executable `#!/bin/sh` hook (git parses the shebang with its
/// bundled sh on Windows too).
fn write_hook(repo: &std::path::Path, name: &str, body: &str) {
    let dir = repo.join(".git/hooks");
    std::fs::create_dir_all(&dir).expect("mkdir hooks");
    let path = dir.join(name);
    std::fs::write(&path, body.replace("\r\n", "\n")).expect("write hook");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&path).expect("meta").permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&path, perms).expect("chmod");
    }
}

// ------------------------------------- MEDIUM: stage only what status offers

/// A GITIGNORED file is refused (`invalidName`) even though `index.add_path`
/// would happily force-add it, and nothing is staged — the audit's "a model
/// could stage and commit `.env`" path.
#[test]
fn stage_refuses_a_gitignored_path_and_stages_nothing() {
    if common::skip_if_no_git() {
        return;
    }
    let repo = common::init_repo();
    let p = repo.path();
    common::build_linear(p, 1);
    common::write_file(p, ".gitignore", ".env\n");
    common::git(p, &["add", ".gitignore"]);
    common::git(p, &["commit", "-m", "ignore .env"]);
    common::write_file(p, ".env", "SECRET_TOKEN=hunter2\n");

    let before = common::porcelain(p);
    let mut c = McpClient::connect(p, true);
    let resp = c.call_tool("bonsai_stage", json!({ "paths": [".env"] }));

    let (kind, message) = refusal(&resp);
    assert_eq!(kind, "invalidName", "{message}");
    assert!(message.contains(".env"), "must name the path: {message}");
    assert!(
        message.contains("bonsai_get_status"),
        "must point at status: {message}"
    );
    assert_eq!(
        common::porcelain(p),
        before,
        "the ignored file must not be staged"
    );
    assert!(
        !common::git(p, &["diff", "--cached", "--name-only"]).contains(".env"),
        "nothing may be in the index"
    );
}

/// A path that exists and is TRACKED but has no change is refused too: status
/// never offers it, so it is not stageable through this tool.
#[test]
fn stage_refuses_an_unchanged_tracked_path() {
    if common::skip_if_no_git() {
        return;
    }
    let repo = common::init_repo();
    let p = repo.path();
    common::build_linear(p, 1);

    let mut c = McpClient::connect(p, true);
    let (kind, message) = refusal(&c.call_tool("bonsai_stage", json!({ "paths": ["f.txt"] })));
    assert_eq!(kind, "invalidName", "{message}");
    assert!(common::porcelain(p).is_empty(), "tree must stay clean");
}

/// One bad path aborts the batch: the legitimate sibling is NOT staged either
/// (the tool's documented atomicity, now including the new precondition).
#[test]
fn stage_refuses_the_whole_batch_when_one_path_is_not_in_status() {
    if common::skip_if_no_git() {
        return;
    }
    let repo = common::init_repo();
    let p = repo.path();
    common::build_linear(p, 1);
    common::write_file(p, ".gitignore", ".env\n");
    common::git(p, &["add", "-A"]);
    common::git(p, &["commit", "-m", "ignore"]);
    common::write_file(p, ".env", "SECRET=1\n");
    common::write_file(p, "new.txt", "legit\n");

    let mut c = McpClient::connect(p, true);
    let (kind, _) = refusal(&c.call_tool(
        "bonsai_stage",
        json!({ "paths": ["new.txt", ".env"] }),
    ));
    assert_eq!(kind, "invalidName");
    assert!(
        common::git(p, &["diff", "--cached", "--name-only"]).is_empty(),
        "the legitimate path must not be staged either"
    );
}

/// No regression: the paths status DOES offer (untracked, modified, and the
/// deletion of a tracked file) still stage normally.
#[test]
fn stage_still_accepts_every_path_status_reports() {
    if common::skip_if_no_git() {
        return;
    }
    let repo = common::init_repo();
    let p = repo.path();
    common::build_linear(p, 1);
    common::write_file(p, "keep.txt", "v1\n");
    common::write_file(p, "gone.txt", "bye\n");
    common::git(p, &["add", "-A"]);
    common::git(p, &["commit", "-m", "fixture"]);

    common::write_file(p, "fresh.txt", "new file\n"); // untracked
    common::write_file(p, "keep.txt", "v2\n"); // modified
    std::fs::remove_file(p.join("gone.txt")).expect("delete tracked file"); // deleted

    let mut c = McpClient::connect(p, true);
    let resp = c.call_tool(
        "bonsai_stage",
        json!({ "paths": ["fresh.txt", "keep.txt", "gone.txt"] }),
    );
    ok_structured(&resp);

    let staged = common::git(p, &["diff", "--cached", "--name-status"]);
    assert!(staged.contains("fresh.txt"), "{staged}");
    assert!(staged.contains("keep.txt"), "{staged}");
    assert!(
        staged.contains('D') && staged.contains("gone.txt"),
        "the deletion of a tracked file must still stage: {staged}"
    );
}

// -------------------------------- LOW: no conflict markers may be staged

/// Build a repo paused on a `bothModified` conflict in `c.txt`.
fn conflicted_repo() -> tempfile::TempDir {
    let repo = common::init_repo();
    let p = repo.path();
    common::write_file(p, "c.txt", "base\n");
    common::git(p, &["add", "-A"]);
    common::git(p, &["commit", "-m", "base"]);
    common::git(p, &["checkout", "-b", "topic"]);
    common::write_file(p, "c.txt", "theirs\n");
    common::git(p, &["commit", "-am", "theirs"]);
    common::git(p, &["checkout", "main"]);
    common::write_file(p, "c.txt", "ours\n");
    common::git(p, &["commit", "-am", "ours"]);
    // Conflicting merge: `git merge` exits non-zero, so do not assert success.
    let _ = std::process::Command::new("git")
        .args(["merge", "topic"])
        .current_dir(p)
        .output()
        .expect("run git merge");
    repo
}

/// `bonsai_resolve_conflict_text` with marker-bearing content is refused
/// (`unresolvedConflicts`) and the worktree file is left exactly as it was —
/// the frontend's Save-button gate, re-established for a model caller.
#[test]
fn resolve_conflict_text_refuses_content_with_markers() {
    if common::skip_if_no_git() {
        return;
    }
    let repo = conflicted_repo();
    let p = repo.path();
    let before = std::fs::read_to_string(p.join("c.txt")).expect("read conflicted file");
    assert!(before.contains("<<<<<<<"), "fixture must be conflicted");

    let mut c = McpClient::connect(p, true);
    let (kind, message) = refusal(&c.call_tool(
        "bonsai_resolve_conflict_text",
        json!({
            "path": "c.txt",
            "content": "<<<<<<< HEAD\nours\n=======\ntheirs\n>>>>>>> topic\n",
        }),
    ));
    assert_eq!(kind, "unresolvedConflicts", "{message}");
    assert!(message.contains("c.txt"), "{message}");
    assert_eq!(
        std::fs::read_to_string(p.join("c.txt")).expect("read"),
        before,
        "the worktree file must be untouched"
    );
    assert!(
        !common::git(p, &["diff", "--name-only", "--diff-filter=U"]).is_empty(),
        "the path must still be conflicted (nothing was staged)"
    );
}

/// The same tool still accepts a clean merged resolution.
#[test]
fn resolve_conflict_text_accepts_clean_merged_content() {
    if common::skip_if_no_git() {
        return;
    }
    let repo = conflicted_repo();
    let p = repo.path();

    let mut c = McpClient::connect(p, true);
    ok_structured(&c.call_tool(
        "bonsai_resolve_conflict_text",
        json!({ "path": "c.txt", "content": "ours\ntheirs\n" }),
    ));
    assert_eq!(
        std::fs::read_to_string(p.join("c.txt")).expect("read"),
        "ours\ntheirs\n"
    );
    assert!(
        common::git(p, &["diff", "--name-only", "--diff-filter=U"]).is_empty(),
        "the conflict must be resolved and staged"
    );
}

/// `markResolved` stages the worktree file AS IS, so it is refused while that
/// file still holds markers (the second "trust the caller" site).
#[test]
fn mark_resolved_refuses_a_file_that_still_has_markers() {
    if common::skip_if_no_git() {
        return;
    }
    let repo = conflicted_repo();
    let p = repo.path();

    let mut c = McpClient::connect(p, true);
    let (kind, message) = refusal(&c.call_tool(
        "bonsai_resolve_conflict",
        json!({ "path": "c.txt", "resolution": "markResolved" }),
    ));
    assert_eq!(kind, "unresolvedConflicts", "{message}");
    assert!(
        !common::git(p, &["diff", "--name-only", "--diff-filter=U"]).is_empty(),
        "the path must still be conflicted"
    );
}

/// …but `markResolved` on a hand-cleaned file still works, and `ours` (which
/// writes a blob side, not caller text) is unaffected by the guard.
#[test]
fn mark_resolved_accepts_a_cleaned_file_and_ours_still_works() {
    if common::skip_if_no_git() {
        return;
    }
    let repo = conflicted_repo();
    let p = repo.path();
    common::write_file(p, "c.txt", "hand merged\n");

    let mut c = McpClient::connect(p, true);
    ok_structured(&c.call_tool(
        "bonsai_resolve_conflict",
        json!({ "path": "c.txt", "resolution": "markResolved" }),
    ));
    assert!(common::git(p, &["diff", "--name-only", "--diff-filter=U"]).is_empty());

    // `ours` on a fresh conflict: no caller-supplied text, no marker check.
    let repo2 = conflicted_repo();
    let mut c2 = McpClient::connect(repo2.path(), true);
    ok_structured(&c2.call_tool(
        "bonsai_resolve_conflict",
        json!({ "path": "c.txt", "resolution": "ours" }),
    ));
    assert_eq!(
        std::fs::read_to_string(repo2.path().join("c.txt")).expect("read"),
        "ours\n"
    );
}

/// The bypass: `bonsai_stage` on a conflicted path is `index.add_path` on the
/// worktree file as it stands — `markResolved` by another name — so it gets the
/// same marker gate. Without this, the two conflict-tool guards above would be
/// one tool call away from irrelevant.
#[test]
fn stage_refuses_a_conflicted_path_that_still_has_markers() {
    if common::skip_if_no_git() {
        return;
    }
    let repo = conflicted_repo();
    let p = repo.path();
    let before = std::fs::read_to_string(p.join("c.txt")).expect("read");
    assert!(before.contains("<<<<<<<"), "fixture must be conflicted");

    let mut c = McpClient::connect(p, true);
    let (kind, message) = refusal(&c.call_tool("bonsai_stage", json!({ "paths": ["c.txt"] })));
    assert_eq!(kind, "unresolvedConflicts", "{message}");
    assert!(
        !common::git(p, &["diff", "--name-only", "--diff-filter=U"]).is_empty(),
        "the path must still be conflicted (nothing was staged)"
    );
    assert_eq!(
        std::fs::read_to_string(p.join("c.txt")).expect("read"),
        before,
        "the worktree file must be untouched"
    );
}

/// …and capability is preserved: once the file holds no markers (here written
/// outside Bonsai, e.g. by the user's editor), staging it works and resolves
/// the conflict.
#[test]
fn stage_accepts_a_conflicted_path_once_the_markers_are_gone() {
    if common::skip_if_no_git() {
        return;
    }
    let repo = conflicted_repo();
    let p = repo.path();
    common::write_file(p, "c.txt", "hand merged
");

    let mut c = McpClient::connect(p, true);
    ok_structured(&c.call_tool("bonsai_stage", json!({ "paths": ["c.txt"] })));
    assert!(
        common::git(p, &["diff", "--name-only", "--diff-filter=U"]).is_empty(),
        "staging a cleaned conflicted file must resolve it"
    );
}

// ------------------------- LOW: undisclosed repository hooks on the MCP path

/// A standalone server has no frontend to show Bonsai's hook disclosure in, so
/// a commit in a repo whose `pre-commit` hook would run is REFUSED and no
/// commit is created. The hook itself never executes (it would have written a
/// witness file).
#[test]
fn commit_refuses_a_repo_with_runnable_hooks_without_allow_hooks() {
    if common::skip_if_no_git() {
        return;
    }
    let repo = common::init_repo();
    let p = repo.path();
    common::build_linear(p, 1);
    write_hook(
        p,
        "pre-commit",
        "#!/bin/sh\necho ran > \"$(git rev-parse --show-toplevel)/hook-ran.txt\"\nexit 0\n",
    );
    common::write_file(p, "staged.txt", "content\n");
    common::git(p, &["add", "staged.txt"]);
    let head_before = common::git(p, &["rev-parse", "HEAD"]);

    let mut c = McpClient::connect(p, true);
    let (kind, message) = refusal(&c.call_tool(
        "bonsai_commit",
        json!({ "message": "agent commit" }),
    ));
    assert_eq!(
        kind, "hooksNotPermitted",
        "a refusal the model can branch on, not the untyped `other` (review          2026-09-11): {message}"
    );
    assert!(message.contains("pre-commit"), "must name the hook: {message}");
    assert!(
        message.contains("--allow-hooks"),
        "must name the consent flag: {message}"
    );
    assert_eq!(
        common::git(p, &["rev-parse", "HEAD"]),
        head_before,
        "no commit may be created"
    );
    assert!(
        !p.join("hook-ran.txt").exists(),
        "the hook must not have executed"
    );
}

/// `--allow-hooks` is the explicit consent: the same commit then succeeds and
/// the hook does run.
#[test]
fn commit_with_allow_hooks_proceeds_and_runs_the_hook() {
    if common::skip_if_no_git() {
        return;
    }
    let repo = common::init_repo();
    let p = repo.path();
    common::build_linear(p, 1);
    write_hook(
        p,
        "pre-commit",
        "#!/bin/sh\necho ran > \"$(git rev-parse --show-toplevel)/hook-ran.txt\"\nexit 0\n",
    );
    common::write_file(p, "staged.txt", "content\n");
    common::git(p, &["add", "staged.txt"]);
    let head_before = common::git(p, &["rev-parse", "HEAD"]);

    let mut c = McpClient::connect_with(p, true, &["--allow-hooks"]);
    ok_structured(&c.call_tool("bonsai_commit", json!({ "message": "agent commit" })));
    assert_ne!(
        common::git(p, &["rev-parse", "HEAD"]),
        head_before,
        "the commit must land"
    );
    assert!(
        p.join("hook-ran.txt").exists(),
        "with consent, the hook runs (it is git's own behaviour)"
    );
}

/// A repo that opted out of hooks (`bonsai.runHooks=false`) is NOT refused:
/// nothing would execute, so there is nothing to disclose. Also the plain
/// no-hooks case must stay unaffected.
#[test]
fn commit_is_allowed_when_no_hook_would_run() {
    if common::skip_if_no_git() {
        return;
    }
    // (a) no hooks at all.
    let repo = common::init_repo();
    let p = repo.path();
    common::build_linear(p, 1);
    common::write_file(p, "a.txt", "a\n");
    common::git(p, &["add", "a.txt"]);
    let mut c = McpClient::connect(p, true);
    ok_structured(&c.call_tool("bonsai_commit", json!({ "message": "no hooks here" })));

    // (b) a hook is installed but the repo disabled Bonsai's hook execution.
    let repo2 = common::init_repo();
    let q = repo2.path();
    common::build_linear(q, 1);
    common::git(q, &["config", "bonsai.runHooks", "false"]);
    write_hook(q, "pre-commit", "#!/bin/sh\nexit 1\n");
    common::write_file(q, "b.txt", "b\n");
    common::git(q, &["add", "b.txt"]);
    let mut c2 = McpClient::connect(q, true);
    ok_structured(&c2.call_tool("bonsai_commit", json!({ "message": "hooks opted out" })));
}

/// A `pre-push`-only repo must NOT block commits: no commit path fires it.
#[test]
fn commit_is_allowed_when_only_a_pre_push_hook_exists() {
    if common::skip_if_no_git() {
        return;
    }
    let repo = common::init_repo();
    let p = repo.path();
    common::build_linear(p, 1);
    write_hook(p, "pre-push", "#!/bin/sh\nexit 1\n");
    common::write_file(p, "a.txt", "a\n");
    common::git(p, &["add", "a.txt"]);

    let mut c = McpClient::connect(p, true);
    ok_structured(&c.call_tool("bonsai_commit", json!({ "message": "pre-push is not a commit hook" })));
}
