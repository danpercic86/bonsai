//! T2 Area 1 (pass B) — diff / search / history / reflog / history-index /
//! ai-history command inners, runtime-free per the `tests.rs` pattern. Each
//! command gets ≥1 happy + ≥1 failure path via its `_inner` seam.

use super::tests_support::*;
use super::*;
use bonsai_core::git::search::{SearchField, SearchQuery};

fn block_on<F: std::future::Future>(f: F) -> F::Output {
    tauri::async_runtime::block_on(f)
}

fn query(text: &str, field: SearchField) -> SearchQuery {
    SearchQuery {
        text: text.to_string(),
        field,
        regex: false,
        case_sensitive: false,
        max_results: 0,
        scope_ref: None,
    }
}

mod diff;
mod search_history;
