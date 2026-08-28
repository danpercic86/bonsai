//! P91 §6 — the bounded sink: a `sync_channel(4096)` feeding one dedicated
//! writer thread.
//!
//! ONE concern: getting a record from any thread to the writer **without ever
//! blocking the caller**. Every producer uses `try_send`; a full channel bumps a
//! drop counter and returns immediately, and the writer emits one `drop` record
//! (§3) when it drains. No lock is held across a write, and the git and UI paths
//! never wait on IO.
//!
//! Shutdown is a HANDSHAKE, not a bare `join`: `Shutdown` carries a reply channel,
//! the writer flushes and acks, and only then is the thread joined. The 2 s §6
//! budget covers the WHOLE handshake — the enqueue of the shutdown message as
//! well as the wait for the ack — and on expiry the writer is detached rather
//! than joined. That is what makes "a wedged writer cannot hang application exit"
//! literally true on the main thread, while a healthy writer still provably
//! loses zero records.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{sync_channel, Receiver, SyncSender, TrySendError};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use bonsai_core::error::AppError;

use super::record::{LogLevel, LogPayload, LogRecord, LogSource, RedactionMode};
use super::redact::Redactor;
use super::writer::{now_ms, LogWriter, WriterConfig};

/// Bounded queue depth (§6).
pub const CHANNEL_CAPACITY: usize = 4096;
/// Idle flush cadence (§6).
pub const FLUSH_INTERVAL: Duration = Duration::from_secs(1);
/// Budget for the exit handshake (§6).
pub const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(2);

enum SinkMsg {
    /// Boxed: the enum's size is otherwise the largest payload variant, and this
    /// value is copied into the channel on every single record.
    Record(Box<LogRecord>),
    /// One `log_append` invocation boundary (§5 `unbatched-sink`). Enqueued
    /// best-effort by [`Sink::note_batch`] BEFORE the batch's records, so the
    /// detector sees call boundaries the individual records cannot express.
    BatchMark,
    Flush,
    /// §6.1 — roll to a fresh session file, then delete every OTHER in-scope
    /// file (and the export artifacts in `exports_dir`) ON THE WRITER THREAD, so
    /// no record is lost between the flush and the new file opening. The reply
    /// carries either the counts + the new file name, or the roll error — a
    /// failed roll purged nothing and must never report as success (§6.1).
    RollAndPurge {
        exports_dir: PathBuf,
        reply: SyncSender<Result<PurgeReply, AppError>>,
    },
    Shutdown(SyncSender<()>),
}

/// §6.1 — the writer thread's answer to a `RollAndPurge`.
#[derive(Debug, Clone)]
pub struct PurgeReply {
    pub deleted_files: u32,
    pub deleted_bytes: u64,
    pub failed_files: u32,
    pub deleted_exports: u32,
    /// NAME (never a path) of the fresh, empty file logging continues into.
    pub active_file: String,
}

/// A live logging session: the writer thread plus everything the commands need
/// to describe it.
///
/// The [`Redactor`] lives HERE, not in the writer, so a future `RollAndPurge`
/// (§6.1) keeps the session's ordinals across the new file — `ref#3` stays
/// `ref#3` after a purge, exactly as §7.2 requires.
pub struct Sink {
    tx: SyncSender<SinkMsg>,
    dropped: Arc<AtomicU64>,
    join: Mutex<Option<std::thread::JoinHandle<()>>>,
    redactor: Arc<Redactor>,
    session_id: String,
    started_ms: i64,
    dir: std::path::PathBuf,
    redaction: RedactionMode,
    level: LogLevel,
    /// Records accepted by the channel (a lower bound on records written).
    accepted: AtomicU64,
    /// Anomaly records the detector has emitted this session (§5), surfaced by
    /// `log_session_info`. Shared with the writer thread.
    anomalies: Arc<AtomicU64>,
    /// §6.3 — parts evicted at the cap this session, shared with the writer.
    dropped_parts: Arc<AtomicU64>,
    /// §8.4 — the log is currently not reaching disk (sticky-until-next-success),
    /// shared with the writer. A BOOL only — never the `io::Error` string (privacy).
    write_failed: Arc<AtomicBool>,
}

