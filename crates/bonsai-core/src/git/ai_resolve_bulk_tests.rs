//! Known-answer tests for the PURE bulk payload/attribution rules (P68b §9).
//!
//! Everything here runs without a repo, a child process or a Tauri app: the
//! split/attribution decisions are exactly the ones that must never regress
//! silently (a lost path, a truncated payload, a markerful body presented as
//! clean), so they are asserted at the cheapest possible level.

use super::*;
use crate::git::conflict::ConflictKind;

fn sides(path: &str, body: &str) -> ConflictSides {
    ConflictSides {
        path: path.to_string(),
        kind: ConflictKind::BothModified,
        base: format!("base of {path}\n"),
        ours: format!("ours of {path}\n"),
        theirs: format!("theirs of {path}\n"),
        conflicted: body.to_string(),
    }
}

fn paths(list: &[&str]) -> Vec<String> {
    list.iter().map(|s| s.to_string()).collect()
}

// ============================================================ payload

#[test]
fn bulk_payload_delimits_every_file_with_its_path_and_sides() {
    let a = sides("i18n/de.json", "<<<<<<< HEAD\nours\n=======\ntheirs\n>>>>>>> topic\n");
    let b = sides("i18n/en.json", "plain\n");
    let payload = build_bulk_payload(&[&a, &b]);

    assert!(payload.starts_with("BONSAI BULK CONFLICT RESOLUTION — 2 files, one merge\n"));
    assert!(payload.contains("===== BONSAI FILE 1/2: i18n/de.json ====="), "{payload}");
    assert!(payload.contains("===== BONSAI FILE 2/2: i18n/en.json ====="), "{payload}");
    // Every labelled section is present for each file.
    assert_eq!(payload.matches("----- ANCESTOR (base) -----").count(), 2);
    assert_eq!(payload.matches("----- OURS -----").count(), 2);
    assert_eq!(payload.matches("----- THEIRS -----").count(), 2);
    assert_eq!(
        payload.matches("----- CONFLICTED (worktree, with markers) -----").count(),
        2
    );
    assert!(payload.contains("CONFLICT KIND: BothModified"));
    // The marker text travels verbatim — that is the model's primary input.
    assert!(payload.contains("<<<<<<< HEAD"), "{payload}");
}

#[test]
fn bulk_payload_renders_an_empty_side_as_absent() {
    let mut s = sides("new.txt", "body\n");
    s.base = String::new();
    let payload = build_bulk_payload(&[&s]);
    assert!(payload.contains(&format!("----- ANCESTOR (base) -----\n{ABSENT}\n")), "{payload}");
}

/// Both streaming prompts must stay SINGLE-LINE: Rust refuses to pass an argument
/// containing a newline to the Windows `.cmd` shim (D13), so a wrapped prompt
/// would break the feature against the real CLI on every npm install.
#[test]
fn streaming_prompts_are_single_line_and_carry_both_clauses() {
    let bulk = bulk_system_prompt();
    for text in [bulk.as_str(), BULK_PROMPT, READ_ONLY_CLAUSE, SENTINEL_CLAUSE] {
        assert!(!text.contains('\n') && !text.contains('\r'), "multi-line prompt: {text:?}");
    }
    assert!(bulk.contains("BONSAI_NEEDS_INPUT:"), "sentinel clause missing");
    assert!(bulk.contains("Read, Grep, Glob"), "read-only clause missing");
    assert!(bulk.contains("BONSAI RESULT:"), "the response contract must be stated");
}

// ============================================================ pack_batches

#[test]
fn pack_batches_splits_by_cap_instead_of_truncating() {
    let parts = vec![
        ("a".to_string(), 1_000),
        ("b".to_string(), 1_000),
        ("c".to_string(), 1_000),
    ];
    // Budget = cap - HEADER_RESERVE = 2_000 ⇒ two files per batch.
    let (batches, failed) = pack_batches(&parts, 2_000 + HEADER_RESERVE);
    assert_eq!(batches, vec![vec![0, 1], vec![2]], "greedy fill, order preserved");
    assert!(failed.is_empty(), "{failed:?}");

    // A cap that fits everything ⇒ ONE batch (the locked "one run" default).
    let (one, failed) = pack_batches(&parts, 400_000);
    assert_eq!(one, vec![vec![0, 1, 2]]);
    assert!(failed.is_empty());
}

