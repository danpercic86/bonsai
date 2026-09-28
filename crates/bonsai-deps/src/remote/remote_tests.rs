use super::depsdev::{map_package, MAX_VERSIONS};
use super::osv::VulnRef;
use super::osv_vuln::map_vuln;
use super::transport::{pct_encode, OutboundRequest};
use super::*;
use crate::check::error::{CheckError, ServiceFault};
use crate::model::{Ecosystem, Severity};
use crate::privacy::PublicPackage;

fn pkg(eco: Ecosystem, name: &str, version: Option<&str>) -> PublicPackage {
    crate::test_pkg::public(eco, name, version)
}

#[test]
fn urls_are_fixed_and_percent_encoded() {
    let cases = [
        (
            Ecosystem::Npm,
            "@types/node",
            "npm/packages/%40types%2Fnode",
        ),
        (Ecosystem::Cargo, "serde_json", "cargo/packages/serde_json"),
        (
            Ecosystem::Pypi,
            "Zope.Interface",
            "pypi/packages/zope-interface",
        ),
        (
            Ecosystem::Go,
            "golang.org/x/text",
            "go/packages/golang.org%2Fx%2Ftext",
        ),
        (
            Ecosystem::Nuget,
            "Newtonsoft.Json",
            "nuget/packages/Newtonsoft.Json",
        ),
    ];
    for (eco, name, tail) in cases {
        let r = OutboundRequest::depsdev_package(&pkg(eco, name, None));
        assert_eq!(r.method(), Method::Get);
        assert_eq!(r.url(), format!("https://api.deps.dev/v3/systems/{tail}"));
        assert!(r.body().is_none());
    }
    assert_eq!(pct_encode("a b/%@?#"), "a%20b%2F%25%40%3F%23");
}

#[test]
fn querybatch_body_has_only_the_contract_fields() {
    let q = [
        pkg(Ecosystem::Npm, "lodash", Some("4.17.20")),
        pkg(Ecosystem::Cargo, "serde", Some("1.0.0")),
    ];
    let r = OutboundRequest::osv_querybatch(&q, &[]);
    assert_eq!(r.method(), Method::Post);
    assert_eq!(r.url(), "https://api.osv.dev/v1/querybatch");
    let v: serde_json::Value = serde_json::from_str(r.body().unwrap_or("")).unwrap_or_default();
    assert_eq!(
        v,
        serde_json::json!({"queries": [
            {"package": {"name": "lodash", "ecosystem": "npm"}, "version": "4.17.20"},
            {"package": {"name": "serde", "ecosystem": "crates.io"}, "version": "1.0.0"},
        ]})
    );
}

#[test]
fn depsdev_mapping() {
    let p = pkg(Ecosystem::Nuget, "Newtonsoft.Json", None);
    let body = br#"{"packageKey":{"name":"newtonsoft.json"},"versions":[
        {"versionKey":{"name":"newtonsoft.json","version":"13.0.4"},"isDefault":true,"links":[{"x":1}]},
        {"versionKey":{"name":"newtonsoft.json","version":"13.0.2"},"isDeprecated":true,"deprecatedReason":"zz"}]}"#;
    let v = map_package(&p, 200, body)
        .ok()
        .flatten()
        .unwrap_or_default();
    assert_eq!(v.len(), 2, "NuGet names compare case-insensitively");
    assert!(v[0].is_default && !v[0].is_deprecated && v[1].is_deprecated);
    // A different package in the response ⇒ Unknown.
    let other = br#"{"versions":[{"versionKey":{"name":"evil","version":"1"}}]}"#;
    assert!(matches!(map_package(&p, 200, other), Ok(None)));
    assert!(matches!(map_package(&p, 404, b""), Ok(None)));
    // PyPI: deps.dev normalises (`Django` ⇒ `django`).
    let dj = pkg(Ecosystem::Pypi, "Django", None);
    let body = br#"{"versions":[{"versionKey":{"name":"django","version":"5.0"}}]}"#;
    assert!(matches!(map_package(&dj, 200, body), Ok(Some(_))));
    // npm is exact.
    let lp = pkg(Ecosystem::Npm, "left-pad", None);
    let body = br#"{"versions":[{"versionKey":{"name":"Left-Pad","version":"1"}}]}"#;
    assert!(matches!(map_package(&lp, 200, body), Ok(None)));
}

#[test]
fn depsdev_version_cap() {
    let p = pkg(Ecosystem::Npm, "x", None);
    let one = r#"{"versionKey":{"name":"x","version":"1"}}"#;
    let body = format!(
        "{{\"versions\":[{}]}}",
        vec![one; MAX_VERSIONS + 1].join(",")
    );
    assert_eq!(
        map_package(&p, 200, body.as_bytes()).err(),
        Some(CheckError::Service(ServiceFault::TooLarge))
    );
    let body = format!("{{\"versions\":[{}]}}", vec![one; MAX_VERSIONS].join(","));
    assert!(matches!(map_package(&p, 200, body.as_bytes()), Ok(Some(_))));
}

