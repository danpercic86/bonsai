//! Snapshots of everything a model READS from this server before it invokes
//! anything (audit 2026-09-11 PROCESS; widened by review 2026-09-11).
//!
//! `rmcp-macros` concatenates every `///` line of a tool handler into the
//! `description` field of its `tools/list` entry, and `schemars` derives the
//! JSON-Schema `description` of every parameter from the arg-struct doc
//! comments. Those doc comments are therefore not comments: they are the
//! instruction text a model reads before invoking worktree-destructive
//! operations. The audit found 222 lines of it shipping on subject-line trust,
//! and named "no test asserts on description text" as exactly the gap.
//!
//! Three checked-in fixtures close it, so ANY wording change shows up as a
//! reviewable diff in the same commit that causes it instead of hiding inside a
//! `docs(mcp)`-looking change:
//!
//! | fixture | what it pins |
//! |---|---|
//! | `fixtures/tool_descriptions.txt` | the tool-name set **with its gate**, the read/write counts, and every description byte |
//! | `fixtures/tool_schemas.txt` | each tool's parameter JSON-Schema — property names, types, requiredness and the per-parameter descriptions (review 2026-09-11: `render` used to read `tool.description` only) |
//! | `fixtures/server_instructions.txt` | `get_info().instructions` for every deployment shape — the string that carries the write-gate and hook claims, and the one that was WRONG about `bonsai_merge_branch` |
//!
//! Plus PROPERTY tests that do not depend on the fixtures at all, so
//! regenerating a snapshot can never bless a weakened safety claim.
//!
//! To update the fixtures after a DELIBERATE wording change:
//!
//! ```text
//! cargo test -p bonsai-mcp --lib -- --ignored regenerate_model_contract_snapshots
//! ```
//!
//! then read the resulting diff as part of the review.

use super::*;

/// Paths of the checked-in snapshots, relative to the crate root.
const DESCRIPTIONS_REL: &str = "src/server/fixtures/tool_descriptions.txt";
const SCHEMAS_REL: &str = "src/server/fixtures/tool_schemas.txt";
const INSTRUCTIONS_REL: &str = "src/server/fixtures/server_instructions.txt";

const DESCRIPTIONS: &str = include_str!("fixtures/tool_descriptions.txt");
const SCHEMAS: &str = include_str!("fixtures/tool_schemas.txt");
const INSTRUCTIONS: &str = include_str!("fixtures/server_instructions.txt");

/// One rendered tool. `gate` is `read` for the always-registered router and
/// `write` for the gated mutation router (including the stash tools, which have
/// their own generated router after the file-size split) — so the snapshot
/// records not just the text but which gate it is behind.
pub(super) struct Row {
    pub(super) gate: &'static str,
    pub(super) name: String,
    pub(super) description: String,
    /// The tool's `input_schema`, canonically rendered (keys sorted at every
    /// level, so the fixture cannot churn on serde_json map ordering).
    schema: String,
}

pub(super) fn rows() -> Vec<Row> {
    let mut out: Vec<Row> = Vec::new();
    let routers = [
        ("read", BonsaiServer::tool_router()),
        ("write", BonsaiServer::write_mutation_router()),
    ];
    for (gate, router) in routers {
        for tool in router.list_all() {
            let description = tool
                .description
                .as_deref()
                .unwrap_or("<no description>")
                // The macro emits `\n`-joined doc lines; normalize anyway so the
                // snapshot is identical on a CRLF checkout.
                .replace("\r\n", "\n");
            let schema = canonical_json(&serde_json::Value::Object(
                tool.input_schema.as_ref().clone(),
            ));
            out.push(Row {
                gate,
                name: tool.name.to_string(),
                description,
                schema,
            });
        }
    }
    out.sort_by(|a, b| (a.gate, &a.name).cmp(&(b.gate, &b.name)));
    out
}

