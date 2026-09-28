//! AC3: `classify_update` per scheme (1.2.3 vs 1.2.5 / 1.4.0 / 2.0.0),
//! `select_latest` pre/deprecated/default cases, and the
//! `classify_update(v, v) == None` property.

use super::*;
use proptest::prelude::*;

const ALL: [Ecosystem; 5] = Ecosystem::ALL;

fn go_form(eco: Ecosystem, v: &str) -> String {
    if eco == Ecosystem::Go {
        format!("v{v}")
    } else {
        v.to_string()
    }
}

#[test]
fn ac3_classify_table_every_scheme() {
    for eco in ALL {
        let c = |cur: &str, lat: &str| classify_update(eco, &go_form(eco, cur), &go_form(eco, lat));
        assert_eq!(c("1.2.3", "1.2.5"), Some(UpdateKind::Patch), "{eco:?}");
        assert_eq!(c("1.2.3", "1.4.0"), Some(UpdateKind::Minor), "{eco:?}");
        assert_eq!(c("1.2.3", "2.0.0"), Some(UpdateKind::Major), "{eco:?}");
        assert_eq!(c("1.2.3", "1.2.3"), None, "{eco:?}");
        assert_eq!(c("1.2.5", "1.2.3"), None, "{eco:?} downgrade");
        assert_eq!(c("2.0.0", "1.9.9"), None, "{eco:?} downgrade major");
    }
}

#[test]
fn prerelease_to_release_is_patch() {
    assert_eq!(
        classify_update(Ecosystem::Npm, "2.0.0-rc.1", "2.0.0"),
        Some(UpdateKind::Patch)
    );
    assert_eq!(
        classify_update(Ecosystem::Pypi, "2.0rc1", "2.0"),
        Some(UpdateKind::Patch)
    );
    assert_eq!(
        classify_update(Ecosystem::Nuget, "2.0.0-beta", "2.0.0"),
        Some(UpdateKind::Patch)
    );
}

#[test]
fn unparseable_side_is_none() {
    assert_eq!(classify_update(Ecosystem::Npm, "garbage", "1.0.0"), None);
    assert_eq!(classify_update(Ecosystem::Cargo, "1.0.0", ""), None);
    assert_eq!(classify_update(Ecosystem::Go, "1.0.0", "v2.0.0"), None);
}

#[test]
fn declared_uses_the_constraint_floor() {
    let d = |eco, decl, lat| classify_update_declared(eco, decl, lat);
    assert_eq!(
        d(Ecosystem::Npm, "^1.2.3", "1.2.9"),
        Some(UpdateKind::Patch)
    );
    assert_eq!(d(Ecosystem::Npm, "~1.2", "1.3.0"), Some(UpdateKind::Minor));
    assert_eq!(d(Ecosystem::Cargo, "1", "2.0.0"), Some(UpdateKind::Major));
    assert_eq!(d(Ecosystem::Pypi, ">=1.4,<2", "1.4.0"), None);
    assert_eq!(
        d(Ecosystem::Nuget, "[1.0,2.0)", "1.1.0"),
        Some(UpdateKind::Minor)
    );
    assert_eq!(d(Ecosystem::Npm, "*", "1.0.0"), None);
}

fn cand(version: &str, is_default: bool, is_deprecated: bool) -> VersionCandidate<'_> {
    VersionCandidate {
        version,
        is_default,
        is_deprecated,
    }
}

#[test]
fn select_prefers_default_when_not_prerelease() {
    let vs = [
        cand("1.0.0", false, false),
        cand("1.5.0", true, false),
        cand("2.0.0", false, false),
    ];
    assert_eq!(
        select_latest(Ecosystem::Npm, &vs, Some("1.0.0")),
        Some("1.5.0")
    );
}

#[test]
fn select_skips_deprecated_or_prerelease_default() {
    let vs = [
        cand("1.0.0", false, false),
        cand("1.9.0", true, true),
        cand("3.0.0-beta.1", true, false),
        cand("2.0.0", false, false),
        cand("2.1.0", false, true),
    ];
    assert_eq!(select_latest(Ecosystem::Npm, &vs, None), Some("2.0.0"));
}

#[test]
fn select_allows_prerelease_when_current_is_prerelease() {
    let vs = [
        cand("2.0.0", true, false),
        cand("3.0.0-beta.2", false, false),
        cand("3.0.0-beta.3", false, true),
    ];
    assert_eq!(
        select_latest(Ecosystem::Cargo, &vs, Some("3.0.0-beta.1")),
        Some("3.0.0-beta.2")
    );
    assert_eq!(
        select_latest(Ecosystem::Cargo, &vs, Some("2.0.0")),
        Some("2.0.0")
    );
}

#[test]
fn select_ignores_unparseable_and_handles_empty() {
    let vs = [
        cand("not-a-version", true, false),
        cand("1.0.0", false, false),
    ];
    assert_eq!(select_latest(Ecosystem::Npm, &vs, None), Some("1.0.0"));
    assert_eq!(select_latest(Ecosystem::Npm, &[], None), None);
    let all_pre = [cand("1.0.0-alpha", false, false)];
    assert_eq!(select_latest(Ecosystem::Npm, &all_pre, None), None);
}

#[test]
fn select_uses_scheme_order_not_string_order() {
    let vs = [
        cand("1.10", false, false),
        cand("1.9", false, false),
        cand("1.10.post1", false, false),
        cand("1.11.dev0", false, false),
    ];
    assert_eq!(
        select_latest(Ecosystem::Pypi, &vs, None),
        Some("1.10.post1")
    );
    let go = [cand("v1.10.0", false, false), cand("v1.9.0", false, false)];
    assert_eq!(select_latest(Ecosystem::Go, &go, None), Some("v1.10.0"));
    let nu = [
        cand("1.0.0.10", false, false),
        cand("1.0.0.9", false, false),
    ];
    assert_eq!(select_latest(Ecosystem::Nuget, &nu, None), Some("1.0.0.10"));
}

fn arb_version() -> impl Strategy<Value = (u64, u64, u64, Option<u64>)> {
    (0u64..50, 0u64..50, 0u64..50, proptest::option::of(0u64..5))
}

proptest! {
    #[test]
    fn classify_same_version_is_none((a, b, c, pre) in arb_version()) {
        for eco in ALL {
            let base = match (eco, pre) {
                (Ecosystem::Pypi, Some(p)) => format!("{a}.{b}.{c}rc{p}"),
                (_, Some(p)) => format!("{a}.{b}.{c}-rc.{p}"),
                (_, None) => format!("{a}.{b}.{c}"),
            };
            let v = go_form(eco, &base);
            prop_assert_eq!(classify_update(eco, &v, &v), None);
        }
    }

    #[test]
    fn classify_is_antisymmetric(x in arb_version(), y in arb_version()) {
        for eco in ALL {
            let f = |(a, b, c, _): (u64, u64, u64, Option<u64>)| go_form(eco, &format!("{a}.{b}.{c}"));
            let (l, r) = (f(x), f(y));
            let both = classify_update(eco, &l, &r).is_some() && classify_update(eco, &r, &l).is_some();
            prop_assert!(!both);
        }
    }
}
