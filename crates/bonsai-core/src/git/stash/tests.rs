use super::*;

/// Wire shapes (P9 §8 test 8): serde tag/casing must match the TS mirrors
/// (ApplyStashOutcome union + StashEntry camelCase).
#[test]
fn wire_shapes_are_camel_case_tagged() {
    let v = serde_json::to_value(ApplyStashOutcome::Applied).expect("json");
    assert_eq!(v, serde_json::json!({ "kind": "applied" }));

    let v = serde_json::to_value(ApplyStashOutcome::Conflicts {
        paths: vec!["src/app.ts".to_string(), "README.md".to_string()],
    })
    .expect("json");
    assert_eq!(
        v,
        serde_json::json!({ "kind": "conflicts", "paths": ["src/app.ts", "README.md"] })
    );

    let v = serde_json::to_value(StashEntry {
        index: 0,
        message: "WIP on main: 1a2b3c4 summary".to_string(),
        oid: "a".repeat(40),
        base_oid: "b".repeat(40),
        ts: 1_700_000_000,
    })
    .expect("json");
    assert_eq!(
        v,
        serde_json::json!({
            "index": 0,
            "message": "WIP on main: 1a2b3c4 summary",
            "oid": "a".repeat(40),
            "baseOid": "b".repeat(40),
            "ts": 1_700_000_000
        })
    );

    let v = serde_json::to_value(ApplyStashOutcome::ReservedPaths {
        paths: vec!["src/Aspire.AppHost/NUL".to_string()],
    })
    .expect("json");
    assert_eq!(
        v,
        serde_json::json!({ "kind": "reservedPaths", "paths": ["src/Aspire.AppHost/NUL"] })
    );

    let v = serde_json::to_value(ApplyStashOutcome::AppliedSkippingReserved {
        skipped: vec!["src/Aspire.AppHost/NUL".to_string()],
    })
    .expect("json");
    assert_eq!(
        v,
        serde_json::json!({ "kind": "appliedSkippingReserved", "skipped": ["src/Aspire.AppHost/NUL"] })
    );

    let v = serde_json::to_value(CreateStashResult { created: true }).expect("json");
    assert_eq!(v, serde_json::json!({ "created": true }));
}

/// `is_windows_reserved` truth table: the reserved device names (any case,
/// with a trailing dot/space or an extension) match; near-misses do not.
#[test]
fn is_windows_reserved_truth_table() {
    for yes in [
        "NUL", "nul", "Nul", "NUL.txt", "NUL.", "NUL ", "CON", "PRN", "AUX", "COM1", "com9",
        "LPT1", "LPT9",
    ] {
        assert!(is_windows_reserved(yes), "{yes:?} must be reserved");
    }
    for no in [
        "NULl", "NULL2", "README", "COM", "COM0", "COM10", "LPT0", "LPT10", "NULfile",
        "myNUL", "",
    ] {
        assert!(!is_windows_reserved(no), "{no:?} must NOT be reserved");
    }
}

// ============================================================ P9 §8 matrix
// Behavioral stash matrix (one test per §8 row, 1..=7). Each asserts BOTH the
// returned outcome AND the on-disk state via a scratch repo, echoing the
// merge.rs (P8) fixtures. Fixtures are built with git2 — deterministic, no
// network, no CLI.

/// Init a scratch repo with a deterministic identity + autocrlf off (== p8_init).
fn s9_init(dir: &Path) -> git2::Repository {
    let repo = git2::Repository::init(dir).expect("init repo");
    let mut cfg = repo.config().expect("config");
    cfg.set_str("user.name", "Test User").expect("name");
    cfg.set_str("user.email", "test@example.com").expect("email");
    cfg.set_bool("core.autocrlf", false).expect("autocrlf");
    drop(cfg);
    repo
}

/// Stage + commit `files` on the CURRENT branch (moves HEAD + worktree).
fn s9_commit(dir: &Path, msg: &str, files: &[(&str, &str)]) {
    use crate::git::stage::stage_paths;
    for (name, content) in files {
        std::fs::write(dir.join(name), content).expect("write file");
    }
    stage_paths(
        dir,
        &files.iter().map(|(n, _)| n.to_string()).collect::<Vec<_>>(),
    )
    .expect("stage");
    crate::git::commit::create_commit(dir, msg, None, false).expect("commit");
}

/// Build a commit on `refname` from `parent`'s tree WITHOUT moving HEAD or the
/// worktree (== p8_commit_on_ref). Used to build a divergent topic tip.
fn s9_commit_on_ref(
    repo: &git2::Repository,
    refname: &str,
    parent: &git2::Commit,
    files: &[(&str, &str)],
    msg: &str,
) -> git2::Oid {
    let sig = git2::Signature::now("Test User", "test@example.com").expect("sig");
    let mut tb = repo
        .treebuilder(Some(&parent.tree().expect("parent tree")))
        .expect("treebuilder");
    for (name, content) in files {
        let blob = repo.blob(content.as_bytes()).expect("blob");
        tb.insert(name, blob, 0o100644).expect("insert");
    }
    let tree = repo.find_tree(tb.write().expect("tree oid")).expect("tree");
    repo.commit(Some(refname), &sig, &sig, &format!("{msg}\n"), &tree, &[parent])
        .expect("commit on ref")
}

fn s9_head_oid(dir: &Path) -> String {
    let repo = git2::Repository::open(dir).expect("open");
    let oid = repo
        .head()
        .expect("HEAD")
        .peel_to_commit()
        .expect("peel")
        .id();
    oid.to_string()
}

fn s9_read(dir: &Path, name: &str) -> String {
    std::fs::read_to_string(dir.join(name)).expect("read file")
}

// ---- Row 1: Round-trip (apply does NOT drop) ---------------------------

#[test]
fn s9_1_round_trip_apply_keeps_stash() {
    let dir = crate::testutil::scratch_dir();
    let d = dir.path();
    s9_init(d);
    s9_commit(d, "base", &[("a.txt", "base\n")]);
    let head = s9_head_oid(d);

    // Dirty: edit a tracked file (unstaged).
    std::fs::write(d.join("a.txt"), "edited\n").expect("edit");

    let res = create_stash(d, None, StashScope::All).expect("create_stash");
    assert!(res.created, "dirty tracked edit must stash");

    // Worktree returned to the committed state (stash reset both index+worktree).
    assert_eq!(s9_read(d, "a.txt"), "base\n", "worktree must be clean after stash");

    let list = list_stashes(d).expect("list");
    assert_eq!(list.len(), 1, "one entry on the stack");
    assert_eq!(list[0].index, 0);
    assert_eq!(list[0].base_oid, head, "base_oid must == HEAD at stash time");
    assert!(!list[0].message.is_empty(), "default message must be non-empty");

    let outcome = apply_stash(d, 0, false, None).expect("apply");
    assert_eq!(outcome, ApplyStashOutcome::Applied, "clean apply");
    assert_eq!(s9_read(d, "a.txt"), "edited\n", "edit restored to worktree");

    let list = list_stashes(d).expect("list after apply");
    assert_eq!(list.len(), 1, "apply must NOT drop the stash");
}

