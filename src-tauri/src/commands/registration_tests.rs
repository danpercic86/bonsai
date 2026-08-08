//! Handler-registration completeness (audit 2026-08-07 §4.1).
//!
//! A `#[tauri::command]` fn that is missing from the `generate_handler!` list
//! in `lib.rs` compiles fine and fails only at runtime ("command not found").
//! This test parses the SOURCE files at test time — dependency-free, line-based
//! — and asserts a bijection between the two sets, in both directions.
//!
//! Test code only: reads sources via `CARGO_MANIFEST_DIR`, touches no app code.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

fn manifest_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Extracts the identifier immediately following the first `fn ` on `line`
/// (after any `pub`/`pub(crate)`/`async` qualifiers), or `None` if the line
/// declares no fn. Tolerates generics/parens right after the name.
fn fn_name_on_line(line: &str) -> Option<String> {
    // Find a standalone `fn` token so e.g. a comment word "fnord" never hits.
    let bytes = line.as_bytes();
    let mut i = 0;
    while let Some(off) = line[i..].find("fn") {
        let at = i + off;
        let before_ok = at == 0 || !bytes[at - 1].is_ascii_alphanumeric() && bytes[at - 1] != b'_';
        let after = at + 2;
        let after_ok = after < bytes.len() && (bytes[after] == b' ' || bytes[after] == b'\t');
        if before_ok && after_ok {
            let rest = line[after..].trim_start();
            let name: String = rest
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                .collect();
            if !name.is_empty() {
                return Some(name);
            }
        }
        i = at + 2;
    }
    None
}

/// Every fn under `#[tauri::command]` (with or without attribute args, other
/// attributes in between, multi-line signatures) in one source file.
fn commands_defined_in(source: &str) -> Vec<String> {
    let lines: Vec<&str> = source.lines().collect();
    let mut out = Vec::new();
    let mut idx = 0;
    while idx < lines.len() {
        let trimmed = lines[idx].trim_start();
        // Attribute position only — doc comments mentioning the attr don't match.
        if trimmed.starts_with("#[tauri::command") {
            // Scan forward (bounded) for the next line that declares a fn,
            // skipping further attributes / attribute continuations.
            let mut j = idx + 1;
            let mut found = false;
            while j < lines.len() && j <= idx + 20 {
                if let Some(name) = fn_name_on_line(lines[j]) {
                    out.push(name);
                    found = true;
                    break;
                }
                j += 1;
            }
            assert!(
                found,
                "found `#[tauri::command]` with no fn within 20 lines (line {})",
                idx + 1
            );
            idx = j;
        }
        idx += 1;
    }
    out
}

/// All `#[tauri::command]` fn names across every `.rs` file in `src/commands/`.
fn all_defined_commands(commands_dir: &Path) -> BTreeSet<String> {
    let mut set = BTreeSet::new();
    for entry in std::fs::read_dir(commands_dir).expect("read src/commands") {
        let path = entry.expect("dir entry").path();
        if path.extension().and_then(|e| e.to_str()) != Some("rs") {
            continue;
        }
        // Skip `#[cfg(test)]` modules: they cannot define real handlers, and
        // THIS file's fixture strings would otherwise trip the scanner.
        let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
        if stem == "tests" || stem.ends_with("_tests") {
            continue;
        }
        let source = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
        for name in commands_defined_in(&source) {
            assert!(
                set.insert(name.clone()),
                "duplicate #[tauri::command] fn name `{name}` (second hit in {})",
                path.display()
            );
        }
    }
    set
}

/// Handler names registered in the `generate_handler![ ... ]` block of
/// `lib.rs`: paths like `commands::name` and bare `name` both count; the last
/// `::` segment is the wire name. Robust to formatting — the block is located
/// by bracket depth, then split on commas.
fn registered_commands(lib_rs: &str) -> BTreeSet<String> {
    let start = lib_rs
        .find("generate_handler![")
        .expect("lib.rs must contain a generate_handler![ block");
    let body_start = start + "generate_handler![".len();
    let mut depth = 1usize;
    let mut end = None;
    for (i, c) in lib_rs[body_start..].char_indices() {
        match c {
            '[' => depth += 1,
            ']' => {
                depth -= 1;
                if depth == 0 {
                    end = Some(body_start + i);
                    break;
                }
            }
            _ => {}
        }
    }
    let body = &lib_rs[body_start..end.expect("unbalanced generate_handler! brackets")];

    // Strip `// ...` line comments FIRST (per line), then comma-split — a
    // trailing comment must not swallow the entry on the following line.
    let no_comments: String = body
        .lines()
        .map(|l| l.split("//").next().unwrap_or(""))
        .collect::<Vec<_>>()
        .join("\n");

    let mut set = BTreeSet::new();
    for raw in no_comments.split(',') {
        let entry = raw.trim();
        if entry.is_empty() {
            continue;
        }
        let name = entry.rsplit("::").next().unwrap_or(entry).trim();
        assert!(
            name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'),
            "unexpected generate_handler! entry shape: `{entry}`"
        );
        assert!(
            set.insert(name.to_string()),
            "handler `{name}` registered twice in generate_handler!"
        );
    }
    set
}

/// The bijection: every `#[tauri::command]` fn under `src/commands/` is
/// registered in `lib.rs`, and every registered handler exists as a command fn.
/// (No count assertion beyond the set equality — counts drift by design.)
#[test]
fn every_command_is_registered_and_vice_versa() {
    let root = manifest_dir();
    let defined = all_defined_commands(&root.join("src").join("commands"));
    let lib_rs = std::fs::read_to_string(root.join("src").join("lib.rs")).expect("read lib.rs");
    let registered = registered_commands(&lib_rs);

    let unregistered: Vec<&String> = defined.difference(&registered).collect();
    let phantom: Vec<&String> = registered.difference(&defined).collect();

    assert!(
        unregistered.is_empty() && phantom.is_empty(),
        "generate_handler! list in src/lib.rs is out of sync with \
         #[tauri::command] fns in src/commands/*.rs\n\
         defined but NOT registered (runtime 'command not found'): {unregistered:?}\n\
         registered but NOT defined: {phantom:?}\n\
         (defined={}, registered={})",
        defined.len(),
        registered.len()
    );
}

/// Parser self-checks so a silent regex-style miss can't quietly turn the main
/// test into a vacuous pass: attribute args, multi-line signatures, extra
/// attributes between the command attr and the fn, and non-pub fns all parse.
#[test]
fn source_scanners_handle_tricky_shapes() {
    let src = r#"
        //! doc mentioning #[tauri::command] must NOT count
        #[tauri::command]
        pub async fn plain(a: u32) {}

        #[tauri::command(rename_all = "snake_case")]
        #[allow(clippy::too_many_arguments)]
        pub(crate) fn with_args(
            state: State<'_, AppState>,
        ) -> Result<(), Err> { Ok(()) }

        #[tauri::command]
        async fn private_cmd() {}
    "#;
    let names = commands_defined_in(src);
    assert_eq!(names, vec!["plain", "with_args", "private_cmd"]);

    let lib = r#"
        .invoke_handler(tauri::generate_handler![
            commands::plain, // trailing comment
            commands::nested::with_args,
            private_cmd
        ])
    "#;
    let reg = registered_commands(lib);
    let want: BTreeSet<String> = ["plain", "with_args", "private_cmd"]
        .iter()
        .map(|s| s.to_string())
        .collect();
    assert_eq!(reg, want);
}
