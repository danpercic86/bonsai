//! PAT storage (overview §F3): OS keychain of record + an in-process,
//! never-log token cache, mirroring `bonsai_core::git::cred_cache`.
//!
//! Key: service = the app identifier `com.bonsai.app`, account = the forge
//! HOST (e.g. `github.com`) so two repos on the same host share one token. The
//! token NEVER lands in settings.json, NEVER appears in a URL, and is NEVER
//! logged. The real keychain I/O is kept behind the [`Keychain`] seam so tests
//! exercise the cache logic with a fake backend and never touch the OS store.

use std::collections::HashMap;
use std::sync::{LazyLock, Mutex, MutexGuard};

use bonsai_core::error::AppError;

use crate::types::ForgeViewer;

/// Keychain service name = the Bonsai app identifier.
const KEYRING_SERVICE: &str = "com.bonsai.app";

/// The thin keychain I/O seam. Production = [`OsKeychain`]; tests inject a fake
/// so no unit test touches the real OS keychain. `get` returns `Ok(None)` when
/// there is simply no entry (not an error).
pub trait Keychain: Send + Sync {
    fn get(&self, host: &str) -> Result<Option<String>, AppError>;
    fn set(&self, host: &str, token: &str) -> Result<(), AppError>;
    fn delete(&self, host: &str) -> Result<(), AppError>;
}

/// Real OS keychain via the `keyring` crate. THIN — no caching here.
pub struct OsKeychain;

impl OsKeychain {
    fn entry(host: &str) -> Result<keyring::Entry, AppError> {
        keyring::Entry::new(KEYRING_SERVICE, host).map_err(map_keyring_err)
    }
}

impl Keychain for OsKeychain {
    fn get(&self, host: &str) -> Result<Option<String>, AppError> {
        match Self::entry(host)?.get_password() {
            Ok(token) => Ok(Some(token)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(map_keyring_err(e)),
        }
    }

    fn set(&self, host: &str, token: &str) -> Result<(), AppError> {
        Self::entry(host)?.set_password(token).map_err(map_keyring_err)
    }

    fn delete(&self, host: &str) -> Result<(), AppError> {
        match Self::entry(host)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(map_keyring_err(e)),
        }
    }
}

/// Map a keychain error to `AppError`. keyring errors never carry the token.
fn map_keyring_err(e: keyring::Error) -> AppError {
    AppError::Other(format!("keychain error: {e}"))
}

/// PAT store: a per-instance never-log cache over an injectable [`Keychain`].
/// Not `Debug` (would risk printing a token). Construct the process-wide store
/// with [`global`]; tests build one with a fake backend.
pub struct TokenStore {
    keychain: Box<dyn Keychain>,
    /// host -> token. Warmed lazily from the keychain; the source of the
    /// never-hit-the-keychain-per-call behavior.
    cache: Mutex<HashMap<String, String>>,
}

impl TokenStore {
    fn with_backend(keychain: Box<dyn Keychain>) -> Self {
        Self {
            keychain,
            cache: Mutex::new(HashMap::new()),
        }
    }

    /// Production store backed by the real OS keychain.
    pub fn new() -> Self {
        Self::with_backend(Box::new(OsKeychain))
    }

    /// Poison-recovering lock (a panic here must not wedge the auth path).
    fn lock(&self) -> MutexGuard<'_, HashMap<String, String>> {
        self.cache.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// Cache-first read. On a miss, load once from the keychain and warm the
    /// cache. `Ok(None)` = no token stored for `host`.
    pub fn get(&self, host: &str) -> Result<Option<String>, AppError> {
        let key = host.to_ascii_lowercase();
        if let Some(token) = self.lock().get(&key).cloned() {
            return Ok(Some(token));
        }
        match self.keychain.get(&key)? {
            Some(token) => {
                self.lock().insert(key, token.clone());
                Ok(Some(token))
            }
            None => Ok(None),
        }
    }

    /// Store `token` for `host` in the keychain and warm the cache. The
    /// keychain is written FIRST so a keychain failure does not leave a cached
    /// token with no store of record.
    pub fn set(&self, host: &str, token: &str) -> Result<(), AppError> {
        let key = host.to_ascii_lowercase();
        self.keychain.set(&key, token)?;
        self.lock().insert(key, token.to_string());
        Ok(())
    }

    /// Delete the token from the keychain and evict the cache. Idempotent.
    pub fn delete(&self, host: &str) -> Result<(), AppError> {
        let key = host.to_ascii_lowercase();
        self.lock().remove(&key);
        self.keychain.delete(&key)
    }

    /// Whether a token exists for `host` — NO network, keychain read only.
    /// A keychain read error degrades to `false` (the panel falls back to the
    /// connect flow rather than erroring on open).
    pub fn has(&self, host: &str) -> bool {
        matches!(self.get(host), Ok(Some(_)))
    }
}

