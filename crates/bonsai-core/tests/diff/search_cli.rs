//! T2 Area 6 — commit search: adversarial queries as LITERAL text, the F-A6-C
//! case-fold pin, truncation, unicode folding, and garbled-ref resilience.
//!
//! Message/Author/All modes never shell out (git2 revwalk + `contains_fold`), so
//! those tests are git-free and deterministic. The Content pickaxe twin-pair
//! (F-A6-C: `-i` folds the `-S` literal too) needs the real `git` CLI and skips
//! when it is absent.

use std::path::Path;

use bonsai_core::error::AppError;
use bonsai_core::git::commit::create_commit;
use bonsai_core::git::search::{search_commits, SearchField, SearchQuery, SpawnGitRunner};
use bonsai_core::git::stage::stage_paths;
use crate::common;

fn init_repo() -> tempfile::TempDir {
    let dir = common::scratch_dir();
    let repo = git2::Repository::init(dir.path()).expect("init");
    let mut cfg = repo.config().expect("config");
    cfg.set_str("user.name", "Test User").expect("name");
    cfg.set_str("user.email", "test@example.com").expect("email");
    cfg.set_bool("core.autocrlf", false).expect("autocrlf");
    dir
}

fn commit(dir: &Path, file: &str, content: &str, msg: &str) -> String {
    std::fs::write(dir.join(file), content).expect("write");
    stage_paths(dir, &[file.to_string()]).expect("stage");
    create_commit(dir, msg, None, false).expect("commit").oid
}

/// A SearchQuery with sensible defaults (case-insensitive, default cap, all refs).
fn q(field: SearchField, text: &str) -> SearchQuery {
    SearchQuery {
        text: text.to_string(),
        field,
        regex: false,
        case_sensitive: false,
        max_results: 0,
        scope_ref: None,
    }
}

fn oids(r: &bonsai_core::git::search::SearchResults) -> Vec<String> {
    r.matches.iter().map(|m| m.oid.clone()).collect()
}

/// Adversarial query TEXT (`--all`, `-Gfoo`, `--output=x`, a leading-dash token,
/// a regex-special string) is matched as a LITERAL substring — never parsed as a
/// git option (Message mode never shells out).
#[test]
fn adversarial_query_text_is_literal() {
    let dir = init_repo();
    let p = dir.path();
    // A benign base commit that must NOT match any of the probes.
    commit(p, "base.txt", "base\n", "ordinary work");
    let cases = [
        ("please pass --all here", "--all"),
        ("bug in -Gfoo handling", "-Gfoo"),
        ("set --output=json now", "--output=json"),
        ("weird -x flag doc", "-x flag"),
        ("regex a.*b literal", "a.*b"),
    ];
    let mut expected = Vec::new();
    for (i, (msg, _)) in cases.iter().enumerate() {
        expected.push(commit(p, &format!("f{i}.txt"), &format!("c{i}\n"), msg));
    }
    for (i, (_, needle)) in cases.iter().enumerate() {
        let res = search_commits(p, &SpawnGitRunner, &q(SearchField::Message, needle))
            .expect("search ok");
        let got = oids(&res);
        assert_eq!(got, vec![expected[i].clone()], "literal match for {needle:?}: {got:?}");
    }
}

/// An empty/whitespace query never opens the repo and returns an empty result.
#[test]
fn blank_query_is_empty() {
    let dir = init_repo();
    for text in ["", "   ", "\t\n"] {
        let res = search_commits(dir.path(), &SpawnGitRunner, &q(SearchField::Message, text))
            .expect("ok");
        assert!(res.matches.is_empty() && !res.truncated, "blank {text:?} → empty");
    }
}

/// Unborn HEAD (no commits) → an empty result, not an error.
#[test]
fn unborn_head_is_empty() {
    let dir = init_repo();
    let res = search_commits(dir.path(), &SpawnGitRunner, &q(SearchField::Message, "anything"))
        .expect("ok");
    assert!(res.matches.is_empty() && !res.truncated);
}

