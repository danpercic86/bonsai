//! Outbound HTTP (contract §7). P125a: only the `osv` newtype skeleton. The
//! transport, `OutboundRequest` (constructors `pub(in crate::remote)`) and
//! the deps.dev builders land in P125d.
//!
//! Nothing under `remote/` may name adapter types; `tests/no_process.rs`
//! enforces it (§7.3 item 4).

pub mod osv;
