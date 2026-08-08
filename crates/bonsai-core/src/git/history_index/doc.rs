//! Per-commit document extraction + the shared tokenizer (P57a contract §3.2).
//!
//! A [`CommitDoc`] is the compact, persisted per-commit record the BM25 retriever
//! scores over: it stores TOKEN FREQUENCIES + display metadata only — NEVER the
//! raw diff text (contract D3). The real diff is re-fetched at synthesis time for
//! the top-K commits (P57c), so grounding is always against the current diff.
//!
//! [`extract_doc`] builds one document from a commit's full message + changed
//! file paths + a bounded sample of added/removed diff line text (first-parent
//! diff via `git/diff.rs::collect_file_diffs`, capped at [`MAX_DOC_DIFF_BYTES`],
//! binary files skipped, root commit vs the empty tree, merge vs first parent).
//!
//! [`tokenize`] is the ONE tokenizer both build and query use (so a query term
//! matches a doc term iff they tokenize identically): lowercase; split on
//! non-alphanumeric; split camelCase / snake_case into sub-tokens AND keep the
//! whole identifier; drop < 2-char tokens and a tiny stopword set. Deterministic.

use std::collections::HashMap;

use crate::error::AppError;
use crate::git::diff::{apply_find_similar, build_diff_options, collect_file_diffs, LineKind};

use super::MAX_DOC_DIFF_BYTES;

/// Message tokens weigh this much in `tf`/`dl`; diff/path tokens weigh 1
/// (contract §3.2 field boost). A message term is 3× a diff term — commit
/// messages are the strongest relevance signal.
pub const MSG_BOOST: u16 = 3;

/// Compact, persisted per-commit document (contract §3.2). Stores token
/// frequencies + metadata only — never raw diff text (D3). `dl` = the total
/// token count post field-boost (BM25 length normalization).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct CommitDoc {
    /// First message line, capped at 120 chars (drives the hit row).
    pub summary: String,
    pub author_name: String,
    pub author_ts: i64,
    /// Document length in tokens (post field-boost) for BM25 length norm.
    pub dl: u32,
    /// term → frequency; message terms are already field-boosted (§3.2).
    pub tf: HashMap<String, u16>,
}

/// A tiny stopword set (contract §3.2). Kept intentionally small — BM25's idf
/// already down-weights ubiquitous terms, so this only strips the very noisiest.
/// All entries are >= 2 chars (shorter tokens are dropped anyway).
const STOPWORDS: &[&str] = &[
    "the", "and", "for", "that", "this", "with", "from", "was", "are", "not", "but", "you", "all",
    "has", "have", "will", "its", "our", "were", "into",
];

/// Extract `oid`'s document (contract §3.2): full message (field-boosted) +
/// changed file paths + a bounded sample of added/removed diff line text. First
/// parent only (matches the app's "commit vs first parent" rule); root commit
/// diffs vs the empty tree; binary files contribute their path but not content.
pub fn extract_doc(repo: &git2::Repository, oid: git2::Oid) -> Result<CommitDoc, AppError> {
    let commit = repo.find_commit(oid)?;
    let message = String::from_utf8_lossy(commit.message_bytes()).into_owned();
    let summary = first_line_capped(&message, 120);
    let author = commit.author();
    let author_name = String::from_utf8_lossy(author.name_bytes()).into_owned();
    let author_ts = author.when().seconds();

    let mut tf: HashMap<String, u16> = HashMap::new();
    let mut dl: u32 = 0;

    // Message terms carry the field boost.
    push_tokens(&message, MSG_BOOST, &mut tf, &mut dl);

    // First-parent diff (root -> empty tree). A bounded sample of add/del line
    // text is tokenized; all changed file paths are tokenized regardless.
    let files = first_parent_files(repo, &commit)?;
    let mut diff_bytes: usize = 0;
    for fd in &files {
        // Changed file PATHS are always indexed (cheap, high-signal); only the
        // add/del LINE TEXT sample is byte-bounded (contract §3.2).
        push_tokens(&fd.path, 1, &mut tf, &mut dl);
        if let Some(orig) = &fd.orig_path {
            push_tokens(orig, 1, &mut tf, &mut dl);
        }
        if fd.binary || diff_bytes >= MAX_DOC_DIFF_BYTES {
            continue; // binary content skipped (§3.2); over-budget files: path only
        }
        for hunk in &fd.hunks {
            for line in &hunk.lines {
                if line.kind == LineKind::Context {
                    continue; // only added/removed text carries change signal
                }
                if diff_bytes >= MAX_DOC_DIFF_BYTES {
                    break;
                }
                diff_bytes = diff_bytes.saturating_add(line.content.len());
                push_tokens(&line.content, 1, &mut tf, &mut dl);
            }
            if diff_bytes >= MAX_DOC_DIFF_BYTES {
                break;
            }
        }
    }

    Ok(CommitDoc {
        summary,
        author_name,
        author_ts,
        dl,
        tf,
    })
}

