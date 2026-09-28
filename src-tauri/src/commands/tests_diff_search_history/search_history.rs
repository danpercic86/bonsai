//! Search / blame / file-history / reflog / history-index / ai-history
//! command inners.

use super::*;

// ============================================================ search

/// Message + author search resolve via the git2 header revwalk (no git binary).
/// An empty query resolves to zero matches without touching git.
#[test]
fn search_message_author_and_empty() {
    let state = AppState::default();
    let (dir, id, _c0) = fixture_repo(&state);
    write_stage_commit(&state, &id, dir.path(), "d.txt", "d\n", "unique-needle-msg");

    let by_msg = block_on(search_commits_inner(
        &state,
        &id,
        query("unique-needle", SearchField::Message),
    ))
    .expect("message search");
    assert_eq!(by_msg.matches.len(), 1, "one commit carries the needle");
    assert!(by_msg.matches[0].summary.contains("unique-needle-msg"));

    let by_author = block_on(search_commits_inner(
        &state,
        &id,
        query("Test User", SearchField::Author),
    ))
    .expect("author search");
    assert!(
        !by_author.matches.is_empty(),
        "the fixture author matches every commit"
    );

    let empty = block_on(search_commits_inner(
        &state,
        &id,
        query("   ", SearchField::Message),
    ))
    .expect("empty query");
    assert!(
        empty.matches.is_empty() && !empty.truncated,
        "whitespace query ⇒ no matches"
    );
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
    write_stage_commit(
        &state,
        &id,
        dir.path(),
        "needle.txt",
        "content-token\n",
        "adds needle",
    );

    let by_path = block_on(search_commits_inner(
        &state,
        &id,
        query("needle.txt", SearchField::Path),
    ))
    .expect("path search");
    assert!(
        !by_path.matches.is_empty(),
        "the commit that added needle.txt matches"
    );

    let by_content = block_on(search_commits_inner(
        &state,
        &id,
        query("content-token", SearchField::Content),
    ))
    .expect("content search");
    assert!(
        !by_content.matches.is_empty(),
        "the pickaxe finds the added token"
    );

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

    let lines: Vec<BlameLine> =
        block_on(blame_file_inner(&state, &id, "a.txt".into(), None)).expect("blame a.txt");
    assert_eq!(lines.len(), 1, "a.txt has one line");
    assert_eq!(lines[0].oid, c0, "the only line was introduced by C0");
    assert_eq!(lines[0].line_text, "base");

    let err = block_on(blame_file_inner(&state, &id, "no/such/file".into(), None))
        .expect_err("blame missing path");
    assert!(
        matches!(err, AppError::Git(_) | AppError::Other(_)),
        "{err:?}"
    );
    let _ = dir;
}

/// file_history is newest-first and the `limit` caps the returned rows.
#[test]
fn file_history_limit_caps_rows() {
    let state = AppState::default();
    let (dir, id, _c0) = fixture_repo(&state);
    for i in 1..=3 {
        write_stage_commit(
            &state,
            &id,
            dir.path(),
            "a.txt",
            &format!("v{i}\n"),
            &format!("edit {i}"),
        );
    }

    let all: Vec<FileHistoryEntry> =
        block_on(file_history_inner(&state, &id, "a.txt".into(), 0)).expect("full history");
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

    let entries: Vec<ReflogEntry> =
        block_on(read_reflog_inner(&state, &id, "HEAD".into())).expect("HEAD reflog");
    assert!(
        entries.len() >= 2,
        "at least two HEAD updates (root + second)"
    );
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
    write_stage_commit(
        &state,
        &id,
        dir.path(),
        "e.txt",
        "authentication refactor\n",
        "rework auth layer",
    );

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
        HistoryQuery {
            text: "auth".into(),
            top_k: 0,
        },
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
        HistoryQuery {
            text: "anything".into(),
            top_k: 0,
        },
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
