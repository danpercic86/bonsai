//! P28 digest tests for `ai_explain` (range resolution, walk caps, empty-range
//! guard, meta formatting). Extracted verbatim from the former inline
//! `mod tests`; shared git2 fixtures live in `test_support`.

use super::test_support::*;
use super::*;

// ---- P28 digest tests -----------------------------------------------------

/// §10.1(1): `AiDigestRange` deserializes the exact TS JSON per variant.
#[test]
fn digest_range_deserializes_each_variant() {
    let br: AiDigestRange =
        serde_json::from_str(r#"{"kind":"betweenRefs","from":"main","to":"feature"}"#)
            .expect("betweenRefs");
    match br {
        AiDigestRange::BetweenRefs { from, to } => {
            assert_eq!(from, "main");
            assert_eq!(to, "feature");
        }
        other => panic!("expected BetweenRefs, got {other:?}"),
    }

    let ld: AiDigestRange =
        serde_json::from_str(r#"{"kind":"lastDays","days":7}"#).expect("lastDays");
    match ld {
        AiDigestRange::LastDays { days } => assert_eq!(days, 7),
        other => panic!("expected LastDays, got {other:?}"),
    }

    let sc: AiDigestRange =
        serde_json::from_str(r#"{"kind":"sinceCommit","oid":"deadbeef"}"#).expect("sinceCommit");
    match sc {
        AiDigestRange::SinceCommit { oid } => assert_eq!(oid, "deadbeef"),
        other => panic!("expected SinceCommit, got {other:?}"),
    }
}

/// §10.1(6): 250 synthetic metas → 200 lines + "... and 50 more commits".
#[test]
fn format_commit_meta_caps_at_200() {
    let lines: Vec<String> = (0..250).map(|i| format!("- {i:07} line")).collect();
    let out = format_commit_meta(&lines);
    assert_eq!(out.lines().count(), MAX_DIGEST_COMMITS + 1);
    assert!(out.ends_with("... and 50 more commits"), "got tail: {out:?}");
    assert!(out.starts_with("- 0000000 line"));
    // Under the cap: joined verbatim, no overflow note.
    let small = format_commit_meta(&lines[..3]);
    assert_eq!(small.lines().count(), 3);
    assert!(!small.contains("more commits"));
}


/// §10.1(2): BetweenRefs{main, feature} → exactly [D, C] newest-first,
/// old_tree = B's tree; header carries the count.
#[test]
fn between_refs_walk_yields_range_commits_and_merge_base_tree() {
    let (dir, [_a, b, c, d]) = digest_fixture();
    let repo = git2::Repository::open(dir.path()).expect("open");
    let range = AiDigestRange::BetweenRefs {
        from: "main".to_string(),
        to: "feature".to_string(),
    };
    let (header, commits, old_tree, new_tree) =
        resolve_digest_range(&repo, &range).expect("resolve");
    let ids: Vec<git2::Oid> = commits.iter().map(|c| c.id()).collect();
    assert_eq!(ids, vec![d, c], "newest-first D then C");
    let b_tree = repo.find_commit(b).expect("B").tree().expect("tree").id();
    assert_eq!(old_tree.expect("old tree").id(), b_tree);
    assert_eq!(new_tree.id(), repo.find_commit(d).expect("D").tree().expect("t").id());
    assert!(header.contains("RANGE main..feature (2 commits)"), "got {header}");
    assert!(!header.contains("no common ancestor"));
}

/// Audit §3.15: a BetweenRefs range LARGER than the walk cap materializes
/// at most `MAX_DIGEST_WALK_COMMITS` commits (walk stops — no unbounded
/// collection) and records the truncation in the header, while the old/new
/// trees still anchor the FULL range.
#[test]
fn between_refs_walk_is_capped_and_marks_truncation() {
    let dir = init_scratch();
    let repo = git2::Repository::open(dir.path()).expect("open");
    let t = 1_700_000_000i64;
    let root = commit_at(&repo, None, "root", t, &[]);
    let mut tip = root;
    for i in 0..(MAX_DIGEST_WALK_COMMITS + 5) {
        let prev = repo.find_commit(tip).expect("prev");
        tip = commit_at(&repo, None, &format!("c{i}"), t + 10 * (i as i64 + 1), &[&prev]);
    }

    let range = AiDigestRange::BetweenRefs {
        from: root.to_string(),
        to: tip.to_string(),
    };
    let (header, commits, old_tree, new_tree) =
        resolve_digest_range(&repo, &range).expect("resolve");
    assert_eq!(commits.len(), MAX_DIGEST_WALK_COMMITS, "walk capped");
    assert_eq!(commits[0].id(), tip, "newest first");
    assert!(
        header.contains(&format!("({MAX_DIGEST_WALK_COMMITS}+ commits)")),
        "got {header}"
    );
    assert!(header.contains("truncated"), "got {header}");
    // Trees still anchor the full range (merge-base(root, tip) == root).
    assert_eq!(
        old_tree.expect("old tree").id(),
        repo.find_commit(root).expect("root").tree().expect("t").id()
    );
    assert_eq!(
        new_tree.id(),
        repo.find_commit(tip).expect("tip").tree().expect("t").id()
    );

    // An in-cap range stays untruncated (no "+", no note). (HEAD is unborn
    // in this fixture — the commits hang off no ref — so use BetweenRefs.)
    let small = AiDigestRange::BetweenRefs {
        from: commits[2].id().to_string(),
        to: tip.to_string(),
    };
    let (small_header, small_commits, _o, _n) =
        resolve_digest_range(&repo, &small).expect("resolve small");
    assert_eq!(small_commits.len(), 2);
    assert!(small_header.contains("(2 commits)"), "got {small_header}");
    assert!(!small_header.contains("truncated"), "got {small_header}");
}

/// §10.1(2): `from == to` → zero commits → `digest_changes` returns
/// `AiFailed("no changes in the selected range")` BEFORE any CLI call.
#[test]
fn empty_range_fails_before_cli() {
    let (dir, _) = digest_fixture();
    let err = digest_changes(
        dir.path(),
        AiDigestRange::BetweenRefs {
            from: "feature".to_string(),
            to: "feature".to_string(),
        },
        RunOpts::default(),
    )
    .expect_err("empty range must fail");
    match err {
        AppError::AiFailed(m) => assert_eq!(m, "no changes in the selected range"),
        other => panic!("expected AiFailed, got {other:?}"),
    }
}

/// §10.1(3): SinceCommit{B} ≡ BetweenRefs{B, HEAD} → [D, C].
#[test]
fn since_commit_is_between_refs_to_head() {
    let (dir, [_a, b, c, d]) = digest_fixture();
    let repo = git2::Repository::open(dir.path()).expect("open");
    let range = AiDigestRange::SinceCommit { oid: b.to_string() };
    let (_h, commits, old_tree, _new) = resolve_digest_range(&repo, &range).expect("resolve");
    let ids: Vec<git2::Oid> = commits.iter().map(|cm| cm.id()).collect();
    assert_eq!(ids, vec![d, c]);
    assert_eq!(
        old_tree.expect("old tree").id(),
        repo.find_commit(b).expect("B").tree().expect("t").id()
    );
}

/// §10.1(4): unrelated histories → no hide (full `to` history), old_tree
/// None (empty tree), header carries the no-common-ancestor note.
#[test]
fn unrelated_histories_diff_vs_empty_tree_with_note() {
    let (dir, [a, b, c, d]) = digest_fixture();
    let repo = git2::Repository::open(dir.path()).expect("open");
    let r = commit_at(&repo, None, "ROOT2", 1_700_000_100, &[]);
    let r_c = repo.find_commit(r).expect("R");
    repo.branch("other", &r_c, true).expect("other");

    let range = AiDigestRange::BetweenRefs {
        from: "other".to_string(),
        to: "feature".to_string(),
    };
    let (header, commits, old_tree, _new) = resolve_digest_range(&repo, &range).expect("resolve");
    let ids: Vec<git2::Oid> = commits.iter().map(|cm| cm.id()).collect();
    assert_eq!(ids, vec![d, c, b, a], "full feature history, newest first");
    assert!(old_tree.is_none(), "unrelated → empty-tree base");
    assert!(header.contains("no common ancestor"), "got {header}");
}

/// §10.1(5): lastDays first-parent walk with controlled committer times —
/// commits at now−1d/−2d/−10d; days=7 collects the two recent, boundary =
/// the 10-day-old commit; days=0 → InvalidName; all-in-window → old_tree None.
#[test]
fn last_days_walk_cutoff_and_boundary() {
    let dir = init_scratch();
    let repo = git2::Repository::open(dir.path()).expect("open");
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_secs() as i64;
    let day = 86_400i64;
    let old = commit_at(&repo, Some("HEAD"), "old-10d", now - 10 * day, &[]);
    let old_c = repo.find_commit(old).expect("old");
    let mid = commit_at(&repo, Some("HEAD"), "mid-2d", now - 2 * day, &[&old_c]);
    let mid_c = repo.find_commit(mid).expect("mid");
    let new = commit_at(&repo, Some("HEAD"), "new-1d", now - day, &[&mid_c]);

    let (header, commits, old_tree, new_tree) =
        resolve_digest_range(&repo, &AiDigestRange::LastDays { days: 7 }).expect("resolve");
    let ids: Vec<git2::Oid> = commits.iter().map(|cm| cm.id()).collect();
    assert_eq!(ids, vec![new, mid], "two in-window commits, newest first");
    assert_eq!(
        old_tree.expect("boundary tree").id(),
        old_c.tree().expect("t").id(),
        "boundary = the 10-day-old commit's tree"
    );
    assert_eq!(new_tree.id(), repo.find_commit(new).expect("n").tree().expect("t").id());
    assert!(header.contains("last 7 day(s)"), "got {header}");
    assert!(header.contains("(2 commits)"), "got {header}");

    // days=0 → InvalidName, before any repo access matters.
    let err = resolve_digest_range(&repo, &AiDigestRange::LastDays { days: 0 })
        .expect_err("days=0 must fail");
    assert!(matches!(err, AppError::InvalidName(_)), "got {err:?}");

    // Whole history inside the window → old_tree None (diff vs empty tree).
    let (_h, commits, old_tree, _n) =
        resolve_digest_range(&repo, &AiDigestRange::LastDays { days: 30 }).expect("resolve");
    assert_eq!(commits.len(), 3);
    assert!(old_tree.is_none(), "all-in-window → empty-tree base");
}

/// The metadata line format: `- {short7} {YYYY-MM-DD} {author}  {subject}`.
#[test]
fn commit_meta_line_format() {
    let (dir, [_a, _b, _c, d]) = digest_fixture();
    let repo = git2::Repository::open(dir.path()).expect("open");
    let d_c = repo.find_commit(d).expect("D");
    let line = commit_meta_line(&d_c);
    let short7: String = d.to_string().chars().take(7).collect();
    assert_eq!(line, format!("- {short7} 2023-11-14 Test User  D"));
}
