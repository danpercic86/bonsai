//! Tauri command entry points, split by domain from the former monolithic
//! `commands.rs`. Each submodule keeps every `#[tauri::command]` next to its
//! `_inner` helper; `shared` holds the cross-cutting imports and `repo_path`.
//!
//! Every public item is re-exported here so the `commands::<fn>` paths
//! registered in `lib.rs`'s `generate_handler!` keep resolving unchanged.

mod activity;
mod ai;
mod ai_assets;
mod ai_stream;
mod bisect;
mod branches;
mod cherrypick;
mod compose;
mod config;
mod debug;
mod diff;
mod discard;
mod external;
mod health;
mod history;
mod hooks;
mod mcp;
mod merge;
mod profiles;
mod rebase;
mod remotes;
mod repo;
mod reset;
mod revert;
mod scheduler;
mod search;
mod shared;
mod signing;
mod staging;
mod stash;
mod status;
mod submodules;
mod tags;
mod ui_settings;
mod undo;
mod worktree;
// P112 §6: the external-tool picker surface. `tools_pick` is the testable
// validate-then-write half of `pick_external_tool`, split from the dialog.
mod forge;
mod forge_accounts;
mod tools;
mod tools_pick;
// DORMANT (audit INFO-2): the `#[tauri::command]` wrapper is gone (see the
// module doc) and nothing in the product reaches the core, so the module is
// compiled for tests only. Deliberately `cfg(test)` rather than a module-wide
// `#[allow(dead_code)]`: the blanket would also hide any FUTURE dead code in a
// credential-deleting module, and this keeps the dormant deletion path out of
// the shipped binary entirely while its 12 tests still cover the logic.
// To rewire: drop this `cfg`, restore the `#[tauri::command]` wrapper, and
// re-add the five plumbing sites the module doc lists.
#[cfg(test)]
mod forge_clear_host;
mod forge_remove_account;
mod git_env;
mod obs;
mod obs_delete;

#[cfg(test)]
mod tests_support;

#[cfg(test)]
mod tests_open_repo_guards;

#[cfg(test)]
mod tests_branch_merge_guards;

#[cfg(test)]
mod tests_repo_isolation;

#[cfg(test)]
mod tests_obs;

#[cfg(test)]
mod tests_ui_settings_patch;

#[cfg(test)]
mod tests_ui_settings_patch_flags;

#[cfg(test)]
mod tests_ui_settings_external_tools;

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
pub use activity::*;
pub use ai::*;
pub use ai_assets::*;
pub use ai_stream::*;
pub use bisect::*;
pub use branches::*;
pub use cherrypick::*;
pub use compose::*;
pub use config::*;
pub use debug::*;
pub use diff::*;
pub use discard::*;
pub use external::*;
pub use forge::*;
pub use forge_accounts::*;
pub use forge_remove_account::*;
pub use git_env::*;
pub use health::*;
pub use history::*;
pub use hooks::*;
pub use mcp::*;
pub use merge::*;
pub use obs::*;
pub use obs_delete::*;
pub use profiles::*;
pub use rebase::*;
pub use remotes::*;
pub use repo::*;
pub use reset::*;
pub use revert::*;
pub use scheduler::*;
pub use search::*;
#[cfg(test)]
pub(crate) use shared::*;
pub use signing::*;
pub use staging::*;
pub use stash::*;
pub use status::*;
pub use submodules::*;
pub use tags::*;
pub use tools::*;
pub use ui_settings::*;
pub use undo::*;
pub use worktree::*;
