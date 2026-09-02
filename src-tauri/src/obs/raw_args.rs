//! P91 Amendment A26 — writer-side enforcement of the raw-mode `args` invariant
//! (`docs/contracts/P91-raw-args-privacy.md` §C).
//!
//! **The producer proposes; the writer enforces.** This module deliberately does
//! NOT read `src/obs/rawArgPolicy.json`: a table shared with the producer would
//! be worthless against the failure that actually matters — a wrong row, or
//! producer code that ignores the table (which is exactly the bug this amendment
//! fixes). It instead enforces a *shape + vocabulary* invariant it can decide on
//! its own, which catches both a bad row and a bad producer:
//!
//! * `args` may only ride on an `ipc.call` record;
//! * every key is a lower-camel parameter **name** (`^[a-z][A-Za-z0-9]*$`) — a
//!   positional key (`"0"`, `"1"`, …) fails, and that single rule kills the
//!   original leak at the writer even if the frontend is never fixed;
//! * no key is in the free-text or credential vocabulary;
//! * every value is a short single-line scalar.
//!
//! Any violation drops the **whole** `args` object — a producer that mislabelled
//! one argument is untrusted about the rest — and stamps a writer-set
//! `argsPolicyViolation: true`. That flag cannot be forged: [`LogPayload`] has no
//! such field, so a producer-supplied one is dropped as an unknown field at
//! deserialisation, and this module is the only writer of it.
//!
//! **`OBS_SCHEMA_VERSION` stays 1** under the §13 row 23 pre-release carve-out:
//! every field this amendment adds (`argsOmitted`, `argsPolicyViolation`) is
//! optional and additive, P91 is branch-only, and no v1 corpus exists on disk.
//!
//! [`LogPayload`]: super::record::LogPayload

use serde_json::Value;

use super::scrub::is_sensitive_key;

/// Longest raw `args` string kept on disk. Mirrors `RAW_ARG_MAX_STR` in
/// `src/obs/rawArgPolicy.ts`.
pub const RAW_ARG_MAX_STR: usize = 512;

/// Writer-set marker: this record carried an `args` object that failed §C.
const VIOLATION_KEY: &str = "argsPolicyViolation";
const ARGS_KEY: &str = "args";
const OMITTED_KEY: &str = "argsOmitted";

/// §B.3 free-text vocabulary — applied to raw `args` KEYS only.
///
/// It is deliberately NOT folded into [`is_sensitive_key`]: `ErrorPayload.message`
/// is a legitimate, already-scrubbed field, and collapsing it there would blind
/// every error record.
const FREE_TEXT: &[&str] = &[
    "message", "msg", "query", "search", "text", "body", "prompt", "descri", "note", "content",
    "comment", "title", "subject", "summary", "patch", "diff", "blurb", "input", "reason",
];

/// Credential names beyond [`is_sensitive_key`]'s `token|secret|password|passphrase|auth`.
const SENSITIVE_EXTRA: &[&str] = &["credential", "apikey", "api_key", "privatekey", "sshkey"];

/// Enforces the raw-mode `args` invariant on a serialized record, IN PLACE.
///
/// Runs in BOTH redaction modes, on every record, BEFORE the credential scrubber
/// so a violating object is gone before anything can partially "rescue" it.
/// Returns true iff it dropped an `args` object, in which case it also set
/// `argsPolicyViolation: true`.
pub fn enforce(v: &mut Value) -> bool {
    let Value::Object(map) = v else {
        return false;
    };
    // Writer-set only. Stripped first so a forged flag can never survive even if
    // some future payload struct starts accepting unknown fields.
    map.remove(VIOLATION_KEY);
    // W6 — `argsOmitted` must be a non-negative integer, else it is not ours.
    let omitted_ok = map
        .get(OMITTED_KEY)
        .is_none_or(|o| o.as_u64().is_some_and(|n| n <= u32::MAX as u64));
    if !omitted_ok {
        map.remove(OMITTED_KEY);
    }
    if !map.contains_key(ARGS_KEY) {
        return false;
    }
    // W1 — `args` rides only on `ipc.call`.
    let is_ipc_call = map.get("kind").and_then(Value::as_str) == Some("ipc.call");
    let conforms = is_ipc_call
        && match map.get(ARGS_KEY) {
            // W2 — must be a JSON object.
            Some(Value::Object(args)) => args
                .iter()
                // W3 / W4 / W5.
                .all(|(k, val)| {
                    is_valid_param_key(k) && !is_denied_param(k) && is_allowed_scalar(val)
                }),
            _ => false,
        };
    if conforms {
        return false;
    }
    map.remove(ARGS_KEY);
    map.insert(VIOLATION_KEY.to_string(), Value::Bool(true));
    true
}

/// W3 — a parameter NAME, never a positional index. `"0"` fails here.
fn is_valid_param_key(k: &str) -> bool {
    let mut chars = k.chars();
    match chars.next() {
        Some(c) if c.is_ascii_lowercase() => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_alphanumeric())
}

/// §B.3 free-text vocabulary. Not used by `scrub.rs` — see [`FREE_TEXT`].
fn is_free_text_param(k: &str) -> bool {
    let lower = k.to_ascii_lowercase();
    FREE_TEXT.iter().any(|w| lower.contains(w))
}

/// W4 — free text or a credential by name.
fn is_denied_param(k: &str) -> bool {
    if is_free_text_param(k) || is_sensitive_key(k) {
        return true;
    }
    let lower = k.to_ascii_lowercase();
    SENSITIVE_EXTRA.iter().any(|w| lower.contains(w)) || contains_word(&lower, "pat")
}

/// `\bpat\b` — a whole word only, so `path`/`origPath` are untouched.
fn contains_word(haystack: &str, needle: &str) -> bool {
    let bytes = haystack.as_bytes();
    let mut from = 0;
    while let Some(rel) = haystack[from..].find(needle) {
        let start = from + rel;
        let end = start + needle.len();
        let before_ok = start == 0 || !is_word_byte(bytes[start - 1]);
        let after_ok = end == bytes.len() || !is_word_byte(bytes[end]);
        if before_ok && after_ok {
            return true;
        }
        from = start + 1;
    }
    false
}

fn is_word_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

/// W5 — objects and arrays are categorically ineligible; strings are short and
/// single-line.
fn is_allowed_scalar(v: &Value) -> bool {
    match v {
        Value::Null | Value::Bool(_) | Value::Number(_) => true,
        Value::String(s) => {
            s.chars().count() <= RAW_ARG_MAX_STR && !s.contains('\n') && !s.contains('\r')
        }
        Value::Array(_) | Value::Object(_) => false,
    }
}
