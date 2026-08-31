use super::*;

// ---------------------------------------- §2.1 wire shape (TS mirrors)

/// The serde tag/casing must match the TS types exactly:
/// `{ "kind": "upToDate" } | { "kind": "pushed", ..., "setUpstream": ... }`.
#[test]
fn wire_shapes_are_camel_case_tagged() {
    let v = serde_json::to_value(PullResult::UpToDate).expect("json");
    assert_eq!(v, serde_json::json!({ "kind": "upToDate" }));

    let v = serde_json::to_value(PullResult::WouldNotFastForward {
        branch: "main".to_string(),
        ahead: 2,
        behind: 1,
        upstream: "origin/main".to_string(),
    })
    .expect("json");
    assert_eq!(
        v,
        serde_json::json!({ "kind": "wouldNotFastForward", "branch": "main", "ahead": 2, "behind": 1, "upstream": "origin/main" })
    );

    let v = serde_json::to_value(PushResult::Pushed {
        remote: "origin".to_string(),
        branch: "topic".to_string(),
        set_upstream: true,
    })
    .expect("json");
    assert_eq!(
        v,
        serde_json::json!({ "kind": "pushed", "remote": "origin", "branch": "topic", "setUpstream": true })
    );

    let v = serde_json::to_value(FetchResult {
        remotes: vec![RemoteFetchResult {
            remote: "origin".to_string(),
            received_objects: 12,
            updated_refs: 1,
        }],
        tag_auto_sync: None,
    })
    .expect("json");
    assert_eq!(
        v,
        serde_json::json!({ "remotes": [{ "remote": "origin", "receivedObjects": 12, "updatedRefs": 1 }] })
    );
}

// -------------------------------------- P37 §3.2 lease message helpers

/// The lease messages interpolate the remote and branch names.
#[test]
fn lease_messages_interpolate_remote_and_branch() {
    let moved = lease_moved_msg("origin", "main");
    assert!(moved.contains("'origin/main'"), "{moved}");
    assert!(moved.contains("moved"), "{moved}");
    assert!(moved.contains("Fetch"), "{moved}");

    let no_baseline = lease_no_baseline_msg("upstream", "topic");
    assert!(no_baseline.contains("'upstream/topic'"), "{no_baseline}");
    assert!(no_baseline.contains("Fetch first"), "{no_baseline}");
}

// ------------------------------------- P59b build_force_push_args (pure)

/// The atomic-lease argv: `push`, force-with-lease keyed on
/// `<remote_ref>:<baseline>`, `--force-if-includes`, remote, then a PLAIN
/// (no `+`) refspec — the lease itself supplies the conditional force; a `+`
/// would be an unconditional force that overrides the lease (P59b).
#[test]
fn force_push_args_exact_vec() {
    let args = build_force_push_args(
        "origin",
        "main",
        "refs/heads/main",
        "1111111111111111111111111111111111111111",
    );
    assert_eq!(
        args,
        vec![
            "push".to_string(),
            "--force-with-lease=refs/heads/main:1111111111111111111111111111111111111111"
                .to_string(),
            "--force-if-includes".to_string(),
            "--no-verify".to_string(),
            "--".to_string(),
            "origin".to_string(),
            "refs/heads/main:refs/heads/main".to_string(),
        ]
    );
    // No leading '+': an unconditional force would defeat --force-with-lease.
    assert!(!args[6].starts_with('+'), "refspec must not force unconditionally");
    // --no-verify present so git does not re-run the pre-push hook we ran.
    assert!(args.contains(&"--no-verify".to_string()), "must suppress git's own pre-push");
    // F-A5-d: `--` immediately precedes the positional remote + refspec.
    assert_eq!(args[4], "--", "end-of-options guards the positionals");
}

/// A slashed branch name flows verbatim into both the lease ref and the
/// refspec (guards nested-ref interpolation).
#[test]
fn force_push_args_nested_branch() {
    let args = build_force_push_args(
        "upstream",
        "feature/x",
        "refs/heads/feature/x",
        "2222222222222222222222222222222222222222",
    );
    assert_eq!(
        args[1],
        "--force-with-lease=refs/heads/feature/x:2222222222222222222222222222222222222222"
    );
    assert_eq!(args[4], "--");
    assert_eq!(args[5], "upstream");
    assert_eq!(args[6], "refs/heads/feature/x:refs/heads/feature/x");
}

// ------------------------------------- P59a-2 build_pre_push_stdin (pure)

/// An existing remote ref: the baseline oid appears as the 4th field, and the
/// line is `<local-ref> <local-oid> <remote-ref> <remote-oid>\n`.
#[test]
fn pre_push_stdin_existing_ref() {
    let local = git2::Oid::from_str("1111111111111111111111111111111111111111").expect("oid");
    let remote = git2::Oid::from_str("2222222222222222222222222222222222222222").expect("oid");
    let line = build_pre_push_stdin("refs/heads/main", local, "refs/heads/main", Some(remote));
    assert_eq!(
        line,
        "refs/heads/main 1111111111111111111111111111111111111111 \
         refs/heads/main 2222222222222222222222222222222222222222\n"
    );
}

