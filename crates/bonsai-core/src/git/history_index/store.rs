//! On-disk persistence for the history index (P57a contract §3.4).
//!
//! One `store.json` per repo under `index_dir` (the command resolves
//! `index_dir` via [`super::index_dir_for`]; core stays runtime-free). Writes
//! are atomic (tmp + rename, mirroring `settings.rs`). [`repo_key`] is the
//! FNV-1a hex of the path-normalized workdir (case-folded on Windows) — the
//! per-repo subdirectory name. Schema invalidation is enforced by the caller
//! (`build_index`/`index_status`) comparing [`IndexStore::schema`].

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::error::AppError;

use super::bm25::Bm25Index;
use super::doc::CommitDoc;
use super::HISTORY_INDEX_SCHEMA;

/// The single persisted file under a repo's `index_dir`.
pub const STORE_FILE: &str = "store.json";

/// The tmp-file infix that marks an in-progress atomic write:
/// `store.json.<pid>.<nonce>.tmp`. Used to build a unique tmp name AND to
/// recognize stale leftovers for best-effort cleanup on load.
const TMP_INFIX: &str = "store.json.";
const TMP_SUFFIX: &str = ".tmp";

/// Per-process monotonic nonce so two saves from the SAME process still get
/// distinct tmp names (the pid alone is not enough under concurrency).
static TMP_NONCE: AtomicU64 = AtomicU64::new(0);

/// The full persisted index for one repo (contract §3.4). `docs` is a `BTreeMap`
/// so the doc ORDER on disk is deterministic; `bm25` is rebuilt from `docs` on
/// every save so it never drifts from the documents.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct IndexStore {
    /// Must equal [`HISTORY_INDEX_SCHEMA`] or the caller discards + rebuilds.
    pub schema: u32,
    /// HEAD (40-hex) at the last build; `None` when HEAD was unborn.
    pub head_oid: Option<String>,
    /// Sorted ref-tip oids at the last build (drives the staleness compare).
    pub tip_oids: Vec<String>,
    /// Unix seconds of the last build.
    pub built_at: Option<i64>,
    /// Full-oid hex -> document.
    pub docs: BTreeMap<String, CommitDoc>,
    /// Corpus stats derived from `docs`.
    pub bm25: Bm25Index,
}

impl IndexStore {
    /// A fresh, empty store stamped at the current schema.
    pub fn empty() -> IndexStore {
        IndexStore {
            schema: HISTORY_INDEX_SCHEMA,
            head_oid: None,
            tip_oids: Vec::new(),
            built_at: None,
            docs: BTreeMap::new(),
            bm25: Bm25Index::default(),
        }
    }
}

/// Load `index_dir/store.json`. Missing/unreadable/unparsable -> `None` (the
/// caller treats that as "not built" / "start empty"). Never errors — a corrupt
/// cache is regenerable derived data, not a fatal condition. Also best-effort
/// removes stale `store.json.<pid>.<nonce>.tmp` leftovers from a crashed or
/// killed build (F-A9-1); a tmp still open by a concurrent build cannot be
/// deleted on Windows, so an in-flight write is left untouched.
pub fn load(index_dir: &Path) -> Option<IndexStore> {
    cleanup_stale_tmp(index_dir);
    let bytes = std::fs::read(index_dir.join(STORE_FILE)).ok()?;
    serde_json::from_slice(&bytes).ok()
}

/// Best-effort removal of leftover atomic-write tmp files. Ignores every error:
/// a tmp still held open by a concurrent build fails to delete (Windows) and is
/// correctly left in place; a truly-stale tmp (crashed build, no open handle) is
/// reclaimed.
fn cleanup_stale_tmp(index_dir: &Path) {
    let Ok(entries) = std::fs::read_dir(index_dir) else {
        return; // dir absent / unreadable -> nothing to clean
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.starts_with(TMP_INFIX) && name.ends_with(TMP_SUFFIX) {
            let _ = std::fs::remove_file(entry.path());
        }
    }
}

