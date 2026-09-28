use std::time::{Duration, Instant};

use super::*;
use crate::fixtures::xml_bombs;

/// Records `(path, attr value)` for every `add@value`, and the text of
/// `RestoreSources`.
#[derive(Default)]
struct Rec {
    values: Vec<String>,
    texts: Vec<String>,
    starts: usize,
}

impl XmlSink for Rec {
    fn start(&mut self, _stack: &[String], elem: &Elem<'_, '_>) -> Result<(), XmlError> {
        self.starts += 1;
        if elem.name() == "add" {
            if let Some(v) = elem.attr("value")? {
                self.values.push(v);
            }
        }
        Ok(())
    }
    fn wants_text(&self, stack: &[String]) -> bool {
        stack.last().is_some_and(|n| n == "RestoreSources")
    }
    fn text(&mut self, _stack: &[String], text: &str) -> Result<(), XmlError> {
        self.texts.push(text.to_string());
        Ok(())
    }
}

fn run(text: &str) -> Result<Rec, XmlError> {
    let mut r = Rec::default();
    walk(text, &mut r)?;
    Ok(r)
}

fn kind(text: &str) -> ParseErrorKind {
    match run(text) {
        Ok(_) => panic!("expected an error"),
        Err(e) => e.kind,
    }
}

#[test]
fn xml_billion_laughs_is_unsupported_fast() {
    let bomb = xml_bombs::billion_laughs();
    let t = Instant::now();
    assert_eq!(kind(&bomb), ParseErrorKind::UnsupportedFeature);
    assert!(t.elapsed() < Duration::from_millis(100));
}

#[test]
fn external_entities_and_any_doctype_are_rejected() {
    assert_eq!(kind(xml_bombs::XXE), ParseErrorKind::UnsupportedFeature);
    assert_eq!(kind("<!doctype x><x/>"), ParseErrorKind::UnsupportedFeature);
    // Even inside a comment: fail closed.
    assert_eq!(
        kind("<x><!-- <!ENTITY a \"b\"> --></x>"),
        ParseErrorKind::UnsupportedFeature
    );
}

#[test]
fn undeclared_entities_are_never_expanded() {
    assert_eq!(
        kind(xml_bombs::UNDECLARED_ENTITY),
        ParseErrorKind::UnsupportedFeature
    );
    assert_eq!(
        kind(xml_bombs::UNDECLARED_ENTITY_TEXT),
        ParseErrorKind::UnsupportedFeature
    );
}

#[test]
fn predefined_and_numeric_references_resolve() {
    let r = run(
        "<c><add value=\"https://h.invalid/?a=1&amp;b=&#50;&#x33;\"/>\
         <RestoreSources>a&amp;b&lt;<![CDATA[&x;]]></RestoreSources></c>",
    )
    .expect("parses");
    assert_eq!(r.values, ["https://h.invalid/?a=1&b=23"]);
    assert_eq!(r.texts, ["a&b<&x;"]);
}

#[test]
fn xml_deep_nesting_is_limit_exceeded_fast() {
    let t = Instant::now();
    assert_eq!(
        kind(&xml_bombs::deep_nesting(100_000)),
        ParseErrorKind::LimitExceeded
    );
    assert!(t.elapsed() < Duration::from_millis(500));
    assert!(run(&xml_bombs::deep_nesting(XML_MAX_DEPTH)).is_ok());
    assert_eq!(
        kind(&xml_bombs::deep_nesting(XML_MAX_DEPTH + 1)),
        ParseErrorKind::LimitExceeded
    );
}

#[test]
fn a_huge_attribute_is_limit_exceeded() {
    assert_eq!(
        kind(&xml_bombs::huge_attribute(XML_MAX_VALUE + 1)),
        ParseErrorKind::LimitExceeded
    );
    assert!(run(&xml_bombs::huge_attribute(1024)).is_ok());
}

#[test]
fn huge_wanted_text_is_limit_exceeded_but_unwanted_text_is_not_kept() {
    let big = "a".repeat(XML_MAX_VALUE + 1);
    let wanted = format!("<P><RestoreSources>{big}</RestoreSources></P>");
    assert_eq!(kind(&wanted), ParseErrorKind::LimitExceeded);
    let unwanted = format!("<P><Other>{big}</Other></P>");
    assert!(run(&unwanted).expect("parses").texts.is_empty());
}

#[test]
fn the_node_cap_is_limit_exceeded() {
    assert_eq!(
        kind(&xml_bombs::wide(XML_MAX_NODES)),
        ParseErrorKind::LimitExceeded
    );
}

#[test]
fn syntax_errors_are_located_and_textless() {
    let e = run("<a>\n  <b></c>\n</a>").err().expect("mismatched end");
    assert_eq!(e.kind, ParseErrorKind::InvalidSyntax);
    assert_eq!(e.line, Some(2));
    let e = run("<a><b>").err().expect("unclosed");
    assert_eq!(e.kind, ParseErrorKind::InvalidSyntax);
}

#[test]
fn a_bom_is_stripped_and_attribute_keys_are_case_insensitive() {
    let r = run("\u{FEFF}<?xml version=\"1.0\"?><c><add VALUE=\"v\"/></c>").expect("parses");
    assert_eq!(r.values, ["v"]);
    assert_eq!(r.starts, 2);
}

#[test]
fn duplicate_attributes_are_invalid() {
    assert_eq!(
        kind("<c><add value=\"a\" value=\"b\"/></c>"),
        ParseErrorKind::InvalidSyntax
    );
    assert_eq!(
        kind("<c><add value=\"a\" Value=\"b\"/></c>"),
        ParseErrorKind::InvalidSyntax
    );
}
