//! T2 Area 1 (pass B) — diff / search / history / reflog / history-index /
//! ai-history command inners, runtime-free per the `tests.rs` pattern. Each
//! command gets ≥1 happy + ≥1 failure path via its `_inner` seam.

use super::tests_support::*;
use super::*;
use bonsai_core::git::search::{SearchField, SearchQuery};

fn block_on<F: std::future::Future>(f: F) -> F::Output {
    tauri::async_runtime::block_on(f)
}

fn query(text: &str, field: SearchField) -> SearchQuery {
    SearchQuery {
        text: text.to_string(),
        field,
        regex: false,
        case_sensitive: false,
        max_results: 0,
        scope_ref: None,
    }
}

// ============================================================ working-dir diff

/// Unstaged vs staged diff of the same file: the modification shows as an
/// added/changed line on the unstaged side; once staged, the staged side
/// carries it and the unstaged side is clean.
#[test]
fn workdir_file_diff_unstaged_then_staged() {
    let state = AppState::default();
    let (dir, id, _c0) = fixture_repo(&state);
    std::fs::write(dir.path().join("a.txt"), "base\nADDED\n").expect("write");

    let unstaged = block_on(get_workdir_file_diff_inner(
        &state, &id, "a.txt".into(), None, false, false, false,
    ))
    .expect("unstaged diff");
    assert_eq!(unstaged.path, "a.txt");
    assert!(!unstaged.binary);
    assert!(
        unstaged.hunks.iter().flat_map(|h| &h.lines).any(|l| l.content == "ADDED"),
        "unstaged diff must contain the added line"
    );

    block_on(stage_inner(&state, &id, vec!["a.txt".into()])).expect("stage");
    let staged = block_on(get_workdir_file_diff_inner(
        &state, &id, "a.txt".into(), None, true, false, false,
    ))
    .expect("staged diff");
    assert!(
        staged.hunks.iter().flat_map(|h| &h.lines).any(|l| l.content == "ADDED"),
        "staged (HEAD vs index) diff must carry the added line"
    );
}

/// intraline=true annotates the changed sub-range on a modified line with
/// `spans`; intraline=false leaves them empty (wire-invisible).
#[test]
fn workdir_file_diff_intraline_spans() {
    let state = AppState::default();
    let (dir, id, _c0) = fixture_repo(&state);
    write_stage_commit(&state, &id, dir.path(), "i.txt", "hello world\n", "seed i");
    std::fs::write(dir.path().join("i.txt"), "hello brave world\n").expect("write");

    let on = block_on(get_workdir_file_diff_inner(
        &state, &id, "i.txt".into(), None, false, false, true,
    ))
    .expect("intraline on");
    let any_span = on.hunks.iter().flat_map(|h| &h.lines).any(|l| !l.spans.is_empty());
    assert!(any_span, "a paired modified line must carry intraline spans");

    let off = block_on(get_workdir_file_diff_inner(
        &state, &id, "i.txt".into(), None, false, false, false,
    ))
    .expect("intraline off");
    assert!(
        off.hunks.iter().flat_map(|h| &h.lines).all(|l| l.spans.is_empty()),
        "intraline=false must emit no spans"
    );
}

/// A workdir diff for an unknown repo is NoRepo (failure path).
#[test]
fn workdir_file_diff_no_repo() {
    let state = AppState::default();
    let err = block_on(get_workdir_file_diff_inner(
        &state, MISSING_ID, "a.txt".into(), None, false, false, false,
    ))
    .expect_err("no repo");
    assert!(matches!(err, AppError::NoRepo), "{err:?}");
}

// ============================================================ commit diff

/// get_commit_diff on the ROOT commit (no parent) lists its whole tree as
/// additions; get_commit_file_diff returns that file's hunks. A malformed oid
/// is a clean Git error.
#[test]
fn commit_diff_root_and_file_and_bad_oid() {
    let state = AppState::default();
    let (dir, id, c0) = fixture_repo(&state);

    let cd: CommitDiff = block_on(get_commit_diff_inner(&state, &id, c0.clone())).expect("root diff");
    assert_eq!(cd.details.oid, c0);
    assert!(cd.details.parents.is_empty(), "root commit has no parent");
    assert!(cd.files.iter().any(|f| f.path == "a.txt"), "a.txt is in the root tree");

    let fd: FileDiff = block_on(get_commit_file_diff_inner(
        &state, &id, c0.clone(), "a.txt".into(), None, false, false,
    ))
    .expect("root file diff");
    assert_eq!(fd.path, "a.txt");
    assert!(
        fd.hunks.iter().flat_map(|h| &h.lines).any(|l| l.content == "base"),
        "root file diff shows the added content"
    );

    let err = block_on(get_commit_diff_inner(&state, &id, "not-a-valid-oid".into()))
        .expect_err("bad oid");
    assert!(matches!(err, AppError::Git(_)), "{err:?}");
    let _ = dir;
}

