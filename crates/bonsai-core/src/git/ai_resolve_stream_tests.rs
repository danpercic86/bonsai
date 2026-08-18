//! Unit tests for the streaming wire shapes and the single-file prompt (P68b). A
//! `#[path]`-included child module — kept in its own file so
//! `ai_resolve_stream.rs` stays inside the ~500-line rule, and a child of that
//! module so it can reach its private items without widening their visibility
//! (the `session_drain_tests` convention).
//!
//! The funnel's own tests moved with it in P68c:
//! `ai_resolve_stream_events_tests.rs`.

use super::*;

/// The serde casing must match the TS `AiResolveBatch` / `AiResolveFailure`
/// types exactly (`runId` / `costUsd`), mirroring
/// `ai_resolve::tests::proposal_wire_shape_is_camel_case`.
#[test]
fn batch_wire_shape_is_camel_case() {
    let v = serde_json::to_value(AiResolveBatch {
        run_id: "ai-abc-0".to_string(),
        proposals: vec![AiResolveProposal {
            path: "a.txt".to_string(),
            proposed_text: "merged\n".to_string(),
            cost_usd: None,
        }],
        failed: vec![AiResolveFailure {
            path: "b.txt".to_string(),
            reason: "no result block returned".to_string(),
        }],
        cost_usd: Some(0.0263),
        turns: 2,
    })
    .expect("json");
    assert_eq!(
        v,
        serde_json::json!({
            "runId": "ai-abc-0",
            "proposals": [{ "path": "a.txt", "proposedText": "merged\n", "costUsd": null }],
            "failed": [{ "path": "b.txt", "reason": "no result block returned" }],
            "costUsd": 0.0263,
            "turns": 2
        })
    );
}

/// The single-file streaming prompt must stay the PROVEN P13 text plus exactly
/// the two P68 clauses, on ONE line (D13).
#[test]
fn single_system_prompt_extends_the_p13_prompt_on_one_line() {
    let p = single_system_prompt();
    assert!(p.starts_with(SYSTEM_PROMPT), "the P13 prompt must be the prefix: {p}");
    assert!(p.ends_with(SENTINEL_CLAUSE));
    assert!(p.contains("Read, Grep, Glob"));
    assert!(!p.contains('\n') && !p.contains('\r'), "prompt must be single-line");
    // L6 (security audit 2026-08-18): the bulk test asserts the literal sentinel
    // TOKEN, this one only asserted the clause constant — so a reworded clause that
    // dropped the token would have kept this test green while `sentinel_question`
    // stopped matching, i.e. mid-run questions would silently become proposals.
    assert!(p.contains(crate::ai::SENTINEL), "the sentinel token must reach the model: {p}");
    assert!(p.contains("BONSAI_NEEDS_INPUT:"), "spelled out, so a renamed const is visible here");
}
