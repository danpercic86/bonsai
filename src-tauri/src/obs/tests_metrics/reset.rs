//! `metrics_reset` is headless + clears every aggregate.

use super::*;

// ------------------------------------------------- reset is headless + clears

#[test]
fn reset_clears_every_aggregate() {
    let path = scratch("reset");
    let day = "2026-08-27";
    let store = MetricsState::for_test(path.clone(), 1_756_000_000);
    store.bump_counter("commit.create", 9, day);
    store.observe_ipc_result("getStatus", 3.0, None, day);
    store
        .reset(&PerfCounters::default(), 1_756_200_000)
        .expect("reset");
    let snap = store.snapshot();
    assert_eq!(snap.days.len(), 0);
    assert_eq!(snap.sessions, 0);
    assert!(snap.lifetime.counters.is_empty());
    let _ = std::fs::remove_file(&path);
}

/// `metrics_reset` is a real command but must appear in NO settings-catalog row
/// (headless by design, §8/§12). Scans the frontend catalog sources for the
/// wire name and its camelCase IPC name.
#[test]
fn metrics_reset_appears_in_no_catalog_row() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root")
        .to_path_buf();
    let catalog = root
        .join("src")
        .join("components")
        .join("settings")
        .join("catalog");
    let mut hits = Vec::new();
    scan_dir_for(&catalog, &["metrics_reset", "metricsReset"], &mut hits);
    assert!(
        hits.is_empty(),
        "metrics_reset must not surface in any settings catalog row: {hits:?}"
    );
    // The other half of the criterion: it MUST exist as a real command. Guard
    // against someone deleting the fn (which would make the catalog scan vacuous).
    let obs_src = std::fs::read_to_string(
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("src")
            .join("commands")
            .join("obs.rs"),
    )
    .expect("read commands/obs.rs");
    assert!(
        obs_src.contains("pub async fn metrics_reset"),
        "metrics_reset must exist as a command fn"
    );
}

fn scan_dir_for(dir: &std::path::Path, needles: &[&str], hits: &mut Vec<String>) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            scan_dir_for(&p, needles, hits);
        } else if p.extension().and_then(|x| x.to_str()) == Some("ts") {
            if let Ok(src) = std::fs::read_to_string(&p) {
                for n in needles {
                    if src.contains(n) {
                        hits.push(format!("{} contains {n}", p.display()));
                    }
                }
            }
        }
    }
}
