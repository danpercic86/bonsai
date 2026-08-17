//! P68b — the three streaming-AI commands at the command layer.
//!
//! Same harness as `tests_ai.rs`: the `_inner` cores are driven directly (the
//! tauri `test` feature is unusable on this machine — STATUS_ENTRYPOINT_NOT_FOUND),
//! with the local `claude` CLI replaced by the committed NDJSON stub
//! (`crates/bonsai-core/tests/fixtures/claude_stub.*`, `BONSAI_STUB_MODE`). No
//! network, no real CLI.
//!
//! What is asserted HERE (the layer's own contract) rather than in core:
//! the consent gate ordering, the settings → `RunLimits` mapping (including the
//! read-only tool allowlist that is the actual fix for the reported timeout), the
//! Channel-as-callback event stream, and single vs bulk attribution end to end.

use super::tests_support::*;
use super::*;
use bonsai_core::ai::{AiRunEventKind, AiRunRegistry};
use std::sync::{Arc, Mutex};

fn block_on<F: std::future::Future>(f: F) -> F::Output {
    tauri::async_runtime::block_on(f)
}

// `env_lock()` / `set_stub()` come from `tests_support` — the SAME lock and the
// same stub path `tests_ai.rs` uses. Two module-private locks do not exclude each
// other, which showed up as the stub answering in another test's
// `BONSAI_STUB_MODE`.

/// A settings file with AI enabled + consented, plus any extra tweak.
fn consent_file(
    base: &std::path::Path,
    tweak: impl FnOnce(&mut settings::Settings),
) -> std::path::PathBuf {
    let file = base.join("settings.json");
    settings::update(&file, |s| {
        s.ai_enabled = true;
        s.ai_consented = true;
        tweak(s);
    })
    .expect("write consent settings");
    file
}

/// Collects the channel events the command pushes (stands in for the Tauri
/// Channel, exactly as `history_index_build_inner` is tested).
#[derive(Clone, Default)]
struct Sink(Arc<Mutex<Vec<AiRunEvent>>>);

impl Sink {
    fn events(&self) -> Vec<AiRunEvent> {
        self.0.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }
    fn kinds(&self) -> Vec<AiRunEventKind> {
        self.events().iter().map(|e| e.kind).collect()
    }
    fn collector(&self) -> impl Fn(AiRunEvent) + Send + Sync + 'static {
        let inner = Arc::clone(&self.0);
        move |ev| inner.lock().unwrap_or_else(|e| e.into_inner()).push(ev)
    }
}

/// Every event: one run id, gap-free monotonic `seq` from 0, `Started` first (D8 —
/// the UI learns the id it must cancel with from the FIRST event, never from the
/// return value).
fn assert_stream_shape(sink: &Sink, run_id: &str) {
    let events = sink.events();
    assert!(!events.is_empty(), "no events emitted");
    for (i, ev) in events.iter().enumerate() {
        assert_eq!(ev.seq, i as u64, "seq must be monotonic from 0: {ev:?}");
        assert_eq!(ev.run_id, run_id, "one run id per run: {ev:?}");
    }
    assert_eq!(events[0].kind, AiRunEventKind::Started);
}


// ============================================================ consent gate

/// The consent gate fires BEFORE any repo work: an UNKNOWN repo id still yields
/// `AiUnavailable` (not `NoRepo`), which is only possible if the gate runs first.
#[test]
fn stream_refuses_without_consent_before_touching_the_repo() {
    let _g = env_lock();
    set_stub("stream_success");
    let state = AppState::default();
    let base = tempfile::TempDir::new().expect("base");
    let no_consent = base.path().join("default.json"); // absent ⇒ consented = false
    let sink = Sink::default();

    let err = block_on(ai_resolve_conflict_stream_inner(
        &state,
        &AiRunRegistry::default(),
        &no_consent,
        "not-an-open-repo",
        vec!["a.txt".to_string()],
        sink.collector(),
    ))
    .expect_err("no consent ⇒ refuse");
    assert!(matches!(err, AppError::AiUnavailable(_)), "{err:?}");
    assert!(sink.events().is_empty(), "a refused run must not emit any event");
}

/// An empty `paths` list is rejected before the repo is resolved.
#[test]
fn stream_rejects_empty_paths() {
    let _g = env_lock();
    set_stub("stream_success");
    let state = AppState::default();
    let base = tempfile::TempDir::new().expect("base");
    let file = consent_file(base.path(), |_| {});
    let sink = Sink::default();

    let err = block_on(ai_resolve_conflict_stream_inner(
        &state,
        &AiRunRegistry::default(),
        &file,
        "not-an-open-repo",
        Vec::new(),
        sink.collector(),
    ))
    .expect_err("empty paths ⇒ AiFailed");
    assert!(matches!(err, AppError::AiFailed(_)), "{err:?}");
}