impl Default for TokenStore {
    fn default() -> Self {
        Self::new()
    }
}

/// Process-wide store used by `open()` and the command layer.
static GLOBAL: LazyLock<TokenStore> = LazyLock::new(TokenStore::new);

/// The process-global [`TokenStore`].
pub fn global() -> &'static TokenStore {
    &GLOBAL
}

// ---- validated-viewer cache (populated by `viewer()` after set-token) ----
//
// A viewer is a public login + avatar (not secret), cached per host so
// `repo_context` can surface `viewer: Some(..)` once a token has been
// validated this process, without an extra network call.

static VIEWER_CACHE: LazyLock<Mutex<HashMap<String, ForgeViewer>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

fn viewer_lock() -> MutexGuard<'static, HashMap<String, ForgeViewer>> {
    VIEWER_CACHE.lock().unwrap_or_else(|p| p.into_inner())
}

/// Remember the validated viewer for `host`.
pub fn cache_viewer(host: &str, viewer: ForgeViewer) {
    viewer_lock().insert(host.to_ascii_lowercase(), viewer);
}

/// The cache-warm viewer for `host`, if a token was validated this process.
pub fn cached_viewer(host: &str) -> Option<ForgeViewer> {
    viewer_lock().get(&host.to_ascii_lowercase()).cloned()
}

/// Drop any cached viewer for `host` (paired with `delete` on sign-out).
pub fn evict_viewer(host: &str) {
    viewer_lock().remove(&host.to_ascii_lowercase());
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    /// In-memory fake keychain that counts backend reads so tests can prove the
    /// cache avoids repeat keychain hits. NEVER touches the OS keychain.
    #[derive(Default)]
    struct FakeKeychain {
        store: Mutex<HashMap<String, String>>,
        reads: Arc<AtomicUsize>,
    }

    impl Keychain for FakeKeychain {
        fn get(&self, host: &str) -> Result<Option<String>, AppError> {
            self.reads.fetch_add(1, Ordering::SeqCst);
            Ok(self
                .store
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .get(host)
                .cloned())
        }
        fn set(&self, host: &str, token: &str) -> Result<(), AppError> {
            self.store
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .insert(host.to_string(), token.to_string());
            Ok(())
        }
        fn delete(&self, host: &str) -> Result<(), AppError> {
            self.store
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .remove(host);
            Ok(())
        }
    }

    fn store_with_reads() -> (TokenStore, Arc<AtomicUsize>) {
        let reads = Arc::new(AtomicUsize::new(0));
        let backend = FakeKeychain {
            store: Mutex::new(HashMap::new()),
            reads: Arc::clone(&reads),
        };
        (TokenStore::with_backend(Box::new(backend)), reads)
    }

    #[test]
    fn set_then_get_returns_token() {
        let (s, _) = store_with_reads();
        assert_eq!(s.get("github.com").unwrap(), None);
        s.set("github.com", "tok-1").unwrap();
        assert_eq!(s.get("github.com").unwrap(), Some("tok-1".to_string()));
        assert!(s.has("github.com"));
    }

    #[test]
    fn get_is_cache_first_after_warm() {
        let (s, reads) = store_with_reads();
        // Miss: reads the (empty) backend once, caches nothing.
        assert_eq!(s.get("github.com").unwrap(), None);
        assert_eq!(reads.load(Ordering::SeqCst), 1);
        // set warms the cache without a read.
        s.set("github.com", "tok-1").unwrap();
        // Subsequent gets are served from cache — no further backend reads.
        for _ in 0..3 {
            assert_eq!(s.get("github.com").unwrap(), Some("tok-1".to_string()));
        }
        assert_eq!(reads.load(Ordering::SeqCst), 1, "cache-hit must not re-read");
    }

    #[test]
    fn cold_get_warms_from_backend_once() {
        let (s, reads) = store_with_reads();
        // Seed the backend directly (simulating a token stored in a prior run).
        s.keychain.set("github.com", "persisted").unwrap();
        assert_eq!(s.get("github.com").unwrap(), Some("persisted".to_string()));
        // A second get is a cache hit — backend read count stays at 1.
        assert_eq!(s.get("github.com").unwrap(), Some("persisted".to_string()));
        assert_eq!(reads.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn delete_evicts_cache_and_backend() {
        let (s, _) = store_with_reads();
        s.set("github.com", "tok-1").unwrap();
        assert!(s.has("github.com"));
        s.delete("github.com").unwrap();
        assert_eq!(s.get("github.com").unwrap(), None);
        assert!(!s.has("github.com"));
        // Idempotent: deleting a missing entry is Ok.
        s.delete("github.com").unwrap();
    }

    #[test]
    fn host_lookup_is_case_insensitive() {
        let (s, _) = store_with_reads();
        s.set("GitHub.com", "tok-1").unwrap();
        assert_eq!(s.get("github.com").unwrap(), Some("tok-1".to_string()));
    }
}