/// A `scope_ref` starting with `-` is rejected up front (option-injection guard)
/// for EVERY field, before any argv/revparse.
#[test]
fn scope_ref_leading_dash_is_rejected() {
    let dir = init_repo();
    commit(dir.path(), "a.txt", "a\n", "seed");
    for field in [SearchField::Message, SearchField::All, SearchField::Content, SearchField::Path] {
        let mut query = q(field, "seed");
        query.scope_ref = Some("-x".to_string());
        let err = search_commits(dir.path(), &SpawnGitRunner, &query)
            .expect_err("leading-dash scope must be rejected");
        assert!(matches!(err, AppError::Other(_)), "got {err:?}");
    }
}

/// Case folding is Unicode-aware: `contains_fold` lowercases both sides, so a
/// mixed-case unicode needle matches a mixed-case unicode message.
#[test]
fn unicode_case_insensitive_fold() {
    let dir = init_repo();
    let p = dir.path();
    let hit = commit(p, "u.txt", "u\n", "Résumé build — CAFÉ Ünïcode ЖЖ");
    // Lowercased needle with different original casing still matches.
    for needle in ["café", "жж", "résumé", "ünïcode"] {
        let res = search_commits(p, &SpawnGitRunner, &q(SearchField::Message, needle))
            .expect("ok");
        assert_eq!(oids(&res), vec![hit.clone()], "unicode fold for {needle:?}");
    }
}

/// Truncation is signalled, never silent: with `max_results = 2` and 3 matches
/// the flag is set and exactly 2 rows are returned.
#[test]
fn truncation_is_signalled() {
    let dir = init_repo();
    let p = dir.path();
    for i in 0..3 {
        commit(p, &format!("m{i}.txt"), &format!("{i}\n"), "matchme please");
    }
    let mut query = q(SearchField::Message, "matchme");
    query.max_results = 2;
    let res = search_commits(p, &SpawnGitRunner, &query).expect("ok");
    assert_eq!(res.matches.len(), 2, "capped to max_results");
    assert!(res.truncated, "more matches than the cap → truncated");
}

/// A dangling branch ref (points at a bogus oid) and a garbled loose-ref file
/// are SKIPPED by `seed_all_refs` — the search still returns the reachable HEAD
/// commits instead of aborting (F-A6-D).
#[test]
fn dangling_and_garbled_refs_survive() {
    let dir = init_repo();
    let p = dir.path();
    let hit = commit(p, "a.txt", "a\n", "findable commit");
    // A branch ref pointing at an oid that does not exist as a commit.
    let bogus = "0".repeat(39) + "1";
    std::fs::write(
        p.join(".git").join("refs").join("heads").join("dangling"),
        format!("{bogus}\n"),
    )
    .expect("write dangling ref");
    // A totally garbled loose ref file.
    std::fs::write(
        p.join(".git").join("refs").join("heads").join("garbled"),
        "this is not an oid at all\n",
    )
    .expect("write garbled ref");

    let res = search_commits(p, &SpawnGitRunner, &q(SearchField::All, "findable"))
        .expect("bad refs must not abort the search");
    assert!(oids(&res).contains(&hit), "reachable HEAD commit still found");
}

/// F-A6-C twin-pair: Content mode under the default (case-INsensitive) must fold
/// the `-S` pickaxe literal too — a lowercase needle finds a mixed-case content
/// change, matching `git log -i -S`. Skips without `git`.
#[test]
fn content_pickaxe_is_case_insensitive_like_cli() {
    if !common::have_git() {
        eprintln!("skipping: `git` CLI not found on PATH");
        return;
    }
    let dir = init_repo();
    let p = dir.path();
    commit(p, "base.txt", "base\n", "base");
    let target = commit(p, "code.txt", "let FooBarBaz = 1;\n", "add FooBarBaz");

    let res = search_commits(p, &SpawnGitRunner, &q(SearchField::Content, "foobarbaz"))
        .expect("content search ok");
    assert!(
        oids(&res).contains(&target),
        "case-insensitive -S must find the mixed-case content change"
    );

    // Oracle: `git log -i -S foobarbaz --all` returns the same commit.
    let cli = common::git(
        p,
        &["log", "-i", "--format=%H", "-Sfoobarbaz", "--all"],
    );
    assert!(cli.lines().any(|l| l == target), "CLI twin agrees: {cli}");
}
