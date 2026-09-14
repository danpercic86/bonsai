//! Declarative [`ToolEnv`] test double (the `external_fake.rs` precedent).
//!
//! Test-only. It exists so the Windows, macOS **and** Linux ladders all execute
//! on one machine — every rung is answered from a table, and nothing is ever
//! spawned or stat-ed. A `FakeToolEnv::new()` with nothing declared is the
//! "nothing exists" env every rung must degrade against (AC3).

use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use super::detect::ToolEnv;

/// Answers every probe from declared tables; records the registry keys asked
/// for so a test can pin the exact key text a rung builds.
pub(crate) struct FakeToolEnv {
    vars: BTreeMap<String, String>,
    /// Files that exist. Not executable unless also in `executables`.
    files: BTreeSet<PathBuf>,
    executables: BTreeSet<PathBuf>,
    /// Real bundles (directory + `Contents/Info.plist`). A directory that is
    /// merely NAMED `*.app` is modelled by leaving it out.
    bundles: BTreeSet<PathBuf>,
    path_hits: BTreeMap<String, PathBuf>,
    /// `"<key>|<value>"` ⇒ data.
    registry: BTreeMap<String, String>,
    home: Option<PathBuf>,
    /// How many registry reads this env will still answer — the fake's stand-in
    /// for `SCAN_REG_BUDGET` exhaustion.
    registry_budget: Cell<usize>,
    registry_calls: RefCell<Vec<(String, String)>>,
}

impl FakeToolEnv {
    /// An env in which nothing exists.
    pub(crate) fn new() -> FakeToolEnv {
        FakeToolEnv {
            vars: BTreeMap::new(),
            files: BTreeSet::new(),
            executables: BTreeSet::new(),
            bundles: BTreeSet::new(),
            path_hits: BTreeMap::new(),
            registry: BTreeMap::new(),
            home: None,
            registry_budget: Cell::new(usize::MAX),
            registry_calls: RefCell::new(Vec::new()),
        }
    }

    pub(crate) fn var(mut self, key: &str, value: &str) -> Self {
        self.vars.insert(key.to_string(), value.to_string());
        self
    }

    /// An existing file WITHOUT an execute bit.
    pub(crate) fn file(mut self, path: &str) -> Self {
        self.files.insert(PathBuf::from(path));
        self
    }

    /// An existing file WITH an execute bit.
    pub(crate) fn exe(mut self, path: &str) -> Self {
        self.files.insert(PathBuf::from(path));
        self.executables.insert(PathBuf::from(path));
        self
    }

    /// A real `.app` bundle (directory + `Contents/Info.plist`).
    pub(crate) fn bundle(mut self, path: &str) -> Self {
        self.bundles.insert(PathBuf::from(path));
        self
    }

    /// `program` resolves on `PATH` to `path` (which must also exist as a file
    /// for the rung to hit — declare it with [`Self::exe`] or [`Self::file`]).
    pub(crate) fn on_path(mut self, program: &str, path: &str) -> Self {
        self.path_hits
            .insert(program.to_string(), PathBuf::from(path));
        self
    }

    /// A registry value. `value` is `""` for a key's default value.
    pub(crate) fn registry(mut self, key: &str, value: &str, data: &str) -> Self {
        self.registry
            .insert(format!("{key}|{value}"), data.to_string());
        self
    }

    pub(crate) fn home(mut self, path: &str) -> Self {
        self.home = Some(PathBuf::from(path));
        self
    }

    /// Answer at most `n` registry reads, then behave like an exhausted budget.
    pub(crate) fn registry_budget(self, n: usize) -> Self {
        self.registry_budget.set(n);
        self
    }

    /// The `(key, value)` pairs [`ToolEnv::registry_string`] was called with,
    /// in order.
    pub(crate) fn registry_calls(&self) -> Vec<(String, String)> {
        self.registry_calls.borrow().clone()
    }
}

impl ToolEnv for FakeToolEnv {
    fn var(&self, key: &str) -> Option<String> {
        self.vars.get(key).cloned()
    }

    fn is_file(&self, p: &Path) -> bool {
        self.files.contains(p)
    }

    fn is_bundle(&self, p: &Path) -> bool {
        self.bundles.contains(p)
    }

    fn is_executable(&self, p: &Path) -> bool {
        self.executables.contains(p)
    }

    fn resolve_on_path(&self, program: &str) -> Option<PathBuf> {
        self.path_hits.get(program).cloned()
    }

    fn registry_string(&self, key: &str, value: &str) -> Option<String> {
        self.registry_calls
            .borrow_mut()
            .push((key.to_string(), value.to_string()));
        let left = self.registry_budget.get();
        if left == 0 {
            return None;
        }
        self.registry_budget.set(left.saturating_sub(1));
        self.registry.get(&format!("{key}|{value}")).cloned()
    }

    fn home(&self) -> Option<PathBuf> {
        self.home.clone()
    }
}
