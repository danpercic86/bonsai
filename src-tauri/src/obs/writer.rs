//! P91 §6 — the on-disk half of the log sink: file naming, JSONL append,
//! rotation, start-of-session pruning and the `session` header record.
//!
//! ONE concern: bytes on disk. It owns no channel and spawns no thread (that is
//! `sink.rs`) and decides nothing about redaction policy (that is `redact.rs`) —
//! it only calls the scrubber immediately before each write, which is the "runs
//! last" guarantee of §7.2.
//!
//! **Roll-readiness (§6.1, increment 7).** [`LogWriter::roll`] closes the current
//! file and opens a fresh one carrying `afterPurge` in its header. That is the
//! entire writer-side half of the future `RollAndPurge` control message: the
//! remaining work is a control variant in `sink.rs` that calls `roll(true)` and
//! then deletes the other `*.jsonl` files. Nothing about the layout here needs to
//! change for it, and the session `Redactor` is deliberately owned by the sink
//! (not by this struct) so a roll keeps the session's ordinals.

use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

use bonsai_core::error::AppError;

use super::record::{LogLevel, LogPayload, LogRecord, LogSource, RedactionMode, OBS_SCHEMA_VERSION};
use super::redact::{self, Redactor};
use super::strict;

/// Size cap of one rotation part (§6).
pub const PART_BYTES: u64 = 16 * 1024 * 1024;
/// Max rotation parts per session (§6).
pub const MAX_PARTS: u32 = 8;
/// Session files kept at session start (§6).
pub const KEEP_SESSIONS: usize = 10;
/// Total bytes kept across all sessions at session start (§6).
pub const TOTAL_BYTES: u64 = 256 * 1024 * 1024;
/// Buffered bytes that force a flush (§6).
pub const FLUSH_BYTES: u64 = 64 * 1024;

/// The §6 caps, as data so tests can drive rotation/pruning without writing
/// hundreds of megabytes. Production always uses [`Limits::default`].
#[derive(Debug, Clone, Copy)]
pub struct Limits {
    pub part_bytes: u64,
    pub max_parts: u32,
    pub keep_sessions: usize,
    pub total_bytes: u64,
    pub flush_bytes: u64,
}

impl Default for Limits {
    fn default() -> Self {
        Limits {
            part_bytes: PART_BYTES,
            max_parts: MAX_PARTS,
            keep_sessions: KEEP_SESSIONS,
            total_bytes: TOTAL_BYTES,
            flush_bytes: FLUSH_BYTES,
        }
    }
}

/// Everything the writer needs to open a session's first file.
#[derive(Debug, Clone)]
pub struct WriterConfig {
    pub dir: PathBuf,
    pub session_id: String,
    /// Epoch SECONDS of session start — the file-name stamp.
    pub started_secs: i64,
    pub app_version: String,
    pub os: String,
    pub level: LogLevel,
    pub redaction: RedactionMode,
    pub limits: Limits,
}

/// Appends JSONL to the current session file, rotating and pruning per §6.
pub struct LogWriter {
    cfg: WriterConfig,
    /// Shared with the sink so a purge roll (§6.1) keeps the session ordinals,
    /// and so strict-mode enforcement can assign them on the writer thread.
    redactor: Arc<Redactor>,
    file: Option<BufWriter<File>>,
    /// Name (not path) of the file currently open.
    active: String,
    part: u32,
    part_bytes: u64,
    buffered: u64,
    seq: u64,
    /// §6.3 — parts of THIS session evicted at the cap. Shared with the sink so
    /// `log_session_info` can report it; the writer thread is the only mutator.
    dropped_parts: Arc<AtomicU64>,
    /// §8.4/§16.9 — the log is currently NOT reaching disk. Sticky-until-a-flush-
    /// reaches-disk: SET on any failed `write_all`, `flush`, or rotation open;
    /// CLEARED only by a `flush()` that actually succeeds (the one operation that
    /// provably hits the disk — a buffered `write_all` "success" proves nothing).
    /// So a persistent failure (disk full, permission loss on the log dir) stays
    /// `true`, and a recovered disk clears within one flush cadence (~1 s). Shared
    /// with the sink so `log_session_info` reads it off the writer thread.
    ///
    /// PRIVACY (increment 1 MUST-FIX): this is a BOOL and nothing else. The
    /// underlying `io::Error` — whose Display embeds the log path — is NEVER
    /// stored here or carried across IPC. The UI shows generic copy only.
    write_failed: Arc<AtomicBool>,
    /// §8.4 — the LAST attempt to open a part failed and the writer is still
    /// holding the previous part's `BufWriter`. Writer-local (never crosses IPC).
    ///
    /// Without it, a persistent rotation block flapped: `open_part` sets
    /// `write_failed`, but the healthy old `BufWriter` keeps flushing fine, so the
    /// sink's ~1 s idle flush cleared the flag again until the next record
    /// re-set it — the §16.9 status row and the header pill oscillated between
    /// "not writing" and healthy while the log was, in fact, stuck. `flush()`
    /// therefore only clears `write_failed` while rotation is healthy.
    rotation_blocked: bool,
}

