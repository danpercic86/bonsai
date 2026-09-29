//! P125h §1 / §2 over `run_check` (contract `P125-followups.md`): a
//! malformed OSV id makes its package "not verified" instead of clean (R2
//! for transitives), and an oversize deps.dev body / advisory record
//! degrades only its package, under the `MAX_DEGRADED_RESPONSES` breaker.
//! Offline: every response comes from a `FakeTransport` responder.

mod common;

use bonsai_deps::check::{ServiceFault, MAX_DEGRADED_RESPONSES};
use bonsai_deps::model::{
    Availability, CachedReport, DepReport, DepRow, ManifestGroup, Relation, Severity,
    UnknownReason, VulnsUnverifiedReason as U,
};
use bonsai_deps::registry_ctx::FakeHostEnv;
use bonsai_deps::remote::{FakeTransport, Method, RawResponse, RecordedRequest, TransportError};
use bonsai_deps::{run_check, CancelToken, CheckCtx, CheckError};

use common::corpus::{corpus_repo, fixtures_dir};
use common::fixture::FixtureRepo;

const CRATES_IO: &str = "source = \"registry+https://github.com/rust-lang/crates.io-index\"\n";
const DEPSDEV: &str = "https://api.deps.dev/v3/systems/cargo/packages/";
const VULNS: &str = "https://api.osv.dev/v1/vulns/";

type Reply = Option<Result<RawResponse, TransportError>>;

/// A Cargo repo: `direct` crates are dependencies, `trans` lock-only
/// entries; every crate is `1.0.0` from crates.io.
fn cargo_repo(direct: &[&str], trans: &[&str]) -> FixtureRepo {
    let deps: String = direct.iter().map(|d| format!("{d} = \"1\"\n")).collect();
    let lock: String = direct
        .iter()
        .chain(trans)
        .map(|n| format!("[[package]]\nname = \"{n}\"\nversion = \"1.0.0\"\n{CRATES_IO}"))
        .collect();
    let r = FixtureRepo::new();
    r.track(
        "Cargo.toml",
        &format!("[package]\nname = \"p\"\nversion = \"0.1.0\"\n[dependencies]\n{deps}"),
    )
    .track("Cargo.lock", &format!("version = 4\n{lock}"));
    r
}

/// Querybatch results come from `hits(name)` (sized to the request);
/// every other request goes to `other` (`None` ⇒ the fake's defaults:
/// deps.dev 404, advisory 404).
fn transport(
    hits: impl Fn(&str) -> serde_json::Value + Send + Sync + 'static,
    other: impl Fn(&RecordedRequest) -> Reply + Send + Sync + 'static,
) -> FakeTransport {
    FakeTransport::new().with_responder(Box::new(move |r: &RecordedRequest| {
        if r.method != Method::Post {
            return other(r);
        }
        let body: serde_json::Value = serde_json::from_str(r.body.as_deref()?).ok()?;
        let results: Vec<serde_json::Value> = body["queries"]
            .as_array()?
            .iter()
            .map(|q| hits(q["package"]["name"].as_str().unwrap_or("")))
            .collect();
        Some(Ok(RawResponse {
            status: 200,
            body: serde_json::json!({ "results": results })
                .to_string()
                .into_bytes(),
        }))
    }))
}

fn vulns(ids: &[&str]) -> serde_json::Value {
    let v: Vec<serde_json::Value> = ids
        .iter()
        .map(|id| serde_json::json!({"id": id, "modified": "m"}))
        .collect();
    serde_json::json!({ "vulns": v })
}

fn too_large() -> Reply {
    Some(Err(TransportError::TooLarge))
}

fn record() -> Reply {
    Some(Ok(RawResponse {
        status: 200,
        body: br#"{"summary":"real record","database_specific":{"severity":"HIGH"}}"#.to_vec(),
    }))
}

fn run_with(
    r: &FixtureRepo,
    t: &FakeTransport,
    prev: Option<&DepReport>,
) -> Result<CachedReport, CheckError> {
    let cancel = CancelToken::new();
    let ctx = CheckCtx {
        transport: t,
        env: &FakeHostEnv::new(),
        cancel: &cancel,
        progress: &|_, _| {},
        prev,
        now_ms: 7,
    };
    run_check(&r.wd, "rid", &ctx)
}

