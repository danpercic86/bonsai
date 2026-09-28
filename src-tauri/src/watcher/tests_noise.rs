//! P113d — the debounce loop is the only watcher-record writer. Loop tests
//! drive `debounce_loop` over a hand-fed channel + a private `NoiseCounters`.
//! Unrelated watchers in the same process can write to the global sink, so
//! tick/noise counts are multiples of `MARK` and assertions only read records
//! carrying that marker (or an `errors` count, which nothing else produces).

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::time::SystemTime;

use super::noise::{NoiseCounters, SkippedNoise};
use super::tests::{fixture_repo, serialize_watcher_test, watch_into_channel};
use super::*;

const MARK: u32 = 1000;
const LONG_FLUSH: Duration = Duration::from_secs(30);
const SHORT_FLUSH: Duration = Duration::from_millis(50);
const FIRE_WAIT: Duration = Duration::from_secs(5);

fn start_sink(dir: &Path) -> Arc<crate::obs::sink::Sink> {
    Arc::new(
        crate::obs::sink::Sink::start(crate::obs::writer::WriterConfig {
            dir: dir.to_path_buf(),
            session_id: "snoise01".into(),
            started_secs: 1_787_839_391,
            app_version: "1.5.0".into(),
            os: "windows".into(),
            level: crate::obs::record::LogLevel::Debug,
            redaction: crate::obs::record::RedactionMode::Strict,
            home_mask: None,
            limits: crate::obs::writer::Limits::default(),
        })
        .unwrap(),
    )
}

/// Every `watcher` record on disk, in `seq` order.
fn watcher_records(dir: &Path) -> Vec<serde_json::Value> {
    let mut out = Vec::new();
    for (name, _) in crate::obs::writer::list_log_files(dir) {
        let text = std::fs::read_to_string(dir.join(name)).unwrap();
        for line in text.lines() {
            let v: serde_json::Value = serde_json::from_str(line).unwrap();
            if v["kind"] == "watcher" {
                out.push(v);
            }
        }
    }
    out.sort_by_key(|v| v["seq"].as_u64());
    out
}

fn marked(n: &serde_json::Value) -> bool {
    n.as_u64()
        .is_some_and(|n| n > 0 && n % u64::from(MARK) == 0)
}

/// This test's records only (see the module doc).
fn mine(dir: &Path) -> Vec<serde_json::Value> {
    watcher_records(dir)
        .into_iter()
        .filter(|v| {
            if v["fired"] == true {
                marked(&v["paths"]) || !v["errors"].is_null()
            } else {
                marked(&v["skippedPaths"])
            }
        })
        .collect()
}

fn u(v: &serde_json::Value, k: &str) -> u64 {
    v[k].as_u64().unwrap_or_else(|| panic!("{k} missing: {v}"))
}

/// One relevant worktree tick of `MARK` raw paths.
fn tick() -> WatchTick {
    WatchTick {
        paths: MARK,
        relevant: 1,
        ..WatchTick::default()
    }
}

/// A tick exactly as the notify `Err(_)` arm builds it.
fn error_tick() -> WatchTick {
    WatchTick {
        paths: 1,
        relevant: 1,
        refs: true,
        error: true,
        ..WatchTick::default()
    }
}

/// A running `debounce_loop` on its own thread.
struct Loop {
    tx: Sender<WatchTick>,
    noise: Arc<NoiseCounters>,
    fires: Receiver<BurstClass>,
    calls: Arc<AtomicU32>,
    thread: std::thread::JoinHandle<()>,
    _repo: tempfile::TempDir,
}

impl Loop {
    fn spawn(noise_flush: Duration) -> Self {
        let (_repo, workdir) = fixture_repo();
        let (tx, rx) = channel();
        let (fire_tx, fires) = channel();
        let noise = Arc::new(NoiseCounters::default());
        let calls = Arc::new(AtomicU32::new(0));
        let (n, c) = (Arc::clone(&noise), Arc::clone(&calls));
        let on_change: Box<dyn Fn(BurstClass) + Send> = Box::new(move |class| {
            c.fetch_add(1, Ordering::SeqCst);
            let _ = fire_tx.send(class);
        });
        let thread =
            std::thread::spawn(move || debounce_loop(rx, n, workdir, on_change, noise_flush));
        Loop {
            tx,
            noise,
            fires,
            calls,
            thread,
            _repo,
        }
    }

    fn send(&self, t: WatchTick) {
        self.tx.send(t).unwrap();
    }

    fn wait_fire(&self) -> BurstClass {
        self.fires
            .recv_timeout(FIRE_WAIT)
            .expect("the burst never fired")
    }

