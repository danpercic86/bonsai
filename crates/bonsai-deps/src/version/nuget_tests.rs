use super::*;
use crate::model::{Ecosystem, UpdateKind};
use crate::version::classify_update;

fn p(s: &str) -> Option<NugetVersion> {
    NugetScheme.parse(s)
}

#[test]
fn parses_two_to_four_segments() {
    assert_eq!(p("1.0"), p("1.0.0"));
    assert_eq!(p("1.0.0"), p("1.0.0.0"));
    assert!(p("1").is_none());
    assert!(p("1.0.0.0.0").is_none());
    assert!(p("1.a").is_none());
}

#[test]
fn fourth_segment_orders_and_is_patch() {
    assert!(p("1.0.0.1") > p("1.0.0"));
    assert_eq!(
        classify_update(Ecosystem::Nuget, "1.2.3.4", "1.2.3.5"),
        Some(UpdateKind::Patch)
    );
}

#[test]
fn prerelease_is_case_insensitive_and_ordered() {
    assert_eq!(p("1.0.0-Beta"), p("1.0.0-beta"));
    assert!(p("1.0.0-alpha") < p("1.0.0-beta"));
    assert!(p("1.0.0-beta.2") < p("1.0.0-beta.10"));
    assert!(p("1.0.0-1") < p("1.0.0-alpha"));
    assert!(p("1.0.0-beta") < p("1.0.0"));
    assert!(p("1.0.0-rc").is_some_and(|v| NugetScheme.is_prerelease(&v)));
}

#[test]
fn metadata_is_ignored() {
    assert_eq!(p("1.0.0+abc"), p("1.0.0"));
}

#[test]
fn lower_bounds() {
    let lb = |c: &str| NugetScheme.lower_bound(c);
    assert_eq!(lb("1.2.3"), p("1.2.3"));
    assert_eq!(lb("[1.0,2.0)"), p("1.0.0"));
    assert_eq!(lb("(1.5,)"), p("1.5.0"));
    assert_eq!(lb("[2.1.0]"), p("2.1.0"));
    assert_eq!(lb("1.*"), p("1.0.0"));
    assert_eq!(lb("1.2.*"), p("1.2.0"));
    assert_eq!(lb("1.0.0-*"), p("1.0.0"));
    assert_eq!(lb("*"), None);
    assert_eq!(lb("(,2.0]"), None);
}