// ---- Row 2: Pop drops --------------------------------------------------

#[test]
fn s9_2_pop_drops_stash() {
    let dir = crate::testutil::scratch_dir();
    let d = dir.path();
    s9_init(d);
    s9_commit(d, "base", &[("a.txt", "base\n")]);

    std::fs::write(d.join("a.txt"), "edited\n").expect("edit");
    let res = create_stash(d, None, StashScope::All).expect("create_stash");
    assert!(res.created);
    assert_eq!(s9_read(d, "a.txt"), "base\n");

    let outcome = pop_stash(d, 0, false, None).expect("pop");
    assert_eq!(outcome, ApplyStashOutcome::Applied, "clean pop");
    assert_eq!(s9_read(d, "a.txt"), "edited\n", "edit restored to worktree");

    let list = list_stashes(d).expect("list after pop");
    assert_eq!(list.len(), 0, "pop must drop the stash on clean apply");
}

// ---- Row 3: Nothing to stash -------------------------------------------

#[test]
fn s9_3_nothing_to_stash() {
    let dir = crate::testutil::scratch_dir();
    let d = dir.path();
    s9_init(d);
    s9_commit(d, "base", &[("a.txt", "base\n")]);

    // Clean tree.
    let res = create_stash(d, None, StashScope::All).expect("create_stash");
    assert!(!res.created, "clean tree -> created:false, NOT an error");

    let list = list_stashes(d).expect("list");
    assert_eq!(list.len(), 0, "nothing pushed onto the stack");
}

// ---- Row 4: Include untracked ------------------------------------------

#[test]
fn s9_4_include_untracked_round_trip() {
    let dir = crate::testutil::scratch_dir();
    let d = dir.path();
    s9_init(d);
    s9_commit(d, "base", &[("a.txt", "base\n")]);

    // An untracked file present.
    std::fs::write(d.join("untracked.txt"), "new\n").expect("write untracked");

    let res = create_stash(d, None, StashScope::AllWithUntracked)
        .expect("create_stash include_untracked");
    assert!(res.created, "untracked file must stash under include_untracked");
    assert!(
        !d.join("untracked.txt").exists(),
        "include_untracked must remove the untracked file from the worktree"
    );

    let list = list_stashes(d).expect("list");
    assert_eq!(list.len(), 1);

    let outcome = pop_stash(d, 0, false, None).expect("pop");
    assert_eq!(outcome, ApplyStashOutcome::Applied);
    assert!(
        d.join("untracked.txt").exists(),
        "pop must restore the stashed untracked file"
    );
    assert_eq!(s9_read(d, "untracked.txt"), "new\n", "untracked content restored");
    assert_eq!(list_stashes(d).expect("list").len(), 0, "clean pop drops entry");
}

// ---- Row 5: Pop conflict retains (P8-lesson data safety) + apply variant

/// Build a repo where popping/applying stash@{0} necessarily conflicts on X:
/// base has X="base"; stash records X="stashed"; then HEAD advances X="head".
/// Returns the scratch dir (kept alive by the caller).
fn s9_conflict_fixture() -> tempfile::TempDir {
    let dir = crate::testutil::scratch_dir();
    let d = dir.path();
    s9_init(d);
    s9_commit(d, "base", &[("x.txt", "base\n")]);

    // Stash an unstaged edit to X.
    std::fs::write(d.join("x.txt"), "stashed\n").expect("edit x");
    let res = create_stash(d, None, StashScope::All).expect("create_stash");
    assert!(res.created);
    assert_eq!(s9_read(d, "x.txt"), "base\n", "worktree reset after stash");

    // Advance HEAD with a DIFFERENT change to the same file -> guaranteed
    // 3-way conflict on re-apply (base=base, ours=head, theirs=stashed).
    s9_commit(d, "head change", &[("x.txt", "head\n")]);
    dir
}

#[test]
fn s9_5_pop_conflict_retains_stash() {
    let dir = s9_conflict_fixture();
    let d = dir.path();

    let outcome = pop_stash(d, 0, false, None).expect("pop");
    assert_eq!(
        outcome,
        ApplyStashOutcome::Conflicts {
            paths: vec!["x.txt".to_string()]
        },
        "conflicting pop must report Conflicts on x.txt"
    );

    // No merge started; only the index carries conflict entries.
    let repo = git2::Repository::open(d).expect("reopen");
    assert_eq!(
        repo.state(),
        git2::RepositoryState::Clean,
        "stash apply must not enter a Merge state"
    );

    assert!(
        s9_read(d, "x.txt").contains("<<<<<<<"),
        "worktree X must carry conflict markers"
    );

    let list = list_stashes(d).expect("list after conflicting pop");
    assert_eq!(
        list.len(),
        1,
        "DATA SAFETY: a conflicting pop must RETAIN the stash (never lossy)"
    );
}

#[test]
fn s9_5b_apply_conflict_retains_stash() {
    let dir = s9_conflict_fixture();
    let d = dir.path();

    let outcome = apply_stash(d, 0, false, None).expect("apply");
    assert_eq!(
        outcome,
        ApplyStashOutcome::Conflicts {
            paths: vec!["x.txt".to_string()]
        },
        "conflicting apply must report Conflicts on x.txt"
    );

    let repo = git2::Repository::open(d).expect("reopen");
    assert_eq!(repo.state(), git2::RepositoryState::Clean);
    assert!(s9_read(d, "x.txt").contains("<<<<<<<"));

    let list = list_stashes(d).expect("list after conflicting apply");
    assert_eq!(list.len(), 1, "apply never drops; stash retained on conflict");
}

// ---- Row 6: Drop re-indexes the stack ----------------------------------

