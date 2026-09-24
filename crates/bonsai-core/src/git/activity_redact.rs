//! P119 §2.8 security: strip URL credentials from a failed run's reason line.
//!
//! The reason line is `AppError::message()` — the toast text — and some of those
//! messages quote the remote URL verbatim (`authentication failed for '<url>'`,
//! `network error talking to '<url>'`, from clone and submodule add). A clone URL
//! can carry `user:token@`, and the activity log is a second, longer-lived
//! surface, so the reason line is scrubbed here before it reaches the wire. The
//! toast/`AppError` text itself is deliberately untouched.
//!
//! Rules (pure, no allocation beyond the output string):
//! - `scheme://user[:pass]@host…` → `scheme://host…` (the userinfo before the
//!   LAST `@` of the authority goes; a `@` after the first `/` of the path is
//!   left alone).
//! - scp-style `user[:pass]@host:path` → `host:path`.
//! - An email-like `a@b` in plain prose is left alone UNLESS the host part is
//!   directly followed by `:` (then it is indistinguishable from scp syntax and
//!   is redacted — over-redacting prose is the safe failure).

use std::ops::Range;

/// Removes URL userinfo (`scheme://user:pass@` → `scheme://`) and scp-style
/// `user@host:` → `host:` from `text`. Everything else is returned unchanged.
pub(crate) fn redact_url_userinfo(text: &str) -> String {
    let mut out = text.to_string();
    // Each pass removes one userinfo span, so the number of `@`s bounds the
    // loop (a password containing a raw `@` takes two passes).
    for _ in 0..=text.matches('@').count() {
        match find_userinfo(&out) {
            Some(span) => out.replace_range(span, ""),
            None => break,
        }
    }
    out
}

/// Characters that end a URL/scp token inside a message.
fn is_delim(c: char) -> bool {
    c.is_whitespace()
        || matches!(
            c,
            '\'' | '"' | '`' | '(' | ')' | '<' | '>' | '[' | ']' | '{' | '}' | ',' | ';'
        )
}

/// The byte span of the first credential-carrying userinfo (including its
/// trailing `@`), if any.
fn find_userinfo(s: &str) -> Option<Range<usize>> {
    for (at, _) in s.match_indices('@') {
        let start = s[..at]
            .char_indices()
            .rev()
            .find(|&(_, c)| is_delim(c))
            .map_or(0, |(i, c)| i + c.len_utf8());
        let token = &s[start..at];
        if let Some(p) = token.find("://") {
            let ui_start = start + p + 3;
            // `https://host/path@x`: the `@` is in the path, not userinfo.
            if ui_start < at && !s[ui_start..at].contains('/') {
                return Some(ui_start..at + 1);
            }
            continue;
        }
        if token.is_empty() || token.contains('/') {
            continue;
        }
        let rest = &s[at + 1..];
        let host_len = rest
            .find(|c: char| !(c.is_ascii_alphanumeric() || c == '.' || c == '-'))
            .unwrap_or(rest.len());
        if host_len > 0 && rest[host_len..].starts_with(':') {
            return Some(start..at + 1);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::redact_url_userinfo as r;

    #[test]
    fn strips_scheme_userinfo() {
        assert_eq!(
            r("authentication failed for 'https://user:tok@host/r.git' (check credentials)"),
            "authentication failed for 'https://host/r.git' (check credentials)"
        );
        assert_eq!(r("https://tok@host/r.git"), "https://host/r.git");
        assert_eq!(r("ssh://git@host:22/r.git"), "ssh://host:22/r.git");
        assert_eq!(
            r("network error talking to 'https://u:p@127.0.0.1:1/r.git': refused"),
            "network error talking to 'https://127.0.0.1:1/r.git': refused"
        );
    }

    #[test]
    fn strips_scp_userinfo() {
        assert_eq!(r("git@github.com:x/y.git"), "github.com:x/y.git");
        assert_eq!(r("failed for 'u:tok@host:x/y'"), "failed for 'host:x/y'");
    }

    #[test]
    fn strips_every_url_and_a_raw_at_in_the_password() {
        assert_eq!(
            r("a https://u:t1@h1/x b git@h2:y c https://t2@h3/z"),
            "a https://h1/x b h2:y c https://h3/z"
        );
        assert_eq!(r("https://u:p@ss@host/r"), "https://host/r");
    }

    #[test]
    fn leaves_other_text_alone() {
        for s in [
            "branch 'x' not found",
            "",
            "contact a@b.com for help",
            "mail a@b",
            "https://host/path@v1",
            "https://@host/r",
            "@ alone",
        ] {
            assert_eq!(r(s), s, "{s:?}");
        }
        // Documented over-redaction: email directly followed by `:` reads as scp.
        assert_eq!(r("ask a@b.com: now"), "ask b.com: now");
    }
}
