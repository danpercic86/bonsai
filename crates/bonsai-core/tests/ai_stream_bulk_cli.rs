//! P68b — BULK payload + a MID-RUN question against a real child process.
//!
//! This is the one combination the committed batch stub provably cannot reach:
//! `set /p` accepts only ~1 KB of a line and leaves the remainder in the pipe, so
//! the reply read would swallow that residue instead of the answer
//! (`claude_stub.cmd` header, P68 §10.1 amendment). It is also exactly where BOTH
//! serious P68a defects lived — D16(a) the pipe-buffer deadlock (a payload larger
//! than ~64 KB with readers not yet spawned) and D16(b) the unkillable run (the
//! write sitting on the session loop thread) — so leaving it to a manual checkpoint
//! would mean the two hardest-won invariants have no automated guard.
//!
//! The CLI is therefore the `claude_echo` helper binary — a `[[bin]]` target, NOT
//! an example (see its file header and `Cargo.toml:24-32`: an `examples/` target is
//! not built by `cargo test --test <name>` and has no `CARGO_BIN_EXE_*`, so this
//! file would silently skip and the D16 guard would be vacuous). Cross-platform
//! Rust, with no `.cmd`/`.sh` twin to diverge: it reads the whole first turn, reports the byte
//! count it received, asks a question on an open stdin, waits for the reply, and
//! answers with one `===== BONSAI RESULT:` block per requested path.
//!
//! What it deliberately does NOT cover: model quality, real CLI flags/auth, the
//! `--include-partial-messages` shapes, and the native UI — those stay USER
//! CHECKPOINT items.
//!
//! Scratch repos live under `D:\Temp\bonsai-scratch` (C: is full); each test skips
//! with a note when `git` is not on PATH.

mod common;

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::mpsc::channel;
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread;
use std::time::{Duration, Instant};

use bonsai_core::ai::{AiRunEvent, AiRunEventKind, AiRunRegistry, RunControl, RunLimits, RunOpts};
use bonsai_core::error::AppError;
use bonsai_core::git::ai_resolve_stream::{resolve_conflicts_streaming, StreamResolveOpts};
use bonsai_core::git::merge::{merge_branch, MergeOutcome};
use common::{commit_fixed, git, init_repo};

const CLAUDE_BIN_ENV: &str = "BONSAI_CLAUDE_BIN";
const ECHO_MODE_ENV: &str = "BONSAI_ECHO_MODE";
/// Per-file body size that puts the bulk payload well past the ~64 KB OS pipe
/// buffer — the size at which D16(a) used to deadlock every time.
const BIG_LINES: usize = 1_200;

macro_rules! require_git {
    () => {
        if !common::have_git() {
            eprintln!("skipping: `git` CLI not found on PATH");
            return;
        }
    };
}

/// Serialize env-mutating tests: `BONSAI_CLAUDE_BIN` / `BONSAI_ECHO_MODE` are
/// process-global and the helper inherits them.
fn env_lock() -> MutexGuard<'static, ()> {
    static LOCK: Mutex<()> = Mutex::new(());
    LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

/// The `claude_echo` helper binary. Cargo builds every `[[bin]]` of the package
/// when an integration test is selected and hands us its path at COMPILE time, so
/// this can neither go stale nor silently skip — a skip would make the D16 guard
/// vacuous, which is the one thing this file exists to prevent.
fn echo_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_claude_echo"))
}

fn set_echo(mode: &str) {
    std::env::set_var(CLAUDE_BIN_ENV, echo_bin());
    std::env::set_var(ECHO_MODE_ENV, mode);
}

/// Thread-safe event collector standing in for the Tauri Channel.
#[derive(Clone, Default)]
struct Sink(Arc<Mutex<Vec<AiRunEvent>>>);

impl Sink {
    fn events(&self) -> Vec<AiRunEvent> {
        self.0.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }
    fn texts(&self) -> Vec<String> {
        self.events().iter().filter_map(|e| e.text.clone()).collect()
    }
    fn kinds(&self) -> Vec<AiRunEventKind> {
        self.events().iter().map(|e| e.kind).collect()
    }
    fn push(&self, ev: AiRunEvent) {
        self.0.lock().unwrap_or_else(|e| e.into_inner()).push(ev);
    }
}

fn wait_until(mut cond: impl FnMut() -> bool, timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if cond() {
            return true;
        }
        thread::sleep(Duration::from_millis(25));
    }
    cond()
}

