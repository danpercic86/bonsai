//! PURE interpretation of the `claude --output-format stream-json` NDJSON
//! protocol, plus the wire event the frontend sees (P68 §A/§F).
//!
//! Nothing here spawns, threads or reads: every function is total over a single
//! line of text, which is exactly what makes the mapping table (§3.2) unit
//! testable without a child process (D12). Lifecycle lives in [`super::session`].

use serde_json::Value;

/// The mid-run question protocol (P68 §B/D9). The CLI's own `SendMessage` tool
/// cannot ask the user anything in `-p` mode (verified dead end), so the system
/// prompt instructs Claude to emit ONE line starting with this token instead.
pub const SENTINEL: &str = "BONSAI_NEEDS_INPUT:";
/// Per-event text cap, in CHARS (never split a char boundary). A conflict body
/// can be hundreds of KB; the dock only ever shows lines (P68 §3.2).
pub const MAX_EVENT_TEXT: usize = 2000;
/// Cap for the `partialText` echo on a terminal event. Generous (it is meant to
/// look like a truncated file body) but still bounded — the complete record is
/// the dock log, not this field (D2/A5).
pub const MAX_PARTIAL_TEXT: usize = 20_000;
/// `⚙ Tool(arg)` lines are decoration, not content — kept short (A3).
const MAX_TOOL_TEXT: usize = 160;
/// Prefix of a tool-use log line. A CONST, not a literal at each site: the M6
/// exemption is keyed on [`StreamLogItem::notable`], and this glyph is what the
/// user recognises, so the two must never drift apart.
const TOOL_GLYPH: &str = "⚙ ";
/// Prefix of a permission-denial log line (security audit 2026-08-18): the model
/// asked for something `--permission-mode manual` refused, i.e. it tried to reach
/// outside the repository. Always shown, log switch or not (M6).
const DENIED_GLYPH: &str = "⛔ ";
/// `rate limit: …` re-serializations are diagnostics only (§3.2 table).
const MAX_RATE_LIMIT_TEXT: usize = 200;

/// One push event on the `ai_resolve_conflict_stream` channel (P68 §F). Compact
/// by design (D1) — no libgit2 objects, at most one line of text. Serialized
/// camelCase; mirrored in TS.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiRunEvent {
    /// Stable for the whole run. FIRST delivered on the `Started` event (D8).
    pub run_id: String,
    /// Monotonic from 0, one sequence per run. The frontend drops any event whose
    /// seq <= the last seen (stale/duplicate guard).
    pub seq: u64,
    pub kind: AiRunEventKind,
    /// One log line, the question text, or the terminal message. Never the whole
    /// payload; hard-truncated to [`MAX_EVENT_TEXT`] chars.
    pub text: Option<String>,
    /// `total_cost_usd` of the turn that just ended (`TurnEnd`) or of the run
    /// (`Done`). LAST value wins — never summed within a run (spike §1.8).
    pub cost_usd: Option<f64>,
    /// Since the run started (not since the turn). Always known (A6).
    pub elapsed_ms: u64,
    /// The file this event is about, when known (bulk attribution). `None` for
    /// run-level events.
    pub path: Option<String>,
    /// 1-based turn counter; 0 on `Started` (A6).
    pub turn: u32,
    /// Only on `Cancelled` / `Failed`: the assistant text accumulated so far
    /// (D2). Display-only — NEVER offered as a proposal (A5).
    ///
    /// LOSSY BY CONSTRUCTION, deliberately: every block is truncated to
    /// [`MAX_EVENT_TEXT`] chars before it is accumulated, the whole echo is
    /// capped at [`MAX_PARTIAL_TEXT`], and `--include-partial-messages` deltas
    /// are excluded (see `stream_event` in [`classify_line`]) so a streamed block
    /// is not counted twice. Do NOT "fix" that exclusion — the dock log, not this
    /// field, is the complete record.
    pub partial_text: Option<String>,
    /// P68d: the CLI's own CUMULATIVE `estimated_tokens` from the latest
    /// `system`/`thinking_tokens` heartbeat, when one carried it.
    ///
    /// Why this field exists: `cost_usd` only arrives at a turn boundary
    /// (`TurnEnd`/`Done`), so a long single-turn run shows `$—` for minutes — and
    /// the user accepted "no spend cap" *because* spend would be visible. This is
    /// the agreed live proxy. **Verified against `claude` v2.1.233** (2026-08-17):
    /// the heartbeat is `{"type":"system","subtype":"thinking_tokens",
    /// "estimated_tokens":350,"estimated_tokens_delta":150,…}`, cumulative and
    /// monotonic, roughly ~1 line per few seconds while the model thinks.
    ///
    /// SCOPE, precisely — it is THINKING tokens only, and estimated: the observed
    /// run ended at `estimated_tokens: 600` against a real
    /// `usage.output_tokens_details.thinking_tokens: 679`. A run that never enters
    /// extended thinking emits NO heartbeats and this stays `None` throughout. It is
    /// deliberately NOT converted to money anywhere — no price table is hard-coded.
    pub thinking_tokens: Option<u64>,
    /// INTERNAL, never on the wire (`serde(skip)`): this `Log` line is one whose
    /// visibility is load-bearing for the read grant — a `⚙ tool(arg)` line or a
    /// `⛔` permission denial — so `RunEvents::forward` must NOT suppress it when
    /// `ai_stream_log` is false (security audit 2026-08-18, M6).
    ///
    /// A FLAG, not a text-prefix test: assistant prose is attacker-controllable, so
    /// a prose block beginning with `⚙ ` could otherwise forge an unsuppressable
    /// "tool line" and fake a read that never happened. Only classification sets it.
    #[serde(skip)]
    pub notable: bool,
}

