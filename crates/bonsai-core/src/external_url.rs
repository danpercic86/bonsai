//! The web-URL half of external launching (P72), split out of `external.rs` in
//! the 2026-09-11 security increment so both files stay under the ~500-line
//! soft limit. ONE concern: turning a *link* into a browser launch safely.
//!
//! It shares `external`'s [`LaunchSpec`] / [`CommandRunner`] / [`launch_first`]
//! machinery and its `spec` / `open_spec` constructors (`pub(crate)`), so the
//! "never a shell command line" invariant is the same code, not a copy.

use crate::error::AppError;
use crate::external::{launch_first, open_spec, spec, CommandRunner, LaunchSpec, TargetOs};
use crate::procutil::safe_cwd;

/// Accept ONLY a plain web URL (P72), so a launcher can never be handed a
/// protocol the OS would resolve to something else. Pure: no fs, no spawn.
///
/// Accepts: an `http://` or `https://` scheme, matched CASE-INSENSITIVELY, with
/// a non-empty host drawn only from `[A-Za-z0-9.:_%-]` plus `[`/`]` for IPv6.
///
/// Three of the rules below came from the P72 security audit. None was
/// exploitable as written, but each is one line and each closes a real class:
///  * **No userinfo** (LOW-3, the sharpest). `@` in the host is rejected, because
///    `https://github.com@evil.example/` passes every other check while the
///    browser navigates to `evil.example`. On the `PrDetailView` path the URL
///    comes from a forge API response, so a hostile or compromised forge could
///    make "Open in browser" open an attacker page under a trustworthy-looking
///    label. Phishing, not code execution — but this is exactly the surface where
///    destination honesty IS the security property.
///  * **No whitespace or control characters ANYWHERE** (LOW-2), not only in the
///    host. A raw newline or tab in the path is inert on Windows/macOS, but
///    `xdg-open` is a shell script whose `$BROWSER`-with-`%s` branch word-splits
///    unquoted, turning a space into extra argv tokens for the browser.
///  * **A 2048-byte cap** (LOW-2), so an over-long forge string fails here with a
///    clean category error instead of an OS "filename or extension is too long"
///    at spawn time.
///
/// The host rule is an ALLOW-list, not a deny-list of the characters someone has
/// thought of so far: a deny-list on a security boundary needs re-auditing every
/// time a new byte is considered.
/// Rejects: every other scheme (`file:`, `javascript:`, `data:`, `ms-msdt:`,
/// `vscode:`), a UNC `\\server\share` path, a bare host with no scheme, a scheme
/// with no host (`https://`, `http:///x`), an empty/whitespace-only string, a
/// host containing a space or a `\`, and any input whose first character is `-`
/// (so the URL can never be parsed as a FLAG by the launcher program).
///
/// Load-bearing, not decorative: `PrDetailView`'s URL comes from a forge API
/// response, i.e. from outside the app. No URL crate is added — this is a
/// deliberate allow-list on a string, matching the crate's
/// hand-rolled-over-dependency house style (base64, percent-encoding).
///
/// SECURITY: the error message is CATEGORY-ONLY and never echoes `url`. A
/// forge-supplied URL can be arbitrarily long and can carry markup or lookalike
/// text; rendering it in a toast would turn a rejected link into a UI-spoofing
/// surface. (A launch *failure* from [`launch_first`] keeps its existing wording
/// and names only the program — never the URL.)
pub fn validate_web_url(url: &str) -> Result<(), AppError> {
    // Generous for any real PR/settings URL; see the audit LOW-2 note above.
    const MAX_LEN: usize = 2048;

    if url.is_empty() || url.starts_with('-') || url.len() > MAX_LEN {
        return Err(AppError::ExternalToolFailed(
            "refused to open a malformed link".to_string(),
        ));
    }
    // The whitespace/control screen covers the WHOLE url, not just the host:
    // `xdg-open` is a shell script whose $BROWSER-with-%s branch word-splits
    // unquoted, so a space in the PATH becomes extra argv tokens for the
    // browser (audit LOW-2).
    if url.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return Err(AppError::ExternalToolFailed(
            "refused to open a malformed link".to_string(),
        ));
    }
    // ASCII-only lowering, so byte offsets below stay valid for the original.
    let lower = url.to_ascii_lowercase();
    let rest = if lower.starts_with("https://") {
        &url["https://".len()..]
    } else if lower.starts_with("http://") {
        &url["http://".len()..]
    } else {
        return Err(AppError::ExternalToolFailed(
            "refused to open a link that is not http or https".to_string(),
        ));
    };
    let host = match rest.find(['/', '?', '#']) {
        Some(end) => &rest[..end],
        None => rest,
    };
    // Allow-list, NOT a deny-list (see the doc comment). Excluding `@` is what
    // rejects `https://github.com@evil.example/` — the userinfo impersonation of
    // audit LOW-3; a backslash is excluded by the same rule.
    let host_ok = !host.is_empty()
        && host.chars().all(|c| {
            c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_' | ':' | '%' | '[' | ']')
        });
    if !host_ok {
        return Err(AppError::ExternalToolFailed(
            "refused to open a link with no host".to_string(),
        ));
    }
    Ok(())
}

