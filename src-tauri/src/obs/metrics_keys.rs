//! P91 §8 — the metrics KEY ALLOW-LIST: the predicates that decide whether a
//! string may become a key in `usage.json`.
//!
//! ONE concern, and it is a privacy concern, not a formatting one. `usage.json`
//! is the durable file that §8 promises carries **no user content**: it is kept
//! forever, it is not covered by `logs_delete_all`, and it has no redaction pass
//! of its own. A path, branch name or commit message that reached a key would
//! therefore be permanent, un-deletable repo content. These predicates are the
//! only thing standing between an IPC-derived string and that file, so they live
//! apart from the aggregation logic where they cannot be lost in a refactor.
//!
//! Shape, not content: `cmd.<name>` accepts bare snake_case identifiers,
//! `<domain>.<action>` counter keys accept the same plus dots, and error codes
//! accept short identifier-ish tokens. Anything carrying a slash, a space, a dot
//! run or an uppercase letter — i.e. anything shaped like user data — is
//! rejected, and the caller drops the observation rather than recording it.

/// True for a command name that may become a `cmd.<name>` duration key: a bare
/// snake_case code identifier. Repo content (paths, refs, messages) carries
/// slashes, dots, spaces or uppercase and is therefore rejected — this is the
/// "no user-derived key" guard for the one key family sourced from IPC.
pub(super) fn is_valid_cmd_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 40
        && name.starts_with(|c: char| c.is_ascii_lowercase())
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

/// True for a `<domain>.<action>` counter key: lowercase ASCII segments joined by
/// single dots, no path separator, no whitespace. Used by `bump_counter`'s
/// `debug_assert` — a user-derived string (branch name, path, ref) fails it.
pub(super) fn is_valid_counter_key(key: &str) -> bool {
    !key.is_empty()
        && key.len() <= 60
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