/// The report, with the §0 row invariant checked on every row.
fn report(r: &FixtureRepo, t: &FakeTransport, prev: Option<&DepReport>) -> DepReport {
    let rep = match run_with(r, t, prev) {
        Ok(c) => c.report,
        Err(e) => panic!("run_check: {e:?}"),
    };
    for row in rep.groups.iter().flat_map(|g| &g.rows) {
        assert!(row.verdict_consistent(), "§0 invariant: {row:?}");
    }
    rep
}

fn cargo(rep: &DepReport) -> &ManifestGroup {
    match rep.groups.iter().find(|g| g.manifest_path == "Cargo.toml") {
        Some(g) => g,
        None => panic!("no Cargo group"),
    }
}

fn row<'a>(rep: &'a DepReport, name: &str) -> &'a DepRow {
    match cargo(rep).rows.iter().find(|r| r.name == name) {
        Some(r) => r,
        None => panic!("no row {name}"),
    }
}

fn ids(r: &DepRow) -> Vec<&str> {
    r.vulns.iter().map(|a| a.id.as_str()).collect()
}

fn vuln_gets(t: &FakeTransport) -> Vec<String> {
    t.requests()
        .into_iter()
        .filter_map(|r| r.url.strip_prefix(VULNS).map(str::to_string))
        .collect()
}

#[test]
fn all_malformed_direct_is_not_verified() {
    let t = transport(
        |n| match n {
            "a" => vulns(&["../x", "a b"]),
            _ => serde_json::json!({}),
        },
        |_| None,
    );
    let rep = report(&cargo_repo(&["a", "b"], &[]), &t, None);
    let a = row(&rep, "a");
    assert!(!a.vulns_checked, "never 'no vulnerabilities'");
    assert_eq!(a.vulns_unverified, Some(U::MalformedAdvisory));
    assert!(a.vulns.is_empty());
    let b = row(&rep, "b");
    assert!(b.vulns_checked && b.vulns_unverified.is_none(), "control");
    assert!(vuln_gets(&t).is_empty(), "a dropped id is never fetched");
}

#[test]
fn partial_malformed_keeps_good_advisories_and_is_not_verified() {
    let t = transport(
        |n| match n {
            "a" => vulns(&["../x", "GHSA-good"]),
            _ => serde_json::json!({}),
        },
        |_| None,
    );
    let rep = report(&cargo_repo(&["a"], &[]), &t, None);
    let a = row(&rep, "a");
    assert_eq!(ids(a), ["GHSA-good"]);
    assert!(!a.vulns_checked);
    assert_eq!(a.vulns_unverified, Some(U::MalformedAdvisory));
    assert_eq!(rep.summary.vulnerable, 1);
    assert_eq!(vuln_gets(&t), ["GHSA-good"]);
}

#[test]
fn all_malformed_transitive_row_is_emitted_unverified_and_not_counted() {
    // R2 (ruled 2026-09-29): shown as a "not verified" transitive row with
    // empty vulns, excluded from `transitive_checked`.
    let t = transport(
        |n| match n {
            "t1" => vulns(&["a b"]),
            _ => serde_json::json!({}),
        },
        |_| None,
    );
    let rep = report(&cargo_repo(&["a"], &["t1", "t2"]), &t, None);
    let g = cargo(&rep);
    let trans: Vec<&DepRow> = g
        .rows
        .iter()
        .filter(|r| r.relation == Relation::Transitive)
        .collect();
    assert_eq!(trans.len(), 1, "t2 is clean ⇒ no row");
    let t1 = trans[0];
    assert_eq!(t1.name, "t1");
    assert!(t1.vulns.is_empty());
    assert!(!t1.vulns_checked);
    assert_eq!(t1.vulns_unverified, Some(U::MalformedAdvisory));
    assert_eq!(t1.availability, Availability::Checked);
    assert_eq!(g.transitive_checked, 1, "only t2 was verified");
    assert_eq!(rep.summary.vulnerable, 0, "shown, not counted");
}

