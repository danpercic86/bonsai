//! P80 multi-account forge model for [`crate::settings`]: the account records,
//! host-default + per-repo-override indices, their upsert/remove helpers, and the
//! lazy P79→P80 migration.
//!
//! Split out of `settings.rs` to keep whole-file reads cheap (CLAUDE.md
//! file-size discipline). Re-exported from the `settings` module so external
//! call sites are unchanged. NEVER stores a token — tokens live only in the OS
//! keychain (see `bonsai_forge::auth`).

use super::Settings;

/// P80: one connected (or previously-connected) forge account. NEVER holds a
/// token — only identity + a display hint.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ForgeAccountRecord {
    /// Stable identity "kind:host:login" (or "kind:host" if login unknown).
    pub account_id: String,
    /// Actual OS-keychain account key holding this account's token
    /// (== account_id for P80 accounts, == host for a migrated legacy account).
    /// NOT a token.
    pub keychain_key: String,
    /// Lowercased host, e.g. "github.com".
    pub host: String,
    pub kind: bonsai_forge::ForgeKind,
    /// None until first successful validation.
    pub login: Option<String>,
    /// Best-effort display hint; never a token.
    pub avatar_url: Option<String>,
}

/// P80: the default account for a host; repos inherit it (via owner match →
/// host default resolution).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ForgeHostDefault {
    /// Lowercased host.
    pub host: String,
    pub account_id: String,
}

/// P80: a repo's pinned account override (OD-1: keyed by canonical workdir path,
/// deduped via [`crate::commands::same_repo_path`], mirroring `recent_repos`).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepoForgeOverride {
    /// Canonical repo workdir path.
    pub repo_path: String,
    pub account_id: String,
}

/// The camelCase wire string for a [`bonsai_forge::ForgeKind`], used to build a
/// stable [`account_id`]. Matches the serde `rename_all = "camelCase"` variants.
pub fn kind_wire_str(kind: bonsai_forge::ForgeKind) -> &'static str {
    use bonsai_forge::ForgeKind::*;
    match kind {
        GitHub => "gitHub",
        GitLab => "gitLab",
        Bitbucket => "bitbucket",
        AzureDevOps => "azureDevOps",
        Unknown => "unknown",
    }
}

/// P80: the stable account identity `kind:host:login` (lowercased throughout),
/// or the two-part legacy/host-default marker `kind:host` when `login` is
/// unknown. Case-insensitive, matching the keychain's lowercasing.
pub fn account_id(
    kind: bonsai_forge::ForgeKind,
    host: &str,
    login: Option<&str>,
) -> String {
    let base = format!("{}:{}", kind_wire_str(kind), host.to_ascii_lowercase());
    match login {
        Some(l) if !l.is_empty() => format!("{base}:{}", l.to_ascii_lowercase()),
        _ => base,
    }
}

/// P80: insert-or-replace an account record keyed by its `account_id`
/// (front-inserted so most-recent is first, mirroring `upsert_forge_host`).
pub fn upsert_forge_account(s: &mut Settings, rec: ForgeAccountRecord) {
    if rec.account_id.is_empty() {
        return;
    }
    s.forge_accounts.retain(|r| r.account_id != rec.account_id);
    s.forge_accounts.insert(0, rec);
}

/// P80: remove an account by `account_id`, then clean every dangling reference:
/// promote/clear the host default if it pointed here, and drop any repo
/// overrides pointing here. Idempotent.
pub fn remove_forge_account(s: &mut Settings, account_id: &str) {
    let removed = s.forge_accounts.iter().find(|r| r.account_id == account_id).cloned();
    s.forge_accounts.retain(|r| r.account_id != account_id);
    s.repo_forge_overrides
        .retain(|o| o.account_id != account_id);
    if let Some(rec) = removed {
        // If this was the host default, promote another account on that host or
        // clear the default entirely.
        let was_default = s
            .forge_host_defaults
            .iter()
            .any(|d| d.host == rec.host && d.account_id == account_id);
        if was_default {
            s.forge_host_defaults.retain(|d| d.host != rec.host);
            if let Some(next) = s.forge_accounts.iter().find(|r| r.host == rec.host) {
                s.forge_host_defaults.push(ForgeHostDefault {
                    host: rec.host.clone(),
                    account_id: next.account_id.clone(),
                });
            }
        }
    }
}

