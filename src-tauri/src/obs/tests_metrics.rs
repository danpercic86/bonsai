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

// ------------------------------------------------------- restart durability

#[test]
fn counters_survive_restart() {
    let path = scratch("restart");
    let day = "2026-08-27";
    {
        let store = MetricsState::for_test(path.clone(), 1_756_000_000);
        store.bump_counter("commit.create", 4, day);
        store.observe_ipc_result("get_status", 12.0, None, day);
        store.flush(&perf(0, 0, 0), 1_756_000_000).expect("flush");
    }
    // A fresh store loading the SAME file is the "restart".
    let store2 = MetricsState::for_test(path.clone(), 1_756_100_000);
    let snap = store2.snapshot();
    let today = snap.days.iter().find(|d| d.date == day).expect("day");
    assert_eq!(today.totals.counters.get("commit.create"), Some(&4));
    assert!(today.totals.durations.contains_key("cmd.get_status"));
    let _ = std::fs::remove_file(&path);
}

// -------------------------------------------------------- daily bucketing

#[test]
fn daily_bucketing_splits_across_a_date_change() {
    let path = scratch("dates");
    let store = MetricsState::for_test(path.clone(), 1_756_000_000);
    store.bump_counter("commit.create", 1, "2026-08-27");
    store.bump_counter("commit.create", 2, "2026-08-28");
    let snap = store.snapshot();
    assert_eq!(snap.days.len(), 2);
    assert_eq!(snap.days[0].date, "2026-08-27");
    assert_eq!(snap.days[1].date, "2026-08-28");
    assert_eq!(snap.days[0].totals.counters.get("commit.create"), Some(&1));
    assert_eq!(snap.days[1].totals.counters.get("commit.create"), Some(&2));
    let _ = std::fs::remove_file(&path);
}

/// Past 400 retained days the oldest bucket folds into `lifetime` (§8), so the
/// `days` vec never grows without bound. Exercises `MetricTotals::merge` +
/// `Histogram::merge`.
#[test]
fn retention_folds_oldest_day_into_lifetime() {
    let path = scratch("retain");
    let store = MetricsState::for_test(path.clone(), 1_756_000_000);
    // 401 distinct synthetic dates, each with one counter tick.
    for i in 0..401u32 {
        let date = format!("2026-{:02}-{:02}", 1 + i / 28, 1 + i % 28);
        store.bump_counter("commit.create", 1, &date);
    }
    let snap = store.snapshot();
    assert_eq!(snap.days.len(), super::RETAIN_DAYS);
    // The very first date was folded out, so it is no longer a day bucket…
    assert_eq!(snap.days[0].date, "2026-01-02");
    // …and its tick landed in lifetime.
    assert_eq!(snap.lifetime.counters.get("commit.create"), Some(&1));
    let _ = std::fs::remove_file(&path);
}

// ------------------------------------------------------- perf absorption

#[test]
fn perf_deltas_appear_as_perf_star() {
    let path = scratch("perf");
    let store = MetricsState::for_test(path.clone(), 1_756_000_000);
    let day = "2026-08-27";
    store.fold_perf(&perf(2, 5, 9), day);
    // A second fold records only the DELTA (monotone counters).
    store.fold_perf(&perf(3, 5, 12), day);
    let snap = store.snapshot();
    let c = &snap.days[0].totals.counters;
    assert_eq!(c.get("perf.repo_opens"), Some(&3)); // 2 + 1
    assert_eq!(c.get("perf.graph_walks"), Some(&5)); // 5 + 0
    assert_eq!(c.get("perf.status_scans"), Some(&12)); // 9 + 3
    let _ = std::fs::remove_file(&path);
}

// ---------------------------------------------------- no user-derived key

#[test]
fn no_user_derived_key_reaches_a_histogram() {
    let path = scratch("nouser");
    let store = MetricsState::for_test(path.clone(), 1_756_000_000);
    let day = "2026-08-27";
    // Repo-content-shaped "command" names must NEVER become a key.
    store.observe_ipc_result("C:/Users/dan/secret-repo", 5.0, None, day);
    store.observe_ipc_result("feature/RED-42", 5.0, None, day);
    store.observe_ipc_result("get_graph", 5.0, None, day); // the one legit name
    // A non-allow-listed span op is dropped whole.
    store.observe_span("evil.op", 9.0, &[], None, day);
    let snap = store.snapshot();
    let keys: Vec<&String> = snap.days[0].totals.durations.keys().collect();
    assert_eq!(keys, vec![&"cmd.get_graph".to_string()]);
    let _ = std::fs::remove_file(&path);
}

