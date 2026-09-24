//! P120 §7 T-R8 (AC 13, P91 §3 binding rule) — `layout.shift`, `focus.lost`
//! and the two anomalies they derive go `LogRecord → LogWriter → disk → read
//! back` byte-identically in `strict` and `raw` mode (header excluded), and no
//! derived `detail` carries a path separator or URL scheme.
//!
//! Its own file rather than an addition to `tests_anomaly_stability.rs`: this is
//! the writer/redaction concern (same split as `tests_repo_redaction.rs`), and
//! it drives the detector only to obtain real anomaly records.

use std::path::Path;

use super::anomaly::AnomalyDetector;
use super::record::{
    FocusLostPayload, FocusVia, LayoutShiftPayload, LogLevel, LogPayload, LogRecord, LogSource,
    RedactionMode, StabilityPhase, StabilityRegion,
};
use super::writer::{Limits, LogWriter, WriterConfig};

fn cfg(dir: &Path, redaction: RedactionMode) -> WriterConfig {
    WriterConfig {
        dir: dir.to_path_buf(),
        session_id: "sp120stab".into(),
        started_secs: 1_772_200_991,
        app_version: "1.6.0".into(),
        os: "windows".into(),
        level: LogLevel::Debug,
        redaction,
        home_mask: None,
        limits: Limits::default(),
    }
}

fn ui(ts: i64, mono: u64, trace: Option<&str>, payload: LogPayload) -> LogRecord {
    LogRecord {
        seq: 0,
        ts,
        mono,
        src: LogSource::Ui,
        lvl: LogLevel::Info,
        trace: trace.map(str::to_string),
        span: None,
        caused_by: None,
        repo: None,
        payload,
    }
}

/// Both stability records plus the `unprompted-shift` and `focus-steal`
/// anomalies the real detector derives from them.
fn records() -> Vec<LogRecord> {
    let shift = ui(
        1_772_200_991_500,
        40,
        None,
        LogPayload::LayoutShift(LayoutShiftPayload {
            region: StabilityRegion::Diff,
            phase: StabilityPhase::Interactive,
            had_recent_input: false,
            resizing: false,
            shifts: 2,
            value: 0.1234,
            first_at: 1_772_200_991_000,
            span_ms: 480,
        }),
    );
    let focus = ui(
        1_772_200_991_550,
        41,
        Some("tr-7"),
        LogPayload::FocusLost(FocusLostPayload {
            from_region: StabilityRegion::Sidebar,
            phase: StabilityPhase::Interactive,
            via: FocusVia::Focusout,
            lost_at: 1_772_200_991_500,
        }),
    );
    let mut det = AnomalyDetector::new(None);
    let mut anomalies = det.observe(&shift, 1);
    anomalies.extend(det.observe(&focus, 2));
    anomalies.extend(det.on_session_end());
    assert_eq!(anomalies.len(), 2, "one unprompted-shift + one focus-steal");
    let mut out = vec![shift, focus];
    for mut a in anomalies {
        // A Rust record at mono 0 is restamped from EACH writer's own session
        // clock, which would differ between the two writers below.
        a.mono = 1;
        a.ts = 1_772_200_992_000;
        out.push(a);
    }
    out
}

/// Writes `recs` in `mode` and returns the non-header lines.
fn written(mode: RedactionMode, recs: Vec<LogRecord>) -> Vec<String> {
    let dir = tempfile::tempdir().expect("tempdir");
    let redactor = std::sync::Arc::new(super::redact::Redactor::with_salt([9; 16]));
    let mut w = LogWriter::open(cfg(dir.path(), mode), redactor).expect("open");
    let name = w.active_file().to_string();
    for r in recs {
        w.write_record(r).expect("write");
    }
    w.flush().expect("flush");
    drop(w);
    std::fs::read_to_string(dir.path().join(&name))
        .expect("read log file")
        .lines()
        .skip(1) // the session header
        .map(str::to_string)
        .collect()
}

#[test]
fn t_r8_stability_lines_are_identical_in_strict_and_raw() {
    let strict = written(RedactionMode::Strict, records());
    let raw = written(RedactionMode::Raw, records());
    assert_eq!(strict.len(), 4);
    assert_eq!(
        strict, raw,
        "record lines must not depend on the redaction mode"
    );
    for kind in ["layout.shift", "focus.lost"] {
        assert!(
            strict
                .iter()
                .any(|l| l.contains(&format!("\"kind\":\"{kind}\""))),
            "{kind} must reach disk: {strict:?}"
        );
    }
    let details: Vec<String> = strict
        .iter()
        .map(|l| serde_json::from_str::<serde_json::Value>(l).expect("valid JSON"))
        .filter(|v| v["kind"] == "anomaly")
        .map(|v| v["detail"].as_str().expect("detail").to_string())
        .collect();
    assert_eq!(details.len(), 2);
    for d in &details {
        assert!(
            !d.contains('/') && !d.contains('\\') && !d.contains("://"),
            "detail must carry no separator: {d:?}"
        );
    }
}
