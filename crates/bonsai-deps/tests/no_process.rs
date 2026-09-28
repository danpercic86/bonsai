//! AC11 / contract §7.4 `[MEDIUM-4]` and §7.3 item 4: a source scan backing
//! the clippy process ban.
//!
//! 1. No process-spawning token appears in the tracker's sources.
//! 2. Every `bonsai_core` path is on the allowlist (`use bonsai_core::{…}`
//!    groups are expanded first, aliases and globs included).
//! 3. Nothing under `src/remote/` names adapter types.
//!
//! The src-tauri roots arrive in P125e; a root that does not exist yet is
//! skipped, not failed.

use std::fs;
use std::path::{Path, PathBuf};

/// §7.4 banned tokens (plain substrings). `std::os::…process` is checked
/// separately by [`os_process_hits`].
const BANNED: &[&str] = &[
    "process::Command",
    "std::process",
    "Command::new",
    "tokio::process",
    "CommandRunner",
    "SpawnRunner",
    "libc::",
];

/// Allowed everywhere.
const CORE_ALLOW: &[&str] = &[
    "bonsai_core::error::AppError",
    "bonsai_core::git::history_index::store::repo_key",
];
/// Additionally allowed under `src-tauri/src/deps/` only.
const CORE_ALLOW_TAURI_DEPS: &[&str] = &["bonsai_core::git::repo::path_ignorecase"];

/// §7.3 item 4: tokens that must not appear under `src/remote/`.
const REMOTE_BANNED: &[&str] = &["adapter::", "Dependency", "LockEntry"];

fn crate_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn repo_root() -> PathBuf {
    crate_dir().join("..").join("..")
}

/// Every `.rs` file under `root` (iterative walk). A missing root yields none.
fn rust_files(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    if root.is_file() {
        out.push(root.to_path_buf());
        return out;
    }
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|e| e == "rs") {
                out.push(path);
            }
        }
    }
    out.sort();
    out
}

fn read(path: &Path) -> String {
    match fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => panic!("cannot read {}: {e}", path.display()),
    }
}

/// `std::os::<anything>::process` (e.g. `std::os::unix::process`).
fn os_process_hits(src: &str) -> Vec<String> {
    let mut hits = Vec::new();
    for (i, _) in src.match_indices("std::os::") {
        let tail: String = src[i..]
            .chars()
            .take_while(|c| c.is_alphanumeric() || *c == '_' || *c == ':')
            .collect();
        if tail.contains("process") {
            hits.push(tail);
        }
    }
    hits
}

fn banned_hits(src: &str) -> Vec<String> {
    let mut hits: Vec<String> = BANNED
        .iter()
        .filter(|t| src.contains(*t))
        .map(|t| t.to_string())
        .collect();
    hits.extend(os_process_hits(src));
    hits
}

