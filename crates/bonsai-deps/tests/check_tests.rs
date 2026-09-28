//! `run_check` over the recorded P125d fixtures, offline (E4 AC2 / 4 / 5 and
//! the LOW-4 known-vulnerable set). No test here touches the network: the
//! `FakeTransport` serves `tests/fixtures/remote/` (see `live_record.rs`).

mod common;

use bonsai_deps::model::{
    Availability, DepReport, DepSummary, Ecosystem, GroupState, ManifestGroup, ParseErrorKind,
    PrivateReason, Relation, Severity, UpdateKind,
};
use bonsai_deps::registry_ctx::FakeHostEnv;
use bonsai_deps::remote::{FakeTransport, RawResponse, RecordedRequest};

use common::corpus::{self, corpus_repo, fixtures_dir};
use common::fixture::FixtureRepo;
use common::recorded_check_with;

fn offline() -> FakeTransport {
    FakeTransport::new().with_fixtures(&fixtures_dir())
}

fn report(r: &FixtureRepo, t: &FakeTransport) -> DepReport {
    match recorded_check_with(&r.wd, &FakeHostEnv::new(), t).report {
        Ok(c) => c.report,
        Err(e) => panic!("run_check: {e:?}"),
    }
}

fn group<'a>(rep: &'a DepReport, path: &str) -> &'a ManifestGroup {
    match rep.groups.iter().find(|g| g.manifest_path == path) {
        Some(g) => g,
        None => panic!("no group {path}"),
    }
}

/// An independent recount of `summarize` (AC2).
fn recount(groups: &[ManifestGroup]) -> DepSummary {
    let rows = || groups.iter().flat_map(|g| &g.rows);
    let n =
        |f: &dyn Fn(&bonsai_deps::model::DepRow) -> bool| rows().filter(|r| f(r)).count() as u32;
    let mut s = DepSummary {
        major: n(&|r| r.update == Some(UpdateKind::Major)),
        minor: n(&|r| r.update == Some(UpdateKind::Minor)),
        patch: n(&|r| r.update == Some(UpdateKind::Patch)),
        vulnerable: n(&|r| !r.vulns.is_empty()),
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
        ..DepSummary::default()
    };
    for r in rows() {
        let b = &mut s.vulnerable_by_severity;
        match r.max_severity() {
            Some(Severity::Critical) => b.critical += 1,
            Some(Severity::High) => b.high += 1,
            Some(Severity::Medium) => b.medium += 1,
            Some(Severity::Low) => b.low += 1,
            Some(Severity::Unknown) => b.unknown += 1,
            None => {}
        }
    }
    s
}

#[test]
fn npm_pnpm_plus_cargo_two_groups() {
    let r = FixtureRepo::new();
    r.track("web/package.json", corpus::PACKAGE_JSON)
        .track("web/pnpm-lock.yaml", corpus::PNPM_LOCK)
        .track("Cargo.toml", corpus::CARGO_TOML)
        .track("Cargo.lock", corpus::CARGO_LOCK);
    let rep = report(&r, &offline());
    let paths: Vec<&str> = rep
        .groups
        .iter()
        .map(|g| g.manifest_path.as_str())
        .collect();
    assert_eq!(
        paths,
        ["Cargo.toml", "web/package.json"],
        "root-level first"
    );
    assert_eq!(rep.summary, recount(&rep.groups));
    assert_eq!(rep.summary.major + rep.summary.minor + rep.summary.patch, 3);
    let lodash = &group(&rep, "web/package.json").rows[0];
    assert_eq!(lodash.latest.as_deref(), Some("4.18.1"));
    assert_eq!(lodash.update, Some(UpdateKind::Minor));
    assert_eq!(
        lodash.registry_url.as_deref(),
        Some("https://www.npmjs.com/package/lodash")
    );
}

