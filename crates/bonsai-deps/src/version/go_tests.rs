use super::*;

fn p(s: &str) -> Option<Version> {
    GoScheme.parse(s)
}

#[test]
fn requires_v_prefix_and_three_components() {
    assert!(p("v1.2.3").is_some());
    assert!(p("1.2.3").is_none());
    assert!(p("v1.2").is_none());
}

#[test]
fn incompatible_is_ignored_for_order() {
    assert_eq!(p("v2.0.0+incompatible"), p("v2.0.0"));
}

#[test]
fn pseudo_versions_are_prerelease_and_ordered() {
    let pseudo = p("v0.0.0-20191109021931-daa7c04131f5");
    assert!(pseudo.as_ref().is_some_and(|v| GoScheme.is_prerelease(v)));
    let base_pseudo = p("v1.2.4-0.20191109021931-daa7c04131f5");
    assert!(base_pseudo > p("v1.2.3"));
    assert!(base_pseudo < p("v1.2.4"));
}

#[test]
fn lower_bound_is_the_required_version() {
    assert_eq!(GoScheme.lower_bound("v1.4.0"), p("v1.4.0"));
    assert_eq!(GoScheme.lower_bound("latest"), None);
}
