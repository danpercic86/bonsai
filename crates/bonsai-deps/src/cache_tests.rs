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
