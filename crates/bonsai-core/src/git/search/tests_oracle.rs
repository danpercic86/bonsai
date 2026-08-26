//! Oracle tests for `search`: git2 revwalk + shell modes cross-checked against
//! the real `git` CLI. Extracted verbatim from the former inline `mod tests`;
//! shared fixtures live in `test_support`.

use super::test_support::*;
use super::*;
use std::collections::BTreeSet;

// ---------------------------------------------------------- oracle: git2 modes

#[test]
fn oracle_message_matches_cli_ordered() {
    if !have_git() {
        eprintln!("skipping: `git` CLI not found");
        return;
    }
    let (dir, [c0, _c1, c2, _c3]) = build_fixture();
    let ours = our_oids(dir.path(), &q(SearchField::Message, "work"));
    let cli = cli_oids(
        dir.path(),
        &["log", "--all", "-i", "-F", "--grep=work", "--format=%H"],
    );
    assert_eq!(ours, cli, "message search == git log --grep");
    // Newest-first C2 then C0 (both messages carry "work").
    assert_eq!(ours, vec![oid_hex(c2), oid_hex(c0)]);
}

#[test]
fn oracle_message_case_sensitivity_differs() {
    if !have_git() {
        return;
    }
    let (dir, [_c0, _c1, _c2, c3]) = build_fixture();
    // Insensitive matches "Feature gamma".
    let ci = our_oids(dir.path(), &q(SearchField::Message, "feature"));
    let ci_cli = cli_oids(
        dir.path(),
        &["log", "--all", "-i", "-F", "--grep=feature", "--format=%H"],
    );
    assert_eq!(ci, ci_cli);
    assert_eq!(ci, vec![oid_hex(c3)]);
    // Sensitive matches nothing ("feature" != "Feature").
    let cs_query = SearchQuery {
        case_sensitive: true,
        ..q(SearchField::Message, "feature")
    };
    let cs = our_oids(dir.path(), &cs_query);
    let cs_cli = cli_oids(
        dir.path(),
        &["log", "--all", "-F", "--grep=feature", "--format=%H"],
    );
    assert_eq!(cs, cs_cli);
    assert!(cs.is_empty());
    assert_ne!(ci, cs, "case flag must change the result set");
}

#[test]
fn oracle_author_matches_cli() {
    if !have_git() {
        return;
    }
    let (dir, [c0, _c1, c2, _c3]) = build_fixture();
    let ours = our_oids(dir.path(), &q(SearchField::Author, "ada"));
    let cli = cli_oids(
        dir.path(),
        &["log", "--all", "-i", "-F", "--author=ada", "--format=%H"],
    );
    assert_eq!(ours, cli);
    assert_eq!(ours, vec![oid_hex(c2), oid_hex(c0)]);
}

#[test]
fn oracle_all_is_union_of_message_and_author() {
    if !have_git() {
        return;
    }
    let (dir, _) = build_fixture();
    let ours: BTreeSet<String> =
        our_oids(dir.path(), &q(SearchField::All, "grace")).into_iter().collect();
    let msg: BTreeSet<String> = cli_oids(
        dir.path(),
        &["log", "--all", "-i", "-F", "--grep=grace", "--format=%H"],
    )
    .into_iter()
    .collect();
    let author: BTreeSet<String> = cli_oids(
        dir.path(),
        &["log", "--all", "-i", "-F", "--author=grace", "--format=%H"],
    )
    .into_iter()
    .collect();
    let union: BTreeSet<String> = msg.union(&author).cloned().collect();
    assert_eq!(ours, union, "all == message ∪ author");
    // Non-degenerate: neither alone equals the union (message hits C0, author hits C1).
    assert_ne!(ours, msg);
    assert_ne!(ours, author);
}

#[test]
fn oracle_scope_ref_is_a_subset() {
    if !have_git() {
        return;
    }
    let (dir, [c0, _c1, _c2, _c3]) = build_fixture();
    let scoped = SearchQuery {
        scope_ref: Some("early".to_string()),
        ..q(SearchField::Message, "work")
    };
    let ours = our_oids(dir.path(), &scoped);
    let cli = cli_oids(
        dir.path(),
        &["log", "early", "-i", "-F", "--grep=work", "--format=%H"],
    );
    assert_eq!(ours, cli);
    // early = C1..C0, so only C0 carries "work" (C2 is outside the scope).
    assert_eq!(ours, vec![oid_hex(c0)]);
}