/// Every duration key produced by the store must be within the fixed §8.1
/// allow-list — no key sourced from arbitrary strings.
#[test]
fn histogram_key_set_stays_within_allow_list() {
    let path = scratch("allowlist");
    let store = MetricsState::for_test(path.clone(), 1_756_000_000);
    let day = "2026-08-27";
    store.observe_ipc_result("get_status", 3.0, None, day);
    store.observe_span(
        "graph.get",
        20.0,
        &[
            PhaseTiming { name: "lane".into(), ms: 5.0, n: None },
            PhaseTiming { name: "revwalk".into(), ms: 8.0, n: None },
            // Not in the graph.get phase allow-list — must be dropped.
            PhaseTiming { name: "sneaky".into(), ms: 3.0, n: None },
        ],
        Some(2),
        day,
    );
    let snap = store.snapshot();
    let mut keys: Vec<String> = snap.days[0].totals.durations.keys().cloned().collect();
    keys.sort();
    assert_eq!(
        keys,
        vec![
            "cmd.get_status".to_string(),
            "op.graph.get".to_string(),
            "op.graph.get.lane".to_string(),
            "op.graph.get.revwalk".to_string(),
            "queue.blocking".to_string(),
        ]
    );
    let _ = std::fs::remove_file(&path);
}

// --------------------------------------------- §8.1 (b) size independence

#[test]
fn stored_size_is_independent_of_observation_count() {
    let path_a = scratch("size1m");
    let path_b = scratch("size2m");
    let day = "2026-08-27";
    let store_a = MetricsState::for_test(path_a.clone(), 1_756_000_000);
    let store_b = MetricsState::for_test(path_b.clone(), 1_756_000_000);
    for _ in 0..1_000_000u64 {
        store_a.observe_ipc_result("get_status", 3.0, None, day);
    }
    for _ in 0..2_000_000u64 {
        store_b.observe_ipc_result("get_status", 3.0, None, day);
    }
    store_a.flush(&perf(0, 0, 0), 1_756_000_000).expect("flush a");
    store_b.flush(&perf(0, 0, 0), 1_756_000_000).expect("flush b");
    let size_a = std::fs::metadata(&path_a).unwrap().len();
    let size_b = std::fs::metadata(&path_b).unwrap().len();
    // 1,000,000 and 2,000,000 share a digit width, as do their 3× sums, so the
    // file is byte-identical: no raw sample is retained, only the 8 bucket
    // counters + aggregates.
    assert_eq!(size_a, size_b, "usage.json size must not grow with sample count");
    let _ = std::fs::remove_file(&path_a);
    let _ = std::fs::remove_file(&path_b);
}

// ----------------------------------- §8.1 (c) derived-on-snapshot, not disk

#[test]
fn percentiles_are_on_snapshot_but_absent_from_disk() {
    let path = scratch("derived");
    let day = "2026-08-27";
    let store = MetricsState::for_test(path.clone(), 1_756_000_000);
    for _ in 0..100 {
        store.observe_ipc_result("get_status", 42.0, None, day);
    }
    store.flush(&perf(0, 0, 0), 1_756_000_000).expect("flush");
    // Snapshot: p50/p95 present.
    let snap = store.snapshot();
    let h = snap.days[0].totals.durations.get("cmd.get_status").unwrap();
    assert!(h.p50_ms.is_some() && h.p95_ms.is_some());
    // Disk: neither key appears.
    let raw = std::fs::read_to_string(&path).unwrap();
    assert!(!raw.contains("p50Ms") && !raw.contains("p95Ms"));
    let _ = std::fs::remove_file(&path);
}

// ------------------------- §8.1 (d) two-day op.graph.get.lane comparability

