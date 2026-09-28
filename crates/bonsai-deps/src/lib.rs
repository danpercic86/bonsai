//! Bonsai dependency tracker (P125; `docs/contracts/P125-dependency-tracker.md`).
//!
//! Two invariants (contract header):
//! 1. No byte of a dependency that `privacy::classify` did not return as
//!    `Visibility::Public` can reach a socket (§7.3).
//! 2. This crate never spawns a process: the `clippy.toml` ban is denied
//!    crate-wide below, and `tests/no_process.rs` scans the sources (§7.4).
//!
//! P125a shipped the model, privacy classification, the `RegistryContext`
//! type, the version schemes and the OSV newtypes. P125b adds safe reads,
//! discovery, the bounded YAML reader, the adapter surface, the npm family
//! and the npm/pnpm/yarn registry-context loader.
#![forbid(unsafe_code)]
#![deny(clippy::disallowed_types, clippy::disallowed_methods)]

pub mod adapter;
mod catalog;
pub mod discover;
pub mod eco;
pub mod fsread;
pub mod model;
pub mod privacy;
pub mod registry_ctx;
pub mod remote;
pub mod version;
mod yaml_events;

#[cfg(test)]
mod fixtures;
