//! P87 git-activity observability: the ONE event model + the recorder/emitter
//! that feeds it.
//!
//! Every git op that runs hooks or does network I/O can thread a
//! [`GitActivityRecorder`] through the core (`None` = the buffered/no-op path,
//! byte-identical to pre-P87). The recorder is a **fire-and-forget observability**
//! seam layered BESIDE the existing request/response — it never gates or changes
//! an operation's success/error (P87 §Non-goals). The frontend derives two views
//! (a live phase readout + a session log) from this ONE stream; Rust emits
//! structured data only (human copy for phases/labels is the UI's job — §11).
//!
//! Pattern mirrors the AI stream (`crate::ai::stream::AiRunEvent`): a flat struct
//! + a `kind` discriminant, camelCase serde, optionals `skip_serializing_if` so a
//! line event stays tiny on the wire.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

/// Per-event line-length cap, in CHARS (never split a char boundary). One hook
/// line can never forge extra log rows: [`activity_line`] strips C0/C1 + bidi
/// controls AND bounds the length. The TOTAL output is still capped by exec's
/// 64 MiB combined counter; the per-run LINE COUNT is bounded on the frontend.
pub const MAX_ACTIVITY_LINE_CHARS: usize = 2000;

/// One push event on the git-activity stream. Compact by design — at most one
/// line of text, and the optionals are absent unless the `kind` carries them.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitActivityEvent {
    /// Stable for the whole activity; FIRST delivered on `Started`.
    pub id: String,
    /// Monotonic from 0 per activity; the frontend drops any event whose
    /// `seq <= the last seen` for its id (stale/duplicate guard).
    pub seq: u64,
    pub kind: GitActivityKind,
    /// `Started` only.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category: Option<GitActivityCategory>,
    /// `Started` + `Phase`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub phase: Option<GitPhase>,
    /// `StdoutLine` / `StderrLine` only; capped + control-stripped.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<String>,
    /// `HookDone` only (e.g. "pre-commit"…"pre-push").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hook: Option<String>,
    /// `HookDone` + `Finished` (None = killed / no single exit code).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<i32>,
    /// `HookDone` + `Finished`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub success: Option<bool>,
    /// `Progress` only (fetch/pull transfer counts) — §14.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub progress: Option<GitTransferProgress>,
    /// Since the `Started` event.
    pub elapsed_ms: u64,
}

/// Exactly seven kinds — locked by the P87 contract §2.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum GitActivityKind {
    Started,
    Phase,
    StdoutLine,
    StderrLine,
    HookDone,
    Finished,
    Progress,
}

/// Which operation an activity is (set once, on `Started`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum GitActivityCategory {
    Commit,
    Amend,
    MergeCommit,
    Push,
    ForcePush,
    Fetch,
    Pull,
}

/// The current phase of an activity (drives the live "Running pre-push hook…" vs
/// "Pushing…" readout). `hook` is set only for a `RunningHook` phase.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitPhase {
    pub kind: GitPhaseKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hook: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum GitPhaseKind {
    Preparing,
    RunningHook,
    Network,
    Finalizing,
}

/// Which captured stream a line came from (CLI exec seam only).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum GitStream {
    Stdout,
    Stderr,
}

/// Structured fetch/pull network-transfer counts — mirrors `git2::Progress`.
/// Present ONLY on a `Progress` event, emitted from git2
/// `RemoteCallbacks::transfer_progress` (§14), never the exec seam.
/// `total_deltas`/`indexed_deltas` are `Some` only during delta-resolution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitTransferProgress {
    pub received_objects: u32,
    pub total_objects: u32,
    pub indexed_objects: u32,
    pub received_bytes: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_deltas: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub indexed_deltas: Option<u32>,
}

/// Object-safe sink the callers thread through core (`None` = buffered/no-op
/// path). Every method takes `&self`, so it can be shared as
/// `Arc<ActivityEmitter>` and handed to core as `&dyn GitActivityRecorder`.
pub trait GitActivityRecorder: Send + Sync {
    /// A phase transition (e.g. `RunningHook` while a hook runs, then `Network`).
    fn phase(&self, kind: GitPhaseKind, hook: Option<&str>);
    /// One captured output line (CLI exec seam).
    fn line(&self, stream: GitStream, line: &str);
    /// A hook finished with the given exit `code`/`success`.
    fn hook_done(&self, hook: &str, code: Option<i32>, success: bool);
    /// Fetch/pull network-transfer counts (§14). DEFAULT no-op → additive: adding
    /// it breaks no existing impl, and it fires only when a recorder is present.
    fn progress(&self, _p: GitTransferProgress) {}
}

/// Owns the activity id + a monotonic seq + the start [`Instant`], and forwards
/// each built event to `emit`. `Send + Sync` (its seq is an `AtomicU64`), so an
/// `Arc<ActivityEmitter>` crosses into `spawn_blocking` and core sees it as
/// `&dyn GitActivityRecorder`.
pub struct ActivityEmitter {
    id: String,
    start: Instant,
    seq: AtomicU64,
    emit: Box<dyn Fn(GitActivityEvent) + Send + Sync>,
}