/// P80: set/replace the default account for `host` (keyed by lowercased host).
pub fn set_host_default(s: &mut Settings, host: &str, account_id: &str) {
    let host = host.to_ascii_lowercase();
    if host.is_empty() {
        return;
    }
    s.forge_host_defaults.retain(|d| d.host != host);
    s.forge_host_defaults.push(ForgeHostDefault {
        host,
        account_id: account_id.to_string(),
    });
}

/// P80: clear the default account for `host`. No-op when absent.
pub fn clear_host_default(s: &mut Settings, host: &str) {
    let host = host.to_ascii_lowercase();
    s.forge_host_defaults.retain(|d| d.host != host);
}

/// P80: pin `account_id` as `repo_path`'s override (deduped via
/// [`crate::commands::same_repo_path`], so the same repo never gets two pins).
pub fn set_repo_override(s: &mut Settings, repo_path: &str, account_id: &str) {
    s.repo_forge_overrides
        .retain(|o| !crate::commands::same_repo_path(&o.repo_path, repo_path));
    s.repo_forge_overrides.insert(
        0,
        RepoForgeOverride {
            repo_path: repo_path.to_string(),
            account_id: account_id.to_string(),
        },
    );
}

/// P80: clear `repo_path`'s override (inherit again). No-op when absent.
pub fn clear_repo_override(s: &mut Settings, repo_path: &str) {
    s.repo_forge_overrides
        .retain(|o| !crate::commands::same_repo_path(&o.repo_path, repo_path));
}

