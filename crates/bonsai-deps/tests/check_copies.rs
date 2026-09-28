//! `[P125d review]` second copies of a direct dependency and NuGet id
//! casing (MEDIUM-D1), offline over the recorded fixtures.

mod common;

use bonsai_deps::model::{DepReport, DepRow, Relation};
use bonsai_deps::registry_ctx::FakeHostEnv;
use bonsai_deps::remote::FakeTransport;

use common::corpus::fixtures_dir;
use common::fixture::FixtureRepo;
use common::recorded_check_with;

fn report(r: &FixtureRepo) -> (DepReport, FakeTransport) {
    let t = FakeTransport::new().with_fixtures(&fixtures_dir());
    match recorded_check_with(&r.wd, &FakeHostEnv::new(), &t).report {
        Ok(c) => (c.report, t),
        Err(e) => panic!("run_check: {e:?}"),
    }
}

fn rows<'a>(rep: &'a DepReport, name: &str) -> Vec<&'a DepRow> {
    rep.groups
        .iter()
        .flat_map(|g| &g.rows)
        .filter(|r| r.name == name)
        .collect()
}

#[test]
fn a_vulnerable_second_npm_copy_is_a_transitive_row() {
    // Direct lodash 4.17.21 (no recorded advisories) + a nested 4.17.20.
    let r = FixtureRepo::new();
    r.track(
        "package.json",
        r#"{"dependencies": {"lodash": "^4.17.21"}}"#,
    )
    .track(
        "pnpm-lock.yaml",
        "lockfileVersion: '9.0'\n\nimporters:\n\n  .:\n    dependencies:\n      lodash:\n        specifier: ^4.17.21\n        version: 4.17.21\n\npackages:\n\n  lodash@4.17.21:\n    resolution: {integrity: sha512-a}\n\n  lodash@4.17.20:\n    resolution: {integrity: sha512-b}\n",
    );
    let (rep, _) = report(&r);
    let lodash = rows(&rep, "lodash");
    let direct = lodash.iter().find(|r| r.relation == Relation::Direct);
    let trans = lodash.iter().find(|r| r.relation == Relation::Transitive);
    assert_eq!(direct.and_then(|r| r.current.as_deref()), Some("4.17.21"));
    let Some(trans) = trans else {
        panic!("the vulnerable copy was dropped: {lodash:?}")
    };
    assert_eq!(trans.current.as_deref(), Some("4.17.20"));
    assert!(trans.vulns.iter().any(|a| a.id == "GHSA-35jh-r3h4-6jhm"));
    assert_eq!(rep.groups[0].transitive_checked, 1);
}

#[test]
fn a_vulnerable_second_cargo_copy_is_a_transitive_row() {
    // Direct smallvec 1.13.2 + smallvec 1.6.0 pulled in by another crate.
    let src = "source = \"registry+https://github.com/rust-lang/crates.io-index\"\n";
    let r = FixtureRepo::new();
    r.track(
        "Cargo.toml",
        "[package]\nname = \"p\"\nversion = \"0.1.0\"\n[dependencies]\nsmallvec = \"1.13\"\n",
    )
    .track(
        "Cargo.lock",
        &format!(
            "version = 4\n[[package]]\nname = \"p\"\nversion = \"0.1.0\"\ndependencies = [\"smallvec 1.13.2\"]\n\
             [[package]]\nname = \"smallvec\"\nversion = \"1.13.2\"\n{src}\
             [[package]]\nname = \"smallvec\"\nversion = \"1.6.0\"\n{src}"
        ),
    );
    let (rep, _) = report(&r);
    let sv = rows(&rep, "smallvec");
    let direct = sv.iter().find(|r| r.relation == Relation::Direct);
    assert_eq!(direct.and_then(|r| r.current.as_deref()), Some("1.13.2"));
    let trans = sv.iter().find(|r| r.relation == Relation::Transitive);
    let Some(trans) = trans else {
        panic!("the vulnerable copy was dropped: {sv:?}")
    };
    assert_eq!(trans.current.as_deref(), Some("1.6.0"));
    assert!(trans.vulns.iter().any(|a| a.id == "RUSTSEC-2021-0003"));
}

const CSPROJ_LOWER: &str = r#"<Project Sdk="Microsoft.NET.Sdk">
  <ItemGroup>
    <PackageReference Include="newtonsoft.json" Version="12.0.3" />
  </ItemGroup>
</Project>
"#;

/// The shape `dotnet restore` wrote for `CSPROJ_LOWER` (P125d probe, .NET
/// 10.0.303): the key is the canonical nuspec casing.
const LOCK_CANONICAL: &str = r#"{
  "version": 1,
  "dependencies": {
    "net10.0": {
      "Newtonsoft.Json": { "type": "Direct", "requested": "[12.0.3, )", "resolved": "12.0.3" }
    }
  }
}"#;

#[test]
fn a_lowercase_nuget_id_without_a_lockfile_is_never_reported_clean() {
    // [MEDIUM-D1] OSV is case-sensitive for NuGet: `newtonsoft.json` gets
    // no hits, so the row must read "not verified", not "no vulns".
    let r = FixtureRepo::new();
    r.track("App/App.csproj", CSPROJ_LOWER);
    let (rep, t) = report(&r);
    let row = rows(&rep, "newtonsoft.json");
    assert_eq!(row.len(), 1, "{:?}", rep.groups);
    assert!(row[0].vulns.is_empty());
    assert!(!row[0].vulns_checked, "zero hits in an unverified casing");
    // Still queried as written (and deps.dev still asked, UI-e).
    assert!(t.requests().iter().any(|q| q
        .body
        .as_deref()
        .is_some_and(|b| b.contains("newtonsoft.json"))));
}

#[test]
fn the_lockfile_casing_is_what_osv_is_asked() {
    let r = FixtureRepo::new();
    r.track("App/App.csproj", CSPROJ_LOWER)
        .track("App/packages.lock.json", LOCK_CANONICAL);
    let (rep, _) = report(&r);
    let row = rows(&rep, "Newtonsoft.Json");
    assert_eq!(row.len(), 1, "{:?}", rep.groups);
    assert!(row[0].vulns_checked);
    assert!(row[0].vulns.iter().any(|a| a.id == "GHSA-5crp-9r3c-p9vr"));
}

#[test]
fn every_row_with_vulns_checked_and_no_hits_is_in_a_verified_casing() {
    // (c) across the full corpus + the lowercase project.
    let r = common::corpus::corpus_repo();
    r.track("lower/App.csproj", CSPROJ_LOWER);
    let (rep, _) = report(&r);
    for row in rep.groups.iter().flat_map(|g| &g.rows) {
        if row.vulns_checked && row.vulns.is_empty() {
            assert_ne!(row.name, "newtonsoft.json", "{row:?}");
        }
    }
}