fn opts(bulk_max_bytes: usize) -> StreamResolveOpts {
    StreamResolveOpts {
        opts: RunOpts::default(),
        limits: RunLimits {
            // No watchdog: these tests are about the payload and the question, and a
            // human answer (here: another thread) may take arbitrarily long (D3).
            idle_timeout: Duration::ZERO,
            ..RunLimits::default()
        },
        bulk_max_bytes,
        stream_log: true,
    }
}

/// A big-but-mergeable file body: `n` identical lines plus one differing line, so
/// the two sides genuinely conflict while the payload stays large.
fn big_body(side: &str, n: usize) -> String {
    let mut s = String::with_capacity(n * 40);
    for i in 0..n {
        s.push_str(&format!("line {i} of a large translation file\n"));
    }
    s.push_str(&format!("changed by {side}\n"));
    s
}

/// A scratch repo paused in a `bothModified` conflict on every path in `files`.
fn bulk_conflict(files: &[&str]) -> tempfile::TempDir {
    let dir = init_repo();
    let d = dir.path();
    for rel in files {
        if let Some(parent) = Path::new(rel).parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(d.join(parent)).expect("mkdir -p");
            }
        }
        std::fs::write(d.join(rel), big_body("base", BIG_LINES)).expect("write base");
    }
    git(d, &["add", "-A"]);
    commit_fixed(d, "base");

    git(d, &["checkout", "-b", "topic"]);
    for rel in files {
        std::fs::write(d.join(rel), big_body("topic", BIG_LINES)).expect("write topic");
    }
    git(d, &["add", "-A"]);
    commit_fixed(d, "topic side");

    git(d, &["checkout", "main"]);
    for rel in files {
        std::fs::write(d.join(rel), big_body("main", BIG_LINES)).expect("write main");
    }
    git(d, &["add", "-A"]);
    commit_fixed(d, "main side");

    match merge_branch(d, "topic", false).expect("merge") {
        MergeOutcome::Conflicts { paths, .. } => {
            for rel in files {
                assert!(paths.iter().any(|p| p == rel), "expected {rel} to conflict: {paths:?}");
            }
        }
        other => panic!("expected Conflicts, got {other:?}"),
    }
    dir
}

fn paths(list: &[&str]) -> Vec<String> {
    list.iter().map(|s| s.to_string()).collect()
}

// ============================================================ the D16 combination