impl LogWriter {
    /// Prunes old sessions (§6) and opens this session's first file, writing the
    /// `session` header as line 1.
    pub fn open(cfg: WriterConfig, redactor: Arc<Redactor>) -> Result<Self, AppError> {
        std::fs::create_dir_all(&cfg.dir)
            .map_err(|e| AppError::Io(format!("cannot create log dir: {e}")))?;
        prune(&cfg.dir, cfg.limits);
        let mut w = LogWriter {
            cfg,
            redactor,
            file: None,
            active: String::new(),
            part: 0,
            part_bytes: 0,
            buffered: 0,
            seq: 0,
            dropped_parts: Arc::new(AtomicU64::new(0)),
            write_failed: Arc::new(AtomicBool::new(false)),
            rotation_blocked: false,
        };
        w.open_part(0, false)?;
        Ok(w)
    }

    /// Name of the file being written right now (never a path — §6.1).
    pub fn active_file(&self) -> &str {
        &self.active
    }

    /// The logs directory this writer owns — used by the `RollAndPurge` handler.
    pub fn dir(&self) -> &Path {
        &self.cfg.dir
    }

    /// The shared eviction counter (§6.3), cloned by the sink at startup so
    /// `log_session_info` can read it without touching the writer thread.
    pub fn dropped_parts_counter(&self) -> Arc<AtomicU64> {
        Arc::clone(&self.dropped_parts)
    }