#[test]
fn s9_6_drop_reindexes_stack() {
    let dir = crate::testutil::scratch_dir();
    let d = dir.path();
    s9_init(d);
    s9_commit(d, "base", &[("a.txt", "base\n")]);

    // Stash A (becomes stash@{1} after the second push).
    std::fs::write(d.join("a.txt"), "edit-A\n").expect("edit A");
    create_stash(d, Some("stash-A"), StashScope::All).expect("stash A");
    // Stash B (most recent, stash@{0}).
    std::fs::write(d.join("a.txt"), "edit-B\n").expect("edit B");
    create_stash(d, Some("stash-B"), StashScope::All).expect("stash B");

    let before = list_stashes(d).expect("list before drop");
    assert_eq!(before.len(), 2);
    assert!(before[0].message.contains("stash-B"), "stash@{{0}} is the newest");
    assert!(before[1].message.contains("stash-A"), "stash@{{1}} is the oldest");
    let survivor_oid = before[1].oid.clone();

    // Drop the most recent (index 0). Entries above shift down by one.
    drop_stash(d, 0, None).expect("drop");

    let after = list_stashes(d).expect("list after drop");
    assert_eq!(after.len(), 1, "one entry survives");
    assert_eq!(after[0].index, 0, "surviving entry re-indexed to 0 (§2.4 shift)");
    assert!(
        after[0].message.contains("stash-A"),
        "the survivor is the entry previously at index 1 (stash-A)"
    );
    assert_eq!(after[0].oid, survivor_oid, "survivor identity confirmed by oid");
}

// ---- Row 7: Op-state guard (Merge in progress) -------------------------

#[test]
fn s9_7_op_state_guard_blocks_all_but_drop() {
    let dir = crate::testutil::scratch_dir();
    let d = dir.path();
    let repo = s9_init(d);

    s9_commit(d, "base", &[("x.txt", "base\n"), ("y.txt", "y-base\n")]);
    let base = repo
        .find_commit(
            repo.head().expect("HEAD").target().expect("oid"),
        )
        .expect("base");
    // topic diverges on x.txt.
    s9_commit_on_ref(&repo, "refs/heads/topic", &base, &[("x.txt", "topic\n")], "topic edits x");
    // main diverges on x.txt (guaranteed conflict).
    s9_commit(d, "main edits x", &[("x.txt", "main\n")]);

    // Dirty unrelated file -> merge autostashes it, then pauses in Merge state.
    // This leaves BOTH a Merge state AND a retained stash on the stack, so we
    // can exercise the guard AND prove drop still works.
    std::fs::write(d.join("y.txt"), "y-edited\n").expect("edit y");

    crate::git::merge::merge_branch(d, "topic", false).expect("merge");
    let repo = git2::Repository::open(d).expect("reopen");
    assert_eq!(
        repo.state(),
        git2::RepositoryState::Merge,
        "conflicting merge over a dirty tree must pause in Merge state"
    );
    assert_eq!(
        list_stashes(d).expect("list").len(),
        1,
        "the retained autostash gives us something to drop"
    );

    // create/apply/pop are all rejected while an operation is in progress.
    match create_stash(d, None, StashScope::All) {
        Err(AppError::OperationInProgress(_)) => {}
        other => panic!("create_stash: expected OperationInProgress, got {other:?}"),
    }
    match apply_stash(d, 0, false, None) {
        Err(AppError::OperationInProgress(_)) => {}
        other => panic!("apply_stash: expected OperationInProgress, got {other:?}"),
    }
    match pop_stash(d, 0, false, None) {
        Err(AppError::OperationInProgress(_)) => {}
        other => panic!("pop_stash: expected OperationInProgress, got {other:?}"),
    }

    // Drop is allowed in ANY repo state (touches only the stash reflog).
    drop_stash(d, 0, None).expect("drop must succeed mid-merge");
    assert_eq!(
        list_stashes(d).expect("list after drop").len(),
        0,
        "drop removed the autostash even though a merge is in progress"
    );
}

// ======================================================= P34 stash scopes
// Behavioral matrix for StashScope (all / allWithUntracked / staged), the new
// data-safety-critical `staged` path (FOLD semantics — orchestrator override:
// mixed staged+unstaged files are folded WHOLE, not rejected), and the
// stacking / rm --cached regression guards. Each test asserts BOTH the
// CreateStashResult/outcome AND the resulting repo state (index, worktree,
// stash stack). Fixtures reuse the s9_* helpers above.

/// Stage `names` (delegates to the real staging path used by the app).
fn p34_stage(dir: &Path, names: &[&str]) {
    use crate::git::stage::stage_paths;
    stage_paths(
        dir,
        &names.iter().map(|n| n.to_string()).collect::<Vec<_>>(),
    )
    .expect("stage");
}

/// `git rm --cached <name>`: stage a deletion in the index while the file
/// STAYS on disk with its HEAD content (index.remove_path, no worktree touch).
fn p34_rm_cached(dir: &Path, name: &str) {
    let repo = git2::Repository::open(dir).expect("open");
    let mut index = repo.index().expect("index");
    index.remove_path(Path::new(name)).expect("remove_path");
    index.write().expect("write index");
}

/// Paths whose INDEX differs from HEAD (== "staged" set). Empty ⇒ index==HEAD.
fn p34_staged_paths(dir: &Path) -> Vec<String> {
    let repo = git2::Repository::open(dir).expect("open");
    let head_tree = repo
        .head()
        .expect("head")
        .peel_to_tree()
        .expect("head tree");
    let diff = repo
        .diff_tree_to_index(Some(&head_tree), None, None)
        .expect("diff tree->index");
    let mut v: Vec<String> = diff
        .deltas()
        .filter_map(|d| {
            d.new_file()
                .path()
                .or_else(|| d.old_file().path())
                .map(|p| p.to_string_lossy().replace('\\', "/"))
        })
        .collect();
    v.sort();
    v.dedup();
    v
}

/// Paths whose WORKTREE differs from the index (tracked "unstaged" set;
/// untracked files are excluded by libgit2's default).
fn p34_unstaged_paths(dir: &Path) -> Vec<String> {
    let repo = git2::Repository::open(dir).expect("open");
    let diff = repo
        .diff_index_to_workdir(None, None)
        .expect("diff index->workdir");
    let mut v: Vec<String> = diff
        .deltas()
        .filter_map(|d| {
            d.new_file()
                .path()
                .or_else(|| d.old_file().path())
                .map(|p| p.to_string_lossy().replace('\\', "/"))
        })
        .collect();
    v.sort();
    v.dedup();
    v
}

fn p34_assert_index_clean(dir: &Path) {
    assert_eq!(
        p34_staged_paths(dir),
        Vec::<String>::new(),
        "index must equal HEAD (nothing staged)"
    );
}

// ---- AC-9: wire mapping of the scope union --------------------------------

#[test]
fn p34_scope_deserializes_from_camel_case() {
    use serde_json::{from_value, json};
    assert_eq!(
        from_value::<StashScope>(json!("all")).expect("all"),
        StashScope::All
    );
    assert_eq!(
        from_value::<StashScope>(json!("allWithUntracked")).expect("allWithUntracked"),
        StashScope::AllWithUntracked
    );
    assert_eq!(
        from_value::<StashScope>(json!("staged")).expect("staged"),
        StashScope::Staged
    );
    assert!(
        from_value::<StashScope>(json!("bogus")).is_err(),
        "unknown scope must not deserialize"
    );
}

