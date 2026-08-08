//! Relevance-ranked retrieval over the persisted history index (P57b contract
//! §3.1/§3.3). PURE IR — NOT AI-gated, and touches NO git objects: it loads the
//! store and BM25-scores `query.text` against the precomputed per-commit docs.
//! Empty/whitespace text or a missing store resolves to empty results.
//!
//! Split out of `mod.rs` to keep that file under the ~500-line limit; the wire
//! types + `search_history` are re-exported there so the public API is
//! `history_index::{HistoryQuery, HistoryHit, HistorySearchResults, search_history}`.

use std::path::Path;

use crate::error::AppError;

use super::doc::{tokenize, CommitDoc};
use super::{bm25, store, DEFAULT_TOP_K, HISTORY_INDEX_SCHEMA, MAX_TOP_K};

/// Retrieval query (pure IR; NOT AI). Deserialize camelCase (contract §2.1).
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryQuery {
    /// NL / keyword query; empty/whitespace ⇒ `Ok(empty)`, no work.
    pub text: String,
    /// `0` ⇒ [`DEFAULT_TOP_K`]; clamped to [`MAX_TOP_K`].
    #[serde(default)]
    pub top_k: u32,
}

/// One relevance-ranked commit (contract §2.1). Serialize camelCase. Overlaps
/// P50's `SearchMatch` so the results UI reuses `revealCommitByOid` + the graph
/// `matchRows` rings.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryHit {
    /// Full 40-hex → `revealCommitByOid`.
    pub oid: String,
    /// First message line, capped 120 (from the doc; drives the hit row).
    pub summary: String,
    pub author_name: String,
    pub author_ts: i64,
    /// BM25 relevance, descending.
    pub score: f32,
}

/// Ranked retrieval results (contract §2.1). Serialize camelCase.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistorySearchResults {
    /// Relevance-desc; tie-break author_ts desc (from `bm25::rank`).
    pub hits: Vec<HistoryHit>,
    /// Hint to offer a rebuild — `true` when no usable index exists yet.
    pub index_stale: bool,
    pub indexed_commits: u32,
}

/// Resolve the effective retrieval depth: `0` ⇒ [`DEFAULT_TOP_K`], then clamped
/// to [`MAX_TOP_K`] (contract §2.1).
fn effective_top_k(top_k: u32) -> usize {
    let k = if top_k == 0 { DEFAULT_TOP_K } else { top_k };
    k.min(MAX_TOP_K) as usize
}

/// Relevance-ranked retrieval over the persisted index (contract §3.1/§3.3).
/// Loads the store, tokenizes `query.text`, BM25-scores, and returns the top-K
/// hits (relevance-desc, author_ts tie-break). Empty/whitespace text ⇒ empty
/// hits (index present); no usable store ⇒ empty hits + `index_stale: true`.
///
/// Touches NO git objects, so `_workdir` is intentionally unused — it is kept in
/// the signature for symmetry with `build_index`/`index_status` (the command
/// layer passes the same workdir to all three).
pub fn search_history(
    _workdir: &Path,
    index_dir: &Path,
    query: &HistoryQuery,
) -> Result<HistorySearchResults, AppError> {
    // No usable index ⇒ empty + a rebuild hint (contract §2.3 IpcApi note).
    let store = match store::load(index_dir) {
        Some(s) if s.schema == HISTORY_INDEX_SCHEMA => s,
        _ => {
            return Ok(HistorySearchResults {
                hits: Vec::new(),
                index_stale: true,
                indexed_commits: 0,
            })
        }
    };
    let indexed_commits = store.docs.len() as u32;

    // Empty/whitespace query ⇒ no work (the index exists, so not stale here;
    // real ref-tip staleness is reported by `index_status`, which opens the repo).
    let terms = tokenize(query.text.trim());
    if terms.is_empty() {
        return Ok(HistorySearchResults {
            hits: Vec::new(),
            index_stale: false,
            indexed_commits,
        });
    }

    let ranked = bm25::rank(
        &store.bm25,
        &store.docs,
        &terms,
        effective_top_k(query.top_k),
    );
    let hits = ranked
        .into_iter()
        .filter_map(|(oid, score)| hit_from_doc(oid, score, store.docs.get(oid)))
        .collect();

    Ok(HistorySearchResults {
        hits,
        index_stale: false,
        indexed_commits,
    })
}