#[test]
fn lane_percentiles_are_per_day_comparable() {
    let path = scratch("twoday");
    let store = MetricsState::for_test(path.clone(), 1_756_000_000);
    let lane = |ms: f64| PhaseTiming { name: "lane".into(), ms, n: None };
    // Day 1: fast lane (~5 ms). Day 2: slow lane (~300 ms) — a regression.
    for _ in 0..40 {
        store.observe_span("graph.get", 50.0, &[lane(5.0)], None, "2026-08-27");
    }
    for _ in 0..40 {
        store.observe_span("graph.get", 400.0, &[lane(300.0)], None, "2026-08-28");
    }
    let snap = store.snapshot();
    let d1 = &snap.days[0].totals.durations.get("op.graph.get.lane").unwrap();
    let d2 = &snap.days[1].totals.durations.get("op.graph.get.lane").unwrap();
    let p95_1 = d1.p95_ms.unwrap();
    let p95_2 = d2.p95_ms.unwrap();
    assert!(
        p95_2 > p95_1 * 3,
        "day-2 lane p95 ({p95_2}) must dwarf day-1 ({p95_1}) — the regression signal"
    );
    let _ = std::fs::remove_file(&path);
}

// ------------------------------ §8.1 (a) percentile vs brute-force reference

#[test]
fn percentile_matches_brute_force_within_one_bucket() {
    use crate::obs::histogram::{Histogram, BUCKET_BOUNDS_MS};
    // 10k deterministic pseudo-random samples in 0..600 ms (an LCG — no dep).
    let mut samples: Vec<u64> = Vec::with_capacity(10_000);
    let mut s: u64 = 0x1234_5678;
    let mut h = Histogram::default();
    for _ in 0..10_000 {
        s = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        let v = (s >> 33) % 600;
        samples.push(v);
        h.observe(v);
    }
    samples.sort_unstable();
    let widest = 1500u32; // the coarsest bucket, (500, 2000]
    for p in [0.5f64, 0.95] {
        let idx = ((p * samples.len() as f64) as usize).min(samples.len() - 1);
        let reference = samples[idx] as i64;
        let est = h.percentile_ms(p as f32).unwrap() as i64;
        assert!(
            (est - reference).unsigned_abs() as u32 <= widest,
            "p{p}: est {est} vs reference {reference} exceeds one bucket width"
        );
    }
    // Sanity: the bucket bounds are the frozen 7 we interpolate within.
    assert_eq!(BUCKET_BOUNDS_MS.len(), 7);
}

/// The `percentile_ms(0.0)` edge (inc-5 review NIT): with an empty `buckets[0]`
/// it must return the LOWER bound of the first non-empty bucket, not `max_ms`.
#[test]
fn percentile_zero_skips_empty_leading_bucket() {
    use crate::obs::histogram::Histogram;
    let mut h = Histogram::default();
    for _ in 0..20 {
        h.observe(20); // lands in bucket (10, 50], index 3
    }
    // Before the fix this returned max_ms (20); now it returns the bucket's lower
    // bound (10).
    assert_eq!(h.percentile_ms(0.0), Some(10));
}

// ------------------------------------------------- reset is headless + clears

#[test]
fn reset_clears_every_aggregate() {
    let path = scratch("reset");
    let day = "2026-08-27";
    let store = MetricsState::for_test(path.clone(), 1_756_000_000);
    store.bump_counter("commit.create", 9, day);
    store.observe_ipc_result("get_status", 3.0, None, day);
    store.reset(1_756_200_000).expect("reset");
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
    let catalog = root.join("src").join("components").join("settings").join("catalog");
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

/// `bump_counter` is `pub`, so the "counters are never user-derived" guarantee
/// (§8: `usage.json` carries no user content) rests entirely on call sites using
/// `<domain>.<action>` literals. The `debug_assert` is what makes a slip fail
/// loudly in tests instead of silently persisting a branch name or a path.
#[test]
#[should_panic(expected = "never user-derived")]
fn bump_counter_rejects_a_user_derived_key_in_debug() {
    let store = MetricsState::default();
    store.bump_counter("feature/my-branch", 1, "2026-08-27");
}

/// The allow-listed shapes the real call sites use must keep passing.
#[test]
fn bump_counter_accepts_domain_action_literals() {
    let store = MetricsState::default();
    for key in ["commit.create", "perf.repo_opens", "graph.get.rows"] {
        store.bump_counter(key, 1, "2026-08-27");
    }
    let snap = store.snapshot();
    let day = snap.days.last().expect("day bucket");
    assert_eq!(day.totals.counters.len(), 3);
}
