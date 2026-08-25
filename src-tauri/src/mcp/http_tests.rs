//! Embedded-HTTP MCP integration tests (P16d contract §12.1 items 1-7). The
//! shared harness lives in `http_support`; this file carries the `require_git!`
//! gate and the seven item tests.

use std::sync::Mutex as StdMutex;

use reqwest::StatusCode;
use serde_json::{json, Value};

use super::http_support::*;
use super::*;

macro_rules! require_git {
    () => {
        if !have_git() {
            eprintln!("skipping: `git` CLI not found on PATH");
            return;
        }
    };
}

#[tokio::test(flavor = "multi_thread")]
async fn item1_security_token_origin_host_gate() {
    require_git!();
    let a = init_repo();
    let b = init_repo();
    build_linear(a.path(), 3);
    build_linear(b.path(), 4);
    let ra = open_repo(a.path());
    let rb = open_repo(b.path());

    let running = spawn(
        false,
        None,
        arc_list(vec![ra.clone(), rb.clone()]),
        seed_some(ra.repo_id.clone()),
    )
    .await;
    let port = running.port;
    let url = format!("http://127.0.0.1:{port}/mcp");
    let client = no_proxy_client();
    let valid_host = format!("127.0.0.1:{port}");

    // No Authorization header -> 401.
    assert_eq!(
        probe_status(&client, &url, None, None, None).await,
        StatusCode::UNAUTHORIZED,
        "missing bearer must be 401"
    );
    // Wrong bearer -> 401.
    assert_eq!(
        probe_status(&client, &url, Some("not-the-token"), None, None).await,
        StatusCode::UNAUTHORIZED,
        "wrong bearer must be 401"
    );
    // Correct bearer but an Origin header present -> 403 (D-3).
    assert_eq!(
        probe_status(&client, &url, Some(TEST_TOKEN), Some("http://evil.test"), None).await,
        StatusCode::FORBIDDEN,
        "any Origin header must be 403"
    );
    // Correct bearer but a disallowed Host -> 403.
    assert_eq!(
        probe_status(&client, &url, Some(TEST_TOKEN), None, Some("evil.test")).await,
        StatusCode::FORBIDDEN,
        "disallowed Host must be 403"
    );
    // Correct bearer + valid Host + no Origin -> reaches the MCP layer (200).
    let ok = probe_status(&client, &url, Some(TEST_TOKEN), None, Some(&valid_host)).await;
    assert!(
        ok != StatusCode::UNAUTHORIZED && ok != StatusCode::FORBIDDEN,
        "authenticated loopback request must pass the gate, got {ok}"
    );
    assert!(ok.is_success(), "authenticated initialize should be 2xx, got {ok}");

    running.stop();
}

// ============================================ item 2: read round-trip + count

#[tokio::test(flavor = "multi_thread")]
async fn item2_read_round_trip_and_tool_count() {
    require_git!();
    let a = init_repo();
    let b = init_repo();
    build_linear(a.path(), 3);
    build_linear(b.path(), 4);
    let ra = open_repo(a.path());
    let rb = open_repo(b.path());

    let running = spawn(
        false,
        None,
        arc_list(vec![ra.clone(), rb.clone()]),
        seed_some(ra.repo_id.clone()),
    )
    .await;

    let mut client = HttpMcp::connect(running.port, TEST_TOKEN).await;

    // Exactly the 14 read tools, no mutation tools.
    let names = client.list_tool_names().await;
    assert_eq!(
        names.len(),
        READ_TOOLS.len(),
        "read-only server must advertise exactly 14 tools, got: {names:?}"
    );
    for t in READ_TOOLS {
        assert!(names.contains(&t.to_string()), "missing read tool {t}");
        assert!(t.starts_with("bonsai_"), "read tool must be bonsai_-prefixed: {t}");
    }
    for t in WRITE_TOOLS {
        assert!(
            !names.contains(&t.to_string()),
            "read-only server must NOT advertise mutation tool {t}"
        );
    }

    // get_graph on the seeded repo (A) matches an in-process compute_graph.
    let layout = ok_structured(&client.call_tool("bonsai_get_graph", json!({})).await);
    let nodes = layout
        .get("nodes")
        .and_then(Value::as_array)
        .expect("layout.nodes");
    assert_eq!(
        nodes.len(),
        graph_node_count(a.path()),
        "HTTP get_graph nodes.len() must equal compute_graph(A)"
    );

    running.stop();
}