/// A second commit diffs against its first parent — only the changed file
/// appears, not the untouched root file.
#[test]
fn commit_diff_against_first_parent() {
    let state = AppState::default();
    let (dir, id, _c0) = fixture_repo(&state);
    let c1 = write_stage_commit(&state, &id, dir.path(), "b.txt", "second\n", "C1").oid;

    let cd = block_on(get_commit_diff_inner(&state, &id, c1.clone())).expect("diff");
    assert_eq!(cd.details.parents.len(), 1);
    let paths: Vec<&str> = cd.files.iter().map(|f| f.path.as_str()).collect();
    assert_eq!(paths, vec!["b.txt"], "only the file introduced by C1: {paths:?}");
}

// ============================================================ compare with HEAD

/// compare_with_head against the root commit reports the net tree delta; the
/// per-file variant returns that file's hunks. Bad oid → Git.
#[test]
fn compare_with_head_happy_and_bad_oid() {
    let state = AppState::default();
    let (dir, id, c0) = fixture_repo(&state);
    write_stage_commit(&state, &id, dir.path(), "c.txt", "extra\n", "C1");

    let cmp: CompareDiff = block_on(compare_with_head_inner(&state, &id, c0.clone())).expect("compare");
    // HEAD(new) has c.txt that the root(old) lacks → c.txt appears in the delta.
    assert!(cmp.files.iter().any(|f| f.path == "c.txt"), "delta must include c.txt");

    let fd = block_on(compare_with_head_file_diff_inner(
        &state, &id, c0, "c.txt".into(), None, false, false,
    ))
    .expect("compare file diff");
    assert_eq!(fd.path, "c.txt");

    let err = block_on(compare_with_head_inner(&state, &id, "zzz".into())).expect_err("bad oid");
    assert!(matches!(err, AppError::Git(_)), "{err:?}");
    let _ = dir;
}

// ============================================================ image diff

const PNG_BYTES: &[u8] = b"\x89PNG\r\n\x1a\nfake-image-payload";

/// get_image_diff over the four side-resolution cases: an untracked image is
/// added (new only); a committed-then-deleted image is deleted (old only); a
/// 0-byte image side is treated as ABSENT (None, not empty base64); a
/// non-image extension resolves to `application/octet-stream`.
#[test]
fn image_diff_added_deleted_zero_byte_and_non_image() {
    let state = AppState::default();
    let (dir, id, _c0) = fixture_repo(&state);

    // ADDED: png in the workdir, not staged/tracked.
    std::fs::write(dir.path().join("logo.png"), PNG_BYTES).expect("write png");
    let added = block_on(get_image_diff_inner(
        &state,
        &id,
        ImageDiffRequest::Workdir { path: "logo.png".into(), orig_path: None, staged: false },
    ))
    .expect("added image");
    assert!(added.old.is_none(), "no old side for an added image");
    let new = added.new.expect("new side present");
    assert_eq!(new.mime, "image/png");
    assert_eq!(new.byte_len as usize, PNG_BYTES.len());

    // DELETED: commit the png, then delete it from the workdir (unstaged).
    write_stage_commit(&state, &id, dir.path(), "logo.png", "PNGDATA", "add png");
    std::fs::remove_file(dir.path().join("logo.png")).expect("rm png");
    let deleted = block_on(get_image_diff_inner(
        &state,
        &id,
        ImageDiffRequest::Workdir { path: "logo.png".into(), orig_path: None, staged: false },
    ))
    .expect("deleted image");
    assert!(deleted.old.is_some(), "old side (index blob) present");
    assert!(deleted.new.is_none(), "deleted → no new side");

    // 0-BYTE → ABSENT: an empty file yields no renderable side.
    std::fs::write(dir.path().join("empty.png"), b"").expect("write empty");
    let zero = block_on(get_image_diff_inner(
        &state,
        &id,
        ImageDiffRequest::Workdir { path: "empty.png".into(), orig_path: None, staged: false },
    ))
    .expect("zero byte");
    assert!(zero.old.is_none() && zero.new.is_none(), "0-byte side is absent, not empty base64");
    assert!(!zero.new_too_large, "0-byte is not over-cap");

    // NON-IMAGE extension: octet-stream mime (a.txt exists from the fixture).
    std::fs::write(dir.path().join("a.txt"), "changed\n").expect("write txt");
    let non = block_on(get_image_diff_inner(
        &state,
        &id,
        ImageDiffRequest::Workdir { path: "a.txt".into(), orig_path: None, staged: false },
    ))
    .expect("non image");
    if let Some(side) = non.new {
        assert_eq!(side.mime, "application/octet-stream");
    }
}

