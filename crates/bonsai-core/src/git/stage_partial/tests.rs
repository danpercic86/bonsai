use super::*;
use crate::git::diff::DiffLine;

// --- split_keep_terminator ---------------------------------------------

#[test]
fn split_empty_is_no_lines() {
    assert!(split_keep_terminator(b"").is_empty());
}

#[test]
fn split_keeps_lf_and_trailing_no_newline() {
    assert_eq!(split_keep_terminator(b"a\nb\nc"), vec![&b"a\n"[..], b"b\n", b"c"]);
    assert_eq!(split_keep_terminator(b"a\nb\n"), vec![&b"a\n"[..], b"b\n"]);
    assert_eq!(split_keep_terminator(b"solo"), vec![&b"solo"[..]]);
    // A lone newline is one line "\n".
    assert_eq!(split_keep_terminator(b"\n"), vec![&b"\n"[..]]);
}

#[test]
fn split_keeps_crlf_inside_the_slice() {
    assert_eq!(
        split_keep_terminator(b"one\r\ntwo\r\n"),
        vec![&b"one\r\n"[..], b"two\r\n"]
    );
}

// --- assemble ----------------------------------------------------------

#[test]
fn assemble_empty_is_empty() {
    assert_eq!(assemble(&[]), b"");
}

#[test]
fn assemble_interior_missing_terminator_gets_lf() {
    // "b" was EOF-with-no-newline in its source but is now interior.
    assert_eq!(assemble(&[b"a\n", b"b", b"c\n"]), b"a\nb\nc\n");
}

#[test]
fn assemble_final_slice_keeps_its_terminator_state() {
    assert_eq!(assemble(&[b"a\n", b"b"]), b"a\nb"); // no final newline
    assert_eq!(assemble(&[b"a\n", b"b\n"]), b"a\nb\n"); // final newline kept
}

// --- reconstruct helpers -----------------------------------------------

fn dl(kind: LineKind, old_no: Option<u32>, new_no: Option<u32>) -> DiffLine {
    DiffLine {
        kind,
        old_no,
        new_no,
        content: String::new(),
        no_newline: false,
        spans: Vec::new(),
    }
}

fn set(v: &[u32]) -> HashSet<u32> {
    v.iter().copied().collect()
}

/// Full stage: reconstruct(Stage) with everything selected == new bytes;
/// nothing selected == old bytes. Byte-exact, CRLF preserved.
#[test]
fn stage_crlf_modification_is_byte_exact() {
    let old = b"one\r\ntwo\r\nthree\r\n"; // index
    let new = b"one\r\ntwo CHANGED\r\nthree\r\n"; // workdir
    let old_lines = split_keep_terminator(old);
    let new_lines = split_keep_terminator(new);
    let hunk = Hunk {
        old_start: 1,
        old_lines: 3,
        new_start: 1,
        new_lines: 3,
        lines: vec![
            dl(LineKind::Context, Some(1), Some(1)),
            dl(LineKind::Del, Some(2), None),
            dl(LineKind::Add, None, Some(2)),
            dl(LineKind::Context, Some(3), Some(3)),
        ],
    };
    // Accept the modification: pick both the del and the add.
    let hunks = std::slice::from_ref(&hunk);
    let got = reconstruct(Direction::Stage, hunks, &old_lines, &new_lines, &set(&[2]), &set(&[2]))
        .expect("reconstruct");
    assert_eq!(assemble(&got), new, "CRLF must survive byte-for-byte");
    // Reject everything: back to the index bytes.
    let none = reconstruct(Direction::Stage, hunks, &old_lines, &new_lines, &set(&[]), &set(&[]))
        .expect("reconstruct");
    assert_eq!(assemble(&none), old);
}

/// no-newline-at-EOF: staging the final-line modification keeps the exact
/// (absent) terminator; staging only the deletion keeps the earlier
/// terminator.
#[test]
fn stage_no_newline_eof_is_byte_exact() {
    let old = b"a\nb\nc"; // no trailing newline
    let new = b"a\nb\nd"; // no trailing newline
    let old_lines = split_keep_terminator(old);
    let new_lines = split_keep_terminator(new);
    let hunk = Hunk {
        old_start: 1,
        old_lines: 3,
        new_start: 1,
        new_lines: 3,
        lines: vec![
            dl(LineKind::Context, Some(1), Some(1)),
            dl(LineKind::Context, Some(2), Some(2)),
            dl(LineKind::Del, Some(3), None),
            dl(LineKind::Add, None, Some(3)),
        ],
    };
    let hunks = std::slice::from_ref(&hunk);
    let full = reconstruct(Direction::Stage, hunks, &old_lines, &new_lines, &set(&[3]), &set(&[3]))
        .expect("reconstruct");
    assert_eq!(assemble(&full), new); // "a\nb\nd", no trailing newline

    // Stage only the deletion of "c": "b" keeps its own newline.
    let del_only = reconstruct(Direction::Stage, hunks, &old_lines, &new_lines, &set(&[]), &set(&[3]))
        .expect("reconstruct");
    assert_eq!(assemble(&del_only), b"a\nb\n");
}