// ================================ item 3: write gating + conflict round-trip

#[tokio::test(flavor = "multi_thread")]
async fn item3_write_gating_and_conflict_round_trip() {
    require_git!();
    let repo = init_repo();
    build_conflict_fixture(repo.path());
    let r = open_repo(repo.path());

    let running = spawn(
        true,
        None,
        arc_list(vec![r.clone()]),
        seed_some(r.repo_id.clone()),
    )
    .await;

    let mut client = HttpMcp::connect(running.port, TEST_TOKEN).await;

    // allow_write=true -> 34 tools.
    let names = client.list_tool_names().await;
    assert_eq!(
        names.len(),
        READ_TOOLS.len() + WRITE_TOOLS.len(),
        "allow-write server must advertise 34 tools, got: {names:?}"
    );
    for t in READ_TOOLS.iter().chain(WRITE_TOOLS.iter()) {
        assert!(names.contains(&t.to_string()), "missing tool {t}");
    }

    // merge feature -> conflicts on a.txt.
    let merge = ok_structured(
        &client
            .call_tool("bonsai_merge_branch", json!({ "name": "feature" }))
            .await,
    );
    assert_eq!(
        merge.get("kind").and_then(Value::as_str),
        Some("conflicts"),
        "expected a conflicts MergeOutcome: {merge}"
    );
    let paths: Vec<String> = merge
        .get("paths")
        .and_then(Value::as_array)
        .expect("conflicts.paths")
        .iter()
        .map(|v| v.as_str().unwrap().to_string())
        .collect();
    assert!(paths.contains(&"a.txt".to_string()), "paths must include a.txt: {paths:?}");

    // get_conflict -> bothModified with the branch versions.
    let conflict = ok_structured(
        &client
            .call_tool("bonsai_get_conflict", json!({ "path": "a.txt" }))
            .await,
    );
    assert_eq!(
        conflict.get("kind").and_then(Value::as_str),
        Some("bothModified"),
        "conflict kind: {conflict}"
    );
    assert_eq!(conflict.get("ours").and_then(Value::as_str), Some(OURS_A));
    assert_eq!(conflict.get("theirs").and_then(Value::as_str), Some(THEIRS_A));

    // resolve + commit_merge.
    ok_structured(
        &client
            .call_tool(
                "bonsai_resolve_conflict_text",
                json!({ "path": "a.txt", "content": MERGED_A }),
            )
            .await,
    );
    let commit = ok_structured(
        &client
            .call_tool("bonsai_commit_merge", json!({ "message": "resolve" }))
            .await,
    );
    assert!(
        commit.get("oid").and_then(Value::as_str).is_some(),
        "commit_merge must return an oid: {commit}"
    );
    running.stop();

    let mcp_tree = git(repo.path(), &["rev-parse", "HEAD^{tree}"]);

    // Oracle: identical history hand-resolved by the git CLI.
    let cli = init_repo();
    build_conflict_fixture(cli.path());
    assert!(
        !git_ok(cli.path(), &["merge", "feature"]),
        "git merge feature should conflict"
    );
    write_file(cli.path(), "a.txt", MERGED_A);
    git(cli.path(), &["add", "a.txt"]);
    git(cli.path(), &["commit", "-m", "resolve"]);
    let cli_tree = git(cli.path(), &["rev-parse", "HEAD^{tree}"]);

    assert_eq!(
        mcp_tree, cli_tree,
        "MCP-resolved tree oid ({mcp_tree}) must equal the git-CLI tree oid ({cli_tree})"
    );
}

