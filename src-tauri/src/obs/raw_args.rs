//! P91 Amendment A26 — writer-side enforcement of the raw-mode `args` invariant
//! (`docs/contracts/P91-observability.md` §7.4.2 — writer rules W1–W6).
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
//! It also applies that same scalar check to the **`repo`** base field
//! (P117 §2.2), for the same reason: `log_append` accepts arbitrary records from
//! the frontend, so `repo` — the one renderer-supplied string that reaches a
//! raw-mode file as a VALUE — cannot be trusted to hold a repoId just because
//! the producer is supposed to put one there. A failing value is replaced by
//! [`REPO_REJECTED`], so the channel is bounded free text in no mode. That is a
//! value rewrite on an existing optional field: no schema move, and it is a
//! no-op in strict mode, where `strict::enforce` has already turned the field
//! into `repo#N` before this runs.
//!
//! Any violation drops the **whole** `args` object — a producer that mislabelled
//! one argument is untrusted about the rest — and stamps a writer-set
//! `argsPolicyViolation: true`. That flag cannot be forged: [`LogPayload`] has no
//! such field, so a producer-supplied one is dropped as an unknown field at
//! deserialisation, and this module is the only writer of it.
//!
//! **This amendment does not move `OBS_SCHEMA_VERSION`**: every field it adds
//! (`argsOmitted`, `argsPolicyViolation`) is optional and additive, and the
//! version only moves when an existing field changes shape or meaning. (It now
//! reads **3**: bumped to 2 for `changedProps`, then to 3 by P117 §2.6 — not for
//! that increment's additive `repo` field, but for the changed *meaning* of
//! existing `anomaly` records, a v3 `redundant-refresh` denoting one repo where
//! a v2 one denoted any set. See `record.rs`'s module note; the §13 row 23
//! pre-release carve-out that once justified 1 no longer applies, a v1 corpus
//! having been written to disk.)
//!
//! [`LogPayload`]: super::record::LogPayload

use serde_json::Value;

use super::scrub::is_sensitive_key;

/// Longest raw `args` string kept on disk. Mirrors `RAW_ARG_MAX_STR` in
/// `src/obs/rawArgPolicy.ts` in VALUE, not in unit: the producer measures
/// `s.length` (UTF-16 code units), the writer `s.chars().count()` (scalar
/// values). Since chars ≤ UTF-16 units, the writer is very slightly LAXER —
/// which is the safe direction: everything the producer keeps, the writer
/// accepts, so no conforming record is dropped over an astral-plane character.
/// Deliberately left as-is; neither side changes.
pub const RAW_ARG_MAX_STR: usize = 512;

/// Writer-set marker: this record carried an `args` object that failed §7.4.2.
const VIOLATION_KEY: &str = "argsPolicyViolation";
const ARGS_KEY: &str = "args";
const OMITTED_KEY: &str = "argsOmitted";
const REPO_KEY: &str = "repo";

/// Replaces a `repo` value that fails the scalar gate. Deliberately carries
/// nothing from the input, and deliberately does NOT match `^repo#\d+$` — a
/// rejected value must not be readable as a strict-mode ordinal.
const REPO_REJECTED: &str = "repo-rejected";

/// §7.4.1 free-text vocabulary — applied to raw `args` KEYS only.
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
    // P117 §2.2 — the `repo` base field, gated by the SAME scalar check as an
    // `args` value. Before the `args` early return, deliberately: the two main
    // `repo` producers (`refresh`, `span{op:"graph.get"}`) carry no `args` at
    // all and would otherwise skip this entirely.
    //
    // Documentation is not enforcement (A26): a renderer bug or a hand-written
    // `logRecord({ repo: … })` can put anything here, and in raw mode the value
    // is written after home-masking and credential scrubbing only. There is no
    // line-forging risk — `serde_json` escapes control characters, so a
    // multi-line value cannot split a JSONL record — this bounds free-text
    // CAPTURE, which the header's `redactionNote` forbids outright.
    if map
        .get(REPO_KEY)
        .is_some_and(|repo| !is_allowed_scalar(repo))
    {
        map.insert(REPO_KEY.to_string(), Value::String(REPO_REJECTED.into()));
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

/// §7.4.1 free-text vocabulary. Not used by `scrub.rs` — see [`FREE_TEXT`].
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
