//! P117 §2 — the repo dimension on `redundant-refresh` and `cache-collapse`
//! (acceptance criteria AC2-1 … AC2-6, AC2-8, AC2-10, AC2-11).
//!
//! Its own file rather than an addition to `tests_anomaly.rs` / `_slow.rs`: the
//! concern is one cross-cutting dimension applied to two rules that live in two
//! different modules, and both host files are already at their topic's size.
//!
//! Every timestamp here is drawn from the measured session the contract quotes
//! (141 ms cross-repo gap, 217 ms five-repo cache window, 400 ms genuine
//! same-repo repeat), so a regression reads as the real defect, not as a
//! synthetic one.

use super::tests_anomaly_support::{ipc_call, refresh, refs_of, span, with_repo, with_trace, H};
use crate::obs::record::{LogPayload, LogRecord};

/// `D:\Repos\my project` — the whitespace-bearing path that the generic strict
/// heuristic mis-handles (see `tests_repo_redaction.rs`). Used here too so the
/// detector is exercised with a realistic canonical `repoId`.
const REPO_A: &str = r"D:\Repos\my project";
const REPO_B: &str = r"D:\Repos\bonsai";

/// A `graph.get` span with a cache outcome, attributed to `repo`.
fn cache_span(ts: i64, kind: &str, repo: Option<&str>) -> LogRecord {
    let s = span(
        ts,
        "graph.get",
        10.0,
        None,
        None,
        None,
        None,
        Some(kind),
        None,
    );
    match repo {
        Some(r) => with_repo(s, r),
        None => s,
    }
}

// ---- AC2-1: the cross-repo false positives -------------------------------

/// Two rounds 141 ms apart in DIFFERENT repos share a scope and nothing else.
/// 17 of 26 firings in the measured session were this shape.
#[test]
fn ac2_1_same_scope_different_repo_is_not_redundant() {
    for scope in ["full", "status", "graph"] {
        let mut h = H::new();
        h.feed(with_repo(refresh(1000, scope), REPO_A));
        h.feed(with_repo(refresh(1141, scope), REPO_B));
        assert_eq!(
            h.count("redundant-refresh"),
            0,
            "{scope}: two repos refreshing 141ms apart is not a redundant refresh"
        );
    }
}

/// `full` carried the whole reported cost (all five `full`-scope pairs were
/// cross-repo), so it gets its own named case with five distinct repos.
#[test]
fn ac2_1_five_repos_full_scope_emit_nothing() {
    let mut h = H::new();
    for (i, repo) in ["r1", "r2", "r3", "r4", "r5"].into_iter().enumerate() {
        h.feed(with_repo(refresh(1000 + i as i64 * 35, "full"), repo));
    }
    assert_eq!(h.count("redundant-refresh"), 0);
}

// ---- AC2-2: the true positive still fires ---------------------------------

#[test]
fn ac2_2_same_repo_same_scope_still_fires() {
    let mut h = H::new();
    let s1 = h.feed(with_repo(refresh(1000, "full"), REPO_A));
    let s2 = h.feed(with_repo(refresh(1400, "full"), REPO_A));
    assert_eq!(h.count("redundant-refresh"), 1);
    let a = h.find("redundant-refresh").expect("fires");
    assert_eq!(refs_of(a), &[s1, s2]);
}

// ---- AC2-3: the `arm` debounce is repo-keyed ------------------------------

/// Proves `arm` AND `refs_for` took the composite key, not just the lookup: a
/// scope-keyed debounce would rate-limit repo B out because repo A just fired.
#[test]
fn ac2_3_arm_debounce_is_repo_keyed() {
    let mut h = H::new();
    h.feed(with_repo(refresh(1000, "full"), REPO_A));
    h.feed(with_repo(refresh(1400, "full"), REPO_A));
    let b1 = h.feed(with_repo(refresh(1500, "full"), REPO_B));
    let b2 = h.feed(with_repo(refresh(1900, "full"), REPO_B));
    assert_eq!(
        h.count("redundant-refresh"),
        2,
        "both repos fire inside one 1s window"
    );
    // The second anomaly's refs are repo B's records only — `refs_for` filtered
    // on the composite key, so repo A's two rounds are not pooled in.
    let second = h
        .out
        .iter()
        .filter(|r| matches!(&r.payload, LogPayload::Anomaly { rule, .. } if rule == "redundant-refresh"))
        .nth(1)
        .expect("two firings");
    assert_eq!(refs_of(second), &[b1, b2]);
}

