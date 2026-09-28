//! P91 §12 row 6 — durable-metrics acceptance tests.
//!
//! Covers: counters survive restart; daily bucketing across a simulated date
//! change; `perf.*` absorption; the no-HTTP + no-user-key structural guards;
//! `metrics_reset` is headless (no catalog row); and §8.1 (a)–(e).

use super::MetricsState;
use crate::obs::record::PhaseTiming;
use crate::perf::PerfCounters;

fn scratch(name: &str) -> std::path::PathBuf {
    let mut p = std::env::temp_dir();
    p.push(format!(
        "bonsai-metrics-t-{}-{}-{:?}",
        name,
        std::process::id(),
        std::thread::current().id()
    ));
    let _ = std::fs::create_dir_all(&p);
    p.push("usage.json");
    let _ = std::fs::remove_file(&p);
    p
}

fn perf(repo_opens: u64, graph_walks: u64, status_scans: u64) -> PerfCounters {
    PerfCounters {
        repo_opens,
        graph_walks,
        graph_cache_hits: 0,
        graph_redecorates: 0,
        status_scans,
    }
}

// ------------------------------------------------------------ no-HTTP guard

/// No source under `obs/` may reach an HTTP client — metrics are LOCAL-ONLY (§8).
#[test]
fn obs_tree_reaches_no_http_dependency() {
    let obs = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("src")
        .join("obs");
    let banned = ["reqwest", "ureq", "hyper", "isahc", "curl::"];
    let mut hits = Vec::new();
    scan_obs_for_http(&obs, &banned, &mut hits);
    assert!(
        hits.is_empty(),
        "obs/ must not reference an HTTP client: {hits:?}"
    );
}

fn scan_obs_for_http(dir: &std::path::Path, banned: &[&str], hits: &mut Vec<String>) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            scan_obs_for_http(&p, banned, hits);
        } else if p.extension().and_then(|x| x.to_str()) == Some("rs") {
            // Skip this very test file (it names the banned tokens on purpose).
            if p.file_name().and_then(|n| n.to_str()) == Some("tests_metrics.rs") {
                continue;
            }
            if let Ok(src) = std::fs::read_to_string(&p) {
                for b in banned {
                    if src.contains(b) {
                        hits.push(format!("{} references {b}", p.display()));
                    }
                }
            }
        }
    }
}

#[path = "tests_metrics/durability.rs"]
mod durability;
#[path = "tests_metrics/keys.rs"]
mod keys;
#[path = "tests_metrics/percentiles.rs"]
mod percentiles;
#[path = "tests_metrics/reset.rs"]
mod reset;
