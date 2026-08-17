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
}

impl StreamLogItem {
    /// Decoration (system/tool/rate-limit/unknown lines): not part of `partial`.
    fn log(text: &str) -> Self {
        StreamLogItem { text: truncate_text(text, MAX_EVENT_TEXT), assistant_text: false }
    }
    /// Real assistant prose — the only thing that feeds `partialText` (D2).
    fn assistant(text: &str) -> Self {
        StreamLogItem { text: truncate_text(text, MAX_EVENT_TEXT), assistant_text: true }
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
    /// A heartbeat: it RESETS the idle watchdog but produces no event (A4).
    Heartbeat,
}

/// Classify one NDJSON line (PURE, D12). NEVER errors — an unknown `type`, an
/// unknown `subtype` and non-JSON input all degrade to `Log` with the raw line
/// truncated, because the CLI's protocol is undocumented and may grow.
pub fn classify_line(raw: &str) -> LineOutcome {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return LineOutcome::Heartbeat;
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
            None => LineOutcome::Heartbeat,
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
        // but emits nothing — one heartbeat per second would drown the dock.
        Some("thinking_tokens") => LineOutcome::Heartbeat,
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
                Some("tool_use") => {
                    let name = item.get("name").and_then(Value::as_str).unwrap_or("tool");
                    let arg = first_string_field(item.get("input")).unwrap_or_default();
                    items.push(StreamLogItem {
                        text: truncate_text(&format!("⚙ {name}({arg})"), MAX_TOOL_TEXT),
                        assistant_text: false,
                    });
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
/// Recognised ONLY when the FIRST non-empty line starts with [`SENTINEL`] (A9):
/// a merged file body whose first line is the token is not a thing, while a body
/// that merely mentions it mid-text is not a question.
pub fn sentinel_question(text: &str) -> Option<String> {
    let first = text.lines().find(|l| !l.trim().is_empty())?;
    let rest = first.trim_start().strip_prefix(SENTINEL)?;
    Some(rest.trim().to_string())
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
mod tests {
    use super::*;

    /// The single-item text of a `Log` outcome (panics the test on any other
    /// shape — these are known-answer cases).
    fn log_text(raw: &str) -> String {
        match classify_line(raw) {
            LineOutcome::Log(items) => {
                assert_eq!(items.len(), 1, "expected exactly one item for {raw}");
                items[0].text.clone()
            }
            other => panic!("expected Log for {raw}, got {other:?}"),
        }
    }

    #[test]
    fn init_line_logs_session_model_and_tools() {
        let raw = r#"{"type":"system","subtype":"init","session_id":"s1","model":"sonnet","tools":["Read","Grep","Glob"]}"#;
        assert_eq!(log_text(raw), "session s1 · model sonnet · tools: Read, Grep, Glob");
    }

    #[test]
    fn init_line_with_empty_tools_says_none() {
        let raw = r#"{"type":"system","subtype":"init","session_id":"s1","model":"sonnet","tools":[]}"#;
        assert_eq!(log_text(raw), "session s1 · model sonnet · tools: none");
        // Missing fields degrade, never panic.
        let bare = r#"{"type":"system","subtype":"init"}"#;
        assert_eq!(log_text(bare), "session ? · model ? · tools: none");
    }

    #[test]
    fn thinking_tokens_is_a_heartbeat() {
        let raw = r#"{"type":"system","subtype":"thinking_tokens","tokens":12}"#;
        assert_eq!(classify_line(raw), LineOutcome::Heartbeat);
    }

    #[test]
    fn post_turn_summary_is_a_hint_log_only() {
        let raw = r#"{"type":"system","subtype":"post_turn_summary","status_category":"blocked","needs_action":true}"#;
        assert_eq!(log_text(raw), "summary: status=blocked needsAction=true");
        let bare = r#"{"type":"system","subtype":"post_turn_summary"}"#;
        assert_eq!(log_text(bare), "summary: status=? needsAction=false");
    }

    #[test]
    fn unknown_system_subtype_degrades_to_log() {
        assert_eq!(log_text(r#"{"type":"system","subtype":"weird"}"#), "system/weird");
        assert_eq!(log_text(r#"{"type":"system"}"#), "system/?");
    }

    #[test]
    fn rate_limit_event_is_compacted_and_capped() {
        let raw = r#"{"type":"rate_limit_event","status":"ok"}"#;
        let text = log_text(raw);
        assert!(text.starts_with("rate limit: {"), "got {text}");
        let long = format!(r#"{{"type":"rate_limit_event","note":"{}"}}"#, "x".repeat(500));
        let capped = log_text(&long);
        // "rate limit: " + <=200 chars of JSON.
        assert_eq!(capped.chars().count(), "rate limit: ".chars().count() + MAX_RATE_LIMIT_TEXT);
    }

    #[test]
    fn replayed_user_message_logs_only_a_byte_count() {
        let secret = "TOP SECRET PAYLOAD";
        let raw = format!(
            r#"{{"type":"user","isReplay":true,"message":{{"role":"user","content":[{{"type":"text","text":"{secret}"}}]}}}}"#
        );
        let text = log_text(&raw);
        assert_eq!(text, format!("» sent {} bytes to Claude", secret.len()));
        assert!(!text.contains("SECRET"), "content must never be logged (A11)");
    }

    #[test]
    fn assistant_text_blocks_become_assistant_items() {
        let raw = r#"{"type":"assistant","message":{"content":[{"type":"text","text":"line A"},{"type":"text","text":"line B"}]}}"#;
        match classify_line(raw) {
            LineOutcome::Log(items) => {
                assert_eq!(items.len(), 2);
                assert_eq!(items[0].text, "line A");
                assert!(items[0].assistant_text, "text blocks feed partialText (D2)");
                assert_eq!(items[1].text, "line B");
            }
            other => panic!("expected Log, got {other:?}"),
        }
    }

    #[test]
    fn tool_use_becomes_a_decorated_log_not_a_new_kind() {
        let raw = r#"{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Read","input":{"file_path":"src/a.rs"}}]}}"#;
        match classify_line(raw) {
            LineOutcome::Log(items) => {
                assert_eq!(items[0].text, "⚙ Read(src/a.rs)");
                assert!(!items[0].assistant_text, "decoration must not feed partialText");
            }
            other => panic!("expected Log, got {other:?}"),
        }
        // No input / unnamed tool still classifies.
        assert_eq!(
            log_text(r#"{"type":"assistant","message":{"content":[{"type":"tool_use"}]}}"#),
            "⚙ tool()"
        );
    }

    #[test]
    fn unknown_assistant_content_item_degrades_to_log() {
        assert_eq!(
            log_text(r#"{"type":"assistant","message":{"content":[{"type":"thinking"}]}}"#),
            "assistant/thinking"
        );
        assert_eq!(
            log_text(r#"{"type":"assistant","message":{"content":[{}]}}"#),
            "assistant/?"
        );
        // No content array at all: an empty Log, never an error.
        assert_eq!(
            classify_line(r#"{"type":"assistant"}"#),
            LineOutcome::Log(Vec::new())
        );
    }

    #[test]
    fn result_line_is_the_result_outcome() {
        let raw = r#"{"type":"result","subtype":"success","is_error":false,"result":"body"}"#;
        assert_eq!(classify_line(raw), LineOutcome::Result);
    }

    #[test]
    fn stream_event_logs_a_delta_or_heartbeats() {
        let raw = r#"{"type":"stream_event","delta":{"text":"par"}}"#;
        assert_eq!(log_text(raw), "par");
        let nested = r#"{"type":"stream_event","event":{"delta":{"text":"tial"}}}"#;
        assert_eq!(log_text(nested), "tial");
        let framing = r#"{"type":"stream_event","event":{"type":"message_start"}}"#;
        assert_eq!(classify_line(framing), LineOutcome::Heartbeat);
    }

    #[test]
    fn unknown_type_and_non_json_degrade_to_log_never_err() {
        assert_eq!(log_text(r#"{"type":"brand_new_thing","x":1}"#), r#"{"type":"brand_new_thing","x":1}"#);
        assert_eq!(log_text("not json at all"), "not json at all");
        assert_eq!(log_text("{ truncated json"), "{ truncated json");
        // A typeless object is still just a log line.
        assert_eq!(log_text("{}"), "{}");
    }

    #[test]
    fn blank_line_is_a_heartbeat() {
        assert_eq!(classify_line(""), LineOutcome::Heartbeat);
        assert_eq!(classify_line("   \t"), LineOutcome::Heartbeat);
    }

    #[test]
    fn long_assistant_text_is_capped_at_max_event_text() {
        let body = "y".repeat(5000);
        let raw = format!(
            r#"{{"type":"assistant","message":{{"content":[{{"type":"text","text":"{body}"}}]}}}}"#
        );
        let text = log_text(&raw);
        assert_eq!(text.chars().count(), MAX_EVENT_TEXT);
        assert!(text.ends_with('…'));
    }

    #[test]
    fn sentinel_matches_only_the_first_non_empty_line() {
        assert_eq!(
            sentinel_question("BONSAI_NEEDS_INPUT: which locale wins?"),
            Some("which locale wins?".to_string())
        );
        // Leading blank lines are skipped.
        assert_eq!(
            sentinel_question("\n\n  BONSAI_NEEDS_INPUT: which?  \n"),
            Some("which?".to_string())
        );
        // A9: the token mid-body is NOT a question.
        assert_eq!(
            sentinel_question("{\n  \"a\": \"BONSAI_NEEDS_INPUT: nope\"\n}"),
            None
        );
        assert_eq!(sentinel_question("merged body"), None);
        assert_eq!(sentinel_question(""), None);
        // A bare sentinel with no question text still blocks the run.
        assert_eq!(sentinel_question("BONSAI_NEEDS_INPUT:"), Some(String::new()));
    }

    #[test]
    fn truncate_text_never_splits_a_char() {
        // Multi-byte chars: char-based cap, byte-based would panic/corrupt.
        let s = "é".repeat(10);
        assert_eq!(truncate_text(&s, 10), s);
        let cut = truncate_text(&s, 5);
        assert_eq!(cut.chars().count(), 5);
        assert_eq!(cut, "éééé…");
        // `cap` is a hard char budget, so cap 0 has room for nothing at all —
        // not even the ellipsis.
        assert_eq!(truncate_text("abc", 0), "");
        assert_eq!(truncate_text("", 0), "");
        assert_eq!(truncate_text("abc", 1), "…");
    }

    #[test]
    fn ai_run_event_wire_shape_is_camel_case() {
        let mut ev = AiRunEvent::new("ai-1", 3, AiRunEventKind::AwaitingInput, 1234, 2);
        ev.text = Some("q?".to_string());
        ev.cost_usd = Some(0.02);
        ev.partial_text = Some("half".to_string());
        let json = serde_json::to_string(&ev).expect("event serializes");
        for key in ["\"runId\"", "\"costUsd\"", "\"elapsedMs\"", "\"partialText\"", "\"awaitingInput\""] {
            assert!(json.contains(key), "missing {key} in {json}");
        }
        assert!(!json.contains("run_id"), "snake_case leaked: {json}");
    }
}