impl ActivityEmitter {
    pub fn new(id: String, emit: Box<dyn Fn(GitActivityEvent) + Send + Sync>) -> Self {
        ActivityEmitter {
            id,
            start: Instant::now(),
            seq: AtomicU64::new(0),
            emit,
        }
    }

    fn next_seq(&self) -> u64 {
        self.seq.fetch_add(1, Ordering::Relaxed)
    }

    fn elapsed_ms(&self) -> u64 {
        u64::try_from(self.start.elapsed().as_millis()).unwrap_or(u64::MAX)
    }

    /// A blank event carrying only the run-level fields; the caller fills the
    /// kind-specific optionals, so each emit site stays one short statement.
    fn base(&self, kind: GitActivityKind) -> GitActivityEvent {
        GitActivityEvent {
            id: self.id.clone(),
            seq: self.next_seq(),
            kind,
            category: None,
            phase: None,
            line: None,
            hook: None,
            code: None,
            success: None,
            progress: None,
            elapsed_ms: self.elapsed_ms(),
        }
    }

    /// The activity's first event (seq 0). Carries the category + the initial
    /// phase so the UI has both from the very first push.
    pub fn started(&self, category: GitActivityCategory, phase: GitPhaseKind) {
        let mut ev = self.base(GitActivityKind::Started);
        ev.category = Some(category);
        ev.phase = Some(GitPhase {
            kind: phase,
            hook: None,
        });
        (self.emit)(ev);
    }

    /// The terminal event. `code`/`success` mirror the op's outcome (best-effort;
    /// `None` code = killed / no single exit code — e.g. a hook rejection).
    pub fn finished(&self, code: Option<i32>, success: bool) {
        let mut ev = self.base(GitActivityKind::Finished);
        ev.code = code;
        ev.success = Some(success);
        (self.emit)(ev);
    }
}

impl GitActivityRecorder for ActivityEmitter {
    fn phase(&self, kind: GitPhaseKind, hook: Option<&str>) {
        let mut ev = self.base(GitActivityKind::Phase);
        ev.phase = Some(GitPhase {
            kind,
            hook: hook.map(str::to_string),
        });
        (self.emit)(ev);
    }

    fn line(&self, stream: GitStream, line: &str) {
        let kind = match stream {
            GitStream::Stdout => GitActivityKind::StdoutLine,
            GitStream::Stderr => GitActivityKind::StderrLine,
        };
        let mut ev = self.base(kind);
        // The single funnel to the wire: bound + control-strip HERE so any line
        // producer (present or future, exec seam or otherwise) is safe. Stronger
        // than pre-truncating at one call site.
        ev.line = Some(activity_line(line));
        (self.emit)(ev);
    }

    fn hook_done(&self, hook: &str, code: Option<i32>, success: bool) {
        let mut ev = self.base(GitActivityKind::HookDone);
        ev.hook = Some(hook.to_string());
        ev.code = code;
        ev.success = Some(success);
        (self.emit)(ev);
    }

    fn progress(&self, p: GitTransferProgress) {
        let mut ev = self.base(GitActivityKind::Progress);
        ev.progress = Some(p);
        (self.emit)(ev);
    }
}

