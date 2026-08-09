//! Shared `#[cfg(test)]`-only helpers for the inline command tests
//! (`tests.rs` + the `tests_*` files). Hoisted from `tests.rs` (T2 Area 1) so
//! every test module builds its fixtures the same runtime-free way: plain
//! `AppState`, git2 scratch repos under `%TMP%`, and the no-op watcher factory
//! (`open_repo_inner(state, path, |_id| Box::new(|| {}))`).

use super::*;

pub(crate) const MISSING_ID: &str = "missing";

pub(crate) fn path_string(p: &std::path::Path) -> String {
    p.to_string_lossy().into_owned()
}

/// Opens `path` runtime-free with a no-op watcher factory (P3e contract
/// §9.1: `open_repo_inner(state, path, |_id| Box::new(|| {}))`).
pub(crate) fn open(
    state: &AppState,
    path: &std::path::Path,
) -> Result<OpenRepoResult, AppError> {
    tauri::async_runtime::block_on(open_repo_inner(
        state,
        path_string(path),
        |_id| Box::new(|| {}),
    ))
}

/// git2-init a repo with a committable identity; returns the temp dir.
pub(crate) fn init_repo_with_identity() -> tempfile::TempDir {
    let dir = tempfile::TempDir::new().expect("create temp dir");
    let repo = git2::Repository::init(dir.path()).expect("init repo");
    let mut cfg = repo.config().expect("open config");
    cfg.set_str("user.name", "Test User").expect("set user.name");
    cfg.set_str("user.email", "test@example.com")
        .expect("set user.email");
    // Deterministic byte-exact worktree contents on Windows: without this a
    // global `core.autocrlf=true` turns every checkout into CRLF and breaks
    // content assertions.
    cfg.set_bool("core.autocrlf", false)
        .expect("set core.autocrlf");
    dir
}

/// Writes `rel` under the workdir, stages it, and commits — via the command
/// inners, so the whole round-trip is keyed by `repo_id`.
pub(crate) fn write_stage_commit(
    state: &AppState,
    repo_id: &str,
    workdir: &std::path::Path,
    rel: &str,
    contents: &str,
    message: &str,
) -> CommitResult {
    std::fs::write(workdir.join(rel), contents).expect("write file");
    tauri::async_runtime::block_on(stage_inner(state, repo_id, vec![rel.to_string()]))
        .expect("stage");
    tauri::async_runtime::block_on(commit_inner(state, repo_id, message.to_string(), None, None))
        .expect("commit")
}

pub(crate) fn repo_count(state: &AppState) -> usize {
    state.repos.lock().expect("repos lock").len()
}

/// True when the `git` CLI is on PATH — the remotes/submodule twin-repo
/// fixtures shell out to it. Tests skip-with-note when it is absent.
pub(crate) fn have_git() -> bool {
    std::process::Command::new("git")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// `git <args>` in `dir`, asserting success; returns trimmed stdout. Used by
/// the file:// twin-repo fixtures (remotes/submodules) — never against a real
/// repo.
pub(crate) fn git(dir: &std::path::Path, args: &[&str]) -> String {
    let out = std::process::Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap_or_else(|e| panic!("run git {args:?}: {e}"));
    assert!(
        out.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

/// `file://` URL for a local path (libgit2-friendly 3-slash form; Windows
/// drive paths get the extra slash). Mirrors `bonsai-core`'s `common::file_url`.
pub(crate) fn file_url(path: &std::path::Path) -> String {
    let s = path.display().to_string().replace('\\', "/");
    if s.starts_with('/') {
        format!("file://{s}")
    } else {
        format!("file:///{s}")
    }
}

/// Full 40-hex oid of HEAD's target commit (panics on unborn — fixtures only).
pub(crate) fn head_oid(workdir: &std::path::Path) -> String {
    let repo = git2::Repository::open(workdir).expect("open repo");
    let head = repo.head().expect("HEAD");
    let commit = head.peel_to_commit().expect("HEAD commit");
    commit.id().to_string()
}

/// Shorthand name of the branch HEAD points at; `None` when detached/unborn.
pub(crate) fn head_branch(workdir: &std::path::Path) -> Option<String> {
    let repo = git2::Repository::open(workdir).expect("open repo");
    let head = repo.head().ok()?;
    if !repo.head_detached().unwrap_or(false) {
        head.shorthand().ok().map(str::to_owned)
    } else {
        None
    }
}

/// Convenience: init-with-identity + open + one initial commit. Returns
/// `(tempdir, repo_id, initial_commit_oid)` — the standard born-HEAD fixture.
pub(crate) fn fixture_repo(state: &AppState) -> (tempfile::TempDir, String, String) {
    let dir = init_repo_with_identity();
    let opened = open(state, dir.path()).expect("open repo");
    let c0 = write_stage_commit(state, &opened.repo_id, dir.path(), "a.txt", "base\n", "C0");
    (dir, opened.repo_id, c0.oid)
}
