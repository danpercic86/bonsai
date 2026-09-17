//! The ONE `#[ignore]`d real-keychain test for `forge_add_account` — separate
//! from `forge_add_account_tests.rs` because it is the only test here that
//! touches the developer's OS credential manager (and to keep both files under
//! the CLAUDE.md 500-line limit).
//!
//! It exists to close the security auditor's stated MEDIUM confidence on
//! MEDIUM-2 source B BY EXECUTION: that path was reasoned from
//! `upsert_forge_account` semantics and never run against a real keychain.

use super::*;

/// Scratch root under `D:\Data\Temp\bonsai-scratch` on Windows (MEMORY rule
/// — never C:, which is critically full).
#[cfg(windows)]
fn scratch_root() -> std::path::PathBuf {
    std::path::PathBuf::from(r"D:\Data\Temp\bonsai-scratch")
}

#[cfg(not(windows))]
fn scratch_root() -> std::path::PathBuf {
    std::env::temp_dir().join("bonsai-scratch")
}

/// RAII scratch dir: `TempDir`'s `Drop` cleans up even when an assert panics.
fn scratch_dir() -> tempfile::TempDir {
    let root = scratch_root();
    std::fs::create_dir_all(&root).expect("create scratch root");
    tempfile::Builder::new()
        .prefix("bonsai-add-acct-real-")
        .tempdir_in(&root)
        .expect("scratch dir")
}

/// Deletes its keychain keys BOTH at construction and on drop, so the test
/// self-heals. `Drop` alone is not enough: a `Drop` impl does not run when the
/// process is KILLED (Ctrl-C, a harness timeout, a debugger stop), and the
/// entries would then survive in the developer's real credential manager. They
/// are derived from a FIXED host for exactly this reason — a unique host would
/// make the survivors unfindable (`keyring` cannot enumerate, so no later run
/// could clean them and the user would have no name to search for), and
/// uniqueness buys nothing here: the test is serial, explicit and `#[ignore]`d.
struct KeyGuard(Vec<String>);

impl KeyGuard {
    /// Claim the keys, clearing any residue a killed earlier run left behind.
    fn claim(keys: Vec<String>) -> Self {
        let guard = Self(keys);
        guard.sweep();
        guard
    }

    fn sweep(&self) {
        for key in &self.0 {
            let _ = bonsai_forge::delete_token(key);
        }
    }
}

impl Drop for KeyGuard {
    fn drop(&mut self) {
        self.sweep();
    }
}

/// Closes the auditor's stated MEDIUM confidence on source B BY EXECUTION: the
/// legacy→re-add sequence was reasoned from `upsert_forge_account` semantics and
/// never run against a real keychain. This runs it end to end with
/// [`AddAccountDeps::default()`] — real `store_token`, real `delete_token`, real
/// `settings::update` — and asserts the bare-host entry is GONE afterwards.
///
/// SCOPE LIMIT (auditor, 2026-09-17): this seeds the BARE host and stores under
/// `aid` — two DIFFERENT keys — so it does NOT exercise the OVERWRITE semantics
/// the rollback discriminator rests on (`keyring::Entry::set_password`
/// replacing a value under the SAME key). Overwrite is source-verified
/// (`crates/bonsai-forge/src/auth.rs`), not executed. What this test executes
/// is source B: the superseded bare-host entry really does leave the keychain.
///
/// `#[ignore]` because it writes to the developer's OS credential manager. The
/// host is a throwaway `bonsai-test.invalid` (RFC 6761 reserved TLD),
/// deliberately never `github.com`-shaped, so it can never collide with a real
/// credential; [`KeyGuard`] removes both keys at test START as well as on drop,
/// so a rerun self-heals after a process kill.
///
/// Run it with:
/// `cargo test -p bonsai --lib real_keychain_legacy_readd_removes_the_bare_host_key -- --ignored --nocapture`
///
/// (its full path is
/// `commands::forge_add_account::real_keychain_tests::real_keychain_legacy_readd_removes_the_bare_host_key`).
#[test]
#[ignore = "writes to the developer's real OS keychain; run explicitly (see doc)"]
fn real_keychain_legacy_readd_removes_the_bare_host_key() {
    // FIXED, not unique: see `KeyGuard`. A killed run leaves entries under
    // these two names, and the next run deletes them before seeding.
    let host = "bonsai-test.invalid".to_string();
    let key = settings::account_id(ForgeKind::GitHub, &host, Some("octocat"));
    let _guard = KeyGuard::claim(vec![host.clone(), key.clone()]);

    let dir = scratch_dir();
    let file = dir.path().join("settings.json");
    // The P79 pre-state: a token under the BARE host, plus the legacy
    // `forge_hosts` record the migration reads.
    bonsai_forge::store_token(&host, "mock-legacy-token").expect("store legacy token");
    let mut s = settings::Settings::default();
    settings::upsert_forge_host(&mut s, &host, ForgeKind::GitHub, Some("octocat".into()));
    settings::save_to(&file, &s).expect("seed settings");
    // `load_from` migrates it to an account keyed by the bare host.
    let migrated = settings::load_from(&file);
    assert_eq!(migrated.forge_accounts.len(), 1);
    assert_eq!(migrated.forge_accounts[0].keychain_key, host);
    assert!(bonsai_forge::auth::global().has(&host));

    let r = tauri::async_runtime::block_on(forge_add_account_inner_with(
        &file,
        host.clone(),
        ForgeKind::GitHub,
        "mock-new-token".to_string(),
        ForgeViewer {
            login: "octocat".to_string(),
            avatar_url: None,
        },
        AddAccountDeps::default(),
    ));
    assert!(r.is_ok(), "the re-add must succeed: {:?}", r.err());

    let after = settings::load_from(&file);
    assert_eq!(after.forge_accounts.len(), 1, "still one account");
    assert_eq!(after.forge_accounts[0].keychain_key, key, "re-keyed to aid");
    assert!(
        bonsai_forge::auth::global().has(&key),
        "the new credential is reachable from the record"
    );
    // The point of the whole fix: no orphan left under the abandoned key.
    assert!(
        !bonsai_forge::auth::global().has(&host),
        "the superseded bare-host credential must be gone from the OS keychain"
    );
}