// ============================================================ single path

/// One path: the proven single-file payload/prompt, the whole `result` body as the
/// proposal, and a `started → … → done` event stream. WRITES NOTHING.
#[test]
fn stream_single_path_proposes_and_writes_nothing() {
    let _g = env_lock();
    set_stub("stream_success");
    let state = AppState::default();
    let (dir, id, c0) = fixture_repo(&state);
    let base = tempfile::TempDir::new().expect("base");
    let file = consent_file(base.path(), |_| {});
    conflicts_on(&state, &id, dir.path(), &c0, &[("a.txt", "main\n", "feature\n")]);
    let before = std::fs::read(dir.path().join("a.txt")).expect("read a.txt");
    let registry = AiRunRegistry::default();
    let sink = Sink::default();

    let batch = block_on(ai_resolve_conflict_stream_inner(
        &state,
        &registry,
        &file,
        &id,
        vec!["a.txt".to_string()],
        sink.collector(),
    ))
    .expect("single-path resolve");

    assert_eq!(batch.proposals.len(), 1);
    assert_eq!(batch.proposals[0].path, "a.txt");
    assert_eq!(batch.proposals[0].proposed_text, "MERGED_STREAM_BODY");
    assert!(batch.failed.is_empty(), "{:?}", batch.failed);
    assert_eq!(batch.cost_usd, Some(0.0238));
    assert_eq!(batch.turns, 1);

    assert_stream_shape(&sink, &batch.run_id);
    let kinds = sink.kinds();
    assert_eq!(kinds.last(), Some(&AiRunEventKind::Done), "{kinds:?}");
    assert_eq!(
        kinds.iter().filter(|k| **k == AiRunEventKind::Started).count(),
        1,
        "exactly one Started per run"
    );
    // Single-path runs attribute every event to their file (bulk attribution).
    assert!(
        sink.events().iter().all(|e| e.path.as_deref() == Some("a.txt")),
        "events must carry the path for a single-file run"
    );
    // D4: a proposal is bytes only — the worktree and the index are untouched.
    assert_eq!(std::fs::read(dir.path().join("a.txt")).expect("read after"), before);
    // The registry entry is released on the success path too.
    assert_eq!(registry.active(), 0, "finish() must run on every exit path");

    block_on(abort_merge_inner(&state, &id)).expect("cleanup");
}

/// Read-only tools (D10) are what the DEFAULT setting sends, and `none`
/// reproduces the old blind `--tools ""`. This is the real fix for the reported
/// "Claude timed out without understanding the app": the model could not read a
/// single other file.
#[test]
fn stream_sends_the_read_only_tool_allowlist_by_default() {
    let _g = env_lock();
    set_stub("stream_tools");
    let state = AppState::default();
    let (dir, id, c0) = fixture_repo(&state);
    let base = tempfile::TempDir::new().expect("base");
    conflicts_on(&state, &id, dir.path(), &c0, &[("a.txt", "main\n", "feature\n")]);

    let default_file = consent_file(base.path(), |_| {});
    let sink = Sink::default();
    let batch = block_on(ai_resolve_conflict_stream_inner(
        &state,
        &AiRunRegistry::default(),
        &default_file,
        &id,
        vec!["a.txt".to_string()],
        sink.collector(),
    ))
    .expect("default tools run");
    // The stub echoes its argv on stderr (forwarded as `stderr: ` log lines), so a
    // failure here names the command line that actually went out.
    let argv: Vec<String> = sink
        .events()
        .iter()
        .filter_map(|e| e.text.clone())
        .filter(|t| t.contains("ARGV:"))
        .collect();
    assert_eq!(
        batch.proposals[0].proposed_text, "TOOLS_READONLY",
        "the default setting must send --tools Read,Grep,Glob; argv was {argv:?}"
    );

    let none_file = base.path().join("none.json");
    settings::update(&none_file, |s| {
        s.ai_enabled = true;
        s.ai_consented = true;
        s.ai_conflict_tools = AiConflictTools::None;
    })
    .expect("write none settings");
    let batch = block_on(ai_resolve_conflict_stream_inner(
        &state,
        &AiRunRegistry::default(),
        &none_file,
        &id,
        vec!["a.txt".to_string()],
        Sink::default().collector(),
    ))
    .expect("tools=none run");
    assert_eq!(
        batch.proposals[0].proposed_text, "TOOLS_EMPTY",
        "`none` must reproduce today's empty allowlist"
    );

    block_on(abort_merge_inner(&state, &id)).expect("cleanup");
}