#[test]
fn pack_batches_marks_a_single_oversize_file_failed_and_keeps_the_others() {
    let parts = vec![
        ("small.txt".to_string(), 100),
        ("huge.json".to_string(), 10_000),
        ("other.txt".to_string(), 100),
    ];
    let (batches, failed) = pack_batches(&parts, 1_000 + HEADER_RESERVE);
    assert_eq!(batches, vec![vec![0, 2]], "the two small files still run");
    assert_eq!(failed.len(), 1);
    assert_eq!(failed[0].path, "huge.json");
    assert!(failed[0].reason.contains("too large"), "{:?}", failed[0].reason);
}

#[test]
fn pack_batches_never_loses_a_path() {
    let parts: Vec<(String, usize)> =
        (0..25).map(|i| (format!("f{i}.txt"), 900 + i * 7)).collect();
    let (batches, failed) = pack_batches(&parts, 3_000 + HEADER_RESERVE);
    let packed: usize = batches.iter().map(Vec::len).sum();
    assert_eq!(packed + failed.len(), parts.len(), "every path is packed or failed");
    assert!(batches.iter().all(|b| !b.is_empty()), "no empty batch");
    // A degenerate cap fails everything rather than truncating anything.
    let (none, all_failed) = pack_batches(&parts, 0);
    assert!(none.is_empty());
    assert_eq!(all_failed.len(), parts.len());
}

// ============================================================ parse_bulk_response

fn block(path: &str, body: &str) -> String {
    format!("===== BONSAI RESULT: {path} =====\n{body}")
}

#[test]
fn parse_attributes_every_requested_path() {
    let requested = paths(&["a/one.json", "b/two.json", "c/three.txt"]);
    let text = format!(
        "{}{}{}",
        block("a/one.json", "ONE\n"),
        block("b/two.json", "TWO\nLINES\n"),
        block("c/three.txt", "THREE\n"),
    );
    let parsed = parse_bulk_response(&text, &requested).expect("3 blocks parse");
    assert!(parsed.failed.is_empty(), "{:?}", parsed.failed);
    assert!(parsed.unknown.is_empty());
    assert_eq!(
        parsed.proposals,
        vec![
            ("a/one.json".to_string(), "ONE\n".to_string()),
            ("b/two.json".to_string(), "TWO\nLINES\n".to_string()),
            ("c/three.txt".to_string(), "THREE\n".to_string()),
        ]
    );
}

#[test]
fn parse_marks_a_missing_path_failed_without_failing_the_batch() {
    let requested = paths(&["a.txt", "b.txt"]);
    let parsed = parse_bulk_response(&block("a.txt", "A\n"), &requested).expect("partial reply");
    assert_eq!(parsed.proposals.len(), 1, "the answered file still resolves");
    assert_eq!(parsed.failed.len(), 1);
    assert_eq!(parsed.failed[0].path, "b.txt");
    assert!(parsed.failed[0].reason.contains("no result block"), "{:?}", parsed.failed[0]);
}

#[test]
fn parse_ignores_a_block_for_a_path_nobody_asked_about() {
    let requested = paths(&["a.txt"]);
    let text = format!("{}{}", block("a.txt", "A\n"), block("../evil.txt", "NOPE\n"));
    let parsed = parse_bulk_response(&text, &requested).expect("extra block");
    assert_eq!(parsed.proposals.len(), 1);
    assert_eq!(parsed.unknown, vec!["../evil.txt".to_string()]);
    assert!(parsed.failed.is_empty(), "an extra block is not a failure: {:?}", parsed.failed);
}

