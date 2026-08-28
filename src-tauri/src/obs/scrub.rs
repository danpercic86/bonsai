//! P91 §7.2 / §7.2.1 — the credential scrubber, split out of `redact.rs` to
//! keep each file under the size limit. ONE concern: recognising credential
//! shapes in a string and replacing them with [`REDACTED_TOKEN`]. It runs LAST,
//! on every string field, in BOTH redaction modes (§7.2.1); the ordinal
//! `Redactor` lives in `redact.rs`.

use serde_json::Value;

use super::redact::REDACTED_TOKEN;

// ------------------------------------------------------------------ scrubbing

/// Credential prefixes that make the WHOLE surrounding word a secret, paired
/// with the minimum word length that makes the match credible (§7.2).
///
/// **The minimum length is load-bearing, in both directions.** Without it, short
/// generic prefixes (`sk-`, `akia`) would fire on ordinary hyphenated words; with
/// a single global floor (the earlier 40-char one) a `glpat-` GitLab PAT — 26
/// chars, and the likeliest non-GitHub token to appear in a *Git client* — was
/// structurally uncatchable. Prefix matching is therefore INDEPENDENT of
/// [`looks_like_opaque_secret`]'s floor, and each prefix carries its own bound.
///
/// Prefixes are lowercase; the candidate word is lowercased before comparison, so
/// `AKIA…` and `akia…` both match.
const TOKEN_PREFIXES: &[(&str, usize)] = &[
    // GitHub
    ("ghp_", 12),
    ("gho_", 12),
    ("ghu_", 12),
    ("ghs_", 12),
    ("ghr_", 12),
    ("github_pat_", 16),
    // Slack
    ("xoxb-", 12),
    ("xoxa-", 12),
    ("xoxp-", 12),
    ("xoxr-", 12),
    ("xoxs-", 12),
    ("xoxe-", 12),
    // GitLab: PAT / deploy / runner tokens.
    ("glpat-", 12),
    ("gldt-", 12),
    ("glrt-", 12),
    // npm
    ("npm_", 20),
    // AWS access-key ids (long-lived and temporary).
    ("akia", 20),
    ("asia", 20),
    // Google API keys
    ("aiza", 20),
    // OpenAI / Stripe-style secret keys.
    ("sk-", 20),
    ("sk_live_", 12),
    ("rk_live_", 12),
    // Docker Hub
    ("dckr_pat_", 12),
];

/// A key whose VALUE is always a secret regardless of shape (§7.2:
/// `/token|secret|password|passphrase|auth/i`).
///
/// **`auth` also matches `author*` — deliberately, do not "fix" it.** §7.1 says
/// author names and emails are never recorded in EITHER mode, so a key called
/// `author` carrying a value is already a bug at the emit site; collapsing it to
/// `<redacted:token>` is the fail-safe outcome. Narrowing the pattern to spare
/// `author` would trade a harmless over-redaction for a real leak.
fn is_sensitive_key(key: &str) -> bool {
    let k = key.to_ascii_lowercase();
    k.contains("token")
        || k.contains("secret")
        || k.contains("password")
        || k.contains("passphrase")
        || k.contains("auth")
}

fn is_word_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.' | '+' | '/' | '=' | '~')
}

/// Longest run of `word` containing no `/`.
fn longest_slash_free_run(word: &str) -> usize {
    word.split('/').map(str::len).max().unwrap_or(0)
}

/// Shape-detector for opaque credential blobs that carry no recognisable prefix
/// — §7.2's "AZDO / base64 PAT shape" (Azure DevOps PATs are 52 base32 chars;
/// generic base64 secrets run 32+).
///
/// The hard part is that the **base64 alphabet contains `/`**, so "contains a
/// slash ⇒ it is a path, leave it" (the first cut) let every slash-bearing base64
/// secret through. The discriminator used instead is SEGMENT LENGTH: a
/// filesystem path is a sequence of *human-named* segments, each typically well
/// under 24 characters, while a random base64 blob hits `/` about once every 64
/// characters. So a slash-bearing candidate qualifies only when some `/`-free run
/// is ≥ 24 chars — which no realistic path segment reaches and every base64
/// secret of interesting length does.
///
/// Three further exclusions keep it from eating data §7.1 says to KEEP:
///   * pure-hex words — a 40-char commit SHA is explicitly retained;
///   * words containing `.` — a hostname or a file name, never a raw secret;
///   * words needing BOTH a letter and a digit — prose, ref names and long
///     all-alphabetic identifiers are not credentials.
///
/// Over 200 chars it stops looking: that is a payload, and the shape signal is
/// worthless at that size.
fn looks_like_opaque_secret(word: &str) -> bool {
    if word.len() < 32 || word.len() > 200 {
        return false;
    }
    if word.contains('.') {
        return false;
    }
    if !word
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '/' | '=' | '-' | '_'))
    {
        return false;
    }
    if word.chars().all(|c| c.is_ascii_hexdigit()) {
        return false;
    }
    if !word.chars().any(|c| c.is_ascii_alphabetic()) || !word.chars().any(|c| c.is_ascii_digit()) {
        return false;
    }
    if word.contains('/') && longest_slash_free_run(word) < 24 {
        return false;
    }
    true
}