    /// The shared write-failure flag (§8.4), cloned by the sink at startup so
    /// `log_session_info` can read it without touching the writer thread. See the
    /// field doc for the sticky-until-next-success semantics and the privacy note.
    pub fn write_failed_flag(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.write_failed)
    }

    /// The last `seq` assigned. Used to stamp `drop` records.
    pub fn seq(&self) -> u64 {
        self.seq
    }

    fn open_part(&mut self, part: u32, after_purge: bool) -> Result<(), AppError> {
        let name = part_name(&self.cfg.session_id, self.cfg.started_secs, part);
        let path = self.cfg.dir.join(&name);
        let file = match OpenOptions::new().create(true).append(true).open(&path) {
            Ok(f) => f,
            Err(e) => {
                // §8.4 — a rotation whose next part cannot be opened (permission
                // loss on the log dir, the path taken by something else) is exactly
                // the "stopped writing" state. Set the flag before surfacing the
                // error. BOOL only; the `io::Error` (path in its Display) stays.
                self.write_failed.store(true, Ordering::Relaxed);
                // The caller (a rotation) keeps writing into the PREVIOUS part's
                // healthy `BufWriter`, which would flush successfully and clear
                // the flag again; latch the block so `flush` cannot do that.
                self.rotation_blocked = true;
                return Err(AppError::Io(format!("cannot open log file: {e}")));
            }
        };
        // A part opened: rotation is healthy again, so a later successful flush is
        // once more allowed to clear `write_failed`.
        self.rotation_blocked = false;
        self.file = Some(BufWriter::new(file));
        self.active = name;
        self.part = part;
        self.part_bytes = 0;
        self.buffered = 0;
        // Line 1 is ALWAYS the session header (§6) — including on a rotation part,
        // so a part handed over on its own is still self-describing. Written via
        // `append_record` (not `write_record`) so a fresh part never re-enters the
        // cap check while opening.
        let header = self.header_record(after_purge);
        self.append_record(header)?;
        Ok(())
    }

    fn header_record(&self, after_purge: bool) -> LogRecord {
        let dropped = self.dropped_parts.load(Ordering::Relaxed) as u32;
        LogRecord {
            seq: 0,
            ts: self.cfg.started_secs.saturating_mul(1000),
            mono: 0,
            src: LogSource::Rust,
            lvl: LogLevel::Info,
            trace: None,
            span: None,
            caused_by: None,
            payload: LogPayload::Session {
                schema: OBS_SCHEMA_VERSION,
                app: self.cfg.app_version.clone(),
                os: self.cfg.os.clone(),
                session_id: self.cfg.session_id.clone(),
                dev_mode: true,
                level: self.cfg.level,
                redaction: self.cfg.redaction,
                redaction_note: self.cfg.redaction.note().to_string(),
                after_purge: after_purge.then_some(true),
                truncated: (dropped > 0).then_some(true),
                dropped_parts: (dropped > 0).then_some(dropped),
            },
        }
    }

    /// Assigns `seq`, scrubs credentials (§7.2, last step before bytes) and
    /// appends one JSONL line, rotating first when the part cap is hit.
    pub fn write_record(&mut self, rec: LogRecord) -> Result<(), AppError> {
        if self.part_bytes >= self.cfg.limits.part_bytes {
            // ORCHESTRATOR-DIRECTED (P91 review round 1; ratified into §6.3).
            //
            // Rotation is never refused, because the newest records are the
            // evidence the user turned Dev mode on to capture. The session is
            // bounded instead by DROPPING THE OLDEST PART once `max_parts` exist:
            // size stays capped at `part_bytes × max_parts` *within* the session,
            // where the earlier "keep appending to the last part" left a storm
            // session growing without any bound at all.
            let next = self.part + 1;
            self.flush()?;
            self.open_part(next, false)?;
            // §6.3: record each on-disk loss in-band, into the surviving newest
            // part, before appending the caller's record.
            for (name, bytes) in self.trim_session_parts() {
                let n = self.dropped_parts.fetch_add(1, Ordering::Relaxed) as u32 + 1;
                let truncate = truncate_record(part_index(&name), bytes, n);
                let _ = self.append_record(truncate);
            }
            // §6.3: enforce the 256 MB total cap continuously, not only at launch.
            prune(&self.cfg.dir, self.cfg.limits);
        }
        self.append_record(rec)
    }

    /// The rotation-free half of [`write_record`]: assign `seq`, redact, write one
    /// line. Kept separate so a `truncate` record or a fresh part's header can be
    /// written WITHOUT re-entering the cap check (which would recurse under the
    /// tiny `part_bytes` limits the tests use).
    fn append_record(&mut self, mut rec: LogRecord) -> Result<(), AppError> {
        self.seq += 1;
        rec.seq = self.seq;
        let mut value = serde_json::to_value(&rec)
            .map_err(|e| AppError::Other(format!("cannot serialize log record: {e}")))?;
        // §7.1 — strict-mode enforcement BEFORE the credential scrubber, because
        // the scrubber must run last (§7.2). The writer, not the producer, is
        // what makes a `redaction: "strict"` header true; see `obs/strict.rs`.
        if self.cfg.redaction == RedactionMode::Strict {
            strict::enforce(&mut value, &self.redactor);
        }
        redact::scrub_value(&mut value);
        let mut line = serde_json::to_string(&value)
            .map_err(|e| AppError::Other(format!("cannot encode log record: {e}")))?;
        // §7.2 — the session salt must reach NO file. It is the one secret that
        // makes the ordinals and `argsHash` more than decoration, so a producer
        // echoing it back (it is handed to the frontend by `log_session_info`)
        // must not be able to park it next to the file it protects. One 32-byte
        // substring check per line, on the writer thread.
        line = redact::scrub_salt(line, &self.redactor.salt_hex());
        line.push('\n');
        let bytes = line.len() as u64;
        let f = self
            .file
            .as_mut()
            .ok_or_else(|| AppError::Io("log file is not open".into()))?;
        // §8.4 — a failed byte-write SETS the flag. It is NOT cleared here: a
        // `write_all` into the `BufWriter` usually only buffers (no disk IO), so a
        // "success" here proves nothing about the disk — clearing on it would make
        // the flag flap under a persistent full disk (buffer-ok, flush-fail, …).
        // The flag is cleared only by a flush that actually reaches disk (below).
        // Only the BOOL is recorded — the `io::Error` (path in its Display) stays.
        if let Err(e) = f.write_all(line.as_bytes()) {
            self.write_failed.store(true, Ordering::Relaxed);
            return Err(AppError::Io(format!("cannot write log record: {e}")));
        }
        self.part_bytes += bytes;
        self.buffered += bytes;
        if self.buffered >= self.cfg.limits.flush_bytes {
            self.flush()?;
        }
        Ok(())
    }

    /// Keeps at most `max_parts` files for THIS session, deleting the oldest
    /// first. Called right after a rotation, so the newest part always survives.
    /// Returns the `(name, bytes)` of each part ACTUALLY removed (size measured
    /// before removal), so the caller can emit one `truncate` record per real
    /// loss (§6.3).
    ///
    /// Scoped to the current session group by construction — it filters on this
    /// session's own file-name prefix, so no other session's file is reachable.
    fn trim_session_parts(&self) -> Vec<(String, u64)> {
        let group = session_group(&self.active);
        let mut mine: Vec<(String, u64)> = list_log_files(&self.cfg.dir)
            .into_iter()
            .filter(|(n, _)| session_group(n) == group)
            .collect();
        let over = mine.len().saturating_sub(self.cfg.limits.max_parts as usize);
        let mut evicted = Vec::new();
        for (name, size) in mine.drain(..over) {
            if name == self.active {
                continue;
            }
            // Best effort: an undeletable old part is not a reason to stop logging.
            // Only a part that was really removed counts as truncation.
            if std::fs::remove_file(self.cfg.dir.join(&name)).is_ok() {
                evicted.push((name, size));
            }
        }
        evicted
    }

    /// Pushes the `BufWriter` to the OS. Cheap and idempotent.
    pub fn flush(&mut self) -> Result<(), AppError> {
        if let Some(f) = self.file.as_mut() {
            // §8.4 — the flush is the ONE operation that provably reaches disk, so
            // it is the sole clear point (sticky-until-a-flush-reaches-disk). A
            // failed flush sets the flag; a successful one clears it. The writer
            // loop flushes at least every 1 s, so a recovered disk clears within
            // ~1 s. BOOL only; the `io::Error` (path in its Display) stays here.
            if let Err(e) = f.flush() {
                self.write_failed.store(true, Ordering::Relaxed);
                return Err(AppError::Io(format!("cannot flush log file: {e}")));
            }
            // …but only while rotation is healthy. A blocked `open_part` leaves
            // this `BufWriter` pointing at the FULL previous part, whose flush
            // still succeeds while nothing more can ever be persisted — clearing
            // on it would make §16.9 report healthy during a persistent block.
            if !self.rotation_blocked {
                self.write_failed.store(false, Ordering::Relaxed);
            }
        }
        self.buffered = 0;
        Ok(())
    }

    /// Flushes and CLOSES the current file, then opens a fresh session file whose
    /// header carries `afterPurge` (§6.1 step 1+2). Closing first is what makes a
    /// subsequent delete work on Windows, where an open handle blocks removal.
    ///
    pub fn roll(&mut self, after_purge: bool) -> Result<(), AppError> {
        self.flush()?;
        self.file = None;
        // Guarantee a filename STRICTLY newer than any existing part of this
        // session. Otherwise, when a purge fires inside the same wall-clock
        // second the session started, `part_name` collides with the old part-0,
        // append-mode reopens it, and pre-purge bytes survive the delete —
        // defeating §6.1's core guarantee that every byte written before the
        // click is gone.
        self.cfg.started_secs = now_secs().max(self.cfg.started_secs + 1);
        // A purge deletes every prior part, so this session's truncation history
        // is erased with them; `afterPurge: true` carries the disclosure instead,
        // and a stale count would make the Dev page warn about vanished files.
        self.dropped_parts.store(0, Ordering::Relaxed);
        self.open_part(0, after_purge)
    }
}