/// P80 lazy P79→P80 migration (§3): for every legacy `forge_hosts` record whose
/// host is not yet represented in `forge_accounts`, add an account whose
/// `keychain_key` is the BARE host (so the existing token is found with zero
/// re-auth) and make it that host's default. Pure/in-memory; returns `true` if
/// it changed anything (for `update_if`). Idempotent.
pub fn migrate_forge_hosts_to_accounts(s: &mut Settings) -> bool {
    let mut changed = false;
    for h in s.forge_hosts.clone() {
        if s.forge_accounts.iter().any(|a| a.host == h.host) {
            continue; // already migrated
        }
        let aid = account_id(h.kind, &h.host, h.login.as_deref());
        s.forge_accounts.push(ForgeAccountRecord {
            account_id: aid.clone(),
            keychain_key: h.host.clone(), // legacy token lives under the bare host key
            host: h.host.clone(),
            kind: h.kind,
            login: h.login.clone(),
            avatar_url: None,
        });
        if !s.forge_host_defaults.iter().any(|d| d.host == h.host) {
            s.forge_host_defaults.push(ForgeHostDefault {
                host: h.host.clone(),
                account_id: aid,
            });
        }
        changed = true;
    }
    changed
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::forge_hosts::ForgeHostRecord;
    use bonsai_forge::ForgeKind;

    fn base() -> Settings {
        Settings::default()
    }

    #[test]
    fn account_id_is_lowercased_three_or_two_part() {
        assert_eq!(
            account_id(ForgeKind::GitHub, "GitHub.com", Some("OctoCat")),
            "gitHub:github.com:octocat"
        );
        assert_eq!(
            account_id(ForgeKind::GitHub, "github.com", None),
            "gitHub:github.com"
        );
    }

    #[test]
    fn migration_creates_default_account_from_legacy_host() {
        let mut s = base();
        s.forge_hosts.push(ForgeHostRecord {
            host: "github.com".into(),
            kind: ForgeKind::GitHub,
            login: Some("octocat".into()),
        });
        assert!(migrate_forge_hosts_to_accounts(&mut s));
        assert_eq!(s.forge_accounts.len(), 1);
        let a = &s.forge_accounts[0];
        assert_eq!(a.account_id, "gitHub:github.com:octocat");
        // Zero keychain rewrite: token stays under the bare host key.
        assert_eq!(a.keychain_key, "github.com");
        assert_eq!(s.forge_host_defaults.len(), 1);
        assert_eq!(s.forge_host_defaults[0].host, "github.com");
        assert_eq!(s.forge_host_defaults[0].account_id, a.account_id);
    }

    #[test]
    fn migration_is_idempotent() {
        let mut s = base();
        s.forge_hosts.push(ForgeHostRecord {
            host: "github.com".into(),
            kind: ForgeKind::GitHub,
            login: None,
        });
        assert!(migrate_forge_hosts_to_accounts(&mut s));
        // Second run: nothing changes.
        assert!(!migrate_forge_hosts_to_accounts(&mut s));
        assert_eq!(s.forge_accounts.len(), 1);
        assert_eq!(s.forge_host_defaults.len(), 1);
    }

    #[test]
    fn remove_account_promotes_or_clears_default() {
        let mut s = base();
        upsert_forge_account(
            &mut s,
            ForgeAccountRecord {
                account_id: "a".into(),
                keychain_key: "a".into(),
                host: "github.com".into(),
                kind: ForgeKind::GitHub,
                login: Some("a".into()),
                avatar_url: None,
            },
        );
        upsert_forge_account(
            &mut s,
            ForgeAccountRecord {
                account_id: "b".into(),
                keychain_key: "b".into(),
                host: "github.com".into(),
                kind: ForgeKind::GitHub,
                login: Some("b".into()),
                avatar_url: None,
            },
        );
        set_host_default(&mut s, "github.com", "a");
        set_repo_override(&mut s, "/tmp/repo", "a");
        remove_forge_account(&mut s, "a");
        // "a" gone, its override cleaned, default promoted to the other account.
        assert!(!s.forge_accounts.iter().any(|r| r.account_id == "a"));
        assert!(s.repo_forge_overrides.is_empty());
        assert_eq!(s.forge_host_defaults.len(), 1);
        assert_eq!(s.forge_host_defaults[0].account_id, "b");
        // Remove the last one → default cleared, not left dangling.
        remove_forge_account(&mut s, "b");
        assert!(s.forge_host_defaults.is_empty());
    }

    /// P80 §7.6: after populating accounts / host-defaults / overrides / the
    /// legacy mirror, the serialized settings.json must never contain a token —
    /// tokens live ONLY in the OS keychain. This locks the invariant against a
    /// field ever accidentally carrying a PAT (e.g. keychainKey holds the key
    /// name, never the secret).
    #[test]
    fn no_token_in_serialized_settings() {
        const SENTINEL_TOKEN: &str = "ghp_supersecretPAT0000000000000000000000";
        let mut s = base();
        // A migrated legacy account (keychainKey == bare host) …
        s.forge_hosts.push(ForgeHostRecord {
            host: "github.com".into(),
            kind: ForgeKind::GitHub,
            login: Some("octocat".into()),
        });
        assert!(migrate_forge_hosts_to_accounts(&mut s));
        // … plus a P80 three-part account, a host default, and a repo override.
        upsert_forge_account(
            &mut s,
            ForgeAccountRecord {
                account_id: "gitHub:github.com:danpercic86".into(),
                keychain_key: "gitHub:github.com:danpercic86".into(),
                host: "github.com".into(),
                kind: ForgeKind::GitHub,
                login: Some("danpercic86".into()),
                avatar_url: Some("https://example.com/a.png".into()),
            },
        );
        set_host_default(&mut s, "github.com", "gitHub:github.com:danpercic86");
        set_repo_override(&mut s, "/tmp/repo", "gitHub:github.com:danpercic86");

        let json = serde_json::to_string_pretty(&s).expect("settings serialize");
        assert!(
            !json.contains(SENTINEL_TOKEN),
            "serialized settings must never contain a forge PAT"
        );
        assert!(
            !json.contains("ghp_"),
            "settings.json must never contain a GitHub PAT prefix; got: {json}"
        );
        // No forge-account struct exposes a token-named field (mcpToken is an
        // unrelated pre-existing MCP field, deliberately excluded).
        assert!(
            !json.contains("forgeToken") && !json.contains("\"token\""),
            "forge account records must expose no token field; got: {json}"
        );
        // Sanity: the non-secret identity fields ARE persisted.
        assert!(json.contains("keychainKey"));
        assert!(json.contains("gitHub:github.com:danpercic86"));
    }
}
