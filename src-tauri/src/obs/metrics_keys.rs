//! P91 §8 — the metrics KEY ALLOW-LIST: the predicates that decide whether a
//! string may become a key in `usage.json`.
//!
//! ONE concern, and it is a privacy concern, not a formatting one. `usage.json`
//! is the durable file that §8 promises carries **no user content**: its
//! `lifetime` totals are never aged out, and it has no redaction pass of its own.
//! A path, branch name or commit message that reached a key would therefore be
//! written verbatim and survive until the user chose to delete. These predicates
//! are the only thing standing between an IPC-derived string and that file, so
//! they live apart from the aggregation logic where they cannot be lost in a
//! refactor.
//!
//! §F6 (2026-09-11) made the file deletable through `logs_delete_all` and capped
//! the per-day profile at 90 days. **That weakens nothing here.** Deletability is
//! a remedy the user has to invoke; these guards stop the bad key from ever being
//! written, and they keep their full force — "un-deletable" was one reason for
//! them, never the only one.
//!
//! Shape, not content: `cmd.<name>` accepts bare snake_case identifiers,
//! `<domain>.<action>` counter keys accept the same plus dots, and error codes
//! accept short identifier-ish tokens. Anything carrying a slash, a space, a dot
//! run or an uppercase letter — i.e. anything shaped like user data — is
//! rejected, and the caller drops the observation rather than recording it.

/// True for a command name that may become a `cmd.<name>` duration key: a bare
/// code identifier — `lowerCamelCase` (what `obs/ipcProxy.ts` sends, since `cmd`
/// IS the `IpcApi` method name) or snake_case. Repo content (paths, refs,
/// messages) carries slashes, dots, spaces or leading uppercase and is rejected
/// here.
///
/// SHAPE ONLY, and therefore NOT sufficient on its own (audit F3): it accepts
/// unlimited well-shaped strings, so `metrics::observe_ipc_result` pairs it with
/// `metrics_cmds::is_known_cmd`, exact membership in the real `IpcApi` method
/// set. This predicate stays as the cheap first gate and as the documented home
/// of the shape rule (§8 decision 25).
pub(super) fn is_valid_cmd_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 40
        && name.starts_with(|c: char| c.is_ascii_lowercase())
        && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// True for a `<domain>.<action>` counter key: lowercase ASCII segments joined by
/// single dots, no path separator, no whitespace. A user-derived string (branch
/// name, path, ref) fails it. Enforced by `MetricsState::bump_validated` — the
/// single sink BOTH counter writers go through (`bump_counter` and the
/// production one, `fold_perf`) — as a RUNTIME `if`, not a `debug_assert`, which
/// release builds compile out (audit F2).
pub(super) fn is_valid_counter_key(key: &str) -> bool {
    !key.is_empty()
        && key.len() <= 60
        // A counter key is `<domain>.<action>`, so the separator is REQUIRED: a
        // bare lowercase token (a forge token, an oid, an id) is shape-valid
        // otherwise, and because every write to `counters` — `fold_perf`
        // included — passes through `bump_validated`, "shape-valid" is exactly
        // what decides whether a key is persisted.
        && key.contains('.')
        && !key.starts_with('.')
        && !key.ends_with('.')
        && !key.contains("..")
        && key
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '.')
}

/// True for an error code that may become an `errors` key. Codes are short
/// identifier-ish tokens; anything else is dropped rather than recorded.
pub(super) fn is_valid_err_code(code: &str) -> bool {
    !code.is_empty()
        && code.len() <= 48
        && code
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-'))
}