impl AiRunEvent {
    /// The run-level fields every event carries; optional payload is filled in by
    /// the caller so each emit site stays one short statement.
    pub fn new(run_id: &str, seq: u64, kind: AiRunEventKind, elapsed_ms: u64, turn: u32) -> Self {
        AiRunEvent {
            run_id: run_id.to_string(),
            seq,
            kind,
            text: None,
            cost_usd: None,
            elapsed_ms,
            path: None,
            turn,
            partial_text: None,
            thinking_tokens: None,
            notable: false,
        }
    }
}

/// Exactly seven kinds — locked by the approved plan. New NDJSON line types do
/// NOT add kinds; they map onto `Log` (D12/A3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum AiRunEventKind {
    /// Always first, seq 0, emitted BEFORE the child is spawned so the UI has the
    /// runId even if the spawn fails (D8).
    Started,
    /// One human-readable line for the dock. High frequency -> batched (D5).
    Log,
    /// A `result` line arrived and parsed; the run may continue (another turn).
    TurnEnd,
    /// The sentinel was seen; the session is blocked on `ai_reply_run`. The
    /// watchdog is paused (D3).
    AwaitingInput,
    /// Terminal: success.
    Done,
    /// Terminal: `text` is the same message as the returned `AiFailed`.
    Failed,
    /// Terminal: user cancel. The command resolves `Err(AiCancelled)`.
    Cancelled,
}

/// One log-ish item produced by classification.
#[derive(Debug, Clone, PartialEq)]
pub struct StreamLogItem {
    pub text: String,
    /// True only for `assistant`/`text` content blocks. The session accumulates
    /// ONLY these into `partialText`, so a cancelled run's partial is a plausible
    /// truncated file body rather than `⚙`/`system`/`stderr` decoration (D2).
    pub assistant_text: bool,
    /// Survives `ai_stream_log: false` — see [`AiRunEvent::notable`] (M6).
    pub notable: bool,
}

impl StreamLogItem {
    /// Decoration (system/rate-limit/unknown lines): not part of `partial`.
    fn log(text: &str) -> Self {
        StreamLogItem {
            text: truncate_text(text, MAX_EVENT_TEXT),
            assistant_text: false,
            notable: false,
        }
    }
    /// Real assistant prose — the only thing that feeds `partialText` (D2).
    fn assistant(text: &str) -> Self {
        StreamLogItem {
            text: truncate_text(text, MAX_EVENT_TEXT),
            assistant_text: true,
            notable: false,
        }
    }
    /// A line the user must see even with the log switched off (M6): what the model
    /// READ (`⚙`) and what the fence REFUSED (`⛔`). Decoration for `partial`
    /// purposes, exactly like [`Self::log`] — it is not the model's prose.
    fn notable(text: &str) -> Self {
        StreamLogItem {
            text: truncate_text(text, MAX_TOOL_TEXT),
            assistant_text: false,
            notable: true,
        }
    }
}

