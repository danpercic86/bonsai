//! `bonsai_mcp` — the reusable tool layer of Bonsai's MCP server.
//!
//! The [`BonsaiServer`] (with its 34 `#[tool]` bodies), the [`WorkdirSource`]
//! abstraction, and the per-session repo-selection types ([`SessionRepos`],
//! [`OpenRepo`]) live here so that BOTH the standalone stdio binary
//! (`src/main.rs`, a `Fixed` workdir) AND the future embedded HTTP server in the
//! Tauri app (`Session`-based per-session selection) can share exactly the same
//! tool implementations.
//!
//! Transport wiring is intentionally NOT part of this crate: the stdio bin owns
//! its `rmcp::transport::stdio()` serve loop; the embedded HTTP server lives in
//! `src-tauri`. This crate stays transport-agnostic.

pub mod server;

pub use server::{BonsaiServer, OpenRepo, SessionRepos, WorkdirSource};