/// A bulk payload FAR bigger than the OS pipe buffer, plus a mid-run question
/// answered through the registry, in ONE run:
/// - D16(a): the run completes at all — with the write before the readers this
///   deadlocks deterministically at this payload size;
/// - the helper reports the byte count it received, so the payload is proved to
///   arrive IN FULL (no truncation, no split write lost);
/// - the answer travels through stdin (D13) and the second turn resolves;
/// - every requested path is attributed from the reply.
#[test]
fn bulk_payload_with_a_mid_run_question_completes_and_attributes_every_path() {
    require_git!();
    let _g = env_lock();
    set_echo("bulk_ask");

    let files = ["i18n/de.json", "i18n/en.json", "i18n/fr.json"];
    let dir = bulk_conflict(&files);
    let reg = AiRunRegistry::default();
    let (run_id, ctl) = reg.register();
    let sink = Sink::default();
    let collect = {
        let s = sink.clone();
        move |ev: AiRunEvent| s.push(ev)
    };

    let batch = thread::scope(|scope| {
        let handle = scope.spawn(move || {
            resolve_conflicts_streaming(
                dir.path(),
                &paths(&files),
                // One batch: the cap is far above the payload.
                opts(4_000_000),
                &ctl,
                &collect,
            )
        });
        assert!(
            wait_until(|| reg.is_awaiting(&run_id), Duration::from_secs(60)),
            "the sentinel should have blocked the run (finished: {})",
            handle.is_finished()
        );
        // The human answer — arbitrarily late, and the watchdog is paused (D3).
        reg.reply(&run_id, "keep the German plural form".to_string()).expect("reply accepted");
        handle.join().expect("session thread must not panic")
    })
    .expect("a bulk run with a question must complete");
    reg.finish(&run_id);

    assert_eq!(batch.proposals.len(), 3, "{:?}", batch.proposals);
    for (i, rel) in files.iter().enumerate() {
        assert_eq!(batch.proposals[i].path, *rel);
        assert!(
            batch.proposals[i].proposed_text.starts_with(&format!("ECHO {rel} bytes=")),
            "unexpected body: {:?}",
            batch.proposals[i].proposed_text
        );
    }
    assert!(batch.failed.is_empty(), "{:?}", batch.failed);
    assert_eq!(batch.turns, 2, "one question + one answer = two turns");
    assert_eq!(batch.cost_usd, Some(0.0263), "the LAST result's cost within a run (A10)");

    // The payload really did arrive in full: the helper echoes the byte count it
    // read, and it must match what we sent (the "batch 1/1: 3 files (N B)" log line
    // carries our side of the number).
    let texts = sink.texts();
    let sent: usize = texts
        .iter()
        .find_map(|t| {
            let rest = t.strip_prefix("batch 1/1: 3 files (")?;
            rest.strip_suffix(" B)")?.parse().ok()
        })
        .unwrap_or_else(|| panic!("no batch log line: {texts:?}"));
    assert!(
        sent > 128 * 1024,
        "the payload must exceed the OS pipe buffer for this test to mean anything ({sent} B)"
    );
    let received: usize = texts
        .iter()
        .find_map(|t| {
            let rest = t.strip_prefix("received ")?;
            rest.split(' ').next()?.parse().ok()
        })
        .unwrap_or_else(|| panic!("helper never reported a byte count: {texts:?}"));
    // The prompt is prepended to the payload in interactive mode (D13), so the
    // helper legitimately sees a little MORE than the payload itself.
    assert!(
        received >= sent,
        "the child saw {received} B of a {sent} B payload — the write was truncated"
    );

    // The question and the answer are visible in the run's event stream.
    let asked = sink
        .events()
        .into_iter()
        .filter(|e| e.kind == AiRunEventKind::AwaitingInput)
        .collect::<Vec<_>>();
    assert_eq!(asked.len(), 1, "exactly one question");
    assert_eq!(asked[0].text.as_deref(), Some("which locale wins?"));
    assert!(
        texts.iter().any(|t| t.contains("» answered (")),
        "the reply must be logged: {texts:?}"
    );
    // One Started, one terminal event, gap-free seq — across the whole run.
    let kinds = sink.kinds();
    assert_eq!(kinds.first(), Some(&AiRunEventKind::Started));
    assert_eq!(kinds.last(), Some(&AiRunEventKind::Done));
    assert_eq!(kinds.iter().filter(|k| **k == AiRunEventKind::Started).count(), 1);
    for (i, ev) in sink.events().iter().enumerate() {
        assert_eq!(ev.seq, i as u64, "gap-free monotonic seq: {ev:?}");
        assert_eq!(ev.run_id, run_id);
    }
}

/// Cancel while a large payload is still being written (D16(b)): with the write on
/// the session loop thread this run was UNKILLABLE — the loop never got back to
/// polling `ctl.cancel`. The helper waits for a reply that never comes, so the only
/// thing that can end this run is the cancel.
#[test]
fn cancel_works_while_a_bulk_payload_is_in_flight() {
    require_git!();
    let _g = env_lock();
    set_echo("bulk_ask");

    let files = ["i18n/de.json", "i18n/en.json"];
    let dir = bulk_conflict(&files);
    let reg = AiRunRegistry::default();
    let (run_id, ctl) = reg.register();
    let sink = Sink::default();
    let collect = {
        let s = sink.clone();
        move |ev: AiRunEvent| s.push(ev)
    };

    let outcome = thread::scope(|scope| {
        let handle = scope.spawn(move || {
            resolve_conflicts_streaming(dir.path(), &paths(&files), opts(4_000_000), &ctl, &collect)
        });
        // Cancel as soon as the child is talking — i.e. mid-run, with the question
        // pending and nobody about to answer it.
        assert!(
            wait_until(|| sink.events().len() >= 3, Duration::from_secs(60)),
            "the helper produced nothing to cancel into: {:?}",
            sink.texts()
        );
        assert!(reg.cancel(&run_id), "the registry should know the run");
        handle.join().expect("session thread must not panic")
    });
    reg.finish(&run_id);

    match &outcome {
        Err(AppError::AiCancelled(m)) => assert_eq!(m, "cancelled by user"),
        other => panic!("expected AiCancelled, got {other:?}"),
    }
    // D2: everything read before the cancel is still in the caller's hands.
    let kinds = sink.kinds();
    assert_eq!(kinds.last(), Some(&AiRunEventKind::Cancelled), "{kinds:?}");
    assert!(
        sink.texts().iter().any(|t| t.contains("received ")),
        "the log collected before the cancel must survive: {:?}",
        sink.texts()
    );
    let cancelled = sink
        .events()
        .into_iter()
        .filter(|e| e.kind == AiRunEventKind::Cancelled)
        .collect::<Vec<_>>();
    assert_eq!(cancelled.len(), 1);
    assert!(cancelled[0].partial_text.is_some(), "the partial echo is always present (D2)");
}

