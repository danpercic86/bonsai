//! Self-tests: the scanner catches what it claims to.

use super::scan::*;

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

#[test]
fn scanner_normalises_spacing_and_expands_std_groups() {
    for c in [
        "use std :: process :: Command;",
        "let _ = std\n    ::process::Command::new(\"x\");",
        "use std::{process::Command};",
        "use std::{io, process};",
        "use std::{io::{self, Read}, process as p};",
        "use ::std::{os::{unix::{process::CommandExt}}};",
        "use std::{os::{windows::process::CommandExt}};",
        "use std::{\n    process\n};",
        "Command :: new(\"x\")",
    ] {
        assert!(!banned_hits(c).is_empty(), "{c}");
    }
    assert!(banned_hits("use std::{io::Read, fs, collections::BTreeMap};").is_empty());
    assert!(banned_hits("use std::os::{raw::c_int};").is_empty());
}

#[test]
fn scanner_flags_std_aliases_and_include_macros() {
    for c in [
        "use std as s;",
        "extern crate core as c;",
        "use std::{self as s};",
        "include!(\"../../other.rs\");",
        "const X: &str = include_str!(\"x.txt\");",
        "static B: &[u8] = include_bytes ! (\"b\");",
    ] {
        assert!(!banned_hits(c).is_empty(), "{c}");
    }
    // Words that merely contain the roots are fine.
    assert!(banned_hits("let stdout = 1; let core_count = 2; my_include!(x);").is_empty());
}

#[test]
fn scanner_resolves_path_attributes() {
    let src_dir = crate_dir().join("src");
    let file = src_dir.join("registry_ctx.rs");
    let ok = "#[path = \"registry_ctx/load_tests.rs\"]\nmod load_tests;";
    assert!(path_escapes(&file, ok, &src_dir).is_empty());
    for bad in [
        "#[path = \"../tests/no_process.rs\"]\nmod x;",
        "#[ path=\"../../../src-tauri/src/lib.rs\" ]\nmod y;",
        "#[path = \"does_not_exist.rs\"]\nmod z;",
    ] {
        assert!(!path_escapes(&file, bad, &src_dir).is_empty(), "{bad}");
    }
}

#[test]
fn scanner_finds_package_renames() {
    let bad = "[dependencies]\ncore2 = { package = \"bonsai-core\", path = \"../bonsai-core\" }\n";
    assert_eq!(package_keys(bad).len(), 1);
    let bad2 = "[dependencies.core2]\npackage=\"bonsai-core\"\n";
    assert_eq!(package_keys(bad2).len(), 1);
    let ok = "[package]\nname = \"x\"\n# package = \"commented\"\nsub-package = 1\n";
    assert!(package_keys(ok).is_empty());
}
