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
        };
        w.open_part(0, false)?;
        Ok(w)
    }

    /// Name of the file being written right now (never a path — §6.1).
    pub fn active_file(&self) -> &str {
        &self.active
    }

    /// The last `seq` assigned. Used to stamp `drop` records.
    pub fn seq(&self) -> u64 {
        self.seq
    }

    fn open_part(&mut self, part: u32, after_purge: bool) -> Result<(), AppError> {
        let name = part_name(&self.cfg.session_id, self.cfg.started_secs, part);
        let path = self.cfg.dir.join(&name);
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .map_err(|e| AppError::Io(format!("cannot open log file: {e}")))?;
        self.file = Some(BufWriter::new(file));
        self.active = name;
        self.part = part;
        self.part_bytes = 0;
        self.buffered = 0;
        // Line 1 is ALWAYS the session header (§6) — including on a rotation part,
        // so a part handed over on its own is still self-describing.
        let header = self.header_record(after_purge);
        self.write_record(header)?;
        Ok(())
    }

    fn header_record(&self, after_purge: bool) -> LogRecord {
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
            },
        }
    }

    /// Assigns `seq`, scrubs credentials (§7.2, last step before bytes) and
    /// appends one JSONL line, rotating first when the part cap is hit.
    pub fn write_record(&mut self, mut rec: LogRecord) -> Result<(), AppError> {
        if self.part_bytes >= self.cfg.limits.part_bytes {
            // ORCHESTRATOR-DIRECTED (P91 review round 1; to be ratified into §6).
            //
            // Rotation is never refused, because the newest records are the
            // evidence the user turned Dev mode on to capture. The session is
            // bounded instead by DROPPING THE OLDEST PART once `max_parts` exist:
            // size stays capped at `part_bytes × max_parts` *within* the session,
            // where the earlier "keep appending to the last part" left a storm
            // session growing without any bound at all (`prune` runs only at
            // `open`, and never prunes the last remaining group).
            let next = self.part + 1;
            self.flush()?;
            self.open_part(next, false)?;
            self.trim_session_parts();
        }
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
        f.write_all(line.as_bytes())
            .map_err(|e| AppError::Io(format!("cannot write log record: {e}")))?;
        self.part_bytes += bytes;
        self.buffered += bytes;
        if self.buffered >= self.cfg.limits.flush_bytes {
            self.flush()?;
        }
        Ok(())
    }

    /// Keeps at most `max_parts` files for THIS session, deleting the oldest
    /// first. Called right after a rotation, so the newest part always survives.
    ///
    /// Scoped to the current session group by construction — it filters on this
    /// session's own file-name prefix, so no other session's file is reachable.
    fn trim_session_parts(&self) {
        let group = session_group(&self.active);
        let mut mine: Vec<(String, u64)> = list_log_files(&self.cfg.dir)
            .into_iter()
            .filter(|(n, _)| session_group(n) == group)
            .collect();
        let over = mine.len().saturating_sub(self.cfg.limits.max_parts as usize);
        for (name, _) in mine.drain(..over) {
            if name == self.active {
                continue;
            }
            // Best effort: an undeletable old part is not a reason to stop logging.
            let _ = std::fs::remove_file(self.cfg.dir.join(name));
        }
    }

    /// Pushes the `BufWriter` to the OS. Cheap and idempotent.
    pub fn flush(&mut self) -> Result<(), AppError> {
        if let Some(f) = self.file.as_mut() {
            f.flush()
                .map_err(|e| AppError::Io(format!("cannot flush log file: {e}")))?;
        }
        self.buffered = 0;
        Ok(())
    }

    /// Flushes and CLOSES the current file, then opens a fresh session file whose
    /// header carries `afterPurge` (§6.1 step 1+2). Closing first is what makes a
    /// subsequent delete work on Windows, where an open handle blocks removal.
    ///
    /// Unused until increment 7 wires `RollAndPurge`; kept here because file
    /// lifecycle is this module's concern and the roll must not be re-derived.
    #[allow(dead_code)]
    pub fn roll(&mut self, after_purge: bool) -> Result<(), AppError> {
        self.flush()?;
        self.file = None;
        self.cfg.started_secs = now_secs();
        self.open_part(0, after_purge)
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

/// `bonsai-2026-08-27T14-03-11-<sessionId>.jsonl`, `…-1.jsonl` for part 1+.
pub fn part_name(session_id: &str, started_secs: i64, part: u32) -> String {
    let base = format!("bonsai-{}-{session_id}", utc_stamp(started_secs));
    if part == 0 {
        format!("{base}.jsonl")
    } else {
        format!("{base}-{part}.jsonl")
    }
}

/// The session group a log file belongs to: everything before the optional
/// `-<part>` suffix. Files of one session prune as a unit (§6 counts SESSIONS).
pub fn session_group(name: &str) -> String {
    let stem = name.strip_suffix(".jsonl").unwrap_or(name);
    match stem.rsplit_once('-') {
        Some((head, tail)) if !tail.is_empty() && tail.chars().all(|c| c.is_ascii_digit()) => {
            head.to_string()
        }
        _ => stem.to_string(),
    }
}

/// The rotation part index encoded in a log file name (0 when there is none).
pub fn part_index(name: &str) -> u32 {
    let stem = name.strip_suffix(".jsonl").unwrap_or(name);
    match stem.rsplit_once('-') {
        Some((_, tail)) => tail.parse().unwrap_or(0),
        None => 0,
    }
}

/// Every `bonsai-*.jsonl` in `dir` with its size, in chronological order:
/// session group first (the name begins with a sortable UTC stamp), then part
/// index.
///
/// Sorting by raw name would be WRONG — `-1.jsonl` sorts before `.jsonl`
/// because `-` < `.` in ASCII — which would report part 1 as older than part 0
/// and make "the last entry is the newest" (used by the export fallback) a lie.
pub fn list_log_files(dir: &Path) -> Vec<(String, u64)> {
    let mut out: Vec<(String, u64)> = Vec::new();
    let Ok(rd) = std::fs::read_dir(dir) else {
        return out;
    };
    for entry in rd.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if !name.starts_with("bonsai-") || !name.ends_with(".jsonl") {
            continue;
        }
        let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
        out.push((name, size));
    }
    out.sort_by(|a, b| {
        session_group(&a.0)
            .cmp(&session_group(&b.0))
            .then(part_index(&a.0).cmp(&part_index(&b.0)))
    });
    out
}

