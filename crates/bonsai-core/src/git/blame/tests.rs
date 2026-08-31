use super::*;

/// git2-init a scratch repo with identity + autocrlf off (mirrors the
/// `ai_explain`/`diff` test fixtures).
fn init_scratch() -> tempfile::TempDir {
    let dir = crate::testutil::scratch_dir();
    let repo = git2::Repository::init(dir.path()).expect("init repo");
    let mut cfg = repo.config().expect("config");
    cfg.set_str("user.name", "Test User").expect("name");
    cfg.set_str("user.email", "test@example.com").expect("email");
    cfg.set_bool("core.autocrlf", false).expect("autocrlf");
    dir
}

/// §7.1: a 3-commit fixture editing distinct lines — `blame_line(path, k)`
/// returns the oid that LAST touched line k plus that line's text; an
/// out-of-range line maps to `Git`.
#[test]
fn blame_line_targets_single_line() {
    use crate::git::commit::create_commit;
    use crate::git::stage::stage_paths;

    let dir = init_scratch();
    let p = dir.path();

    std::fs::write(p.join("f.txt"), "a\nb\nc\n").expect("write");
    stage_paths(p, &["f.txt".into()]).expect("stage");
    let c1 = create_commit(p, "add f", None, false).expect("commit").oid;

    std::fs::write(p.join("f.txt"), "a\nb2\nc\n").expect("write");
    stage_paths(p, &["f.txt".into()]).expect("stage");
    let c2 = create_commit(p, "edit line 2", None, false).expect("commit").oid;

    std::fs::write(p.join("f.txt"), "a\nb2\nc3\n").expect("write");
    stage_paths(p, &["f.txt".into()]).expect("stage");
    let c3 = create_commit(p, "edit line 3", None, false).expect("commit").oid;

    let l1 = blame_line(p, "f.txt", 1, None).expect("blame l1");
    assert_eq!(l1.oid, c1, "line 1 last touched by the first commit");
    assert_eq!(l1.line_text, "a");
    assert_eq!(l1.final_line_no, 1);

    let l2 = blame_line(p, "f.txt", 2, None).expect("blame l2");
    assert_eq!(l2.oid, c2, "line 2 last touched by the second commit");
    assert_eq!(l2.line_text, "b2");

    let l3 = blame_line(p, "f.txt", 3, None).expect("blame l3");
    assert_eq!(l3.oid, c3, "line 3 last touched by the third commit");
    assert_eq!(l3.line_text, "c3");

    // Out-of-range line (and line 0) => Git, not a panic.
    let err = blame_line(p, "f.txt", 99, None).expect_err("out of range");
    assert!(matches!(err, AppError::Git(_)), "got {err:?}");
    let err0 = blame_line(p, "f.txt", 0, None).expect_err("line 0");
    assert!(matches!(err0, AppError::Git(_)), "got {err0:?}");
}

/// `at_oid = Some(..)` is honored: two commits touch the SAME line, so
/// blaming that line at the PAST commit returns a different introducing
/// commit (and the past line text) than blaming at HEAD. If the parameter
/// were ignored (always HEAD), both calls would agree and this fails.
#[test]
fn blame_line_honors_at_oid() {
    use crate::git::commit::create_commit;
    use crate::git::stage::stage_paths;

    let dir = init_scratch();
    let p = dir.path();

    // c1 introduces line 2 as "old"; c2 rewrites the SAME line to "new".
    std::fs::write(p.join("f.txt"), "a\nold\nc\n").expect("write");
    stage_paths(p, &["f.txt".into()]).expect("stage");
    let c1 = create_commit(p, "introduce old", None, false).expect("commit").oid;

    std::fs::write(p.join("f.txt"), "a\nnew\nc\n").expect("write");
    stage_paths(p, &["f.txt".into()]).expect("stage");
    let c2 = create_commit(p, "rewrite line 2", None, false).expect("commit").oid;

    // At HEAD (both None and the explicit HEAD oid) line 2 blames to c2.
    let head = blame_line(p, "f.txt", 2, None).expect("blame at HEAD");
    assert_eq!(head.oid, c2);
    assert_eq!(head.line_text, "new");
    let at_c2 = blame_line(p, "f.txt", 2, Some(&c2)).expect("blame at c2");
    assert_eq!(at_c2.oid, c2);

    // At the PAST commit the same line blames to c1 with the OLD text —
    // proving `at_oid` seeds the blame walk, not HEAD.
    let past = blame_line(p, "f.txt", 2, Some(&c1)).expect("blame at c1");
    assert_eq!(past.oid, c1, "at c1 the line must blame to c1, not HEAD's c2");
    assert_eq!(past.line_text, "old", "line text must come from the c1 blob");
    assert_eq!(past.summary, "introduce old");
    assert_ne!(past.oid, head.oid, "past and HEAD blames must differ");
}

/// A traversing path is rejected as `Other` (via `validate_rel_path`) before
/// any repo access, exactly like `blame_file`.
#[test]
fn blame_line_rejects_bad_path() {
    let dir = std::env::temp_dir();
    let err = blame_line(&dir, "../secret", 1, None).expect_err("must reject ..");
    assert!(matches!(err, AppError::Other(_)), "got {err:?}");
}

/// `BlameLine` serializes with EXACTLY the camelCase keys the TS wire type
/// declares (contract §9.3 / §10.1).
#[test]
fn blame_line_wire_shape_is_camel_case() {
    let v = serde_json::to_value(BlameLine {
        oid: "abc".to_string(),
        author_name: "Ada".to_string(),
        author_email: "ada@example.com".to_string(),
        author_ts: 1_700_000_000,
        summary: "init".to_string(),
        orig_line_no: 1,
        final_line_no: 2,
        line_text: "let x = 1;".to_string(),
    })
    .expect("json");
    assert_eq!(
        v,
        serde_json::json!({
            "oid": "abc",
            "authorName": "Ada",
            "authorEmail": "ada@example.com",
            "authorTs": 1_700_000_000i64,
            "summary": "init",
            "origLineNo": 1,
            "finalLineNo": 2,
            "lineText": "let x = 1;"
        })
    );
}

/// `FileHistoryEntry` serializes with EXACTLY the camelCase keys the TS wire
/// type declares.
#[test]
fn file_history_entry_wire_shape_is_camel_case() {
    let v = serde_json::to_value(FileHistoryEntry {
        oid: "def".to_string(),
        summary: "edit".to_string(),
        author_name: "Grace".to_string(),
        author_email: "grace@example.com".to_string(),
        author_ts: 42,
    })
    .expect("json");
    assert_eq!(
        v,
        serde_json::json!({
            "oid": "def",
            "summary": "edit",
            "authorName": "Grace",
            "authorEmail": "grace@example.com",
            "authorTs": 42
        })
    );
}

/// A traversing / absolute / backslash path is rejected as `Other` BEFORE
/// any repo access (reuses `validate_rel_path`).
#[test]
fn blame_rejects_bad_path() {
    let dir = std::env::temp_dir();
    let err = blame_file(&dir, "../secret", None).expect_err("must reject ..");
    assert!(matches!(err, AppError::Other(_)), "got {err:?}");

    let err = file_history(&dir, "../secret", 10).expect_err("must reject ..");
    assert!(matches!(err, AppError::Other(_)), "got {err:?}");
}
