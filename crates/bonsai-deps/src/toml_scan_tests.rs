//! `[HIGH-2]` §4.2 TOML depth pre-scan.

use std::time::{Duration, Instant};

use super::*;
use crate::fixtures::toml_bombs::{dotted_header, dotted_key, nested_arrays, nested_inline_tables};

#[test]
fn the_limit_is_inclusive_for_every_nesting_form() {
    let max = TOML_MAX_DEPTH as usize;
    for (form, make) in [
        ("arrays", nested_arrays as fn(usize) -> String),
        ("inline tables", nested_inline_tables),
        ("dotted key", dotted_key),
    ] {
        assert_eq!(check_depth(&make(max)), Ok(()), "{form} at the limit");
        assert!(
            check_depth(&make(max + 1)).is_err(),
            "{form} over the limit"
        );
    }
    // A header is itself one level: `[a]` = 1, each dot adds one.
    assert_eq!(check_depth(&dotted_header(max - 1)), Ok(()));
    assert!(check_depth(&dotted_header(max)).is_err());
}

#[test]
fn a_huge_bomb_is_rejected_fast_and_reports_its_line() {
    let bomb = format!("x = 1\n{}", nested_arrays(2_000_000));
    let t = Instant::now();
    assert_eq!(check_depth(&bomb), Err(TooDeep { line: 2 }));
    assert!(
        t.elapsed() < Duration::from_millis(100),
        "{:?}",
        t.elapsed()
    );
}

#[test]
fn delimiters_inside_strings_and_comments_do_not_count() {
    let deep = "[".repeat(200);
    let docs = [
        format!("a = \"{deep}\"\n"),
        format!("a = '{deep}'\n"),
        format!("a = \"\"\"\n{deep}\n\"\"\"\n"),
        format!("a = '''\n{deep}\n'''\n"),
        format!("a = \"\\\"{deep}\"\n"),
        format!("# {deep}\na = 1\n"),
        format!("a = 1 # {deep}\n"),
        // Quoted dotted keys are one segment.
        format!("\"{}\" = 1\n", "a.".repeat(200)),
        format!("['{}']\nx = 1\n", "a.".repeat(200)),
        // Floats in values are not key dots.
        format!("a = [{}]\n", vec!["1.5"; 500].join(", ")),
    ];
    for d in &docs {
        assert_eq!(check_depth(d), Ok(()), "{}", &d[..d.len().min(40)]);
    }
}

#[test]
fn multiline_strings_close_with_extra_quotes() {
    // `""""` = a closing `"""` preceded by one literal quote; the bracket
    // after it is structural again.
    let doc = format!(
        "a = \"\"\"x\"\"\"\"\nb = {}\n",
        nested_arrays(65).trim_start_matches("a = ")
    );
    assert!(check_depth(&doc).is_err());
}

#[test]
fn keys_that_open_containers_add_their_dots() {
    // 40 dots + 30 arrays = 70 > 64, though neither alone exceeds it.
    let doc = format!(
        "{} = {}{}\n",
        vec!["a"; 41].join("."),
        "[".repeat(30),
        "]".repeat(30)
    );
    assert!(check_depth(&doc).is_err());
    // Siblings do not accumulate.
    let flat: String = (0..500).map(|i| format!("k{i}.a.b = [[1]]\n")).collect();
    assert_eq!(check_depth(&flat), Ok(()));
}

#[test]
fn real_manifests_pass() {
    for doc in [
        crate::fixtures::cargo::ROOT_MANIFEST,
        crate::fixtures::cargo::LOCK,
        crate::fixtures::cargo::REPLACE_CONFIG,
        "[workspace.lints.clippy]
all = { level = \"deny\", priority = -1 }
# [[[ comment
",
        "[target.'cfg(windows)'.dependencies]\nwinapi = { version = \"0.3\", features = [\"a\", \"b\"] }\n",
        "[[package]]\nname = \"x\"\nversion = \"1.0.0\"\ndependencies = [\n \"a\",\n \"b 1.0\",\n]\n",
    ] {
        assert_eq!(check_depth(doc), Ok(()));
    }
}
