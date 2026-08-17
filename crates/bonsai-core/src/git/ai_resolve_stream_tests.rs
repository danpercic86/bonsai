//! Unit tests for the run-level event funnel and the streaming wire shapes
//! (P68b). A `#[path]`-included child module — kept in its own file so
//! `ai_resolve_stream.rs` stays inside the ~500-line rule, and a child of that
//! module so it can reach the private `RunEvents` without widening its visibility
//! (the `session_drain_tests` convention).

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
}

/// The funnel's contract: ONE monotonic sequence, one `Started`, per-batch
/// terminal events swallowed, and the partial echo carried over.
#[test]
fn run_events_renumber_sessions_into_one_sequence() {
    let seen: Mutex<Vec<AiRunEvent>> = Mutex::new(Vec::new());
    let sink = |ev: AiRunEvent| seen.lock().unwrap_or_else(|e| e.into_inner()).push(ev);
    let events = RunEvents::new("ai-1".to_string(), &sink, true);
    events.emit_run_level(AiRunEventKind::Started, None, None);

    // Batch 1: its own sequence, starting at 0 again.
    let mut log = AiRunEvent::new("ai-1", 0, AiRunEventKind::Started, 0, 0);
    events.forward(log.clone());
    log = AiRunEvent::new("ai-1", 1, AiRunEventKind::Log, 5, 0);
    log.text = Some("hello".to_string());
    events.forward(log);
    let mut turn_end = AiRunEvent::new("ai-1", 2, AiRunEventKind::TurnEnd, 7, 1);
    turn_end.cost_usd = Some(0.01);
    events.forward(turn_end);
    let mut done = AiRunEvent::new("ai-1", 3, AiRunEventKind::Done, 9, 1);
    done.cost_usd = Some(0.01);
    events.forward(done);

    // Batch 2 restarts at seq 0 — the whole reason this funnel exists.
    let mut b2 = AiRunEvent::new("ai-1", 0, AiRunEventKind::Started, 0, 0);
    events.forward(b2.clone());
    b2 = AiRunEvent::new("ai-1", 1, AiRunEventKind::Log, 3, 0);
    b2.text = Some("second batch".to_string());
    events.forward(b2);
    let mut failed = AiRunEvent::new("ai-1", 2, AiRunEventKind::Failed, 4, 2);
    failed.partial_text = Some("HALF".to_string());
    events.forward(failed);

    events.emit_run_level(AiRunEventKind::Failed, Some("boom".to_string()), None);

    let out = seen.lock().unwrap_or_else(|e| e.into_inner()).clone();
    let kinds: Vec<AiRunEventKind> = out.iter().map(|e| e.kind).collect();
    assert_eq!(
        kinds,
        vec![
            AiRunEventKind::Started,
            AiRunEventKind::Log,
            AiRunEventKind::TurnEnd,
            AiRunEventKind::Log,
            AiRunEventKind::Failed,
        ],
        "one Started, one terminal, per-batch lifecycle swallowed"
    );
    for (i, ev) in out.iter().enumerate() {
        assert_eq!(ev.seq, i as u64, "gap-free monotonic seq: {ev:?}");
        assert_eq!(ev.run_id, "ai-1");
    }
    assert_eq!(out[1].text.as_deref(), Some("hello"), "payload preserved");
    assert_eq!(out[2].cost_usd, Some(0.01));
    assert_eq!(out[4].partial_text.as_deref(), Some("HALF"), "partial echo carried over");
    assert_eq!(events.max_turn(), 2, "max turn across batches");
}

/// `ai_stream_log == false` drops `Log` at the SOURCE (no IPC cost) while every
/// status-changing event still goes out (§8.3).
#[test]
fn stream_log_off_suppresses_only_log_events() {
    let seen: Mutex<Vec<AiRunEvent>> = Mutex::new(Vec::new());
    let sink = |ev: AiRunEvent| seen.lock().unwrap_or_else(|e| e.into_inner()).push(ev);
    let events = RunEvents::new("ai-2".to_string(), &sink, false);
    events.emit_run_level(AiRunEventKind::Started, None, None);
    events.log("noise".to_string());
    let mut log = AiRunEvent::new("ai-2", 1, AiRunEventKind::Log, 1, 0);
    log.text = Some("more noise".to_string());
    events.forward(log);
    let mut awaiting = AiRunEvent::new("ai-2", 2, AiRunEventKind::AwaitingInput, 2, 1);
    awaiting.text = Some("which one?".to_string());
    events.forward(awaiting);
    events.emit_run_level(AiRunEventKind::Done, None, Some(0.02));

    let out = seen.lock().unwrap_or_else(|e| e.into_inner()).clone();
    let kinds: Vec<AiRunEventKind> = out.iter().map(|e| e.kind).collect();
    assert_eq!(
        kinds,
        vec![
            AiRunEventKind::Started,
            AiRunEventKind::AwaitingInput,
            AiRunEventKind::Done
        ]
    );
    // Still gap-free: suppressed lines never consume a seq number.
    for (i, ev) in out.iter().enumerate() {
        assert_eq!(ev.seq, i as u64, "{ev:?}");
    }
}
