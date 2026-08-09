//! T2 Area 8 — adversarial stdio frame corpus for the `bonsai-mcp` server.
//!
//! Every test drives the *built* server binary over stdio (via
//! [`common::McpClient`]) and asserts the server survives hostile / malformed
//! input: it never panics, never wedges, and continues answering well-formed
//! requests afterwards. The survival witness is a follow-up `tools/list`
//! (`assert_alive`) returning the tool set.
//!
//! Corpus (contract §2 adversarial mandate + Area 8 test plan):
//! garbage-then-valid, bare JSON array / string, wrong `jsonrpc` version,
//! unknown method, unknown tool, `tools/call` with wrong param types / a missing
//! field (→ invalid-params, not death), a ~5 MB frame, CRLF-terminated frames,
//! EOF mid-line (→ clean exit), and read tools on an empty / unborn-HEAD repo.

mod common;

use common::{is_well_formed_reply, McpClient};
use serde_json::{json, Value};
use std::time::Duration;

/// Non-JSON noise interleaved with a valid request: the noise is dropped
/// silently (rmcp ignores unparsable input) and the valid request still
/// resolves.
#[test]
fn garbage_then_valid_line_survives() {
    if common::skip_if_no_git() {
        return;
    }
    let repo = common::init_repo();
    common::build_linear(repo.path(), 2);
    let mut c = McpClient::connect(repo.path(), false);

    c.write_line("this is not json at all");
    c.write_line("}{ broken braces ][");
    c.write_line("\0\x01\x02 binary noise");
    // A well-formed request after the noise must still get a reply.
    c.assert_alive();
}

/// Structurally-valid JSON that is NOT a JSON-RPC request object (array, bare
/// string, number). rmcp answers wrong-shape JSON with an invalid_request error
/// (or ignores it) but never dies.
#[test]
fn wrong_shape_json_frames_survive() {
    if common::skip_if_no_git() {
        return;
    }
    let repo = common::init_repo();
    common::build_linear(repo.path(), 1);
    let mut c = McpClient::connect(repo.path(), false);

    c.write_line("[1, 2, 3]");
    c.write_line("\"just a bare string\"");
    c.write_line("42");
    c.write_line("true");
    c.write_line("null");
    c.assert_alive();
}

