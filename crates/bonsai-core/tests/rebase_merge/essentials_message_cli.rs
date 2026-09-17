//! P47 CLI-oracle suite (contract §7.2), the EDITABLE cherry-pick message —
//! split out of `essentials_autostash_cli.rs` to keep each file under the
//! ~500-line limit. Declared as a child module of `essentials_autostash_cli`,
//! so it reuses that file's `require_git!` macro and twin-repo fixtures.
//!
//! Covers the normalized custom message on a clean pick and its survival across
//! a conflict pause via `MERGE_MSG`. Each test skips (passes with a note) if
//! `git` is absent. All scratch repos live under `D:\Data\Temp\bonsai-scratch`.

use super::{author_epoch, build_conflicting_pick, build_disjoint_pick, repo_state, tree_oid};
use crate::common;
use crate::common::{git, init_repo};
use bonsai_core::git::cherrypick::{cherrypick_commit, cherrypick_continue, CherrypickOutcome};
use bonsai_core::git::conflict::resolve_conflict_text;

// ====================================================== §7.2 custom-message parity

/// `cherrypick_commit(Some(m))` on a clean pick commits with the normalized
/// custom message, preserves the ORIGINAL author, uses the configured committer,
/// and produces the SAME tree as a plain `git cherry-pick`.
#[test]
fn p47_cherrypick_custom_message_matches_cli() {
    require_git!();
    let a_dir = init_repo();
    let b_dir = init_repo();
    let a = a_dir.path();
    let b = b_dir.path();

    let (pick_a, _main_a) = build_disjoint_pick(a);
    let (pick_b, _main_b) = build_disjoint_pick(b);
    assert_eq!(pick_a, pick_b, "twin pick oids must match");
    let pick_author_at = author_epoch(a, &pick_a);

    let custom = "custom subject\n\nbody line one\nbody line two";
    let outcome = cherrypick_commit(a, &pick_a, Some(custom)).expect("bonsai cherry-pick");
    match outcome {
        CherrypickOutcome::Committed { stashed, .. } => {
            assert!(!stashed, "clean tree → no autostash");
        }
        other => panic!("expected Committed, got {other:?}"),
    }

    // Twin: plain cherry-pick — the message differs but the TREE is identical
    // (message never affects the tree).
    git(b, &["cherry-pick", &pick_b]);
    assert_eq!(
        tree_oid(a),
        tree_oid(b),
        "custom-message pick tree must match a plain pick"
    );

    let repo = git2::Repository::open(a).expect("open A");
    let head = repo.head().expect("head").peel_to_commit().expect("peel");
    // Message == normalize(custom): trimmed + single trailing newline.
    assert_eq!(
        head.message().ok(),
        Some("custom subject\n\nbody line one\nbody line two\n"),
        "committed message must equal the normalized custom text"
    );
    // Author == the ORIGINAL commit's author (name + time preserved).
    assert_eq!(head.author().name().ok(), Some("Test User"));
    assert_eq!(head.author().email().ok(), Some("test@example.com"));
    assert_eq!(
        head.author().when().seconds(),
        pick_author_at,
        "custom-message pick preserves the original author time"
    );
    // Committer == the configured signature.
    assert_eq!(head.committer().name().ok(), Some("Test User"));
    assert_eq!(head.committer().email().ok(), Some("test@example.com"));
    assert_eq!(repo_state(a), git2::RepositoryState::Clean);
}

// ============================================ §7.2 custom-message survives conflict

/// A conflicting pick with `Some(m)` persists the normalized message to
/// `.git/MERGE_MSG`; after resolving + `cherrypick_continue`, the final commit
/// message equals `normalize(m)` (proving the override survives the pause).
#[test]
fn p47_cherrypick_custom_message_survives_conflict() {
    require_git!();
    let dir = init_repo();
    let d = dir.path();
    let pick = build_conflicting_pick(d);

    let custom = "reworded pick subject\n\nrationale in the body";
    match cherrypick_commit(d, &pick, Some(custom)).expect("bonsai cherry-pick") {
        CherrypickOutcome::Conflicts { paths, stashed } => {
            assert_eq!(paths, vec!["x.txt".to_string()], "x.txt must be conflicted");
            assert!(!stashed, "clean tree → no autostash");
        }
        other => panic!("expected Conflicts, got {other:?}"),
    }
    assert_eq!(repo_state(d), git2::RepositoryState::CherryPick);

    // The override is persisted (normalized) to MERGE_MSG so continue honors it.
    let merge_msg = std::fs::read_to_string(d.join(".git").join("MERGE_MSG")).expect("MERGE_MSG");
    assert_eq!(
        merge_msg, "reworded pick subject\n\nrationale in the body\n",
        "the custom message must be persisted to MERGE_MSG across the pause"
    );

    // Resolve + continue.
    resolve_conflict_text(d, "x.txt", "line1\nresolved\nline3\n").expect("resolve index");
    match cherrypick_continue(d).expect("bonsai continue") {
        CherrypickOutcome::Committed { .. } => {}
        other => panic!("expected Committed after resolve, got {other:?}"),
    }

    let repo = git2::Repository::open(d).expect("open");
    let head = repo.head().expect("head").peel_to_commit().expect("peel");
    assert_eq!(
        head.message().ok(),
        Some("reworded pick subject\n\nrationale in the body\n"),
        "the final commit message must be the custom override, not the picked message"
    );
    assert_eq!(repo_state(d), git2::RepositoryState::Clean);
}