#[test]
fn known_vulnerable_fixture_per_ecosystem() {
    // [LOW-4] one pinned known-vulnerable version per ecosystem, offline.
    let rep = report(&corpus_repo(), &offline());
    assert_eq!(rep.summary, recount(&rep.groups));
    for (path, name, id) in [
        ("web/package.json", "lodash", "GHSA-35jh-r3h4-6jhm"),
        ("Cargo.toml", "smallvec", "RUSTSEC-2021-0003"),
        ("py/requirements.txt", "requests", "PYSEC-2018-28"),
        ("go/go.mod", "github.com/gorilla/websocket", "GO-2020-0019"),
        (
            "dotnet/App/App.csproj",
            "Newtonsoft.Json",
            "GHSA-5crp-9r3c-p9vr",
        ),
    ] {
        let g = group(&rep, path);
        let row = g.rows.iter().find(|r| r.name == name);
        let Some(row) = row else {
            panic!("{path}: no {name} row")
        };
        assert!(row.vulns_checked, "{name}");
        assert!(
            row.vulns.iter().any(|a| a.id == id),
            "{name}: {:?}",
            row.vulns
        );
        assert!(row
            .vulns
            .iter()
            .all(|a| a.url == format!("https://osv.dev/vulnerability/{}", a.id)));
    }
    // NuGet: deps.dev folds the name to lowercase; still Checked.
    let nj = &group(&rep, "dotnet/App/App.csproj").rows[0];
    assert_eq!(nj.availability, Availability::Checked);
    assert_eq!(nj.update, Some(UpdateKind::Major));
}

