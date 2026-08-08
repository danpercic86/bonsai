//! AI semantic-history synthesis (P57c contract §3.1/§3.5). Retrieves the top-K
//! relevant commits from the persisted BM25 index (P57a/b), re-fetches the REAL
//! first-parent diff for the top few, renders a labeled grounding payload, and
//! asks the local `claude` CLI to answer the developer's natural-language
//! question — GROUNDED in real commits (C1 "WHY, not WHAT"). Read-only prose out;
//! WRITES NOTHING. Pure git2 + crate::ai.
//!
//! No index / no relevant commits ⇒ `AiFailed(...)` BEFORE any CLI call (OQ3 —
//! reuse `aiFailed`, no new `AppError` variant), mirroring `summarize_range`'s
//! empty-range guard. Local-`claude`-CLI-only (OD1): `RunOpts::default()`, no
//! model-tier abstraction.

use std::path::Path;

use crate::ai::payload;
use crate::ai::{self, RunOpts};
use crate::error::AppError;
use crate::git::ai_explain::cap_review_payload;
use crate::git::diff::{commit_diff, commit_file_diff, FileDiff};
use crate::git::history_index::{search_history, HistoryHit, HistoryQuery};
use crate::git::timefmt::epoch_to_ymd;

/// Of the retrieved top-K, how many are grounded with a FULL real diff in the
/// synthesis payload (the rest ride only as a commit list). Keeps the CLI
/// payload bounded (contract §2.1 / §3.5).
pub const SYNTH_DIFF_K: usize = 8;

/// System prompt (via `--append-system-prompt`) for the history answer (contract
/// §3.5, verbatim). SINGLE line — on Windows the `claude` CLI is a `.cmd` shim and
/// Rust's `Command` REFUSES an argv arg containing a newline; multi-line content
/// only ever flows through the stdin payload.
const HISTORY_SYSTEM_PROMPT: &str = "You are answering a developer's question about a git repository's history, using ONLY the commits provided on standard input. Explain the WHY — the intent and evolution — and cite the specific commits by their short hash (e.g. a1b2c3d). If the provided commits do not contain the answer, say so plainly rather than guessing. Be concise. Output prose only — no markdown code fences.";

/// The `-p` positional prompt (contract §3.5, verbatim single line).
const HISTORY_PROMPT: &str = "Answer the question on standard input from the provided commits, citing commit hashes.";

/// AI answer grounded in retrieved commits (contract §2.2). Serialize camelCase
/// (mirrored in TS). Prose `text` mirrors `AiAnalysis`, plus the citations and
/// the retrieved set (for the UI list + reveal).
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryAnswer {
    /// Fence-stripped prose answer.
    pub text: String,
    /// Short-oids the answer references (best-effort parse of 7+-hex tokens that
    /// prefix a retrieved oid), for UI emphasis.
    pub cited: Vec<String>,
    /// The commits fed to the model (drives the results list + reveal).
    pub retrieved: Vec<HistoryHit>,
    pub cost_usd: Option<f64>,
}

/// Blocking, CPU-heavy (a diff per top hit) ⇒ ALWAYS call under `spawn_blocking`.
/// Retrieves the top-K commits from the persisted index, re-fetches the REAL
/// first-parent diff for the top [`SYNTH_DIFF_K`], renders the §3.5 grounding
/// payload, calls `run_claude`, and parses citations. No index / no relevant
/// commits ⇒ `AiFailed(...)` BEFORE any CLI call (OQ3). Errors: `aiFailed`
/// (no index / no matches / CLI fail) | `git` (bad oid) | (`aiUnavailable` via
/// the command-layer gate).
pub fn answer_history(
    workdir: &Path,
    index_dir: &Path,
    question: &str,
    top_k: usize,
    opts: RunOpts,
) -> Result<HistoryAnswer, AppError> {
    // 1. An empty question is nothing to answer — no retrieval, no CLI.
    let question = question.trim();
    if question.is_empty() {
        return Err(AppError::AiFailed(
            "ask a question to search the history".to_string(),
        ));
    }

    // 2. Retrieve the top-K over the persisted index (pure IR; touches no git
    //    objects). `top_k` is already clamped by the command layer; 0 ⇒ default.
    let results = search_history(
        workdir,
        index_dir,
        &HistoryQuery {
            text: question.to_string(),
            top_k: u32::try_from(top_k).unwrap_or(u32::MAX),
        },
    )?;

    // 3. Guard BEFORE any CLI call (mirrors `summarize_range`). Distinguish "no
    //    index yet" (offer a build) from "index has no relevant commits".
    if results.hits.is_empty() {
        if results.indexed_commits == 0 {
            return Err(AppError::AiFailed(
                "history index not built — build it first".to_string(),
            ));
        }
        return Err(AppError::AiFailed(
            "no commits in the history match that question".to_string(),
        ));
    }
    let hits = results.hits;

    // 4. Assemble the labeled grounding payload (multi-line ⇒ stdin ONLY).
    let payload_text = build_history_payload(workdir, question, &hits)?;

    // 5. Ask the CLI (system prompt set here; caller's `opts` carry model/timeout).
    let result = ai::run_claude(
        workdir,
        HISTORY_PROMPT,
        Some(&payload_text),
        RunOpts {
            system_prompt: Some(HISTORY_SYSTEM_PROMPT.to_string()),
            ..opts
        },
    )?;

    // 6. Best-effort citations, then hand back the retrieved set for the UI.
    let cited = parse_cited(&result.text, &hits);
    Ok(HistoryAnswer {
        text: result.text,
        cited,
        retrieved: hits,
        cost_usd: result.cost_usd,
    })
}

