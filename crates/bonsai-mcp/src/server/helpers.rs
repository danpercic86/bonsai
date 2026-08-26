//! Result/serialization helpers shared by the tool handlers: `ok_json`,
//! `ok_null`, `err_result`, the compact-summary formatter, and the lock-poison
//! mapper. Split out of `server.rs`; re-exported (`use helpers::*`) from the
//! module root so existing paths resolve unchanged. Behavior unchanged.

use super::*;

/// Map a poisoned lock to a domain error rather than panicking.
pub(crate) fn pois<T>(_: std::sync::PoisonError<T>) -> AppError {
    AppError::Other("state lock poisoned".into())
}
/// Success result: the full payload lives in `structured_content` (serde of the
/// core type); the `content` text block is a COMPACT one-line summary (shape +
/// count, never the payload).
///
/// `CallToolResult::structured` would echo the *entire* JSON a SECOND time as a
/// text `ContentBlock` (`value.to_string()`), so a multi-MB `bonsai_get_graph`
/// response would be transmitted twice (F-A8-a). We build the result directly:
/// the MCP client still receives the complete structured payload, plus a tiny
/// human-readable descriptor instead of the duplicated blob.
pub(crate) fn ok_json<T: serde::Serialize>(v: &T) -> CallToolResult {
    match serde_json::to_value(v) {
        Ok(value) => {
            // `structured` sets `is_error=false` + `structured_content`, but ALSO
            // echoes the full JSON as a text block; overwrite that text with the
            // compact summary so the payload is transmitted exactly once.
            let summary = compact_summary(&value);
            let mut result = CallToolResult::structured(value);
            result.content = vec![ContentBlock::text(summary)];
            result
        }
        Err(e) => err_result(AppError::Other(format!("serialization error: {e}"))),
    }
}

/// A tiny, payload-free descriptor of a JSON value for the `content` text block:
/// arrays report their length, objects report their top-level keys (capped),
/// and scalars are rendered char-safe-truncated. Never echoes large strings or
/// nested structures — the full data is in `structured_content`.
pub(crate) fn compact_summary(value: &serde_json::Value) -> String {
    use serde_json::Value;
    match value {
        Value::Array(a) => format!("[{} items]", a.len()),
        Value::Object(m) => {
            const MAX_KEYS: usize = 12;
            let shown: Vec<&str> = m.keys().take(MAX_KEYS).map(String::as_str).collect();
            let mut s = format!("{{{}", shown.join(", "));
            if m.len() > shown.len() {
                s.push_str(", …");
            }
            s.push('}');
            s
        }
        Value::Null => "null".to_string(),
        other => {
            let s = other.to_string();
            match s.char_indices().nth(80) {
                Some((idx, _)) => format!("{}…", &s[..idx]),
                None => s,
            }
        }
    }
}

/// Success result for a mutation that returns no data (`() -> null`).
pub(crate) fn ok_null() -> CallToolResult {
    CallToolResult::structured(serde_json::Value::Null)
}

/// Domain-error result: preserves `AppError`'s `{ kind, message }` in structured
/// content (via its custom `Serialize`) plus a human `"<kind>: <message>"` text.
/// `is_error = true`.
pub(crate) fn err_result(e: AppError) -> CallToolResult {
    let value = serde_json::to_value(&e).unwrap_or_else(|_| {
        serde_json::json!({ "kind": "other", "message": "unserializable error" })
    });
    let kind = value.get("kind").and_then(|v| v.as_str()).unwrap_or("other");
    let message = value.get("message").and_then(|v| v.as_str()).unwrap_or("");
    let text = format!("{kind}: {message}");
    let mut result = CallToolResult::structured_error(value);
    result.content = vec![ContentBlock::text(text)];
    result
}