/// What one NDJSON line means to the session loop (§3.2).
#[derive(Debug, Clone, PartialEq)]
pub enum LineOutcome {
    /// Emit these as `Log` events, in order. May legitimately be empty (an
    /// `assistant` line with no content blocks).
    Log(Vec<StreamLogItem>),
    /// This is a `result` line: the session re-parses the RAW line through
    /// [`super::parse_result_envelope`] (spike §1.3) and does turn accounting.
    Result,
    /// A heartbeat: it RESETS the idle watchdog but produces NO log line (A4 — one
    /// per second would drown the dock).
    ///
    /// `Some(n)` = the line carried a cumulative `estimated_tokens` count (P68d);
    /// the session forwards it as a *metrics-only* event (no `text`) so the UI has a
    /// live number while `cost_usd` is still unknown. `None` = pure liveness (a
    /// blank line, or a `stream_event` with no text delta).
    Heartbeat(Option<u64>),
}

/// Classify one NDJSON line (PURE, D12). NEVER errors — an unknown `type`, an
/// unknown `subtype` and non-JSON input all degrade to `Log` with the raw line
/// truncated, because the CLI's protocol is undocumented and may grow.
pub fn classify_line(raw: &str) -> LineOutcome {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return LineOutcome::Heartbeat(None);
    }
    let Ok(v) = serde_json::from_str::<Value>(trimmed) else {
        return log_one(trimmed);
    };
    match v.get("type").and_then(Value::as_str) {
        Some("system") => classify_system(&v),
        Some("rate_limit_event") => {
            log_one(&format!("rate limit: {}", truncate_text(&compact(&v, trimmed), MAX_RATE_LIMIT_TEXT)))
        }
        // A11: `--replay-user-messages` would otherwise dump the whole payload
        // (up to a few hundred KB) into the dock log. Size only, never content.
        Some("user") => log_one(&format!("» sent {} bytes to Claude", user_payload_bytes(&v, trimmed))),
        Some("assistant") => classify_assistant(&v),
        Some("result") => LineOutcome::Result,
        // `--include-partial-messages` (setting-gated, default off): the delta
        // shape is UNVERIFIED (spike §1.8), so probe the known paths and fall
        // back to a silent heartbeat rather than logging framing noise.
        //
        // `StreamLogItem::log` (NOT `::assistant`) on purpose: the final
        // `assistant` line repeats the same prose in full, so counting deltas
        // into `partialText` would double it. That makes `partialText` a LOSSY
        // echo by design — the dock log is the complete record (D2/A5).
        Some("stream_event") => match find_text_delta(&v) {
            Some(t) => LineOutcome::Log(vec![StreamLogItem::log(t)]),
            None => LineOutcome::Heartbeat(None),
        },
        _ => log_one(trimmed),
    }
}

fn classify_system(v: &Value) -> LineOutcome {
    match v.get("subtype").and_then(Value::as_str) {
        Some("init") => {
            let session = v.get("session_id").and_then(Value::as_str).unwrap_or("?");
            let model = v.get("model").and_then(Value::as_str).unwrap_or("?");
            let tools: Vec<&str> = v
                .get("tools")
                .and_then(Value::as_array)
                .map(|a| a.iter().filter_map(Value::as_str).collect())
                .unwrap_or_default();
            let tools = if tools.is_empty() { "none".to_string() } else { tools.join(", ") };
            log_one(&format!("session {session} · model {model} · tools: {tools}"))
        }
        // A4: resets the watchdog (the session does that for every stdout line)
        // and emits NO log line — one heartbeat per second would drown the dock.
        //
        // P68d: it does carry the run's only LIVE spend signal, though. Verified
        // shape (`claude` v2.1.233): `estimated_tokens` is cumulative thinking
        // tokens, `estimated_tokens_delta` the step. Forward the cumulative value;
        // a missing/negative/non-integer field degrades to `None`, never an error
        // (D12), and the session then treats the line as pure liveness.
        Some("thinking_tokens") => LineOutcome::Heartbeat(
            v.get("estimated_tokens").and_then(Value::as_u64),
        ),
        // D9: a corroborating HINT only. It never drives `AwaitingInput`.
        Some("post_turn_summary") => {
            let status = v.get("status_category").and_then(Value::as_str).unwrap_or("?");
            let needs = v.get("needs_action").and_then(Value::as_bool).unwrap_or(false);
            log_one(&format!("summary: status={status} needsAction={needs}"))
        }
        Some(other) => log_one(&format!("system/{other}")),
        None => log_one("system/?"),
    }
}