/// A NEW remote ref (no baseline): the remote-oid field is 40 zeros, exactly
/// as git synthesizes it for a create.
#[test]
fn pre_push_stdin_new_ref_is_zeros() {
    let local = git2::Oid::from_str("3333333333333333333333333333333333333333").expect("oid");
    let line = build_pre_push_stdin("refs/heads/feature/x", local, "refs/heads/feature/x", None);
    assert_eq!(
        line,
        "refs/heads/feature/x 3333333333333333333333333333333333333333 \
         refs/heads/feature/x 0000000000000000000000000000000000000000\n"
    );
    // Trailing LF so `read` in a `while read` hook loop terminates the line.
    assert!(line.ends_with('\n'));
}

// ------------------------------------- P59b classify_push_stderr (pure)

/// git's atomic --force-with-lease / --force-if-includes refusal maps to
/// `PushRejected` (the caller then prepends the contextual lease_moved_msg).
#[test]
fn classify_lease_refusal_is_push_rejected() {
    for s in [
        " ! [rejected]        main -> main (stale info)",
        "error: remote ref updated since checkout",
        "! refusing to lose commits: force-with-lease",
    ] {
        assert!(
            matches!(classify_push_stderr(s), AppError::PushRejected(_)),
            "stderr should map to PushRejected: {s:?}"
        );
    }
}

/// Never-prompt credential failures map to `AuthFailed`.
#[test]
fn classify_auth_failure_is_auth_failed() {
    for s in [
        "fatal: Authentication failed for 'https://example.com/x.git/'",
        "fatal: could not read Username for 'https://example.com': terminal prompts disabled",
    ] {
        assert!(
            matches!(classify_push_stderr(s), AppError::AuthFailed(_)),
            "stderr should map to AuthFailed: {s:?}"
        );
    }
}

/// Connect / DNS / TLS failures map to `NetworkError`.
#[test]
fn classify_network_failure_is_network_error() {
    for s in [
        "fatal: unable to access 'https://x/': Could not resolve host: x",
        "fatal: unable to access 'https://x/': SSL certificate problem",
    ] {
        assert!(
            matches!(classify_push_stderr(s), AppError::NetworkError(_)),
            "stderr should map to NetworkError: {s:?}"
        );
    }
}

/// Anything unrecognized falls through to a generic `Git` error carrying the
/// stderr tail.
#[test]
fn classify_unknown_is_git() {
    match classify_push_stderr("fatal: something entirely unexpected happened") {
        AppError::Git(m) => assert!(m.contains("unexpected"), "{m}"),
        other => panic!("expected Git, got {other:?}"),
    }
}

/// The stderr tail keeps only the last few non-empty lines and never panics
/// on empty input.
#[test]
fn push_stderr_tail_is_compact() {
    assert_eq!(push_stderr_tail("   \n  \n"), "git push failed");
    let many = (0..20).map(|i| format!("line{i}")).collect::<Vec<_>>().join("\n");
    let tail = push_stderr_tail(&many);
    assert_eq!(tail.lines().count(), 6);
    assert!(tail.contains("line19"));
    assert!(!tail.contains("line13"));
}

// ------------------------------------------------ §8.3 RemoteInfo shape

/// `RemoteInfo` serializes camelCase with `url: null` when absent.
#[test]
fn remote_info_wire_shape() {
    let v = serde_json::to_value(RemoteInfo {
        name: "origin".to_string(),
        url: Some("https://example.com/repo.git".to_string()),
    })
    .expect("json");
    assert_eq!(
        v,
        serde_json::json!({ "name": "origin", "url": "https://example.com/repo.git" })
    );

    let v = serde_json::to_value(RemoteInfo {
        name: "origin".to_string(),
        url: None,
    })
    .expect("json");
    assert_eq!(v, serde_json::json!({ "name": "origin", "url": null }));
}
/// `list_remotes` sort: case-insensitive primary, exact tie-break.
#[test]
fn remote_info_sort_order() {
    let mut v = [
        RemoteInfo { name: "Zeta".to_string(), url: None },
        RemoteInfo { name: "alpha".to_string(), url: None },
        RemoteInfo { name: "Beta".to_string(), url: None },
        RemoteInfo { name: "beta".to_string(), url: None },
    ];
    v.sort_by(|a, b| {
        a.name
            .to_lowercase()
            .cmp(&b.name.to_lowercase())
            .then_with(|| a.name.cmp(&b.name))
    });
    let names: Vec<&str> = v.iter().map(|r| r.name.as_str()).collect();
    // case-insensitive order: alpha, Beta/beta (tie → 'B' < 'b'), Zeta.
    assert_eq!(names, vec!["alpha", "Beta", "beta", "Zeta"]);
}
