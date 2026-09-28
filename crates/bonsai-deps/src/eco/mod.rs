//! Ecosystem adapters (contract §1, E2), one format family per file.

pub mod cargo;
mod cargo_lock;
pub mod go_mod;
pub mod npm;
mod npm_lock;
pub mod nuget;
mod nuget_lock;
mod nuget_proj;
mod pep508;
mod pnpm_lock;
pub mod pypi;
mod pypi_lock;
pub(crate) mod requirements;
mod yarn_lock;