/// Assemble a `HistoryHit` from a scored oid + its doc. `bm25::rank` only returns
/// oids that key `docs`, so `doc` is always `Some`; the `Option` guard just keeps
/// this total without an `unwrap` on store-derived data.
fn hit_from_doc(oid: &str, score: f32, doc: Option<&CommitDoc>) -> Option<HistoryHit> {
    let doc = doc?;
    Some(HistoryHit {
        oid: oid.to_string(),
        summary: doc.summary.clone(),
        author_name: doc.author_name.clone(),
        author_ts: doc.author_ts,
        score,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    use crate::git::history_index::store::IndexStore;

    /// One hand-built doc: `(oid, summary, author, ts, [(term, freq)])`. oids are
    /// arbitrary keys here (as in the `bm25` unit tests) — `search_history` never
    /// parses them.
    type DocSpec<'a> = (&'a str, &'a str, &'a str, i64, &'a [(&'a str, u16)]);

    /// Hand-build a doc-only store (no git) and persist it, so retrieval tests
    /// stay pure.
    fn save_store(index_dir: &Path, docs: &[DocSpec]) {
        let mut store = IndexStore::empty();
        for (oid, summary, author, ts, terms) in docs {
            let mut tf = HashMap::new();
            let mut dl = 0u32;
            for (t, f) in *terms {
                tf.insert((*t).to_string(), *f);
                dl += *f as u32;
            }
            store.docs.insert(
                (*oid).to_string(),
                CommitDoc {
                    summary: (*summary).to_string(),
                    author_name: (*author).to_string(),
                    author_ts: *ts,
                    dl,
                    tf,
                },
            );
        }
        store.bm25 = bm25::Bm25Index::build_stats(&store.docs);
        store::save(index_dir, &store).expect("save store");
    }

    fn q(text: &str, top_k: u32) -> HistoryQuery {
        HistoryQuery {
            text: text.to_string(),
            top_k,
        }
    }

    #[test]
    fn no_store_is_empty_and_stale() {
        let dir = crate::testutil::scratch_dir();
        let res = search_history(dir.path(), &dir.path().join("absent"), &q("anything", 0))
            .expect("search");
        assert!(res.hits.is_empty());
        assert!(res.index_stale, "a missing index hints a rebuild");
        assert_eq!(res.indexed_commits, 0);
    }

    #[test]
    fn empty_text_does_no_work_but_reports_count() {
        let dir = crate::testutil::scratch_dir();
        let idx = dir.path().join("idx");
        save_store(&idx, &[("c1", "seed alpha", "Ada", 1000, &[("alpha", 3)])]);
        for text in ["", "   ", "\t\n "] {
            let res = search_history(dir.path(), &idx, &q(text, 0)).expect("search");
            assert!(res.hits.is_empty(), "empty/whitespace query => no hits");
            assert!(!res.index_stale, "the index exists => not stale");
            assert_eq!(res.indexed_commits, 1);
        }
    }

    /// Pure-store companion to mod.rs's git-fixture §7.6 test: the term unique to
    /// one commit returns that oid first through the public `search_history` API.
    #[test]
    fn search_history_ranks_unique_term_first() {
        let dir = crate::testutil::scratch_dir();
        let idx = dir.path().join("idx");
        save_store(
            &idx,
            &[
                ("c1", "seed alpha", "Ada", 1000, &[("alpha", 3)]),
                ("c2", "add beta", "Ada", 2000, &[("beta", 3)]),
                (
                    "c3",
                    "wire the zebracorn subsystem",
                    "Ada",
                    3000,
                    &[("zebracorn", 3), ("subsystem", 3)],
                ),
                ("c4", "delta cleanup", "Ada", 4000, &[("delta", 3)]),
            ],
        );
        let res = search_history(dir.path(), &idx, &q("zebracorn", 0)).expect("search");
        assert_eq!(res.hits.len(), 1, "only the zebracorn commit matches");
        assert_eq!(res.hits[0].oid, "c3", "unique-term commit ranks first");
        assert_eq!(res.hits[0].summary, "wire the zebracorn subsystem");
        assert!(res.hits[0].score > 0.0);
        assert!(!res.index_stale);
        assert_eq!(res.indexed_commits, 4);
    }

    #[test]
    fn top_k_clamps_and_defaults() {
        assert_eq!(effective_top_k(0), DEFAULT_TOP_K as usize, "0 => default");
        assert_eq!(effective_top_k(5), 5);
        assert_eq!(effective_top_k(MAX_TOP_K), MAX_TOP_K as usize);
        assert_eq!(effective_top_k(9_999), MAX_TOP_K as usize, "clamped to max");
    }

    // ---------------------------------------------------- wire shapes

    #[test]
    fn history_query_deserializes_with_default_top_k() {
        // `topK` omitted ⇒ serde default 0 (⇒ DEFAULT_TOP_K at query time).
        let q: HistoryQuery = serde_json::from_str(r#"{"text":"x"}"#).expect("de");
        assert_eq!(q.text, "x");
        assert_eq!(q.top_k, 0, "omitted topK defaults to 0");

        let q2: HistoryQuery = serde_json::from_str(r#"{"text":"y","topK":7}"#).expect("de");
        assert_eq!(q2.top_k, 7, "camelCase topK deserializes");
    }

    #[test]
    fn history_hit_wire_shape_is_camel_case() {
        let v = serde_json::to_value(HistoryHit {
            oid: "abc".to_string(),
            summary: "fix auth".to_string(),
            author_name: "Ada".to_string(),
            author_ts: 42,
            score: 1.5,
        })
        .expect("json");
        assert_eq!(v["oid"], "abc");
        assert_eq!(v["summary"], "fix auth");
        assert_eq!(v["authorName"], "Ada");
        assert_eq!(v["authorTs"], 42);
        assert_eq!(v["score"], 1.5);
    }

    #[test]
    fn history_search_results_wire_shape_is_camel_case() {
        let v = serde_json::to_value(HistorySearchResults {
            hits: Vec::new(),
            index_stale: true,
            indexed_commits: 3,
        })
        .expect("json");
        assert_eq!(v["hits"], serde_json::json!([]));
        assert_eq!(v["indexStale"], true);
        assert_eq!(v["indexedCommits"], 3);
    }
}
