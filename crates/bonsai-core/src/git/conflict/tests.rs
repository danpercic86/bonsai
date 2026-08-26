use super::*;

// ---------------------------------------------- wire shape (TS mirrors)

/// The serde casing must match the TS ConflictEntry / ConflictFile /
/// ConflictKind / ConflictResolution types exactly.
#[test]
fn wire_shapes_are_camel_case() {
    let v = serde_json::to_value(ConflictEntry {
        path: "src/auth.ts".to_string(),
        kind: ConflictKind::BothModified,
        has_base: true,
        has_ours: true,
        has_theirs: true,
    })
    .expect("json");
    assert_eq!(
        v,
        serde_json::json!({
            "path": "src/auth.ts",
            "kind": "bothModified",
            "hasBase": true,
            "hasOurs": true,
            "hasTheirs": true
        })
    );

    let v = serde_json::to_value(ConflictFile {
        path: "README.md".to_string(),
        kind: ConflictKind::DeletedByThem,
        binary: false,
        too_large: false,
        missing: true,
        text: String::new(),
        ours: String::new(),
        theirs: String::new(),
    })
    .expect("json");
    assert_eq!(
        v,
        serde_json::json!({
            "path": "README.md",
            "kind": "deletedByThem",
            "binary": false,
            "tooLarge": false,
            "missing": true,
            "text": "",
            "ours": "",
            "theirs": ""
        })
    );

    // A text-mergeable (bothModified) sample carries non-empty ours/theirs.
    let v = serde_json::to_value(ConflictFile {
        path: "src/auth.ts".to_string(),
        kind: ConflictKind::BothModified,
        binary: false,
        too_large: false,
        missing: false,
        text: "<<<<<<< HEAD\nours\n=======\ntheirs\n>>>>>>> feature\n".to_string(),
        ours: "ours\n".to_string(),
        theirs: "theirs\n".to_string(),
    })
    .expect("json");
    assert_eq!(
        v,
        serde_json::json!({
            "path": "src/auth.ts",
            "kind": "bothModified",
            "binary": false,
            "tooLarge": false,
            "missing": false,
            "text": "<<<<<<< HEAD\nours\n=======\ntheirs\n>>>>>>> feature\n",
            "ours": "ours\n",
            "theirs": "theirs\n"
        })
    );

    for (kind, name) in [
        (ConflictKind::BothModified, "bothModified"),
        (ConflictKind::BothAdded, "bothAdded"),
        (ConflictKind::DeletedByUs, "deletedByUs"),
        (ConflictKind::DeletedByThem, "deletedByThem"),
        (ConflictKind::AddedByUs, "addedByUs"),
        (ConflictKind::AddedByThem, "addedByThem"),
        (ConflictKind::BothDeleted, "bothDeleted"),
    ] {
        let v = serde_json::to_value(kind).expect("json");
        assert_eq!(v, serde_json::json!(name));
    }

    for (json, expect) in [
        ("\"ours\"", ConflictResolution::Ours),
        ("\"theirs\"", ConflictResolution::Theirs),
        ("\"markResolved\"", ConflictResolution::MarkResolved),
    ] {
        let r: ConflictResolution = serde_json::from_str(json).expect("deserialize");
        assert_eq!(r, expect);
    }
}

// ------------------------------------------- §3.1 kind derivation table

#[test]
fn kind_derivation_truth_table() {
    assert_eq!(derive_kind(true, true, true), ConflictKind::BothModified);
    assert_eq!(derive_kind(false, true, true), ConflictKind::BothAdded);
    assert_eq!(derive_kind(true, false, true), ConflictKind::DeletedByUs);
    assert_eq!(derive_kind(true, true, false), ConflictKind::DeletedByThem);
    assert_eq!(derive_kind(false, true, false), ConflictKind::AddedByUs);
    assert_eq!(derive_kind(false, false, true), ConflictKind::AddedByThem);
    assert_eq!(derive_kind(true, false, false), ConflictKind::BothDeleted);
}