/// Builds the §3.5 grounding payload: a QUESTION header, RELEVANT COMMITS
/// (`render_commit_list` of ALL hits), then TOP MATCHES IN DETAIL — per-hit
/// COMMIT/AUTHOR/MESSAGE/CHANGES for the top [`SYNTH_DIFF_K`] hits, whose CHANGES
/// are the commit's REAL first-parent diff (`render_file_diffs`). The whole
/// string is byte-capped by `cap_review_payload` (per-section caps ride inside
/// `render_file_diffs`). `pub(crate)` so the payload shape is unit-testable
/// without a CLI. Blocking (a diff per top hit).
pub(crate) fn build_history_payload(
    workdir: &Path,
    question: &str,
    hits: &[HistoryHit],
) -> Result<String, AppError> {
    // RELEVANT COMMITS: the full retrieved set as short-oid · summary · author.
    let commit_lines: Vec<payload::CommitLine> = hits
        .iter()
        .map(|h| payload::CommitLine {
            short_oid: short7(&h.oid),
            summary: h.summary.clone(),
            author: h.author_name.clone(),
        })
        .collect();
    let relevant = payload::render_commit_list(&commit_lines);

    // TOP MATCHES IN DETAIL: re-fetch the REAL first-parent diff for the top
    // SYNTH_DIFF_K hits so the answer is grounded in the CURRENT diff (D3), and
    // render COMMIT/AUTHOR/MESSAGE/CHANGES per hit.
    let mut detail = String::new();
    for hit in hits.iter().take(SYNTH_DIFF_K) {
        let cd = commit_diff(workdir, &hit.oid)?;
        let date = epoch_to_ymd(cd.details.author_ts);
        let mut file_diffs: Vec<FileDiff> = Vec::with_capacity(cd.files.len());
        for h in &cd.files {
            let fd = commit_file_diff(workdir, &hit.oid, &h.path, h.orig_path.as_deref(), false)?;
            file_diffs.push(fd);
        }
        let rendered = payload::render_file_diffs(&file_diffs);
        detail.push_str(&format!(
            "COMMIT {}  {}\nAUTHOR {}  {}\nMESSAGE:\n{}\nCHANGES:\n{}\n",
            short7(&cd.details.oid),
            cd.details.summary,
            cd.details.author_name,
            date,
            cd.details.message,
            rendered.text
        ));
    }

    let payload_text = format!(
        "QUESTION:\n{question}\n\nRELEVANT COMMITS (most relevant first):\n{relevant}\n===== TOP MATCHES IN DETAIL =====\n{detail}"
    );
    Ok(cap_review_payload(payload_text))
}

/// First 7 hex chars of a full oid (char-safe though oids are ASCII).
fn short7(oid: &str) -> String {
    oid.chars().take(7).collect()
}

/// Best-effort citation parse (contract §2.2/§3.5): scan the answer for maximal
/// hex runs of length >= 7 and, for each that is a PREFIX of some retrieved oid,
/// record that commit's short-7. Deduped, retrieved order. Requiring a real-oid
/// prefix keeps hex-looking English words ("added", "beef") from false-citing.
fn parse_cited(text: &str, hits: &[HistoryHit]) -> Vec<String> {
    let mut cited: Vec<String> = Vec::new();
    for run in hex_runs(text) {
        if run.len() < 7 {
            continue;
        }
        let lower = run.to_lowercase();
        for h in hits {
            if h.oid.starts_with(&lower) {
                let s = short7(&h.oid);
                if !cited.contains(&s) {
                    cited.push(s);
                }
            }
        }
    }
    cited
}