// ---- Case 1: `All` == old DEFAULT behavior --------------------------------

#[test]
fn p34_all_stashes_tracked_leaves_untracked() {
    let dir = crate::testutil::scratch_dir();
    let d = dir.path();
    s9_init(d);
    s9_commit(d, "base", &[("a.txt", "base\n"), ("b.txt", "b-base\n")]);

    // staged modify (a), unstaged modify (b), untracked (u).
    std::fs::write(d.join("a.txt"), "a-staged\n").expect("edit a");
    p34_stage(d, &["a.txt"]);
    std::fs::write(d.join("b.txt"), "b-unstaged\n").expect("edit b");
    std::fs::write(d.join("u.txt"), "untracked\n").expect("write u");

    let res = create_stash(d, None, StashScope::All).expect("create_stash all");
    assert!(res.created, "tracked changes must stash");

    // Tracked worktree changes gone; untracked survives.
    assert_eq!(s9_read(d, "a.txt"), "base\n", "staged a reverted");
    assert_eq!(s9_read(d, "b.txt"), "b-base\n", "unstaged b reverted");
    assert!(d.join("u.txt").exists(), "untracked left in place");
    p34_assert_index_clean(d);
    assert_eq!(list_stashes(d).expect("list").len(), 1, "one entry");
}

// ---- Case 2: `AllWithUntracked` also captures untracked -------------------

#[test]
fn p34_all_with_untracked_captures_untracked() {
    let dir = crate::testutil::scratch_dir();
    let d = dir.path();
    s9_init(d);
    s9_commit(d, "base", &[("a.txt", "base\n")]);

    std::fs::write(d.join("a.txt"), "a-staged\n").expect("edit a");
    p34_stage(d, &["a.txt"]);
    std::fs::write(d.join("u.txt"), "untracked\n").expect("write u");

    let res = create_stash(d, None, StashScope::AllWithUntracked).expect("create_stash");
    assert!(res.created);
    assert_eq!(s9_read(d, "a.txt"), "base\n", "tracked reverted");
    assert!(
        !d.join("u.txt").exists(),
        "allWithUntracked must sweep the untracked file"
    );
    p34_assert_index_clean(d);
    assert_eq!(list_stashes(d).expect("list").len(), 1);

    // Round-trip restores the untracked file too.
    let outcome = pop_stash(d, 0, false, None).expect("pop");
    assert_eq!(outcome, ApplyStashOutcome::Applied);
    assert!(d.join("u.txt").exists(), "untracked restored on pop");
    assert_eq!(s9_read(d, "u.txt"), "untracked\n");
    assert_eq!(list_stashes(d).expect("list").len(), 0, "clean pop drops");
}

// ---- Case 3: nothing to stash, per scope ----------------------------------

#[test]
fn p34_nothing_to_stash_each_scope() {
    for scope in [
        StashScope::All,
        StashScope::AllWithUntracked,
        StashScope::Staged,
    ] {
        let dir = crate::testutil::scratch_dir();
        let d = dir.path();
        s9_init(d);
        s9_commit(d, "base", &[("a.txt", "base\n")]);
        let head = s9_head_oid(d);

        let res = create_stash(d, None, scope).unwrap_or_else(|e| panic!("{scope:?}: {e:?}"));
        assert!(!res.created, "{scope:?}: clean tree -> created:false");
        assert_eq!(
            list_stashes(d).expect("list").len(),
            0,
            "{scope:?}: no entry pushed"
        );
        assert_eq!(s9_head_oid(d), head, "{scope:?}: HEAD unchanged");
        assert_eq!(s9_read(d, "a.txt"), "base\n", "{scope:?}: worktree unchanged");
    }
}

// ---- Case 4: pure-staged modify (the core new path) -----------------------

#[test]
fn p34_staged_pure_modify_round_trip() {
    let dir = crate::testutil::scratch_dir();
    let d = dir.path();
    s9_init(d);
    s9_commit(d, "base", &[("a.txt", "base\n")]);

    std::fs::write(d.join("a.txt"), "a-staged\n").expect("edit a");
    p34_stage(d, &["a.txt"]); // staged, no further unstaged edit

    let res = create_stash(d, None, StashScope::Staged).expect("create_stash staged");
    assert!(res.created, "staged change must stash");

    // A reverts to HEAD; index == HEAD; one entry.
    assert_eq!(s9_read(d, "a.txt"), "base\n", "worktree reverts to HEAD");
    p34_assert_index_clean(d);
    assert_eq!(
        p34_unstaged_paths(d),
        Vec::<String>::new(),
        "no residual unstaged change"
    );
    assert_eq!(list_stashes(d).expect("list").len(), 1);

    // Pop restores the staged content as an UNSTAGED edit (F-1: no reinstate).
    let outcome = pop_stash(d, 0, false, None).expect("pop");
    assert_eq!(outcome, ApplyStashOutcome::Applied);
    assert_eq!(s9_read(d, "a.txt"), "a-staged\n", "staged content restored");
    p34_assert_index_clean(d);
    assert_eq!(
        p34_unstaged_paths(d),
        vec!["a.txt".to_string()],
        "restored as UNSTAGED, not re-staged"
    );
    assert_eq!(list_stashes(d).expect("list").len(), 0, "clean pop drops");
}

// ---- Case 5: mixed file FOLD (orchestrator override) ----------------------

#[test]
fn p34_staged_mixed_file_folds_whole() {
    let dir = crate::testutil::scratch_dir();
    let d = dir.path();
    s9_init(d);
    s9_commit(d, "base", &[("b.txt", "base\n")]);

    std::fs::write(d.join("b.txt"), "b-staged\n").expect("stage-edit b");
    p34_stage(d, &["b.txt"]);
    // Further UNSTAGED edit on the SAME path → mixed file.
    std::fs::write(d.join("b.txt"), "b-staged-then-unstaged\n").expect("unstage-edit b");

    let res = create_stash(d, None, StashScope::Staged).expect("create_stash staged");
    assert!(res.created, "mixed file must fold, not reject");

    // FOLD: B reverts to HEAD, index clean; the FULL worktree content is held.
    assert_eq!(s9_read(d, "b.txt"), "base\n", "b reverted to HEAD");
    p34_assert_index_clean(d);
    assert_eq!(list_stashes(d).expect("list").len(), 1);

    let outcome = pop_stash(d, 0, false, None).expect("pop");
    assert_eq!(outcome, ApplyStashOutcome::Applied);
    assert_eq!(
        s9_read(d, "b.txt"),
        "b-staged-then-unstaged\n",
        "stash held the FULL folded worktree content"
    );
}