/// Pretty JSON with object keys sorted at EVERY level. `serde_json`'s map is a
/// `BTreeMap` or an insertion-ordered map depending on the `preserve_order`
/// feature — which a transitive dependency can turn on — so the rendering is
/// made order-independent here rather than trusting either.
fn canonical_json(value: &serde_json::Value) -> String {
    fn sorted(value: &serde_json::Value) -> serde_json::Value {
        match value {
            serde_json::Value::Object(map) => {
                let mut keys: Vec<&String> = map.keys().collect();
                keys.sort();
                let mut out = serde_json::Map::new();
                for key in keys {
                    if let Some(v) = map.get(key) {
                        out.insert(key.clone(), sorted(v));
                    }
                }
                serde_json::Value::Object(out)
            }
            serde_json::Value::Array(items) => {
                serde_json::Value::Array(items.iter().map(sorted).collect())
            }
            other => other.clone(),
        }
    }
    serde_json::to_string_pretty(&sorted(value)).unwrap_or_else(|e| format!("<unrenderable: {e}>"))
}

/// Render the live descriptions: a count header, then one `--- <gate> <name>`
/// block per tool followed by its description verbatim.
fn render_descriptions() -> String {
    let rows = rows();
    let reads = rows.iter().filter(|r| r.gate == "read").count();
    let writes = rows.len() - reads;
    let mut out = format!("# {reads} read tools, {writes} write tools\n");
    for row in rows {
        out.push_str(&format!(
            "\n--- {} {}\n{}\n",
            row.gate, row.name, row.description
        ));
    }
    out
}

/// Render the live parameter schemas in the same block format.
fn render_schemas() -> String {
    let rows = rows();
    let mut out = format!("# {} tools, parameter schemas\n", rows.len());
    for row in rows {
        out.push_str(&format!(
            "\n--- {} {}\n{}\n",
            row.gate, row.name, row.schema
        ));
    }
    out
}

/// Every deployment shape whose `instructions` differ, keyed by a stable name.
/// The workdir path is never interpolated into the instructions, so this is
/// host-independent.
fn instruction_configs() -> Vec<(&'static str, BonsaiServer)> {
    let repo = std::path::PathBuf::from("/repo");
    let session = || Arc::new(SessionRepos::new(None, Box::new(Vec::new)));
    vec![
        (
            "standalone-readonly",
            BonsaiServer::new(repo.clone(), false, false),
        ),
        (
            "standalone-write",
            BonsaiServer::new(repo.clone(), true, false),
        ),
        (
            "standalone-write-allow-hooks",
            BonsaiServer::new(repo.clone(), true, true),
        ),
        (
            "standalone-readonly-allow-hooks",
            BonsaiServer::new(repo, false, true),
        ),
        (
            "embedded-readonly",
            BonsaiServer::with_session(session(), false),
        ),
        (
            "embedded-write",
            BonsaiServer::with_session(session(), true),
        ),
    ]
}

/// Render `get_info().instructions` for each deployment shape.
fn render_instructions() -> String {
    let configs = instruction_configs();
    let mut out = format!("# {} deployment shapes\n", configs.len());
    for (name, server) in configs {
        let text = server
            .get_info()
            .instructions
            .unwrap_or_else(|| "<none>".to_string())
            .replace("\r\n", "\n");
        out.push_str(&format!("\n--- {name}\n{text}\n"));
    }
    out
}

/// Parse a snapshot rendering back into `(header, [(key, body)])` so a mismatch
/// can be reported per block instead of as one 300-line blob.
fn parse(text: &str) -> (String, Vec<(String, String)>) {
    let normalized = text.replace("\r\n", "\n");
    let mut header = String::new();
    let mut blocks: Vec<(String, String)> = Vec::new();
    for (i, chunk) in normalized.split("\n--- ").enumerate() {
        if i == 0 {
            header = chunk.trim().to_string();
            continue;
        }
        match chunk.split_once('\n') {
            Some((key, body)) => blocks.push((key.trim().to_string(), body.to_string())),
            None => blocks.push((chunk.trim().to_string(), String::new())),
        }
    }
    (header, blocks)
}

