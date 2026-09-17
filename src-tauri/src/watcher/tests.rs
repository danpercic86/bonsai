//! Tests for the filesystem watcher's debounce + burst classification.
//!
//! Split out of `mod.rs` to keep that file under the ~500-line limit. Child
//! module of `watcher`, so `use super::*` reaches its private items
//! (`WatchTick`, `accumulate`, `log_watcher`).

use super::*;
use std::sync::mpsc::{channel, RecvTimeoutError, Sender};
use std::time::Instant;

/// Serializes the fixture-based watcher tests. Each spawns a real
/// ReadDirectoryChangesW watcher whose stale `git2::init` events are
/// flushed lazily; under parallel load concurrent watcher fixtures delay
/// each other's flushes. `watch_into_channel` synchronizes positively via
/// a sentinel file (see there), but serialization still keeps the fixtures
/// from generating events into each other's drains.
/// Poison-recovered so one failing test can't error out the rest.
static WATCHER_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn serialize_watcher_test() -> std::sync::MutexGuard<'static, ()> {
    WATCHER_TEST_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// git2-init'd repo in a temp dir; returns (tempdir, workdir path).
fn fixture_repo() -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempfile::TempDir::new().unwrap();
    git2::Repository::init(dir.path()).unwrap();
    let workdir = dir.path().to_path_buf();
    (dir, workdir)
}

fn watch_into_channel(workdir: &Path) -> (WatcherHandle, mpsc::Receiver<(Instant, BurstClass)>) {
    let (tx, rx): (Sender<(Instant, BurstClass)>, _) = channel();
    let handle = spawn_watcher(
        workdir,
        Box::new(move |class| {
            let _ = tx.send((Instant::now(), class));
        }),
    )
    .unwrap();
    // POSITIVE synchronization against stale `git2::Repository::init`
    // events. On some volumes (observed on this machine's D: scratch
    // drive) ReadDirectoryChangesW flushes the init burst lazily, AFTER
    // the watch is registered — e.g. a late `.git\refs` creation event,
    // which correctly passes the relevance filter and would poison the
    // test. A fixed-width drain window was widened three times
    // (700 ms → 1.5 s → 2.5 s) and still flaked under full-workspace
    // load, so instead of sleeping we synchronize on events we CAUSE:
    //
    // 1. Write a sentinel file and wait for its debounced callback. The
    //    debounce is trailing-edge (300 ms of quiet), so any stale init
    //    event that reached the watcher BEFORE the sentinel write has
    //    been coalesced into this same callback.
    // 2. Delete the sentinel and wait for THAT callback too, so the
    //    deletion event can't leak into the test body.
    // 3. Belt-and-braces residual sweep: drain anything already queued
    //    (try_recv), then keep draining while a conservative 1 s quiet
    //    window still yields events — this is no longer the primary
    //    sync, just insurance against an init flush so late it landed
    //    after step 1's callback fired.
    let sentinel = workdir.join(".bonsai-test-sentinel");
    std::fs::write(&sentinel, "sync").unwrap();
    rx.recv_timeout(Duration::from_secs(10)).expect(
        "watcher never reported the sentinel-file creation — watch not armed or events lost",
    );
    std::fs::remove_file(&sentinel).unwrap();
    rx.recv_timeout(Duration::from_secs(10)).expect(
        "watcher never reported the sentinel-file deletion — watch not armed or events lost",
    );
    while rx.try_recv().is_ok() {}
    while rx.recv_timeout(Duration::from_millis(1000)).is_ok() {}
    (handle, rx)
}

#[test]
fn fires_once_after_touch() {
    let _serial = serialize_watcher_test();
    let (_dir, workdir) = fixture_repo();
    let (_handle, rx) = watch_into_channel(&workdir);

    std::fs::write(workdir.join("a.txt"), "hello").unwrap();

    // Exactly one debounced callback within a generous window…
    rx.recv_timeout(Duration::from_secs(5))
        .expect("expected one repo-changed callback");
    // …and no second one for a further second.
    assert_eq!(
        rx.recv_timeout(Duration::from_secs(1)).unwrap_err(),
        RecvTimeoutError::Timeout,
        "expected no second callback after a single touch"
    );
}

