//! T2 Area 1 (pass B) — remote command inners over `file://` twin-repo
//! fixtures. Remote-MANAGEMENT (list/add/remove/rename/set-url) is pure git2 and
//! always runs; the NETWORK ops (fetch/pull/push/force-push) shell out to the
//! `git` binary and skip cleanly when it is absent. All fixtures live under
//! `%TMP%` (= `D:\Data\Temp` on Windows) — never a real repo.

use super::tests_support::*;
use super::*;

fn block_on<F: std::future::Future>(f: F) -> F::Output {
    tauri::async_runtime::block_on(f)
}

macro_rules! require_git {
    () => {
        if !have_git() {
            eprintln!("skipping: git CLI not on PATH");
            return;
        }
    };
}

// ============================================================ remote management (git2)

/// add → list → set-url → rename → remove happy path, plus the three failure
/// paths: duplicate add (Git "already exists"), and remove/rename/set-url of a
/// missing remote (NoRemote).
#[test]
fn remote_management_crud_and_failures() {
    let state = AppState::default();
    let (_dir, id, _c0) = fixture_repo(&state);

    block_on(add_remote_inner(&state, &id, "origin".into(), "https://example.com/a.git".into()))
        .expect("add origin");
    let remotes = block_on(list_remotes_inner(&state, &id)).expect("list");
    assert!(remotes.iter().any(|r| r.name == "origin"), "origin listed");

    // Duplicate add → Git error.
    let err = block_on(add_remote_inner(&state, &id, "origin".into(), "https://example.com/b.git".into()))
        .expect_err("duplicate");
    assert!(matches!(err, AppError::Git(_)), "{err:?}");

    // set-url happy, then a missing remote → NoRemote.
    block_on(set_remote_url_inner(&state, &id, "origin".into(), "https://example.com/c.git".into()))
        .expect("set url");
    let url = block_on(list_remotes_inner(&state, &id)).unwrap()
        .into_iter().find(|r| r.name == "origin").unwrap().url;
    assert_eq!(url.as_deref(), Some("https://example.com/c.git"));
    let err = block_on(set_remote_url_inner(&state, &id, "ghost".into(), "https://x/y.git".into()))
        .expect_err("set-url missing");
    assert!(matches!(err, AppError::NoRemote(_)), "{err:?}");

    // rename happy, then a missing remote → NoRemote.
    block_on(rename_remote_inner(&state, &id, "origin".into(), "upstream".into())).expect("rename");
    assert!(block_on(list_remotes_inner(&state, &id)).unwrap().iter().any(|r| r.name == "upstream"));
    let err = block_on(rename_remote_inner(&state, &id, "ghost".into(), "x".into()))
        .expect_err("rename missing");
    assert!(matches!(err, AppError::NoRemote(_)), "{err:?}");

    // remove happy, then removing it again → NoRemote.
    block_on(remove_remote_inner(&state, &id, "upstream".into())).expect("remove");
    assert!(block_on(list_remotes_inner(&state, &id)).unwrap().is_empty());
    let err = block_on(remove_remote_inner(&state, &id, "upstream".into())).expect_err("remove missing");
    assert!(matches!(err, AppError::NoRemote(_)), "{err:?}");
}

// ============================================================ network fixtures

/// A local repo (C0 committed) with an `origin` bare remote it has already
/// pushed to (so its branch has an upstream). Returns (local, id, origin, branch).
fn pushed_local(state: &AppState) -> (tempfile::TempDir, String, tempfile::TempDir, String) {
    let (local, id, _c0) = fixture_repo(state);
    let branch = head_branch(local.path()).expect("attached head");
    let origin = tempfile::TempDir::new().expect("origin dir");
    git(origin.path(), &["init", "--bare"]);
    block_on(add_remote_inner(state, &id, "origin".into(), file_url(origin.path())))
        .expect("add origin");
    match block_on(push_inner(state, &id, None)).expect("initial push") {
        PushResult::Pushed { set_upstream, .. } => assert!(set_upstream, "first push sets upstream"),
        other => panic!("expected Pushed, got {other:?}"),
    }
    // Make origin's HEAD point at the pushed branch so a fresh clone checks it out.
    git(origin.path(), &["symbolic-ref", "HEAD", &format!("refs/heads/{branch}")]);
    (local, id, origin, branch)
}

/// Clone `origin`, add one commit, and push it back — advancing the remote
/// branch out from under the local repo (used to build FF / non-FF / stale
/// scenarios).
fn poke_advance(origin: &std::path::Path, branch: &str, file: &str) {
    let poke = tempfile::TempDir::new().expect("poke dir");
    git(poke.path(), &["clone", &file_url(origin), "wc"]);
    let wc = poke.path().join("wc");
    git(&wc, &["config", "user.name", "Poke"]);
    git(&wc, &["config", "user.email", "poke@example.com"]);
    std::fs::write(wc.join(file), "poke\n").expect("write poke file");
    git(&wc, &["add", "-A"]);
    git(&wc, &["commit", "-m", "poke advance"]);
    git(&wc, &["push", "origin", branch]);
}