/// Split `text` into maximal runs of ASCII hex digits (all single-byte, so a
/// run's byte length equals its char count).
fn hex_runs(text: &str) -> Vec<String> {
    let mut runs = Vec::new();
    let mut cur = String::new();
    for c in text.chars() {
        if c.is_ascii_hexdigit() {
            cur.push(c);
        } else if !cur.is_empty() {
            runs.push(std::mem::take(&mut cur));
        }
    }
    if !cur.is_empty() {
        runs.push(cur);
    }
    runs
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::history_index::{build_index, search_history, HistoryQuery};

    /// The prompt/system-prompt consts MUST be single-line (Windows argv
    /// constraint): a newline in either would make `claude.cmd` reject the arg.
    #[test]
    fn prompts_are_single_line() {
        for s in [HISTORY_SYSTEM_PROMPT, HISTORY_PROMPT] {
            assert!(!s.contains('\n'), "prompt must be single-line: {s:?}");
            assert!(!s.contains('\r'), "prompt must be single-line: {s:?}");
        }
    }

    /// §8.4-adjacent: serde casing must match the TS `HistoryAnswer` type
    /// (`text` / `cited` / `retrieved` / `costUsd`); `None` cost ⇒ `null`, and a
    /// nested `HistoryHit` keeps its own camelCase (`authorName` / `authorTs`).
    #[test]
    fn history_answer_wire_shape_is_camel_case() {
        let v = serde_json::to_value(HistoryAnswer {
            text: "we moved off X for licensing".to_string(),
            cited: vec!["a1b2c3d".to_string()],
            retrieved: vec![HistoryHit {
                oid: "a1b2c3d4e5f60718293a4b5c6d7e8f9012345678".to_string(),
                summary: "drop library X".to_string(),
                author_name: "Ada".to_string(),
                author_ts: 42,
                score: 1.5,
            }],
            cost_usd: Some(0.01),
        })
        .expect("json");
        assert_eq!(v["text"], "we moved off X for licensing");
        assert_eq!(v["cited"], serde_json::json!(["a1b2c3d"]));
        assert_eq!(v["retrieved"][0]["authorName"], "Ada");
        assert_eq!(v["retrieved"][0]["authorTs"], 42);
        assert_eq!(v["costUsd"], 0.01);

        let none = serde_json::to_value(HistoryAnswer {
            text: "no cost".to_string(),
            cited: Vec::new(),
            retrieved: Vec::new(),
            cost_usd: None,
        })
        .expect("json");
        assert_eq!(none["costUsd"], serde_json::Value::Null, "None -> null");
        assert_eq!(none["cited"], serde_json::json!([]));
    }

    /// §7.14: `parse_cited` extracts the short-7 of a retrieved commit whose oid
    /// the answer references (7-char prefix OR the full 40-hex), dedupes, and
    /// ignores hex-looking words that are NOT a real-oid prefix.
    #[test]
    fn parse_cited_extracts_referenced_short_oids() {
        let hits = vec![
            HistoryHit {
                oid: "a1b2c3d4e5f60718293a4b5c6d7e8f9012345678".to_string(),
                summary: "one".to_string(),
                author_name: "Ada".to_string(),
                author_ts: 1,
                score: 2.0,
            },
            HistoryHit {
                oid: "fedcba98765432100123456789abcdef01234567".to_string(),
                summary: "two".to_string(),
                author_name: "Ada".to_string(),
                author_ts: 2,
                score: 1.0,
            },
        ];
        // References the first by short-7 (twice → deduped) and the second by its
        // full oid; "added" is hex-looking (5 chars) but under the 7 floor, and
        // "deadbeef" is 8 hex chars but prefixes NO retrieved oid → ignored.
        let text = "As of a1b2c3d we dropped it (see also a1b2c3d), then \
                    fedcba98765432100123456789abcdef01234567 reverted. added deadbeef";
        let cited = parse_cited(text, &hits);
        assert_eq!(cited, vec!["a1b2c3d".to_string(), "fedcba9".to_string()]);

        // No hex references → empty.
        assert!(parse_cited("nothing hexy here at all", &hits).is_empty());
    }

    // ---- git-fixture payload-shape test (pure — no CLI) ----------------------

    /// git2-init a `main`-headed scratch repo with pinned identity + autocrlf off
    /// (mirrors the history_index / diff fixtures).
    fn init_scratch() -> (tempfile::TempDir, git2::Repository) {
        let dir = crate::testutil::scratch_dir();
        let repo = git2::Repository::init_opts(
            dir.path(),
            git2::RepositoryInitOptions::new().initial_head("main"),
        )
        .expect("init repo");
        {
            let mut cfg = repo.config().expect("config");
            cfg.set_str("user.name", "Test User").expect("name");
            cfg.set_str("user.email", "test@example.com").expect("email");
            cfg.set_bool("core.autocrlf", false).expect("autocrlf");
        }
        (dir, repo)
    }

    /// One commit built from `parent`'s tree + text `files`, on HEAD, both times
    /// pinned to `t`. Returns the new oid.
    fn mk_commit(
        repo: &git2::Repository,
        parent: Option<git2::Oid>,
        files: &[(&str, &str)],
        msg: &str,
        t: i64,
    ) -> git2::Oid {
        let sig = git2::Signature::new("Ada Lovelace", "ada@example.com", &git2::Time::new(t, 0))
            .expect("sig");
        let parent_commit = parent.map(|p| repo.find_commit(p).expect("parent"));
        let mut tb = match &parent_commit {
            Some(pc) => repo
                .treebuilder(Some(&pc.tree().expect("parent tree")))
                .expect("tb"),
            None => repo.treebuilder(None).expect("tb"),
        };
        for (name, content) in files {
            let blob = repo.blob(content.as_bytes()).expect("blob");
            tb.insert(name, blob, 0o100_644).expect("insert");
        }
        let tree = repo.find_tree(tb.write().expect("write tree")).expect("tree");
        let parents: Vec<&git2::Commit> = parent_commit.iter().collect();
        repo.commit(Some("HEAD"), &sig, &sig, msg, &tree, &parents)
            .expect("commit")
    }

    /// §7.12 (payload half — no CLI): over a built index, the assembled payload
    /// carries the labeled QUESTION / RELEVANT COMMITS / TOP MATCHES IN DETAIL
    /// sections, the top hit's MESSAGE + CHANGES (the REAL diff `===== FILE:`
    /// block), and that hit's short-7 in a COMMIT line.
    #[test]
    fn history_payload_has_grounding_sections() {
        let (dir, repo) = init_scratch();
        let c0 = mk_commit(&repo, None, &[("a.txt", "alpha\n")], "seed alpha", 1000);
        let c1 = mk_commit(&repo, Some(c0), &[("b.txt", "beta\n")], "add beta", 2000);
        let c2 = mk_commit(
            &repo,
            Some(c1),
            &[("c.txt", "zebracorn payload\n")],
            "wire the zebracorn subsystem",
            3000,
        );
        let _c3 = mk_commit(&repo, Some(c2), &[("d.txt", "delta\n")], "delta cleanup", 4000);
        let idx = crate::testutil::scratch_dir();
        build_index(dir.path(), idx.path(), |_p| {}).expect("build index");

        let results = search_history(
            dir.path(),
            idx.path(),
            &HistoryQuery {
                text: "zebracorn".to_string(),
                top_k: 0,
            },
        )
        .expect("search");
        assert!(!results.hits.is_empty(), "the keyword commit is retrieved");

        let payload = build_history_payload(dir.path(), "why zebracorn?", &results.hits)
            .expect("payload");

        assert!(payload.contains("QUESTION:\nwhy zebracorn?"), "{payload}");
        assert!(
            payload.contains("RELEVANT COMMITS (most relevant first):"),
            "{payload}"
        );
        assert!(payload.contains("===== TOP MATCHES IN DETAIL ====="), "{payload}");
        assert!(payload.contains("MESSAGE:"), "{payload}");
        assert!(payload.contains("CHANGES:"), "{payload}");
        // The top hit is c2; its short-7 heads a COMMIT line and its real diff
        // (the added c.txt) reaches the CHANGES section.
        let s7: String = c2.to_string().chars().take(7).collect();
        assert!(payload.contains(&format!("COMMIT {s7}")), "{payload}");
        assert!(payload.contains("===== FILE: c.txt"), "{payload}");
        assert!(payload.contains("+zebracorn payload"), "{payload}");
    }
}