fn vref() -> VulnRef {
    // Mint through the real response path.
    let t = FakeTransport::new().with_responder(Box::new(|_| {
        Some(Ok(RawResponse {
            status: 200,
            body: br#"{"results":[{"vulns":[{"id":"GHSA-aaaa-bbbb-cccc","modified":"2026-01-01T00:00:00Z"}]}]}"#.to_vec(),
        }))
    }));
    let q = [pkg(Ecosystem::Npm, "x", Some("1.0.0"))];
    let hits = osv::query_chunk(&t, &q, &|| false).ok().unwrap_or_default();
    hits.into_iter()
        .flatten()
        .next()
        .unwrap_or_else(|| panic!("no hit"))
}

#[test]
fn vuln_mapping_reads_only_the_listed_fields() {
    let r = vref();
    let body = serde_json::json!({
        "id": "IGNORED",
        "summary": "  Bad \u{202E}thing\n ",
        "aliases": ["CVE-2021-1", "../evil", "a b"],
        "severity": [
            {"type": "CVSS_V3", "score": "CVSS:3.1/AV:N/AC:L/PR:N/UI:N/S:U/C:N/I:N/A:L"},
            {"type": "CVSS_V3", "score": "CVSS:3.1/AV:N/AC:L/PR:N/UI:N/S:U/C:H/I:H/A:H"},
            {"type": "CVSS_V4", "score": "CVSS:4.0/AV:N"}
        ],
        "database_specific": {"url": "https://evil.invalid/"}
    });
    let a = map_vuln(&r, 200, body.to_string().as_bytes()).unwrap_or_else(|e| panic!("{e:?}"));
    assert_eq!(a.id, "GHSA-aaaa-bbbb-cccc");
    assert_eq!(a.url, "https://osv.dev/vulnerability/GHSA-aaaa-bbbb-cccc");
    assert_eq!(a.summary.as_deref(), Some("Bad thing"));
    assert_eq!(a.aliases, ["CVE-2021-1"]);
    assert_eq!((a.severity, a.cvss_score), (Severity::Critical, Some(9.8)));
    assert_eq!(
        a.modified, "2026-01-01T00:00:00Z",
        "the querybatch reuse key"
    );
    let ghsa = serde_json::json!({"database_specific": {"severity": "MODERATE"},
        "severity": [{"type": "CVSS_V3", "score": "CVSS:3.1/AV:N/AC:L/PR:N/UI:N/S:U/C:H/I:H/A:H"}]});
    let a = map_vuln(&r, 200, ghsa.to_string().as_bytes()).unwrap_or_else(|e| panic!("{e:?}"));
    assert_eq!(a.severity, Severity::Medium, "db severity wins over CVSS");
    let missing = map_vuln(&r, 404, b"").unwrap_or_else(|e| panic!("{e:?}"));
    assert_eq!(
        (missing.severity, missing.summary),
        (Severity::Unknown, None)
    );
    let many: Vec<String> = (0..40).map(|i| format!("CVE-{i}")).collect();
    let body = serde_json::json!({ "aliases": many });
    let a = map_vuln(&r, 200, body.to_string().as_bytes()).unwrap_or_else(|e| panic!("{e:?}"));
    assert_eq!(a.aliases.len(), osv_vuln::MAX_ALIASES);
    assert_eq!(
        map_vuln(&r, 200, b"[").err(),
        Some(CheckError::Service(ServiceFault::BadJson))
    );
}

#[test]
fn a_result_count_mismatch_is_bad_json() {
    let t = FakeTransport::new().with_responder(Box::new(|_| {
        Some(Ok(RawResponse {
            status: 200,
            body: br#"{"results":[]}"#.to_vec(),
        }))
    }));
    let q = [pkg(Ecosystem::Npm, "x", Some("1.0.0"))];
    assert_eq!(
        osv::query_chunk(&t, &q, &|| false).err(),
        Some(CheckError::Service(ServiceFault::BadJson))
    );
}

#[test]
fn the_fake_enforces_the_cap_and_records_everything() {
    let t = FakeTransport::new().with_responder(Box::new(|_| {
        Some(Ok(RawResponse {
            status: 200,
            body: vec![b'x'; 11],
        }))
    }));
    let req = OutboundRequest::depsdev_package(&pkg(Ecosystem::Npm, "x", None));
    assert!(matches!(t.send(&req, 10), Err(TransportError::TooLarge)));
    assert!(t.send(&req, 11).is_ok());
    assert_eq!(t.requests().len(), 2);
}

#[test]
fn the_real_transport_builds() {
    // No request is sent: construction only (TLS provider + builder).
    // [P125d review] if a CI image lacks an OS trust store and this fails,
    // `#[ignore]` it with a note — NEVER fall back to bundled roots.
    assert!(ReqwestTransport::new().is_ok());
}