/// Clean repo: empty conflict list; get/resolve of any path -> "has no
/// conflict"; escape paths -> invalid path.
#[test]
fn clean_repo_has_no_conflicts() {
    let dir = crate::testutil::scratch_dir();
    git2::Repository::init(dir.path()).expect("init repo");

    assert!(list_conflicts(dir.path()).expect("list").is_empty());

    let err = get_conflict(dir.path(), "a.txt").expect_err("no conflict");
    match err {
        AppError::Git(m) => assert!(m.contains("has no conflict"), "got: {m}"),
        other => panic!("expected Git, got {other:?}"),
    }

    let err =
        resolve_conflict(dir.path(), "a.txt", ConflictResolution::Ours).expect_err("none");
    assert!(matches!(err, AppError::Git(_)));

    let err = resolve_conflict(dir.path(), "../escape", ConflictResolution::Ours)
        .expect_err("escape path");
    match err {
        AppError::InvalidName(m) => assert!(m.contains("invalid path"), "got: {m}"),
        other => panic!("expected InvalidName, got {other:?}"),
    }
}

// ------------------------------------- git2 bothModified fixture builders

/// Commits everything in the worktree (`add_all("*")`) on `HEAD` with the
/// given parents; returns the new commit oid.
fn commit_all(
    repo: &git2::Repository,
    msg: &str,
    parents: &[&git2::Commit],
) -> git2::Oid {
    let mut index = repo.index().expect("index");
    index
        .add_all(["*"].iter(), git2::IndexAddOption::DEFAULT, None)
        .expect("add_all");
    index.write().expect("write index");
    let tree_id = index.write_tree().expect("write tree");
    let tree = repo.find_tree(tree_id).expect("find tree");
    let sig = git2::Signature::now("Test", "test@example.com").expect("sig");
    repo.commit(Some("HEAD"), &sig, &sig, msg, &tree, parents)
        .expect("commit")
}

/// Builds a scratch repo with an in-progress `bothModified` merge conflict on
/// `a.txt` (ours = "main" line, theirs = "topic" line) plus a non-conflicted
/// tracked `keep.txt`. Returns the scratch dir (HEAD = default branch, mid-merge).
fn both_modified_conflict() -> tempfile::TempDir {
    let dir = crate::testutil::scratch_dir();
    let repo = git2::Repository::init(dir.path()).expect("init");

    // base commit on the default branch
    std::fs::write(dir.path().join("a.txt"), "line1\nbase\nline3\n").expect("write a");
    std::fs::write(dir.path().join("keep.txt"), "keep\n").expect("write keep");
    let base = commit_all(&repo, "base", &[]);
    let base_commit = repo.find_commit(base).expect("base commit");
    let default_branch = repo
        .head()
        .expect("head")
        .shorthand()
        .expect("branch name")
        .to_string();

    // topic branch (theirs): change the middle line
    repo.branch("topic", &base_commit, false).expect("branch topic");
    repo.set_head("refs/heads/topic").expect("set head topic");
    std::fs::write(dir.path().join("a.txt"), "line1\ntopic\nline3\n").expect("write topic");
    commit_all(&repo, "topic change", &[&base_commit]);

    // back to the default branch (ours): change the same middle line differently
    repo.set_head(&format!("refs/heads/{default_branch}"))
        .expect("set head default");
    std::fs::write(dir.path().join("a.txt"), "line1\nmain\nline3\n").expect("write main");
    commit_all(&repo, "main change", &[&base_commit]);

    // merge topic -> produces an index conflict + worktree markers
    let outcome = crate::git::merge::merge_branch(dir.path(), "topic", false).expect("merge");
    assert!(
        matches!(outcome, crate::git::merge::MergeOutcome::Conflicts { .. }),
        "expected conflicts, got {outcome:?}"
    );
    dir
}

/// Reads the stage-`n` blob content for `path` from the repo's index.
fn stage_blob(dir: &Path, path: &str, stage: i32) -> Vec<u8> {
    let repo = git2::Repository::open(dir).expect("open");
    let index = repo.index().expect("index");
    let entry = index
        .get_path(Path::new(path), stage)
        .unwrap_or_else(|| panic!("no stage {stage} entry for {path}"));
    let blob = repo.find_blob(entry.id).expect("blob");
    blob.content().to_vec()
}

