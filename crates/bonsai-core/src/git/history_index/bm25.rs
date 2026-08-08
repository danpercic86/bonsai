//! Okapi BM25 inverted-index stats + scoring (P57a contract §3.3).
//!
//! PURE — no git, no IO. This is the load-bearing retrieval algorithm, unit-
//! tested in isolation over a hand-built corpus. Corpus stats ([`Bm25Index`])
//! are derived from the [`CommitDoc`] store at build time and persisted, so
//! retrieval never recomputes them; per-doc `tf`/`dl` live in the docs.

use std::collections::{BTreeMap, HashMap, HashSet};

use super::doc::CommitDoc;

/// Standard Okapi BM25 term-frequency saturation (contract §3.3 / OQ4).
pub const K1: f32 = 1.2;
/// Standard Okapi BM25 length-normalization strength (contract §3.3 / OQ4).
pub const B: f32 = 0.75;

/// Corpus statistics for BM25 scoring — rebuilt from `docs` on every save.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Bm25Index {
    /// Corpus size (number of documents).
    pub n: u32,
    /// Mean document length in tokens (guarded to 1.0 for an empty corpus).
    pub avgdl: f32,
    /// Document frequency per term (how many docs contain it).
    pub df: HashMap<String, u32>,
}

impl Bm25Index {
    /// Build corpus stats over the doc store (contract §3.3 `build_stats`).
    /// O(total tokens); cheap next to extraction. `avgdl` is guarded to 1.0 when
    /// the corpus is empty or every doc is zero-length (avoids a 0/0 in scoring).
    pub fn build_stats(docs: &BTreeMap<String, CommitDoc>) -> Bm25Index {
        let mut df: HashMap<String, u32> = HashMap::new();
        let mut total_dl: u64 = 0;
        for doc in docs.values() {
            total_dl += doc.dl as u64;
            for term in doc.tf.keys() {
                *df.entry(term.clone()).or_insert(0) += 1;
            }
        }
        let n = docs.len() as u32;
        let avgdl = if n == 0 || total_dl == 0 {
            1.0
        } else {
            total_dl as f32 / n as f32
        };
        Bm25Index { n, avgdl, df }
    }

    /// Non-negative BM25(+) idf (contract §3.3): `ln(1 + (N - df + 0.5)/(df + 0.5))`.
    /// The `1 +` keeps the log argument >= 1 (for `df <= N`), so no term ever
    /// contributes a negative weight — even a term present in every document.
    pub fn idf(&self, term: &str) -> f32 {
        let df = *self.df.get(term).unwrap_or(&0) as f32;
        let n = self.n as f32;
        (1.0 + (n - df + 0.5) / (df + 0.5)).ln()
    }

    /// BM25 score of `doc` for the (already tokenized) query terms (contract
    /// §3.3 `score`). Each DISTINCT query term contributes once; terms the doc
    /// lacks are skipped. `avgdl > 0` and `doc.dl > 0` hold whenever `f > 0`
    /// (a term contributes to both `tf` and `dl`), so the denominator is finite.
    pub fn score(&self, query_terms: &[String], doc: &CommitDoc) -> f32 {
        let mut seen: HashSet<&str> = HashSet::new();
        let mut s = 0.0f32;
        for t in query_terms {
            if !seen.insert(t.as_str()) {
                continue;
            }
            let f = match doc.tf.get(t) {
                Some(&freq) if freq > 0 => freq as f32,
                _ => continue,
            };
            let denom = f + K1 * (1.0 - B + B * (doc.dl as f32) / self.avgdl);
            s += self.idf(t) * (f * (K1 + 1.0)) / denom;
        }
        s
    }
}