/// Does `word` look like a JWT? (§7.2.1 increment-3 addition.) A JWT is
/// `header.payload.signature`, where the header is base64url of `{"…`, i.e. it
/// begins `eyJ`. The regex is `eyJ[A-Za-z0-9_-]{10,}\.`: an `eyJ`-prefixed
/// segment of ≥13 chars followed by a `.`. High-precision — `eyJ` is base64 of
/// `{"`, so the false-positive risk is near zero.
///
/// The whole `header.payload.sig` token is scrubbed, not just the prefix,
/// because [`is_word_char`] treats `.` as part of a word, so the entire JWT is
/// collected as ONE word before this runs.
fn looks_like_jwt(word: &str) -> bool {
    let Some(dot) = word.find('.') else {
        return false;
    };
    let header = &word[..dot];
    header.len() >= 13
        && header.starts_with("eyJ")
        && header[3..]
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '+' | '/' | '='))
}

/// The word after a `key=value` / `key: value` credential pair should be
/// scrubbed only when the value is opaque material — not when it is an auth
/// SCHEME keyword (`Bearer`, `Basic`, …), which the dedicated scheme branch
/// handles and whose credential is the word AFTER it. Guards the in-string
/// `key=value` rule from eating `Authorization: Bearer <token>` and leaving the
/// real token behind.
fn is_auth_scheme_word(word: &str) -> bool {
    matches!(
        word.to_ascii_lowercase().as_str(),
        "bearer" | "basic" | "negotiate" | "digest" | "ntlm"
    )
}

/// Does `word` look like the base64 blob following an auth scheme? Deliberately
/// looser than [`looks_like_opaque_secret`] (a `Basic` credential is only ~24
/// chars) because the preceding `Basic`/`Bearer` keyword already establishes
/// intent — the shape check only guards against swallowing ordinary prose.
fn looks_like_auth_material(word: &str) -> bool {
    word.len() >= 8
        && word
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '/' | '=' | '-' | '_' | '.'))
        && word.chars().any(|c| c.is_ascii_alphanumeric())
}