#[test]
fn ours_theirs_equal_stage_blobs_and_text_keeps_markers() {
    let dir = both_modified_conflict();
    let view = get_conflict(dir.path(), "a.txt").expect("get_conflict");

    assert_eq!(view.kind, ConflictKind::BothModified);
    assert!(!view.binary && !view.too_large && !view.missing);

    let stage2 = String::from_utf8_lossy(&stage_blob(dir.path(), "a.txt", 2)).into_owned();
    let stage3 = String::from_utf8_lossy(&stage_blob(dir.path(), "a.txt", 3)).into_owned();
    assert_eq!(view.ours, stage2, "ours must equal the stage-2 blob");
    assert_eq!(view.theirs, stage3, "theirs must equal the stage-3 blob");
    assert!(!view.ours.is_empty() && !view.theirs.is_empty());

    assert!(
        view.text.contains("<<<<<<<")
            && view.text.contains("=======")
            && view.text.contains(">>>>>>>"),
        "text must still carry the conflict markers"
    );
}

#[test]
fn too_large_suppresses_all_three_strings() {
    let dir = both_modified_conflict();
    std::fs::write(
        dir.path().join("a.txt"),
        vec![b'a'; MAX_CONFLICT_BYTES as usize + 1],
    )
    .expect("write huge");
    let view = get_conflict(dir.path(), "a.txt").expect("get_conflict");
    assert!(view.too_large && !view.binary && !view.missing);
    assert_eq!(view.text, "");
    assert_eq!(view.ours, "");
    assert_eq!(view.theirs, "");
}

#[test]
fn binary_suppresses_all_three_strings() {
    let dir = both_modified_conflict();
    std::fs::write(dir.path().join("a.txt"), b"\x00\x01binary blob").expect("write binary");
    let view = get_conflict(dir.path(), "a.txt").expect("get_conflict");
    assert!(view.binary && !view.too_large && !view.missing);
    assert_eq!(view.text, "");
    assert_eq!(view.ours, "");
    assert_eq!(view.theirs, "");
}

#[test]
fn resolve_conflict_text_round_trip() {
    let dir = both_modified_conflict();
    let merged = "line1\nmerged by hand\nline3\n";
    resolve_conflict_text(dir.path(), "a.txt", merged).expect("resolve text");

    // no longer conflicted
    assert!(
        !list_conflicts(dir.path())
            .expect("list")
            .iter()
            .any(|e| e.path == "a.txt"),
        "a.txt still conflicted after resolve_conflict_text"
    );

    // index has a stage-0 entry and no conflict stages for a.txt
    let repo = git2::Repository::open(dir.path()).expect("open");
    let index = repo.index().expect("index");
    assert!(
        index.get_path(Path::new("a.txt"), 0).is_some(),
        "expected a stage-0 index entry for a.txt"
    );
    for stage in [1, 2, 3] {
        assert!(
            index.get_path(Path::new("a.txt"), stage).is_none(),
            "unexpected stage-{stage} entry after resolve"
        );
    }

    // worktree bytes equal the resolved content verbatim
    let bytes = std::fs::read(dir.path().join("a.txt")).expect("read a");
    assert_eq!(bytes, merged.as_bytes());
}

#[test]
fn resolve_conflict_text_non_conflicted_path_errors() {
    let dir = both_modified_conflict();
    let err = resolve_conflict_text(dir.path(), "keep.txt", "x").expect_err("no conflict");
    match err {
        AppError::Git(m) => assert!(m.contains("has no conflict"), "got: {m}"),
        other => panic!("expected Git, got {other:?}"),
    }
}

#[test]
fn resolve_conflict_text_escape_path_errors() {
    let dir = both_modified_conflict();
    for bad in ["../escape", "..\\escape", "C:\\Windows\\evil"] {
        let err = resolve_conflict_text(dir.path(), bad, "x").expect_err("escape");
        assert!(
            matches!(err, AppError::InvalidName(_)),
            "path {bad:?}: expected InvalidName, got {err:?}"
        );
    }
}

#[test]
fn resolve_conflict_text_accepts_leftover_markers() {
    let dir = both_modified_conflict();
    // Trust model: leftover <<<<<<< markers are NOT rejected (same as git add).
    let content = "<<<<<<< HEAD\nline1\nmain\n=======\ntopic\n>>>>>>> topic\n";
    resolve_conflict_text(dir.path(), "a.txt", content).expect("accept markers");

    let repo = git2::Repository::open(dir.path()).expect("open");
    let index = repo.index().expect("index");
    assert!(
        index.get_path(Path::new("a.txt"), 0).is_some(),
        "leftover-marker content must still stage at stage 0"
    );
    assert!(index.get_path(Path::new("a.txt"), 2).is_none());
    let bytes = std::fs::read(dir.path().join("a.txt")).expect("read a");
    assert_eq!(bytes, content.as_bytes());
}