/// A reply queued while the run was NOT awaiting must never answer a LATER
/// question (P68c FIX 1). The reply channel spans batches — one `RunControl` drives
/// every child of a bulk run — so a second click that arrived inside one 250 ms
/// tick used to sit in the channel and silently answer batch 2's question with
/// batch 1's text.
///
/// The `RunControl` is hand-built rather than taken from the registry on purpose:
/// `AiRunRegistry::reply` refuses a reply unless the run is awaiting, which is
/// exactly the gate the bug slips past, and driving the channel directly makes the
/// scenario deterministic instead of a 250 ms race.
///
/// Negative control (verified): with `drain_stale_replies` removed from
/// `session.rs`, the `answered() == 1` assertion below fails — the leftover reply
/// is consumed on the next tick and the run answers itself.
#[test]
fn a_stale_reply_never_answers_the_next_batchs_question() {
    require_git!();
    let _g = env_lock();
    set_echo("bulk_ask");

    let files = ["i18n/de.json", "i18n/en.json"];
    let dir = bulk_conflict(&files);

    let (reply_tx, replies) = channel::<String>();
    let cancel = Arc::new(AtomicBool::new(false));
    let awaiting = Arc::new(AtomicBool::new(false));
    let ctl = RunControl {
        run_id: "ai-stale-reply-test".to_string(),
        cancel: Arc::clone(&cancel),
        awaiting: Arc::clone(&awaiting),
        pid: Arc::new(AtomicU32::new(0)),
        replies,
    };
    let sink = Sink::default();
    let collect = {
        let s = sink.clone();
        move |ev: AiRunEvent| s.push(ev)
    };
    let asked =
        || sink.events().iter().filter(|e| e.kind == AiRunEventKind::AwaitingInput).count();
    let answered = || sink.texts().iter().filter(|t| t.starts_with("» answered (")).count();

    let batch = thread::scope(|scope| {
        let handle = scope.spawn(move || {
            // A cap below the two-file payload forces two sequential batches, each
            // its own child, each asking its own question.
            resolve_conflicts_streaming(dir.path(), &paths(&files), opts(200_000), &ctl, &collect)
        });

        assert!(
            wait_until(|| asked() >= 1, Duration::from_secs(60)),
            "batch 1 never asked: {:?}",
            sink.texts()
        );
        // TWO answers for ONE question: the session consumes the first and the
        // second is left in the channel.
        reply_tx.send("answer for batch 1".to_string()).expect("queue reply 1");
        reply_tx.send("a stray second click".to_string()).expect("queue reply 2");

        assert!(
            wait_until(|| asked() >= 2, Duration::from_secs(60)),
            "batch 2 never asked: {:?}",
            sink.texts()
        );
        // Batch 2 must still be waiting on a HUMAN several ticks later.
        thread::sleep(Duration::from_millis(1500));
        assert_eq!(
            answered(),
            1,
            "batch 2's question was answered by batch 1's leftover text: {:?}",
            sink.texts()
        );
        assert!(awaiting.load(Ordering::Relaxed), "the run must still be blocked on the user");

        reply_tx.send("answer for batch 2".to_string()).expect("queue reply 3");
        // Never let the scope block forever if the batch geometry ever drifts: a
        // hung run is cancelled so the assertions below report the problem.
        if !wait_until(|| handle.is_finished(), Duration::from_secs(60)) {
            cancel.store(true, Ordering::Relaxed);
        }
        handle.join().expect("session thread must not panic")
    })
    .expect("the run must complete once each question is answered");

    assert_eq!(batch.proposals.len(), 2, "{:?}", batch.proposals);
    let texts = sink.texts();
    assert_eq!(
        texts.iter().filter(|t| t.starts_with("batch ")).count(),
        2,
        "this test needs exactly two batches: {texts:?}"
    );
    assert!(
        texts.iter().any(|t| t.contains("discarded 1 stale reply")),
        "the discarded reply must be logged: {texts:?}"
    );
    assert_eq!(answered(), 2, "exactly one answer per question");
}