#[test]
fn one_oversize_package_degrades_only_it() {
    // The real 8 MiB cap, enforced by the fake, on lodash only.
    let big = format!(
        "{{\"versions\":[],\"pad\":\"{}\"}}",
        "x".repeat(8 * 1024 * 1024)
    )
    .into_bytes();
    let t = FakeTransport::new()
        .with_fixtures(&fixtures_dir())
        .with_responder(Box::new(move |r: &RecordedRequest| {
            r.url.ends_with("/npm/packages/lodash").then(|| {
                Ok(RawResponse {
                    status: 200,
                    body: big.clone(),
                })
            })
        }));
    let rep = report(&corpus_repo(), &t, None);
    let rows: Vec<&DepRow> = rep.groups.iter().flat_map(|g| &g.rows).collect();
    let lodash = rows.iter().find(|r| r.name == "lodash");
    let Some(lodash) = lodash else {
        panic!("no lodash row")
    };
    assert_eq!(
        lodash.availability,
        Availability::Unknown {
            reason: UnknownReason::TooLarge
        }
    );
    assert_eq!(lodash.latest, None);
    assert!(!lodash.vulns.is_empty(), "OSV is still asked");
    assert!(lodash.vulns_checked);
    let others: Vec<&&DepRow> = rows
        .iter()
        .filter(|r| r.relation == Relation::Direct && r.name != "lodash")
        .collect();
    assert!(!others.is_empty());
    assert!(
        others
            .iter()
            .all(|r| r.availability == Availability::Checked),
        "{others:?}"
    );
}

#[test]
fn oversize_advisory_marks_rows_response_too_large() {
    let t = transport(
        |n| match n {
            "a" => vulns(&["GHSA-big"]),
            "b" => vulns(&["GHSA-big", "GHSA-ok"]),
            "c" => vulns(&["GHSA-ok"]),
            // A dropped id AND an oversize record ⇒ MalformedAdvisory wins.
            "d" => vulns(&["a b", "GHSA-big"]),
            _ => serde_json::json!({}),
        },
        |r| {
            if r.url.ends_with("/GHSA-big") {
                too_large()
            } else if r.url.ends_with("/GHSA-ok") {
                record()
            } else {
                None
            }
        },
    );
    let rep = report(&cargo_repo(&["a", "b", "c", "d"], &[]), &t, None);
    for (name, want) in [
        ("a", Some(U::ResponseTooLarge)),
        ("b", Some(U::ResponseTooLarge)),
        ("c", None),
        ("d", Some(U::MalformedAdvisory)),
    ] {
        let r = row(&rep, name);
        assert_eq!(r.vulns_unverified, want, "{name}");
        assert_eq!(r.vulns_checked, want.is_none(), "{name}");
    }
    let a = row(&rep, "a");
    assert_eq!(ids(a), ["GHSA-big"], "the placeholder is shown");
    assert_eq!(a.vulns[0].severity, Severity::Unknown);
    assert_eq!(a.vulns[0].summary, None);
    assert_eq!(ids(row(&rep, "b")), ["GHSA-ok", "GHSA-big"]);
    let mut gets = vuln_gets(&t);
    gets.sort();
    assert_eq!(gets, ["GHSA-big", "GHSA-ok"], "each id fetched once");
}

/// `n_depsdev` deps.dev bodies and `n_vulns` advisory records over the cap.
fn degraded(n_depsdev: usize, n_vulns: usize) -> Result<CachedReport, CheckError> {
    let names: Vec<String> = (0..n_depsdev + n_vulns).map(|i| format!("c{i}")).collect();
    let refs: Vec<&str> = names.iter().map(String::as_str).collect();
    let t = transport(
        move |n| {
            let i: usize = n.trim_start_matches('c').parse().unwrap_or(0);
            if i >= n_depsdev {
                vulns(&[&format!("GHSA-big-{i}")])
            } else {
                serde_json::json!({})
            }
        },
        move |r| {
            let depsdev = r
                .url
                .strip_prefix(DEPSDEV)
                .and_then(|n| n.trim_start_matches('c').parse::<usize>().ok());
            match depsdev {
                Some(i) if i < n_depsdev => too_large(),
                Some(_) => None,
                None => r
                    .url
                    .contains("/GHSA-big-")
                    .then_some(Err(TransportError::TooLarge)),
            }
        },
    );
    run_with(&cargo_repo(&refs, &[]), &t, None)
}

