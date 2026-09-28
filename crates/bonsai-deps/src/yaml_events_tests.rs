//! `[HIGH-2]` yaml_events tests (E4).

use std::time::{Duration, Instant};

use super::*;
use crate::fixtures::yaml_bombs::*;

fn all(_: &[String]) -> bool {
    true
}

fn kind(r: Result<Vec<YNode>, YamlError>) -> ParseErrorKind {
    match r {
        Err(e) => e.kind,
        Ok(d) => panic!("expected an error, parsed {} docs", d.len()),
    }
}

#[test]
fn yaml_alias_bomb_is_unsupported_fast() {
    let bomb = alias_bomb();
    let t = Instant::now();
    assert_eq!(
        kind(parse_documents(&bomb, &all)),
        ParseErrorKind::UnsupportedFeature
    );
    assert!(
        t.elapsed() < Duration::from_millis(100),
        "{:?}",
        t.elapsed()
    );
}

#[test]
fn yaml_anchor_only_is_unsupported() {
    assert_eq!(
        kind(parse_documents(ANCHOR_ONLY, &all)),
        ParseErrorKind::UnsupportedFeature
    );
    // An anchor on a scalar, too.
    assert_eq!(
        kind(parse_documents("a: &x 1\n", &all)),
        ParseErrorKind::UnsupportedFeature
    );
}

#[test]
fn a_lone_alias_is_rejected() {
    let k = kind(parse_documents(ALIAS_ONLY, &all));
    assert!(
        matches!(
            k,
            ParseErrorKind::UnsupportedFeature | ParseErrorKind::InvalidSyntax
        ),
        "{k:?}"
    );
}

#[test]
fn yaml_deep_nesting_is_limit_exceeded() {
    assert_eq!(
        kind(parse_documents(&block_nesting(YAML_MAX_DEPTH + 1), &all)),
        ParseErrorKind::LimitExceeded
    );
    // Far past the cap: stopped at the first offending event, no stack use.
    assert_eq!(
        kind(parse_documents(&block_nesting(20_000), &all)),
        ParseErrorKind::LimitExceeded
    );
    // Flow nesting: our cap fires at 65 unless the scanner's own u8
    // flow-level cap (it scans a run of `[` ahead) fires first, which
    // surfaces as InvalidSyntax. Either way the parse stops, without stack
    // growth.
    assert_eq!(
        kind(parse_documents(&flow_nesting(YAML_MAX_DEPTH + 1), &all)),
        ParseErrorKind::LimitExceeded
    );
    let k = kind(parse_documents(&flow_nesting(100_000), &all));
    assert!(
        matches!(
            k,
            ParseErrorKind::LimitExceeded | ParseErrorKind::InvalidSyntax
        ),
        "{k:?}"
    );
}

#[test]
fn nesting_at_the_cap_parses() {
    // block_nesting(n) opens n mappings (the root is one of them).
    let docs = parse_documents(&block_nesting(YAML_MAX_DEPTH), &all).unwrap();
    assert_eq!(docs.len(), 1);
}

#[test]
fn yaml_node_cap_is_limit_exceeded() {
    // YAML_MAX_NODES scalars + the sequence node itself = cap + 1.
    let wide = wide_seq(YAML_MAX_NODES);
    assert_eq!(
        kind(parse_documents(&wide, &all)),
        ParseErrorKind::LimitExceeded
    );
    assert!(parse_documents(&wide_seq(1000), &all).is_ok());
}

#[test]
fn the_node_cap_spans_documents() {
    // Two documents of just over half the cap each.
    let half = wide_seq(YAML_MAX_NODES / 2 + 1);
    let two = format!("{half}\n---\n{half}\n");
    assert_eq!(
        kind(parse_documents(&two, &all)),
        ParseErrorKind::LimitExceeded
    );
}

#[test]
fn keep_predicate_skips_subtrees_but_still_checks_them() {
    let text = "a:\n  x: 1\nb:\n  y: 2\n";
    let keep = |p: &[String]| p.first().is_none_or(|k| k == "a");
    let doc = parse_first(text, &keep).unwrap();
    assert_eq!(
        doc.get("a")
            .and_then(|a| a.get("x"))
            .and_then(YNode::as_str),
        Some("1")
    );
    assert_eq!(doc.get("b"), Some(&YNode::Skipped));
    // An anchor inside a skipped subtree is still rejected.
    let hidden = "a: 1\nb:\n  c: &z 2\n";
    assert_eq!(
        kind(parse_documents(hidden, &keep)),
        ParseErrorKind::UnsupportedFeature
    );
    // Depth is counted inside skipped subtrees too.
    let deep = format!("a: 1\nb:\n{}", indent(&block_nesting(YAML_MAX_DEPTH), 2));
    assert_eq!(
        kind(parse_documents(&deep, &keep)),
        ParseErrorKind::LimitExceeded
    );
}

fn indent(s: &str, n: usize) -> String {
    s.lines()
        .map(|l| format!("{}{l}\n", " ".repeat(n)))
        .collect()
}

#[test]
fn complex_keys_are_unsupported() {
    assert_eq!(
        kind(parse_documents("? [a, b]\n: 1\n", &all)),
        ParseErrorKind::UnsupportedFeature
    );
}

#[test]
fn syntax_errors_are_located_and_textless() {
    let err = parse_documents("a: 1\nb: [zz-sentinel-token\n", &all).unwrap_err();
    assert_eq!(err.kind, ParseErrorKind::InvalidSyntax);
    assert!(err.line.is_some());
    assert!(!format!("{err:?}").contains("zz-sentinel-token"));
}

#[test]
fn multiple_documents_are_returned_in_order() {
    let docs = parse_documents("a: 1\n---\nb: 2\n", &all).unwrap();
    assert_eq!(docs.len(), 2);
    assert!(docs[1].get("b").is_some());
}

#[test]
fn quoted_keys_and_sequences_materialise() {
    let text = "'@scope/pkg@1.0.0':\n  resolution: {integrity: sha512-x, tarball: https://h/p.tgz}\nlist:\n  - one\n  - two\n";
    let doc = parse_first(text, &all).unwrap();
    let res = doc
        .get("@scope/pkg@1.0.0")
        .and_then(|n| n.get("resolution"));
    assert_eq!(
        res.and_then(|r| r.get("tarball")).and_then(YNode::as_str),
        Some("https://h/p.tgz")
    );
    assert_eq!(
        doc.get("list"),
        Some(&YNode::Seq(vec![
            YNode::Scalar("one".into()),
            YNode::Scalar("two".into())
        ]))
    );
}

#[test]
fn empty_input_is_an_empty_map() {
    assert_eq!(parse_first("", &all).unwrap(), YNode::Map(Vec::new()));
}

#[test]
fn leading_bom_is_kept_in_the_first_key_by_yaml_rust2() {
    // Why `parse_documents` strips it: the raw parser does not.
    let mut p = Parser::new_from_str("\u{FEFF}k: 1\n");
    let mut first_scalar = None;
    while let Ok((ev, _)) = p.next_token() {
        match ev {
            Event::Scalar(s, ..) => {
                first_scalar = Some(s);
                break;
            }
            Event::StreamEnd => break,
            _ => {}
        }
    }
    assert_eq!(first_scalar.as_deref(), Some("\u{FEFF}k"));
}

#[test]
fn leading_bom_is_stripped_before_parsing() {
    let doc = parse_first("\u{FEFF}npmRegistryServer: x\n", &all).unwrap();
    assert_eq!(
        doc.get("npmRegistryServer").and_then(YNode::as_str),
        Some("x")
    );
}
