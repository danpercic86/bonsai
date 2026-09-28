//! The no-process source scanner (helpers for `tests/no_process.rs`).

use std::fs;
use std::path::{Path, PathBuf};

/// §7.4 banned tokens (plain substrings). `std::os::…process` is checked
/// separately by [`os_process_hits`].
pub(crate) const BANNED: &[&str] = &[
    "process::Command",
    "std::process",
    "Command::new",
    "tokio::process",
    "CommandRunner",
    "SpawnRunner",
    "libc::",
];

/// Allowed everywhere.
pub(crate) const CORE_ALLOW: &[&str] = &[
    "bonsai_core::error::AppError",
    "bonsai_core::git::history_index::store::repo_key",
];
/// Additionally allowed under `src-tauri/src/deps/` only.
pub(crate) const CORE_ALLOW_TAURI_DEPS: &[&str] = &["bonsai_core::git::repo::path_ignorecase"];

/// §7.3 item 4: tokens that must not appear under `src/remote/`.
pub(crate) const REMOTE_BANNED: &[&str] = &["adapter::", "Dependency", "LockEntry"];

pub(crate) fn crate_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

pub(crate) fn repo_root() -> PathBuf {
    crate_dir().join("..").join("..")
}

/// Every `.rs` file under `root` (iterative walk). A missing root yields none.
pub(crate) fn rust_files(root: &Path) -> Vec<PathBuf> {
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

pub(crate) fn read(path: &Path) -> String {
    match fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => panic!("cannot read {}: {e}", path.display()),
    }
}

/// `std::os::<anything>::process` (e.g. `std::os::unix::process`).
pub(crate) fn os_process_hits(src: &str) -> Vec<String> {
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

/// Collapses whitespace around every `::` (`std :: process` ⇒
/// `std::process`), so spacing cannot split a banned path.
pub(crate) fn normalize(src: &str) -> String {
    let mut out = String::with_capacity(src.len());
    let mut rest = src;
    while let Some(i) = rest.find("::") {
        out.push_str(rest[..i].trim_end());
        out.push_str("::");
        rest = rest[i + 2..].trim_start();
    }
    out.push_str(rest);
    out
}

/// The body of the `{…}` group that starts at `s[0] == '{'`, if balanced.
pub(crate) fn group_body(s: &str) -> Option<&str> {
    let mut depth = 0usize;
    for (j, c) in s.char_indices() {
        match c {
            '{' => depth += 1,
            '}' => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return Some(&s[1..j]);
                }
            }
            _ => {}
        }
    }
    None
}

/// Every `std::{…}` / `core::{…}` group expanded to flat paths (on
/// normalised source), e.g. `use std::{process::Command}` ⇒
/// `std::process::Command`.
pub(crate) fn std_group_paths(src: &str) -> Vec<String> {
    let mut out = Vec::new();
    for root in ["std", "core"] {
        let pat = format!("{root}::{{");
        for (i, _) in src.match_indices(&pat) {
            if src[..i].chars().next_back().is_some_and(is_ident) {
                continue;
            }
            let open = i + pat.len() - 1;
            let body = group_body(&src[open..]).unwrap_or(&src[open + 1..]);
            expand_group(root, body, &mut out);
        }
    }
    out
}

/// `use std as s;` / `extern crate core as c;` / `std::{self as s}` would
/// let a later `s::process` evade the token list.
pub(crate) fn std_alias_hits(src: &str) -> Vec<String> {
    let mut hits = Vec::new();
    for root in ["std", "core"] {
        for (i, _) in src.match_indices(root) {
            let before = src[..i].chars().next_back();
            let after = &src[i + root.len()..];
            if before.is_some_and(is_ident) || after.chars().next().is_some_and(is_ident) {
                continue;
            }
            if after.split_whitespace().next() == Some("as") {
                hits.push(format!("{root} aliased"));
            }
        }
    }
    for p in std_group_paths(src) {
        if p == "std" || p == "core" {
            hits.push(format!("{p} aliased via self"));
        }
    }
    hits
}

