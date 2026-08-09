//! T2 Area 1 (pass B) — repo lifecycle / scheduler / mcp-guard / recents /
//! session command inners. Where a command is AppHandle-/State-bound with no
//! `_inner` seam (recents, session, set_active_repo, register_mcp), the exact
//! logic it wraps (`settings::*`, `AppState` fields, `mcp::status_of`,
//! `resolve_register_cwd`) is exercised directly — noted per case.

use super::tests_support::*;
use super::*;
use crate::scheduler::{JobKind, SchedulerState};

fn block_on<F: std::future::Future>(f: F) -> F::Output {
    tauri::async_runtime::block_on(f)
}

// ============================================================ open_repo_inner

/// Opening a plain (non-repo) directory reports `is_repo == false` and inserts
/// NO entry; opening a SUBFOLDER of a repo is ALSO `is_repo == false` (Bonsai
/// opens exact workdir roots only — `read_repo_info` does no upward discovery),
/// so it too creates no entry and never shadows the root.
#[test]
fn open_non_repo_and_subfolder_are_not_repos() {
    let state = AppState::default();

    // Non-repo dir → not a repo, no entry.
    let plain = tempfile::TempDir::new().expect("plain dir");
    let n = open(&state, plain.path()).expect("open non-repo");
    assert!(!n.info.is_repo);
    assert_eq!(repo_count(&state), 0);

    // Repo + a nested subfolder: the subfolder is reported not-a-repo.
    let (dir, _root_id, _c0) = fixture_repo(&state);
    assert_eq!(repo_count(&state), 1);
    let sub = dir.path().join("nested").join("deep");
    std::fs::create_dir_all(&sub).expect("mkdir nested");

    let opened = open(&state, &sub).expect("open subfolder");
    assert!(!opened.info.is_repo, "a subfolder is not itself a workdir root");
    assert_eq!(repo_count(&state), 1, "subfolder open adds no entry, root untouched");
}

/// BUG-1 (fixed) probe: opening a repo whose directory name is NON-ASCII, then
/// re-opening via a case-variant of that path, focuses the SAME entry via
/// `same_repo_path`'s canonicalize compare (the old `eq_ignore_ascii_case`
/// missed non-ASCII case folding). On a case-sensitive FS the variant simply
/// won't canonicalize to the same path and the probe self-skips.
#[test]
fn open_dedupes_non_ascii_case_variant() {
    let state = AppState::default();
    let parent = tempfile::TempDir::new().expect("parent");
    let repo_dir = parent.path().join("Übung");
    std::fs::create_dir_all(&repo_dir).expect("mkdir Übung");
    git2::Repository::init(&repo_dir).expect("init");

    let first = open(&state, &repo_dir).expect("open Übung").repo_id;
    assert_eq!(repo_count(&state), 1);

    let variant = parent.path().join("übung");
    // Only meaningful when the FS folds case to the same real path.
    let same_on_disk = matches!(
        (std::fs::canonicalize(&repo_dir), std::fs::canonicalize(&variant)),
        (Ok(a), Ok(b)) if a == b
    );
    if !same_on_disk {
        eprintln!("skipping: case-sensitive FS, non-ASCII variant is a distinct path");
        return;
    }
    let again = open(&state, &variant).expect("re-open case variant").repo_id;
    assert_eq!(again, first, "non-ASCII case variant dedupes to the same id");
    assert_eq!(repo_count(&state), 1, "no duplicate entry");
}

/// close_repo_inner is idempotent: closing an open repo removes it; a second
/// close of the same id is still Ok and the map is unchanged.
#[test]
fn close_repo_is_idempotent() {
    let state = AppState::default();
    let (_dir, id, _c0) = fixture_repo(&state);
    assert_eq!(repo_count(&state), 1);

    block_on(close_repo_inner(&state, &id)).expect("close");
    assert_eq!(repo_count(&state), 0);
    block_on(close_repo_inner(&state, &id)).expect("second close is a no-op Ok");
    assert_eq!(repo_count(&state), 0);
}

// ============================================================ init_repo

/// init_repo (direct command call — not repo-scoped) creates a usable repo at a
/// fresh path and is idempotent (re-init opens the existing repo).
#[test]
fn init_repo_creates_and_is_idempotent() {
    let dir = tempfile::TempDir::new().expect("dir");
    let path = path_string(dir.path());

    let workdir = block_on(init_repo(path.clone())).expect("init");
    assert!(git2::Repository::open(&workdir).is_ok(), "init produced a real repo");

    let again = block_on(init_repo(path)).expect("re-init opens the existing repo");
    assert_eq!(
        std::fs::canonicalize(&again).unwrap(),
        std::fs::canonicalize(&workdir).unwrap(),
        "re-init returns the same workdir"
    );
}

// ============================================================ set_active_repo (state field)

/// set_active_repo persists the focused-tab id into `AppState.active_repo`
/// (the command is `State`-bound with no `_inner`; its whole effect is this
/// field write, exercised directly here).
#[test]
fn active_repo_field_round_trips() {
    let state = AppState::default();
    assert!(state.active_repo.lock().unwrap().is_none(), "none by default");

    *state.active_repo.lock().unwrap() = Some("repo-x".to_string());
    assert_eq!(state.active_repo.lock().unwrap().as_deref(), Some("repo-x"));

    *state.active_repo.lock().unwrap() = None;
    assert!(state.active_repo.lock().unwrap().is_none(), "cleared");
}