/// A `..` path in an image request is rejected up front (Other/invalid path),
/// never reaching the filesystem.
#[test]
fn image_diff_rejects_traversal_path() {
    let state = AppState::default();
    let (_dir, id, _c0) = fixture_repo(&state);
    let err = block_on(get_image_diff_inner(
        &state,
        &id,
        ImageDiffRequest::Workdir { path: "../evil.png".into(), orig_path: None, staged: false },
    ))
    .expect_err("traversal rejected");
    assert!(matches!(err, AppError::Other(_) | AppError::InvalidName(_)), "{err:?}");
}

// ============================================================ search

/// Message + author search resolve via the git2 header revwalk (no git binary).
/// An empty query resolves to zero matches without touching git.
#[test]
fn search_message_author_and_empty() {
    let state = AppState::default();
    let (dir, id, _c0) = fixture_repo(&state);
    write_stage_commit(&state, &id, dir.path(), "d.txt", "d\n", "unique-needle-msg");

    let by_msg = block_on(search_commits_inner(&state, &id, query("unique-needle", SearchField::Message)))
        .expect("message search");
    assert_eq!(by_msg.matches.len(), 1, "one commit carries the needle");
    assert!(by_msg.matches[0].summary.contains("unique-needle-msg"));

    let by_author = block_on(search_commits_inner(&state, &id, query("Test User", SearchField::Author)))
        .expect("author search");
    assert!(!by_author.matches.is_empty(), "the fixture author matches every commit");

    let empty = block_on(search_commits_inner(&state, &id, query("   ", SearchField::Message)))
        .expect("empty query");
    assert!(empty.matches.is_empty() && !empty.truncated, "whitespace query ⇒ no matches");
    let _ = dir;
}

/// Path + content search shell out to `git log`; a malformed `-G` regex is a
/// clean Git error. Skips cleanly when the git binary is absent.
#[test]
fn search_path_content_and_bad_regex() {
    if !have_git() {
        eprintln!("skipping: git CLI not on PATH");
        return;
    }
    let state = AppState::default();
    let (dir, id, _c0) = fixture_repo(&state);
    write_stage_commit(&state, &id, dir.path(), "needle.txt", "content-token\n", "adds needle");

    let by_path = block_on(search_commits_inner(&state, &id, query("needle.txt", SearchField::Path)))
        .expect("path search");
    assert!(!by_path.matches.is_empty(), "the commit that added needle.txt matches");

    let by_content = block_on(search_commits_inner(&state, &id, query("content-token", SearchField::Content)))
        .expect("content search");
    assert!(!by_content.matches.is_empty(), "the pickaxe finds the added token");

    let mut bad = query("[unterminated", SearchField::Content);
    bad.regex = true;
    let err = block_on(search_commits_inner(&state, &id, bad)).expect_err("bad -G regex");
    assert!(matches!(err, AppError::Git(_)), "{err:?}");
    let _ = dir;
}

// ============================================================ blame + history

/// blame_file returns one entry per line attributed to the introducing commit;
/// a nonexistent path is a clean error (never a panic).
#[test]
fn blame_happy_and_bad_path() {
    let state = AppState::default();
    let (dir, id, c0) = fixture_repo(&state);

    let lines: Vec<BlameLine> = block_on(blame_file_inner(&state, &id, "a.txt".into(), None))
        .expect("blame a.txt");
    assert_eq!(lines.len(), 1, "a.txt has one line");
    assert_eq!(lines[0].oid, c0, "the only line was introduced by C0");
    assert_eq!(lines[0].line_text, "base");

    let err = block_on(blame_file_inner(&state, &id, "no/such/file".into(), None))
        .expect_err("blame missing path");
    assert!(matches!(err, AppError::Git(_) | AppError::Other(_)), "{err:?}");
    let _ = dir;
}

