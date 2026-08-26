//! Wire-shape / serde / sanitization / hex-gate tests for `ai_operation`.
//! Extracted verbatim from the former inline `mod tests`; shared fixtures live
//! in `test_support`.

use super::test_support::{expect_unsupported, linear_repo};
use super::*;

// -------------------------------------------------------- §11.10 wire shape

/// §11.10: `PlanOutcome` / `ProposedOperation` / `SafeOp` / `OperationPreview`
/// serialize with the EXACT camelCase tags + keys the TS union expects.
#[test]
fn plan_outcome_and_safe_op_wire_shape_is_camel_case() {
    let outcome = PlanOutcome::Proposed {
        operation: Box::new(ProposedOperation {
            op: SafeOp::Reset {
                target_oid: "a".repeat(40),
                target_short: "aaaaaaa".to_string(),
                mode: ResetMode::Mixed,
            },
            preview: OperationPreview {
                title: "Undo last merge".to_string(),
                summary: "Move `main` back to c3d4e5f.".to_string(),
                danger: DangerLevel::Destructive,
                ref_changes: vec![RefChange {
                    name: "main".to_string(),
                    from_short: "c3d4e5f".to_string(),
                    to_short: "aaaaaaa".to_string(),
                }],
                dropped_commits: vec![CommitRef {
                    short: "c3d4e5f".to_string(),
                    summary: "Merge branch 'feature/x'".to_string(),
                }],
                added_commits: 0,
                worktree_warning: None,
                confirm_label: "Undo merge".to_string(),
            },
            rationale: "why".to_string(),
            cost_usd: Some(0.01),
        }),
    };
    let v = serde_json::to_value(&outcome).expect("json");
    assert_eq!(
        v,
        serde_json::json!({
            "kind": "proposed",
            "operation": {
                "op": {
                    "kind": "reset",
                    "targetOid": "a".repeat(40),
                    "targetShort": "aaaaaaa",
                    "mode": "mixed"
                },
                "preview": {
                    "title": "Undo last merge",
                    "summary": "Move `main` back to c3d4e5f.",
                    "danger": "destructive",
                    "refChanges": [
                        { "name": "main", "fromShort": "c3d4e5f", "toShort": "aaaaaaa" }
                    ],
                    "droppedCommits": [
                        { "short": "c3d4e5f", "summary": "Merge branch 'feature/x'" }
                    ],
                    "addedCommits": 0,
                    "worktreeWarning": null,
                    "confirmLabel": "Undo merge"
                },
                "rationale": "why",
                "costUsd": 0.01
            }
        })
    );

    let unsupported = PlanOutcome::Unsupported {
        reason: "no".to_string(),
        cost_usd: None,
    };
    assert_eq!(
        serde_json::to_value(&unsupported).expect("json"),
        serde_json::json!({ "kind": "unsupported", "reason": "no", "costUsd": null })
    );

    // The six P55b SafeOp variants round-trip their camelCase tags + fields
    // (the wire contract the frontend mock + dispatch rely on).
    assert_eq!(
        serde_json::to_value(SafeOp::Revert {
            oid: "b".repeat(40),
            short: "bbbbbbb".to_string(),
        })
        .expect("json"),
        serde_json::json!({ "kind": "revert", "oid": "b".repeat(40), "short": "bbbbbbb" })
    );
    assert_eq!(
        serde_json::to_value(SafeOp::SwitchBranch {
            name: "origin/x".to_string(),
            remote: true,
        })
        .expect("json"),
        serde_json::json!({ "kind": "switchBranch", "name": "origin/x", "remote": true })
    );
    assert_eq!(
        serde_json::to_value(SafeOp::CreateBranch {
            name: "feat/x".to_string(),
            at_oid: None,
        })
        .expect("json"),
        serde_json::json!({ "kind": "createBranch", "name": "feat/x", "atOid": null })
    );
    assert_eq!(
        serde_json::to_value(SafeOp::DeleteBranch {
            name: "old".to_string(),
        })
        .expect("json"),
        serde_json::json!({ "kind": "deleteBranch", "name": "old" })
    );
    assert_eq!(
        serde_json::to_value(SafeOp::Stash {
            message: None,
            include_untracked: true,
        })
        .expect("json"),
        serde_json::json!({ "kind": "stash", "message": null, "includeUntracked": true })
    );
    assert_eq!(
        serde_json::to_value(SafeOp::Discard {
            paths: vec!["a.txt".to_string()],
        })
        .expect("json"),
        serde_json::json!({ "kind": "discard", "paths": ["a.txt"] })
    );
    assert_eq!(
        serde_json::to_value(SafeOp::Merge {
            name: "topic".to_string(),
        })
        .expect("json"),
        serde_json::json!({ "kind": "merge", "name": "topic" })
    );
}

// ------------------------------------------------------- §11.11 single-line

/// §11.11: the prompt/system-prompt consts MUST be single-line (Windows argv
/// constraint — a newline would make `claude.cmd` reject the argument).
#[test]
fn prompts_are_single_line() {
    for s in [PLAN_SYSTEM_PROMPT, PLAN_PROMPT] {
        assert!(!s.contains('\n'), "prompt must be single-line: {s:?}");
        assert!(!s.contains('\r'), "prompt must be single-line: {s:?}");
    }
}

