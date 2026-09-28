//! No user-derived key reaches a histogram or a counter.

use super::*;

// ---------------------------------------------------- no user-derived key

#[test]
fn no_user_derived_key_reaches_a_histogram() {
    let path = scratch("nouser");
    let store = MetricsState::for_test(path.clone(), 1_756_000_000);
    let day = "2026-08-27";
    // Repo-content-shaped "command" names must NEVER become a key.
    store.observe_ipc_result("C:/Users/dan/secret-repo", 5.0, None, day);
    store.observe_ipc_result("feature/RED-42", 5.0, None, day);
    store.observe_ipc_result("getGraph", 5.0, None, day); // the one legit name
                                                          // A non-allow-listed span op is dropped whole.
    store.observe_span("evil.op", 9.0, &[], None, day);
    let snap = store.snapshot();
    let keys: Vec<&String> = snap.days[0].totals.durations.keys().collect();
    assert_eq!(keys, vec![&"cmd.getGraph".to_string()]);
    let _ = std::fs::remove_file(&path);
}

/// Every duration key produced by the store must be within the fixed §8.1
/// allow-list — no key sourced from arbitrary strings.
#[test]
fn histogram_key_set_stays_within_allow_list() {
    let path = scratch("allowlist");
    let store = MetricsState::for_test(path.clone(), 1_756_000_000);
    let day = "2026-08-27";
    store.observe_ipc_result("getStatus", 3.0, None, day);
    store.observe_span(
        "graph.get",
        20.0,
        &[
            PhaseTiming {
                name: "lane".into(),
                ms: 5.0,
                n: None,
            },
            PhaseTiming {
                name: "revwalk".into(),
                ms: 8.0,
                n: None,
            },
            // Not in the graph.get phase allow-list — must be dropped.
            PhaseTiming {
                name: "sneaky".into(),
                ms: 3.0,
                n: None,
            },
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
            "cmd.getStatus".to_string(),
            "op.graph.get".to_string(),
            "op.graph.get.lane".to_string(),
            "op.graph.get.revwalk".to_string(),
            "queue.blocking".to_string(),
        ]
    );
    let _ = std::fs::remove_file(&path);
}

/// REGRESSION (audit F2) — `bump_counter` DROPS a user-derived key at runtime,
/// in every profile.
///
/// This guarantee used to rest on a `debug_assert`, which `debug-assertions =
/// false` compiles out; the crate defines no `[profile.release]` override, so a
/// shipped binary had no guard at all and would have persisted a branch name or
/// a path into `usage.json` — durable, un-redacted, and outside the
/// `logs_delete_all` scope (§8, decision 25).
///
/// The test deliberately asserts the DROP (release-path behaviour) rather than a
/// panic: were the guard reverted to a `debug_assert`, this test would panic in a
/// debug build and record the key in a release one — it fails either way.
#[test]
fn bump_counter_drops_a_user_derived_key_in_every_profile() {
    let store = MetricsState::default();
    let day = "2026-08-27";
    for key in [
        "feature/my-branch",
        "C:/Users/dan/secret-repo",
        "Fix the parser",
        "ghp_0123456789abcdef",
        "",
    ] {
        store.bump_counter(key, 1, day);
    }
    let snap = store.snapshot();
    let recorded: Vec<&String> = snap
        .days
        .iter()
        .flat_map(|d| d.totals.counters.keys())
        .collect();
    assert!(
        recorded.is_empty(),
        "no user-derived key may reach usage.json, got {recorded:?}"
    );
}

/// The production counter writer is `fold_perf`, not `bump_counter`, and both
/// now share the validated sink (`MetricsState::bump_validated`). So drive EVERY
/// `PERF_KEYS` entry through it: a key that failed `is_valid_counter_key` would
/// be dropped silently, and the missing counter is what this asserts.
#[test]
fn every_perf_key_survives_the_counter_key_guard() {
    let store = MetricsState::default();
    // All five fields non-zero, so all five deltas are > 0 on the first fold.
    let all = PerfCounters {
        repo_opens: 1,
        graph_walks: 2,
        graph_cache_hits: 3,
        graph_redecorates: 4,
        status_scans: 5,
    };
    store.fold_perf(&all, "2026-08-27");
    let snap = store.snapshot();
    let day = snap.days.last().expect("day bucket");
    let keys: Vec<&str> = day.totals.counters.keys().map(String::as_str).collect();
    assert_eq!(
        keys,
        [
            "perf.graph_cache_hits",
            "perf.graph_redecorates",
            "perf.graph_walks",
            "perf.repo_opens",
            "perf.status_scans",
        ],
        "a PERF_KEYS literal was rejected by the counter-key guard"
    );
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