// ---- Case 6: unstaged-only path is preserved ------------------------------

#[test]
fn p34_staged_preserves_unstaged_only_path() {
    let dir = crate::testutil::scratch_dir();
    let d = dir.path();
    s9_init(d);
    s9_commit(d, "base", &[("a.txt", "base\n"), ("c.txt", "c-base\n")]);

    // a.txt staged; c.txt unstaged-only (index == HEAD).
    std::fs::write(d.join("a.txt"), "a-staged\n").expect("edit a");
    p34_stage(d, &["a.txt"]);
    std::fs::write(d.join("c.txt"), "c-unstaged\n").expect("edit c");

    let res = create_stash(d, None, StashScope::Staged).expect("create_stash staged");
    assert!(res.created);

    assert_eq!(s9_read(d, "a.txt"), "base\n", "staged a reverted");
    assert_eq!(
        s9_read(d, "c.txt"),
        "c-unstaged\n",
        "unstaged-only c must NOT be stashed"
    );
    p34_assert_index_clean(d);
    assert_eq!(
        p34_unstaged_paths(d),
        vec!["c.txt".to_string()],
        "c remains an unstaged change"
    );
    assert_eq!(list_stashes(d).expect("list").len(), 1);
}

// ---- Case 7: untracked file is preserved ----------------------------------

#[test]
fn p34_staged_preserves_untracked() {
    let dir = crate::testutil::scratch_dir();
    let d = dir.path();
    s9_init(d);
    s9_commit(d, "base", &[("a.txt", "base\n")]);

    std::fs::write(d.join("a.txt"), "a-staged\n").expect("edit a");
    p34_stage(d, &["a.txt"]);
    std::fs::write(d.join("u.txt"), "untracked\n").expect("write u");

    let res = create_stash(d, None, StashScope::Staged).expect("create_stash staged");
    assert!(res.created);

    assert!(d.join("u.txt").exists(), "untracked survives a staged stash");
    assert_eq!(s9_read(d, "u.txt"), "untracked\n", "untracked content intact");
    assert_eq!(list_stashes(d).expect("list").len(), 1);
}

// ---- Case 8: staged ADD ---------------------------------------------------

#[test]
fn p34_staged_add_round_trip() {
    let dir = crate::testutil::scratch_dir();
    let d = dir.path();
    s9_init(d);
    s9_commit(d, "base", &[("a.txt", "base\n")]);

    std::fs::write(d.join("new.txt"), "new\n").expect("write new");
    p34_stage(d, &["new.txt"]); // staged add

    let res = create_stash(d, None, StashScope::Staged).expect("create_stash staged");
    assert!(res.created, "staged add must stash");

    assert!(
        !d.join("new.txt").exists(),
        "staged add removed from worktree after stash"
    );
    p34_assert_index_clean(d);
    assert_eq!(list_stashes(d).expect("list").len(), 1);

    let outcome = pop_stash(d, 0, false, None).expect("pop");
    assert_eq!(outcome, ApplyStashOutcome::Applied);
    assert!(d.join("new.txt").exists(), "pop restores the added file");
    assert_eq!(s9_read(d, "new.txt"), "new\n", "added content restored");
}

// ---- Case 9: staged DELETE (real worktree deletion) -----------------------

#[test]
fn p34_staged_delete_round_trip() {
    let dir = crate::testutil::scratch_dir();
    let d = dir.path();
    s9_init(d);
    s9_commit(d, "base", &[("a.txt", "base\n"), ("del.txt", "gone\n")]);

    std::fs::remove_file(d.join("del.txt")).expect("rm del");
    p34_stage(d, &["del.txt"]); // stages the deletion (file absent on disk)

    let res = create_stash(d, None, StashScope::Staged).expect("create_stash staged");
    assert!(res.created, "staged deletion must stash");

    // The staged deletion is reverted → file back on disk == HEAD; index clean.
    assert!(
        d.join("del.txt").exists(),
        "staged deletion reverted: file restored to worktree"
    );
    assert_eq!(s9_read(d, "del.txt"), "gone\n", "restored to HEAD content");
    p34_assert_index_clean(d);
    assert_eq!(list_stashes(d).expect("list").len(), 1);

    let outcome = pop_stash(d, 0, false, None).expect("pop");
    assert_eq!(outcome, ApplyStashOutcome::Applied);
    assert!(
        !d.join("del.txt").exists(),
        "pop reintroduces the staged deletion"
    );
}

// ---- Case 10: `git rm --cached` (staged deletion, file still on disk) ------

#[test]
fn p34_staged_rm_cached_deletion_captured() {
    let dir = crate::testutil::scratch_dir();
    let d = dir.path();
    s9_init(d);
    s9_commit(d, "base", &[("a.txt", "base\n"), ("rm.txt", "content\n")]);

    p34_rm_cached(d, "rm.txt"); // index deletion; file STAYS on disk w/ HEAD content
    assert_eq!(
        p34_staged_paths(d),
        vec!["rm.txt".to_string()],
        "precondition: rm.txt is a staged deletion"
    );

    let res = create_stash(d, None, StashScope::Staged).expect("create_stash staged");
    assert!(
        res.created,
        "the staged deletion must be captured, not silently dropped"
    );

    // After stash: staged deletion reverted; file present == HEAD; index clean.
    assert!(d.join("rm.txt").exists(), "file back on disk after revert");
    assert_eq!(s9_read(d, "rm.txt"), "content\n");
    p34_assert_index_clean(d);
    assert_eq!(list_stashes(d).expect("list").len(), 1);

    // Pop reintroduces the deletion (proves the deletion was in the entry).
    let outcome = pop_stash(d, 0, false, None).expect("pop");
    assert_eq!(outcome, ApplyStashOutcome::Applied);
    assert!(
        !d.join("rm.txt").exists(),
        "pop reintroduces the staged deletion"
    );
}

// ---- Case 11: STACKING native + staged (no double-log regression) ---------

