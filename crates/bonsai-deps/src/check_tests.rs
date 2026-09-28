use super::groups::{guarded_parse, parse_all, Budget, GroupInput};
use super::rows::{parse_message, sort_rows};
use super::*;
use crate::adapter::{ParseError, ParsedGroup};
use crate::discover::FileSet;
use crate::model::{
    Advisory, Availability, DepRow, DepScope, Ecosystem, ParseErrorKind, Relation, Severity,
    UpdateKind,
};

#[test]
fn a_panicking_parse_is_an_internal_error() {
    let r = guarded_parse("Cargo.lock", || panic!("adapter bug"));
    assert_eq!(
        r.err(),
        Some(ParseError::new("Cargo.lock", ParseErrorKind::Internal))
    );
    let ok = guarded_parse("x", || Ok(ParsedGroup::default()));
    assert!(ok.is_ok());
}

#[test]
fn the_parse_budget_trips_once_and_stays_tripped() {
    let mut b = Budget::new(10);
    assert!(b.take(6));
    assert!(!b.take(5), "6 + 5 > 10");
    assert!(!b.take(1), "later files are LimitExceeded too");
}

fn write(dir: &std::path::Path, rel: &str, body: &str) {
    let p = dir.join(rel);
    if let Some(parent) = p.parent() {
        std::fs::create_dir_all(parent).unwrap_or_else(|e| panic!("{e}"));
    }
    std::fs::write(p, body).unwrap_or_else(|e| panic!("{e}"));
}

#[test]
fn max_parse_bytes_makes_later_groups_limit_exceeded() {
    // [LOW-3] a small budget stands in for MAX_PARSE_BYTES (256 MiB).
    let dir = tempfile::tempdir().unwrap_or_else(|e| panic!("{e}"));
    let wd = std::fs::canonicalize(dir.path()).unwrap_or_else(|e| panic!("{e}"));
    let pkg = r#"{"dependencies": {"left-pad": "1.3.0"}}"#;
    for p in ["a/package.json", "b/package.json", "c/package.json"] {
        write(&wd, p, pkg);
    }
    let files = FileSet::from_paths(
        ["a/package.json", "b/package.json", "c/package.json"].map(String::from),
    );
    let budget = pkg.len() as u64 + 1;
    let out = parse_all(&wd, &files, budget, &CancelToken::new());
    let kinds: Vec<Option<ParseErrorKind>> = out
        .unwrap_or_default()
        .iter()
        .map(|g| match g {
            GroupInput::Failed { err, .. } => Some(err.kind),
            _ => None,
        })
        .collect();
    assert_eq!(
        kinds,
        [
            None,
            Some(ParseErrorKind::LimitExceeded),
            Some(ParseErrorKind::LimitExceeded)
        ]
    );
    assert_eq!(
        parse_all(&wd, &files, MAX_PARSE_BYTES, &{
            let c = CancelToken::new();
            c.cancel();
            c
        })
        .err(),
        Some(CheckError::Cancelled)
    );
}

#[test]
fn parse_messages_come_from_the_fixed_table() {
    assert_eq!(
        parse_message(ParseErrorKind::InvalidSyntax, Some(12), Some(4)),
        "invalid syntax at 12:4"
    );
    assert_eq!(
        parse_message(ParseErrorKind::LimitExceeded, Some(3), None),
        "limit exceeded at line 3"
    );
    assert_eq!(
        parse_message(ParseErrorKind::TooLarge, None, None),
        "file too large"
    );
}

fn row(name: &str, rel: Relation, update: Option<UpdateKind>, sev: Option<Severity>) -> DepRow {
    DepRow {
        name: name.into(),
        ecosystem: Ecosystem::Npm,
        relation: rel,
        scope: DepScope::Normal,
        declared: None,
        current: None,
        latest: None,
        update,
        availability: Availability::Checked,
        vulns_checked: true,
        vulns: sev
            .map(|severity| Advisory {
                id: "X-1".into(),
                aliases: Vec::new(),
                summary: None,
                severity,
                cvss_score: None,
                modified: String::new(),
                url: String::new(),
            })
            .into_iter()
            .collect(),
        registry_url: None,
    }
}

#[test]
fn rows_sort_by_severity_update_relation_name() {
    let mut rows = vec![
        row("z", Relation::Direct, None, None),
        row("b", Relation::Direct, Some(UpdateKind::Patch), None),
        row("a", Relation::Direct, Some(UpdateKind::Major), None),
        row("t", Relation::Transitive, None, Some(Severity::High)),
        row("c", Relation::Direct, None, Some(Severity::High)),
        row("u", Relation::Direct, None, Some(Severity::Unknown)),
        row("k", Relation::Direct, None, Some(Severity::Critical)),
    ];
    sort_rows(&mut rows);
    let names: Vec<&str> = rows.iter().map(|r| r.name.as_str()).collect();
    assert_eq!(names, ["k", "c", "t", "u", "a", "b", "z"]);
}

#[test]
fn cancel_token_clones_share_the_flag() {
    let a = CancelToken::new();
    let b = a.clone();
    assert!(!b.is_cancelled());
    a.cancel();
    assert!(b.is_cancelled());
    assert_eq!(CHECK_DEADLINE.as_secs(), 600);
    assert_eq!(DEPSDEV_PARALLEL, 6);
}