/// Tokenize + accumulate into `tf`/`dl` at `weight` (saturating, so a
/// pathological repeat can never overflow `u16`/`u32`).
fn push_tokens(text: &str, weight: u16, tf: &mut HashMap<String, u16>, dl: &mut u32) {
    for tok in tokenize(text) {
        let entry = tf.entry(tok).or_insert(0);
        *entry = entry.saturating_add(weight);
        *dl = dl.saturating_add(weight as u32);
    }
}

/// The changed `FileDiff`s of `commit` vs its FIRST parent (root -> empty tree),
/// with rename detection — the tokenization source for [`extract_doc`].
fn first_parent_files(
    repo: &git2::Repository,
    commit: &git2::Commit,
) -> Result<Vec<crate::git::diff::FileDiff>, AppError> {
    let new_tree = commit.tree()?;
    let old_tree = if commit.parent_count() == 0 {
        None
    } else {
        Some(commit.parent(0)?.tree()?)
    };
    let mut opts = build_diff_options(&[], false);
    let mut diff = repo.diff_tree_to_tree(old_tree.as_ref(), Some(&new_tree), Some(&mut opts))?;
    apply_find_similar(&mut diff)?;
    collect_file_diffs(&diff)
}

/// First line of `s`, capped char-safe at `max` chars.
fn first_line_capped(s: &str, max: usize) -> String {
    s.lines().next().unwrap_or("").chars().take(max).collect()
}

/// Shared tokenizer (contract §3.2). Deterministic. See the module docs.
pub fn tokenize(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut word = String::new();
    for c in text.chars() {
        if c.is_alphanumeric() || c == '_' {
            word.push(c);
        } else if !word.is_empty() {
            flush_word(&word, &mut out);
            word.clear();
        }
    }
    if !word.is_empty() {
        flush_word(&word, &mut out);
    }
    out
}

/// Emit the sub-tokens of one raw word (camelCase + snake_case pieces) plus the
/// whole identifier (when it differs from a single sub-token), each subject to
/// the length/stopword filter.
fn flush_word(raw: &str, out: &mut Vec<String>) {
    let whole = raw.to_lowercase();
    let mut subs: Vec<String> = Vec::new();
    for part in raw.split('_') {
        if part.is_empty() {
            continue;
        }
        for piece in split_camel(part) {
            subs.push(piece);
        }
    }
    for s in &subs {
        keep(s.clone(), out);
    }
    // Keep the whole identifier only when it adds signal beyond the sub-tokens
    // (a simple word is its own only sub-token — don't double-count it).
    if !subs.iter().any(|s| s == &whole) {
        keep(whole, out);
    }
}

/// Split a (separator-free) word at camelCase / letter-digit boundaries; each
/// piece is lowercased. `getHTTPResponse2` -> [get, http, response, 2].
fn split_camel(word: &str) -> Vec<String> {
    let chars: Vec<char> = word.chars().collect();
    let mut pieces: Vec<String> = Vec::new();
    let mut cur = String::new();
    for i in 0..chars.len() {
        let c = chars[i];
        if i > 0 {
            let prev = chars[i - 1];
            let acronym_boundary = prev.is_uppercase()
                && c.is_uppercase()
                && chars.get(i + 1).is_some_and(|n| n.is_lowercase());
            let boundary = (prev.is_lowercase() && c.is_uppercase())
                || (prev.is_numeric() != c.is_numeric())
                || acronym_boundary;
            if boundary && !cur.is_empty() {
                pieces.push(std::mem::take(&mut cur));
            }
        }
        cur.push(c);
    }
    if !cur.is_empty() {
        pieces.push(cur);
    }
    pieces.into_iter().map(|p| p.to_lowercase()).collect()
}