#[test]
fn p34_stacking_native_then_staged_no_double_log() {
    let dir = crate::testutil::scratch_dir();
    let d = dir.path();
    s9_init(d);
    s9_commit(d, "base", &[("a.txt", "a-base\n"), ("b.txt", "b-base\n")]);

    // 1) Native stash (All) of an unstaged edit to a.txt.
    std::fs::write(d.join("a.txt"), "a-native\n").expect("edit a");
    let r1 = create_stash(d, Some("native-stash"), StashScope::All).expect("native");
    assert!(r1.created);

    // 2) Staged stash of b.txt.
    std::fs::write(d.join("b.txt"), "b-staged\n").expect("edit b");
    p34_stage(d, &["b.txt"]);
    let r2 = create_stash(d, Some("staged-stash"), StashScope::Staged).expect("staged");
    assert!(r2.created);

    // Exactly TWO entries — the hand-rolled push must not double-log.
    let list = list_stashes(d).expect("list");
    assert_eq!(list.len(), 2, "stacking must yield 2 entries, not 3");
    assert!(
        list[0].message.contains("staged-stash"),
        "stash@{{0}} is the staged one, got {:?}",
        list[0].message
    );
    assert!(
        list[1].message.contains("native-stash"),
        "stash@{{1}} is the older native one, got {:?}",
        list[1].message
    );
    let native_oid = list[1].oid.clone();

    // drop@{0} leaves the native survivor intact and re-indexed to 0.
    drop_stash(d, 0, None).expect("drop 0");
    let after = list_stashes(d).expect("list after drop");
    assert_eq!(after.len(), 1, "native survivor remains");
    assert_eq!(after[0].index, 0, "re-indexed to 0");
    assert_eq!(after[0].oid, native_oid, "survivor is the native entry");
    assert!(after[0].message.contains("native-stash"));

    // apply-by-index resolves the survivor (restores a.txt's native edit).
    let outcome = apply_stash(d, 0, false, None).expect("apply survivor");
    assert_eq!(outcome, ApplyStashOutcome::Applied);
    assert_eq!(s9_read(d, "a.txt"), "a-native\n", "native edit re-applied");
}

// ---- Case 12: `Staged` with nothing staged (unstaged present) -------------

#[test]
fn p34_staged_nothing_staged_but_unstaged_present() {
    let dir = crate::testutil::scratch_dir();
    let d = dir.path();
    s9_init(d);
    s9_commit(d, "base", &[("a.txt", "base\n")]);

    std::fs::write(d.join("a.txt"), "a-unstaged\n").expect("edit a"); // unstaged only

    let res = create_stash(d, None, StashScope::Staged).expect("create_stash staged");
    assert!(!res.created, "nothing staged -> created:false");
    assert_eq!(
        s9_read(d, "a.txt"),
        "a-unstaged\n",
        "unstaged change untouched"
    );
    assert_eq!(
        p34_unstaged_paths(d),
        vec!["a.txt".to_string()],
        "still an unstaged change"
    );
    assert_eq!(list_stashes(d).expect("list").len(), 0, "no entry");
}

// ---- Case 13: `Staged` rejected mid-merge (require_clean guard) -----------

#[test]
fn p34_staged_rejected_mid_merge() {
    let dir = crate::testutil::scratch_dir();
    let d = dir.path();
    let repo = s9_init(d);

    s9_commit(d, "base", &[("x.txt", "base\n"), ("y.txt", "y-base\n")]);
    let base = repo
        .find_commit(repo.head().expect("HEAD").target().expect("oid"))
        .expect("base");
    s9_commit_on_ref(
        &repo,
        "refs/heads/topic",
        &base,
        &[("x.txt", "topic\n")],
        "topic edits x",
    );
    s9_commit(d, "main edits x", &[("x.txt", "main\n")]);

    // Dirty unrelated file → conflicting merge pauses in Merge state.
    std::fs::write(d.join("y.txt"), "y-edited\n").expect("edit y");
    crate::git::merge::merge_branch(d, "topic", false).expect("merge");
    assert_eq!(
        git2::Repository::open(d).expect("reopen").state(),
        git2::RepositoryState::Merge,
        "precondition: mid-merge"
    );

    let before = list_stashes(d).expect("list before");
    match create_stash(d, None, StashScope::Staged) {
        Err(AppError::OperationInProgress(_)) => {}
        other => panic!("expected OperationInProgress, got {other:?}"),
    }
    assert_eq!(
        list_stashes(d).expect("list after").len(),
        before.len(),
        "rejected create must not mutate the stash stack"
    );
}

// =============================================== P33b reserved-name recovery
// Coverage for the Windows-reserved-path stash-apply fix (is_windows_reserved
// already truth-tabled above; wire shapes already covered). Three tiers:
//   A  stash_path_sets partitioning on SYNTHESIZED trees (cross-platform, no
//      real files — a `NUL` blob lives purely in the object DB);
//   B  the real git_stash_apply skip path, exercised end-to-end with an actual
//      on-disk `NUL` file (legal only on non-Windows → #[cfg(not(windows))]);
//   C  Windows detection + reflog-resolution via a fully synthesized stash
//      commit (a real `NUL` file cannot exist on NTFS → #[cfg(windows)]).

/// Build a tree from LEAF paths (forward-slash separators) via an in-memory
/// index — each becomes a blob entry, nested paths produce nested subtrees.
/// Synthesizes a stash's `^3` untracked tree (or a tracked stash tree) with a
/// reserved-name blob (e.g. `NUL`) that never touches the working directory,
/// so it lives purely in the object DB even on Windows.
fn rs_leaf_tree(repo: &git2::Repository, leaves: &[&str]) -> git2::Oid {
    let mut idx = git2::Index::new().expect("in-memory index");
    for p in leaves {
        let blob = repo.blob(format!("content:{p}\n").as_bytes()).expect("blob");
        let entry = make_index_entry(Path::new(p), blob, 0o100644).expect("entry");
        idx.add(&entry).expect("add");
    }
    idx.write_tree_to(repo).expect("write tree")
}

/// Register `oid` as stash@{0}: force-update `refs/stash` and guarantee EXACTLY
/// one reflog entry (mirrors create_staged_stash's log-once wiring — libgit2
/// auto-logs only when the reflog file already exists). This is what
/// stash_commit_oid / stash_path_sets / list_stashes resolve via reflog.get(0).
fn rs_register_stash(repo: &git2::Repository, oid: git2::Oid, msg: &str) {
    let before = repo.reflog("refs/stash").map(|r| r.len()).unwrap_or(0);
    repo.reference("refs/stash", oid, true, msg)
        .expect("force refs/stash");
    let after = repo.reflog("refs/stash").map(|r| r.len()).unwrap_or(0);
    if after == before {
        let sig = git2::Signature::now("Test User", "test@example.com").expect("sig");
        let mut reflog = repo.reflog("refs/stash").expect("reflog");
        reflog.append(oid, &sig, Some(msg)).expect("append");
        reflog.write().expect("write reflog");
    }
}

