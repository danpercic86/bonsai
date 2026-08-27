//! P91 §7 — redaction: the per-session salt, stable ordinal assignment, the
//! salted args hash, and the credential scrubber.
//!
//! ONE concern: turning a value into something safe to write. It performs no
//! IO and knows nothing about files or channels.
//!
//! ## Why FNV-1a-64 and not SHA-256
//!
//! `argsHash` is an EQUALITY token: `dup-ipc` (§5) fires when two calls share a
//! `cmd` + `argsHash`. It is never a proof of possession and never verified. The
//! privacy property comes from the salt — **16 random bytes minted per session
//! and never persisted** — not from the hash's collision resistance, so a
//! preimage attack would have to guess a secret that no longer exists once the
//! app closes.
//!
//! FNV-1a is chosen because §7.2 requires the FRONTEND to reproduce the identical
//! scheme (`src/obs/redact.ts`, increment 2) **synchronously**, inside the IPC
//! proxy's hot path. WebCrypto's SHA-256 is async-only and would force the proxy
//! to either buffer or block; a hand-rolled JS SHA-256 would be a second
//! cryptographic implementation to keep in step. A 10-line FNV mirror cannot
//! drift. This is documented rather than silently chosen: it is NOT a
//! cryptographic hash and must never be used as one.
//!
//! ## Why `Mutex<HashMap>` and not `DashMap`
//!
//! The contract sketches `DashMap`; the workspace has no such dependency and
//! this milestone is not the place to add one. Contention is nil in practice —
//! the map is touched once per redacted value, held for a hash lookup, and
//! never across IO — and §11's ≤5 µs/record budget has ~3 orders of magnitude of
//! headroom over an uncontended `Mutex` lock.

use std::collections::HashMap;
use std::sync::Mutex;

use serde_json::Value;

/// The placeholder every credential match collapses to.
pub const REDACTED_TOKEN: &str = "<redacted:token>";

/// Which ordinal namespace a value belongs to. Each kind counts from 1
/// independently, so `ref#3` and `path#3` are unrelated values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Kind {
    Repo,
    Path,
    Ref,
    Remote,
    Other,
}

impl Kind {
    fn prefix(self) -> &'static str {
        match self {
            Kind::Repo => "repo",
            Kind::Path => "path",
            Kind::Ref => "ref",
            Kind::Remote => "remote",
            Kind::Other => "other",
        }
    }
}

#[derive(Default)]
struct Ordinals {
    ids: HashMap<(Kind, String), u32>,
    next: HashMap<Kind, u32>,
}

/// Session-scoped redaction state (§7.2).
///
/// One `Redactor` lives for exactly one logging session and is SHARED by a purge
/// roll (§6.1) so the new file continues the same ordinals. It is dropped — salt
/// and all — when Dev mode is turned off.
pub struct Redactor {
    salt: [u8; 16],
    ordinals: Mutex<Ordinals>,
}

impl Redactor {
    /// Mints a redactor with 16 fresh random bytes. Held in memory only, never
    /// written to disk: this is what makes the same branch `ref#3` throughout one
    /// file and a DIFFERENT ordinal in tomorrow's file.
    pub fn new() -> Self {
        let mut salt = [0u8; 16];
        rand::fill(&mut salt);
        Redactor {
            salt,
            ordinals: Mutex::new(Ordinals::default()),
        }
    }

    /// Constructs a redactor with a caller-supplied salt. TEST ONLY — production
    /// always uses [`Redactor::new`]; a fixed salt would defeat §7.2.
    #[cfg(test)]
    pub fn with_salt(salt: [u8; 16]) -> Self {
        Redactor {
            salt,
            ordinals: Mutex::new(Ordinals::default()),
        }
    }

    /// Lowercase hex of the session salt, handed to the frontend once at boot via
    /// `log_session_info` so `src/obs/redact.ts` produces the SAME `ref#3`
    /// (§7.2). It never leaves the process any other way and is never persisted.
    pub fn salt_hex(&self) -> String {
        self.salt.iter().map(|b| format!("{b:02x}")).collect()
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Ordinals> {
        // No invariant spans a panic here (a plain map + counter), so a poisoned
        // lock is recoverable — recovering beats failing every later redaction.
        self.ordinals
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// `"path#7"` — first sight of a value assigns the next ordinal for its kind;
    /// every later sight in the same session returns the same string.
    pub fn tag(&self, kind: Kind, value: &str) -> String {
        let mut g = self.lock();
        let key = (kind, value.to_string());
        if let Some(n) = g.ids.get(&key) {
            return format!("{}#{n}", kind.prefix());
        }
        // Counters START at a salt-derived offset, they do not start at 0.
        //
        // This is what makes §7.2's cross-session requirement true: with a plain
        // 0-based counter, two sessions that happen to see the same values in the
        // same order (the overwhelmingly common case — open the same repo, expand
        // the same sidebar) would produce IDENTICAL ordinals, and two files could
        // be cross-linked or dictionary-attacked exactly as §7.2 forbids. Seeding
        // from the never-persisted session salt costs nothing, keeps ordinals
        // small and readable, and keeps them collision-free within a session.
        let seed = 1 + (fnv1a64(&self.salt, kind.prefix().as_bytes()) % 900) as u32;
        let counter = g.next.entry(kind).or_insert(seed);
        *counter += 1;
        let n = *counter;
        g.ids.insert(key, n);
        format!("{}#{n}", kind.prefix())
    }

    /// `"path#7.ts"` — §7.1 keeps a file's EXTENSION (it is a language signal, not
    /// an identifier) while replacing the path itself.
    pub fn tag_path(&self, value: &str) -> String {
        let base = self.tag(Kind::Path, value);
        let file = value.rsplit(['/', '\\']).next().unwrap_or(value);
        match file.rsplit_once('.') {
            // A leading dot is the whole name (`.gitignore`), not an extension.
            Some((stem, ext)) if !stem.is_empty() && !ext.is_empty() && ext.len() <= 12 => {
                format!("{base}.{ext}")
            }
            _ => base,
        }
    }

    /// Salted 8-hex digest of canonicalized JSON args — powers `dup-ipc` (§5)
    /// without storing content (§7.2). See the module header on the hash choice.
    pub fn hash_args(&self, canonical_json: &str) -> String {
        let h = fnv1a64(&self.salt, canonical_json.as_bytes());
        // Fold 64 → 32 bits so the token is 8 hex chars, as specified.
        let folded = ((h >> 32) ^ (h & 0xffff_ffff)) as u32;
        format!("{folded:08x}")
    }
}

impl Default for Redactor {
    fn default() -> Self {
        Redactor::new()
    }
}

/// FNV-1a over `salt || bytes`. Deliberately trivial so the TS mirror is a
/// line-for-line copy (see the module header).
fn fnv1a64(salt: &[u8; 16], bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in salt.iter().chain(bytes.iter()) {
        h ^= *b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

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
        let prefixed = TOKEN_PREFIXES
            .iter()
            .any(|(p, min)| lower.starts_with(p) && word.len() >= *min);
        if prefixed || looks_like_opaque_secret(&word) {
            out.push_str(REDACTED_TOKEN);
            continue;
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
