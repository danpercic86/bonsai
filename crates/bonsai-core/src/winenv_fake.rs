//! Table-backed [`WinEnv`] fake, shared by `winenv_tests.rs` and
//! `winenv_merge_tests.rs`.
//!
//! Test-only (`#[cfg(test)]` at the declaration site). It exists so that
//! **every** P71 assertion — including the `applied: true` branch — runs
//! against injected data on any host OS, mutating no process state: the `PATH`
//! write is captured in a [`RefCell`] instead of reaching `std::env::set_var`.
//!
//! It also RECORDS which process variables were consulted, so contract §5.3.1
//! ("profile variables must not come from the inherited environment") is an
//! assertion rather than a hope.

use std::cell::RefCell;
use std::collections::HashMap;

use super::{PathRehydration, WinEnv, PATH_VALUE, SYSTEM_PATH_KEY, USER_PATH_KEY, VOLATILE_ENV_KEY};

/// Fixed registry values + fixed process vars + recorded reads/writes.
#[derive(Default)]
pub(crate) struct FakeWinEnv {
    registry: HashMap<(String, String), String>,
    vars: HashMap<String, String>,
    /// Every name passed to [`WinEnv::var`], in order.
    var_reads: RefCell<Vec<String>>,
    /// Every value handed to [`WinEnv::set_path`], in order. Empty means the
    /// process `PATH` was never touched.
    writes: RefCell<Vec<String>>,
    /// When true, `set_path` reports failure — the production non-Windows
    /// no-op.
    refuse_writes: bool,
}

impl FakeWinEnv {
    pub(crate) fn with_registry(mut self, key: &str, value: &str, data: &str) -> Self {
        self.registry
            .insert((key.to_string(), value.to_string()), data.to_string());
        self
    }

    pub(crate) fn with_system_path(self, data: &str) -> Self {
        self.with_registry(SYSTEM_PATH_KEY, PATH_VALUE, data)
    }

    pub(crate) fn with_user_path(self, data: &str) -> Self {
        self.with_registry(USER_PATH_KEY, PATH_VALUE, data)
    }

    /// Seed one value under `HKCU\Volatile Environment` (the profile block).
    pub(crate) fn with_profile_var(self, name: &str, data: &str) -> Self {
        self.with_registry(VOLATILE_ENV_KEY, name, data)
    }

    pub(crate) fn with_var(mut self, k: &str, v: &str) -> Self {
        self.vars.insert(k.to_string(), v.to_string());
        self
    }

    pub(crate) fn refusing_writes(mut self) -> Self {
        self.refuse_writes = true;
        self
    }

    /// The values passed to `set_path`, if any.
    pub(crate) fn writes(&self) -> Vec<String> {
        self.writes.borrow().clone()
    }

    /// Was `name` ever looked up in the *process* environment?
    pub(crate) fn read_process_var(&self, name: &str) -> bool {
        self.var_reads
            .borrow()
            .iter()
            .any(|n| n.eq_ignore_ascii_case(name))
    }
}

impl WinEnv for FakeWinEnv {
    fn registry_string(&self, key: &str, value: &str) -> Option<String> {
        // Value names are case-insensitive in the registry, and `Path` vs
        // `PATH` differs between hives — mirror `parse_reg_query`.
        self.registry
            .iter()
            .find(|((k, v), _)| k == key && v.eq_ignore_ascii_case(value))
            .map(|(_, data)| data.clone())
    }

    fn var(&self, key: &str) -> Option<String> {
        self.var_reads.borrow_mut().push(key.to_string());
        self.vars.get(key).cloned()
    }

    fn set_path(&self, value: &str) -> bool {
        if self.refuse_writes {
            return false;
        }
        self.writes.borrow_mut().push(value.to_string());
        true
    }
}

/// Convenience for the apply-path tests: run [`super::rehydrate_path`] and
/// return both the outcome and what (if anything) was written.
pub(crate) fn rehydrate(env: &FakeWinEnv) -> (PathRehydration, Vec<String>) {
    let out = super::rehydrate_path(env);
    (out, env.writes())
}