// ---- AC2-4: cache-collapse across five repos -----------------------------

#[test]
fn ac2_4_five_repos_one_miss_each_is_not_a_collapse() {
    let mut h = H::new();
    for (i, repo) in ["r1", "r2", "r3", "r4", "r5"].into_iter().enumerate() {
        h.feed(cache_span(1000 + i as i64 * 54, "miss", Some(repo)));
    }
    assert_eq!(
        h.count("cache-collapse"),
        0,
        "five cold caches are not one collapsing cache"
    );
}

#[test]
fn ac2_4_five_misses_in_one_repo_fire_with_only_that_repos_refs() {
    let mut h = H::new();
    // The same pooled five-repo window as above — it must contribute nothing.
    for (i, repo) in ["r1", "r2", "r3", "r4", "r5"].into_iter().enumerate() {
        h.feed(cache_span(1000 + i as i64 * 54, "miss", Some(repo)));
    }
    let mut mine = Vec::new();
    for i in 0..5 {
        mine.push(h.feed(cache_span(1300 + i * 20, "miss", Some(REPO_A))));
    }
    assert_eq!(h.count("cache-collapse"), 1);
    let a = h.find("cache-collapse").expect("fires");
    assert_eq!(
        refs_of(a),
        mine.as_slice(),
        "refs are one repo's spans only"
    );
}

/// The `None` bucket is its own bucket (§2.5): the non-routed
/// `stream_graph_cached` entry point keeps today's repo-blind behaviour and is
/// never merged into an attributed repo's group.
#[test]
fn ac2_4_unattributed_spans_form_their_own_bucket() {
    let mut h = H::new();
    for i in 0..4 {
        h.feed(cache_span(1000 + i * 10, "miss", Some(REPO_A)));
    }
    h.feed(cache_span(1050, "miss", None));
    assert_eq!(
        h.count("cache-collapse"),
        0,
        "4 attributed + 1 unattributed is neither group reaching 5"
    );
    h.feed(cache_span(1060, "miss", Some(REPO_A)));
    assert_eq!(
        h.count("cache-collapse"),
        1,
        "the 5th of repo A's own fires"
    );
}

// ---- AC2-5: cross-repo mutation suppression is gone ----------------------

/// A mutation is intervening only when it is attributed to THIS repo (or
/// unattributed). The mutation sits strictly BETWEEN the pair — placed before
/// both it could not suppress regardless of repo, and the case would be vacuous.
#[test]
fn ac2_5_refresh_mutation_in_repo_a_does_not_suppress_repo_b() {
    let mut h = H::new();
    h.feed(with_repo(refresh(1000, "full"), REPO_B));
    h.feed(with_repo(ipc_call(1100, "commit", "m1"), REPO_A));
    h.feed(with_repo(refresh(1400, "full"), REPO_B));
    assert_eq!(h.count("redundant-refresh"), 1, "repo B still fires");
}

#[test]
fn ac2_5_refresh_mutation_in_the_same_repo_still_suppresses() {
    let mut h = H::new();
    h.feed(with_repo(refresh(1000, "full"), REPO_A));
    h.feed(with_repo(ipc_call(1100, "commit", "m1"), REPO_A));
    h.feed(with_repo(refresh(1400, "full"), REPO_A));
    assert_eq!(h.count("redundant-refresh"), 0);
}

/// §2.4 — an UNATTRIBUTED mutation suppresses EVERYWHERE for its window.
/// Conservative by design: suppression costs a missed anomaly, never a false
/// one.
///
/// The `cmd` is `fetch`, a real wire name that `is_mutation_cmd` really matches
/// — unlike the `clone_repo` this fixture used to feed, which no producer can
/// emit (the wire name is `cloneRepo`, and the snake_case table never matches
/// it), so the case asserted on an input that cannot occur.
///
/// Where does an unattributed one come from, though? NOT from a recognised
/// mutation on today's wire: review fix 1 gave all 29 of them a `repoId`
/// position, so `repoIdArg` attributes every one. The `None` bucket is reached
/// by a `schema: 2` line replayed from an older log (§2.6/AC2-10 — no `repo`
/// key at all), by a lift that failed its non-empty-string check, or by a future
/// producer. §2.4's rule is about the BUCKET, however a record landed in it.
#[test]
fn ac2_5_refresh_unattributed_mutation_suppresses_both_repos() {
    for repo in [REPO_A, REPO_B] {
        let mut h = H::new();
        h.feed(with_repo(refresh(1000, "full"), repo));
        h.feed(ipc_call(1100, "fetch", "m1")); // no repo attribution
        h.feed(with_repo(refresh(1400, "full"), repo));
        assert_eq!(
            h.count("redundant-refresh"),
            0,
            "an unattributed mutation suppresses every repo"
        );
    }
}

