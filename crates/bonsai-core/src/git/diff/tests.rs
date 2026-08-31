use super::*;
use crate::error::AppError;
use crate::git::status::FileStatus;

#[test]
fn normalize_content_strips_one_newline_then_one_cr() {
    assert_eq!(normalize_content(b"plain"), "plain");
    assert_eq!(normalize_content(b"lf\n"), "lf");
    assert_eq!(normalize_content(b"crlf\r\n"), "crlf");
    assert_eq!(normalize_content(b"cr-only\r"), "cr-only");
    // Only ONE of each is stripped; interior \r preserved.
    assert_eq!(normalize_content(b"a\r\r\n"), "a\r");
    assert_eq!(normalize_content(b"a\n\n"), "a\n");
    assert_eq!(normalize_content(b"mid\rline\n"), "mid\rline");
    // Lossy UTF-8, never an error.
    assert_eq!(normalize_content(b"\xff\xfe\n"), "\u{fffd}\u{fffd}");
}

#[test]
fn delta_status_map_matches_contract() {
    assert_eq!(map_status(git2::Delta::Added), FileStatus::Added);
    assert_eq!(map_status(git2::Delta::Deleted), FileStatus::Deleted);
    assert_eq!(map_status(git2::Delta::Modified), FileStatus::Modified);
    assert_eq!(map_status(git2::Delta::Renamed), FileStatus::Renamed);
    assert_eq!(map_status(git2::Delta::Copied), FileStatus::Renamed);
    assert_eq!(map_status(git2::Delta::Typechange), FileStatus::Typechange);
    assert_eq!(map_status(git2::Delta::Untracked), FileStatus::Untracked);
    assert_eq!(map_status(git2::Delta::Conflicted), FileStatus::Conflicted);
    assert_eq!(map_status(git2::Delta::Unmodified), FileStatus::Modified);
    assert_eq!(map_status(git2::Delta::Ignored), FileStatus::Modified);
    assert_eq!(map_status(git2::Delta::Unreadable), FileStatus::Modified);
}

/// Wire shape: camelCase keys, `noNewline` omitted when false, kinds as
/// lowercase strings.
#[test]
fn wire_serialization_shape() {
    let fd = FileDiff {
        path: "a.txt".to_string(),
        orig_path: None,
        status: FileStatus::Modified,
        binary: false,
        too_large: false,
        hunks: vec![Hunk {
            old_start: 1,
            old_lines: 1,
            new_start: 1,
            new_lines: 1,
            lines: vec![
                DiffLine {
                    kind: LineKind::Del,
                    old_no: Some(1),
                    new_no: None,
                    content: "old".to_string(),
                    no_newline: false,
                    spans: Vec::new(),
                },
                DiffLine {
                    kind: LineKind::Add,
                    old_no: None,
                    new_no: Some(1),
                    content: "new".to_string(),
                    no_newline: true,
                    spans: Vec::new(),
                },
            ],
        }],
    };
    let json = serde_json::to_string(&fd).expect("serialize FileDiff");
    assert!(json.contains("\"origPath\":null"), "{json}");
    assert!(json.contains("\"tooLarge\":false"), "{json}");
    assert!(json.contains("\"oldStart\":1"), "{json}");
    assert!(json.contains("\"kind\":\"del\""), "{json}");
    assert!(json.contains("\"kind\":\"add\""), "{json}");
    assert!(json.contains("\"noNewline\":true"), "{json}");
    // no_newline: false is skipped entirely.
    assert_eq!(json.matches("noNewline").count(), 1, "{json}");
    // P61a: empty `spans` is wire-invisible (byte-identical to pre-P61a).
    // `intraline=false` never populates spans, so the key must not appear.
    assert!(!json.contains("spans"), "empty spans must be skipped: {json}");
}

/// P61a: when a diff is intraline-annotated, changed paired lines serialize
/// a `spans` array of `[start, len]` code-point ranges; the byte-off case is
/// the same fixture with `annotate_hunk` NOT run (guarded above).
#[test]
fn wire_serialization_spans_present_when_annotated() {
    let mut hunk = Hunk {
        old_start: 1,
        old_lines: 1,
        new_start: 1,
        new_lines: 1,
        lines: vec![
            DiffLine {
                kind: LineKind::Del,
                old_no: Some(1),
                new_no: None,
                content: "const x = 1;".to_string(),
                no_newline: false,
                spans: Vec::new(),
            },
            DiffLine {
                kind: LineKind::Add,
                old_no: None,
                new_no: Some(1),
                content: "const x = 42;".to_string(),
                no_newline: false,
                spans: Vec::new(),
            },
        ],
    };
    crate::git::intraline::annotate_hunk(&mut hunk);
    let json = serde_json::to_string(&hunk).expect("serialize Hunk");
    // Emphasis only on 1 -> 42 (code-point index 10).
    assert!(json.contains("\"spans\":[[10,1]]"), "del spans: {json}");
    assert!(json.contains("\"spans\":[[10,2]]"), "add spans: {json}");
}

