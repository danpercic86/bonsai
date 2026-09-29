use super::*;
use crate::model::{DepReport, DepSummary, FileStamp};

fn cached(repo_id: &str) -> CachedReport {
    CachedReport {
        schema: CACHE_SCHEMA,
        repo_id: repo_id.into(),
        stamps: vec![FileStamp {
            path: "package.json".into(),
            len: 3,
            mtime_ms: 9,
        }],
        report: DepReport {
            repo_id: repo_id.into(),
            checked_at_ms: 5,
            groups: Vec::new(),
            summary: DepSummary::default(),
            network_profile: false,
        },
    }
}

fn tmp() -> tempfile::TempDir {
    tempfile::tempdir().unwrap_or_else(|e| panic!("{e}"))
}

#[test]
fn roundtrip_and_rejects() {
    let dir = tmp();
    let root = dir.path();
    let c = cached("r1");
    assert_eq!(save(root, "abc123", &c), Ok(()));
    assert_eq!(load(root, "abc123", "r1"), Some(c.clone()));
    assert!(!root.join(CACHE_DIR).join("abc123.json.tmp").exists());
    // repo_id mismatch
    assert_eq!(load(root, "abc123", "r2"), None);
    // missing
    assert_eq!(load(root, "fff", "r1"), None);
    // schema mismatch
    let mut old = c.clone();
    old.schema = CACHE_SCHEMA + 1;
    assert_eq!(save(root, "abc123", &old), Ok(()));
    assert_eq!(load(root, "abc123", "r1"), None);
    // bad JSON
    let path = cache_path(root, "abc123").unwrap_or_default();
    std::fs::write(&path, b"{not json").unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(load(root, "abc123", "r1"), None);
    // over the cap
    let big = vec![b' '; (MAX_CACHE_BYTES + 1) as usize];
    std::fs::write(&path, big).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(load(root, "abc123", "r1"), None);
    // overwrite replaces
    assert_eq!(save(root, "abc123", &c), Ok(()));
    assert_eq!(load(root, "abc123", "r1"), Some(c));
}

#[test]
fn a_v1_cache_file_still_loads() {
    // P125h §0: the additive fields default, so `CACHE_SCHEMA` stays 1 and a
    // file written before P125h keeps AC7's instant report.
    assert_eq!(CACHE_SCHEMA, 1);
    let dir = tmp();
    let root = dir.path();
    let v1 = r#"{"schema":1,"repoId":"r1","stamps":[{"path":"package.json","len":3,"mtimeMs":9}],
      "report":{"repoId":"r1","checkedAtMs":5,"groups":[{"manifestPath":"package.json",
        "lockfilePath":null,"ecosystem":"npm","state":{"kind":"ok"},"transitiveChecked":0,
        "rows":[{"name":"lodash","ecosystem":"npm","relation":"direct","scope":"normal",
          "declared":"^4","current":"4.17.20","latest":null,"update":null,
          "availability":{"kind":"unknown"},"vulnsChecked":true,"vulns":[],"registryUrl":null}]}],
      "summary":{"major":0,"minor":0,"patch":0,"vulnerable":0,
        "vulnerableBySeverity":{"critical":0,"high":0,"medium":0,"low":0,"unknown":0},
        "upToDate":0,"unknown":1,"local":0,"private":0,"notTracked":0,"parseErrors":0}}}"#;
    let path = cache_path(root, "abc123").unwrap_or_default();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap_or_else(|e| panic!("{e}"));
    }
    std::fs::write(&path, v1).unwrap_or_else(|e| panic!("{e}"));
    let Some(c) = load(root, "abc123", "r1") else {
        panic!("a v1 cache file must load")
    };
    assert!(!c.report.network_profile);
    let row = &c.report.groups[0].rows[0];
    assert_eq!(row.vulns_unverified, None);
    assert_eq!(
        row.availability,
        crate::model::Availability::Unknown {
            reason: crate::model::UnknownReason::NotFound
        }
    );
}

#[test]
fn a_bad_key_cannot_steer_the_path() {
    let dir = tmp();
    for key in ["", "../x", "a/b", "a\\b", "C:x", "a.json", &"a".repeat(129)] {
        assert_eq!(cache_path(dir.path(), key), None, "{key}");
        assert_eq!(save(dir.path(), key, &cached("r")), Err(CacheError::BadKey));
        assert_eq!(load(dir.path(), key, "r"), None);
    }
    assert_eq!(
        cache_path(dir.path(), "0af3"),
        Some(dir.path().join("deps-tracker").join("0af3.json"))
    );
}