/// Synthesize + register a git-shaped 3-parent stash whose stash tree == base's
/// tree (no tracked delta) and whose `^3` untracked tree holds `untracked`
/// leaves. Parents: [base, index-commit, untracked-commit], mirroring git's
/// `stash_save --include-untracked` object shape.
fn rs_synth_untracked_stash(
    repo: &git2::Repository,
    base: &git2::Commit,
    untracked: &[&str],
    msg: &str,
) -> git2::Oid {
    let sig = git2::Signature::now("Test User", "test@example.com").expect("sig");
    let base_tree = base.tree().expect("base tree");
    let untracked_tree = repo
        .find_tree(rs_leaf_tree(repo, untracked))
        .expect("untracked tree");
    let untracked_commit = repo
        .find_commit(
            repo.commit(None, &sig, &sig, "untracked files on synthetic", &untracked_tree, &[base])
                .expect("untracked commit"),
        )
        .expect("find untracked commit");
    let index_commit = repo
        .find_commit(
            repo.commit(None, &sig, &sig, "index on synthetic", &base_tree, &[base])
                .expect("index commit"),
        )
        .expect("find index commit");
    let stash_oid = repo
        .commit(
            None,
            &sig,
            &sig,
            msg,
            &base_tree,
            &[base, &index_commit, &untracked_commit],
        )
        .expect("stash commit");
    rs_register_stash(repo, stash_oid, msg);
    stash_oid
}

// ---- Tier A.1: partition an untracked ^3 tree (reserved vs allowed) --------

#[test]
fn rs_a_untracked_reserved_partition() {
    let dir = crate::testutil::scratch_dir();
    let d = dir.path();
    let repo = s9_init(d);
    s9_commit(d, "base", &[("a.txt", "base\n")]);
    let base = repo.head().expect("HEAD").peel_to_commit().expect("base");

    // ^3 untracked tree: three reserved leaves + three benign look-alikes.
    rs_synth_untracked_stash(
        &repo,
        &base,
        &[
            "src/x/NUL",
            "a/b/PRN",
            "COM1",
            "src/x/keep.txt",
            "NULl",
            "readme.md",
        ],
        "WIP on main: synthetic reserved stash",
    );

    let (reserved, allowed) = stash_path_sets(&repo, 0).expect("path sets");
    assert_eq!(
        reserved,
        vec![
            "COM1".to_string(),
            "a/b/PRN".to_string(),
            "src/x/NUL".to_string(),
        ],
        "reserved must be the sorted device-name leaves only"
    );
    for benign in ["NULl", "readme.md", "src/x/keep.txt"] {
        assert!(
            allowed.contains(&benign.to_string()),
            "{benign} must be in the allowed set, got {allowed:?}"
        );
    }
    // Leaf paths only — no directory prefixes ever leak into either set.
    for p in reserved.iter().chain(allowed.iter()) {
        assert!(
            !matches!(p.as_str(), "src" | "src/x" | "a" | "a/b"),
            "directory prefix leaked into a path set: {p}"
        );
    }
}

// ---- Tier A.2: a 2-parent stash has no ^3 → untracked walk skipped --------

#[test]
fn rs_a_two_parent_stash_no_reserved() {
    let dir = crate::testutil::scratch_dir();
    let d = dir.path();
    let repo = s9_init(d);
    s9_commit(d, "base", &[("a.txt", "base\n")]);
    let base = repo.head().expect("HEAD").peel_to_commit().expect("base");
    let base_tree = base.tree().expect("base tree");
    let sig = git2::Signature::now("Test User", "test@example.com").expect("sig");

    // stash tree = base + one benign tracked add; parents = [base, index] (NO ^3).
    let stash_tree = {
        let mut tb = repo.treebuilder(Some(&base_tree)).expect("treebuilder");
        let blob = repo.blob(b"tracked\n").expect("blob");
        tb.insert("tracked.txt", blob, 0o100644).expect("insert");
        repo.find_tree(tb.write().expect("tree oid")).expect("tree")
    };
    let index_commit = repo
        .find_commit(
            repo.commit(None, &sig, &sig, "index on synthetic", &base_tree, &[&base])
                .expect("index commit"),
        )
        .expect("find index commit");
    let stash_oid = repo
        .commit(
            None,
            &sig,
            &sig,
            "WIP two-parent (no untracked)",
            &stash_tree,
            &[&base, &index_commit],
        )
        .expect("stash commit");
    rs_register_stash(&repo, stash_oid, "WIP two-parent (no untracked)");

    let (reserved, allowed) = stash_path_sets(&repo, 0).expect("path sets");
    assert!(
        reserved.is_empty(),
        "parent_count 2 → no ^3 walk → no reserved paths, got {reserved:?}"
    );
    assert_eq!(
        allowed,
        vec!["tracked.txt".to_string()],
        "only the tracked leaf is collected"
    );
}

// ---- Tier B: real-NUL end-to-end (the definitive skip test) ---------------
// `NUL` is a legal filename off Windows, so these exercise the actual
// git_stash_apply checkout path. Compiled + run on Linux CI; cfg'd out here.

#[cfg(not(windows))]
fn rs_b_tempdir() -> tempfile::TempDir {
    tempfile::Builder::new()
        .prefix("bonsai-nul-")
        .tempdir()
        .expect("tempdir")
}

/// Build a scratch repo with an untracked `dir/NUL` + `dir/keep.txt` AND a
/// tracked modification, then `stash push -u`. Returns the live TempDir.
#[cfg(not(windows))]
fn rs_b_nul_stash_fixture() -> tempfile::TempDir {
    let dir = rs_b_tempdir();
    let d = dir.path();
    s9_init(d);
    s9_commit(d, "base", &[("tracked.txt", "base\n")]);

    std::fs::create_dir_all(d.join("dir")).expect("mkdir");
    std::fs::write(d.join("dir/NUL"), "nul-content\n").expect("write NUL");
    std::fs::write(d.join("dir/keep.txt"), "keep\n").expect("write keep");
    std::fs::write(d.join("tracked.txt"), "modified\n").expect("modify tracked");

    let res = create_stash(d, None, StashScope::AllWithUntracked).expect("create_stash -u");
    assert!(res.created, "dirty tree + untracked NUL must stash");
    dir
}

