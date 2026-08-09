//! T5 property suite (contract §2.3): BM25 round-trip + idf/tf/rank invariants.
//! Corpus is built directly as `CommitDoc`s (cheaper than a repo); one
//! deterministic end-to-end `build_index`→`search_history` test guards the real
//! git-backed path.

#[path = "prop_common/mod.rs"]
mod prop_common;

use std::collections::HashMap;

use bonsai_core::git::history_index::bm25::{rank, Bm25Index};
use bonsai_core::git::history_index::doc::CommitDoc;
use bonsai_core::git::history_index::{build_index, search_history, HistoryQuery};
use proptest::prelude::*;

use prop_common::common;

/// Build a `CommitDoc` from `(term, freq)` pairs; `dl` = sum of freqs.
fn mk_doc(author_ts: i64, terms: &[(String, u16)]) -> CommitDoc {
    let mut tf = HashMap::new();
    let mut dl = 0u32;
    for (t, f) in terms {
        *tf.entry(t.clone()).or_insert(0) += *f;
        dl += *f as u32;
    }
    CommitDoc {
        summary: String::new(),
        author_name: String::new(),
        author_ts,
        dl,
        tf,
    }
}

/// A corpus: each doc is `(author_ts, Vec<(term, freq)>)`. Terms are `[a-y]`
/// only (no 'z') so a `z…` nonsense token is guaranteed absent.
fn corpus_strat() -> impl Strategy<Value = std::collections::BTreeMap<String, CommitDoc>> {
    let doc = (
        any::<i64>(),
        prop::collection::vec(("[a-y]{2,10}", 1u16..=6), 1..=8),
    );
    prop::collection::vec(doc, 1..=10).prop_map(|docs| {
        docs.into_iter()
            .enumerate()
            .map(|(i, (ts, terms))| (format!("{i:040}"), mk_doc(ts, &terms)))
            .collect()
    })
}

fn terms(words: &[&str]) -> Vec<String> {
    words.iter().map(|w| w.to_string()).collect()
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 64, ..ProptestConfig::default() })]

    /// Item 1 (round-trip): a fresh nonsense token injected into exactly one doc
    /// makes that doc the sole, top-ranked hit; a never-injected token is absent.
    #[test]
    fn round_trip_unique_token(
        mut docs in corpus_strat(),
        target_sel in any::<usize>(),
        nonsense in "z[a-z]{3,8}",
        other in "z[a-z]{3,8}",
    ) {
        prop_assume!(nonsense != other);
        let keys: Vec<String> = docs.keys().cloned().collect();
        let target = keys[target_sel % keys.len()].clone();
        // Inject the nonsense term into exactly the target doc.
        if let Some(d) = docs.get_mut(&target) {
            *d.tf.entry(nonsense.clone()).or_insert(0) += 3;
            d.dl += 3;
        }
        let index = Bm25Index::build_stats(&docs);
        let hits = rank(&index, &docs, &terms(&[&nonsense]), 50);
        prop_assert_eq!(hits.len(), 1, "only the injected doc matches");
        prop_assert_eq!(hits[0].0, target.as_str(), "injected doc ranks first");
        prop_assert!(hits[0].1 > 0.0);

        // A token nobody carries returns nothing.
        let none = rank(&index, &docs, &terms(&[&other]), 50);
        prop_assert!(none.is_empty(), "absent token ⇒ zero hits");
    }

    /// Item 2 (idf finite + non-negative) for present AND absent terms.
    #[test]
    fn idf_is_finite_and_non_negative(
        docs in corpus_strat(),
        probe in "[a-z]{2,10}",
    ) {
        let index = Bm25Index::build_stats(&docs);
        for t in [probe.as_str(), "aa", "zzzzz"] {
            let v = index.idf(t);
            prop_assert!(v.is_finite(), "idf finite for {t:?}: {v}");
            prop_assert!(v >= 0.0, "idf non-negative for {t:?}: {v}");
        }
    }

    /// Item 3 (tf monotonicity): equal `dl`, more occurrences ⇒ strictly higher
    /// score.
    #[test]
    fn score_monotone_in_tf(hi in 2u16..=8, lo in 1u16..=7, pad_total in 8u32..=20) {
        prop_assume!(hi > lo);
        let dl_target = pad_total.max(hi as u32) + 4;
        // doc A: "auth" hi times + padding to dl_target.
        let a = mk_doc(10, &[("auth".to_string(), hi), ("pad".to_string(), (dl_target - hi as u32) as u16)]);
        let b = mk_doc(20, &[("auth".to_string(), lo), ("pad".to_string(), (dl_target - lo as u32) as u16)]);
        prop_assert_eq!(a.dl, b.dl, "equal document length");
        let mut docs = std::collections::BTreeMap::new();
        docs.insert("a".to_string(), a);
        docs.insert("b".to_string(), b);
        let index = Bm25Index::build_stats(&docs);
        let sa = index.score(&terms(&["auth"]), &docs["a"]);
        let sb = index.score(&terms(&["auth"]), &docs["b"]);
        prop_assert!(sa > sb, "more tf ⇒ higher score: {sa} > {sb}");
    }

    /// Item 4 (rank contract): score-desc, ties author_ts-desc, len <= top_k,
    /// all scores positive.
    #[test]
    fn rank_is_sorted_and_capped(
        docs in corpus_strat(),
        query in prop::collection::vec("[a-y]{2,10}", 1..=4),
        top_k in 1usize..=8,
    ) {
        let index = Bm25Index::build_stats(&docs);
        let hits = rank(&index, &docs, &query, top_k);
        prop_assert!(hits.len() <= top_k, "capped to top_k");
        for w in hits.windows(2) {
            let (oa, sa) = w[0];
            let (ob, sb) = w[1];
            prop_assert!(sa >= sb, "score descending: {sa} >= {sb}");
            if sa == sb {
                let ta = docs[oa].author_ts;
                let tb = docs[ob].author_ts;
                prop_assert!(ta >= tb, "tie broken by author_ts desc");
            }
        }
        for (_, s) in &hits {
            prop_assert!(*s > 0.0, "only positive-score hits");
        }
    }
}