/// Compare one live rendering against its checked-in fixture. A failure is NOT
/// necessarily a bug — it means the model-facing contract changed and must be
/// reviewed. The message names only the drifted blocks.
fn assert_snapshot(what: &str, rel: &str, snapshot: &str, live: &str) {
    let (want_header, want) = parse(snapshot);
    let (got_header, got) = parse(live);

    let want_keys: Vec<&str> = want.iter().map(|(k, _)| k.as_str()).collect();
    let got_keys: Vec<&str> = got.iter().map(|(k, _)| k.as_str()).collect();
    assert_eq!(
        got_keys, want_keys,
        "the {what} block set changed (added / removed / renamed / moved between gates). \
         Regenerate {rel} and review the diff."
    );
    assert_eq!(
        got_header, want_header,
        "the {what} header (counts) changed. Regenerate {rel} and review the diff."
    );

    let drifted: Vec<String> = got
        .iter()
        .zip(want.iter())
        .filter(|((_, g), (_, w))| g != w)
        .map(|((key, g), (_, w))| format!("--- {key}\n  live:     {g:?}\n  snapshot: {w:?}"))
        .collect();
    assert!(
        drifted.is_empty(),
        "{} {what} block(s) drifted from {rel}. This text is what a model reads before \
         invoking these tools, so review the change, then regenerate with:\n  \
         cargo test -p bonsai-mcp --lib -- --ignored regenerate_model_contract_snapshots\
         \n\n{}",
        drifted.len(),
        drifted.join("\n\n")
    );
}

#[test]
fn tool_descriptions_match_the_checked_in_snapshot() {
    assert_snapshot(
        "tool description",
        DESCRIPTIONS_REL,
        DESCRIPTIONS,
        &render_descriptions(),
    );
}

/// The parameter schemas are as model-facing as the descriptions: `schemars`
/// turns each arg-struct field's doc comment into that property's schema
/// `description`, and the property set / requiredness is the tool's actual
/// calling convention (review 2026-09-11 — previously unpinned).
#[test]
fn tool_parameter_schemas_match_the_checked_in_snapshot() {
    assert_snapshot("parameter schema", SCHEMAS_REL, SCHEMAS, &render_schemas());
}

/// `get_info().instructions` is read once per session, before any tool call,
/// and makes CLAIMS (what is enabled, what is refused, that content is
/// untrusted data). It is pinned for every deployment shape because the claims
/// differ per shape — and because the review found the hook sentence was false
/// for `bonsai_merge_branch` while no test looked at this string at all.
#[test]
fn server_instructions_match_the_checked_in_snapshot() {
    assert_snapshot(
        "server instructions",
        INSTRUCTIONS_REL,
        INSTRUCTIONS,
        &render_instructions(),
    );
}

/// Regeneration helper — NOT run by default (no gate tier runs ignored tests).
/// Writes the live renderings over the checked-in fixtures so a deliberate
/// wording change lands as a diff on tracked files:
///
/// ```text
/// cargo test -p bonsai-mcp --lib -- --ignored regenerate_model_contract_snapshots
/// ```
#[test]
#[ignore = "writes the snapshot fixtures; run explicitly after a deliberate wording change"]
fn regenerate_model_contract_snapshots() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    for (rel, text) in [
        (DESCRIPTIONS_REL, render_descriptions()),
        (SCHEMAS_REL, render_schemas()),
        (INSTRUCTIONS_REL, render_instructions()),
    ] {
        let path = root.join(rel);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("create fixtures dir");
        }
        std::fs::write(&path, text).expect("write snapshot");
        eprintln!("wrote {}", path.display());
    }
}

#[cfg(test)]
mod properties;