/// Scrubs every credential shape from one string (§7.2). Runs in BOTH modes —
/// `raw` relaxes name redaction, never this.
pub fn scrub_string(s: &str) -> String {
    // 1. PEM private keys: the whole value goes, not just the header line.
    if s.contains("-----BEGIN") && s.contains("PRIVATE KEY") {
        return REDACTED_TOKEN.to_string();
    }

    let mut out = String::with_capacity(s.len());
    let bytes: Vec<char> = s.chars().collect();
    let mut i = 0usize;
    while i < bytes.len() {
        // 2. URL userinfo: `scheme://user:pass@host` — replace the userinfo span.
        if bytes[i] == ':' && bytes.get(i + 1) == Some(&'/') && bytes.get(i + 2) == Some(&'/') {
            out.push_str("://");
            i += 3;
            let start = i;
            let mut j = i;
            let mut at: Option<usize> = None;
            while j < bytes.len() {
                match bytes[j] {
                    '@' => {
                        at = Some(j);
                        break;
                    }
                    '/' | '?' | '#' | ' ' | '"' => break,
                    _ => j += 1,
                }
            }
            if let Some(at) = at {
                out.push_str(REDACTED_TOKEN);
                out.push('@');
                i = at + 1;
            } else {
                i = start;
            }
            continue;
        }

        if !is_word_char(bytes[i]) {
            out.push(bytes[i]);
            i += 1;
            continue;
        }

        // Collect one word.
        let start = i;
        while i < bytes.len() && is_word_char(bytes[i]) {
            i += 1;
        }
        let word: String = bytes[start..i].iter().collect();

        // 3. `Bearer <token>` / `Authorization: <scheme> <token>` — the word after
        //    a bearer-ish scheme is the secret.
        let lower = word.to_ascii_lowercase();
        // §7.2.1 (inc. 3): a JWT that lost its `Bearer ` keyword — the `.`
        // disqualifies the opaque-secret shape, so it would otherwise pass.
        if looks_like_jwt(&word) {
            out.push_str(REDACTED_TOKEN);
            continue;
        }
        let prefixed = TOKEN_PREFIXES
            .iter()
            .any(|(p, min)| lower.starts_with(p) && word.len() >= *min);
        if prefixed || looks_like_opaque_secret(&word) {
            out.push_str(REDACTED_TOKEN);
            continue;
        }
        // §7.2.1 (inc. 3): in-string `key=value` / `key: value` credential pairs
        // — the shape credential-helper and `.netrc` output takes inside a single
        // string value, which `is_sensitive_key` (JSON-key-based) never sees.
        // Reuses the SAME sensitive-key vocabulary. Skips auth-scheme values so
        // the dedicated `Bearer`/`Basic` branch still scrubs the real token.
        //
        // `=` is a word char (so `password=secret` is collected as ONE word),
        // hence the in-word split; `:` is NOT (so `password: secret` arrives as
        // separate words), hence the lookahead below.
        if let Some(eq) = word.find('=') {
            let key = &word[..eq];
            let value = &word[eq + 1..];
            if !value.is_empty() && is_sensitive_key(key) && !is_auth_scheme_word(value) {
                out.push_str(key);
                out.push('=');
                out.push_str(REDACTED_TOKEN);
                continue;
            }
        }
        if is_sensitive_key(&word) {
            let mut j = i;
            while j < bytes.len() && bytes[j] == ' ' {
                j += 1;
            }
            if j < bytes.len() && (bytes[j] == '=' || bytes[j] == ':') {
                j += 1;
                while j < bytes.len() && bytes[j] == ' ' {
                    j += 1;
                }
                let val_start = j;
                while j < bytes.len() && is_word_char(bytes[j]) {
                    j += 1;
                }
                let value: String = bytes[val_start..j].iter().collect();
                if !value.is_empty() && !is_auth_scheme_word(&value) {
                    out.push_str(&word);
                    out.extend(bytes[i..val_start].iter());
                    out.push_str(REDACTED_TOKEN);
                    i = j;
                    continue;
                }
            }
        }
        // `Bearer <token>` / `Basic <base64>`. `basic` additionally requires the
        // following word to LOOK like credential material, because "basic" occurs
        // in ordinary prose ("basic auth is off") where swallowing the next word
        // would corrupt the message for no privacy gain. `bearer` needs no such
        // guard — it does not appear in this codebase's prose at all.
        if lower == "bearer" || lower == "basic" {
            let mut sep = String::new();
            let mut j = i;
            while j < bytes.len() && !is_word_char(bytes[j]) {
                sep.push(bytes[j]);
                j += 1;
            }
            let start_next = j;
            while j < bytes.len() && is_word_char(bytes[j]) {
                j += 1;
            }
            let next: String = bytes[start_next..j].iter().collect();
            if !next.is_empty() && (lower == "bearer" || looks_like_auth_material(&next)) {
                out.push_str(&word);
                out.push_str(&sep);
                out.push_str(REDACTED_TOKEN);
                i = j;
                continue;
            }
        }
        out.push_str(&word);
    }
    out
}

/// Removes the literal session salt from an already-encoded line (§7.2: the salt
/// is "held in memory only, never persisted — not to a log file, not into an
/// export zip").
///
/// The salt legitimately leaves the process once, over in-process IPC, so the
/// frontend can seed its own counter. This is the backstop that keeps a producer
/// echoing it back — or a future field that forgets — from writing it to disk,
/// where it would hand a reader the key to both the ordinals and `argsHash`.
/// Case-insensitive on the hex, since either casing decodes to the same bytes.
pub fn scrub_salt(line: String, salt_hex: &str) -> String {
    if salt_hex.is_empty() {
        return line;
    }
    if line.contains(salt_hex) {
        return line.replace(salt_hex, REDACTED_TOKEN);
    }
    let upper = salt_hex.to_ascii_uppercase();
    if line.contains(&upper) {
        return line.replace(&upper, REDACTED_TOKEN);
    }
    line
}

/// Walks a serialized record and scrubs it IN PLACE (§7.2 "runs last, on every
/// string field, in both modes").
///
/// Applied on the writer thread, after serialization and immediately before the
/// line is written, so no emit site can forget it and no producer pays for it.
pub fn scrub_value(v: &mut Value) {
    match v {
        Value::String(s) => {
            let scrubbed = scrub_string(s);
            if &scrubbed != s {
                *s = scrubbed;
            }
        }
        Value::Array(items) => {
            for item in items {
                scrub_value(item);
            }
        }
        Value::Object(map) => {
            for (k, val) in map.iter_mut() {
                if is_sensitive_key(k) {
                    if !val.is_null() {
                        *val = Value::String(REDACTED_TOKEN.to_string());
                    }
                    continue;
                }
                scrub_value(val);
            }
        }
        _ => {}
    }
}
