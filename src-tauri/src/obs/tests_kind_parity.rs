//! P120 §7 T-R9 / T-R10 (AC 14) — the record `kind` vocabulary cannot drift.
//!
//! T-R9 guards Rust ↔ TypeScript: [`LogPayload::ALL_KINDS`] must equal the
//! `'…'` literals of `export type LogKind` in `src/ipc/types/obs.ts`, in both
//! directions. T-R10 guards Rust ↔ Rust: one sample per variant, so a variant
//! whose `kind()` is missing from `ALL_KINDS` (or a stale `ALL_KINDS` entry with
//! no variant) goes red. `OBS_SCHEMA_VERSION` parity is `tests_schema_parity`'s.
//!
//! `include_str!` for the same reasons as `tests_schema_parity.rs`: cwd-free
//! under nextest, and a moved TS file is a build error, not a silent skip.

use std::collections::BTreeSet;

use super::record::{
    AnomalySeverity, FocusLostPayload, FocusVia, FrameDim, IpcOutcome, LayoutShiftPayload,
    LogLevel, LogPayload, RedactionMode, StabilityPhase, StabilityRegion,
};

/// Removes `//` line comments and `/* … */` block comments. The `LogKind` block
/// holds no string containing either sequence, so a lexer-free strip is exact.
fn strip_comments(src: &str) -> String {
    let mut out = String::with_capacity(src.len());
    let mut rest = src;
    while !rest.is_empty() {
        if let Some(after) = rest.strip_prefix("/*") {
            rest = after.find("*/").map_or("", |i| &after[i + 2..]);
        } else if let Some(after) = rest.strip_prefix("//") {
            rest = after.find('\n').map_or("", |i| &after[i..]);
        } else {
            let mut chars = rest.chars();
            if let Some(c) = chars.next() {
                out.push(c);
            }
            rest = chars.as_str();
        }
    }
    out
}

/// The `'…'` literals of `export type LogKind = …;`.
fn ts_log_kinds(src: &str) -> BTreeSet<String> {
    let code = strip_comments(src);
    let start = code
        .find("export type LogKind =")
        .expect("src/ipc/types/obs.ts must declare `export type LogKind =`");
    let block = &code[start..];
    let block = &block[..block.find(';').expect("LogKind block ends with `;`")];
    block
        .split('\'')
        .skip(1)
        .step_by(2)
        .map(str::to_string)
        .collect()
}

#[test]
fn ts_log_kind_matches_rust_all_kinds() {
    const TS: &str = include_str!("../../../src/ipc/types/obs.ts");
    let ts = ts_log_kinds(TS);
    let rust: BTreeSet<String> = LogPayload::ALL_KINDS
        .iter()
        .map(|k| k.to_string())
        .collect();
    let missing_in_ts: Vec<_> = rust.difference(&ts).collect();
    let missing_in_rust: Vec<_> = ts.difference(&rust).collect();
    assert!(
        missing_in_ts.is_empty() && missing_in_rust.is_empty(),
        "LogKind drift — in Rust ALL_KINDS but not TS LogKind: {missing_in_ts:?}; \
         in TS LogKind but not Rust ALL_KINDS: {missing_in_rust:?}"
    );
}

#[test]
fn strip_comments_removes_prose_kinds() {
    let src = "// 'bogus'\nexport type LogKind =\n  /* 'nope' */ | 'a' // 'b'\n  | 'c';";
    let kinds = ts_log_kinds(src);
    assert_eq!(
        kinds,
        ["a", "c"]
            .iter()
            .map(|s| s.to_string())
            .collect::<BTreeSet<_>>()
    );
}

