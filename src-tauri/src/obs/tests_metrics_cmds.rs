//! Drift guard for the `cmd.<name>` allow-list (audit F3).
//!
//! `KNOWN_CMDS` is generated from the `IpcApi` interface declarations. If it
//! drifts, the failure mode is silent — a new command's durations simply never
//! appear in `usage.json` — so this test re-derives the set from the `.ts`
//! sources at test time and asserts an exact bijection, in both directions.
//!
//! Test code only: reads sources via `CARGO_MANIFEST_DIR`, touches no app code.

use std::collections::BTreeSet;
use std::path::PathBuf;

use super::{is_known_cmd, KNOWN_CMDS};

/// The three files whose interfaces make up `IpcApi` (it extends the other two).
const IPC_API_FILES: [&str; 3] = [
    "src/ipc/types/ipc-api.ts",
    "src/ipc/types/ipc-api-forge.ts",
    "src/ipc/types/ipc-api-obs.ts",
];

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("src-tauri has a parent")
        .to_path_buf()
}

/// Method names declared in a TS interface body: a line indented exactly two
/// spaces whose first token is a `lowerCamelCase` identifier followed by `(`.
/// Doc comments (`  /** … */`) and nested members (indented further) never match.
fn methods_in(source: &str) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for line in source.lines() {
        let Some(rest) = line.strip_prefix("  ") else {
            continue;
        };
        if rest.starts_with(' ') {
            continue;
        }
        if !rest.starts_with(|c: char| c.is_ascii_lowercase()) {
            continue;
        }
        let name: String = rest
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric())
            .collect();
        if name.is_empty() {
            continue;
        }
        if rest[name.len()..].starts_with('(') {
            out.insert(name);
        }
    }
    out
}

fn declared_methods() -> BTreeSet<String> {
    let root = repo_root();
    let mut all = BTreeSet::new();
    for rel in IPC_API_FILES {
        let path = root.join(rel);
        let src = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
        let found = methods_in(&src);
        assert!(!found.is_empty(), "no IpcApi methods parsed from {rel}");
        all.extend(found);
    }
    all
}

#[test]
fn allow_list_matches_the_ipc_api_interfaces_exactly() {
    let declared = declared_methods();
    let listed: BTreeSet<String> = KNOWN_CMDS.iter().map(|s| (*s).to_string()).collect();

    let missing: Vec<&String> = declared.difference(&listed).collect();
    let extra: Vec<&String> = listed.difference(&declared).collect();
    assert!(
        missing.is_empty() && extra.is_empty(),
        "obs/metrics_cmds.rs has drifted from the IpcApi interfaces.\n\
         missing (add): {missing:?}\nstale (remove): {extra:?}"
    );
}

/// `is_known_cmd` binary-searches, which is only correct on a sorted table.
#[test]
fn the_table_is_sorted_and_deduplicated() {
    let mut sorted = KNOWN_CMDS.to_vec();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(sorted.as_slice(), KNOWN_CMDS.as_slice());
}

#[test]
fn membership_accepts_real_names_and_rejects_everything_else() {
    for name in ["openRepo", "getStatus", "logAppend"] {
        assert!(is_known_cmd(name), "{name} is an IpcApi method");
    }
    for name in [
        "",
        "notACommand",
        "openrepo",
        "open_repo",
        "feature/RED-42",
        "C:/Users/dan/secret-repo",
    ] {
        assert!(!is_known_cmd(name), "{name} must not be a metric key");
    }
}
