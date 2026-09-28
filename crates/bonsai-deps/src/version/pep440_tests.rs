use super::*;
use crate::model::{Ecosystem, UpdateKind};
use crate::version::classify_update;

fn p(s: &str) -> Pep440Version {
    match parse_pep440(s) {
        Some(v) => v,
        None => panic!("{s:?} should parse"),
    }
}

/// PEP 440's own ordering example (spec "Summary of permitted suffixes and
/// relative ordering"), ascending.
#[test]
fn pep440_total_order() {
    let ordered = [
        "1.dev0",
        "1.0.dev456",
        "1.0a1",
        "1.0a2.dev456",
        "1.0a12.dev456",
        "1.0a12",
        "1.0b1.dev456",
        "1.0b2",
        "1.0b2.post345.dev456",
        "1.0b2.post345",
        "1.0rc1.dev456",
        "1.0rc1",
        "1.0",
        "1.0+abc.5",
        "1.0+abc.7",
        "1.0+5",
        "1.0.post456.dev34",
        "1.0.post456",
        "1.0.15",
        "1.1.dev1",
    ];
    for w in ordered.windows(2) {
        assert!(p(w[0]) < p(w[1]), "{} < {}", w[0], w[1]);
    }
}

#[test]
fn normalizations() {
    assert_eq!(p("1.0"), p("1.0.0"));
    assert_eq!(p("v1.0"), p("1.0"));
    assert_eq!(p("1.0ALPHA1"), p("1.0a1"));
    assert_eq!(p("1.0-beta.2"), p("1.0b2"));
    assert_eq!(p("1.0c1"), p("1.0rc1"));
    assert_eq!(p("1.0pre1"), p("1.0rc1"));
    assert_eq!(p("1.0preview_1"), p("1.0rc1"));
    assert_eq!(p("1.0-1"), p("1.0.post1"));
    assert_eq!(p("1.0rev1"), p("1.0.post1"));
    assert_eq!(p("1.0.post"), p("1.0.post0"));
    assert_eq!(p("1.0-dev"), p("1.0.dev0"));
    assert_eq!(p("1.0a"), p("1.0a0"));
}

#[test]
fn rejects_invalid() {
    for s in ["", "abc", "1.0+", "1.0+a..b", "1..0", "1.0 junk", "1.0-"] {
        assert!(parse_pep440(s).is_none(), "{s:?}");
    }
}

#[test]
fn prerelease_detection() {
    let pre = |s: &str| Pep440Scheme.is_prerelease(&p(s));
    assert!(pre("1.0a1"));
    assert!(pre("1.0.dev1"));
    assert!(pre("1.0.post1.dev1"));
    assert!(!pre("1.0"));
    assert!(!pre("1.0.post1"));
}

#[test]
fn epoch_change_is_major() {
    assert_eq!(
        classify_update(Ecosystem::Pypi, "2.0", "1!1.0"),
        Some(UpdateKind::Major)
    );
    assert_eq!(
        classify_update(Ecosystem::Pypi, "1.0", "1.0.post1"),
        Some(UpdateKind::Patch)
    );
}

#[test]
fn lower_bounds() {
    let lb = |c: &str| Pep440Scheme.lower_bound(c);
    assert_eq!(lb(">=1.4,<2"), Some(p("1.4")));
    assert_eq!(lb("~=1.4.2"), Some(p("1.4.2")));
    assert_eq!(lb("==1.2.*"), Some(p("1.2")));
    assert_eq!(lb("^1.2"), Some(p("1.2")));
    assert_eq!(lb(">=1.0, !=1.5, >=1.2"), Some(p("1.2")));
    assert_eq!(lb("<2"), None);
    assert_eq!(lb("*"), None);
}

#[test]
fn long_input_is_linear_not_recursive() {
    let long = format!("1{}", ".0".repeat(100_000));
    assert_eq!(parse_pep440(&long), Some(p("1")));
}