impl Sink {
    /// Opens the session file and starts the writer thread. Any IO failure is
    /// returned rather than swallowed — the caller surfaces it to the UI.
    pub fn start(cfg: WriterConfig) -> Result<Sink, AppError> {
        let session_id = cfg.session_id.clone();
        let dir = cfg.dir.clone();
        let redaction = cfg.redaction;
        let level = cfg.level;
        // Open BEFORE spawning so a bad directory/permission is reported to the
        // caller instead of dying silently on a detached thread.
        let redactor = Arc::new(Redactor::new());
        let writer = LogWriter::open(cfg, Arc::clone(&redactor))?;
        // Share the writer's eviction counter (§6.3) BEFORE it moves into the
        // thread, so `log_session_info` can read it without touching the writer.
        let dropped_parts = writer.dropped_parts_counter();
        // Share the write-failure flag (§8.4) BEFORE the writer moves into the
        // thread, same pattern as `dropped_parts`.
        let write_failed = writer.write_failed_flag();
        let (tx, rx) = sync_channel::<SinkMsg>(CHANNEL_CAPACITY);
        let dropped = Arc::new(AtomicU64::new(0));
        let thread_dropped = Arc::clone(&dropped);
        let anomalies = Arc::new(AtomicU64::new(0));
        let thread_anomalies = Arc::clone(&anomalies);
        let join = std::thread::Builder::new()
            .name("bonsai-obs-writer".into())
            .spawn(move || writer_loop(writer, rx, thread_dropped, thread_anomalies))
            .map_err(|e| AppError::Other(format!("cannot start log writer thread: {e}")))?;
        Ok(Sink {
            tx,
            dropped,
            join: Mutex::new(Some(join)),
            redactor,
            session_id,
            started_ms: now_ms(),
            dir,
            redaction,
            level,
            accepted: AtomicU64::new(0),
            anomalies,
            dropped_parts,
            write_failed,
        })
    }

    /// §6.3 — parts of this session evicted at the cap.
    pub fn dropped_parts(&self) -> u32 {
        self.dropped_parts.load(Ordering::Relaxed) as u32
    }

    /// §8.4 — whether the log is currently NOT reaching disk (sticky-until-next-
    /// success). A BOOL only; the underlying error string never crosses this API.
    pub fn write_failed(&self) -> bool {
        self.write_failed.load(Ordering::Relaxed)
    }

    /// §6.1 — roll to a fresh file and purge every other in-scope artifact on the
    /// writer thread. BLOCKS the caller until the writer replies, so only ever
    /// called from `spawn_blocking`. Surfaces the roll error rather than reporting
    /// a failed purge as a zero-deletion success.
    pub fn roll_and_purge(&self, exports_dir: PathBuf) -> Result<PurgeReply, AppError> {
        let (reply_tx, reply_rx) = sync_channel::<Result<PurgeReply, AppError>>(1);
        self.tx
            .send(SinkMsg::RollAndPurge {
                exports_dir,
                reply: reply_tx,
            })
            .map_err(|_| AppError::Other("the log writer thread is gone".into()))?;
        reply_rx
            .recv()
            .map_err(|_| AppError::Other("the log writer did not complete the purge".into()))?
    }

    pub fn redactor(&self) -> &Arc<Redactor> {
        &self.redactor
    }

    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    pub fn dir(&self) -> &std::path::Path {
        &self.dir
    }

    pub fn redaction(&self) -> RedactionMode {
        self.redaction
    }

    pub fn level(&self) -> LogLevel {
        self.level
    }