    /// Disconnects the channel and joins the thread; returns `on_change` calls.
    fn finish(self) -> u32 {
        drop(self.tx);
        self.thread.join().unwrap();
        self.calls.load(Ordering::SeqCst)
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}

#[test]
fn no_record_until_the_burst_fires() {
    let _s = crate::obs::trace::test_sink_lock();
    let log = tempfile::TempDir::new().unwrap();
    let sink = start_sink(log.path());
    crate::obs::trace::set_active_sink(Some(Arc::clone(&sink)));
    let l = Loop::spawn(LONG_FLUSH);
    for _ in 0..5 {
        l.send(tick());
        std::thread::sleep(Duration::from_millis(40));
    }
    let last_send = now_ms();
    l.wait_fire();
    assert_eq!(l.finish(), 1);
    crate::obs::trace::set_active_sink(None);
    sink.shutdown();

    let recs = mine(log.path());
    assert_eq!(recs.len(), 1, "exactly one record per burst: {recs:?}");
    assert_eq!(recs[0]["fired"], true);
    assert!(
        u(&recs[0], "ts") >= last_send,
        "written before the burst ended"
    );
}

#[test]
fn fired_record_carries_batches_and_burst_ms() {
    let _s = crate::obs::trace::test_sink_lock();
    let log = tempfile::TempDir::new().unwrap();
    let sink = start_sink(log.path());
    crate::obs::trace::set_active_sink(Some(Arc::clone(&sink)));
    let l = Loop::spawn(LONG_FLUSH);
    for _ in 0..4 {
        l.send(tick());
    }
    l.wait_fire();
    l.finish();
    crate::obs::trace::set_active_sink(None);
    sink.shutdown();

    let recs = mine(log.path());
    assert_eq!(recs.len(), 1, "{recs:?}");
    let v = &recs[0];
    assert_eq!(u(v, "batches"), 4);
    assert_eq!(u(v, "paths"), 4 * u64::from(MARK));
    assert_eq!(u(v, "relevant"), 4);
    // `started.elapsed()` at the fire includes the trailing quiet period.
    assert!(u(v, "burstMs") >= DEBOUNCE.as_millis() as u64, "{v}");
    assert!(
        v["errors"].is_null() && v["skippedBatches"].is_null(),
        "{v}"
    );
}

#[test]
fn noise_rides_on_next_fired_record() {
    let _s = crate::obs::trace::test_sink_lock();
    let log = tempfile::TempDir::new().unwrap();
    let sink = start_sink(log.path());
    crate::obs::trace::set_active_sink(Some(Arc::clone(&sink)));
    let l = Loop::spawn(LONG_FLUSH);
    for _ in 0..3 {
        l.noise.absorb(MARK);
    }
    l.send(tick());
    l.wait_fire();
    assert_eq!(l.noise.take(), None, "the fire drained the counters");
    l.finish();
    crate::obs::trace::set_active_sink(None);
    sink.shutdown();

    let recs = mine(log.path());
    assert_eq!(recs.len(), 1, "{recs:?}");
    assert_eq!(recs[0]["fired"], true);
    assert_eq!(u(&recs[0], "skippedBatches"), 3);
    assert_eq!(u(&recs[0], "skippedPaths"), 3 * u64::from(MARK));
}

#[test]
fn noise_summary_when_no_fire() {
    let _s = crate::obs::trace::test_sink_lock();
    let log = tempfile::TempDir::new().unwrap();
    let sink = start_sink(log.path());
    crate::obs::trace::set_active_sink(Some(Arc::clone(&sink)));
    let l = Loop::spawn(SHORT_FLUSH);
    for _ in 0..4 {
        l.noise.absorb(MARK);
    }
    // Several flush periods: the counters drain once, then stay empty.
    std::thread::sleep(SHORT_FLUSH * 6);
    assert_eq!(l.finish(), 0);
    crate::obs::trace::set_active_sink(None);
    sink.shutdown();

    let recs = mine(log.path());
    assert_eq!(
        recs.len(),
        1,
        "one summary, not one per flush tick: {recs:?}"
    );
    let v = &recs[0];
    assert_eq!((u(v, "paths"), u(v, "relevant")), (0, 0));
    assert_eq!(
        (v["fired"].clone(), v["suppressed"].clone()),
        (false.into(), false.into())
    );
    assert_eq!(u(v, "skippedBatches"), 4);
    assert_eq!(u(v, "skippedPaths"), 4 * u64::from(MARK));
    assert!(v["burstClass"].is_null() && v["batches"].is_null(), "{v}");
}

#[test]
fn error_tick_counted_and_refs() {
    let _s = crate::obs::trace::test_sink_lock();
    let log = tempfile::TempDir::new().unwrap();
    let sink = start_sink(log.path());
    crate::obs::trace::set_active_sink(Some(Arc::clone(&sink)));
    let l = Loop::spawn(LONG_FLUSH);
    l.send(error_tick());
    assert_eq!(l.wait_fire(), BurstClass::Refs);
    for _ in 0..3 {
        l.send(error_tick());
    }
    assert_eq!(l.wait_fire(), BurstClass::Refs);
    assert_eq!(l.finish(), 2);
    crate::obs::trace::set_active_sink(None);
    sink.shutdown();

    let recs = mine(log.path());
    let errors: Vec<u64> = recs.iter().map(|v| u(v, "errors")).collect();
    assert_eq!(errors, vec![1, 3], "{recs:?}");
    for v in &recs {
        assert_eq!(v["fired"], true);
        assert_eq!(v["burstClass"], "refs");
        assert_eq!(v["suppressed"], false);
        assert_eq!(u(v, "batches"), u(v, "errors"));
    }
}

#[test]
fn raw_batch_count_recoverable() {
    let _s = crate::obs::trace::test_sink_lock();
    let log = tempfile::TempDir::new().unwrap();
    let sink = start_sink(log.path());
    crate::obs::trace::set_active_sink(Some(Arc::clone(&sink)));
    let l = Loop::spawn(LONG_FLUSH);
    // (true batches, true paths, true errors) tallied as we go.
    let (mut batches, mut paths, mut errs) = (0u64, 0u64, 0u64);
    let noise = |l: &Loop, n: u32| {
        for _ in 0..n {
            l.noise.absorb(MARK);
        }
        (u64::from(n), u64::from(n) * u64::from(MARK))
    };
    // Burst A: noise first, then 3 relevant ticks.
    let (b, p) = noise(&l, 2);
    (batches, paths) = (batches + b, paths + p);
    for _ in 0..3 {
        l.send(tick());
    }
    (batches, paths) = (batches + 3, paths + 3 * u64::from(MARK));
    l.wait_fire();
    // Burst B: relevant + 2 errors, with noise arriving mid-burst.
    l.send(tick());
    let (b, p) = noise(&l, 4);
    l.send(error_tick());
    l.send(error_tick());
    (batches, paths, errs) = (batches + b + 3, paths + p + u64::from(MARK) + 2, errs + 2);
    l.wait_fire();
    // Tail: noise with no burst, flushed at teardown.
    let (b, p) = noise(&l, 1);
    (batches, paths) = (batches + b, paths + p);
    assert_eq!(l.finish(), 2);
    crate::obs::trace::set_active_sink(None);
    sink.shutdown();

    let recs = mine(log.path());
    let opt = |v: &serde_json::Value, k: &str| v[k].as_u64().unwrap_or(0);
    let fired: Vec<_> = recs.iter().filter(|v| v["fired"] == true).collect();
    let raw_batches: u64 = recs
        .iter()
        .map(|v| opt(v, "batches") + opt(v, "skippedBatches"))
        .sum();
    let raw_paths: u64 = fired.iter().map(|v| u(v, "paths")).sum::<u64>()
        + recs.iter().map(|v| opt(v, "skippedPaths")).sum::<u64>();
    let notify_errors: u64 = recs.iter().map(|v| opt(v, "errors")).sum();
    assert_eq!(fired.len(), 2, "{recs:?}");
    assert_eq!(recs.len(), 3, "2 fired + 1 tail summary: {recs:?}");
    assert_eq!(raw_batches, batches);
    assert_eq!(raw_paths, paths);
    assert_eq!(notify_errors, errs);
}

#[test]
fn noise_counters_drained_when_sink_absent() {
    let _s = crate::obs::trace::test_sink_lock();
    crate::obs::trace::set_active_sink(None);
    let log = tempfile::TempDir::new().unwrap();
    let sink = start_sink(log.path());
    let l = Loop::spawn(LONG_FLUSH);
    l.noise.absorb(MARK);
    l.noise.absorb(MARK);
    l.send(tick());
    l.wait_fire();
    assert_eq!(l.noise.take(), None, "drained even with no sink");
    crate::obs::trace::set_active_sink(Some(Arc::clone(&sink)));
    l.send(tick());
    l.wait_fire();
    l.finish();
    crate::obs::trace::set_active_sink(None);
    sink.shutdown();

    let recs = mine(log.path());
    assert_eq!(recs.len(), 1, "{recs:?}");
    assert!(recs[0]["skippedBatches"].is_null(), "{}", recs[0]);
    assert!(recs[0]["skippedPaths"].is_null(), "{}", recs[0]);
}

#[test]
fn teardown_mid_burst_emits_no_fired_record() {
    let _s = crate::obs::trace::test_sink_lock();
    let log = tempfile::TempDir::new().unwrap();
    let sink = start_sink(log.path());
    crate::obs::trace::set_active_sink(Some(Arc::clone(&sink)));
    let l = Loop::spawn(LONG_FLUSH);
    l.send(tick());
    l.noise.absorb(MARK);
    l.noise.absorb(MARK);
    // Disconnect well inside `DEBOUNCE`.
    assert_eq!(
        l.finish(),
        0,
        "on_change must never run for a torn-down burst"
    );
    crate::obs::trace::set_active_sink(None);
    sink.shutdown();

    let recs = mine(log.path());
    assert_eq!(recs.len(), 1, "{recs:?}");
    assert_eq!(recs[0]["fired"], false);
    assert_eq!(u(&recs[0], "skippedBatches"), 2);
}

#[test]
fn dropped_burst_carries_aggregates() {
    let _s = crate::obs::trace::test_sink_lock();
    let log = tempfile::TempDir::new().unwrap();
    let sink = start_sink(log.path());
    crate::obs::trace::set_active_sink(Some(Arc::clone(&sink)));
    // The `tests_ignore.rs` `wt_acc(3, 3)` / `stats(3, 3, 3)` shapes.
    let acc = BurstAcc::new(WatchTick {
        paths: 3,
        relevant: 3,
        refs: false,
        worktree_paths: (0..3)
            .map(|i| std::path::PathBuf::from(format!("dist/f{i}.js")))
            .collect(),
        worktree_count: 3,
        error: false,
    });
    let stats = IgnoreStats {
        worktree_paths: 3,
        ignore_checked: Some(3),
        ignored: Some(3),
        ignored_untracked: Some(3),
        all_relative: true,
    };
    assert_eq!(
        burst_verdict(&acc, Some(&stats), Duration::from_secs(1)),
        BurstVerdict::DropIgnored
    );
    let skipped = SkippedNoise {
        batches: 2,
        paths: 2 * MARK,
    };
    log_watch_fired(&acc, acc.class(), Some(&stats), true, Some(skipped));
    crate::obs::trace::set_active_sink(None);
    sink.shutdown();

    let recs: Vec<_> = watcher_records(log.path())
        .into_iter()
        .filter(|v| v["skippedPaths"] == 2 * MARK)
        .collect();
    assert_eq!(recs.len(), 1, "{recs:?}");
    let v = &recs[0];
    assert_eq!(
        (v["fired"].clone(), v["suppressed"].clone()),
        (true.into(), true.into())
    );
    assert_eq!(v["suppressReason"], "ignored");
    assert_eq!(u(v, "batches"), 1);
    assert!(v["burstMs"].is_u64(), "{v}");
    assert_eq!(u(v, "skippedBatches"), 2);
    assert!(v["errors"].is_null(), "{v}");
}

#[test]
fn noise_counters_absorb_take() {
    let n = NoiseCounters::default();
    assert_eq!(n.take(), None);
    n.absorb(3);
    n.absorb(0);
    assert_eq!(
        n.take(),
        Some(SkippedNoise {
            batches: 2,
            paths: 3
        })
    );
    assert_eq!(n.take(), None);
}

/// Integration: dropping a real `WatcherHandle` flushes the noise tail onto
/// the sink before `drop` returns (the `stop_all_watchers` exit path).
#[test]
fn tail_noise_record_on_handle_drop() {
    let _serial = serialize_watcher_test();
    let _s = crate::obs::trace::test_sink_lock();
    let (_dir, workdir) = fixture_repo();
    let log = tempfile::TempDir::new().unwrap();
    let sink = start_sink(log.path());
    let (handle, _rx) = watch_into_channel(&workdir);
    crate::obs::trace::set_active_sink(Some(Arc::clone(&sink)));
    let objects = workdir.join(".git").join("objects").join("zz");
    std::fs::create_dir_all(&objects).unwrap();
    std::fs::write(objects.join("noise"), "x").unwrap();
    // Let notify deliver the (irrelevant) events; nothing fires for them.
    std::thread::sleep(Duration::from_millis(500));
    drop(handle);
    crate::obs::trace::set_active_sink(None);
    sink.shutdown();

    let with_noise = watcher_records(log.path())
        .into_iter()
        .filter(|v| v["skippedBatches"].as_u64().is_some_and(|n| n > 0))
        .count();
    // Windows may coalesce notify batches: at least one, never an exact count.
    assert!(with_noise >= 1, "no record carried the noise tail");
}
