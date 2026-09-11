//! Recording [`CommandRunner`] fake, shared by `external_tests.rs` and
//! `external_url_tests.rs`.
//!
//! Test-only (`#[cfg(test)]` at the declaration site). It exists so every ladder
//! assertion — fallback order, "the template is the only candidate", "validation
//! runs before any spawn" — executes on any host OS while **never launching a
//! process**: `run` only records the program name and answers from a table.

use super::{CommandRunner, LaunchSpec};
use std::cell::RefCell;

/// Records every `run` call and succeeds only for the programs in `succeed`.
pub(crate) struct FakeRunner {
    succeed: Vec<String>,
    calls: RefCell<Vec<String>>,
}

impl FakeRunner {
    pub(crate) fn new(succeed: &[&str]) -> FakeRunner {
        FakeRunner {
            succeed: succeed.iter().map(|s| s.to_string()).collect(),
            calls: RefCell::new(Vec::new()),
        }
    }

    /// The programs `run` was called with, in order.
    pub(crate) fn calls(&self) -> Vec<String> {
        self.calls.borrow().clone()
    }
}

impl CommandRunner for FakeRunner {
    fn run(&self, spec: &LaunchSpec) -> Result<(), String> {
        self.calls.borrow_mut().push(spec.program.clone());
        if self.succeed.iter().any(|s| s == &spec.program) {
            Ok(())
        } else {
            Err(format!("mock: `{}` not found", spec.program))
        }
    }
}
