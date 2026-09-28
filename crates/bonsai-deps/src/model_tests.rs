//! `[LOW-3]` clean_display, `summarize` vs an independent recount, and the
//! serde wire shape the E6 TS mirror depends on.

use super::*;

#[test]
fn clean_display_strips_bidi_override_and_zero_width() {
    assert_eq!(clean_display("ab\u{202E}cd\u{200B}ef", 256), "abcdef");
}

#[test]
fn clean_display_strips_cc_and_other_cf() {
    let s = "a\u{0000}b\u{0007}c\n\t\u{007F}d\u{2066}e\u{2069}f\u{FEFF}g\u{00AD}h\u{E0041}i";
    assert_eq!(clean_display(s, 256), "abcdefghi");
}

#[test]
fn clean_display_truncates_257_to_256_chars() {
    let s = "x".repeat(257);
    assert_eq!(clean_display(&s, MAX_DISPLAY).chars().count(), 256);
}

#[test]
fn clean_display_counts_chars_not_bytes() {
    let s = "é".repeat(300);
    let out = clean_display(&s, MAX_SUMMARY_DISPLAY);
    assert_eq!(out.chars().count(), 300);
    let out = clean_display(&s, MAX_ID_DISPLAY);
    assert_eq!(out.chars().count(), 64);
    assert_eq!(out.len(), 128);
}

#[test]
fn clean_display_keeps_ordinary_unicode() {
    assert_eq!(clean_display("Łódź 日本 🦀", 256), "Łódź 日本 🦀");
}

fn adv(sev: Severity) -> Advisory {
    Advisory {
        id: "GHSA-aaaa-bbbb-cccc".into(),
        aliases: vec![],
        summary: None,
        severity: sev,
        cvss_score: None,
        modified: "2026-01-01T00:00:00Z".into(),
        url: "https://osv.dev/vulnerability/GHSA-aaaa-bbbb-cccc".into(),
    }
}

fn row(
    relation: Relation,
    update: Option<UpdateKind>,
    availability: Availability,
    vulns: Vec<Advisory>,
) -> DepRow {
    DepRow {
        name: "pkg".into(),
        ecosystem: Ecosystem::Npm,
        relation,
        scope: DepScope::Normal,
        declared: Some("^1.0.0".into()),
        current: Some("1.0.0".into()),
        latest: None,
        update,
        availability,
        vulns_checked: true,
        vulns,
        registry_url: None,
    }
}

fn group(state: GroupState, rows: Vec<DepRow>) -> ManifestGroup {
    ManifestGroup {
        manifest_path: "package.json".into(),
        lockfile_path: None,
        ecosystem: Some(Ecosystem::Npm),
        state,
        rows,
        transitive_checked: 0,
    }
}

fn sample_groups() -> Vec<ManifestGroup> {
    use Availability as A;
    use Relation::{Direct, Transitive};
    let private = A::Private {
        reason: PrivateReason::ScopedRegistry,
        registry_host: Some("npm.corp.invalid".into()),
    };
    vec![
        group(
            GroupState::Ok,
            vec![
                row(Direct, Some(UpdateKind::Major), A::Checked, vec![]),
                row(Direct, Some(UpdateKind::Minor), A::Checked, vec![]),
                row(Direct, None, A::Checked, vec![]),
                row(
                    Direct,
                    None,
                    A::Checked,
                    vec![adv(Severity::Low), adv(Severity::High)],
                ),
                row(Transitive, None, A::Checked, vec![adv(Severity::Critical)]),
                row(
                    Direct,
                    Some(UpdateKind::Patch),
                    A::Unknown,
                    vec![adv(Severity::Unknown)],
                ),
                row(
                    Direct,
                    None,
                    A::Local {
                        reason: LocalReason::Path,
                    },
                    vec![],
                ),
                row(Direct, None, private, vec![]),
            ],
        ),
        group(
            GroupState::NotTracked {
                label: "Gradle".into(),
            },
            vec![],
        ),
        group(
            GroupState::ParseError {
                file: "Cargo.lock".into(),
                category: ParseErrorKind::InvalidSyntax,
                line: Some(12),
                col: Some(4),
                message: "invalid syntax at 12:4".into(),
            },
            vec![],
        ),
    ]
}

