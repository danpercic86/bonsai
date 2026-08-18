//! Unit tests for the run-level event funnel (P68b, moved here in P68c with the
//! funnel itself). A `#[path]`-included child module so it can reach the private
//! `RunEvents`/`EventState` without widening their visibility (the
//! `session_drain_tests` convention).

use super::*;
use std::sync::Mutex;

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

/// M6 (security audit 2026-08-18). `ai_stream_log: false` means "less noise", NOT
/// "stop telling me what the model touched": `⚙` tool lines and `⛔` denials are the
/// only evidence of what the read grant read and of what the fence refused, and that
/// visibility is what makes the grant acceptable. Suppressing them would let a
/// settings toggle turn the tool grant invisible.
#[test]
fn stream_log_off_still_lets_tool_and_denial_lines_through() {
    let seen: Mutex<Vec<AiRunEvent>> = Mutex::new(Vec::new());
    let sink = |ev: AiRunEvent| seen.lock().unwrap_or_else(|e| e.into_inner()).push(ev);
    let events = RunEvents::new("ai-3".to_string(), &sink, false);

    let mut chatty = AiRunEvent::new("ai-3", 0, AiRunEventKind::Log, 1, 0);
    chatty.text = Some("session abc · model sonnet".to_string());
    events.forward(chatty);

    let mut tool = AiRunEvent::new("ai-3", 1, AiRunEventKind::Log, 2, 0);
    tool.text = Some("⚙ Read(src/a.rs)".to_string());
    tool.notable = true;
    events.forward(tool);

    let mut denied = AiRunEvent::new("ai-3", 2, AiRunEventKind::Log, 3, 0);
    denied.text = Some("⛔ denied Read(/etc/passwd) — outside this repository".to_string());
    denied.notable = true;
    events.forward(denied);

    // Metrics-only heartbeats keep their P68d exemption (text: None).
    let mut metrics = AiRunEvent::new("ai-3", 3, AiRunEventKind::Log, 4, 0);
    metrics.thinking_tokens = Some(350);
    events.forward(metrics);

    let out = seen.lock().unwrap_or_else(|e| e.into_inner()).clone();
    let texts: Vec<Option<String>> = out.iter().map(|e| e.text.clone()).collect();
    assert_eq!(
        texts,
        vec![
            Some("⚙ Read(src/a.rs)".to_string()),
            Some("⛔ denied Read(/etc/passwd) — outside this repository".to_string()),
            None,
        ],
        "the chatty line is suppressed; tool, denial and metrics survive"
    );
    assert!(out[0].notable && out[1].notable, "the flag must survive relabelling");
    for (i, ev) in out.iter().enumerate() {
        assert_eq!(ev.seq, i as u64, "gap-free after suppression: {ev:?}");
    }
}