#[test]
fn oracle_message_cap_truncates() {
    if !have_git() {
        return;
    }
    let (dir, _) = build_fixture();
    // Every message contains "a"; cap at 2 ⇒ truncated with exactly 2 rows.
    let capped = SearchQuery {
        max_results: 2,
        ..q(SearchField::Message, "a")
    };
    let results = search_commits(dir.path(), &SpawnGitRunner, &capped).expect("search");
    assert!(results.truncated, "4 matches, cap 2 ⇒ truncated");
    assert_eq!(results.matches.len(), 2);
}

#[test]
fn oracle_empty_results_is_ok() {
    if !have_git() {
        return;
    }
    let (dir, _) = build_fixture();
    let results =
        search_commits(dir.path(), &SpawnGitRunner, &q(SearchField::Message, "zzznotfound"))
            .expect("search");
    assert!(results.matches.is_empty());
    assert!(!results.truncated);
}

#[test]
fn oracle_all_refs_seeds_remotes_and_tags() {
    // Cross-checks `seed_all_refs`' remote-tracking + tag + `*/HEAD`-skip
    // seeding, which build_fixture (local branches only) never exercised.
    if !have_git() {
        return;
    }
    let (dir, [c_base, c_remote, c_tag]) = build_refs_fixture();
    // Default scope = all refs. c_remote is reachable ONLY via origin/feature
    // and c_tag ONLY via the v1.0 tag, so a full-message search finding them
    // proves those refs are seeded — and it must match `git log --all` exactly.
    let ours = our_oids(dir.path(), &q(SearchField::Message, "work"));
    let cli = cli_oids(
        dir.path(),
        &["log", "--all", "-i", "-F", "--grep=work", "--format=%H"],
    );
    assert_eq!(ours, cli, "all-refs message search == git log --all --grep");
    // Newest-first by timestamp: c_tag(3000), c_remote(2000), c_base(1000).
    assert_eq!(ours, vec![oid_hex(c_tag), oid_hex(c_remote), oid_hex(c_base)]);
    // Spell out the load-bearing claim: the remote-only and tag-only commits
    // are present (not just reachable via the local `main` branch at c_base).
    assert!(ours.contains(&oid_hex(c_remote)), "remote-tracking ref seeded");
    assert!(ours.contains(&oid_hex(c_tag)), "tag ref seeded");
}

#[test]
fn oracle_all_matched_label_message_wins() {
    // `all` mode checks message BEFORE author; when BOTH match, the row must
    // be labelled Message. (The union oracle ignores the `matched` label.)
    if !have_git() {
        return;
    }
    let dir = crate::testutil::scratch_dir();
    let repo = init_repo(dir.path());
    // "hopper" is in BOTH the message and the author (name + email).
    let c = mk_commit(
        &repo,
        None,
        &[("f.txt", "x\n")],
        "hopper refactor",
        "Grace Hopper",
        1000,
    );
    // CLI confirms the match is genuinely on both fields (not just our claim).
    let by_msg = cli_oids(
        dir.path(),
        &["log", "--all", "-i", "-F", "--grep=hopper", "--format=%H"],
    );
    let by_author = cli_oids(
        dir.path(),
        &["log", "--all", "-i", "-F", "--author=hopper", "--format=%H"],
    );
    assert_eq!(by_msg, vec![oid_hex(c)], "message matches");
    assert_eq!(by_author, vec![oid_hex(c)], "author matches");
    // Our `all` search returns the one commit, labelled Message (message wins).
    let results = search_commits(dir.path(), &SpawnGitRunner, &q(SearchField::All, "hopper"))
        .expect("search");
    assert_eq!(results.matches.len(), 1);
    assert_eq!(results.matches[0].oid, oid_hex(c));
    assert_eq!(results.matches[0].matched, MatchedField::Message);
}

// ---------------------------------------------------------- oracle: shell modes

#[test]
fn oracle_path_matches_cli() {
    if !have_git() {
        return;
    }
    let (dir, [c0, _c1, c2, _c3]) = build_fixture();
    let ours = our_oids(dir.path(), &q(SearchField::Path, "a.txt"));
    let cli = cli_oids(dir.path(), &["log", "--all", "--format=%H", "--", "a.txt"]);
    assert_eq!(ours, cli);
    assert_eq!(ours, vec![oid_hex(c2), oid_hex(c0)]);
}

#[test]
fn oracle_content_pickaxe_s_matches_cli() {
    if !have_git() {
        return;
    }
    let (dir, _) = build_fixture();
    let ours = our_oids(dir.path(), &q(SearchField::Content, "alpha"));
    let cli = cli_oids(dir.path(), &["log", "--all", "--format=%H", "-Salpha"]);
    assert_eq!(ours, cli, "content -S == git log -S");
}