#[test]
fn ac2_5_cache_mutation_in_repo_a_does_not_suppress_repo_b() {
    let mut h = H::new();
    h.feed(with_repo(ipc_call(900, "commit", "m1"), REPO_A));
    for i in 0..5 {
        h.feed(cache_span(1000 + i * 10, "miss", Some(REPO_B)));
    }
    assert_eq!(h.count("cache-collapse"), 1, "repo B still fires");
}

#[test]
fn ac2_5_cache_mutation_in_the_same_repo_still_suppresses() {
    let mut h = H::new();
    h.feed(with_repo(ipc_call(900, "commit", "m1"), REPO_B));
    for i in 0..5 {
        h.feed(cache_span(1000 + i * 10, "miss", Some(REPO_B)));
    }
    assert_eq!(h.count("cache-collapse"), 0);
}

/// The `cache-collapse` half of the case above — same reasoning about `fetch`
/// and about where a `repo: None` mutation record comes from.
#[test]
fn ac2_5_cache_unattributed_mutation_suppresses_both_repos() {
    for repo in [REPO_A, REPO_B] {
        let mut h = H::new();
        h.feed(ipc_call(900, "fetch", "m1")); // no repo attribution
        for i in 0..5 {
            h.feed(cache_span(1000 + i * 10, "miss", Some(repo)));
        }
        assert_eq!(h.count("cache-collapse"), 0);
    }
}

// ---- AC2-6: dup-ipc is unchanged -----------------------------------------

/// Two calls of the same `cmd` for different repos do not collide — not because
/// of a new repo key, but because `argsHash` already hashes the whole args
/// object, `repoId` included. No rule change (§2.5).
#[test]
fn ac2_6_dup_ipc_different_repos_do_not_collide() {
    let mut h = H::new();
    h.feed(with_repo(ipc_call(1000, "get_status", "hash-of-A"), REPO_A));
    h.feed(with_repo(ipc_call(1100, "get_status", "hash-of-B"), REPO_B));
    assert_eq!(h.count("dup-ipc"), 0, "different argsHash, different call");
}

/// And the attribution must not leak INTO `dup-ipc`: same `cmd` + same
/// `argsHash` still fires even when the base `repo` field differs, because the
/// rule ignores the dimension entirely.
#[test]
fn ac2_6_dup_ipc_ignores_the_repo_dimension() {
    let mut h = H::new();
    h.feed(with_repo(ipc_call(1000, "get_status", "same"), REPO_A));
    h.feed(with_repo(ipc_call(1100, "get_status", "same"), REPO_B));
    assert_eq!(h.count("dup-ipc"), 1);
}

/// `dup-ipc` also keeps treating ALL mutations as intervening, attributed or
/// not — its closure destructures the new tuple and drops the repo.
#[test]
fn ac2_6_dup_ipc_is_suppressed_by_a_foreign_repos_mutation() {
    let mut h = H::new();
    h.feed(with_repo(ipc_call(1000, "get_status", "same"), REPO_A));
    h.feed(with_repo(ipc_call(1050, "commit", "m1"), REPO_B));
    h.feed(with_repo(ipc_call(1100, "get_status", "same"), REPO_A));
    assert_eq!(h.count("dup-ipc"), 0);
}

// ---- AC2-8: no repo value in any anomaly ---------------------------------

/// The detector has no `Redactor`, so interpolating the value would leak a path
/// into `detail` in raw mode AND break the strict/raw byte-identity of the
/// anomaly stream. `refs` already points at the records that carry it.
#[test]
fn ac2_8_no_repo_value_reaches_an_anomaly_record() {
    let mut h = H::new();
    h.feed(with_repo(refresh(1000, "full"), REPO_A));
    h.feed(with_repo(refresh(1400, "full"), REPO_A));
    for i in 0..5 {
        h.feed(cache_span(2000 + i * 10, "miss", Some(REPO_A)));
    }
    assert_eq!(h.count("redundant-refresh"), 1);
    assert_eq!(h.count("cache-collapse"), 1);
    let json = serde_json::to_string(&h.out).expect("anomalies serialise");
    assert!(
        !json.contains("Repos") && !json.contains("my project") && !json.contains("repo#"),
        "no repo value, raw or redacted, may appear in an anomaly: {json}"
    );
    for r in &h.out {
        assert!(r.repo.is_none(), "an anomaly record carries no repo field");
    }
}