#[test]
fn breaker_aborts_after_32_degraded() {
    assert_eq!(MAX_DEGRADED_RESPONSES, 32);
    let abort = Some(CheckError::Service(ServiceFault::TooLarge));
    // 33 deps.dev bodies ⇒ abort; 32 ⇒ a report of Unknown{TooLarge} rows.
    assert_eq!(degraded(33, 0).err(), abort);
    let ok = degraded(32, 0).map(|c| c.report);
    let Ok(rep) = ok else { panic!("{ok:?}") };
    assert_eq!(cargo(&rep).rows.len(), 32);
    assert!(cargo(&rep).rows.iter().all(|r| r.availability
        == Availability::Unknown {
            reason: UnknownReason::TooLarge
        }));
    // deps.dev and advisory responses share ONE counter.
    assert_eq!(degraded(20, 13).err(), abort);
    assert!(degraded(20, 12).is_ok());
}

#[test]
fn querybatch_too_large_still_aborts() {
    let t = FakeTransport::new().with_responder(Box::new(|r: &RecordedRequest| {
        (r.method == Method::Post).then_some(Err(TransportError::TooLarge))
    }));
    assert_eq!(
        run_with(&cargo_repo(&["a"], &[]), &t, None).err(),
        Some(CheckError::Service(ServiceFault::TooLarge))
    );
}

#[test]
fn too_large_placeholder_is_refetched_next_run() {
    let hits = |n: &str| match n {
        "a" => vulns(&["GHSA-big"]),
        "b" => vulns(&["GHSA-ok"]),
        _ => serde_json::json!({}),
    };
    let repo = cargo_repo(&["a", "b"], &[]);
    let t1 = transport(hits, |r| {
        if r.url.ends_with("/GHSA-big") {
            too_large()
        } else {
            record()
        }
    });
    let first = report(&repo, &t1, None);
    assert_eq!(row(&first, "a").vulns_unverified, Some(U::ResponseTooLarge));
    // Run 2: same `modified`, the record now fits. The placeholder is NOT
    // reused; the verified row's advisory still is.
    let t2 = transport(hits, |_| record());
    let second = report(&repo, &t2, Some(&first));
    assert_eq!(vuln_gets(&t2), ["GHSA-big"]);
    let a = row(&second, "a");
    assert!(a.vulns_checked && a.vulns_unverified.is_none());
    assert_eq!(a.vulns[0].severity, Severity::High);
    assert_eq!(a.vulns[0].summary.as_deref(), Some("real record"));
}

/// `[P125h §2 reuse guard]` the mixed case: a row carrying BOTH a dropped
/// id and an oversize placeholder is tagged `MalformedAdvisory`, not
/// `ResponseTooLarge` — its placeholder must still be re-fetched next run
/// (the guard skips every advisory of any unverified prev row).
#[test]
fn malformed_row_placeholder_is_refetched_next_run() {
    let hits = |n: &str| match n {
        "d" => vulns(&["a b", "GHSA-big"]),
        "e" => vulns(&["GHSA-ok"]),
        _ => serde_json::json!({}),
    };
    let repo = cargo_repo(&["d", "e"], &[]);
    let t1 = transport(hits, |r| {
        if r.url.ends_with("/GHSA-big") {
            too_large()
        } else {
            record()
        }
    });
    let first = report(&repo, &t1, None);
    let d1 = row(&first, "d");
    assert_eq!(d1.vulns_unverified, Some(U::MalformedAdvisory));
    assert_eq!(d1.vulns[0].severity, Severity::Unknown, "the placeholder");
    // Run 2: same `modified`, the record now fits.
    let t2 = transport(hits, |_| record());
    let second = report(&repo, &t2, Some(&first));
    assert_eq!(
        vuln_gets(&t2),
        ["GHSA-big"],
        "the placeholder is re-fetched; the verified row's advisory is reused"
    );
    let d2 = row(&second, "d");
    assert_eq!(
        d2.vulns_unverified,
        Some(U::MalformedAdvisory),
        "still a dropped id"
    );
    assert_eq!(ids(d2), ["GHSA-big"]);
    assert_eq!(d2.vulns[0].severity, Severity::High);
    assert_eq!(d2.vulns[0].summary.as_deref(), Some("real record"));
    assert!(row(&second, "e").vulns_checked);
}