/// A request carrying the wrong `jsonrpc` protocol version must not crash the
/// server; a subsequent valid request still resolves.
#[test]
fn wrong_jsonrpc_version_survives() {
    if common::skip_if_no_git() {
        return;
    }
    let repo = common::init_repo();
    common::build_linear(repo.path(), 1);
    let mut c = McpClient::connect(repo.path(), false);

    c.write_line(r#"{"jsonrpc":"1.0","id":777,"method":"tools/list","params":{}}"#);
    c.write_line(r#"{"id":778,"method":"tools/list","params":{}}"#); // no jsonrpc field
    c.assert_alive();
}

/// An unknown JSON-RPC method returns a correlated error response (method not
/// found), not a crash.
#[test]
fn unknown_method_is_error_response_not_death() {
    if common::skip_if_no_git() {
        return;
    }
    let repo = common::init_repo();
    common::build_linear(repo.path(), 1);
    let mut c = McpClient::connect(repo.path(), false);

    let resp = c.request("bonsai/does_not_exist", json!({}));
    assert!(
        resp.get("error").is_some(),
        "unknown method must yield a JSON-RPC error, got: {resp}"
    );
    c.assert_alive();
}

/// `tools/call` on a non-existent tool is a JSON-RPC error (-32602 tool not
/// found), and the server keeps serving.
#[test]
fn unknown_tool_is_error_response_not_death() {
    if common::skip_if_no_git() {
        return;
    }
    let repo = common::init_repo();
    common::build_linear(repo.path(), 1);
    let mut c = McpClient::connect(repo.path(), false);

    let resp = c.call_tool("bonsai_totally_made_up", json!({}));
    let err = resp
        .get("error")
        .unwrap_or_else(|| panic!("unknown tool must be a JSON-RPC error, got: {resp}"));
    assert_eq!(
        err.get("code").and_then(Value::as_i64),
        Some(-32602),
        "expected INVALID_PARAMS (-32602 tool not found): {err}"
    );
    c.assert_alive();
}

/// `tools/call` with a wrong-typed argument and with a missing required field
/// each surface as an error (invalid params) — never a panic — and the server
/// survives both.
#[test]
fn tools_call_bad_params_are_invalid_params_not_death() {
    if common::skip_if_no_git() {
        return;
    }
    let repo = common::init_repo();
    common::build_linear(repo.path(), 2);
    let mut c = McpClient::connect(repo.path(), false);

    // `oid` must be a string; passing a number is a schema/type violation.
    let wrong_type = c.call_tool("bonsai_get_commit_diff", json!({ "oid": 12345 }));
    assert!(
        is_reply_error(&wrong_type),
        "wrong-typed arg must be an error (not ok, not a crash): {wrong_type}"
    );

    // Missing the required `oid` field entirely.
    let missing = c.call_tool("bonsai_get_commit_diff", json!({}));
    assert!(
        is_reply_error(&missing),
        "missing required field must be an error: {missing}"
    );

    // A wrong-typed arg on an array-param tool.
    let bad_paths = c.call_tool("bonsai_get_status", json!({ "unexpected": [1, 2] }));
    assert!(
        is_well_formed_reply(&bad_paths),
        "extra/unknown args must still yield a well-formed reply: {bad_paths}"
    );

    c.assert_alive();
}

/// A ~5 MB frame (a giant `oid` argument) is accepted, processed, and answered
/// with a correlated reply — the transport has no line-length cap and the server
/// does not choke.
#[test]
fn five_megabyte_frame_is_handled() {
    if common::skip_if_no_git() {
        return;
    }
    let repo = common::init_repo();
    common::build_linear(repo.path(), 2);
    let mut c = McpClient::connect(repo.path(), false);

    let huge = "a".repeat(5 * 1024 * 1024); // ~5 MiB
    let resp = c.call_tool("bonsai_get_commit_diff", json!({ "oid": huge }));
    assert!(
        is_well_formed_reply(&resp),
        "a ~5MB frame must get a correlated reply, got: {resp}"
    );
    // It is an error (a 5MB string is not a valid oid), not a success.
    assert!(is_reply_error(&resp), "giant oid must be rejected: {resp}");
    c.assert_alive();
}

/// CRLF (`\r\n`) frame terminators are tolerated — rmcp strips the trailing
/// carriage return before parsing.
#[test]
fn crlf_framed_requests_work() {
    if common::skip_if_no_git() {
        return;
    }
    let repo = common::init_repo();
    common::build_linear(repo.path(), 1);
    let mut c = McpClient::connect(repo.path(), false);

    let id = c.take_id();
    let msg = format!(r#"{{"jsonrpc":"2.0","id":{id},"method":"tools/list","params":{{}}}}"#);
    // Write the frame with an explicit CRLF terminator (no extra \n).
    c.write_raw(format!("{msg}\r\n").as_bytes());
    let resp = c.await_id(id);
    assert!(
        resp.get("result").is_some(),
        "CRLF-terminated tools/list must succeed: {resp}"
    );
}

/// A partial line followed by EOF (stdin closed) makes the server exit cleanly
/// rather than hang or panic on the incomplete frame.
#[test]
fn eof_mid_line_exits_cleanly() {
    if common::skip_if_no_git() {
        return;
    }
    let repo = common::init_repo();
    common::build_linear(repo.path(), 1);
    let mut c = McpClient::connect(repo.path(), false);

    // An incomplete JSON-RPC frame with NO terminating newline, then EOF.
    c.write_raw(br#"{"jsonrpc":"2.0","id":99,"method":"tools/li"#);
    c.close_stdin();
    assert!(
        c.wait_for_exit(Duration::from_secs(10)),
        "server must exit on stdin EOF with an incomplete trailing frame"
    );
}

/// Read tools on an empty / unborn-HEAD repo (initialised, zero commits) return
/// a well-formed reply instead of panicking on the missing HEAD.
#[test]
fn read_tools_on_unborn_head_repo_return() {
    if common::skip_if_no_git() {
        return;
    }
    // init_repo does NOT commit → HEAD is unborn.
    let repo = common::init_repo();
    let mut c = McpClient::connect(repo.path(), false);

    for tool in ["bonsai_get_graph", "bonsai_get_status", "bonsai_list_branches"] {
        let resp = c.call_tool(tool, json!({}));
        assert!(
            is_well_formed_reply(&resp),
            "{tool} on an unborn-HEAD repo must return a well-formed reply, got: {resp}"
        );
    }
    c.assert_alive();
}

// --------------------------------------------------------------------- helpers

/// A `tools/call` reply that represents *some* failure: either a JSON-RPC error
/// or a tool-domain error (`isError == true`). Neither is a panic/death.
fn is_reply_error(resp: &Value) -> bool {
    if resp.get("error").is_some() {
        return true;
    }
    resp.get("result")
        .and_then(|r| r.get("isError"))
        .and_then(Value::as_bool)
        .unwrap_or(false)
}