#[test]
fn storm_coalesces() {
    let _serial = serialize_watcher_test();
    let (_dir, workdir) = fixture_repo();
    let (_handle, rx) = watch_into_channel(&workdir);

    // 50 writes in a tight loop (well under 300 ms).
    for i in 0..50 {
        std::fs::write(workdir.join(format!("f{i}.txt")), "x").unwrap();
    }

    // Count callbacks over a 5 s observation window.
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut count = 0u32;
    loop {
        let now = Instant::now();
        if now >= deadline {
            break;
        }
        match rx.recv_timeout(deadline - now) {
            Ok(_) => count += 1,
            Err(_) => break,
        }
    }
    assert!(count >= 1, "storm produced no callback");
    assert!(
        count <= 2,
        "storm produced {count} callbacks, expected <= 2"
    );
}

#[test]
fn git_internals_filtered() {
    let _serial = serialize_watcher_test();
    let (_dir, workdir) = fixture_repo();
    let (_handle, rx) = watch_into_channel(&workdir);

    // Writes under .git/objects must be filtered out. The 1.5 s negative
    // window below is sound after watch_into_channel's sentinel sync: any
    // stale init event that arrived before the sentinel-create callback
    // was coalesced into it (trailing-edge debounce), the sentinel-delete
    // round-trip absorbed everything up to a second quiet period, and the
    // final 1 s residual sweep only returned once the channel stayed quiet
    // for a full second — so a callback inside this window can only come
    // from the writes below, which is exactly what the assertion checks.
    let obj_dir = workdir.join(".git").join("objects").join("aa");
    std::fs::create_dir_all(&obj_dir).unwrap();
    std::fs::write(obj_dir.join("dummy"), "blob").unwrap();
    assert_eq!(
        rx.recv_timeout(Duration::from_millis(1500)).unwrap_err(),
        RecvTimeoutError::Timeout,
        "writes under .git/objects must not trigger a callback"
    );

    // Touching .git/HEAD IS relevant.
    std::fs::write(workdir.join(".git").join("HEAD"), "ref: refs/heads/main\n").unwrap();
    rx.recv_timeout(Duration::from_secs(5))
        .expect("touching .git/HEAD should trigger a callback");
}

#[test]
fn drop_is_clean() {
    let _serial = serialize_watcher_test();
    let (_dir, workdir) = fixture_repo();
    let (handle, rx) = watch_into_channel(&workdir);

    // Explicit drop must not hang (watcher drops, debounce thread joins).
    drop(handle);

    // A write after the drop must produce no callbacks. The channel may
    // still hold callbacks sent BEFORE the drop finished (a stale fixture
    // event firing the debounce right as we dropped — seen under heavy
    // parallel-test load); drain those, then require Disconnected. A
    // Timeout here would mean the debounce thread is still alive.
    std::fs::write(workdir.join("after-drop.txt"), "x").unwrap();
    loop {
        match rx.recv_timeout(Duration::from_secs(1)) {
            Ok(_) => continue, // pre-drop stale callback: not a violation
            Err(e) => {
                assert_eq!(
                    e,
                    RecvTimeoutError::Disconnected,
                    "callback sender should be disconnected after drop"
                );
                break;
            }
        }
    }
}

/// P91 §2.4: a debounce firing records a `watcher` record with the right
/// `paths`/`relevant`/`debounceMs`/`fired`. Drives `log_watcher` directly (a
/// real git-op burst is tester/integration scope).
#[test]
fn watcher_record_shape() {
    let _serial = crate::obs::trace::test_sink_lock();
    let log_dir = tempfile::TempDir::new().unwrap();
    let sink = std::sync::Arc::new(
        crate::obs::sink::Sink::start(crate::obs::writer::WriterConfig {
            dir: log_dir.path().to_path_buf(),
            session_id: "swatch01".into(),
            started_secs: 1_787_839_391,
            app_version: "1.5.0".into(),
            os: "windows".into(),
            level: crate::obs::record::LogLevel::Debug,
            redaction: crate::obs::record::RedactionMode::Strict,
            home_mask: None,
            limits: crate::obs::writer::Limits::default(),
        })
        .unwrap(),
    );
    crate::obs::trace::set_active_sink(Some(std::sync::Arc::clone(&sink)));
    log_watcher(5, 3, true, Some(BurstClass::Refs));
    // P110: a non-fired (raw-batch) record carries NO class — the proof that
    // the pre-P110 record shape is byte-identical when the field is absent.
    log_watcher(2, 0, false, None);
    crate::obs::trace::set_active_sink(None);
    sink.shutdown();

    let mut fired = None;
    let mut not_fired = None;
    for (name, _) in crate::obs::writer::list_log_files(log_dir.path()) {
        let text = std::fs::read_to_string(log_dir.path().join(name)).unwrap();
        for line in text.lines() {
            let v: serde_json::Value = serde_json::from_str(line).unwrap();
            if v["kind"] != "watcher" {
                continue;
            }
            if v["fired"] == true {
                fired = Some(v);
            } else {
                not_fired = Some(v);
            }
        }
    }
    let v = fired.expect("a fired watcher record");
    assert_eq!(v["paths"], 5);
    assert_eq!(v["relevant"], 3);
    assert_eq!(v["debounceMs"], 300);
    assert_eq!(v["suppressed"], false);
    // P110: the burst class rides along on `fired` records…
    assert_eq!(v["burstClass"], "refs");
    // …and is OMITTED entirely otherwise (unchanged legacy record shape).
    let raw = not_fired.expect("a non-fired watcher record");
    assert!(
        raw["burstClass"].is_null(),
        "burstClass must be absent on non-fired records, got {}",
        raw["burstClass"]
    );
}