/// A reply block missing for one path marks THAT path `failed` and nothing else
/// (D11) — asserted against a real child, not only against `parse_bulk_response`.
#[test]
fn a_missing_result_block_fails_only_its_own_path() {
    require_git!();
    let _g = env_lock();
    set_echo("bulk_missing");

    let files = ["i18n/de.json", "i18n/en.json", "i18n/fr.json"];
    let dir = bulk_conflict(&files);
    let reg = AiRunRegistry::default();
    let (run_id, ctl) = reg.register();
    let sink = Sink::default();
    let collect = {
        let s = sink.clone();
        move |ev: AiRunEvent| s.push(ev)
    };

    let batch =
        resolve_conflicts_streaming(dir.path(), &paths(&files), opts(4_000_000), &ctl, &collect)
            .expect("a missing block must not fail the batch");
    reg.finish(&run_id);

    assert_eq!(batch.proposals.len(), 2, "{:?}", batch.proposals);
    assert_eq!(batch.failed.len(), 1);
    assert_eq!(batch.failed[0].path, "i18n/fr.json");
    assert!(batch.failed[0].reason.contains("no result block"), "{:?}", batch.failed[0]);
    assert_eq!(sink.kinds().last(), Some(&AiRunEventKind::Done));
}

/// A cap smaller than the payload SPLITS the run into sequential batches under ONE
/// run id (§6.3) — never truncates — and the frontend still sees one monotonic
/// sequence with a single `Started` and a single terminal event (the funnel in
/// `ai_resolve_stream`, without which batch 2's events would all be dropped as
/// stale).
#[test]
fn an_oversized_request_is_split_into_sequential_batches_under_one_run_id() {
    require_git!();
    let _g = env_lock();
    // No question here: each batch is its own child, and the point is the split.
    set_echo("bulk");

    let files = ["i18n/de.json", "i18n/en.json", "i18n/fr.json"];
    let dir = bulk_conflict(&files);
    let reg = AiRunRegistry::default();
    let (run_id, ctl) = reg.register();
    let sink = Sink::default();
    let collect = {
        let s = sink.clone();
        move |ev: AiRunEvent| s.push(ev)
    };

    // Each rendered part is ~4 × BIG_LINES lines; a 200 KB cap forces >1 batch
    // while leaving every single file comfortably under it.
    let batch =
        resolve_conflicts_streaming(dir.path(), &paths(&files), opts(200_000), &ctl, &collect)
            .expect("a split run must still complete");
    reg.finish(&run_id);

    let texts = sink.texts();
    let batch_lines: Vec<&String> =
        texts.iter().filter(|t| t.starts_with("batch ")).collect();
    assert!(batch_lines.len() >= 2, "the cap must have split the run: {batch_lines:?}");
    // Every path is still resolved, exactly once.
    assert_eq!(batch.proposals.len(), 3, "{:?}", batch.proposals);
    let mut got: Vec<&str> = batch.proposals.iter().map(|p| p.path.as_str()).collect();
    got.sort_unstable();
    assert_eq!(got, files);
    assert!(batch.failed.is_empty(), "{:?}", batch.failed);
    // Cost is SUMMED across batches (A10: separate processes, independent totals).
    let expected = 0.0263 * batch_lines.len() as f64;
    let cost = batch.cost_usd.expect("cost");
    assert!((cost - expected).abs() < 1e-9, "cost {cost} != {expected}");

    // ONE Started, ONE terminal event, gap-free seq across every batch.
    let kinds = sink.kinds();
    assert_eq!(kinds.iter().filter(|k| **k == AiRunEventKind::Started).count(), 1, "{kinds:?}");
    assert_eq!(kinds.first(), Some(&AiRunEventKind::Started));
    assert_eq!(kinds.last(), Some(&AiRunEventKind::Done));
    assert_eq!(
        kinds.iter().filter(|k| **k == AiRunEventKind::TurnEnd).count(),
        batch_lines.len(),
        "one turn per batch: {kinds:?}"
    );
    for (i, ev) in sink.events().iter().enumerate() {
        assert_eq!(ev.seq, i as u64, "gap-free monotonic seq: {ev:?}");
        assert_eq!(ev.run_id, run_id);
    }
}
