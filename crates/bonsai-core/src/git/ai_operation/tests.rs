//! Plan-spine tests: the two NON-NEGOTIABLE safety guarantees
//! (`plan_never_mutates`, `out_of_allowlist_is_unsupported`) and the
//! deserialize lock. The per-intent resolution/preview tests live next to the
//! code they exercise in the `ai_operation_resolve` module.
//!
//! Extracted verbatim from the former inline `mod tests`; shared fixtures live
//! in `test_support`.

use super::test_support::{expect_unsupported, linear_repo, merge_repo, snapshot};
use super::*;
use crate::git::ai_operation_resolve::resolve_intent;

// ----------------------------------------------- §11.1 plan_never_mutates

/// §11.1 (NON-NEGOTIABLE): the resolve+preview path (the only repo-touching
/// code after the read-only grounding + pure CLI text transform) mutates
/// NOTHING for EVERY intent — the four reset/revert intents, the six P55b
/// ones (switch/create/delete/stash/discard/merge), the escape hatch, and
/// unparseable garbage. The full `plan_operation` spawn path is additionally
/// proven in `tests/ai_operation_cli.rs` (process-isolated from the CLI env).
#[test]
fn plan_never_mutates() {
    let (dir, a, _m, _branch) = merge_repo();
    let p = dir.path();
    let short_a: String = a.chars().take(7).collect();
    let repo = git2::Repository::open(p).expect("open");

    let replies: Vec<String> = vec![
        r#"{"intent":"undoLastCommit","keepChanges":true}"#.to_string(),
        r#"{"intent":"undoLastCommit","keepChanges":false}"#.to_string(),
        r#"{"intent":"undoLastMerge"}"#.to_string(),
        format!(r#"{{"intent":"resetToCommit","commit":"{short_a}","keepChanges":true}}"#),
        format!(r#"{{"intent":"revertCommit","commit":"{short_a}"}}"#),
        r#"{"intent":"switchBranch","branch":"feature"}"#.to_string(),
        r#"{"intent":"createBranch","name":"x","atCommit":null}"#.to_string(),
        r#"{"intent":"deleteBranch","branch":"feature"}"#.to_string(),
        r#"{"intent":"stashChanges","message":null,"includeUntracked":true}"#.to_string(),
        r#"{"intent":"discardChanges","paths":["a.txt"]}"#.to_string(),
        r#"{"intent":"mergeBranch","branch":"feature"}"#.to_string(),
        r#"{"intent":"unsupported","reason":"nope"}"#.to_string(),
        "this is not JSON at all".to_string(),
        "git reset --hard HEAD~5".to_string(),
    ];

    let before = snapshot(p);
    for reply in &replies {
        // Ignore the outcome; the guarantee under test is "writes nothing".
        let _ = plan_from_reply(&repo, reply, Some(0.001)).expect("plan_from_reply");
        assert_eq!(
            snapshot(p),
            before,
            "plan resolution mutated the repo for reply: {reply}"
        );
    }
}

// --------------------------------------- §11.2 out_of_allowlist_is_unsupported

/// §11.2 (NON-NEGOTIABLE): every off-allowlist model output — invalid JSON,
/// an unknown tag, a raw shell string, an unresolvable ref, and
/// undoLastMerge-when-HEAD-is-not-a-merge — yields `Ok(Unsupported)` (NOT a
/// guessed op, NOT `Err`), and mutates nothing.
#[test]
fn out_of_allowlist_is_unsupported() {
    let (dir, _a, _b) = linear_repo();
    let p = dir.path();
    let repo = git2::Repository::open(p).expect("open");
    let before = snapshot(p);

    // (1) invalid JSON, (2) unknown tag, (3) raw shell string — all fail the
    // CLOSED parse and degrade to Unsupported.
    for reply in [
        "not json",
        r#"{"intent":"rmRf"}"#,
        "git reset --hard HEAD~5",
        r#"{"intent":"deleteEverything","force":true}"#,
    ] {
        let outcome = plan_from_reply(&repo, reply, None).expect("Ok(Unsupported)");
        expect_unsupported(outcome);
    }

    // (4) unresolvable ref (a P55a intent that passes the parse but fails L4).
    let bad_ref = resolve_intent(
        &repo,
        AiOpIntent::ResetToCommit {
            commit: "no-such-ref".to_string(),
            keep_changes: true,
        },
        None,
    )
    .expect("Ok");
    assert!(expect_unsupported(bad_ref).contains("couldn't find a commit"));

    // (5) undoLastMerge when HEAD is NOT a merge.
    let not_merge = resolve_intent(&repo, AiOpIntent::UndoLastMerge, None).expect("Ok");
    assert!(expect_unsupported(not_merge).contains("isn't a merge"));

    assert_eq!(snapshot(p), before, "rejecting an intent must mutate nothing");
}

// ---------------------------------------------------------- §11.9 deserialize

/// §11.9: `AiOpIntent` deserializes from the EXACT JSON the TS union / the
/// system prompt describe, incl. `keepChanges` and `atCommit:null`; an
/// unknown tag is an Err (⇒ fail-closed at the call site).
#[test]
fn ai_op_intent_deserializes_each_variant() {
    let p = |s: &str| serde_json::from_str::<AiOpIntent>(s);

    match p(r#"{"intent":"undoLastCommit","keepChanges":true}"#).expect("undoLastCommit") {
        AiOpIntent::UndoLastCommit { keep_changes } => assert!(keep_changes),
        other => panic!("got {other:?}"),
    }
    // keepChanges omitted → serde default false.
    match p(r#"{"intent":"undoLastCommit"}"#).expect("undoLastCommit default") {
        AiOpIntent::UndoLastCommit { keep_changes } => assert!(!keep_changes),
        other => panic!("got {other:?}"),
    }
    assert!(matches!(
        p(r#"{"intent":"undoLastMerge"}"#).expect("undoLastMerge"),
        AiOpIntent::UndoLastMerge
    ));
    match p(r#"{"intent":"resetToCommit","commit":"a1b2c3d","keepChanges":false}"#)
        .expect("resetToCommit")
    {
        AiOpIntent::ResetToCommit {
            commit,
            keep_changes,
        } => {
            assert_eq!(commit, "a1b2c3d");
            assert!(!keep_changes);
        }
        other => panic!("got {other:?}"),
    }
    match p(r#"{"intent":"revertCommit","commit":"a1b2c3d"}"#).expect("revertCommit") {
        AiOpIntent::RevertCommit { commit } => assert_eq!(commit, "a1b2c3d"),
        other => panic!("got {other:?}"),
    }
    match p(r#"{"intent":"switchBranch","branch":"main"}"#).expect("switchBranch") {
        AiOpIntent::SwitchBranch { branch } => assert_eq!(branch, "main"),
        other => panic!("got {other:?}"),
    }
    match p(r#"{"intent":"createBranch","name":"feat/x","atCommit":null}"#)
        .expect("createBranch")
    {
        AiOpIntent::CreateBranch { name, at_commit } => {
            assert_eq!(name, "feat/x");
            assert_eq!(at_commit, None);
        }
        other => panic!("got {other:?}"),
    }
    match p(r#"{"intent":"deleteBranch","branch":"old"}"#).expect("deleteBranch") {
        AiOpIntent::DeleteBranch { branch } => assert_eq!(branch, "old"),
        other => panic!("got {other:?}"),
    }
    match p(r#"{"intent":"stashChanges","message":null,"includeUntracked":true}"#)
        .expect("stashChanges")
    {
        AiOpIntent::StashChanges {
            message,
            include_untracked,
        } => {
            assert_eq!(message, None);
            assert!(include_untracked);
        }
        other => panic!("got {other:?}"),
    }
    match p(r#"{"intent":"discardChanges","paths":["a.txt","b.txt"]}"#)
        .expect("discardChanges")
    {
        AiOpIntent::DiscardChanges { paths } => assert_eq!(paths, vec!["a.txt", "b.txt"]),
        other => panic!("got {other:?}"),
    }
    match p(r#"{"intent":"mergeBranch","branch":"topic"}"#).expect("mergeBranch") {
        AiOpIntent::MergeBranch { branch } => assert_eq!(branch, "topic"),
        other => panic!("got {other:?}"),
    }
    match p(r#"{"intent":"unsupported","reason":"nope"}"#).expect("unsupported") {
        AiOpIntent::Unsupported { reason } => assert_eq!(reason, "nope"),
        other => panic!("got {other:?}"),
    }

    // Unknown tag ⇒ Err (the fail-closed call site maps it to Unsupported).
    assert!(p(r#"{"intent":"rmRf"}"#).is_err(), "unknown tag must NOT parse");
}
