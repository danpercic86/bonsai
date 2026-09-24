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

use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::time::Instant;

pub use crate::git::activity_category::{GitActivityCategory, GitRunOutcome};
pub use target_type::{ActivityTarget, MAX_ACTIVITY_TARGET_CHARS};

use crate::git::activity_redact::redact_url_userinfo;
use crate::git::activity_target::RunSubject;

#[path = "activity_target_type.rs"]
mod target_type;

/// Per-event line-length cap, in CHARS (never split a char boundary). One hook
/// line can never forge extra log rows: [`activity_line`] strips C0/C1 + bidi
/// controls AND bounds the length. The TOTAL output is still capped by exec's
/// 64 MiB combined counter; the per-run LINE COUNT is bounded by
/// [`MAX_ACTIVITY_LINE_EVENTS`] here (backend) AND again on the frontend.
pub const MAX_ACTIVITY_LINE_CHARS: usize = 2000;

/// Per-activity cap on the COUNT of line EVENTS emitted onto the activity stream.
/// The exec seam's 64 MiB combined counter bounds captured BYTES; this bounds the
/// NUMBER of tiny line events that cross the Tauri IPC boundary. Without it a
/// hostile hook flooding stdout (`yes ''`, `while true; do echo; done`) sprays
/// tens of millions of line events + serialized `GitActivityEvent`s across IPC
/// (multi-GiB transient RSS, wedged UI) BEFORE the byte cap even trips (SECURITY).
/// ~10× the frontend's 500-row display cap for headroom, but enforced BEFORE IPC:
/// past it [`ActivityEmitter::line`] suppresses and [`ActivityEmitter::finished`]
/// emits ONE truncation marker. This caps only EMITTED events — the captured
/// `GitOutput` (and thus `HookRejected` / `HookOutputDialog`) stays FULL up to the
/// byte cap and byte-identical to the buffered path.
pub const MAX_ACTIVITY_LINE_EVENTS: usize = 5000;

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
    /// `Started` ONLY — the run's target ref (e.g. `origin/main`, `main`).
    /// A RAW git identifier, already sanitized + capped to
    /// [`MAX_ACTIVITY_TARGET_CHARS`] by [`ActivityTarget`]; absent when the run
    /// has no target (the UI then renders the bare category noun — FU-1 §3.3).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    /// `Started` ONLY; present iff the run acts on ≥2 items (then `target` is
    /// absent). A count, not copy — the frontend owns "Delete 3 branches". P119.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_count: Option<u32>,
    /// `Finished` ONLY, and only when `success == true`. P119 §2.6.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub outcome: Option<GitRunOutcome>,
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
    /// The run's target ref, set ONCE at construction and read only by
    /// [`Self::started`]. No setter and no `&mut self` anywhere, so a mid-run
    /// change is not representable (FU-1 §3.6-2); [`GitActivityRecorder`] gains
    /// no method, so core cannot touch it either.
    target: Option<String>,
    /// P119 §2.7: the item count of a multi-item run (≥2), same rules as
    /// `target` — set once here, read only by [`Self::started`].
    target_count: Option<u32>,
    start: Instant,
    seq: AtomicU64,
    /// Count of `line` calls this activity — the per-activity line-event cap
    /// ([`MAX_ACTIVITY_LINE_EVENTS`]) is enforced against this before emitting.
    line_events: AtomicUsize,
    emit: Box<dyn Fn(GitActivityEvent) + Send + Sync>,
}

impl ActivityEmitter {
    /// `target` is the run's ref, resolved BEFORE the run starts (see
    /// [`crate::git::activity_target::resolve_activity_target`]); `None` when the
    /// run has no target or the resolver declined.
    pub fn new(
        id: String,
        target: Option<ActivityTarget>,
        emit: Box<dyn Fn(GitActivityEvent) + Send + Sync>,
    ) -> Self {
        Self::with_subject(id, RunSubject::from(target), emit)
    }

