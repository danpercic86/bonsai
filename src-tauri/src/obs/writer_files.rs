//! P91 §6 — the pure, stateless half of the log sink's on-disk concern: file
//! naming, chronological listing, start-of-session pruning and the epoch/UTC
//! time helpers. Split out of `writer.rs` to keep that file focused on the
//! stateful [`super::writer::LogWriter`]; these are free functions with no
//! writer state, re-exported from `writer` so every existing `writer::…` call
//! site is unchanged.

use std::path::Path;

use super::writer::Limits;

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
