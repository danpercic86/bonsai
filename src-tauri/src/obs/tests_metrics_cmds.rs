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

/// The three files whose interfaces make up `IpcApi` (it extends the other two),
/// each with a FLOOR on the member count.
///
/// The floor is what keeps this guard from going blind: if the parser below ever
/// stops recognising a declaration form, the bijection at the bottom still
/// passes (a member missing from BOTH sides cancels out), and the only symptom
/// is a smaller parse. Counts at the time of writing are 172 / 20 / 7; a file
/// that legitimately shrinks past its floor should have the floor lowered
/// deliberately, in the same commit that removes the commands.
const IPC_API_FILES: [(&str, usize); 3] = [
    ("src/ipc/types/ipc-api.ts", 150),
    ("src/ipc/types/ipc-api-forge.ts", 15),
    ("src/ipc/types/ipc-api-obs.ts", 5),
];

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("src-tauri has a parent")
        .to_path_buf()
}

/// Callable members declared in a TS interface body: a line indented exactly two
/// spaces whose first token is a `lowerCamelCase` identifier, in EITHER form —
///
/// * method shorthand — `foo(a: T): Promise<R>;`
/// * property with a function type — `foo: (a: T) => Promise<R>;`
///
/// Both forms are recorded by `ipcProxy`, so recognising only the shorthand
/// would let a property-style member be invisible to BOTH sides of the bijection
/// and silently never record `cmd.foo`. A non-callable property (`v: string;`)
/// is deliberately NOT matched — the `:` must be followed by `(`.
///
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
        let after = rest[name.len()..].trim_start();
        let callable = after.starts_with('(')
            || after
                .strip_prefix(':')
                .is_some_and(|t| t.trim_start().starts_with('('));
        if callable {
            out.insert(name);
        }
    }
    out
}

fn declared_methods() -> BTreeSet<String> {
    let root = repo_root();
    let mut all = BTreeSet::new();
    for (rel, floor) in IPC_API_FILES {
        let path = root.join(rel);
        let src = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
        let found = methods_in(&src);
        assert!(
            found.len() >= floor,
            "only {} IpcApi members parsed from {rel} (floor {floor}) — the parser              has most likely gone blind to a declaration form",
            found.len()
        );
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

/// Parser self-check — the half that used to be blind. A property-style member
/// must be parsed, so that adding one to `IpcApi` without adding it to
/// `KNOWN_CMDS` shows up as `missing` instead of cancelling out on both sides.
#[test]
fn methods_in_sees_both_declaration_forms() {
    let src = "export interface IpcApi {\n\
               \x20 /** doc */\n\
               \x20 openRepo(path: string): Promise<void>;\n\
               \x20 propStyle: (repoId: string) => Promise<void>;\n\
               \x20 spacedProp : (repoId: string) => Promise<void>;\n\
               \x20 notCallable: string;\n\
               \x20 Uppercase(): void;\n\
               \x20   nested(): void;\n\
               }\n";
    let found = methods_in(src);
    let names: Vec<&str> = found.iter().map(String::as_str).collect();
    assert_eq!(names, ["openRepo", "propStyle", "spacedProp"], "{found:?}");
}