    /// P119 §2.7: the full constructor — a run is about at most one target OR
    /// a count of ≥2 items ([`RunSubject`] guarantees never both).
    pub fn with_subject(
        id: String,
        subject: RunSubject,
        emit: Box<dyn Fn(GitActivityEvent) + Send + Sync>,
    ) -> Self {
        ActivityEmitter {
            id,
            target: subject.target.map(ActivityTarget::into_string),
            target_count: subject.count,
            start: Instant::now(),
            seq: AtomicU64::new(0),
            line_events: AtomicUsize::new(0),
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
            // Hard-coded: ONLY `started` ever carries a target (FU-1 §3.6-2).
            target: None,
            // Same rule: `targetCount` only on `started`, `outcome` only on
            // `finished` (P119 §2.6/§2.7).
            target_count: None,
            outcome: None,
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
        // The ONLY read of `self.target`/`self.target_count`, and the only
        // event that carries either.
        ev.target = self.target.clone();
        ev.target_count = self.target_count;
        (self.emit)(ev);
    }

    /// The terminal event. `code`/`success` mirror the op's outcome (best-effort;
    /// `None` code = killed / no single exit code — e.g. a hook rejection).
    /// Shorthand for [`Self::finish`] with no outcome and no reason line.
    pub fn finished(&self, code: Option<i32>, success: bool) {
        self.finish(code, success, None, None);
    }

    /// Terminal event, P119 form. Order: flush the truncation marker → if
    /// `reason` is `Some`, ONE `StderrLine` carrying `activity_line(reason)` →
    /// `Finished{code, success, outcome}`.
    ///
    /// The reason line is emitted directly, NOT through [`GitActivityRecorder::line`],
    /// so it is exempt from [`MAX_ACTIVITY_LINE_EVENTS`] and always delivered
    /// (P119 §2.8). `outcome` is dropped on a failed run: it describes how a
    /// SUCCESSFUL run ended (§2.6).
    pub fn finish(
        &self,
        code: Option<i32>,
        success: bool,
        outcome: Option<GitRunOutcome>,
        reason: Option<&str>,
    ) {
        self.flush_line_truncation();
        if let Some(reason) = reason {
            let mut ev = self.base(GitActivityKind::StderrLine);
            // A multi-line message would have its words glued together by the
            // control strip; turn line breaks into spaces first. SECURITY: some
            // messages quote the remote URL, which can carry `user:token@`, so
            // URL/scp userinfo is stripped before it reaches the wire.
            let one_line =
                redact_url_userinfo(&reason.replace("\r\n", " ").replace(['\n', '\r'], " "));
            ev.line = Some(activity_line(&one_line));
            (self.emit)(ev);
        }
        let mut ev = self.base(GitActivityKind::Finished);
        ev.code = code;
        ev.success = Some(success);
        ev.outcome = outcome.filter(|_| success);
        (self.emit)(ev);
    }

    /// If [`Self::line`] hit [`MAX_ACTIVITY_LINE_EVENTS`], emit exactly ONE final
    /// marker line naming how many further lines were suppressed. It is a normal
    /// (sanitized) line event — no new kind. Called once, from [`Self::finish`],
    /// so the suppressed total is exact by the time it fires.
    fn flush_line_truncation(&self) {
        let total = self.line_events.load(Ordering::Relaxed);
        let suppressed = total.saturating_sub(MAX_ACTIVITY_LINE_EVENTS);
        if suppressed == 0 {
            return;
        }
        let mut ev = self.base(GitActivityKind::StdoutLine);
        ev.line = Some(activity_line(&format!(
            "[bonsai] output truncated — {suppressed} more lines suppressed"
        )));
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
        // SECURITY: cap the COUNT of emitted line events per activity. The exec
        // seam's 64 MiB counter bounds captured BYTES, but a hostile hook that
        // floods stdout with tiny lines (`yes ''`) could otherwise push tens of
        // millions of line events across the IPC boundary. Past the cap we count
        // only (bounded, O(1)); `finished` flushes ONE marker with the suppressed
        // total. The CAPTURE path is untouched, so `GitOutput` stays full.
        let n = self.line_events.fetch_add(1, Ordering::Relaxed);
        if n >= MAX_ACTIVITY_LINE_EVENTS {
            return;
        }
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

/// Drop the characters that let one line of output pretend to be several, read
/// backwards, or hide content: C0/C1 controls (so `\n`, `\r`, `\t`, `\u{7f}`),
/// the bidi overrides/isolates, AND the zero-width chars (ZWSP/ZWNJ/ZWJ/BOM) that
/// can splice or obfuscate log lines. Mirrors the rule in `crate::ai::stream`
/// (duplicated per the P87 contract §2, to keep `git` decoupled from `ai`).
fn strip_control_chars(text: &str) -> String {
    text.chars()
        .filter(|c| {
            let bidi = matches!(c,
                '\u{200e}' | '\u{200f}' | '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}');
            let zero_width = matches!(c, '\u{200b}' | '\u{200c}' | '\u{200d}' | '\u{feff}');
            !c.is_control() && !bidi && !zero_width
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
#[path = "activity_tests.rs"]
mod tests;
