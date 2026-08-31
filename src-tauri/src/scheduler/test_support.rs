//! Shared fixtures for the scheduler integration tests: scratch repos +
//! `file://` remotes built with the real `git` CLI, a collecting event sink,
//! and the tick/run-now drivers. Split out of the test module for size.

use std::path::PathBuf;
use std::process::Command;
use std::sync::Mutex as StdMutex;

use super::*;

pub(super) const MIN: i64 = 60_000;

// ---------------- tick/job integration (real repos, injected time) ----

/// Scratch dir under `D:\Temp\bonsai-scratch` on Windows (MEMORY rule —
/// never C:). On macOS/Linux there is no such constraint, so scratch
/// dirs fall back to `std::env::temp_dir()/bonsai-scratch`.
#[cfg(windows)]
pub(super) fn scratch_root() -> std::path::PathBuf {
    std::path::PathBuf::from("D:\\Temp\\bonsai-scratch")
}

#[cfg(not(windows))]
pub(super) fn scratch_root() -> std::path::PathBuf {
    std::env::temp_dir().join("bonsai-scratch")
}

pub(super) fn scratch_dir() -> tempfile::TempDir {
    let root = scratch_root();
    std::fs::create_dir_all(&root).expect("create scratch root");
    tempfile::Builder::new()
        .prefix("bonsai-sched-")
        .tempdir_in(&root)
        .expect("scratch dir")
}

/// Builds a `file://` URL for a local path. On POSIX the path already
/// starts with `/`, so `file://` + path gives the correct 3-slash form;
/// prepending a bare `file:///` unconditionally (as Windows drive paths
/// need) double-slashes it into `file:////...`, which libgit2 rejects as
/// "not a valid local file URI" even though the real `git` CLI tolerates
/// it — that mismatch masked as widespread scheduler test failures.
pub(super) fn file_url(path: &std::path::Path) -> String {
    let s = path.display().to_string().replace('\\', "/");
    if s.starts_with('/') {
        format!("file://{s}")
    } else {
        format!("file:///{s}")
    }
}

pub(super) fn git(dir: &std::path::Path, args: &[&str]) {
    let out = Command::new("git")
        .current_dir(dir)
        .args(args)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_TERMINAL_PROMPT", "0")
        .output()
        .expect("run git");
    assert!(
        out.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

pub(super) fn init_repo(dir: &std::path::Path) {
    git(dir, &["init", "-b", "main"]);
    git(dir, &["config", "user.name", "Test User"]);
    git(dir, &["config", "user.email", "test@example.com"]);
    git(dir, &["config", "commit.gpgsign", "false"]);
}

pub(super) fn commit_file(dir: &std::path::Path, rel: &str, contents: &str, msg: &str) {
    std::fs::write(dir.join(rel), contents).expect("write file");
    git(dir, &["add", "."]);
    git(dir, &["commit", "-m", msg]);
}

/// Collector emitter: appends every event to a shared Vec.
pub(super) fn collecting_emitter() -> (EmitFn, Arc<StdMutex<Vec<SchedulerEvent>>>) {
    let events: Arc<StdMutex<Vec<SchedulerEvent>>> = Arc::new(StdMutex::new(Vec::new()));
    let sink = events.clone();
    let emit: EmitFn = Arc::new(move |ev| {
        sink.lock().expect("events lock").push(ev);
    });
    (emit, events)
}

pub(super) fn job_statuses(events: &Arc<StdMutex<Vec<SchedulerEvent>>>) -> Vec<JobStatusChangedPayload> {
    events
        .lock()
        .expect("events lock")
        .iter()
        .filter_map(|e| match e {
            SchedulerEvent::JobStatus(p) => Some(p.clone()),
            _ => None,
        })
        .collect()
}

pub(super) fn repo_changed_count(events: &Arc<StdMutex<Vec<SchedulerEvent>>>) -> usize {
    events
        .lock()
        .expect("events lock")
        .iter()
        .filter(|e| matches!(e, SchedulerEvent::RepoChanged(_)))
        .count()
}

pub(super) fn sched_with_auto_fetch(interval_minutes: u32) -> SchedulerState {
    let sched = SchedulerState::default();
    apply_config(
        &sched,
        JobsConfig {
            auto_fetch: AutoFetch {
                enabled: true,
                interval_minutes,
            },
            health_refresh: HealthRefresh {
                enabled: false,
                interval_minutes: 30,
            },
        },
    );
    sched
}

pub(super) fn drive_tick(
    repos: &[(String, PathBuf)],
    sched: &SchedulerState,
    now_ms: i64,
    emit: &EmitFn,
) {
    let handles = tick_once(repos, sched, now_ms, emit);
    tauri::async_runtime::block_on(async {
        for h in handles {
            let _ = h.await;
        }
    });
}

/// Builds work repo + bare `file://` remote; pushes an initial commit and
/// fetches so `refs/remotes/origin/main` exists. Returns (work, bare, other
/// clone used to push new commits).
pub(super) fn fetch_fixture(root: &std::path::Path) -> (PathBuf, PathBuf, PathBuf) {
    let work = root.join("work");
    let bare = root.join("remote.git");
    let other = root.join("other");
    std::fs::create_dir_all(&work).expect("mkdir work");
    init_repo(&work);
    commit_file(&work, "a.txt", "one\n", "c1");
    git(root, &["init", "--bare", "-b", "main", "remote.git"]);
    let bare_url = file_url(&bare);
    git(&work, &["remote", "add", "origin", &bare_url]);
    git(&work, &["push", "-u", "origin", "main"]);
    git(root, &["clone", &bare_url, "other"]);
    git(&other, &["config", "user.name", "Test User"]);
    git(&other, &["config", "user.email", "test@example.com"]);
    git(&other, &["config", "commit.gpgsign", "false"]);
    (work, bare, other)
}

pub(super) fn rev_parse(dir: &std::path::Path, rev: &str) -> String {
    let out = Command::new("git")
        .current_dir(dir)
        .args(["rev-parse", rev])
        .output()
        .expect("rev-parse");
    assert!(out.status.success(), "rev-parse {rev} failed");
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}