/// §7.2 (c) extended to the new dimension: the anomaly output depends on the
/// PARTITIONING, never on the representation. Two runs whose repo values differ
/// only in form (raw path vs an ordinal-shaped token) are byte-identical.
#[test]
fn ac2_8_anomaly_output_is_independent_of_the_repo_representation() {
    fn run(a: &str, b: &str) -> String {
        let mut h = H::new();
        h.feed(with_repo(refresh(1000, "full"), a));
        h.feed(with_repo(refresh(1141, "full"), b));
        h.feed(with_repo(refresh(1400, "full"), a));
        h.feed(with_trace(
            with_repo(ipc_call(1500, "commit", "m"), b),
            "t1",
        ));
        serde_json::to_string(&h.out).expect("serialise")
    }
    let raw = run(REPO_A, REPO_B);
    let ordinals = run("repo#7", "repo#8");
    assert_eq!(raw, ordinals, "anomalies must not depend on repo form");
    assert!(raw.contains("redundant-refresh"), "not a vacuous assertion");
}

// ---- AC2-10: a v2 line replays repo-blind --------------------------------

/// A `schema: 2` record has no `repo` key at all. It must deserialise (never
/// error, never drop), land in the `""` bucket, and produce the PRE-P117 firing
/// for the same input sequence.
#[test]
fn ac2_10_v2_records_deserialise_and_replay_repo_blind() {
    fn v2_refresh(ts: i64, scope: &str) -> LogRecord {
        let line = format!(
            r#"{{"seq":0,"ts":{ts},"mono":1,"src":"ui","lvl":"debug","kind":"refresh",
               "round":1,"scope":"{scope}","origins":[],"contributingTraces":[],
               "collapsed":0,"ms":1.0}}"#
        );
        let rec: LogRecord = serde_json::from_str(&line).expect("a v2 line still deserialises");
        assert!(rec.repo.is_none(), "a missing repo key reads as None");
        rec
    }
    let mut h = H::new();
    h.feed(v2_refresh(1000, "full"));
    h.feed(v2_refresh(1400, "full"));
    assert_eq!(
        h.count("redundant-refresh"),
        1,
        "two v2 rounds of one scope fire exactly as they did pre-P117"
    );
}

// ---- AC2-11: boundedness --------------------------------------------------

/// §11 "bounded" is literal, and both new key spaces are unbounded in principle
/// (a repoId is a filesystem path). Each of 250 repos really fires — an `arm`
/// that never stamps would make this vacuous — and the maps must still hold only
/// the keys that fired inside one window.
#[test]
fn ac2_11_repo_keyed_debounce_maps_stay_bounded() {
    let mut h = H::new();
    for i in 0..250i64 {
        let repo = format!(r"D:\Repos\r{i}");
        // Pairs 400 ms apart, repos 2 s apart — past W_REFRESH_MS (1 s).
        let base = 10_000 + i * 2_000;
        h.feed(with_repo(refresh(base, "full"), &repo));
        h.feed(with_repo(refresh(base + 400, "full"), &repo));
    }
    assert_eq!(h.count("redundant-refresh"), 250, "every pair really fires");
    let len = h.detector().refresh_last_fire_len();
    assert!(
        len <= 4,
        "redundant-refresh debounce map must stay within one window's keys, got {len}"
    );

    let mut h = H::new();
    for i in 0..250i64 {
        let repo = format!(r"D:\Repos\r{i}");
        // Five misses inside 1 s, repos 11 s apart — past W_CACHE_MS (10 s).
        let base = 100_000 + i * 11_000;
        for k in 0..5 {
            h.feed(cache_span(base + k * 10, "miss", Some(repo.as_str())));
        }
    }
    assert_eq!(h.count("cache-collapse"), 250, "every repo really fires");
    let len = h.detector().cache_last_fire_len();
    assert!(
        len <= 4,
        "cache-collapse debounce map must stay within one window's keys, got {len}"
    );
}
