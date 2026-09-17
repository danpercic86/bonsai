//! Test-only helpers shared by the forge ACCOUNT command tests
//! (`forge_add_account_tests` and `forge_set_token_tests`).
//!
//! Extracted when `forge_set_token` grew the same `AddAccountDeps` seam: a
//! second copy of the call recorder is how two test suites start disagreeing
//! about what "the same deps" mean. The boxed closure types here are structural
//! matches for each command's `StoreTokenFn` / `DeleteTokenFn` /
//! `UpdateSettingsFn` aliases, so no production type has to be widened to
//! `pub(crate)` for the tests' benefit.

use std::path::Path;
use std::sync::{Arc, Mutex};

use super::shared::*;

/// Structural twins of each command's own `StoreTokenFn` / `DeleteTokenFn` /
/// `UpdateSettingsFn`. Declared here rather than widening the production
/// aliases to `pub(crate)` for the tests' benefit; a mismatch would simply not
/// compile at the `AddAccountDeps { .. }` literal.
type StoreTokenFn = Box<dyn Fn(&str, &str) -> Result<(), AppError> + Send + 'static>;
type DeleteTokenFn = Box<dyn Fn(&str) -> Result<(), AppError> + Send + 'static>;
type UpdateSettingsFn = Box<
    dyn Fn(&Path, &mut dyn FnMut(&mut settings::Settings)) -> Result<(), AppError> + Send + 'static,
>;

/// Scratch root under `D:\Data\Temp\bonsai-scratch` on Windows (MEMORY rule —
/// never C:, which is critically full).
#[cfg(windows)]
fn scratch_root() -> std::path::PathBuf {
    std::path::PathBuf::from("D:\\Data\\Temp\\bonsai-scratch")
}

#[cfg(not(windows))]
fn scratch_root() -> std::path::PathBuf {
    std::env::temp_dir().join("bonsai-scratch")
}

/// RAII scratch dir: `TempDir`'s `Drop` cleans up even when an assert panics.
pub(crate) fn scratch_dir(prefix: &str) -> tempfile::TempDir {
    let root = scratch_root();
    std::fs::create_dir_all(&root).expect("create scratch root");
    tempfile::Builder::new()
        .prefix(prefix)
        .tempdir_in(&root)
        .expect("scratch dir")
}

/// A recorder of every `store_token` / `delete_token` / settings-write call, in
/// ORDER, so tests can assert both WHICH keychain keys were touched and that
/// the settings write came first (audit MEDIUM-2 source B ordering).
#[derive(Default)]
pub(crate) struct Calls {
    log: Arc<Mutex<Vec<String>>>,
}

impl Calls {
    pub(crate) fn log(&self) -> Vec<String> {
        self.log.lock().unwrap_or_else(|p| p.into_inner()).clone()
    }

    fn push(log: &Arc<Mutex<Vec<String>>>, entry: String) {
        log.lock().unwrap_or_else(|p| p.into_inner()).push(entry);
    }

    /// A store that always succeeds, logging `store:<key>`.
    pub(crate) fn store_token(&self) -> StoreTokenFn {
        let log = Arc::clone(&self.log);
        Box::new(move |key, _token| {
            Self::push(&log, format!("store:{key}"));
            Ok(())
        })
    }

    /// A delete logging `delete:<key>`; `result = Some(msg)` makes it refuse.
    pub(crate) fn delete_token(&self, result: Option<&'static str>) -> DeleteTokenFn {
        let log = Arc::clone(&self.log);
        Box::new(move |key| {
            Self::push(&log, format!("delete:{key}"));
            match result {
                None => Ok(()),
                Some(msg) => Err(AppError::Other(msg.to_string())),
            }
        })
    }

    /// A settings write that records itself and then fails, so ordering against
    /// the deletes is observable.
    ///
    /// It DOES run `mutate` — against a scratch [`settings::Settings`] that is
    /// then dropped, so nothing reaches disk. Executing the caller's closure is
    /// the point: `forge_set_token_with` wraps this callback to carry its repo
    /// pin, and a fake that never calls `mutate` would leave that wrapper's
    /// failure path unexercised. It is NOT what discriminates a pin written
    /// outside the injected transaction — the tests' on-disk assertions
    /// (`override_for(&file) == None`) do that, plus the success test's
    /// single-`"settings"`-entry log assertion for a second, deps-routed write.
    pub(crate) fn failing_update(&self) -> UpdateSettingsFn {
        let log = Arc::clone(&self.log);
        Box::new(move |_file, mutate| {
            Self::push(&log, "settings".to_string());
            let mut scratch = settings::Settings::default();
            mutate(&mut scratch);
            Err(AppError::Other("io error: disk full".to_string()))
        })
    }

    /// The real transaction, plus an ordering marker.
    pub(crate) fn real_update(&self) -> UpdateSettingsFn {
        let log = Arc::clone(&self.log);
        Box::new(move |file, mutate| {
            Self::push(&log, "settings".to_string());
            settings::update(file, |s| mutate(s)).map(|_| ())
        })
    }
}