/// Rank the doc store for `query_terms`, returning `(oid, score)` for every doc
/// with a positive score, sorted by (score desc, author_ts desc, oid asc) — a
/// fully deterministic order — and truncated to `top_k`. PURE (no git, no IO):
/// the P57a build/store tests and the P57b retrieval command both use this.
pub fn rank<'a>(
    index: &Bm25Index,
    docs: &'a BTreeMap<String, CommitDoc>,
    query_terms: &[String],
    top_k: usize,
) -> Vec<(&'a str, f32)> {
    let mut scored: Vec<(&str, f32, i64)> = docs
        .iter()
        .map(|(oid, doc)| (oid.as_str(), index.score(query_terms, doc), doc.author_ts))
        .filter(|(_, s, _)| *s > 0.0)
        .collect();
    scored.sort_by(|a, b| {
        b.1.partial_cmp(&a.1)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(b.2.cmp(&a.2))
            .then(a.0.cmp(b.0))
    });
    scored
        .into_iter()
        .take(top_k)
        .map(|(oid, s, _)| (oid, s))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Hand-build a `CommitDoc` from `(term, freq)` pairs; `dl` = sum of freqs.
    fn doc(oid_ts: i64, terms: &[(&str, u16)]) -> CommitDoc {
        let mut tf = HashMap::new();
        let mut dl = 0u32;
        for (t, f) in terms {
            tf.insert((*t).to_string(), *f);
            dl += *f as u32;
        }
        CommitDoc {
            summary: String::new(),
            author_name: String::new(),
            author_ts: oid_ts,
            dl,
            tf,
        }
    }

    fn terms(words: &[&str]) -> Vec<String> {
        words.iter().map(|w| w.to_string()).collect()
    }

    // ---------------------------------------------------- §7.2 ranks relevant

    #[test]
    fn bm25_ranks_relevant_above_noise() {
        // 5-doc corpus. Only d1/d3 mention "auth"; d1 mentions it more strongly.
        let mut docs = BTreeMap::new();
        docs.insert("1".to_string(), doc(10, &[("auth", 4), ("login", 1)]));
        docs.insert("2".to_string(), doc(20, &[("render", 3), ("canvas", 2)]));
        docs.insert("3".to_string(), doc(30, &[("auth", 1), ("token", 5)]));
        docs.insert("4".to_string(), doc(40, &[("graph", 6)]));
        docs.insert("5".to_string(), doc(50, &[("diff", 2), ("hunk", 2)]));
        let index = Bm25Index::build_stats(&docs);

        let ranked = rank(&index, &docs, &terms(&["auth"]), 20);
        let oids: Vec<&str> = ranked.iter().map(|(o, _)| *o).collect();
        assert_eq!(oids, vec!["1", "3"], "only auth-bearing docs, strongest first");
        for (_, s) in &ranked {
            assert!(*s > 0.0);
        }

        // A term nobody has scores nothing.
        assert!(rank(&index, &docs, &terms(&["zzz"]), 20).is_empty());
    }

    // ---------------------------------------------------- §7.3 non-negative idf

    #[test]
    fn bm25_idf_non_negative_for_ubiquitous_term() {
        // "common" is in EVERY doc (df == n) — the classic negative-idf trap.
        let mut docs = BTreeMap::new();
        for i in 0..4 {
            docs.insert(i.to_string(), doc(i, &[("common", 1), ("uniq", 1)]));
        }
        let index = Bm25Index::build_stats(&docs);
        assert!(index.idf("common") >= 0.0, "idf must never be negative");
        // A rarer term has strictly higher idf than a ubiquitous one.
        let mut docs2 = docs.clone();
        docs2.insert("rare".to_string(), doc(99, &[("rare", 1)]));
        let index2 = Bm25Index::build_stats(&docs2);
        assert!(index2.idf("rare") > index2.idf("common"));
    }

    // ---------------------------------------------------- §7.4 field boost

    #[test]
    fn field_boost_prefers_message_match() {
        // Two docs of EQUAL length: doc "m" carries the query term with a boosted
        // frequency (as a message term would, weight MSG_BOOST=3), doc "d" carries
        // it once (a diff-only match) padded to equal length. The boosted doc wins.
        let mut docs = BTreeMap::new();
        docs.insert("m".to_string(), doc(10, &[("auth", 3), ("pad", 1)]));
        docs.insert("d".to_string(), doc(20, &[("auth", 1), ("pad", 3)]));
        let index = Bm25Index::build_stats(&docs);
        let ranked = rank(&index, &docs, &terms(&["auth"]), 20);
        assert_eq!(ranked[0].0, "m", "message-boosted match ranks first");
        assert!(ranked[0].1 > ranked[1].1);
    }

    #[test]
    fn empty_corpus_and_empty_query_are_safe() {
        let empty: BTreeMap<String, CommitDoc> = BTreeMap::new();
        let index = Bm25Index::build_stats(&empty);
        assert_eq!(index.n, 0);
        assert_eq!(index.avgdl, 1.0, "guarded avgdl for an empty corpus");
        assert!(rank(&index, &empty, &terms(&["x"]), 5).is_empty());

        let mut docs = BTreeMap::new();
        docs.insert("1".to_string(), doc(1, &[("a2", 1)]));
        let index = Bm25Index::build_stats(&docs);
        assert!(rank(&index, &docs, &[], 5).is_empty(), "empty query => no hits");
    }
}
