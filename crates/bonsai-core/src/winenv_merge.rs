//! Pure text half of the P71 R2 `PATH` backstop: `reg.exe` output parsing,
//! per-segment `%VAR%` expansion, the absolute-path guard, and the merge.
//!
//! Split out of [`crate::winenv`] so neither file approaches the 500-line
//! limit. Nothing here touches the process, the registry, or `std::env`
//! directly — every environment interaction goes through the injected
//! [`WinEnv`] seam, so all of it is unit-testable on any host OS.

use crate::winenv::{WinEnv, PROFILE_VARS, VOLATILE_ENV_KEY};

/// Longest value Windows will accept for one environment variable: the block
/// entry is capped at 32,767 UTF-16 units *including* the terminating NUL, so
/// the value itself may hold at most 32,766.
///
/// We compare against the UTF-8 **byte** length, which is never smaller than
/// the UTF-16 unit count for any string a `PATH` can contain — so the check is
/// conservative in the safe direction (it may refuse a value Windows would have
/// accepted; it can never wave through one it would reject).
pub(crate) const MAX_ENV_VALUE_LEN: usize = 32_766;

// ---- reg.exe output parsing ---------------------------------------------------

/// Parse `reg.exe query` stdout into every `(value name, string data)` pair it
/// contains.
///
/// Sibling of `gitbin::parse_reg_query`, kept separate for one reason: the
/// value names here (`Path`, `USERPROFILE`, …) have inconsistent stored casing
/// between hives and Windows builds, so name matching must be
/// case-INsensitive. Equally defensive: a localized, truncated, or garbage
/// block yields no pairs rather than a panic, and only genuine string types
/// (`REG_SZ` / `REG_EXPAND_SZ`) are accepted.
///
/// The `reg.exe` line shape is `<indent><name><ws><TYPE><ws><data>`; data may
/// itself contain spaces, so everything after the type token is the value.
#[cfg_attr(not(windows), allow(dead_code))]
pub(crate) fn parse_reg_values(stdout: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for line in stdout.lines() {
        let trimmed = line.trim();
        // The first whitespace token must be the WHOLE name, so a value name
        // that is a prefix of another (`Path` vs `PathExt`) never cross-matches.
        let Some(name) = trimmed.split_whitespace().next() else {
            continue;
        };
        for ty in ["REG_EXPAND_SZ", "REG_SZ"] {
            let Some(idx) = trimmed.find(ty) else {
                continue;
            };
            // Guard against a *name* that merely contains the type token.
            if idx < name.len() {
                continue;
            }
            let data = trimmed[idx + ty.len()..].trim();
            if !data.is_empty() {
                out.push((name.to_string(), data.to_string()));
            }
            break;
        }
    }
    out
}

/// Parse `reg.exe query <key> /v <value>` stdout for `value`'s string data.
/// `None` when the value is absent, non-string, or the block is unparseable.
#[cfg_attr(not(windows), allow(dead_code))]
pub(crate) fn parse_reg_query(stdout: &str, value: &str) -> Option<String> {
    parse_reg_values(stdout)
        .into_iter()
        .find(|(name, _)| name.eq_ignore_ascii_case(value))
        .map(|(_, data)| data)
}

// ---- %VAR% expansion ----------------------------------------------------------

/// Resolve one `%NAME%` reference (contract §5.3.1).
///
/// A name in [`PROFILE_VARS`] comes from `HKCU\Volatile Environment` — the
/// live per-user profile block — and falls back to the process environment
/// **only** when that read fails. Everything else (machine-scope names such as
/// `SystemRoot`, `ProgramFiles`, `ProgramW6432`, `ProgramData`) is identical
/// for every process on the box, so the inherited value is fine.
///
/// Windows environment names are case-insensitive, so the [`PROFILE_VARS`]
/// membership test is too.
fn lookup_var(name: &str, env: &dyn WinEnv) -> Option<String> {
    let is_profile_var = PROFILE_VARS.iter().any(|v| v.eq_ignore_ascii_case(name));
    if is_profile_var {
        if let Some(v) = env.registry_string(VOLATILE_ENV_KEY, name) {
            if !v.trim().is_empty() {
                return Some(v);
            }
        }
    }
    env.var(name).filter(|v| !v.trim().is_empty())
}

/// Expand `%NAME%` references in ONE `PATH` segment, or drop the segment.
///
/// - `NAME` resolves through [`lookup_var`] (profile block first for
///   [`PROFILE_VARS`], process environment otherwise).
/// - **An unresolvable `NAME` drops the whole segment** (`None`). This is
///   deliberately stricter than Windows' `RtlExpandEnvironmentStrings`, which
///   leaves the reference literal, and far stricter than expanding to empty:
///   `%SOMEVAR%\tools` expanded to empty becomes the **drive-relative**
///   `\tools`, which Windows resolves against the current drive's root, and
///   `C:\` grants `CREATE_FOLDER`/`APPEND_DATA` to `Authenticated Users` by
///   default — i.e. a plantable `C:\tools\git.exe` (contract §5.3.2).
/// - An unterminated `%` is left literal; [`is_absolute_windows_path`] then
///   judges the result (a residue like `%SOMEVAR\bin` is not absolute, so it
///   is dropped anyway).
/// - `%%` yields the empty name, which never resolves, so the segment is
///   dropped — where `ExpandEnvironmentStrings` would have left it literal.
///   Same end state: the segment does not reach `PATH`.
/// - **Single pass, no recursion**: an expansion result is never re-scanned, so
///   a self-referential value (`Path` containing `%Path%`) cannot loop or grow
///   unbounded. Never panics.
/// - The result is trimmed; an empty segment yields `None`.
pub fn expand_segment(raw: &str, env: &dyn WinEnv) -> Option<String> {
    let mut out = String::with_capacity(raw.len());
    let mut rest = raw;
    while let Some(start) = rest.find('%') {
        out.push_str(&rest[..start]);
        let after = &rest[start + 1..];
        match after.find('%') {
            // Unterminated: everything from this `%` on is literal.
            None => {
                out.push('%');
                out.push_str(after);
                rest = "";
                break;
            }
            Some(end) => {
                // Unresolvable => the segment is DROPPED, never emptied.
                out.push_str(&lookup_var(&after[..end], env)?);
                rest = &after[end + 1..];
            }
        }
    }
    out.push_str(rest);
    let trimmed = out.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}