/// Atomically write `store` to `index_dir/store.json`: create the dir, write to
/// a UNIQUE `store.json.<pid>.<nonce>.tmp`, then rename over the target (mirrors
/// `settings.rs::save_to`; on Windows `rename` replaces an existing
/// destination). The tmp name is per-process + per-call unique (F-A9-1) so two
/// concurrent builds never write the same tmp and corrupt each other's
/// rename-into-place (or hit Windows `ACCESS_DENIED` sharing the tmp handle).
pub fn save(index_dir: &Path, store: &IndexStore) -> Result<(), AppError> {
    std::fs::create_dir_all(index_dir)
        .map_err(|e| AppError::Io(format!("create index dir {}: {e}", index_dir.display())))?;
    let file = index_dir.join(STORE_FILE);
    let json =
        serde_json::to_vec(store).map_err(|e| AppError::Io(format!("serialize index: {e}")))?;

    let nonce = TMP_NONCE.fetch_add(1, Ordering::Relaxed);
    let tmp = index_dir.join(format!(
        "{TMP_INFIX}{}.{nonce}{TMP_SUFFIX}",
        std::process::id()
    ));

    std::fs::write(&tmp, &json).map_err(|e| AppError::Io(format!("write {}: {e}", tmp.display())))?;
    std::fs::rename(&tmp, &file).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        AppError::Io(format!(
            "rename {} -> {}: {e}",
            tmp.display(),
            file.display()
        ))
    })?;
    Ok(())
}

/// Per-repo subdirectory name: FNV-1a hex of the path-normalized workdir.
/// Normalization makes the key stable across separator / trailing-slash /
/// (Windows) case differences for the same physical directory (contract §3.4).
pub fn repo_key(workdir: &Path) -> String {
    fnv1a_hex(normalize_path(workdir).as_bytes())
}

/// Normalize a workdir path for keying: forward slashes, no trailing slash,
/// lowercased on Windows (case-insensitive filesystem).
fn normalize_path(p: &Path) -> String {
    let mut s = p.to_string_lossy().replace('\\', "/");
    while s.len() > 1 && s.ends_with('/') {
        s.pop();
    }
    #[cfg(windows)]
    {
        s = s.to_lowercase();
    }
    s
}