/// Independent recount written against the §2.1 field comments, not against
/// `summarize`'s control flow.
fn recount(groups: &[ManifestGroup]) -> DepSummary {
    let rows: Vec<&DepRow> = groups.iter().flat_map(|g| g.rows.iter()).collect();
    let n = |f: &dyn Fn(&DepRow) -> bool| rows.iter().filter(|r| f(r)).count() as u32;
    let by_max = |sev: Severity| n(&|r| r.max_severity() == Some(sev));
    DepSummary {
        major: n(&|r| r.update == Some(UpdateKind::Major)),
        minor: n(&|r| r.update == Some(UpdateKind::Minor)),
        patch: n(&|r| r.update == Some(UpdateKind::Patch)),
        vulnerable: n(&|r| !r.vulns.is_empty()),
        vulnerable_by_severity: SeverityCounts {
            critical: by_max(Severity::Critical),
            high: by_max(Severity::High),
            medium: by_max(Severity::Medium),
            low: by_max(Severity::Low),
            unknown: by_max(Severity::Unknown),
        },
        up_to_date: n(&|r| {
            r.availability == Availability::Checked
                && r.relation == Relation::Direct
                && r.update.is_none()
        }),
        unknown: n(&|r| r.availability == Availability::Unknown),
        local: n(&|r| matches!(r.availability, Availability::Local { .. })),
        private: n(&|r| matches!(r.availability, Availability::Private { .. })),
        not_tracked: groups
            .iter()
            .filter(|g| matches!(g.state, GroupState::NotTracked { .. }))
            .count() as u32,
        parse_errors: groups
            .iter()
            .filter(|g| matches!(g.state, GroupState::ParseError { .. }))
            .count() as u32,
    }
}

#[test]
fn summarize_matches_independent_recount() {
    let groups = sample_groups();
    let s = summarize(&groups);
    assert_eq!(s, recount(&groups));
    // Pin the literal numbers too, so a shared mistake cannot hide.
    assert_eq!((s.major, s.minor, s.patch), (1, 1, 1));
    assert_eq!(s.vulnerable, 3);
    assert_eq!(s.vulnerable_by_severity.critical, 1);
    assert_eq!(s.vulnerable_by_severity.high, 1);
    assert_eq!(s.vulnerable_by_severity.low, 0);
    assert_eq!(s.vulnerable_by_severity.unknown, 1);
    assert_eq!(s.up_to_date, 2);
    assert_eq!((s.unknown, s.local, s.private), (1, 1, 1));
    assert_eq!((s.not_tracked, s.parse_errors), (1, 1));
}

#[test]
fn summarize_empty_is_all_zero() {
    assert_eq!(summarize(&[]), DepSummary::default());
}

#[test]
fn wire_shape_is_camel_case_with_kind_tags() {
    let groups = sample_groups();
    let report = DepReport {
        repo_id: "r1".into(),
        checked_at_ms: 1,
        summary: summarize(&groups),
        groups,
    };
    let v = serde_json::to_value(&report).unwrap();
    let g0 = &v["groups"][0];
    assert_eq!(g0["manifestPath"], "package.json");
    assert_eq!(g0["transitiveChecked"], 0);
    assert_eq!(g0["state"]["kind"], "ok");
    assert_eq!(g0["ecosystem"], "npm");
    let r0 = &g0["rows"][0];
    assert_eq!(r0["update"], "major");
    assert_eq!(r0["relation"], "direct");
    assert_eq!(r0["scope"], "normal");
    assert_eq!(r0["vulnsChecked"], true);
    assert_eq!(r0["availability"]["kind"], "checked");
    assert_eq!(g0["rows"][3]["vulns"][0]["severity"], "low");
    assert!(g0["rows"][3]["vulns"][0].get("cvssScore").is_some());
    assert_eq!(g0["rows"][6]["availability"]["kind"], "local");
    assert_eq!(g0["rows"][6]["availability"]["reason"], "path");
    let p = &g0["rows"][7]["availability"];
    assert_eq!(p["kind"], "private");
    assert_eq!(p["reason"], "scopedRegistry");
    assert_eq!(p["registryHost"], "npm.corp.invalid");
    assert_eq!(v["groups"][1]["state"]["kind"], "notTracked");
    assert_eq!(v["groups"][1]["state"]["label"], "Gradle");
    let pe = &v["groups"][2]["state"];
    assert_eq!(pe["kind"], "parseError");
    assert_eq!(pe["category"], "invalidSyntax");
    assert_eq!(v["summary"]["vulnerableBySeverity"]["critical"], 1);
    assert_eq!(v["summary"]["upToDate"], 2);
    assert_eq!(v["summary"]["parseErrors"], 1);
    assert_eq!(v["checkedAtMs"], 1);
    let back: DepReport = serde_json::from_value(v).unwrap();
    assert_eq!(back, report);
}

#[test]
fn private_reason_config_unreadable_wire_name() {
    let a = Availability::Private {
        reason: PrivateReason::ConfigUnreadable,
        registry_host: None,
    };
    let v = serde_json::to_value(&a).unwrap();
    assert_eq!(v["reason"], "configUnreadable");
    assert!(v["registryHost"].is_null());
}

#[test]
fn cached_report_roundtrips() {
    let c = CachedReport {
        schema: CACHE_SCHEMA,
        repo_id: "r".into(),
        stamps: vec![FileStamp {
            path: "package.json".into(),
            len: 10,
            mtime_ms: 5,
        }],
        report: DepReport {
            repo_id: "r".into(),
            checked_at_ms: 0,
            groups: vec![],
            summary: DepSummary::default(),
        },
    };
    let v = serde_json::to_value(&c).unwrap();
    assert_eq!(v["stamps"][0]["mtimeMs"], 5);
    let back: CachedReport = serde_json::from_value(v).unwrap();
    assert_eq!(back, c);
}
