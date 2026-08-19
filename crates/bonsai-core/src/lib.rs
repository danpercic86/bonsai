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
/// Windows PATH rehydration backstop for an installer-inherited environment (P71 R2).
pub mod winenv;
/// Pure text half of [`winenv`]: reg.exe parsing, `%VAR%` expansion, the merge.
pub mod winenv_merge;