/// P110 SF3: the burst-accumulation rule itself — deterministic, no
/// wall-clock. Deleting `refs |= t.refs` from `BurstAcc::absorb` (the single
/// fold the debounce loop also uses) fails the two mixed cases below.
#[test]
fn burst_accumulation_is_conservative() {
    let wt = WatchTick {
        paths: 3,
        relevant: 2,
        refs: false,
    };
    let rf = WatchTick {
        paths: 1,
        relevant: 1,
        refs: true,
    };

    // Worktree + Worktree → Worktree.
    assert_eq!(accumulate(wt, &[wt]).2, BurstClass::Worktree);
    // Worktree + Refs → Refs, IN BOTH ORDERS (the fold must not depend on
    // which batch arrived first).
    assert_eq!(accumulate(wt, &[rf]).2, BurstClass::Refs);
    assert_eq!(accumulate(rf, &[wt]).2, BurstClass::Refs);
    // Single-batch windows keep their own class.
    assert_eq!(accumulate(rf, &[]).2, BurstClass::Refs);
    assert_eq!(accumulate(wt, &[]).2, BurstClass::Worktree);
    // Refs survives an arbitrarily long tail of worktree batches (the
    // checkout case: one ref write, then thousands of files landing).
    let tail = vec![wt; 50];
    assert_eq!(accumulate(rf, &tail).2, BurstClass::Refs);

    // Counts still sum across the window (unchanged §2.4 semantics).
    let (paths, relevant, _) = accumulate(wt, &[wt, rf]);
    assert_eq!((paths, relevant), (7, 5));
}

/// P110: a burst of pure working-tree writes classifies as `Worktree` — the
/// commit graph cannot have changed, so the frontend refreshes narrowly.
#[test]
fn worktree_only_burst_classifies_as_worktree() {
    let _serial = serialize_watcher_test();
    let (_dir, workdir) = fixture_repo();
    let (_handle, rx) = watch_into_channel(&workdir);

    for i in 0..20 {
        std::fs::write(workdir.join(format!("w{i}.txt")), "x").unwrap();
    }

    let (_, class) = rx
        .recv_timeout(Duration::from_secs(5))
        .expect("worktree writes should produce a callback");
    assert_eq!(class, BurstClass::Worktree);
    // Any straggler burst from the same writes must also be Worktree.
    while let Ok((_, class)) = rx.recv_timeout(Duration::from_millis(800)) {
        assert_eq!(class, BurstClass::Worktree, "straggler burst misclassified");
    }
}

/// P110: the conservative rule — one refs path anywhere in a coalesced burst
/// makes the WHOLE burst `Refs`.
#[test]
fn mixed_burst_classifies_as_refs() {
    let _serial = serialize_watcher_test();
    let (_dir, workdir) = fixture_repo();
    let (_handle, rx) = watch_into_channel(&workdir);

    for i in 0..10 {
        std::fs::write(workdir.join(format!("m{i}.txt")), "x").unwrap();
    }
    std::fs::write(
        workdir.join(".git").join("HEAD"),
        "ref: refs/heads/main
",
    )
    .unwrap();
    for i in 10..20 {
        std::fs::write(workdir.join(format!("m{i}.txt")), "x").unwrap();
    }

    // The HEAD write may land in the first or a following coalesced burst
    // (natural gaps > 300 ms are exactly the checkout case this fixes), so
    // require that SOME burst in the window is Refs.
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut saw_refs = false;
    while let Some(remaining) = deadline.checked_duration_since(Instant::now()) {
        match rx.recv_timeout(remaining) {
            Ok((_, BurstClass::Refs)) => {
                saw_refs = true;
                break;
            }
            Ok((_, BurstClass::Worktree)) => continue,
            Err(_) => break,
        }
    }
    assert!(
        saw_refs,
        "a burst containing .git/HEAD must classify as Refs"
    );
}
