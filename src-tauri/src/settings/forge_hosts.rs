//! P79 forge known-hosts index for [`crate::settings`]: the [`ForgeHostRecord`]
//! type plus its upsert / remove / backfill helpers.
//!
//! Split out of `settings.rs` to keep whole-file reads cheap (CLAUDE.md
//! file-size discipline). Re-exported from the `settings` module so external
//! call sites are unchanged.

use super::Settings;

/// P79: one forge host Bonsai has stored a PAT for. The keychain is the store of
/// record for the token; this index only remembers WHICH hosts exist (the
/// keychain can't be enumerated portably) plus a display hint. NEVER holds the
/// token.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ForgeHostRecord {
    /// Lowercased host, e.g. "github.com". Keychain account key.
    pub host: String,
    /// Provider kind, so add-for-host / list can pick the right API without a
    /// repo. Serialized as `bonsai_forge::ForgeKind` camelCase ("gitHub" | ...).
    pub kind: bonsai_forge::ForgeKind,
    /// Last-known login for offline display (avatar is fetched fresh / from the
    /// viewer cache; not persisted). `None` until first successful validation.
    pub login: Option<String>,
}

/// P79: insert-or-replace the known-hosts index record for `host` (keyed by the
/// lowercased host). Called after every successful set-token (per-repo and
/// host-based). NEVER stores a token — only host + kind + optional login.
pub fn upsert_forge_host(
    s: &mut Settings,
    host: &str,
    kind: bonsai_forge::ForgeKind,
    login: Option<String>,
) {
    let host = host.to_ascii_lowercase();
    if host.is_empty() {
        return;
    }
    s.forge_hosts.retain(|r| r.host != host);
    s.forge_hosts
        .insert(0, ForgeHostRecord { host, kind, login });
}

/// P79: remove the known-hosts index record for `host`. Called after every
/// clear-token (per-repo and host-based). No-op when absent.
pub fn remove_forge_host(s: &mut Settings, host: &str) {
    let host = host.to_ascii_lowercase();
    s.forge_hosts.retain(|r| r.host != host);
}

/// P79 lazy backfill (OD-1): add a record for `host` ONLY when it is absent from
/// the index (a token exists in the keychain but was stored pre-P79 / by another
/// path). Does not clobber an existing record's login. Returns `true` if it
/// inserted, so the caller can skip the write when nothing changed.
pub fn backfill_forge_host(
    s: &mut Settings,
    host: &str,
    kind: bonsai_forge::ForgeKind,
    login: Option<String>,
) -> bool {
    let host = host.to_ascii_lowercase();
    if host.is_empty() || s.forge_hosts.iter().any(|r| r.host == host) {
        return false;
    }
    s.forge_hosts
        .insert(0, ForgeHostRecord { host, kind, login });
    true
}