#[cfg(not(windows))]
#[test]
fn rs_b_apply_reserved_then_skip() {
    let dir = rs_b_nul_stash_fixture();
    let d = dir.path();

    // Attempt 1: skip_reserved=false → blocked, nothing applied, stash retained.
    match apply_stash(d, 0, false, None).expect("apply(false)") {
        ApplyStashOutcome::ReservedPaths { paths } => assert!(
            paths.iter().any(|p| p == "dir/NUL"),
            "ReservedPaths must name dir/NUL, got {paths:?}"
        ),
        other => panic!("expected ReservedPaths, got {other:?}"),
    }
    assert_eq!(s9_read(d, "tracked.txt"), "base\n", "tracked mod NOT applied");
    assert!(!d.join("dir/keep.txt").exists(), "benign untracked NOT applied");
    assert_eq!(list_stashes(d).expect("list").len(), 1, "stash retained");

    // Attempt 2: skip_reserved=true → applies everything but the NUL leaf.
    match apply_stash(d, 0, true, None).expect("apply(true)") {
        ApplyStashOutcome::AppliedSkippingReserved { skipped } => assert!(
            skipped.iter().any(|p| p == "dir/NUL"),
            "skipped must name dir/NUL, got {skipped:?}"
        ),
        other => panic!("expected AppliedSkippingReserved, got {other:?}"),
    }
    assert_eq!(s9_read(d, "dir/keep.txt"), "keep\n", "benign untracked restored");
    assert_eq!(s9_read(d, "tracked.txt"), "modified\n", "tracked mod restored");
    assert!(!d.join("dir/NUL").exists(), "reserved NUL NOT restored");
    assert_eq!(
        list_stashes(d).expect("list").len(),
        1,
        "apply must NOT drop the stash"
    );
}

#[cfg(not(windows))]
#[test]
fn rs_b_pop_skip_retains_stash() {
    let dir = rs_b_nul_stash_fixture();
    let d = dir.path();

    match pop_stash(d, 0, true, None).expect("pop(true)") {
        ApplyStashOutcome::AppliedSkippingReserved { skipped } => assert!(
            skipped.iter().any(|p| p == "dir/NUL"),
            "skipped must name dir/NUL, got {skipped:?}"
        ),
        other => panic!("expected AppliedSkippingReserved, got {other:?}"),
    }
    assert_eq!(s9_read(d, "dir/keep.txt"), "keep\n", "benign untracked restored");
    assert_eq!(s9_read(d, "tracked.txt"), "modified\n", "tracked mod restored");
    assert!(!d.join("dir/NUL").exists(), "reserved NUL NOT restored");
    assert_eq!(
        list_stashes(d).expect("list").len(),
        1,
        "DATA SAFETY: pop+skip must RETAIN the stash (reserved blobs live only there)"
    );
}

// ---- Tier C: Windows synthetic-stash detection (no real NUL possible) ------

#[cfg(windows)]
#[test]
fn rs_c_windows_synthetic_reserved_detection() {
    let dir = crate::testutil::scratch_dir();
    let d = dir.path();
    let repo = s9_init(d);
    s9_commit(d, "base", &[("a.txt", "base\n")]);
    let base = repo.head().expect("HEAD").peel_to_commit().expect("base");

    // ^3 untracked tree: an un-writable NUL blob + a benign keep.txt. The whole
    // stash is synthesized in the object DB — no file ever hits NTFS.
    rs_synth_untracked_stash(
        &repo,
        &base,
        &["dir/NUL", "dir/keep.txt"],
        "WIP on main: synthetic NUL stash",
    );

    // The `false` path validates Windows detection + reflog resolution without
    // needing the un-writable file: preflight blocks, mutating nothing.
    match apply_stash(d, 0, false, None).expect("apply(false)") {
        ApplyStashOutcome::ReservedPaths { paths } => assert!(
            paths.iter().any(|p| p == "dir/NUL"),
            "ReservedPaths must name dir/NUL, got {paths:?}"
        ),
        other => panic!("expected ReservedPaths, got {other:?}"),
    }
    assert!(
        !d.join("dir/keep.txt").exists(),
        "preflight must not write anything"
    );
    assert_eq!(list_stashes(d).expect("list").len(), 1, "stash retained");
}

// ===================================================== audit 2026-08-07

/// §2.2: `create_staged_stash` must fold CHECK-IN FILTERED worktree bytes.
/// Under `core.autocrlf=true` a CRLF worktree file must land in the stash
/// tree as an LF blob (what `git add` would stage), never raw CRLF.
#[test]
fn staged_stash_folds_filtered_worktree_bytes_under_autocrlf() {
    let dir = crate::testutil::scratch_dir();
    let d = dir.path();
    let repo = s9_init(d);
    repo.config()
        .expect("config")
        .set_bool("core.autocrlf", true)
        .expect("autocrlf");
    s9_commit(d, "base", &[("f.txt", "one\r\n")]); // blob is LF via filter

    // Stage a CRLF modification, then edit the worktree AGAIN (CRLF) so the
    // stash fold has fresher worktree content than the staged blob.
    std::fs::write(d.join("f.txt"), "one\r\ntwo\r\n").expect("edit");
    crate::git::stage::stage_paths(d, &["f.txt".to_string()]).expect("stage");
    std::fs::write(d.join("f.txt"), "one\r\ntwo\r\nthree\r\n").expect("edit again");

    let res = create_stash(d, None, StashScope::Staged).expect("staged stash");
    assert!(res.created);

    let entries = list_stashes(d).expect("list");
    let repo2 = git2::Repository::open(d).expect("open");
    let tree = repo2
        .find_commit(git2::Oid::from_str(&entries[0].oid).expect("oid"))
        .expect("stash commit")
        .tree()
        .expect("stash tree");
    let blob_id = tree.get_name("f.txt").expect("f.txt in stash tree").id();
    let content = repo2.find_blob(blob_id).expect("blob").content().to_vec();
    assert_eq!(
        content, b"one\ntwo\nthree\n",
        "stash tree blob must be LF-only (check-in filtered)"
    );
}

/// §3.2: a CHECKOUT-level GIT_ECONFLICT (a dirty file in the way; nothing
/// applied, no index conflict entries) must surface as `AppError::Git`
/// carrying libgit2's message — NOT as `Conflicts { paths: [] }`. The
/// stash is retained and the dirty file untouched.
#[test]
fn apply_blocked_at_checkout_errors_instead_of_empty_conflicts() {
    let dir = crate::testutil::scratch_dir();
    let d = dir.path();
    s9_init(d);
    s9_commit(d, "base", &[("f.txt", "base\n")]);

    // Stash a change (worktree reverts to base), then dirty the same file.
    std::fs::write(d.join("f.txt"), "stashed\n").expect("edit");
    assert!(create_stash(d, None, StashScope::All).expect("stash").created);
    std::fs::write(d.join("f.txt"), "dirty\n").expect("dirty");

    for (label, result) in [
        ("apply", apply_stash(d, 0, false, None)),
        ("pop", pop_stash(d, 0, false, None)),
    ] {
        let err = result.expect_err(&format!("{label} must error, not empty Conflicts"));
        assert!(
            matches!(&err, AppError::Git(m) if m.contains("blocked at checkout")),
            "{label}: got {err:?}"
        );
    }
    assert_eq!(list_stashes(d).expect("list").len(), 1, "stash retained");
    assert_eq!(
        std::fs::read_to_string(d.join("f.txt")).expect("read"),
        "dirty\n",
        "the blocking dirty file is untouched"
    );
}