/// A single-path request keeps P13's error surface: an ineligible path rejects
/// with its own error instead of resolving to an empty batch.
#[test]
fn stream_single_path_rejects_an_ineligible_path() {
    let _g = env_lock();
    set_stub("stream_success");
    let state = AppState::default();
    let (_dir, id, _c0) = fixture_repo(&state);
    let base = tempfile::TempDir::new().expect("base");
    let file = consent_file(base.path(), |_| {});

    // Not conflicted at all ⇒ the `git` error `get_conflict` produces.
    let err = block_on(ai_resolve_conflict_stream_inner(
        &state,
        &AiRunRegistry::default(),
        &file,
        &id,
        vec!["a.txt".to_string()],
        Sink::default().collector(),
    ))
    .expect_err("a.txt has no conflict");
    assert!(matches!(err, AppError::Git(_)), "{err:?}");

    // Traversal ⇒ invalidName, never a confusing "has no conflict".
    let err = block_on(ai_resolve_conflict_stream_inner(
        &state,
        &AiRunRegistry::default(),
        &file,
        &id,
        vec!["../escape.txt".to_string()],
        Sink::default().collector(),
    ))
    .expect_err("traversal");
    assert!(matches!(err, AppError::InvalidName(_)), "{err:?}");
}

// ============================================================ bulk

/// TWO paths ⇒ ONE run (the locked decision), the bulk delimiter payload, and
/// per-path attribution of the reply. The stub answers with blocks for exactly
/// `a/one.json` and `b/two.json`.
#[test]
fn stream_bulk_attributes_one_run_to_every_path() {
    let _g = env_lock();
    set_stub("stream_bulk");
    let state = AppState::default();
    let (dir, id, c0) = fixture_repo(&state);
    let base = tempfile::TempDir::new().expect("base");
    let file = consent_file(base.path(), |_| {});
    conflicts_on(
        &state,
        &id,
        dir.path(),
        &c0,
        &[
            ("a/one.json", "{\"k\":\"main\"}\n", "{\"k\":\"feature\"}\n"),
            ("b/two.json", "{\"j\":\"main\"}\n", "{\"j\":\"feature\"}\n"),
        ],
    );
    let sink = Sink::default();

    let batch = block_on(ai_resolve_conflict_stream_inner(
        &state,
        &AiRunRegistry::default(),
        &file,
        &id,
        vec!["a/one.json".to_string(), "b/two.json".to_string()],
        sink.collector(),
    ))
    .expect("bulk resolve");

    assert_eq!(batch.proposals.len(), 2, "{:?}", batch.proposals);
    assert_eq!(batch.proposals[0].path, "a/one.json");
    assert_eq!(batch.proposals[0].proposed_text, "ONE_BODY\n");
    assert_eq!(batch.proposals[1].path, "b/two.json");
    assert_eq!(batch.proposals[1].proposed_text, "TWO_BODY\n");
    assert!(batch.failed.is_empty(), "{:?}", batch.failed);
    assert_eq!(batch.cost_usd, Some(0.03));

    assert_stream_shape(&sink, &batch.run_id);
    let kinds = sink.kinds();
    assert_eq!(kinds.last(), Some(&AiRunEventKind::Done));
    assert_eq!(kinds.iter().filter(|k| **k == AiRunEventKind::Started).count(), 1);
    let texts: Vec<String> = sink.events().iter().filter_map(|e| e.text.clone()).collect();
    assert!(
        texts.iter().any(|t| t.starts_with("batch 1/1: 2 files")),
        "the batch plan must be logged: {texts:?}"
    );

    block_on(abort_merge_inner(&state, &id)).expect("cleanup");
}

/// A path the model does not answer for is marked `failed` INDIVIDUALLY — the
/// other file still resolves (D11). The `stream_bulk` stub only ever returns
/// blocks for its two known paths, so a third requested path is the missing one.
#[test]
fn stream_bulk_marks_an_unanswered_path_failed_without_failing_the_batch() {
    let _g = env_lock();
    set_stub("stream_bulk");
    let state = AppState::default();
    let (dir, id, c0) = fixture_repo(&state);
    let base = tempfile::TempDir::new().expect("base");
    let file = consent_file(base.path(), |_| {});
    conflicts_on(
        &state,
        &id,
        dir.path(),
        &c0,
        &[
            ("a/one.json", "{\"k\":\"main\"}\n", "{\"k\":\"feature\"}\n"),
            ("b/two.json", "{\"j\":\"main\"}\n", "{\"j\":\"feature\"}\n"),
            ("c.txt", "main\n", "feature\n"),
        ],
    );

    let batch = block_on(ai_resolve_conflict_stream_inner(
        &state,
        &AiRunRegistry::default(),
        &file,
        &id,
        vec!["a/one.json".to_string(), "b/two.json".to_string(), "c.txt".to_string()],
        Sink::default().collector(),
    ))
    .expect("a missing file must not fail the batch");

    assert_eq!(batch.proposals.len(), 2, "{:?}", batch.proposals);
    assert_eq!(batch.failed.len(), 1);
    assert_eq!(batch.failed[0].path, "c.txt");
    assert!(batch.failed[0].reason.contains("no result block"), "{:?}", batch.failed[0]);

    block_on(abort_merge_inner(&state, &id)).expect("cleanup");
}