/// A process-unique activity id, e.g. `git-<pid>-<counter>`. Cheap + unguessable
/// enough for a local channel key (mirrors `AiRunRegistry`'s id shape).
pub fn new_activity_id() -> String {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    format!(
        "git-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    )
}

/// Sanitize one raw output line for the wire: strip the control chars that let
/// one line pretend to be several (or read backwards), THEN bound the length to
/// [`MAX_ACTIVITY_LINE_CHARS`] chars (trailing `…`). Order matters — stripping
/// first collapses a `\n`-laden blob before truncation measures it.
pub fn activity_line(raw: &str) -> String {
    truncate_chars(&strip_control_chars(raw), MAX_ACTIVITY_LINE_CHARS)
}

/// Drop the characters that let one line of output pretend to be several, or to
/// read backwards: C0/C1 controls (so `\n`, `\r`, `\t`, `\u{7f}`) plus the bidi
/// overrides and isolates. Mirrors the rule in `crate::ai::stream` (duplicated
/// per the P87 contract §2, to keep `git` decoupled from `ai` internals).
fn strip_control_chars(text: &str) -> String {
    text.chars()
        .filter(|c| {
            let bidi = matches!(c,
                '\u{200e}' | '\u{200f}' | '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}');
            !c.is_control() && !bidi
        })
        .collect()
}

/// Truncate to `cap` CHARS (never bytes — a split boundary would corrupt UTF-8
/// on the wire). An over-long string ends in `…` and is exactly `cap` chars long
/// (including `cap == 0`, which yields the empty string).
fn truncate_chars(text: &str, cap: usize) -> String {
    if cap == 0 {
        return String::new();
    }
    if text.chars().count() <= cap {
        return text.to_string();
    }
    let mut out: String = text.chars().take(cap.saturating_sub(1)).collect();
    out.push('…');
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    /// Collects every emitted event so an assertion can inspect the full sequence.
    fn recording() -> (Arc<ActivityEmitter>, Arc<Mutex<Vec<GitActivityEvent>>>) {
        let log = Arc::new(Mutex::new(Vec::new()));
        let sink = Arc::clone(&log);
        let emitter = Arc::new(ActivityEmitter::new(
            "git-test-0".to_string(),
            Box::new(move |ev| sink.lock().expect("lock").push(ev)),
        ));
        (emitter, log)
    }

    #[test]
    fn started_is_seq_zero_and_carries_category_and_phase() {
        let (em, log) = recording();
        em.started(GitActivityCategory::Push, GitPhaseKind::Preparing);
        let events = log.lock().expect("lock");
        assert_eq!(events.len(), 1);
        let e = &events[0];
        assert_eq!(e.seq, 0);
        assert_eq!(e.kind, GitActivityKind::Started);
        assert_eq!(e.category, Some(GitActivityCategory::Push));
        assert_eq!(
            e.phase,
            Some(GitPhase {
                kind: GitPhaseKind::Preparing,
                hook: None
            })
        );
    }

    #[test]
    fn seq_is_monotonic_across_kinds() {
        let (em, log) = recording();
        em.started(GitActivityCategory::Commit, GitPhaseKind::Preparing);
        em.phase(GitPhaseKind::RunningHook, Some("pre-commit"));
        em.line(GitStream::Stdout, "hello");
        em.hook_done("pre-commit", Some(0), true);
        em.finished(Some(0), true);
        let events = log.lock().expect("lock");
        let seqs: Vec<u64> = events.iter().map(|e| e.seq).collect();
        assert_eq!(seqs, vec![0, 1, 2, 3, 4]);
        assert_eq!(events[1].phase.as_ref().map(|p| p.hook.as_deref()), Some(Some("pre-commit")));
        assert_eq!(events[2].line.as_deref(), Some("hello"));
        assert_eq!(events[3].kind, GitActivityKind::HookDone);
        assert_eq!(events[3].hook.as_deref(), Some("pre-commit"));
    }

    #[test]
    fn line_kind_follows_stream() {
        let (em, log) = recording();
        em.line(GitStream::Stdout, "out");
        em.line(GitStream::Stderr, "err");
        let events = log.lock().expect("lock");
        assert_eq!(events[0].kind, GitActivityKind::StdoutLine);
        assert_eq!(events[1].kind, GitActivityKind::StderrLine);
    }

    /// `line` is control-stripped so an injected `\n` can never forge extra rows.
    #[test]
    fn line_strips_controls() {
        let (em, log) = recording();
        em.line(GitStream::Stdout, "safe\nINJECTED\r\tdone");
        let events = log.lock().expect("lock");
        assert_eq!(events[0].line.as_deref(), Some("safeINJECTEDdone"));
    }

    #[test]
    fn activity_line_truncates_to_char_cap_with_ellipsis() {
        let long = "x".repeat(MAX_ACTIVITY_LINE_CHARS + 50);
        let out = activity_line(&long);
        assert_eq!(out.chars().count(), MAX_ACTIVITY_LINE_CHARS);
        assert!(out.ends_with('…'));
        // A short line is unchanged.
        assert_eq!(activity_line("short"), "short");
    }

    #[test]
    fn progress_event_carries_counts_only() {
        let (em, log) = recording();
        em.progress(GitTransferProgress {
            received_objects: 10,
            total_objects: 100,
            indexed_objects: 5,
            received_bytes: 2048,
            total_deltas: None,
            indexed_deltas: None,
        });
        let events = log.lock().expect("lock");
        let e = &events[0];
        assert_eq!(e.kind, GitActivityKind::Progress);
        assert_eq!(e.progress.map(|p| p.total_objects), Some(100));
        assert!(e.line.is_none() && e.phase.is_none() && e.category.is_none());
    }

    /// Wire shape: camelCase, optionals ABSENT (not null) when unset, so a line
    /// event stays tiny. Mirrors the TS `GitActivityEvent`.
    #[test]
    fn wire_shape_omits_absent_optionals() {
        let (em, log) = recording();
        em.line(GitStream::Stdout, "hi");
        let events = log.lock().expect("lock");
        let v = serde_json::to_value(&events[0]).expect("json");
        let obj = v.as_object().expect("object");
        // Exactly the run-level fields + `line` — no null optionals on the wire.
        let mut keys: Vec<&str> = obj.keys().map(String::as_str).collect();
        keys.sort_unstable();
        assert_eq!(keys, vec!["elapsedMs", "id", "kind", "line", "seq"]);
        assert_eq!(obj.get("kind").and_then(|k| k.as_str()), Some("stdoutLine"));
        assert_eq!(obj.get("line").and_then(|k| k.as_str()), Some("hi"));
    }

    #[test]
    fn new_activity_id_is_unique_and_prefixed() {
        let a = new_activity_id();
        let b = new_activity_id();
        assert_ne!(a, b);
        assert!(a.starts_with("git-"), "unexpected id: {a}");
    }
}