/// file_history is newest-first and the `limit` caps the returned rows.
#[test]
fn file_history_limit_caps_rows() {
    let state = AppState::default();
    let (dir, id, _c0) = fixture_repo(&state);
    for i in 1..=3 {
        write_stage_commit(&state, &id, dir.path(), "a.txt", &format!("v{i}\n"), &format!("edit {i}"));
    }

    let all: Vec<FileHistoryEntry> = block_on(file_history_inner(&state, &id, "a.txt".into(), 0))
        .expect("full history");
    assert_eq!(all.len(), 4, "root + 3 edits touch a.txt");
    assert_eq!(all[0].summary, "edit 3", "newest first");

    let capped = block_on(file_history_inner(&state, &id, "a.txt".into(), 2)).expect("capped");
    assert_eq!(capped.len(), 2, "limit=2 caps the rows");
    assert_eq!(capped[0].summary, "edit 3");
}

// ============================================================ reflog

/// read_reflog("HEAD") reports the commit walk newest-first; a ref that was
/// never updated (unborn HEAD, no commits) yields an empty log, not an error.
#[test]
fn reflog_head_and_never_updated() {
    let state = AppState::default();
    let (dir, id, _c0) = fixture_repo(&state);
    write_stage_commit(&state, &id, dir.path(), "a.txt", "v1\n", "second");

    let entries: Vec<ReflogEntry> = block_on(read_reflog_inner(&state, &id, "HEAD".into()))
        .expect("HEAD reflog");
    assert!(entries.len() >= 2, "at least two HEAD updates (root + second)");
    assert_eq!(entries[0].index, 0, "newest first");

    // Never-updated: a fresh unborn-HEAD repo has an empty HEAD reflog.
    let quiet = init_repo_with_identity();
    let qid = open(&state, quiet.path()).expect("open quiet").repo_id;
    let empty = block_on(read_reflog_inner(&state, &qid, "HEAD".into()))
        .expect("empty reflog is not an error");
    assert!(empty.is_empty(), "an unborn HEAD reflog is []");
    let _ = dir;
}

// ============================================================ history index lifecycle

/// build → status → search over the BM25 index via the `_inner` seams (temp
/// base dir): a freshly built index reports built=true and a keyword search
/// returns the matching commit.
#[test]
fn history_index_build_status_search_lifecycle() {
    let state = AppState::default();
    let (dir, id, _c0) = fixture_repo(&state);
    write_stage_commit(&state, &id, dir.path(), "e.txt", "authentication refactor\n", "rework auth layer");

    let base = tempfile::TempDir::new().expect("index base");

    let built: IndexStatus =
        block_on(history_index_build_inner(&state, base.path(), &id, |_p| {})).expect("build");
    assert!(built.built, "index reports built");
    assert!(built.indexed_commits >= 2, "root + the auth commit indexed");

    let status = block_on(history_index_status_inner(&state, base.path(), &id)).expect("status");
    assert!(status.built);
    assert_eq!(status.indexed_commits, built.indexed_commits);

    let results: HistorySearchResults = block_on(history_search_inner(
        &state,
        base.path(),
        &id,
        HistoryQuery { text: "auth".into(), top_k: 0 },
    ))
    .expect("search");
    assert!(!results.index_stale, "index exists");
    assert!(
        results.hits.iter().any(|h| h.summary.contains("auth")),
        "the auth commit ranks as a hit"
    );
    let _ = dir;
}

/// history_search with NO index built reports `index_stale: true` and no hits
/// (the UI then offers Build) — not an error.
#[test]
fn history_search_without_index_is_stale_not_error() {
    let state = AppState::default();
    let (_dir, id, _c0) = fixture_repo(&state);
    let base = tempfile::TempDir::new().expect("empty base");

    let results = block_on(history_search_inner(
        &state,
        base.path(),
        &id,
        HistoryQuery { text: "anything".into(), top_k: 0 },
    ))
    .expect("search without an index is Ok");
    assert!(results.index_stale, "no index ⇒ stale");
    assert!(results.hits.is_empty());
    assert_eq!(results.indexed_commits, 0);
}

// ============================================================ ai history consent gate

/// ai_search_history refuses with AiUnavailable BEFORE any repo/index work when
/// consent is not granted (a default settings file has ai_consented=false).
#[test]
fn ai_search_history_refuses_without_consent() {
    let state = AppState::default();
    let (_dir, id, _c0) = fixture_repo(&state);
    let base = tempfile::TempDir::new().expect("base");
    // A nonexistent settings file loads defaults: ai_enabled=true, ai_consented=false.
    let settings_file = base.path().join("no-such-settings.json");

    let err = block_on(ai_search_history_inner(
        &state,
        &settings_file,
        base.path(),
        &id,
        "why did auth change?".into(),
        0,
    ))
    .expect_err("consent gate must refuse");
    assert!(matches!(err, AppError::AiUnavailable(_)), "{err:?}");
}
