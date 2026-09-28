//! P113d — the two `watcher` record producers. Both run on the debounce thread
//! only (single writer, totally ordered); the notify handler emits nothing.
//!
//! Both go through the process-wide obs sink (`root("watcher")`) and are a
//! no-op when Dev mode is off. The watcher is Tauri-decoupled and holds no
//! `AppHandle`, so it uses the same global sink handle the phase recorder uses.
//! Callers drain `NoiseCounters::take()` BEFORE calling (unconditionally), so a
//! Dev-mode-off period never leaks its noise into a later record.

use crate::obs::record::{LogLevel, LogPayload};

use super::burst::{BurstAcc, IgnoreStats};
use super::noise::SkippedNoise;
use super::{BurstClass, PathClass, DEBOUNCE};

/// One debounce firing — dispatched OR P124b-dropped — with the burst sums
/// plus any noise drained with it. `batches`/`burstMs` are always present (the
/// per-record P113d-era discriminator, contract §3).
pub(super) fn log_watch_fired(
    acc: &BurstAcc,
    class: BurstClass,
    ignore: Option<&IgnoreStats>,
    dropped: bool,
    skipped: Option<SkippedNoise>,
) {
    let Some(sink) = crate::obs::trace::active_sink() else {
        return;
    };
    let meta = crate::obs::TraceMeta::root("watcher");
    let burst_ms = u32::try_from(acc.started.elapsed().as_millis()).unwrap_or(u32::MAX);
    let rec = crate::obs::trace::make_record(
        &sink,
        LogLevel::Debug,
        &meta,
        LogPayload::Watcher {
            paths: acc.paths,
            relevant: acc.relevant,
            debounce_ms: debounce_ms(),
            fired: true,
            // P124b: a dropped all-ignored burst still logs its `fired`
            // record, marked so, and no refresh round follows it.
            suppressed: dropped,
            suppress_reason: dropped.then(|| "ignored".to_string()),
            burst_class: Some(
                match class {
                    PathClass::Worktree => "worktree",
                    PathClass::Refs => "refs",
                }
                .to_string(),
            ),
            worktree_paths: ignore.map(|s| s.worktree_paths),
            ignore_checked: ignore.and_then(|s| s.ignore_checked),
            ignored: ignore.and_then(|s| s.ignored),
            batches: Some(acc.batches),
            burst_ms: Some(burst_ms),
            errors: (acc.errors > 0).then_some(acc.errors),
            skipped_batches: skipped.map(|s| s.batches),
            skipped_paths: skipped.map(|s| s.paths),
        },
    );
    sink.enqueue(rec);
}

/// Noise with no fire to carry it (the `NOISE_FLUSH` tick, or shutdown):
/// `paths: 0, relevant: 0, fired: false, suppressed: false`, every optional
/// field `None` except `skippedBatches` (> 0) / `skippedPaths`.
pub(super) fn log_watch_noise(skipped: SkippedNoise) {
    let Some(sink) = crate::obs::trace::active_sink() else {
        return;
    };
    let meta = crate::obs::TraceMeta::root("watcher");
    let rec = crate::obs::trace::make_record(
        &sink,
        LogLevel::Debug,
        &meta,
        LogPayload::Watcher {
            paths: 0,
            relevant: 0,
            debounce_ms: debounce_ms(),
            fired: false,
            suppressed: false,
            suppress_reason: None,
            burst_class: None,
            worktree_paths: None,
            ignore_checked: None,
            ignored: None,
            batches: None,
            burst_ms: None,
            errors: None,
            skipped_batches: Some(skipped.batches),
            skipped_paths: Some(skipped.paths),
        },
    );
    sink.enqueue(rec);
}

fn debounce_ms() -> u32 {
    u32::try_from(DEBOUNCE.as_millis()).unwrap_or(u32::MAX)
}