/// Unstage restores the HEAD line for a selected Del and keeps unselected
/// index changes.
#[test]
fn unstage_restores_head_line_for_selected_del() {
    // HEAD (old) has "b"; index (new) deleted it and added "x".
    let head = b"a\nb\nc\n";
    let index = b"a\nx\nc\n";
    let old_lines = split_keep_terminator(head);
    let new_lines = split_keep_terminator(index);
    let hunk = Hunk {
        old_start: 1,
        old_lines: 3,
        new_start: 1,
        new_lines: 3,
        lines: vec![
            dl(LineKind::Context, Some(1), Some(1)),
            dl(LineKind::Del, Some(2), None),
            dl(LineKind::Add, None, Some(2)),
            dl(LineKind::Context, Some(3), Some(3)),
        ],
    };
    // Unstage BOTH the add and the del -> index reverts to HEAD.
    let hunks = std::slice::from_ref(&hunk);
    let both = reconstruct(Direction::Unstage, hunks, &old_lines, &new_lines, &set(&[2]), &set(&[2]))
        .expect("reconstruct");
    assert_eq!(assemble(&both), head);
    // Unstage only the add -> "x" removed but "b" not restored yet.
    let add_only = reconstruct(Direction::Unstage, hunks, &old_lines, &new_lines, &set(&[2]), &set(&[]))
        .expect("reconstruct");
    assert_eq!(assemble(&add_only), b"a\nc\n");
}

/// Inter-hunk gap fill: a two-hunk stage where only the first hunk's change
/// is selected keeps the untouched middle region and reverts the 2nd hunk.
#[test]
fn stage_two_hunks_gap_filled_from_base() {
    // 6-line file; edits at line 2 and line 5.
    let old = b"l1\nl2\nl3\nl4\nl5\nl6\n";
    let new = b"l1\nL2\nl3\nl4\nL5\nl6\n";
    let old_lines = split_keep_terminator(old);
    let new_lines = split_keep_terminator(new);
    let h1 = Hunk {
        old_start: 1,
        old_lines: 3,
        new_start: 1,
        new_lines: 3,
        lines: vec![
            dl(LineKind::Context, Some(1), Some(1)),
            dl(LineKind::Del, Some(2), None),
            dl(LineKind::Add, None, Some(2)),
            dl(LineKind::Context, Some(3), Some(3)),
        ],
    };
    let h2 = Hunk {
        old_start: 4,
        old_lines: 3,
        new_start: 4,
        new_lines: 3,
        lines: vec![
            dl(LineKind::Context, Some(4), Some(4)),
            dl(LineKind::Del, Some(5), None),
            dl(LineKind::Add, None, Some(5)),
            dl(LineKind::Context, Some(6), Some(6)),
        ],
    };
    // Select only hunk 1's change (new line 2). Hunk 2 reverted; gap (l4)
    // filled from old.
    let got = reconstruct(
        Direction::Stage,
        &[h1, h2],
        &old_lines,
        &new_lines,
        &set(&[2]),
        &set(&[2]),
    )
    .expect("reconstruct");
    assert_eq!(assemble(&got), b"l1\nL2\nl3\nl4\nl5\nl6\n");
}

#[test]
fn stale_line_number_out_of_range_errors() {
    let old_lines = split_keep_terminator(b"a\n");
    let new_lines = split_keep_terminator(b"a\nb\n");
    let hunk = Hunk {
        old_start: 1,
        old_lines: 1,
        new_start: 1,
        new_lines: 2,
        lines: vec![
            dl(LineKind::Context, Some(1), Some(1)),
            // Add referencing new line 2, which is present.
            dl(LineKind::Add, None, Some(2)),
        ],
    };
    // A hunk that references an old line 5 that does not exist -> stale.
    let bad = Hunk {
        old_start: 5,
        old_lines: 0,
        new_start: 3,
        new_lines: 0,
        lines: vec![dl(LineKind::Context, Some(5), Some(3))],
    };
    let err = reconstruct(
        Direction::Stage,
        &[hunk, bad],
        &old_lines,
        &new_lines,
        &set(&[2]),
        &set(&[]),
    )
    .expect_err("out-of-range context must be stale");
    assert!(matches!(err, AppError::Other(m) if m.contains("stale")));
}