#[test]
fn oracle_content_pickaxe_g_regex_matches_cli() {
    if !have_git() {
        return;
    }
    let (dir, _) = build_fixture();
    let regex_query = SearchQuery {
        regex: true,
        ..q(SearchField::Content, "al.ha")
    };
    let ours = our_oids(dir.path(), &regex_query);
    // -i is added (default case-insensitive) — mirror it in the oracle.
    let cli = cli_oids(dir.path(), &["log", "--all", "-i", "--format=%H", "-Gal.ha"]);
    assert_eq!(ours, cli, "content -G == git log -G");
    assert!(!ours.is_empty(), "regex should match the alpha edits");
}

/// A REAL `git log` accepts the `--end-of-options` marker before an
/// explicit scope (audit §2.6) and returns the same rows as the plain CLI.
#[test]
fn oracle_path_scoped_with_end_of_options_matches_cli() {
    if !have_git() {
        return;
    }
    let (dir, [c0, _c1, _c2, _c3]) = build_fixture();
    let scoped = SearchQuery {
        scope_ref: Some("early".to_string()),
        ..q(SearchField::Path, "a.txt")
    };
    let ours = our_oids(dir.path(), &scoped);
    let cli = cli_oids(dir.path(), &["log", "early", "--format=%H", "--", "a.txt"]);
    assert_eq!(ours, cli, "scoped path search == git log <scope> -- <path>");
    // early = C1..C0; only C0 touches a.txt inside the scope.
    assert_eq!(ours, vec![oid_hex(c0)]);
}

#[test]
fn oracle_shell_cap_truncates() {
    if !have_git() {
        return;
    }
    let (dir, [_c0, _c1, c2, _c3]) = build_fixture();
    // a.txt has 2 touching commits; cap 1 ⇒ --max-count 2 ⇒ truncated, 1 row.
    let capped = SearchQuery {
        max_results: 1,
        ..q(SearchField::Path, "a.txt")
    };
    let results = search_commits(dir.path(), &SpawnGitRunner, &capped).expect("search");
    assert!(results.truncated, "2 matches, cap 1 ⇒ truncated");
    assert_eq!(results.matches.len(), 1);
    assert_eq!(results.matches[0].oid, oid_hex(c2)); // newest kept
}

#[test]
fn invalid_content_regex_is_git_error() {
    if !have_git() {
        return;
    }
    let (dir, _) = build_fixture();
    let bad = SearchQuery {
        regex: true,
        ..q(SearchField::Content, "[") // unterminated bracket ⇒ git exits non-zero
    };
    let err = search_commits(dir.path(), &SpawnGitRunner, &bad).expect_err("invalid regex");
    assert!(matches!(err, AppError::Git(_)), "got {err:?}");
}

// ---------------------------------------------------------- fake-runner argv

#[test]
fn spawn_path_passes_exact_argv_to_runner() {
    // The command dispatch hands the runner EXACTLY build_log_args' output.
    let runner = FakeGitRunner::new("");
    let query = q(SearchField::Path, "src/main.rs");
    let _ = search_commits(Path::new("/tmp/repo"), &runner, &query).expect("ok");
    let calls = runner.calls.borrow();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0], build_log_args(&query, MAX_SEARCH_RESULTS));
}

// ---------------------------------------------------------- F-A6-D

/// One garbled loose ref (branch AND tag) must be SKIPPED, not abort the
/// whole seeding — the walk still yields every reachable commit.
#[test]
fn seed_all_refs_skips_garbled_loose_refs() {
    let dir = crate::testutil::scratch_dir();
    let d = dir.path();
    let repo = git2::Repository::init(d).expect("init");
    {
        let mut cfg = repo.config().expect("config");
        cfg.set_str("user.name", "Test User").expect("name");
        cfg.set_str("user.email", "t@example.com").expect("email");
    }
    std::fs::write(d.join("f.txt"), "x\n").expect("write");
    crate::git::stage::stage_paths(d, &["f.txt".to_string()]).expect("stage");
    crate::git::commit::create_commit(d, "base", None, false).expect("commit");
    let head = repo.head().expect("HEAD").target().expect("oid");

    // Garbled loose refs: not-40-hex content in branch + tag ref files.
    std::fs::write(d.join(".git/refs/heads/garbled"), "not-a-hex-oid\n")
        .expect("garbled branch");
    std::fs::create_dir_all(d.join(".git/refs/tags")).expect("tags dir");
    std::fs::write(d.join(".git/refs/tags/garbled"), "also-garbage\n")
        .expect("garbled tag");

    let mut walk = repo.revwalk().expect("revwalk");
    seed_all_refs(&repo, &mut walk).expect("garbled refs must be skipped, not abort");
    let oids: Vec<git2::Oid> = walk.collect::<Result<_, _>>().expect("walk");
    assert!(oids.contains(&head), "HEAD commit still seeded, got {oids:?}");
}