/// Push `tok` iff it clears the length floor (>= 2 chars) and is not a stopword.
fn keep(tok: String, out: &mut Vec<String>) {
    if tok.chars().count() < 2 {
        return;
    }
    if STOPWORDS.contains(&tok.as_str()) {
        return;
    }
    out.push(tok);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// git2-init a `main`-headed scratch repo with pinned identity + autocrlf off
    /// (mirrors the diff/search fixtures).
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

    /// One commit built from `parent`'s tree + `files`, on HEAD, both times pinned
    /// to `t`. Returns the new oid.
    fn mk_commit(
        repo: &git2::Repository,
        parent: Option<git2::Oid>,
        files: &[(&str, &[u8])],
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
            let blob = repo.blob(content).expect("blob");
            tb.insert(name, blob, 0o100_644).expect("insert");
        }
        let tree = repo.find_tree(tb.write().expect("write tree")).expect("tree");
        let parents: Vec<&git2::Commit> = parent_commit.iter().collect();
        repo.commit(Some("HEAD"), &sig, &sig, msg, &tree, &parents)
            .expect("commit")
    }

    // ---------------------------------------------------------- §7.1 tokenize

    #[test]
    fn tokenize_splits_identifiers() {
        // camelCase -> sub-tokens + whole; lowercasing.
        let toks = tokenize("getUserId");
        assert!(toks.contains(&"get".to_string()));
        assert!(toks.contains(&"user".to_string()));
        assert!(toks.contains(&"id".to_string()));
        assert!(toks.contains(&"getuserid".to_string()), "keeps the whole");

        // snake_case -> sub-tokens + whole.
        let snake = tokenize("parse_http_header");
        assert!(snake.contains(&"parse".to_string()));
        assert!(snake.contains(&"http".to_string()));
        assert!(snake.contains(&"header".to_string()));
        assert!(snake.contains(&"parse_http_header".to_string()));

        // Acronym boundary: getHTTPResponse -> get, http, response.
        let acr = tokenize("getHTTPResponse");
        assert!(acr.contains(&"get".to_string()));
        assert!(acr.contains(&"http".to_string()));
        assert!(acr.contains(&"response".to_string()));

        // A simple word is emitted exactly once (not doubled as sub + whole).
        assert_eq!(tokenize("alpha"), vec!["alpha".to_string()]);

        // Lowercasing + non-alphanumeric split.
        assert_eq!(tokenize("Foo.Bar!"), vec!["foo".to_string(), "bar".to_string()]);

        // Short tokens (< 2 chars) and stopwords dropped; nothing survives here.
        assert!(tokenize("a I x").is_empty());
        assert!(tokenize("the and for with").is_empty());

        // Deterministic: same input, same output.
        assert_eq!(tokenize("mixItUp_now"), tokenize("mixItUp_now"));
    }

    // -------------------------------------------- §7.5 extract_doc bounds/parent

    #[test]
    fn extract_doc_root_vs_empty_tree() {
        let (_dir, repo) = init_scratch();
        let c0 = mk_commit(
            &repo,
            None,
            &[("alpha.txt", b"alphaword betaword\n")],
            "seed the project",
            1000,
        );
        let doc = extract_doc(&repo, c0).expect("extract");
        // Root commit diffs vs the empty tree => all content is Added.
        assert!(doc.tf.contains_key("alphaword"), "root diff content indexed");
        assert!(doc.tf.contains_key("betaword"));
        // Message terms are field-boosted (weight MSG_BOOST).
        assert_eq!(doc.tf.get("seed").copied(), Some(MSG_BOOST));
        assert_eq!(doc.summary, "seed the project");
    }

    #[test]
    fn extract_doc_merge_uses_first_parent() {
        let (_dir, repo) = init_scratch();
        let c0 = mk_commit(&repo, None, &[("base.txt", b"base\n")], "base", 1000);
        // main: adds alpha.txt.
        let c1 = mk_commit(
            &repo,
            Some(c0),
            &[("base.txt", b"base\n"), ("alpha.txt", b"alphaunique\n")],
            "add alpha",
            2000,
        );
        // feature: a DANGLING commit off c0 adding beta.txt (no alpha). Dangling
        // (ref = None) because HEAD is at c1 — mk_commit targets HEAD and would be
        // rejected for a c0-parented commit here.
        let cf = {
            let sig =
                git2::Signature::new("Ada Lovelace", "ada@example.com", &git2::Time::new(3000, 0))
                    .expect("sig");
            let c0_commit = repo.find_commit(c0).unwrap();
            let mut tb = repo.treebuilder(Some(&c0_commit.tree().unwrap())).unwrap();
            let beta = repo.blob(b"betaunique\n").expect("blob");
            tb.insert("beta.txt", beta, 0o100_644).expect("insert");
            let tree = repo.find_tree(tb.write().unwrap()).expect("tree");
            repo.commit(None, &sig, &sig, "add beta", &tree, &[&c0_commit])
                .expect("dangling beta commit")
        };
        // Merge cm with parents [c1, cf] (first = c1); tree has base+alpha+beta.
        let sig = git2::Signature::new("Ada Lovelace", "ada@example.com", &git2::Time::new(4000, 0))
            .expect("sig");
        let mut tb = repo
            .treebuilder(Some(&repo.find_commit(c1).unwrap().tree().unwrap()))
            .expect("tb");
        let beta_blob = repo.blob(b"betaunique\n").expect("blob");
        tb.insert("beta.txt", beta_blob, 0o100_644).expect("insert");
        let tree = repo.find_tree(tb.write().unwrap()).expect("tree");
        // Dangling merge (ref = None): HEAD is at cf, not the first parent c1, so
        // updating HEAD would be rejected — and extract_doc reads by oid anyway.
        let cm = repo
            .commit(
                None,
                &sig,
                &sig,
                "merge feature",
                &tree,
                &[&repo.find_commit(c1).unwrap(), &repo.find_commit(cf).unwrap()],
            )
            .expect("merge commit");

        let doc = extract_doc(&repo, cm).expect("extract");
        // Diff cm-vs-c1 introduces ONLY beta.txt (alpha was already in c1).
        assert!(doc.tf.contains_key("betaunique"), "first-parent diff content");
        assert!(
            !doc.tf.contains_key("alphaunique"),
            "second-parent-only content must be absent (first-parent rule)"
        );
    }

    #[test]
    fn extract_doc_bounds_large_and_skips_binary() {
        let (_dir, repo) = init_scratch();
        // A text file whose add-lines far exceed MAX_DOC_DIFF_BYTES of unique
        // tokens: early tokens are indexed, late tokens are cut off by the cap.
        let mut big = String::new();
        for i in 0..1200 {
            big.push_str(&format!("tok{i:05} "));
            if i % 10 == 9 {
                big.push('\n');
            }
        }
        // A binary file (embedded NUL): libgit2 flags it binary => content skipped.
        let binary: &[u8] = b"secretbinaryword\x00\x00\x01\x02rest";
        let c0 = mk_commit(
            &repo,
            None,
            &[("big.txt", big.as_bytes()), ("data.bin", binary)],
            "bulk",
            1000,
        );
        let doc = extract_doc(&repo, c0).expect("extract");
        // Early token present, a very-late token dropped by the byte cap.
        assert!(doc.tf.contains_key("tok00000"), "early diff token indexed");
        assert!(
            !doc.tf.contains_key("tok01199"),
            "late token beyond MAX_DOC_DIFF_BYTES must be truncated"
        );
        // Binary path is tokenized, but its content is NOT.
        assert!(doc.tf.contains_key("data"), "binary path token present");
        assert!(
            !doc.tf.contains_key("secretbinaryword"),
            "binary content must be skipped"
        );
    }

    #[test]
    fn commit_doc_round_trips_json() {
        let mut tf = HashMap::new();
        tf.insert("auth".to_string(), 3u16);
        let doc = CommitDoc {
            summary: "fix auth".to_string(),
            author_name: "Ada".to_string(),
            author_ts: 42,
            dl: 3,
            tf,
        };
        let json = serde_json::to_string(&doc).expect("ser");
        let back: CommitDoc = serde_json::from_str(&json).expect("de");
        assert_eq!(doc, back);
    }
}