/// Empty selection is a no-op before any repo work (no path/repo needed).
#[test]
fn empty_selection_is_a_noop() {
    let dir = crate::testutil::scratch_dir();
    let missing = dir.path().join("not-a-repo");
    assert!(stage_partial(&missing, "file.txt", None, &[]).is_ok());
    assert!(unstage_partial(&missing, "file.txt", None, &[]).is_ok());
}

/// Audit 2026-08-07 §2.1: partial staging must apply CHECK-IN filters.
/// Under `core.autocrlf=true` a CRLF workdir file must stage an LF blob,
/// and a full-selection partial stage must produce the EXACT index blob
/// `git add` (`Index::add_path`, which filters) would produce.
#[test]
fn stage_partial_applies_checkin_filters_under_autocrlf() {
    let dir = crate::testutil::scratch_dir();
    let repo = git2::Repository::init(dir.path()).expect("init");
    repo.config()
        .expect("config")
        .set_bool("core.autocrlf", true)
        .expect("autocrlf");
    let sig = git2::Signature::now("Test", "t@example.com").expect("sig");

    // Base commit: CRLF on disk -> LF in the ODB via the filtering add_path.
    std::fs::write(dir.path().join("f.txt"), "one\r\ntwo\r\n").expect("write base");
    let mut idx = repo.index().expect("index");
    idx.add_path(Path::new("f.txt")).expect("add base");
    idx.write().expect("write index");
    let tree_oid = idx.write_tree().expect("tree");
    let tree = repo.find_tree(tree_oid).expect("find tree");
    repo.commit(Some("HEAD"), &sig, &sig, "base", &tree, &[])
        .expect("commit");
    drop(tree);

    // Workdir edit (CRLF): append line 3.
    std::fs::write(dir.path().join("f.txt"), "one\r\ntwo\r\nthree\r\n").expect("edit");

    // Oracle: the blob oid `git add` would stage, then reset the index back
    // to HEAD so the partial stage starts from a clean index.
    idx.add_path(Path::new("f.txt")).expect("oracle add");
    let expected_oid = idx.get_path(Path::new("f.txt"), 0).expect("oracle entry").id;
    let head_tree = repo.head().expect("head").peel_to_tree().expect("head tree");
    idx.read_tree(&head_tree).expect("reset index");
    idx.write().expect("write reset index");
    drop(head_tree);
    drop(idx);

    // Partial-stage the single added line — the FULL selection of this diff.
    let sel = vec![LineSelection {
        kind: LineKind::Add,
        old_no: None,
        new_no: Some(3),
    }];
    stage_partial(dir.path(), "f.txt", None, &sel).expect("stage_partial");

    // Fresh open: stage_partial wrote through its own Repository handle.
    let repo2 = git2::Repository::open(dir.path()).expect("reopen");
    let entry = repo2
        .index()
        .expect("index")
        .get_path(Path::new("f.txt"), 0)
        .expect("staged entry");
    let content = repo2.find_blob(entry.id).expect("blob").content().to_vec();
    assert_eq!(content, b"one\ntwo\nthree\n", "staged blob must be LF-only");
    assert_eq!(
        entry.id, expected_oid,
        "full-selection partial stage must equal `git add` byte-for-byte"
    );
}

/// Invalid paths are rejected by the reused validator, before repo work.
#[test]
fn invalid_paths_are_rejected() {
    let dir = crate::testutil::scratch_dir();
    let sel = vec![LineSelection {
        kind: LineKind::Add,
        old_no: None,
        new_no: Some(1),
    }];
    for bad in ["", "../escape", "/abs", "a\\b"] {
        let err = stage_partial(dir.path(), bad, None, &sel)
            .expect_err(&format!("must reject {bad:?}"));
        assert!(matches!(err, AppError::Other(m) if m.contains("invalid path")));
    }
    let err = stage_partial(dir.path(), "ok.txt", Some("../escape"), &sel)
        .expect_err("bad orig_path");
    assert!(matches!(err, AppError::Other(m) if m.contains("invalid path")));
}