/// A bulk request whose ineligible member is skipped individually (D11) — and the
/// remaining single file falls back to the proven single-file format, so the
/// `stream_success` stub (which knows nothing about result blocks) resolves it.
#[test]
fn stream_bulk_skips_an_ineligible_path_individually() {
    let _g = env_lock();
    set_stub("stream_success");
    let state = AppState::default();
    let (dir, id, c0) = fixture_repo(&state);
    let base = tempfile::TempDir::new().expect("base");
    let file = consent_file(base.path(), |_| {});
    conflicts_on(&state, &id, dir.path(), &c0, &[("a.txt", "main\n", "feature\n")]);

    let batch = block_on(ai_resolve_conflict_stream_inner(
        &state,
        &AiRunRegistry::default(),
        &file,
        &id,
        vec!["a.txt".to_string(), "never-conflicted.txt".to_string()],
        Sink::default().collector(),
    ))
    .expect("one eligible path is enough");

    assert_eq!(batch.proposals.len(), 1);
    assert_eq!(batch.proposals[0].proposed_text, "MERGED_STREAM_BODY");
    assert_eq!(batch.failed.len(), 1);
    assert_eq!(batch.failed[0].path, "never-conflicted.txt");

    block_on(abort_merge_inner(&state, &id)).expect("cleanup");
}

/// A file too big for the payload cap is reported per-file and NEVER truncated;
/// with no file left to send, the run fails with a clear message.
#[test]
fn stream_bulk_reports_an_oversize_file_instead_of_truncating() {
    let _g = env_lock();
    set_stub("stream_bulk");
    let state = AppState::default();
    let (dir, id, c0) = fixture_repo(&state);
    let base = tempfile::TempDir::new().expect("base");
    // The smallest permitted cap (20 KB) against two ~12 KB files ⇒ both oversize.
    let file = consent_file(base.path(), |s| s.ai_bulk_max_bytes = 20_000);
    let big_main = format!("{}\n", "m".repeat(12_000));
    let big_feature = format!("{}\n", "f".repeat(12_000));
    conflicts_on(
        &state,
        &id,
        dir.path(),
        &c0,
        &[
            ("a/one.json", &big_main, &big_feature),
            ("b/two.json", &big_main, &big_feature),
        ],
    );

    let err = block_on(ai_resolve_conflict_stream_inner(
        &state,
        &AiRunRegistry::default(),
        &file,
        &id,
        vec!["a/one.json".to_string(), "b/two.json".to_string()],
        Sink::default().collector(),
    ))
    .expect_err("nothing can be sent");
    match &err {
        AppError::AiFailed(m) => assert!(m.contains("too large"), "{m}"),
        other => panic!("expected AiFailed, got {other:?}"),
    }

    block_on(abort_merge_inner(&state, &id)).expect("cleanup");
}

// ============================================================ cancel / reply

/// `ai_cancel_run` is IDEMPOTENT: an unknown or already-finished id resolves `Ok`
/// (a cancel racing a completion is normal — the UI must not show an error).
#[test]
fn cancel_run_is_ok_for_unknown_ids_and_flips_a_live_flag() {
    let registry = AiRunRegistry::default();
    ai_cancel_run_inner(&registry, "ai-nope").expect("unknown id ⇒ Ok");

    let (run_id, ctl) = registry.register();
    ai_cancel_run_inner(&registry, &run_id).expect("known id ⇒ Ok");
    assert!(ctl.cancel.load(std::sync::atomic::Ordering::Relaxed), "flag must be set");
    ai_cancel_run_inner(&registry, &run_id).expect("repeat cancel ⇒ still Ok");
}

/// `ai_reply_run` refuses a run that is unknown or not awaiting input, so a stray
/// reply is never swallowed; an awaiting run receives the text.
#[test]
fn reply_run_requires_a_run_that_is_awaiting_input() {
    let registry = AiRunRegistry::default();
    let err = ai_reply_run_inner(&registry, "ai-nope", "x".into()).expect_err("unknown id");
    assert!(matches!(err, AppError::AiFailed(_)), "{err:?}");

    let (run_id, ctl) = registry.register();
    let err = ai_reply_run_inner(&registry, &run_id, "x".into()).expect_err("not awaiting");
    assert!(matches!(err, AppError::AiFailed(_)), "{err:?}");

    ctl.awaiting.store(true, std::sync::atomic::Ordering::Relaxed);
    ai_reply_run_inner(&registry, &run_id, "the plural form".into()).expect("awaiting ⇒ Ok");
    assert_eq!(ctl.replies.try_recv().ok().as_deref(), Some("the plural form"));
}