// ------------------------------------------------- F-A2-1 sanitize_model_text

/// F-A2-1 truth table: `\n`/`\t` → space; other C0/C1 controls stripped;
/// bidi override/isolate chars stripped; ≤200 chars pass through; longer
/// input is char-boundary truncated with a `…` marker.
#[test]
fn sanitize_model_text_truth_table() {
    // Plain text is untouched.
    assert_eq!(sanitize_model_text("hello world"), "hello world");
    assert_eq!(sanitize_model_text(""), "");
    // Newline / tab become a single space each.
    assert_eq!(sanitize_model_text("a\nb\tc"), "a b c");
    // Other C0 controls (CR, ESC, NUL, BEL) and DEL are stripped.
    assert_eq!(sanitize_model_text("a\r\x1b\x00\x07\x7fb"), "ab");
    // C1 controls (U+0080–U+009F) are stripped.
    assert_eq!(sanitize_model_text("a\u{0085}\u{009f}b"), "ab");
    // Bidi override + isolate chars are stripped (U+202A–E, U+2066–69).
    assert_eq!(
        sanitize_model_text("a\u{202a}\u{202b}\u{202c}\u{202d}\u{202e}b"),
        "ab"
    );
    assert_eq!(
        sanitize_model_text("x\u{2066}\u{2067}\u{2068}\u{2069}y"),
        "xy"
    );
    // Classic RTL-override spoof is neutralized.
    assert_eq!(sanitize_model_text("gpj.\u{202e}exe"), "gpj.exe");
    // Exactly MAX chars: no truncation marker.
    let exact: String = "a".repeat(MAX_MODEL_TEXT);
    assert_eq!(sanitize_model_text(&exact), exact);
    // MAX+1 chars: capped at MAX + '…'.
    let long: String = "a".repeat(MAX_MODEL_TEXT + 50);
    let out = sanitize_model_text(&long);
    assert_eq!(out.chars().count(), MAX_MODEL_TEXT + 1);
    assert!(out.ends_with('…'));
    // Multibyte chars: cap counts CHARS, never splits a boundary.
    let multi: String = "é".repeat(MAX_MODEL_TEXT + 5);
    let out = sanitize_model_text(&multi);
    assert_eq!(out.chars().count(), MAX_MODEL_TEXT + 1);
    assert!(out.ends_with('…'));
    assert!(out.chars().take(MAX_MODEL_TEXT).all(|c| c == 'é'));
    // Stripped controls do not count toward the cap.
    let padded = format!("{}{}", "\x01".repeat(300), "ok");
    assert_eq!(sanitize_model_text(&padded), "ok");
}

// ---------------------------------------------------- F-A2-2 revparse hex gate

/// F-A2-2: `revparse_commit` accepts ONLY `[0-9a-fA-F]{4,40}` — every
/// non-hex revspec (HEAD~n, reflog, branch names, :/pattern, too short,
/// too long) is rejected without touching revparse.
#[test]
fn revparse_commit_is_hex_gated() {
    let (dir, a, _b) = linear_repo();
    let repo = git2::Repository::open(dir.path()).expect("open");
    let short_a: String = a.chars().take(7).collect();

    // Accepted: full oid, short oid, 4-char prefix, uppercase gate-pass.
    assert!(revparse_commit(&repo, &a).is_some(), "full oid resolves");
    assert!(revparse_commit(&repo, &short_a).is_some(), "short oid resolves");
    assert!(revparse_commit(&repo, &a[..4]).is_some(), "4-char prefix resolves");

    // Rejected by the gate (would otherwise resolve!): revspecs + refs.
    for spec in [
        "HEAD",
        "HEAD~1",
        "HEAD^",
        "@{0}",
        "HEAD@{2.days.ago}",
        ":/A",
        "main",
        "refs/heads/main",
        "abc",                       // 3 chars: too short
        &"a".repeat(41),             // 41 chars: too long
        "deadbeeg",                  // non-hex char
        "",                          //
        "	deadbeef",                // leading control char
    ] {
        assert!(
            revparse_commit(&repo, spec).is_none(),
            "spec {spec:?} must be rejected"
        );
    }
}

// ------------------------------------------------ NIT deny_unknown_fields

/// An otherwise-valid intent carrying an EXTRA field must fail the parse
/// (`deny_unknown_fields`) and therefore fail CLOSED to `Unsupported` at
/// the plan level — off-schema output is never partially honored.
#[test]
fn extra_fields_fail_closed_to_unsupported() {
    for raw in [
        r#"{"intent":"undoLastCommit","keepChanges":true,"force":true}"#,
        r#"{"intent":"deleteBranch","branch":"x","cascade":true}"#,
        r#"{"intent":"unsupported","reason":"r","hint":"h"}"#,
    ] {
        assert!(
            serde_json::from_str::<AiOpIntent>(raw).is_err(),
            "extra field must not parse: {raw}"
        );
    }
    let (dir, _a, _b) = linear_repo();
    let repo = git2::Repository::open(dir.path()).expect("open");
    let outcome = plan_from_reply(
        &repo,
        r#"{"intent":"undoLastCommit","keepChanges":true,"force":true}"#,
        None,
    )
    .expect("Ok");
    expect_unsupported(outcome);
}