/// One value per `LogPayload` variant. Adding a variant without extending this
/// list is caught by review (see `record_kind.rs`); adding one here without an
/// `ALL_KINDS` entry is caught below.
fn one_per_variant() -> Vec<LogPayload> {
    let s = String::new;
    vec![
        LogPayload::Session {
            schema: 3,
            app: s(),
            os: s(),
            session_id: s(),
            dev_mode: true,
            level: LogLevel::Debug,
            redaction: RedactionMode::Strict,
            redaction_note: s(),
            home_masking: true,
            after_purge: None,
            truncated: None,
            dropped_parts: None,
        },
        LogPayload::Gesture {
            origin: s(),
            gesture: s(),
        },
        LogPayload::IpcCall {
            cmd: s(),
            args_hash: s(),
            args_shape: None,
            args: None,
            args_omitted: None,
        },
        LogPayload::IpcResult {
            cmd: s(),
            args_hash: s(),
            ms: 0.0,
            outcome: IpcOutcome::Ok,
            err_code: None,
            result_shape: None,
        },
        LogPayload::IpcRecv { cmd: s() },
        LogPayload::Event {
            name: s(),
            reason: None,
            delivered: true,
            listeners: 0,
        },
        LogPayload::Channel {
            name: s(),
            phase: s(),
            chunks: None,
            bytes: None,
            ms: None,
            outcome: None,
        },
        LogPayload::Watcher {
            paths: 0,
            relevant: 0,
            debounce_ms: 0,
            fired: false,
            suppressed: false,
            suppress_reason: None,
            burst_class: None,
            worktree_paths: None,
            ignore_checked: None,
            ignored: None,
        },
        LogPayload::Refresh {
            round: 0,
            scope: s(),
            origins: vec![],
            contributing_traces: vec![],
            collapsed: 0,
            ms: 0.0,
        },
        LogPayload::Render {
            component: s(),
            count: 0,
            since_ms: 0.0,
            changed_props: None,
        },
        LogPayload::RenderTally {
            component: s(),
            window_ms: 0.0,
            renders: 0,
            instances: 0,
            changed_props: None,
            traces: vec![],
            strict_mode_doubling: false,
        },
        LogPayload::Effect {
            component: s(),
            effect: s(),
            run: 0,
            changed_deps: vec![],
            dep_count: 0,
        },
        LogPayload::State {
            store: s(),
            field: s(),
            from: s(),
            to: s(),
        },
        LogPayload::Frame {
            dim: FrameDim::Paint,
            paint_ms: 0.0,
            gap_ms: 0.0,
            over33: 0,
            over100: 0,
            worst_ms: 0.0,
        },
        LogPayload::Error {
            location: s(),
            code: None,
            message: s(),
            stack_hash: None,
        },
        LogPayload::Anomaly {
            rule: s(),
            severity: AnomalySeverity::Info,
            detail: s(),
            refs: vec![],
            traces: vec![],
        },
        LogPayload::Span {
            op: s(),
            ms: 0.0,
            phases: None,
            queued_ms: None,
            pool_inflight: None,
            pool_max: None,
            deadline_frac: None,
            cache: None,
            items: None,
            outcome: None,
        },
        LogPayload::Drop {
            dropped: 0,
            since_seq: 0,
        },
        LogPayload::Truncate {
            reason: s(),
            dropped_parts: 0,
            dropped_part: s(),
            bytes: 0,
            first_retained_seq: None,
        },
        LogPayload::LayoutShift(LayoutShiftPayload {
            region: StabilityRegion::Graph,
            phase: StabilityPhase::Interactive,
            had_recent_input: false,
            resizing: false,
            shifts: 1,
            value: 0.0,
            first_at: 0,
            span_ms: 0,
        }),
        LogPayload::FocusLost(FocusLostPayload {
            from_region: StabilityRegion::Details,
            phase: StabilityPhase::Interactive,
            via: FocusVia::Detached,
            lost_at: 0,
        }),
    ]
}

#[test]
fn every_variant_kind_is_in_all_kinds_and_vice_versa() {
    let all: BTreeSet<&str> = LogPayload::ALL_KINDS.iter().copied().collect();
    assert_eq!(
        all.len(),
        LogPayload::ALL_KINDS.len(),
        "ALL_KINDS has duplicates"
    );
    let samples: BTreeSet<&str> = one_per_variant().iter().map(LogPayload::kind).collect();
    for k in &samples {
        assert!(all.contains(k), "variant kind {k:?} missing from ALL_KINDS");
    }
    assert_eq!(samples, all, "sample kinds must equal ALL_KINDS");
}
