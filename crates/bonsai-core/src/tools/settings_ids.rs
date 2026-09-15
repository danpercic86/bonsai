//! The two PURE settings functions: what a renderer-written tool id coerces to
//! (P112 §5.2), and what a legacy free-text command migrates to (§5.3).
//!
//! Both live here rather than in `tools/mod.rs` because neither touches the
//! filesystem, the scan cache, or a `ToolEnv` — they are string→id lookups over
//! the compile-time catalog, and that is the whole reason they are safe to run
//! on the settings-write path.
//!
//! ## Why coercion and not validation
//!
//! Save-time *validation* was ruled out: the settings writer merges pending
//! keys into one patch and re-queues on failure (recorded in the deleted
//! `external_cmd.rs` module doc, P112 §5.2),
//! so a rejection would wedge every later settings write. Coercion cannot fail
//! — anything that is not a catalog id becomes `""`, which means "auto ladder".
//! A renderer can therefore write garbage as often as it likes and the only
//! effect is that nothing is selected.
//!
//! ## The STORED output is always a `&'static str` from the catalog
//!
//! Both *id* functions return either `String::new()` or a clone of a catalog
//! literal (`entry.id` / a `LEGACY_ALIASES` target) — never a caller-supplied
//! substring. That is deliberate: it makes "a stored setting can name a
//! program" untrue by construction rather than by argument.
//!
//! [`legacy_tool_stem`] is the one carve-out and returns caller-supplied text,
//! so it is kept honest by role: it is the normalisation the lookup runs on,
//! exposed for DIAGNOSTICS ONLY (the §5.3 migration logs it when the lookup
//! misses). Nothing stores it, and nothing may — it is a program name.

use super::catalog::{self, CUSTOM_ID, LEGACY_ALIASES};
use super::ToolKind;

/// The write-time coercion (P112 §5.2). PURE: catalog lookup, zero IO, never
/// errors.
///
/// Returns the id unchanged when `CATALOG` holds it for `kind` on **any** OS
/// (settings sync between machines, and a stale selection is kept on purpose —
/// the OQ1 ruling: it is named from the label map and falls back to the auto
/// ladder at launch), or `CUSTOM_ID` when `has_custom_path`. Everything else —
/// a program name, a path, a template, an unknown or wrong-kind id, `"custom"`
/// with nothing browsed — becomes `""`.
///
/// `has_custom_path` is the caller's `!custom_*_path.is_empty()`; the path
/// itself is never passed here, because nothing about it is a lookup key.
pub fn coerce_tool_id(value: &str, kind: ToolKind, has_custom_path: bool) -> String {
    if value == CUSTOM_ID {
        // `"custom"` is renderer-sendable AND coerce-safe: it can only select a
        // path the user already browsed to. With none stored there is nothing
        // to select, so it degrades to the auto ladder.
        return if has_custom_path {
            CUSTOM_ID.to_string()
        } else {
            String::new()
        };
    }
    // `entry.id`, not `value`: the stored string is then literally a catalog
    // literal, whatever the lookup's matching rules ever become.
    catalog::find(kind, value).map_or_else(String::new, |e| e.id.to_string())
}

/// The one-shot migration of a legacy `terminalCommand` / `editorCommand`
/// (P112 §5.3). PURE: no filesystem, no probe, no spawn.
///
/// A hit is a catalog id; a miss is `""` — **accepted, not preserved**. The
/// output can never be `CUSTOM_ID` and can never be a path: migration must not
/// be able to manufacture the human dialog click §5.4 requires, because a
/// legacy `C:\Tools\npp\notepad++.exe` becoming `custom_editor_path` would
/// reintroduce exactly the capability P112 deletes.
///
/// Normalisation, in order: drop `{path}` and everything after it, take the
/// first whitespace-delimited token, take its file stem, lowercase, look up.
/// So `"code {path}"` ⇒ `vscode`, `"cmd /K"` ⇒ `cmd`, `"powershell -c calc"` ⇒
/// `powershell` (the user asked for PowerShell; dropping the `-c calc` tail is
/// the point), and an absolute path with spaces normalises to its first token
/// and therefore usually misses.
pub fn legacy_tool_id(legacy: &str, kind: ToolKind) -> String {
    let stem = legacy_tool_stem(legacy);
    if stem.is_empty() {
        return String::new();
    }
    LEGACY_ALIASES
        .iter()
        .find(|((k, alias), _)| *k == kind && *alias == stem)
        .map_or_else(String::new, |(_, id)| (*id).to_string())
}

/// The normalised lookup key [`legacy_tool_id`] searches `LEGACY_ALIASES` with:
/// up to the first `{` (drops `{path}` and any tail), first whitespace-delimited
/// token, file stem, lowercased. `""` when nothing is left.
///
/// Exposed because the §5.3 migration logs it on a MISS — a legacy command that
/// maps to nothing is dropped, and this is the only thing that says so. It is
/// the one function here that returns caller-supplied text (module doc), so:
/// **log it, never store it.** It is safe to log precisely because the
/// normalisation has already removed the directories and the arguments —
/// `C:\Tools\npp\notepad++.exe -multiInst` is `notepad++` by the time it is
/// returned — so a diagnostic cannot leak a user's path.
pub fn legacy_tool_stem(legacy: &str) -> String {
    let head = match legacy.find('{') {
        Some(i) => &legacy[..i],
        None => legacy,
    };
    let token = head.split_whitespace().next().unwrap_or_default();
    file_stem_of(token).to_lowercase()
}

/// `Path::file_stem`, but answering the same way on every host.
///
/// `Path` is host-relative: on unix `C:\Tools\x.exe` has no directory
/// component at all, so the real `file_stem` would yield `C:\Tools\x` there and
/// `x` on Windows — a migration whose result depended on which machine ran it.
/// Both separators are stripped here, on every host.
fn file_stem_of(token: &str) -> &str {
    let name = match token.rfind(['/', '\\']) {
        Some(i) => &token[i + 1..],
        None => token,
    };
    // `i > 0` mirrors `file_stem`: a leading dot is part of the name.
    match name.rfind('.') {
        Some(i) if i > 0 => &name[..i],
        _ => name,
    }
}
