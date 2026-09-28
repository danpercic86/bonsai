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
//! and the npm/pnpm/yarn registry-context loader. P125c1 adds the Cargo and
//! Go adapters, their registry-context loaders (incl. the Q7 above-workdir
//! `.cargo/config{,.toml}` walk) and the TOML depth pre-scan. P125c2 adds
//! the PyPI and NuGet adapters, their loaders (pip / uv / Poetry configs;
//! the NuGet.Config chain incl. the Q7 above-workdir walk) and the bounded
//! XML reader. P125d adds the transport (`remote/`: reqwest over rustls +
//! ring, the recording `FakeTransport`), deps.dev + OSV, CVSS scoring,
//! `run_check` and the report cache.
#![forbid(unsafe_code)]
#![deny(clippy::disallowed_types, clippy::disallowed_methods)]

pub mod adapter;
pub mod cache;
mod catalog;
pub mod check;
pub mod cvss;
pub mod discover;
pub mod eco;
pub mod fsread;
pub mod model;
pub mod privacy;
pub mod registry_ctx;
pub mod remote;
mod toml_scan;
pub mod version;
mod xml_events;
mod yaml_events;

pub use check::{run_check, CancelToken, CheckCtx, CheckError, CHECK_DEADLINE, DEPSDEV_PARALLEL};
pub use remote::transport::{OutboundRequest, Transport};

#[cfg(test)]
mod fixtures;
#[cfg(test)]
mod test_pkg;