fn classify_assistant(v: &Value) -> LineOutcome {
    let mut items = Vec::new();
    if let Some(content) = v.pointer("/message/content").and_then(Value::as_array) {
        for item in content {
            match item.get("type").and_then(Value::as_str) {
                Some("text") => {
                    items.push(StreamLogItem::assistant(
                        item.get("text").and_then(Value::as_str).unwrap_or(""),
                    ));
                }
                // A3: no new event kind — read-only tool use shows up as a log
                // line, which is what makes D10 visible in the dock.
                //
                // M6: `notable`, so `ai_stream_log: false` cannot silence it — this
                // line is the user's ONLY signal that the model read something, and
                // that visibility is what makes the read grant acceptable.
                Some("tool_use") => {
                    let name = item.get("name").and_then(Value::as_str).unwrap_or("tool");
                    let arg = first_string_field(item.get("input")).unwrap_or_default();
                    items.push(StreamLogItem::notable(&format!("{TOOL_GLYPH}{name}({arg})")));
                }
                Some(other) => items.push(StreamLogItem::log(&format!("assistant/{other}"))),
                None => items.push(StreamLogItem::log("assistant/?")),
            }
        }
    }
    LineOutcome::Log(items)
}

fn log_one(text: &str) -> LineOutcome {
    LineOutcome::Log(vec![StreamLogItem::log(text)])
}

/// Compact re-serialization for diagnostics; falls back to the raw line if the
/// value somehow refuses to serialize.
fn compact(v: &Value, raw: &str) -> String {
    serde_json::to_string(v).unwrap_or_else(|_| raw.to_string())
}

/// Byte SIZE of a replayed user message (A11): the sum of its text blocks, else
/// the serialized `message`, else the whole line. Never the content itself.
fn user_payload_bytes(v: &Value, raw: &str) -> usize {
    if let Some(items) = v.pointer("/message/content").and_then(Value::as_array) {
        let sum: usize = items
            .iter()
            .filter_map(|i| i.get("text").and_then(Value::as_str))
            .map(str::len)
            .sum();
        if sum > 0 {
            return sum;
        }
    }
    match v.get("message") {
        Some(m) => serde_json::to_string(m).map(|s| s.len()).unwrap_or_else(|_| raw.len()),
        None => raw.len(),
    }
}

