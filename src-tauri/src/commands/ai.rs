//! `ai` commands — split from the former monolithic `commands.rs`.
//!
//! The command handlers live in focused sibling files, wired here as submodules
//! (via `#[path]`, so they stay flat `commands/*.rs` files) and re-exported so
//! every existing `commands::ai::*` path (and the parent's `pub use ai::*`) is
//! unchanged.

#[path = "ai_authoring.rs"]
mod authoring;
#[path = "ai_resolve.rs"]
mod resolve;
#[path = "ai_summarize.rs"]
mod summarize;

pub use authoring::*;
pub use resolve::*;
pub use summarize::*;