/// §6 pruning, run ONCE at session start: keep the `keep_sessions` most recent
/// session groups, then drop oldest groups until the total is under
/// `total_bytes`. This is the ONLY automatic deletion path (§6/decision 4) —
/// notably, turning Dev mode OFF deletes nothing because it never gets here.
pub fn prune(dir: &Path, limits: Limits) {
    let files = list_log_files(dir);
    if files.is_empty() {
        return;
    }
    // Group, preserving the chronological order of first appearance.
    let mut groups: Vec<(String, Vec<(String, u64)>)> = Vec::new();
    for (name, size) in files {
        let key = session_group(&name);
        match groups.last_mut() {
            Some((g, items)) if *g == key => items.push((name, size)),
            _ => groups.push((key, vec![(name, size)])),
        }
    }

    let mut doomed: Vec<String> = Vec::new();
    let over = groups.len().saturating_sub(limits.keep_sessions);
    for (_, items) in groups.drain(..over) {
        doomed.extend(items.into_iter().map(|(n, _)| n));
    }
    let mut total: u64 = groups
        .iter()
        .flat_map(|(_, items)| items.iter().map(|(_, s)| *s))
        .sum();
    let mut idx = 0;
    // Never prune the last remaining group: the newest session is what the user
    // is about to record into.
    while total > limits.total_bytes && idx + 1 < groups.len() {
        for (n, s) in &groups[idx].1 {
            total = total.saturating_sub(*s);
            doomed.push(n.clone());
        }
        idx += 1;
    }
    for name in doomed {
        // Best effort — an undeletable old file is not a reason to refuse to log.
        let _ = std::fs::remove_file(dir.join(name));
    }
}

/// Epoch seconds now (0 if the clock is before the epoch).
pub fn now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Epoch ms now.
pub fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// `2026-08-27T14-03-11` (UTC, filename-safe). Hand-rolled civil-date conversion:
/// the workspace carries no date crate, and adding one for a filename stamp is
/// not worth a dependency. Algorithm is Howard Hinnant's `civil_from_days`.
pub fn utc_stamp(secs: i64) -> String {
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    let (y, m, d) = civil_from_days(days);
    let (hh, mm, ss) = (rem / 3600, (rem % 3600) / 60, rem % 60);
    format!("{y:04}-{m:02}-{d:02}T{hh:02}-{mm:02}-{ss:02}")
}

/// `2026-08-27` (UTC calendar date) — the daily-bucket key for §8 metrics.
/// Same civil-date conversion as [`utc_stamp`], dropping the time-of-day.
pub fn utc_date(secs: i64) -> String {
    let days = secs.div_euclid(86_400);
    let (y, m, d) = civil_from_days(days);
    format!("{y:04}-{m:02}-{d:02}")
}

fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}