    pub fn dropped(&self) -> u64 {
        self.dropped.load(Ordering::Relaxed)
    }

    pub fn accepted(&self) -> u64 {
        self.accepted.load(Ordering::Relaxed)
    }

    /// Anomaly records emitted by the detector this session (§5).
    pub fn anomalies(&self) -> u64 {
        self.anomalies.load(Ordering::Relaxed)
    }

    /// Signals one `log_append` boundary to the detector (§5 `unbatched-sink`).
    /// Non-blocking, best-effort — a dropped mark only softens one `info` rule.
    pub fn note_batch(&self) {
        let _ = self.tx.try_send(SinkMsg::BatchMark);
    }

    /// NEVER blocks. A full queue costs one atomic increment and the record is
    /// gone — accounted for by the `drop` record the writer emits (§6).
    pub fn enqueue(&self, rec: LogRecord) {
        match self.tx.try_send(SinkMsg::Record(Box::new(rec))) {
            Ok(()) => {
                self.accepted.fetch_add(1, Ordering::Relaxed);
            }
            Err(TrySendError::Full(_)) | Err(TrySendError::Disconnected(_)) => {
                self.dropped.fetch_add(1, Ordering::Relaxed);
            }
        }
    }

    /// Best-effort explicit flush (window blur, `log_session_info`). Also
    /// non-blocking: the writer flushes on its own 1 s cadence regardless.
    pub fn request_flush(&self) {
        let _ = self.tx.try_send(SinkMsg::Flush);
    }

    /// Flush-and-stop handshake (§6). Blocks the CALLER for at most
    /// `SHUTDOWN_TIMEOUT`; only ever called from `spawn_blocking` or the exit hook.
    pub fn shutdown(&self) {
        let deadline = std::time::Instant::now() + SHUTDOWN_TIMEOUT;
        let (ack_tx, ack_rx) = sync_channel::<()>(1);

        // The ENTIRE handshake is budgeted, send included. A blocking `send`
        // would wait for a writer wedged mid-`write_all` (full disk, network
        // drive, an AV scanner holding the handle) — and this runs on the MAIN
        // thread from `RunEvent::ExitRequested`, so that would hang app exit.
        // A dropped log tail is an acceptable price; a frozen window is not.
        let mut msg = SinkMsg::Shutdown(ack_tx);
        let mut sent = false;
        while std::time::Instant::now() < deadline {
            match self.tx.try_send(msg) {
                Ok(()) => {
                    sent = true;
                    break;
                }
                Err(TrySendError::Full(returned)) => {
                    msg = returned;
                    std::thread::sleep(Duration::from_millis(5));
                }
                // Writer already gone: nothing to hand off, nothing to wait for.
                Err(TrySendError::Disconnected(_)) => return,
            }
        }
        if !sent {
            return;
        }
        let remaining = deadline.saturating_duration_since(std::time::Instant::now());
        if ack_rx.recv_timeout(remaining).is_err() {
            // No ack inside the budget ⇒ the writer is wedged. DETACH rather than
            // join: the handle stays in place, the thread dies with the process,
            // and exit proceeds.
            return;
        }
        let handle = self
            .join
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take();
        if let Some(h) = handle {
            // The ack already proves the flush completed, so this join returns
            // immediately.
            let _ = h.join();
        }
    }
}

impl Drop for Sink {
    fn drop(&mut self) {
        // Runs on whatever thread releases the last `Arc` — which, via
        // `ObsState`, can be the main thread. [`Sink::shutdown`] is fully
        // budgeted for exactly that reason.
        self.shutdown();
    }
}

