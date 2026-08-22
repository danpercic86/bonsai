//! Tauri command entry points, split by domain from the former monolithic
//! `commands.rs`. Each submodule keeps every `#[tauri::command]` next to its
//! `_inner` helper; `shared` holds the cross-cutting imports and `repo_path`.
//!
//! Every public item is re-exported here so the `commands::<fn>` paths
//! registered in `lib.rs`'s `generate_handler!` keep resolving unchanged.

mod shared;
mod debug;
mod repo;
mod ui_settings;
mod mcp;
mod status;
mod staging;
mod compose;
mod diff;
mod discard;
mod reset;
mod branches;
mod remotes;
mod merge;
mod hooks;
mod scheduler;
mod ai;
mod ai_stream;
mod rebase;
mod bisect;
mod history;
mod undo;
mod config;
mod stash;
mod cherrypick;
mod revert;
mod search;
mod signing;
mod submodules;
mod worktree;
mod health;
mod tags;
mod ai_assets;
mod profiles;
mod external;
mod forge;
mod forge_accounts;
mod git_env;

#[cfg(test)]
mod tests_support;

#[cfg(test)]
mod tests_open_repo_guards;

#[cfg(test)]
mod tests_branch_merge_guards;

#[cfg(test)]
mod tests_repo_isolation;

#[cfg(test)]
mod tests_ui_settings_patch;

#[cfg(test)]
mod tests_ui_settings_patch_flags;

#[cfg(test)]
mod tests_ai_consent_gate;

#[cfg(test)]
mod tests_staging;

#[cfg(test)]
mod tests_discard_reset_compose;

#[cfg(test)]
mod tests_branches_tags;

#[cfg(test)]
mod tests_merge_rebase;

#[cfg(test)]
mod tests_bisect_stash;

#[cfg(test)]
mod tests_diff_search_history;

#[cfg(test)]
mod tests_config_worktree_submodule;

#[cfg(test)]
mod tests_repo_session_misc;

#[cfg(test)]
mod tests_remotes;

#[cfg(test)]
mod tests_ai;

#[cfg(test)]
mod tests_ai_stream;

#[cfg(test)]
mod registration_tests;

// `shared` re-exports the cross-cutting imports and `repo_path`; only the test
// module reaches them through `commands::` (via `use super::*`), so gate the
// re-export to test builds to avoid an unused-import warning in normal builds.
#[cfg(test)]
pub(crate) use shared::*;
pub use debug::*;
pub use repo::*;
pub use ui_settings::*;
pub use mcp::*;
pub use status::*;
pub use staging::*;
pub use compose::*;
pub use diff::*;
pub use discard::*;
pub use reset::*;
pub use branches::*;
pub use remotes::*;
pub use merge::*;
pub use hooks::*;
pub use scheduler::*;
pub use ai::*;
pub use ai_stream::*;
pub use rebase::*;
pub use bisect::*;
pub use history::*;
pub use undo::*;
pub use config::*;
pub use stash::*;
pub use cherrypick::*;
pub use revert::*;
pub use search::*;
pub use signing::*;
pub use submodules::*;
pub use worktree::*;
pub use health::*;
pub use tags::*;
pub use ai_assets::*;
pub use profiles::*;
pub use external::*;
pub use forge::*;
pub use forge_accounts::*;
pub use git_env::*;