/// 64-bit FNV-1a hex digest (hand-rolled — no `fnv` crate dependency).
fn fnv1a_hex(bytes: &[u8]) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in bytes {
        hash ^= b as u64;
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{hash:016x}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn sample_store() -> IndexStore {
        let mut store = IndexStore::empty();
        let mut tf = HashMap::new();
        tf.insert("auth".to_string(), 3u16);
        tf.insert("login".to_string(), 1u16);
        store.docs.insert(
            "a".repeat(40),
            CommitDoc {
                summary: "fix auth".to_string(),
                author_name: "Ada".to_string(),
                author_ts: 1000,
                dl: 4,
                tf,
            },
        );
        store.bm25 = Bm25Index::build_stats(&store.docs);
        store.head_oid = Some("a".repeat(40));
        store.tip_oids = vec!["a".repeat(40)];
        store.built_at = Some(1_700_000_000);
        store
    }

    // ---------------------------------------------------- §7.10 round trip

    #[test]
    fn save_load_round_trip() {
        let dir = crate::testutil::scratch_dir();
        let index_dir = dir.path().join("idx");
        let store = sample_store();
        save(&index_dir, &store).expect("save");

        // The file exists and no .tmp is left behind (atomic rename).
        assert!(index_dir.join(STORE_FILE).exists(), "store.json present");
        assert!(
            !index_dir.join("store.json.tmp").exists(),
            "no leftover tmp"
        );

        let back = load(&index_dir).expect("load");
        assert_eq!(back, store, "round-trips byte-for-content");

        // Overwriting is also atomic and leaves no tmp.
        save(&index_dir, &store).expect("re-save");
        assert!(!index_dir.join("store.json.tmp").exists());
    }

    #[test]
    fn load_missing_is_none() {
        let dir = crate::testutil::scratch_dir();
        assert!(load(&dir.path().join("nope")).is_none());
    }

    // -------------------------------------------- F-A9-1 tmp isolation/cleanup

    /// Concurrent saves into the SAME index dir must each use a unique tmp file
    /// (pid+nonce), so no build corrupts another's rename-into-place. After the
    /// storm the store parses and NO `store.json.*.tmp` leftover remains.
    #[test]
    fn concurrent_saves_use_isolated_tmp_and_leave_no_leftover() {
        let dir = crate::testutil::scratch_dir();
        let index_dir = std::sync::Arc::new(dir.path().join("idx"));
        std::fs::create_dir_all(index_dir.as_path()).expect("mkdir");

        let mut handles = Vec::new();
        for _ in 0..8 {
            let index_dir = std::sync::Arc::clone(&index_dir);
            handles.push(std::thread::spawn(move || {
                let store = sample_store();
                // Each thread saves repeatedly to widen the interleave window.
                for _ in 0..10 {
                    save(&index_dir, &store).expect("concurrent save");
                }
            }));
        }
        for h in handles {
            h.join().expect("join");
        }

        // The store is intact and no tmp file survives.
        assert!(load(index_dir.as_path()).is_some(), "store parses after storm");
        let leftovers: Vec<_> = std::fs::read_dir(index_dir.as_path())
            .expect("read_dir")
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|n| n.starts_with("store.json.") && n.ends_with(".tmp"))
            .collect();
        assert!(leftovers.is_empty(), "no leftover tmp files: {leftovers:?}");
    }

    /// A stale tmp (crashed build) is reclaimed on the next `load`; the real
    /// `store.json` is never touched.
    #[test]
    fn load_cleans_up_stale_tmp() {
        let dir = crate::testutil::scratch_dir();
        let index_dir = dir.path().join("idx");
        save(&index_dir, &sample_store()).expect("save");

        // Simulate a crashed build's orphan tmp.
        let stale = index_dir.join(format!("store.json.{}.999.tmp", std::process::id()));
        std::fs::write(&stale, b"partial garbage").expect("write stale tmp");
        assert!(stale.exists());

        let back = load(&index_dir).expect("load still works");
        assert_eq!(back, sample_store(), "real store untouched");
        assert!(!stale.exists(), "stale tmp reclaimed on load");
    }

    // ---------------------------------------------------- §7.11 repo_key

    #[test]
    fn repo_key_is_stable_and_path_normalized() {
        let a = repo_key(Path::new("/home/user/repo"));
        let b = repo_key(Path::new("/home/user/repo"));
        assert_eq!(a, b, "same workdir => same key");
        assert_eq!(a.len(), 16, "16 hex chars (u64)");
        assert!(a.chars().all(|c| c.is_ascii_hexdigit()));

        // Trailing slash + separator flavor normalize to the same key.
        assert_eq!(repo_key(Path::new("/home/user/repo/")), a);

        // Distinct repos => distinct keys.
        assert_ne!(a, repo_key(Path::new("/home/user/other")));

        // On Windows, case + backslash normalize to one key.
        #[cfg(windows)]
        {
            let w = repo_key(Path::new("C:\\Repos\\Bonsai"));
            assert_eq!(w, repo_key(Path::new("c:/repos/bonsai")));
        }
    }

    #[test]
    fn fnv1a_matches_known_vector() {
        // FNV-1a/64 of "" is the offset basis; of "a" the documented vector.
        assert_eq!(fnv1a_hex(b""), "cbf29ce484222325");
        assert_eq!(fnv1a_hex(b"a"), "af63dc4c8601ec8c");
    }
}