/// The benign-race contract (§2.2): a clean path yields an empty FileDiff,
/// not an error — for both staged and unstaged modes.
#[test]
fn clean_path_returns_empty_filediff() {
    let dir = crate::testutil::scratch_dir();
    let repo = git2::Repository::init(dir.path()).expect("init repo");
    {
        let mut cfg = repo.config().expect("config");
        cfg.set_str("user.name", "Test User").expect("name");
        cfg.set_str("user.email", "test@example.com").expect("email");
        cfg.set_bool("core.autocrlf", false).expect("autocrlf");
    }
    std::fs::write(dir.path().join("a.txt"), "one\n").expect("write");
    crate::git::stage::stage_paths(dir.path(), &["a.txt".to_string()]).expect("stage");
    crate::git::commit::create_commit(dir.path(), "base", None, false).expect("commit");

    for staged in [false, true] {
        let fd = workdir_file_diff(dir.path(), "a.txt", None, staged, false, false)
            .expect("clean path must not error");
        assert_eq!(fd.path, "a.txt");
        assert_eq!(fd.status, FileStatus::Modified);
        assert!(!fd.binary && !fd.too_large);
        assert!(fd.hunks.is_empty());
    }
}

/// Pathspecs are literal (fixlet): a file whose NAME contains glob
/// metachars must not fnmatch sibling deltas. `*` is illegal in Windows
/// filenames, but `[`/`]` are legal AND are fnmatch metachars — the glob
/// `a[ab].txt` would match `aa.txt` and `ab.txt`, merging three deltas
/// into one corrupted FileDiff without `disable_pathspec_match`.
#[test]
fn glob_metachar_filename_matches_literally() {
    let dir = crate::testutil::scratch_dir();
    let repo = git2::Repository::init(dir.path()).expect("init repo");
    {
        let mut cfg = repo.config().expect("config");
        cfg.set_str("user.name", "Test User").expect("name");
        cfg.set_str("user.email", "test@example.com").expect("email");
        cfg.set_bool("core.autocrlf", false).expect("autocrlf");
    }
    for name in ["a[ab].txt", "aa.txt", "ab.txt"] {
        std::fs::write(dir.path().join(name), format!("{name} old\n")).expect("write");
    }
    crate::git::stage::stage_paths(
        dir.path(),
        &["a[ab].txt".into(), "aa.txt".into(), "ab.txt".into()],
    )
    .expect("stage");
    crate::git::commit::create_commit(dir.path(), "base", None, false).expect("commit");
    for name in ["a[ab].txt", "aa.txt", "ab.txt"] {
        std::fs::write(dir.path().join(name), format!("{name} new\n")).expect("rewrite");
    }

    let fd = workdir_file_diff(dir.path(), "a[ab].txt", None, false, false, false).expect("diff");
    assert_eq!(fd.path, "a[ab].txt");
    assert_eq!(fd.status, FileStatus::Modified);
    assert_eq!(fd.hunks.len(), 1, "exactly one delta must match");
    let lines = &fd.hunks[0].lines;
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0].content, "a[ab].txt old");
    assert_eq!(lines[1].content, "a[ab].txt new");
}

#[test]
fn invalid_paths_are_rejected() {
    let dir = crate::testutil::scratch_dir();
    git2::Repository::init(dir.path()).expect("init repo");
    for bad in ["", "../escape", "/abs", "a\\b"] {
        let err = workdir_file_diff(dir.path(), bad, None, false, false, false)
            .expect_err(&format!("must reject {bad:?}"));
        assert!(matches!(err, AppError::Other(m) if m.contains("invalid path")));
    }
    // orig_path is validated too.
    let err = workdir_file_diff(dir.path(), "ok.txt", Some("../escape"), false, false, false)
        .expect_err("must reject bad orig_path");
    assert!(matches!(err, AppError::Other(m) if m.contains("invalid path")));
}