/// §6.3 — one `truncate` record for a part just evicted at the cap. `dropped_idx`
/// is the part index (redacted to `part#<n>`, never a path); `dropped_parts` is
/// the running total after this eviction.
fn truncate_record(dropped_idx: u32, bytes: u64, dropped_parts: u32) -> LogRecord {
    LogRecord {
        seq: 0,
        ts: now_ms(),
        mono: 0,
        src: LogSource::Rust,
        lvl: LogLevel::Warn,
        trace: None,
        span: None,
        caused_by: None,
        payload: LogPayload::Truncate {
            reason: "max-parts".to_string(),
            dropped_parts,
            dropped_part: format!("part#{dropped_idx}"),
            bytes,
            first_retained_seq: None,
        },
    }
}

/// §6.1 — count and delete of in-scope log/export artifacts.
#[derive(Debug, Default, Clone, Copy)]
pub struct PurgeCounts {
    pub deleted_files: u32,
    pub deleted_bytes: u64,
    pub failed_files: u32,
    pub deleted_exports: u32,
}

/// §6.1/§6.2 — delete every in-scope artifact and report honest counts.
///
/// Scope, EXACTLY: `<logs>/*.jsonl`, `<logs>/*.jsonl.tmp`, `<logs>/*.zip`
/// (stray/legacy exports) and `<exports>/*.zip`. Nothing else — `metrics/`,
/// `settings.json` and files outside these two directories are never touched.
///
/// `keep` names the one file in `logs/` to spare (the live file after a purge
/// roll); pass `None` when Dev mode is off and there is no writer.
pub fn purge_scope(logs_dir: &Path, exports_dir: &Path, keep: Option<&str>) -> PurgeCounts {
    let mut c = PurgeCounts::default();
    purge_dir(logs_dir, keep, true, &mut c);
    purge_dir(exports_dir, None, false, &mut c);
    c
}

