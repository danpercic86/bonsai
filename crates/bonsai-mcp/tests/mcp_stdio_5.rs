//! End-to-end proof of the MERGE hook gate (review 2026-09-11 MUST-FIX),
//! driven through the real binary over stdio against real scratch repos.
//!
//! `bonsai_merge_branch`'s clean auto-merge runs the repository's `commit-msg`
//! hook. That used to happen with NO gate while the server's own instructions
//! claimed commits were refused — so these tests assert both halves: the typed
//! refusal the model sees, and that a refused merge left the repository exactly
//! as it was (no commit, no MERGE_HEAD, no stash, hook never executed).
//!
//! Its own file rather than appended to the 471-line `mcp_stdio_4.rs`
//! (file-size discipline). The gate's placement — and that FF / up-to-date
//! merges are never refused — is also covered at unit level in
//! `bonsai-core`'s `merge::hook_gate_tests`, which is cheaper; here we prove the
//! MCP wiring picks the gate up.

mod common;

use common::{ok_structured, McpClient};
use serde_json::json;

/// `{ kind, message }` of a refused tool call.
fn refusal(resp: &serde_json::Value) -> (String, String) {
    let sc = common::err_structured(resp);
    let get = |key: &str| {
        sc.get(key)
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string()
    };
    (get("kind"), get("message"))
}

/// An executable hook that records having run, so "the hook never executed"
/// is an assertion about the filesystem rather than about our own bookkeeping.
fn write_witness_hook(repo: &std::path::Path, name: &str) {
    let dir = repo.join(".git/hooks");
    std::fs::create_dir_all(&dir).expect("mkdir hooks");
    let path = dir.join(name);
    std::fs::write(
        &path,
        format!("#!/bin/sh\necho ran > \"$(git rev-parse --show-toplevel)/{name}-ran.txt\"\nexit 0\n"),
    )
    .expect("write hook");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&path).expect("meta").permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&path, perms).expect("chmod");
    }
}

/// `main` and `topic` diverge from a shared base, so merging `topic` is a
/// NORMAL merge that auto-commits (and would run `commit-msg`).
fn diverged_repo() -> tempfile::TempDir {
    let repo = common::init_repo();
    let p = repo.path();
    common::build_linear(p, 1);
    common::git(p, &["checkout", "-b", "topic"]);
    common::write_file(p, "topic.txt", "topic\n");
    common::git(p, &["add", "-A"]);
    common::git(p, &["commit", "-m", "topic side"]);
    common::git(p, &["checkout", "main"]);
    common::write_file(p, "main.txt", "main\n");
    common::git(p, &["add", "-A"]);
    common::git(p, &["commit", "-m", "main side"]);
    repo
}

/// `topic` is strictly ahead of `main`, so merging it fast-forwards: no commit,
/// no hook, and therefore no refusal.
fn ff_repo() -> tempfile::TempDir {
    let repo = common::init_repo();
    let p = repo.path();
    common::build_linear(p, 1);
    common::git(p, &["checkout", "-b", "topic"]);
    common::write_file(p, "feature.txt", "feature\n");
    common::git(p, &["add", "-A"]);
    common::git(p, &["commit", "-m", "topic advance"]);
    common::git(p, &["checkout", "main"]);
    repo
}

/// A real (non-FF) merge in a repo with a runnable `commit-msg` hook is
/// REFUSED with `hooksNotPermitted`, and the repository is untouched: HEAD
/// unchanged, no MERGE_HEAD, no autostash of the dirty file, the dirty edit
/// still on disk, and the hook never ran.
#[test]
fn merge_refuses_a_commit_msg_hook_and_changes_nothing() {
    if common::skip_if_no_git() {
        return;
    }
    let repo = diverged_repo();
    let p = repo.path();
    write_witness_hook(p, "commit-msg");
    // Dirty tracked file: what an autostash would have swept away.
    common::write_file(p, "f.txt", "locally edited\n");
    let head_before = common::git(p, &["rev-parse", "HEAD"]);
    let refs_before = common::ref_snapshot(p);

    let mut c = McpClient::connect(p, true);
    let (kind, message) = refusal(&c.call_tool("bonsai_merge_branch", json!({ "name": "topic" })));
    assert_eq!(kind, "hooksNotPermitted", "{message}");
    assert!(
        message.contains("commit-msg"),
        "must name the hook that would run: {message}"
    );
    assert!(
        !message.contains("pre-commit") && !message.contains("post-commit"),
        "a merge never fires those, so naming them would be wrong: {message}"
    );
    assert!(
        message.contains("--allow-hooks"),
        "must name the consent flag: {message}"
    );

    assert_eq!(common::git(p, &["rev-parse", "HEAD"]), head_before);
    assert_eq!(common::ref_snapshot(p), refs_before, "no ref may move");
    assert!(
        !p.join(".git/MERGE_HEAD").exists(),
        "a refusal must not park the repo mid-merge"
    );
    assert_eq!(
        common::git(p, &["stash", "list"]).trim(),
        "",
        "a refusal must not leave an autostash behind"
    );
    assert_eq!(
        std::fs::read_to_string(p.join("f.txt")).expect("read f.txt"),
        "locally edited\n",
        "the dirty edit must still be in the worktree"
    );
    assert!(
        !p.join("topic.txt").exists(),
        "nothing from the incoming branch may be checked out"
    );
    assert!(
        !p.join("commit-msg-ran.txt").exists(),
        "the hook must not have executed"
    );
}

