//! T2 Area 8 — write-gate matrix + tool-count drift guard for `bonsai-mcp`.
//!
//! Two concerns:
//!
//! 1. **Write-gate matrix** — a read-only server (no `--allow-write`) must
//!    advertise NONE of the 20 mutation tools, and a `tools/call` on each is a
//!    JSON-RPC "tool not found" error with ZERO repo side effect (working-tree
//!    porcelain and the full ref set are byte-identical before and after the
//!    whole barrage).
//! 2. **Drift guard** (F-A8-b) — the live routers (`BonsaiServer::read_tool_*`
//!    / `write_tool_*`) are the single source of truth for the tool set, so the
//!    test catalogs AND a live `tools/list` must equal them. Adding/renaming a
//!    tool without updating everything fails here.

mod common;

use common::{McpClient, READ_TOOLS, WRITE_TOOLS};
use serde_json::{json, Value};

use bonsai_mcp::BonsaiServer;

/// Read-only server: every mutation tool is absent from `tools/list`, and a
/// direct `tools/call` on each is rejected (-32602) with no repo mutation.
#[test]
fn read_only_server_rejects_all_mutation_tools_without_side_effect() {
    if common::skip_if_no_git() {
        return;
    }
    let repo = common::init_repo();
    common::build_linear(repo.path(), 3);
    // Add ref + working-tree state so a stray mutation WOULD be observable.
    common::git(repo.path(), &["branch", "feature"]);
    common::write_file(repo.path(), "dirty.txt", "uncommitted\n");

    let before_porcelain = common::porcelain(repo.path());
    let before_refs = common::ref_snapshot(repo.path());
    assert!(
        !before_porcelain.is_empty(),
        "fixture must have observable working-tree state"
    );

    let mut c = McpClient::connect(repo.path(), false);

    // tools/list advertises exactly the read set, none of the write set.
    let names = c.list_tool_names();
    assert_eq!(
        names.len(),
        READ_TOOLS.len(),
        "read-only server must advertise exactly {} tools, got {names:?}",
        READ_TOOLS.len()
    );
    for t in WRITE_TOOLS {
        assert!(
            !names.contains(&t.to_string()),
            "read-only server must NOT advertise mutation tool {t}"
        );
    }

    // Each mutation tool, called directly, is a JSON-RPC "tool not found" error.
    for t in WRITE_TOOLS {
        let resp = c.call_tool(t, json!({}));
        let err = resp
            .get("error")
            .unwrap_or_else(|| panic!("mutation {t} on a read-only server must be a JSON-RPC error, got: {resp}"));
        assert_eq!(
            err.get("code").and_then(Value::as_i64),
            Some(-32602),
            "mutation {t} must be INVALID_PARAMS/-32602 (tool not found): {err}"
        );
    }
    drop(c);

    // No side effect: working tree and refs are byte-identical.
    assert_eq!(
        common::porcelain(repo.path()),
        before_porcelain,
        "read-only server must not have changed the working tree"
    );
    assert_eq!(
        common::ref_snapshot(repo.path()),
        before_refs,
        "read-only server must not have moved any ref"
    );
}

/// Drift guard: the live routers ARE the source of truth — the test catalogs and
/// the derived counts must match them exactly (names + lengths).
#[test]
fn tool_catalogs_match_live_routers() {
    let read = BonsaiServer::read_tool_names();
    let write = BonsaiServer::write_tool_names();

    let mut expected_read: Vec<String> = READ_TOOLS.iter().map(|s| s.to_string()).collect();
    expected_read.sort();
    let mut expected_write: Vec<String> = WRITE_TOOLS.iter().map(|s| s.to_string()).collect();
    expected_write.sort();

    assert_eq!(
        read, expected_read,
        "read-tool catalog drifted from the live read router"
    );
    assert_eq!(
        write, expected_write,
        "write-tool catalog drifted from the live write router"
    );

    // Counts derive from the same routers `src-tauri`'s McpStatus uses.
    assert_eq!(BonsaiServer::read_tool_count(), READ_TOOLS.len());
    assert_eq!(BonsaiServer::write_tool_count(), WRITE_TOOLS.len());
    assert_eq!(BonsaiServer::read_tool_count(), 14);
    assert_eq!(BonsaiServer::write_tool_count(), 20);

    // Read and write sets are disjoint (no tool registered on both routers).
    for w in &write {
        assert!(
            !read.contains(w),
            "tool {w} must not be in both the read and write routers"
        );
    }
}

/// The drift guard, end-to-end: a LIVE read-only server's `tools/list` equals
/// `read_tool_names()`, and a live write-enabled server's equals the union.
#[test]
fn live_tools_list_matches_router_names() {
    if common::skip_if_no_git() {
        return;
    }
    let repo = common::init_repo();
    common::build_linear(repo.path(), 1);

    let mut ro = McpClient::connect(repo.path(), false);
    assert_eq!(
        ro.list_tool_names(),
        BonsaiServer::read_tool_names(),
        "read-only tools/list must equal the live read router"
    );
    drop(ro);

    let mut union: Vec<String> = BonsaiServer::read_tool_names();
    union.extend(BonsaiServer::write_tool_names());
    union.sort();

    let mut rw = McpClient::connect(repo.path(), true);
    let mut live = rw.list_tool_names();
    live.sort();
    assert_eq!(
        live, union,
        "write-enabled tools/list must equal read ∪ write routers"
    );
}