fn purge_dir(dir: &Path, keep: Option<&str>, is_logs: bool, c: &mut PurgeCounts) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in rd.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        let lower = name.to_ascii_lowercase();
        let in_scope = if is_logs {
            lower.ends_with(".jsonl") || lower.ends_with(".jsonl.tmp") || lower.ends_with(".zip")
        } else {
            lower.ends_with(".zip")
        };
        if !in_scope || keep == Some(name.as_str()) {
            continue;
        }
        let is_zip = lower.ends_with(".zip");
        // Size read IMMEDIATELY before removal, so `deleted_bytes` is what was
        // actually reclaimed (§6.1).
        let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
        match std::fs::remove_file(entry.path()) {
            Ok(()) => {
                c.deleted_files += 1;
                c.deleted_bytes += size;
                if is_zip {
                    c.deleted_exports += 1;
                }
            }
            // An unremovable file (locked, permission denied, or a directory
            // that happens to end in `.jsonl`) is reported, never fatal (§6.1).
            Err(_) => c.failed_files += 1,
        }
    }
}

impl Drop for LogWriter {
    fn drop(&mut self) {
        // Best effort: a failure here has nowhere to go, and the file is closing
        // anyway. The zero-loss guarantee comes from the sink's explicit
        // flush-then-ack shutdown, not from this.
        let _ = self.flush();
    }
}

// §6 file naming, chronological listing, start-of-session pruning and the
// epoch/UTC time helpers live in the sibling `writer_files` module (pure, no
// writer state). Re-exported here so every `writer::…` call site is unchanged.
pub use super::writer_files::{
    list_log_files, now_ms, now_secs, part_index, part_name, prune, session_group, utc_date,
    utc_stamp,
};
