pub mod ai;
pub mod assets;
pub mod error;
pub mod external;
#[doc(hidden)]
pub mod fixture;
pub mod git;
/// Git-executable resolution + honest "git not found" diagnostics (P70).
pub mod gitbin;
pub mod graph;
pub mod health;
pub mod procutil;
#[cfg(test)]
pub mod testutil;
