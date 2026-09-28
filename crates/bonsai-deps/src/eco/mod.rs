//! Ecosystem adapters (contract §1, E2), one format family per file.

pub mod cargo;
mod cargo_lock;
pub mod go_mod;
pub mod npm;
mod npm_lock;
mod pnpm_lock;
mod yarn_lock;