/// `--allow-hooks` is the explicit consent: the same merge then lands and the
/// hook does run.
#[test]
fn merge_with_allow_hooks_proceeds_and_runs_the_hook() {
    if common::skip_if_no_git() {
        return;
    }
    let repo = diverged_repo();
    let p = repo.path();
    write_witness_hook(p, "commit-msg");

    let mut c = McpClient::connect_with(p, true, &["--allow-hooks"]);
    let out = ok_structured(&c.call_tool("bonsai_merge_branch", json!({ "name": "topic" })));
    assert_eq!(
        out.get("kind").and_then(|v| v.as_str()),
        Some("merged"),
        "{out:?}"
    );
    assert!(
        p.join("commit-msg-ran.txt").exists(),
        "with consent the hook must run"
    );
}

/// A FAST-FORWARD merge creates no commit, so it runs no hook and must NOT be
/// refused even with a `commit-msg` hook installed and no `--allow-hooks`. This
/// is the over-refusal the naive "gate before the call" fix would have caused.
#[test]
fn merge_does_not_refuse_a_fast_forward() {
    if common::skip_if_no_git() {
        return;
    }
    let repo = ff_repo();
    let p = repo.path();
    write_witness_hook(p, "commit-msg");

    let mut c = McpClient::connect(p, true);
    let out = ok_structured(&c.call_tool("bonsai_merge_branch", json!({ "name": "topic" })));
    assert_eq!(
        out.get("kind").and_then(|v| v.as_str()),
        Some("fastForwarded"),
        "{out:?}"
    );
    assert!(
        p.join("feature.txt").exists(),
        "the fast-forward must actually have happened"
    );
    assert!(
        !p.join("commit-msg-ran.txt").exists(),
        "a fast-forward runs no hook"
    );
}

/// The probe is MERGE-scoped: a repo with only a `pre-commit` hook runs nothing
/// on the auto-merge commit, so the merge proceeds. (`bonsai_commit` in the same
/// repo would still be refused — that hook does fire there.)
#[test]
fn merge_does_not_refuse_a_pre_commit_only_repo() {
    if common::skip_if_no_git() {
        return;
    }
    let repo = diverged_repo();
    let p = repo.path();
    write_witness_hook(p, "pre-commit");

    let mut c = McpClient::connect(p, true);
    let out = ok_structured(&c.call_tool("bonsai_merge_branch", json!({ "name": "topic" })));
    assert_eq!(
        out.get("kind").and_then(|v| v.as_str()),
        Some("merged"),
        "a hook a merge never fires must not block it: {out:?}"
    );
    assert!(
        !p.join("pre-commit-ran.txt").exists(),
        "the auto-merge commit does not fire pre-commit"
    );

    // …while the commit tool in the very same repo IS refused for it. (The
    // hook gate runs before any nothing-to-commit check, so this holds whatever
    // the stage call did — staging is here only to make the scenario realistic.)
    common::write_file(p, "new.txt", "new\n");
    c.call_tool("bonsai_stage", json!({ "paths": ["new.txt"] }));
    let (kind, message) = refusal(&c.call_tool("bonsai_commit", json!({ "message": "agent" })));
    assert_eq!(kind, "hooksNotPermitted", "{message}");
    assert!(message.contains("pre-commit"), "{message}");
}

/// `bonsai.runHooks=false` means nothing would run, so there is nothing to
/// disclose and the merge proceeds without consent.
#[test]
fn merge_proceeds_when_the_repo_opted_out_of_hooks() {
    if common::skip_if_no_git() {
        return;
    }
    let repo = diverged_repo();
    let p = repo.path();
    write_witness_hook(p, "commit-msg");
    common::git(p, &["config", "bonsai.runHooks", "false"]);

    let mut c = McpClient::connect(p, true);
    let out = ok_structured(&c.call_tool("bonsai_merge_branch", json!({ "name": "topic" })));
    assert_eq!(
        out.get("kind").and_then(|v| v.as_str()),
        Some("merged"),
        "{out:?}"
    );
    assert!(
        !p.join("commit-msg-ran.txt").exists(),
        "the opt-out must also mean the hook did not run"
    );
}
