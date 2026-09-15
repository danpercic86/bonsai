pub mod ai;
pub mod assets;
pub mod error;
pub mod external;
/// The web-URL half of external launching (P72), split from [`external`].
pub mod external_url;
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
/// Detection of the external terminals/editors actually installed, replacing
/// user-supplied launch command templates (P112).
pub mod tools;
/// Windows PATH rehydration backstop for an installer-inherited environment (P71 R2).
pub mod winenv;
/// Pure text half of [`winenv`]: reg.exe parsing, `%VAR%` expansion, the merge.
pub mod winenv_merge;