/// Ordered browser-launch candidates for `url` (P72). Pure — takes an explicit
/// [`TargetOs`], never `cfg!`, so every branch runs in unit tests on one host.
/// The caller MUST have validated `url` first ([`validate_web_url`]).
///
/// `cwd` is [`safe_cwd`] for every entry: no repo path is involved here, so
/// nothing observable depends on it — and `"."` (the process cwd, which is the
/// repo when Bonsai was launched from one) would have left exactly the
/// DLL-search-order primitive audit LOW-1 is about. One helper, one rule, for
/// every launcher in the crate.
///
/// `cmd /c start` is explicitly NOT a rung: `start` is a `cmd.exe` builtin, so
/// using it means handing a string to a shell (the exact thing P49 D2 forbids),
/// `cmd` would apply its own parsing to `&`, `^` and `%VAR%`, and its `start`
/// builtin treats the first quoted token as a window *title*. `explorer` and
/// `rundll32` each take the URL as a single argv token with no shell involved.
///
/// **Deliberately NOT `pub`** (audit LOW-1). The `rundll32
/// url.dll,FileProtocolHandler` rung is a general ShellExecute dispatcher: handed
/// a `.exe`, a `.hta`, a UNC path or an `ms-msdt:` string it would launch it. The
/// only thing between that and arbitrary execution is that the caller validated
/// first — so the ladder is not exported, leaving [`open_url`] (which validates
/// unconditionally) as the sole way in. A doc comment is not a sufficient guard
/// for a primitive of that shape.
fn url_ladder(os: TargetOs, url: &str) -> Vec<LaunchSpec> {
    let cwd = safe_cwd();
    match os {
        // Both Windows rungs stay detached (`wait_for_exit: false`) for the same
        // reason as `reveal_spec`: `explorer` habitually exits non-zero AFTER a
        // successful hand-off, so waiting would report a bogus failure and
        // pointlessly advance the ladder.
        TargetOs::Windows => vec![
            spec("explorer", &[url], &cwd, true, false),
            spec(
                "rundll32",
                &["url.dll,FileProtocolHandler", url],
                &cwd,
                true,
                false,
            ),
        ],
        // `open` always spawns fine and reports a failure only through its exit
        // code, so it gets the documented `wait_for_exit` treatment.
        TargetOs::MacOs => vec![open_spec(&[url], &cwd, false)],
        TargetOs::Linux => vec![spec("xdg-open", &[url], &cwd, true, false)],
    }
}

/// Validate `url`, then open it in the user's default browser via the first
/// candidate that launches (P72). Validation runs BEFORE any spawn, so a
/// rejected URL never reaches a process. `what` is `"browser"`, so a total
/// failure reads `could not launch browser (rundll32): …`.
pub fn open_url(runner: &dyn CommandRunner, os: TargetOs, url: &str) -> Result<(), AppError> {
    validate_web_url(url)?;
    launch_first(runner, &url_ladder(os, url), "browser")
}

#[cfg(test)]
#[path = "external_url_tests.rs"]
mod tests;