// ---- segment validation -------------------------------------------------------

/// `true` iff `seg` is an absolute Windows path: a drive-letter root (`X:\` /
/// `X:/`) or a UNC prefix (`\\server\share`, forward slashes accepted).
///
/// Everything else is rejected — drive-relative (`\tools`), drive-current
/// (`C:tools`), bare-relative (`tools`), `.` and `..`, and any unexpanded
/// `%VAR%` residue (none of which is absolute). Contract §5.3.2.
///
/// Applies ONLY to segments R2 introduces; the inherited `PATH` is copied
/// through verbatim and is never filtered.
pub fn is_absolute_windows_path(seg: &str) -> bool {
    let bytes = seg.trim().as_bytes();
    // UNC: `\\server\share`. Requires something after the two separators.
    if bytes.len() > 2 && matches!(bytes[0], b'\\' | b'/') && matches!(bytes[1], b'\\' | b'/') {
        return true;
    }
    // Drive-rooted: `X:\...`. `X:` and `X:tools` are drive-RELATIVE — refused.
    bytes.len() >= 3
        && bytes[0].is_ascii_alphabetic()
        && bytes[1] == b':'
        && matches!(bytes[2], b'\\' | b'/')
}

/// Normalized comparison key for one `PATH` segment: surrounding whitespace and
/// trailing `\`/`/` trimmed, then lowercased (Windows paths are
/// case-insensitive). Empty segments yield `None` and are ignored — an empty
/// component means "current directory" and must never be introduced.
///
/// Only TRAILING separators are normalized; an interior `/` vs `\` difference
/// still compares unequal. Deliberately conservative: adding a duplicate
/// spelling of a directory is harmless, dropping a real entry is not.
fn normalize_segment(seg: &str) -> Option<String> {
    let trimmed = seg.trim().trim_end_matches(['\\', '/']);
    (!trimmed.is_empty()).then(|| trimmed.to_lowercase())
}

/// Is the merged value something we may hand to `set_var`?
///
/// `std::env::set_var` **panics** on a value containing NUL, and on Windows on
/// any `SetEnvironmentVariableW` failure — including one caused by exceeding
/// the per-variable length limit. Since this runs before the first paint, a
/// panic here would mean the app never opens: strictly worse than the bug being
/// fixed. Both conditions therefore degrade to "no rehydration".
pub(crate) fn is_applicable(merged: &str) -> bool {
    !merged.contains('\0') && merged.len() <= MAX_ENV_VALUE_LEN
}

// ---- the merge ----------------------------------------------------------------

/// Compute the repaired `PATH`.
///
/// - Each registry segment is expanded ([`expand_segment`]); a segment that
///   fails to expand, or that fails [`is_absolute_windows_path`], is DROPPED
///   and never appears in `added`.
/// - Comparison against the process `PATH`: case-insensitive, after trimming
///   trailing `\`/`/` and surrounding whitespace. Empty segments ignored.
/// - The existing process `PATH` is emitted **FIRST and verbatim** — never
///   reordered, never deduplicated, never dropped. `merged` always starts with
///   `process_path` byte-for-byte.
/// - Missing entries are **APPENDED** after it, system-sourced before
///   user-sourced. Nothing on the inherited `PATH` is ever shadowed — see the
///   ordering note in [`crate::winenv`]; contract §5.5 (do not re-flip).
/// - Returns `None` when nothing survives as missing, so the caller skips
///   `set_var`.
pub fn merge_path(
    system_path: &str,
    user_path: &str,
    process_path: &str,
    env: &dyn WinEnv,
) -> Option<(String, Vec<String>)> {
    let mut seen: Vec<String> = process_path.split(';').filter_map(normalize_segment).collect();

    let mut added: Vec<String> = Vec::new();
    for source in [system_path, user_path] {
        for raw in source.split(';') {
            let Some(seg) = expand_segment(raw, env) else {
                continue;
            };
            if !is_absolute_windows_path(&seg) {
                continue;
            }
            let Some(key) = normalize_segment(&seg) else {
                continue;
            };
            // Linear scan: `seen` is a PATH, i.e. tens of entries, so the O(n²)
            // membership test costs microseconds — a HashSet would only add a
            // second normalized copy of every segment for no measurable gain.
            if seen.contains(&key) {
                continue;
            }
            // Record the key so a duplicate inside the registry value itself
            // (or across the two hives) is only appended once.
            seen.push(key);
            added.push(seg);
        }
    }

    if added.is_empty() {
        return None;
    }
    let suffix = added.join(";");
    // `process_path` is emitted FIRST and BYTE-FOR-BYTE. When it is empty no
    // separator is emitted, and when it already ends in `;` (a trailing empty
    // component) no second one is added — we never introduce an empty
    // ("current directory") component that was not already there.
    let merged = if process_path.is_empty() {
        suffix
    } else if process_path.ends_with(';') {
        format!("{process_path}{suffix}")
    } else {
        format!("{process_path};{suffix}")
    };
    Some((merged, added))
}

#[cfg(test)]
#[path = "winenv_merge_tests.rs"]
mod tests;