// =============================================== item 4: no selection (seed None)

#[tokio::test(flavor = "multi_thread")]
async fn item4_no_selection_is_no_repo_not_panic() {
    require_git!();
    let a = init_repo();
    let b = init_repo();
    build_linear(a.path(), 3);
    build_linear(b.path(), 4);
    let ra = open_repo(a.path());
    let rb = open_repo(b.path());

    // Seed None: this session has no selection.
    let running = spawn(false, None, arc_list(vec![ra, rb]), seed_none()).await;
    let mut client = HttpMcp::connect(running.port, TEST_TOKEN).await;

    let resp = client.call_tool("bonsai_get_status", json!({})).await;
    let err = err_structured(&resp);
    assert_eq!(
        err.get("kind").and_then(Value::as_str),
        Some("noRepo"),
        "no selection must surface kind=noRepo (no panic / no 500): {err}"
    );

    running.stop();
}

// ===================== item 5: list/select + acting on a non-seed repo (B)

#[tokio::test(flavor = "multi_thread")]
async fn item5_list_select_and_act_on_non_seed_repo() {
    require_git!();
    let a = init_repo();
    let b = init_repo();
    build_linear(a.path(), 3);
    build_linear(b.path(), 6); // distinct node count from A
    let ra = open_repo(a.path());
    let rb = open_repo(b.path());
    let a_id = ra.repo_id.clone();
    let b_id = rb.repo_id.clone();

    let count_a = graph_node_count(a.path());
    let count_b = graph_node_count(b.path());
    assert_ne!(count_a, count_b, "fixtures A and B must differ in node count");

    let running = spawn(
        false,
        None,
        arc_list(vec![ra, rb]),
        seed_some(a_id.clone()),
    )
    .await;
    let mut client = HttpMcp::connect(running.port, TEST_TOKEN).await;

    // list_repos: A and B, A marked selected (the seed).
    let list = ok_structured(&client.call_tool("bonsai_list_repos", json!({})).await);
    let arr = list.as_array().expect("list_repos returns an array");
    assert_eq!(arr.len(), 2, "list_repos must return both open tabs: {list}");
    let find = |id: &str| {
        arr.iter()
            .find(|e| e.get("repoId").and_then(Value::as_str) == Some(id))
            .unwrap_or_else(|| panic!("repoId {id} not in list: {list}"))
    };
    assert_eq!(
        find(&a_id).get("selected").and_then(Value::as_bool),
        Some(true),
        "seed repo A must be selected"
    );
    assert_eq!(
        find(&b_id).get("selected").and_then(Value::as_bool),
        Some(false),
        "non-seed repo B must not be selected"
    );

    // Seeded session acts on A.
    let g_a = ok_structured(&client.call_tool("bonsai_get_graph", json!({})).await);
    assert_eq!(
        g_a.get("nodes").and_then(Value::as_array).map(|n| n.len()),
        Some(count_a),
        "before select, get_graph reflects seed A"
    );

    // select B -> succeeds, returns B's summary (selected=true).
    let sel = ok_structured(
        &client
            .call_tool("bonsai_select_repo", json!({ "repoId": b_id }))
            .await,
    );
    assert_eq!(sel.get("repoId").and_then(Value::as_str), Some(b_id.as_str()));
    assert_eq!(sel.get("selected").and_then(Value::as_bool), Some(true));

    // Now get_graph reflects B (the non-focused/non-seed tab) — call-time resolution.
    let g_b = ok_structured(&client.call_tool("bonsai_get_graph", json!({})).await);
    assert_eq!(
        g_b.get("nodes").and_then(Value::as_array).map(|n| n.len()),
        Some(count_b),
        "after select(B), get_graph reflects B, independent of the seed"
    );

    running.stop();
}

// ============================= item 6: unknown/closed repoId rejection

