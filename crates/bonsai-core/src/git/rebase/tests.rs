use super::*;

// ---------------------------------------------- wire shape (TS mirrors)

/// The serde tag/casing must match the TS RebaseOutcome union exactly.
#[test]
fn wire_shapes_are_camel_case_tagged() {
    let v = serde_json::to_value(RebaseOutcome::UpToDate).expect("json");
    assert_eq!(v, serde_json::json!({ "kind": "upToDate" }));

    let v = serde_json::to_value(RebaseOutcome::FastForwarded {
        branch: "topic".to_string(),
        to: "a".repeat(40),
    })
    .expect("json");
    assert_eq!(
        v,
        serde_json::json!({ "kind": "fastForwarded", "branch": "topic", "to": "a".repeat(40) })
    );

    let v = serde_json::to_value(RebaseOutcome::Rebased {
        branch: "topic".to_string(),
        head: "b".repeat(40),
        steps: 2,
        warnings: Vec::new(),
    })
    .expect("json");
    assert_eq!(
        v,
        serde_json::json!({ "kind": "rebased", "branch": "topic", "head": "b".repeat(40), "steps": 2 }),
        "empty warnings are omitted from the wire (skip_serializing_if)"
    );

    // A non-empty warnings list surfaces as a `warnings` array (toasted by the UI).
    let v = serde_json::to_value(RebaseOutcome::Rebased {
        branch: "topic".to_string(),
        head: "b".repeat(40),
        steps: 2,
        warnings: vec!["reword of 1234567 was dropped".to_string()],
    })
    .expect("json");
    assert_eq!(
        v,
        serde_json::json!({
            "kind": "rebased",
            "branch": "topic",
            "head": "b".repeat(40),
            "steps": 2,
            "warnings": ["reword of 1234567 was dropped"]
        })
    );

    let v = serde_json::to_value(RebaseOutcome::Conflicts {
        paths: vec!["README.md".to_string(), "src/auth.ts".to_string()],
        current_step: 2,
        total_steps: 3,
    })
    .expect("json");
    assert_eq!(
        v,
        serde_json::json!({
            "kind": "conflicts",
            "paths": ["README.md", "src/auth.ts"],
            "currentStep": 2,
            "totalSteps": 3
        })
    );
}

// ------------------------------------------------------- preconditions

#[test]
fn rebase_preconditions_on_fresh_repo() {
    let dir = crate::testutil::scratch_dir();
    git2::Repository::init(dir.path()).expect("init repo");

    // Unborn HEAD refuses before onto resolution.
    let err = rebase_branch(dir.path(), "main").expect_err("unborn");
    match err {
        AppError::Git(m) => assert!(m.contains("no commits yet"), "got: {m}"),
        other => panic!("expected Git, got {other:?}"),
    }

    // continue / skip / abort with no rebase in progress.
    let err = rebase_continue(dir.path()).expect_err("no rebase");
    assert!(matches!(err, AppError::NoOperationInProgress(_)));
    let err = rebase_skip(dir.path()).expect_err("no rebase");
    assert!(matches!(err, AppError::NoOperationInProgress(_)));
    let err = rebase_abort(dir.path()).expect_err("no rebase");
    assert!(matches!(err, AppError::NoOperationInProgress(_)));
}