// ============================================================ get_repo_health

/// get_repo_health resolves all four sections for an open repo and is NoRepo
/// for an unknown id.
#[test]
fn repo_health_sections_and_no_repo() {
    let state = AppState::default();
    let (_dir, id, _c0) = fixture_repo(&state);

    let health = block_on(get_repo_health_inner(&state, &id)).expect("health");
    assert!(health.stats.data.is_some(), "{:?}", health.stats.error);
    assert!(health.branches.data.is_some(), "{:?}", health.branches.error);
    assert!(health.working_state.data.is_some(), "{:?}", health.working_state.error);
    assert!(health.structure.data.is_some(), "{:?}", health.structure.error);

    let err = block_on(get_repo_health_inner(&state, MISSING_ID)).expect_err("no repo");
    assert!(matches!(err, AppError::NoRepo), "{err:?}");
}

// ============================================================ get_job_status

/// get_job_status_inner returns exactly the two background jobs (AutoFetch,
/// HealthRefresh) for an open repo, and NoRepo for an unknown id.
#[test]
fn job_status_both_jobs_and_unknown_repo() {
    let state = AppState::default();
    let sched = SchedulerState::default();
    let (_dir, id, _c0) = fixture_repo(&state);

    let jobs = get_job_status_inner(&state, &sched, &id).expect("job status");
    assert_eq!(jobs.len(), 2, "exactly two jobs");
    let kinds: Vec<JobKind> = jobs.iter().map(|j| j.job).collect();
    assert!(kinds.contains(&JobKind::AutoFetch) && kinds.contains(&JobKind::HealthRefresh));
    // A never-run job has no last_run and is not in backoff.
    assert!(jobs.iter().all(|j| j.last_run_ms.is_none() && !j.in_backoff));

    let err = get_job_status_inner(&state, &sched, MISSING_ID).expect_err("no repo");
    assert!(matches!(err, AppError::NoRepo), "{err:?}");
}

// ============================================================ register_mcp guard

/// register_mcp_with_claude refuses when the embedded server is not running.
/// The command has no `_inner`; its guard reads `mcp::status_of`, whose
/// `url`/`token` are `None` on a default (never-started) server — the exact
/// precondition that yields the "MCP server is not running" error. Its cwd
/// precheck (`resolve_register_cwd`) is asserted for a missing dir too.
#[test]
fn register_mcp_guard_not_running_and_bad_cwd() {
    let mcp_state = crate::mcp::McpServerState::default();
    let status = crate::mcp::status_of(&mcp_state);
    assert!(
        status.url.is_none() && status.token.is_none(),
        "a default MCP server is not running ⇒ register refuses"
    );

    let err = resolve_register_cwd(Some("D:/no/such/dir/xyzzy".to_string()))
        .expect_err("missing cwd dir");
    assert!(matches!(err, AppError::Io(_)), "{err:?}");
}

// ============================================================ recents (settings layer)

/// get/remove recents round-trip through the persisted settings file. The
/// commands are AppHandle-bound; their bodies are `settings::record_recent` /
/// `settings::update` + a `retain` — exercised directly against a temp file.
#[test]
fn recents_record_and_remove_round_trip() {
    let dir = tempfile::TempDir::new().expect("dir");
    let file = dir.path().join("settings.json");

    // record two opens (newest first), then a re-open of the first dedupes it
    // back to the front.
    settings::update(&file, |s| settings::record_recent(s, "D:\\Repos\\alpha", 100)).expect("rec1");
    settings::update(&file, |s| settings::record_recent(s, "D:\\Repos\\beta", 200)).expect("rec2");
    settings::update(&file, |s| settings::record_recent(s, "D:\\Repos\\alpha", 300)).expect("rec3");

    let recents = settings::load_from(&file).recent_repos;
    assert_eq!(recents.len(), 2, "dedupe keeps one entry per path");
    assert_eq!(recents[0].path, "D:\\Repos\\alpha", "re-open moves it to the front");
    assert_eq!(recents[0].last_opened, 300);

    // remove_recent_repo's body: retain everything not matching the path.
    let after = settings::update(&file, |s| {
        s.recent_repos.retain(|r| !r.path.eq_ignore_ascii_case("D:\\Repos\\alpha"));
    })
    .expect("remove");
    assert_eq!(after.recent_repos.len(), 1);
    assert_eq!(after.recent_repos[0].path, "D:\\Repos\\beta");
}

// ============================================================ session (settings layer)

/// get/set_session round-trip through the persisted settings file. The
/// commands are AppHandle-bound; their bodies write/read `open_repos` +
/// `active_repo` — exercised directly here, defaulting to an empty session.
#[test]
fn session_round_trip() {
    let dir = tempfile::TempDir::new().expect("dir");
    let file = dir.path().join("settings.json");

    // A fresh settings file defaults to an empty session.
    let fresh = settings::load_from(&file);
    assert!(fresh.open_repos.is_empty() && fresh.active_repo.is_none());

    let tabs = vec!["D:\\a".to_string(), "D:\\b".to_string()];
    settings::update(&file, |s| {
        s.open_repos = tabs.clone();
        s.active_repo = Some("D:\\b".to_string());
    })
    .expect("set session");

    let loaded = settings::load_from(&file);
    assert_eq!(loaded.open_repos, tabs, "tabs persist in order");
    assert_eq!(loaded.active_repo.as_deref(), Some("D:\\b"), "active tab persists");
}
