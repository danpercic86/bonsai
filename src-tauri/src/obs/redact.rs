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

// ---------------------------------------------------------------- scrubbing
//
// The credential scrubber (§7.2.1) lives in its own `scrub` module to keep this
// file under the size limit; re-exported so `redact::scrub_*` call sites (the
// writer, tests) are unchanged.
pub use super::scrub::{scrub_salt, scrub_string, scrub_value};