#[test]
fn go_versions_are_sent_with_their_v_prefix() {
    // §0.1 (P125d probe): OSV accepts `v1.4.0` and `1.4.0` alike; the
    // pipeline sends the go.mod version as-is.
    let t = offline();
    report(&corpus_repo(), &t);
    let batch = t
        .requests()
        .into_iter()
        .find(|r| r.url.ends_with("/v1/querybatch"))
        .and_then(|r| r.body)
        .unwrap_or_default();
    assert!(
        batch.contains(r#""name":"github.com/gorilla/websocket""#),
        "{batch}"
    );
    assert!(batch.contains(r#""version":"v1.4.0""#), "{batch}");
    assert!(t
        .requests()
        .iter()
        .any(|r| r.url
            == "https://api.deps.dev/v3/systems/go/packages/github.com%2Fgorilla%2Fwebsocket"));
}

#[test]
fn transitive_only_when_vulnerable() {
    let rep = report(&corpus_repo(), &offline());
    let g = group(&rep, "Cargo.toml");
    let trans: Vec<&str> = g
        .rows
        .iter()
        .filter(|r| r.relation == Relation::Transitive)
        .map(|r| r.name.as_str())
        .collect();
    assert_eq!(trans, ["smallvec"], "itoa is clean ⇒ no row");
    assert_eq!(g.transitive_checked, 2, "smallvec + itoa were queried");
    let web = group(&rep, "web/package.json");
    assert_eq!(web.transitive_checked, 1);
    assert!(web.rows.iter().all(|r| r.relation == Relation::Direct));
}

#[test]
fn unknown_row_keeps_vulns() {
    // [UI-e] deps.dev 404 ⇒ Unknown, but OSV is still asked.
    let t = offline().with_responder(Box::new(|r: &RecordedRequest| {
        r.url.contains("api.deps.dev").then(|| {
            Ok(RawResponse {
                status: 404,
                body: Vec::new(),
            })
        })
    }));
    let rep = report(&corpus_repo(), &t);
    let lodash = &group(&rep, "web/package.json").rows[0];
    assert_eq!(lodash.availability, Availability::Unknown);
    assert_eq!(lodash.latest, None);
    assert!(!lodash.vulns.is_empty());
    assert!(
        lodash.registry_url.is_some(),
        "Unknown rows keep their link"
    );
    assert_eq!(rep.summary, recount(&rep.groups));
}

#[test]
fn advisory_severity_mapping() {
    let rep = report(&corpus_repo(), &offline());
    let adv = |id: &str| {
        rep.groups
            .iter()
            .flat_map(|g| &g.rows)
            .flat_map(|r| &r.vulns)
            .find(|a| a.id == id)
            .cloned()
    };
    let sev = |id: &str| adv(id).map(|a| (a.severity, a.cvss_score));
    // GHSA: database_specific.severity wins; CVSS 3.1 PR:H vector = 7.2.
    assert_eq!(
        sev("GHSA-35jh-r3h4-6jhm"),
        Some((Severity::High, Some(7.2)))
    );
    // RUSTSEC: no db severity ⇒ the CVSS_V3 band (9.8 ⇒ Critical).
    assert_eq!(
        sev("RUSTSEC-2021-0003"),
        Some((Severity::Critical, Some(9.8)))
    );
    // GO- / PYSEC- records without severity data ⇒ Unknown.
    assert_eq!(sev("GO-2020-0019"), Some((Severity::Unknown, None)));
    assert_eq!(sev("PYSEC-2018-28"), Some((Severity::Unknown, None)));
    let a = adv("GHSA-35jh-r3h4-6jhm").map(|a| (a.summary, a.aliases));
    assert_eq!(
        a,
        Some((
            Some("Command Injection in lodash".to_string()),
            vec![
                "CVE-2021-23337".to_string(),
                "CVE-2026-4800".to_string(),
                "GHSA-r5fr-rjxr-66jc".to_string()
            ]
        ))
    );
    // Rows: severity desc, then id asc.
    let req = &group(&rep, "py/requirements.txt").rows[0].vulns;
    let ranks: Vec<u8> = req.iter().map(|a| a.severity.rank()).collect();
    assert!(ranks.windows(2).all(|w| w[0] >= w[1]), "{ranks:?}");
}

#[test]
fn yanked_versions_are_never_latest() {
    // §3.3 probe: deps.dev marks crates.io / PyPI yanks isDeprecated.
    let rep = report(&corpus_repo(), &offline());
    let tokio = group(&rep, "Cargo.toml")
        .rows
        .iter()
        .find(|r| r.name == "tokio");
    assert_eq!(tokio.and_then(|r| r.latest.as_deref()), Some("1.53.1"));
    for (file, version) in [
        ("cargo/tokio.json", "1.39.0"),
        ("pypi/requests.json", "2.32.0"),
    ] {
        let text =
            std::fs::read_to_string(fixtures_dir().join("depsdev").join(file)).unwrap_or_default();
        let v: serde_json::Value = serde_json::from_str(&text).unwrap_or_default();
        let hit = v["versions"]
            .as_array()
            .into_iter()
            .flatten()
            .find(|x| x["versionKey"]["version"] == version);
        let Some(hit) = hit else {
            panic!("{file}: {version} missing")
        };
        assert_eq!(hit["isDeprecated"], true, "{file} {version}");
        assert_eq!(hit["isDefault"], false, "{file} {version}");
    }
}

#[test]
fn not_tracked_gradle_isolated_and_a_bad_lock_is_its_own_group() {
    let r = corpus_repo();
    r.track("android/build.gradle", "plugins { id 'java' }\n")
        .track("Cargo.lock", "version = 4\n[[package]\nname = \"x\"\n");
    let rep = report(&r, &offline());
    let gradle = group(&rep, "android/build.gradle");
    assert_eq!(gradle.ecosystem, None);
    assert_eq!(
        gradle.state,
        GroupState::NotTracked {
            label: "Gradle".into()
        }
    );
    let cargo = group(&rep, "Cargo.toml");
    let GroupState::ParseError {
        file,
        category,
        message,
        ..
    } = &cargo.state
    else {
        panic!("{:?}", cargo.state)
    };
    assert_eq!(
        (file.as_str(), *category),
        ("Cargo.lock", ParseErrorKind::InvalidSyntax)
    );
    assert!(message.starts_with("invalid syntax"), "{message}");
    assert!(cargo.rows.is_empty());
    // Every other group is unaffected.
    assert!(group(&rep, "web/package.json")
        .rows
        .iter()
        .any(|r| !r.vulns.is_empty()));
    assert_eq!(rep.summary.parse_errors, 1);
    assert_eq!(rep.summary.not_tracked, 1);
}

#[test]
fn display_strings_are_cleaned_and_capped() {
    // [LOW-3] a bidi override + zero-width char in a repo-derived name, and
    // a 300-char name, reach the row cleaned and capped at 256.
    let long = "a".repeat(300);
    let evil: String = ['e', 'v', 'i', 'l', '\u{202E}', 'g', 'n', 'p', '\u{200B}']
        .iter()
        .collect();
    let manifest = serde_json::json!({ "dependencies": { evil: "1.0.0", long: "1.0.0" } });
    let r = FixtureRepo::new();
    r.track("package.json", &manifest.to_string());
    let rep = report(&r, &FakeTransport::new());
    let names: Vec<&str> = rep.groups[0].rows.iter().map(|r| r.name.as_str()).collect();
    assert!(names.contains(&"evilgnp"), "{names:?}");
    assert!(names.iter().all(|n| n.chars().count() <= 256));
    assert!(rep.groups[0].rows.iter().all(|r| matches!(
        r.availability,
        Availability::Private {
            reason: PrivateReason::CustomRegistry,
            ..
        }
    )));
}

#[test]
fn ecosystems_of_the_corpus() {
    let rep = report(&corpus_repo(), &offline());
    let ecos: Vec<Option<Ecosystem>> = rep.groups.iter().map(|g| g.ecosystem).collect();
    assert_eq!(
        ecos,
        [
            Some(Ecosystem::Cargo),
            Some(Ecosystem::Nuget),
            Some(Ecosystem::Go),
            Some(Ecosystem::Pypi),
            Some(Ecosystem::Npm)
        ]
    );
}