// ---- deterministic end-to-end (git-backed build + search) -------------------

/// One real repo: build the index and confirm the commit carrying a unique word
/// is retrieved first (guards the extraction + persistence + retrieval path).
#[test]
fn end_to_end_build_and_search() {
    let dir = common::scratch_dir();
    let idx = common::scratch_dir();
    let repo = git2::Repository::init_opts(
        dir.path(),
        git2::RepositoryInitOptions::new().initial_head("main"),
    )
    .expect("init");
    {
        let mut cfg = repo.config().expect("cfg");
        cfg.set_str("user.name", "Ada").expect("n");
        cfg.set_str("user.email", "ada@example.com").expect("e");
    }
    let mk = |parent: Option<git2::Oid>, file: &str, content: &str, msg: &str, t: i64| {
        let sig = git2::Signature::new("Ada", "ada@example.com", &git2::Time::new(t, 0)).unwrap();
        let pc = parent.map(|p| repo.find_commit(p).unwrap());
        let mut tb = match &pc {
            Some(c) => repo.treebuilder(Some(&c.tree().unwrap())).unwrap(),
            None => repo.treebuilder(None).unwrap(),
        };
        let blob = repo.blob(content.as_bytes()).unwrap();
        tb.insert(file, blob, 0o100_644).unwrap();
        let tree = repo.find_tree(tb.write().unwrap()).unwrap();
        let parents: Vec<&git2::Commit> = pc.iter().collect();
        repo.commit(Some("HEAD"), &sig, &sig, msg, &tree, &parents).unwrap()
    };
    let c0 = mk(None, "a.txt", "alpha\n", "seed alpha", 1000);
    let c1 = mk(Some(c0), "b.txt", "beta\n", "wire the zebracorn subsystem", 2000);
    let _c2 = mk(Some(c1), "c.txt", "gamma\n", "delta cleanup", 3000);

    build_index(dir.path(), idx.path(), |_p| {}).expect("build_index");
    let res = search_history(
        dir.path(),
        idx.path(),
        &HistoryQuery { text: "zebracorn".into(), top_k: 0 },
    )
    .expect("search");
    assert!(!res.hits.is_empty(), "unique term retrieved");
    assert_eq!(res.hits[0].oid, c1.to_string(), "unique-term commit ranks first");
}
