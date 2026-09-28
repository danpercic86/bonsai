use super::*;

fn p(s: &str) -> Option<Version> {
    SemverScheme.parse(s)
}

#[test]
fn parses_loose_prefixes() {
    assert_eq!(p("v1.2.3"), p("1.2.3"));
    assert_eq!(p("=1.2.3"), p("1.2.3"));
    assert!(p("1.2").is_none(), "concrete versions only");
    assert!(p("^1.2.3").is_none());
}

#[test]
fn build_metadata_is_ignored_for_order() {
    assert_eq!(p("1.2.3+a"), p("1.2.3+b"));
}

#[test]
fn prerelease_orders_before_release() {
    let (pre, rel) = (p("1.0.0-alpha.1"), p("1.0.0"));
    assert!(pre < rel);
    assert!(pre.is_some_and(|v| SemverScheme.is_prerelease(&v)));
    assert!(p("1.0.0-alpha.2") < p("1.0.0-alpha.10"));
}

#[test]
fn lower_bounds() {
    let lb = |c: &str| SemverScheme.lower_bound(c);
    assert_eq!(lb("^1.2.3"), p("1.2.3"));
    assert_eq!(lb("~1.2"), p("1.2.0"));
    assert_eq!(lb("1.x"), p("1.0.0"));
    assert_eq!(lb(">=1.0.0 <2.0.0"), p("1.0.0"));
    assert_eq!(lb(">= 1.1.0 < 2"), p("1.1.0"));
    assert_eq!(lb(">=1, <2"), p("1.0.0"));
    assert_eq!(lb("1.2.3 - 2.0.0"), p("1.2.3"));
    assert_eq!(lb("^2.0.0 || ^1.4.0"), p("1.4.0"));
    assert_eq!(lb("=1.2.3-beta.1"), p("1.2.3-beta.1"));
    assert_eq!(lb("*"), None);
    assert_eq!(lb("<2.0.0"), None);
    assert_eq!(lb("latest"), None);
}