#[tokio::test(flavor = "multi_thread")]
async fn item6_unknown_and_closed_repo_rejection() {
    require_git!();
    let a = init_repo();
    let b = init_repo();
    build_linear(a.path(), 3);
    build_linear(b.path(), 4);
    let ra = open_repo(a.path());
    let rb = open_repo(b.path());
    let a_id = ra.repo_id.clone();
    let b_id = rb.repo_id.clone();

    // list_open closes over a flag we can flip to "close" B mid-session.
    let include_b = Arc::new(StdMutex::new(true));
    let flag = Arc::clone(&include_b);
    let list_open: Arc<dyn Fn() -> Vec<OpenRepo> + Send + Sync> = Arc::new(move || {
        let mut v = vec![ra.clone()];
        if *flag.lock().unwrap() {
            v.push(rb.clone());
        }
        v
    });

    let running = spawn(false, None, list_open, seed_some(a_id.clone())).await;
    let mut client = HttpMcp::connect(running.port, TEST_TOKEN).await;

    // Unknown repoId -> invalidName.
    let unknown = err_structured(
        &client
            .call_tool("bonsai_select_repo", json!({ "repoId": "C:/not/open" }))
            .await,
    );
    assert_eq!(
        unknown.get("kind").and_then(Value::as_str),
        Some("invalidName"),
        "unknown repoId must be invalidName: {unknown}"
    );

    // Select B (currently open) -> ok.
    ok_structured(
        &client
            .call_tool("bonsai_select_repo", json!({ "repoId": b_id }))
            .await,
    );

    // Close B (stop returning it from list_open), then a git tool -> noRepo.
    *include_b.lock().unwrap() = false;
    let closed = err_structured(&client.call_tool("bonsai_get_status", json!({})).await);
    assert_eq!(
        closed.get("kind").and_then(Value::as_str),
        Some("noRepo"),
        "acting on a since-closed selection must be noRepo: {closed}"
    );

    running.stop();
}

// ==================== item 7 (reviewer NIT-2): write->off revoke on bounce

#[tokio::test(flavor = "multi_thread")]
async fn item7_write_off_bounce_renegotiates_read_only() {
    require_git!();
    let a = init_repo();
    build_linear(a.path(), 3);
    let ra = open_repo(a.path());
    let a_id = ra.repo_id.clone();

    // Start with write ON (34 tools).
    let running = spawn(
        true,
        None,
        arc_list(vec![ra.clone()]),
        seed_some(a_id.clone()),
    )
    .await;
    let port = running.port;
    {
        let mut client = HttpMcp::connect(port, TEST_TOKEN).await;
        assert_eq!(
            client.list_tool_names().await.len(),
            READ_TOOLS.len() + WRITE_TOOLS.len(),
            "write-on server must advertise 34 tools"
        );
    }

    // Simulate the write->off bounce: stop, then restart read-only on the SAME
    // port (as `set_allow_write` does).
    running.stop();
    let running2 = spawn(
        false,
        Some(port),
        arc_list(vec![ra]),
        seed_some(a_id),
    )
    .await;
    assert_eq!(running2.port, port, "bounce should re-bind the same port");

    // A NEW client session re-negotiates the now-14 tool set.
    let mut client2 = HttpMcp::connect(running2.port, TEST_TOKEN).await;
    let names = client2.list_tool_names().await;
    assert_eq!(
        names.len(),
        READ_TOOLS.len(),
        "after write->off bounce, a fresh session sees exactly 14 tools: {names:?}"
    );
    assert!(
        !names.contains(&"bonsai_create_branch".to_string()),
        "mutation tools must be gone after the bounce"
    );

    // A mutation call is now rejected as an unregistered tool (JSON-RPC error).
    let resp = client2
        .call_tool("bonsai_create_branch", json!({ "name": "x" }))
        .await;
    assert!(
        resp.get("error").is_some(),
        "mutation on a read-only (bounced) server must be a JSON-RPC error: {resp}"
    );

    running2.stop();
}