// ============================================================ fetch

/// fetch with no configured remote → NoRemote; fetch after the remote advanced
/// reports updated refs.
#[test]
fn fetch_no_remote_and_happy() {
    require_git!();
    let state = AppState::default();

    let (_bare, id0, _c0) = fixture_repo(&state);
    let err = block_on(fetch_inner(&state, &id0)).expect_err("no remotes");
    assert!(matches!(err, AppError::NoRemote(_)), "{err:?}");

    let (_local, id, origin, branch) = pushed_local(&state);
    poke_advance(origin.path(), &branch, "adv.txt");
    let res = block_on(fetch_inner(&state, &id)).expect("fetch");
    assert!(
        res.remotes.iter().any(|r| r.updated_refs > 0),
        "the advanced remote-tracking ref updated"
    );
}

// ============================================================ pull

/// pull fast-forwards when the remote is strictly ahead; refuses (a RESULT, not
/// an error) when local and remote diverged; and reports NoUpstream when the
/// branch has no upstream configured.
#[test]
fn pull_ff_diverged_and_no_upstream() {
    require_git!();
    let state = AppState::default();

    // FF.
    let (_l1, id1, origin1, branch1) = pushed_local(&state);
    poke_advance(origin1.path(), &branch1, "ff.txt");
    match block_on(pull_inner(&state, &id1)).expect("pull ff") {
        PullResult::FastForwarded { .. } => {}
        other => panic!("expected FastForwarded, got {other:?}"),
    }

    // Diverged → WouldNotFastForward.
    let (l2, id2, origin2, branch2) = pushed_local(&state);
    write_stage_commit(&state, &id2, l2.path(), "local.txt", "local\n", "local divergence");
    poke_advance(origin2.path(), &branch2, "remote.txt");
    match block_on(pull_inner(&state, &id2)).expect("pull diverged") {
        PullResult::WouldNotFastForward { ahead, behind, .. } => {
            assert!(ahead > 0 && behind > 0, "genuinely diverged: ahead={ahead} behind={behind}");
        }
        other => panic!("expected WouldNotFastForward, got {other:?}"),
    }

    // No upstream (remote added but never pushed).
    let (_l3, id3, _c0) = fixture_repo(&state);
    let origin3 = tempfile::TempDir::new().expect("origin3");
    git(origin3.path(), &["init", "--bare"]);
    block_on(add_remote_inner(&state, &id3, "origin".into(), file_url(origin3.path()))).expect("add");
    let err = block_on(pull_inner(&state, &id3)).expect_err("no upstream");
    assert!(matches!(err, AppError::NoUpstream(_)), "{err:?}");
}

// ============================================================ push

/// push rejects a non-fast-forward update: the remote advanced and the local
/// branch has its own new commit, so the plain (non-force) push is rejected.
#[test]
fn push_rejected_on_non_fast_forward() {
    require_git!();
    let state = AppState::default();
    let (local, id, origin, branch) = pushed_local(&state);

    poke_advance(origin.path(), &branch, "remote.txt");
    write_stage_commit(&state, &id, local.path(), "mine.txt", "mine\n", "my commit");

    let err = block_on(push_inner(&state, &id, None)).expect_err("non-ff push");
    assert!(matches!(err, AppError::PushRejected(_)), "{err:?}");
}

// ============================================================ force_push (with lease)

/// force_push with a fresh lease succeeds (remote-tracking baseline matches the
/// remote); a STALE lease (remote moved since the last fetch) is refused.
#[test]
fn force_push_lease_happy_and_stale() {
    require_git!();
    let state = AppState::default();

    // Happy: baseline == remote, rewrite HEAD, force-push lands.
    let (l1, id1, _origin1, _b1) = pushed_local(&state);
    block_on(fetch_inner(&state, &id1)).expect("fetch baseline");
    block_on(commit_amend_inner(&state, &id1, "C0 rewritten".into(), None, None)).expect("amend");
    match block_on(force_push_inner(&state, &id1, None)).expect("force push") {
        PushResult::Pushed { .. } | PushResult::UpToDate { .. } => {}
    }
    let _ = l1;

    // Stale: remote advanced after the last fetch, so the lease check refuses.
    let (l2, id2, origin2, branch2) = pushed_local(&state);
    block_on(fetch_inner(&state, &id2)).expect("fetch baseline");
    poke_advance(origin2.path(), &branch2, "moved.txt");
    block_on(commit_amend_inner(&state, &id2, "rewrite while stale".into(), None, None)).expect("amend");
    let err = block_on(force_push_inner(&state, &id2, None)).expect_err("stale lease");
    assert!(matches!(err, AppError::PushRejected(_)), "{err:?}");
    let _ = l2;
}