fn is_ident(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Expands `prefix::{a, b::{c, d as e}, self, *}` into flat paths. Iterative
/// over a work list; `as` aliases are dropped (the aliased item is what is
/// checked).
fn expand_group(prefix: &str, body: &str, out: &mut Vec<String>) {
    let mut work = vec![(prefix.to_string(), body.to_string())];
    while let Some((pre, body)) = work.pop() {
        let mut depth = 0usize;
        let mut start = 0usize;
        let mut items = Vec::new();
        for (i, c) in body.char_indices() {
            match c {
                '{' => depth += 1,
                '}' => depth = depth.saturating_sub(1),
                ',' if depth == 0 => {
                    items.push(body[start..i].to_string());
                    start = i + 1;
                }
                _ => {}
            }
        }
        items.push(body[start..].to_string());
        for item in items {
            let item: String = item.split_whitespace().collect::<Vec<_>>().join(" ");
            let item = item.split(" as ").next().unwrap_or("").trim().to_string();
            if item.is_empty() {
                continue;
            }
            if let Some(open) = item.find('{') {
                let head = item[..open].trim().trim_end_matches("::");
                let inner = item[open + 1..].trim_end().trim_end_matches('}');
                let p = if head.is_empty() {
                    pre.clone()
                } else {
                    format!("{pre}::{head}")
                };
                work.push((p, inner.to_string()));
            } else if item == "self" {
                out.push(pre.clone());
            } else {
                out.push(format!("{pre}::{}", item.replace(' ', "")));
            }
        }
    }
}

/// Every `bonsai_core` path in `src`, groups expanded.
fn core_paths(src: &str) -> Vec<String> {
    let mut out = Vec::new();
    for (i, _) in src.match_indices("bonsai_core") {
        let before = src[..i].chars().next_back();
        let after = src[i + "bonsai_core".len()..].chars().next();
        if before.is_some_and(is_ident) || after.is_some_and(is_ident) {
            continue;
        }
        // Read the `a::b::c` path, then an optional `::{…}` group.
        let rest = &src[i..];
        let mut end = 0usize;
        for (j, c) in rest.char_indices() {
            if is_ident(c)
                || c == ':'
                || (c.is_whitespace() && rest[j..].trim_start().starts_with("::"))
            {
                end = j + c.len_utf8();
            } else {
                break;
            }
        }
        let path: String = rest[..end].split_whitespace().collect();
        let path = path.trim_end_matches(':').to_string();
        let after_path = rest[end..].trim_start();
        if rest[..end].trim_end().ends_with("::") && after_path.starts_with('{') {
            let mut depth = 0usize;
            let mut close = None;
            for (j, c) in after_path.char_indices() {
                match c {
                    '{' => depth += 1,
                    '}' => {
                        depth -= 1;
                        if depth == 0 {
                            close = Some(j);
                            break;
                        }
                    }
                    _ => {}
                }
            }
            let body = &after_path[1..close.unwrap_or(after_path.len())];
            expand_group(&path, body, &mut out);
        } else if after_path.starts_with('*') {
            out.push(format!("{path}::*"));
        } else {
            out.push(path);
        }
    }
    out
}

fn core_violations(src: &str, allow_tauri_deps: bool) -> Vec<String> {
    core_paths(src)
        .into_iter()
        .filter(|p| {
            let ok = CORE_ALLOW.contains(&p.as_str())
                || (allow_tauri_deps && CORE_ALLOW_TAURI_DEPS.contains(&p.as_str()));
            !ok
        })
        .collect()
}

struct Root {
    path: PathBuf,
    tauri_deps: bool,
}

fn roots() -> Vec<Root> {
    let tauri = repo_root().join("src-tauri").join("src");
    vec![
        Root {
            path: crate_dir().join("src"),
            tauri_deps: false,
        },
        Root {
            path: tauri.join("deps"),
            tauri_deps: true,
        },
        Root {
            path: tauri.join("commands").join("deps.rs"),
            tauri_deps: false,
        },
    ]
}

#[test]
fn tracker_sources_contain_no_process_tokens() {
    let mut failures = Vec::new();
    let mut scanned = 0usize;
    for root in roots() {
        for file in rust_files(&root.path) {
            scanned += 1;
            for hit in banned_hits(&read(&file)) {
                failures.push(format!("{}: {hit}", file.display()));
            }
        }
    }
    assert!(scanned > 0, "the scan found no sources at all");
    assert!(
        failures.is_empty(),
        "process tokens found:\n{}",
        failures.join("\n")
    );
}

#[test]
fn bonsai_core_imports_are_allowlisted() {
    let mut failures = Vec::new();
    for root in roots() {
        for file in rust_files(&root.path) {
            for p in core_violations(&read(&file), root.tauri_deps) {
                failures.push(format!("{}: {p}", file.display()));
            }
        }
    }
    assert!(
        failures.is_empty(),
        "non-allowlisted bonsai_core paths:\n{}",
        failures.join("\n")
    );
}

#[test]
fn remote_names_no_adapter_types() {
    let remote = crate_dir().join("src").join("remote");
    let files = rust_files(&remote);
    assert!(!files.is_empty(), "src/remote/ must exist");
    let mut failures = Vec::new();
    for file in files {
        let src = read(&file);
        for t in REMOTE_BANNED.iter().filter(|t| src.contains(*t)) {
            failures.push(format!("{}: {t}", file.display()));
        }
    }
    assert!(
        failures.is_empty(),
        "adapter types under remote/:\n{}",
        failures.join("\n")
    );
}

// ---- the scanner itself catches what it claims to -------------------------

#[test]
fn scanner_catches_process_tokens() {
    let cases = [
        "use std::process;",
        "let c = std::process::Command::new(\"x\");",
        "use tokio::process::Command;",
        "use std::os::windows::process::CommandExt;",
        "use std::os::unix::process::ExitStatusExt;",
        "unsafe { libc::fork() }",
        "struct R(CommandRunner);",
        "SpawnRunner::default()",
    ];
    for c in cases {
        assert!(!banned_hits(c).is_empty(), "{c}");
    }
    assert!(banned_hits("use std::os::raw::c_int; // no spawn").is_empty());
}

#[test]
fn scanner_expands_core_groups_and_aliases() {
    let ok = "use bonsai_core::error::AppError;\n\
              use bonsai_core::{error::AppError, git::history_index::store::repo_key as rk};\n\
              fn f() -> bonsai_core::error::AppError { todo!() }";
    assert!(
        core_violations(ok, false).is_empty(),
        "{:?}",
        core_paths(ok)
    );

    let grouped = "use bonsai_core::{error::AppError, git::{remote::push, repo}};";
    assert_eq!(
        core_violations(grouped, false),
        vec![
            "bonsai_core::git::remote::push".to_string(),
            "bonsai_core::git::repo".to_string()
        ]
    );
    let multi_line = "use bonsai_core::{\n    error::AppError,\n    tools::custom::run,\n};";
    assert_eq!(
        core_violations(multi_line, false),
        vec!["bonsai_core::tools::custom::run".to_string()]
    );
    for bad in [
        "use bonsai_core as core;",
        "use bonsai_core::*;",
        "use bonsai_core::error::{self};",
        "extern crate bonsai_core;",
        "let _ = ::bonsai_core::git::repo::open(p);",
    ] {
        assert!(!core_violations(bad, false).is_empty(), "{bad}");
    }
    let ign = "use bonsai_core::git::repo::path_ignorecase;";
    assert!(!core_violations(ign, false).is_empty());
    assert!(core_violations(ign, true).is_empty());
    assert!(core_paths("let bonsai_core_like = 1; my_bonsai_core::x();").is_empty());
}