/// `include!`-family macros splice unscanned text into the crate.
pub(crate) const INCLUDE_MACROS: &[&str] = &["include", "include_str", "include_bytes"];

pub(crate) fn include_hits(src: &str) -> Vec<String> {
    let mut hits = Vec::new();
    for m in INCLUDE_MACROS {
        for (i, _) in src.match_indices(m) {
            let before = src[..i].chars().next_back();
            let after = src[i + m.len()..].trim_start();
            if !before.is_some_and(is_ident) && after.starts_with('!') {
                hits.push(format!("{m}!"));
            }
        }
    }
    hits
}

pub(crate) fn banned_hits(src: &str) -> Vec<String> {
    let src = &normalize(src);
    let mut hits: Vec<String> = BANNED
        .iter()
        .filter(|t| src.contains(*t))
        .map(|t| t.to_string())
        .collect();
    hits.extend(os_process_hits(src));
    for p in std_group_paths(src) {
        let p = format!("{p}::");
        if BANNED.iter().any(|t| p.contains(t)) || !os_process_hits(&p).is_empty() {
            hits.push(p);
        }
    }
    hits.extend(std_alias_hits(src));
    hits.extend(include_hits(src));
    hits
}

/// The value of every `#[path = "…"]` attribute in `src`.
pub(crate) fn path_attrs(src: &str) -> Vec<String> {
    let flat: String = src.split_whitespace().collect();
    let pat = "#[path=\"";
    let mut out = Vec::new();
    for (i, _) in flat.match_indices(pat) {
        let rest = &flat[i + pat.len()..];
        if let Some(end) = rest.find('"') {
            out.push(rest[..end].to_string());
        }
    }
    out
}

/// `#[path]` targets of `file` that resolve outside `src_dir` (or not at
/// all). Paths resolve against the declaring file's directory.
pub(crate) fn path_escapes(file: &Path, src: &str, src_dir: &Path) -> Vec<String> {
    let Ok(src_dir) = fs::canonicalize(src_dir) else {
        return vec![format!("cannot canonicalize {}", src_dir.display())];
    };
    let base = file.parent().unwrap_or(Path::new("."));
    path_attrs(src)
        .into_iter()
        .filter(|p| match fs::canonicalize(base.join(p)) {
            Ok(t) => !t.starts_with(&src_dir),
            Err(_) => true,
        })
        .collect()
}

pub(crate) fn is_ident(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Expands `prefix::{a, b::{c, d as e}, self, *}` into flat paths. Iterative
/// over a work list; `as` aliases are dropped (the aliased item is what is
/// checked).
pub(crate) fn expand_group(prefix: &str, body: &str, out: &mut Vec<String>) {
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
pub(crate) fn core_paths(src: &str) -> Vec<String> {
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

pub(crate) fn core_violations(src: &str, allow_tauri_deps: bool) -> Vec<String> {
    core_paths(src)
        .into_iter()
        .filter(|p| {
            let ok = CORE_ALLOW.contains(&p.as_str())
                || (allow_tauri_deps && CORE_ALLOW_TAURI_DEPS.contains(&p.as_str()));
            !ok
        })
        .collect()
}

/// Every `package = …` key (a renamed dependency) in a manifest, comments
/// dropped. `[package]` headers and keys like `sub-package` do not match.
pub(crate) fn package_keys(toml: &str) -> Vec<String> {
    toml.lines()
        .map(|l| l.split('#').next().unwrap_or(""))
        .filter(|l| {
            let flat: String = l.split_whitespace().collect();
            flat.match_indices("package=").any(|(i, _)| {
                !flat[..i]
                    .chars()
                    .next_back()
                    .is_some_and(|c| c.is_alphanumeric() || c == '_' || c == '-')
            })
        })
        .map(|l| l.trim().to_string())
        .collect()
}