/// The denials recorded on ONE `result` line, as ready-to-log `⛔` items (PURE).
///
/// Why this exists: `--permission-mode manual` fences `Read`/`Grep`/`Glob` to
/// `cwd`, and a refusal is reported ONLY here — `{"permission_denials":[{"tool_name":
/// "Read","tool_input":{"file_path":"…"}}]}` on the `result` envelope (verified,
/// CLI v2.1.234). An empty array is the normal case and yields nothing. A denial
/// means the model tried to reach OUTSIDE the repository, which the user must be
/// told about whatever `ai_stream_log` says (M6) — hence `notable`.
///
/// Total, like everything else here (D12): a missing/mis-shaped field degrades to
/// fewer lines, never an error. Bounded at [`MAX_DENIAL_LINES`] so a model in a
/// denial loop cannot flood the dock; the count is stated when it truncates.
pub fn permission_denial_lines(raw: &str) -> Vec<StreamLogItem> {
    let Ok(v) = serde_json::from_str::<Value>(raw.trim()) else {
        return Vec::new();
    };
    let Some(denials) = v.get("permission_denials").and_then(Value::as_array) else {
        return Vec::new();
    };
    let mut items: Vec<StreamLogItem> = denials
        .iter()
        .take(MAX_DENIAL_LINES)
        .map(|d| {
            let name = d.get("tool_name").and_then(Value::as_str).unwrap_or("tool");
            let arg = first_string_field(d.get("tool_input")).unwrap_or_default();
            // The path is model-supplied text: keep it on one line so it cannot
            // fake extra dock rows (`truncate_text` bounds length, not newlines).
            let arg = strip_control_chars(&arg);
            StreamLogItem::notable(&format!(
                "{DENIED_GLYPH}denied {name}({arg}) — outside this repository"
            ))
        })
        .collect();
    if denials.len() > MAX_DENIAL_LINES {
        items.push(StreamLogItem::notable(&format!(
            "{DENIED_GLYPH}{} denials in total (list truncated)",
            denials.len()
        )));
    }
    items
}

/// At most this many `⛔` lines per `result` line, plus one summary line.
const MAX_DENIAL_LINES: usize = 20;

/// Drop the characters that let one line of model text pretend to be several, or
/// to read backwards: C0/C1 controls (so `\n`, `\r`, `\t`, `\u{7f}`) plus the
/// bidi overrides and isolates. Used on every piece of model-authored text that
/// Bonsai renders as a single line (the question, a denied path).
pub(crate) fn strip_control_chars(text: &str) -> String {
    text.chars()
        .filter(|c| {
            let bidi = matches!(c,
                '\u{200e}' | '\u{200f}' | '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}');
            !c.is_control() && !bidi
        })
        .collect()
}

/// The first string-valued field of a `tool_use` input object (serde_json orders
/// object keys, so this is deterministic).
fn first_string_field(input: Option<&Value>) -> Option<String> {
    input?.as_object()?.values().find_map(Value::as_str).map(str::to_string)
}

/// Probe the KNOWN partial-message shapes for a text delta. Unverified protocol
/// (spike §1.8): a miss is a heartbeat, never an error.
fn find_text_delta(v: &Value) -> Option<&str> {
    for path in ["/delta/text", "/event/delta/text", "/content_block/text"] {
        if let Some(t) = v.pointer(path).and_then(Value::as_str) {
            return Some(t);
        }
    }
    None
}

/// The mid-run question, if this turn's (already fence-stripped) result IS one.
///
/// Recognised ONLY when the sentinel line is the SOLE non-empty line of the result
/// (A9 as amended by the security audit, M3). A9 originally argued that a merged
/// body starting with the token "is not a thing" — true of accidents, false of
/// adversaries: a conflicted file whose BOTH sides begin with that literal line
/// reproduces it through a *faithful* merge, no jailbreak needed, and the result
/// was then shown to the user as a question with a focused reply box. Requiring
/// the line to stand alone removes that: a real question is one line and nothing
/// else, while a file body always carries more content after it, so an injected
/// first line now degrades to a normal (reviewable) proposal.
///
/// The returned text is still fully model-authored. It is control-stripped here so
/// it cannot forge extra UI lines, and the caller must attribute it as model output
/// (see `AiActivityAsk`) — stripping is not trust.
pub fn sentinel_question(text: &str) -> Option<String> {
    let mut non_empty = text.lines().filter(|l| !l.trim().is_empty());
    let rest = non_empty.next()?.trim_start().strip_prefix(SENTINEL)?;
    if non_empty.next().is_some() {
        return None;
    }
    Some(strip_control_chars(rest.trim()))
}

/// Truncate to `cap` CHARS (never bytes — a split char boundary would panic on
/// `String` reassembly and corrupt UTF-8 on the wire). An over-long string ends
/// in `…` and is exactly `cap` chars long — including `cap == 0`, which yields
/// the empty string (the `…` would otherwise be a 1-char overflow).
pub(crate) fn truncate_text(text: &str, cap: usize) -> String {
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
#[path = "stream_tests.rs"]
mod tests;