/// The safety net that matters most: a body that still has markers must NEVER
/// reach `proposals`, where the caller would treat it as clean.
#[test]
fn parse_marks_a_markerful_body_failed() {
    let requested = paths(&["a.txt", "b.txt"]);
    let text = format!(
        "{}{}",
        block("a.txt", "clean\n"),
        block("b.txt", "<<<<<<< HEAD\nours\n=======\ntheirs\n>>>>>>> topic\n"),
    );
    let parsed = parse_bulk_response(&text, &requested).expect("markerful body");
    assert_eq!(parsed.proposals.len(), 1);
    assert_eq!(parsed.proposals[0].0, "a.txt");
    assert_eq!(parsed.failed.len(), 1);
    assert_eq!(parsed.failed[0].path, "b.txt");
    assert!(parsed.failed[0].reason.contains("unresolved conflict markers"));
}

#[test]
fn parse_marks_an_empty_body_failed() {
    let requested = paths(&["a.txt", "b.txt"]);
    let text = format!("{}{}", block("a.txt", "\n   \n"), block("b.txt", "ok\n"));
    let parsed = parse_bulk_response(&text, &requested).expect("empty body");
    assert_eq!(parsed.proposals.len(), 1);
    assert_eq!(parsed.failed[0].path, "a.txt");
    assert!(parsed.failed[0].reason.contains("empty"), "{:?}", parsed.failed[0]);
}

#[test]
fn parse_strips_a_fence_and_one_framing_blank_line() {
    let requested = paths(&["a.txt", "b.txt"]);
    let text = format!(
        "{}{}",
        block("a.txt", "```json\n{\"k\": 1}\n```\n"),
        block("b.txt", "\nbody\n\n"),
    );
    let parsed = parse_bulk_response(&text, &requested).expect("fenced + framed");
    assert_eq!(parsed.proposals[0].1, "{\"k\": 1}\n", "fence stripped");
    assert_eq!(parsed.proposals[1].1, "body\n", "one framing blank line dropped");
}

#[test]
fn parse_tolerates_prose_before_the_first_block_and_odd_spacing() {
    let requested = paths(&["a.txt"]);
    let text = "Sure, here you go:\n\n  =====  BONSAI RESULT: a.txt  =====  \nBODY\n";
    let parsed = parse_bulk_response(text, &requested).expect("lenient header match");
    assert_eq!(parsed.proposals, vec![("a.txt".to_string(), "BODY\n".to_string())]);
}

#[test]
fn parse_keeps_the_first_of_two_blocks_for_the_same_path() {
    let requested = paths(&["a.txt"]);
    let text = format!("{}{}", block("a.txt", "FIRST\n"), block("a.txt", "SECOND\n"));
    let parsed = parse_bulk_response(&text, &requested).expect("duplicate block");
    assert_eq!(parsed.proposals.len(), 1);
    assert_eq!(parsed.proposals[0].1, "FIRST\n");
}

#[test]
fn parse_fails_the_batch_only_when_no_block_at_all_and_several_requested() {
    let requested = paths(&["a.txt", "b.txt"]);
    let err = parse_bulk_response("I merged them for you, all good!", &requested)
        .expect_err("no blocks + >1 requested is a protocol break");
    match err {
        AppError::AiFailed(m) => assert!(m.contains("per-file result blocks"), "{m}"),
        other => panic!("expected AiFailed, got {other:?}"),
    }
}

/// A batch of ONE file (a big file packed alone) whose reply is the bare body:
/// attribute it rather than failing, since there is no ambiguity about the path.
#[test]
fn parse_accepts_a_bare_body_when_exactly_one_path_was_requested() {
    let requested = paths(&["only.txt"]);
    let parsed = parse_bulk_response("MERGED BODY\n", &requested).expect("bare body");
    assert_eq!(parsed.proposals, vec![("only.txt".to_string(), "MERGED BODY\n".to_string())]);
}

#[test]
fn markers_rule_matches_the_frontend_rule() {
    // Present at column 0 in any of the three runs of seven.
    assert!(has_conflict_markers("a\n<<<<<<< HEAD\nb\n"));
    assert!(has_conflict_markers("=======\n"));
    assert!(has_conflict_markers(">>>>>>> topic\n"));
    // Not at column 0, or shorter than seven ⇒ not a marker (mirrors MARKER_RE).
    assert!(!has_conflict_markers(" <<<<<<< indented\n"));
    assert!(!has_conflict_markers("====== six\n"));
    assert!(!has_conflict_markers("a normal file\nwith === separators\n"));
}