/// The writer thread. Owns the file exclusively; assigns `seq` in write order.
///
/// It also OWNS the session's [`AnomalyDetector`] (§5): this is the one place that
/// sees both frontend and backend records in `seq` order. Each original record is
/// fed to the detector AFTER it is written (so `seq` is known); any derived
/// anomalies are written straight back into the stream. The detector's own
/// anomaly records are never fed back — no recursion.
fn writer_loop(
    mut writer: LogWriter,
    rx: Receiver<SinkMsg>,
    dropped: Arc<AtomicU64>,
    anomalies: Arc<AtomicU64>,
) {
    use super::anomaly::AnomalyDetector;
    let mut reported_drops: u64 = 0;
    let mut detector = AnomalyDetector::new(Some(anomalies));
    loop {
        match rx.recv_timeout(FLUSH_INTERVAL) {
            Ok(SinkMsg::Record(rec)) => {
                emit_pending_drops(&mut writer, &dropped, &mut reported_drops);
                let record = *rec;
                // A write failure (disk full, folder deleted underneath us) must
                // not kill the thread: the next record may well succeed, and a
                // dead writer would silently stop all logging.
                if writer.write_record(record.clone()).is_ok() {
                    let seq = writer.seq();
                    for anomaly in detector.observe(&record, seq) {
                        let _ = writer.write_record(anomaly);
                    }
                }
            }
            Ok(SinkMsg::BatchMark) => {
                // The next record written will carry `seq()+1`; point the rule's
                // refs at that first record of the batch.
                for anomaly in detector.on_batch_mark(now_ms(), writer.seq() + 1) {
                    let _ = writer.write_record(anomaly);
                }
            }
            Ok(SinkMsg::Flush) => {
                let _ = writer.flush();
            }
            Ok(SinkMsg::RollAndPurge { exports_dir, reply }) => {
                // Roll to a fresh, empty file (its header carries afterPurge:true),
                // then delete every OTHER in-scope file — including the just-closed
                // one — plus the export artifacts. Same thread as every write, so
                // no record is lost between the close and the new file opening.
                let result = match writer.roll(true) {
                    Ok(()) => {
                        let active = writer.active_file().to_string();
                        let c =
                            super::writer::purge_scope(writer.dir(), &exports_dir, Some(&active));
                        Ok(PurgeReply {
                            deleted_files: c.deleted_files,
                            deleted_bytes: c.deleted_bytes,
                            failed_files: c.failed_files,
                            deleted_exports: c.deleted_exports,
                            active_file: active,
                        })
                    }
                    Err(e) => Err(e),
                };
                let _ = reply.send(result);
            }
            Ok(SinkMsg::Shutdown(ack)) => {
                emit_pending_drops(&mut writer, &dropped, &mut reported_drops);
                for anomaly in detector.on_session_end() {
                    let _ = writer.write_record(anomaly);
                }
                let _ = writer.flush();
                let _ = ack.send(());
                return;
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                emit_pending_drops(&mut writer, &dropped, &mut reported_drops);
                let _ = writer.flush();
            }
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                for anomaly in detector.on_session_end() {
                    let _ = writer.write_record(anomaly);
                }
                let _ = writer.flush();
                return;
            }
        }
    }
}

/// Emits ONE `drop` record covering everything backpressure discarded since the
/// last report (§3 `DropPayload`), so a storm costs one line, not thousands.
pub(super) fn emit_pending_drops(writer: &mut LogWriter, dropped: &AtomicU64, reported: &mut u64) {
    let total = dropped.load(Ordering::Relaxed);
    if total <= *reported {
        return;
    }
    let since_seq = writer.seq();
    let delta = total - *reported;
    *reported = total;
    let _ = writer.write_record(LogRecord {
        seq: 0,
        ts: now_ms(),
        mono: 0,
        src: LogSource::Rust,
        lvl: LogLevel::Error,
        trace: None,
        span: None,
        caused_by: None,
        payload: LogPayload::Drop {
            dropped: delta,
            since_seq,
        },
    });
}

impl Sink {
    /// Ms since session start, for records minted on the Rust side.
    pub fn mono(&self) -> u64 {
        (now_ms() - self.started_ms).max(0) as u64
    }
}
