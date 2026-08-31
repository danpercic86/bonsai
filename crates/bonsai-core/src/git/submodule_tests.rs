//! Unit tests for [`crate::git::submodule`] (P19 §8.2), split out of
//! `submodule.rs` to keep that file under the ~500-line limit (CLAUDE.md).
//! Declared from `submodule.rs` via `#[cfg(test)] #[path = ...] mod tests;`,
//! the same pattern as `cred.rs` — so `use super::*` still names the
//! submodule module.

use super::*;

/// §8.2 #6: wire shapes must match the TS mirrors — the status enum
/// serializes to camelCase strings and `SubmoduleInfo` to camelCase keys.
#[test]
fn status_enum_serializes_camel_case() {
    let cases = [
        (SubmoduleStatus::Uninitialized, "uninitialized"),
        (SubmoduleStatus::UpToDate, "upToDate"),
        (SubmoduleStatus::OutOfSync, "outOfSync"),
        (SubmoduleStatus::ModifiedWorkdir, "modifiedWorkdir"),
    ];
    for (variant, wire) in cases {
        let v = serde_json::to_value(variant).expect("json");
        assert_eq!(v, serde_json::json!(wire));
    }
}

#[test]
fn info_serializes_camel_case_keys() {
    let info = SubmoduleInfo {
        name: "vendor/libcore".to_string(),
        path: "vendor/libcore".to_string(),
        abs_path: "/repo/vendor/libcore".to_string(),
        url: Some("https://example.com/libcore.git".to_string()),
        head_oid: Some("a".repeat(40)),
        index_oid: Some("b".repeat(40)),
        wt_oid: None,
        status: SubmoduleStatus::Uninitialized,
    };
    let v = serde_json::to_value(&info).expect("json");
    assert_eq!(
        v,
        serde_json::json!({
            "name": "vendor/libcore",
            "path": "vendor/libcore",
            "absPath": "/repo/vendor/libcore",
            "url": "https://example.com/libcore.git",
            "headOid": "a".repeat(40),
            "indexOid": "b".repeat(40),
            "wtOid": null,
            "status": "uninitialized"
        })
    );
}

/// §8.2 #7: `classify_status` truth table, including the priority tie-breaks.
#[test]
fn classify_status_priority_table() {
    use git2::SubmoduleStatus as S;

    // Uninitialized wins even when combined with anything else.
    assert_eq!(
        classify_status(S::WD_UNINITIALIZED),
        SubmoduleStatus::Uninitialized
    );
    assert_eq!(
        classify_status(S::WD_UNINITIALIZED | S::WD_MODIFIED | S::WD_WD_MODIFIED),
        SubmoduleStatus::Uninitialized
    );

    // Superproject-pointer / checked-out-commit mismatch → OutOfSync.
    assert_eq!(classify_status(S::WD_MODIFIED), SubmoduleStatus::OutOfSync);
    assert_eq!(classify_status(S::INDEX_ADDED), SubmoduleStatus::OutOfSync);
    assert_eq!(
        classify_status(S::INDEX_DELETED),
        SubmoduleStatus::OutOfSync
    );
    assert_eq!(
        classify_status(S::INDEX_MODIFIED),
        SubmoduleStatus::OutOfSync
    );

    // OutOfSync outranks internal dirtiness (documented tie-break).
    assert_eq!(
        classify_status(S::WD_MODIFIED | S::WD_WD_MODIFIED),
        SubmoduleStatus::OutOfSync
    );

    // Internal dirtiness only → ModifiedWorkdir.
    assert_eq!(
        classify_status(S::WD_INDEX_MODIFIED),
        SubmoduleStatus::ModifiedWorkdir
    );
    assert_eq!(
        classify_status(S::WD_WD_MODIFIED),
        SubmoduleStatus::ModifiedWorkdir
    );
    assert_eq!(
        classify_status(S::WD_UNTRACKED),
        SubmoduleStatus::ModifiedWorkdir
    );

    // Clean, checked-out, matching.
    assert_eq!(classify_status(S::IN_HEAD), SubmoduleStatus::UpToDate);
    assert_eq!(
        classify_status(S::IN_HEAD | S::IN_INDEX | S::IN_CONFIG | S::IN_WD),
        SubmoduleStatus::UpToDate
    );
}

/// Blank / whitespace names are rejected before touching the repo.
#[test]
fn blank_name_is_invalid() {
    let dir = crate::testutil::scratch_dir();
    let repo = git2::Repository::init(dir.path()).expect("init");
    for name in ["", "   "] {
        match open_submodule(&repo, name).map(|_| ()) {
            Err(AppError::InvalidName(_)) => {}
            other => panic!("expected InvalidName for {name:?}, got {other:?}"),
        }
    }
}

/// An unknown submodule name maps NotFound → `AppError::Git` (§OPEN-3).
#[test]
fn unknown_name_maps_to_git_error() {
    let dir = crate::testutil::scratch_dir();
    let repo = git2::Repository::init(dir.path()).expect("init");
    match open_submodule(&repo, "does/not/exist").map(|_| ()) {
        Err(AppError::Git(m)) => assert!(m.contains("does/not/exist"), "{m}"),
        other => panic!("expected Git error, got {other:?}"),
    }
}

/// P60d/P82: the deinit/remove argv builders are byte-exact — `-f` appears only
/// when `force`, and `path` is the FINAL token, immediately after `--`.
#[test]
fn deinit_args_exact() {
    assert_eq!(
        deinit_args("vendor/sub", true),
        ["submodule", "deinit", "-f", "--", "vendor/sub"]
            .map(String::from)
            .to_vec()
    );
    assert_eq!(
        deinit_args("vendor/sub", false),
        ["submodule", "deinit", "--", "vendor/sub"]
            .map(String::from)
            .to_vec()
    );
}

#[test]
fn rm_args_exact() {
    assert_eq!(
        rm_args("vendor/sub", true),
        ["rm", "-f", "--", "vendor/sub"].map(String::from).to_vec()
    );
    assert_eq!(
        rm_args("vendor/sub", false),
        ["rm", "--", "vendor/sub"].map(String::from).to_vec()
    );
}

/// P82: the two outcome enums serialize to the tagged, camelCase wire shapes the
/// TS mirrors expect.
#[test]
fn outcome_enums_serialize_camel_case_kind() {
    assert_eq!(
        serde_json::to_value(SubmoduleDeinitOutcome::Deinitialized).expect("json"),
        serde_json::json!({ "kind": "deinitialized" })
    );
    assert_eq!(
        serde_json::to_value(SubmoduleDeinitOutcome::DirtyNeedsForce).expect("json"),
        serde_json::json!({ "kind": "dirtyNeedsForce" })
    );
    assert_eq!(
        serde_json::to_value(SubmoduleRemoveOutcome::Removed).expect("json"),
        serde_json::json!({ "kind": "removed" })
    );
    assert_eq!(
        serde_json::to_value(SubmoduleRemoveOutcome::DirtyNeedsForce).expect("json"),
        serde_json::json!({ "kind": "dirtyNeedsForce" })
    );
}

/// P60d injection-safety: a space/`;`-bearing path stays exactly ONE argv
/// token, always the element AFTER `--` — never split, never a 2nd command.
#[test]
fn args_keep_metachar_path_as_single_token_after_dashdash() {
    let evil = "a b; rm -rf /";
    let d = deinit_args(evil, true);
    assert_eq!(d.last().unwrap(), evil);
    assert_eq!(d[d.len() - 2], "--");
    let r = rm_args(evil, true);
    assert_eq!(r.last().unwrap(), evil);
    assert_eq!(r[r.len() - 2], "--");
}

/// F-A7-2: the .git/modules name guard — traversal/absolute/dot components
/// reject; ordinary (incl. nested) names pass.
#[test]
fn modules_name_validation_rejects_traversal() {
    for bad in [
        "..", "../x", "a/../b", "..\\evil", "a\\..\\b", "/abs", "C:/abs", "C:\\abs", "", "   ",
        "a//b", "./x", "a/.",
    ] {
        match validate_modules_name(bad) {
            Err(AppError::Git(_)) => {}
            other => panic!("name {bad:?} must be rejected, got {other:?}"),
        }
    }
    for good in [
        "sub",
        "vendor/libcore",
        "a.b",
        "with space",
        "..dots",
        "x..",
    ] {
        validate_modules_name(good).unwrap_or_else(|e| panic!("name {good:?} must pass: {e:?}"));
    }
}

/// F-A7-2: `remove_submodule` refuses a hostile name BEFORE running any
/// git command (the runner panics if invoked).
#[test]
fn remove_submodule_rejects_hostile_name_before_running_git() {
    struct PanicRunner;
    impl GitRunner for PanicRunner {
        fn run(&self, _args: &[String], _cwd: &Path) -> Result<String, AppError> {
            panic!("runner must not be invoked for a hostile submodule name");
        }
    }
    let dir = crate::testutil::scratch_dir();
    git2::Repository::init(dir.path()).expect("init");
    match remove_submodule(dir.path(), &PanicRunner, "../../escape", false) {
        Err(AppError::Git(m)) => assert!(m.contains("unsafe name"), "{m}"),
        other => panic!("hostile name must be Git error, got {other:?}"),
    }
}

/// A recording runner: captures every argv it is handed and returns Ok("").
#[derive(Default)]
struct RecordRunner {
    calls: RefCell<Vec<Vec<String>>>,
}
impl GitRunner for RecordRunner {
    fn run(&self, args: &[String], _cwd: &Path) -> Result<String, AppError> {
        self.calls.borrow_mut().push(args.to_vec());
        Ok(String::new())
    }
}

/// Seed a superproject with one submodule checked out from a local source, and
/// return (superproject dir, submodule name). The submodule worktree is clean.
fn seed_superproject_with_submodule() -> (tempfile::TempDir, tempfile::TempDir, String) {
    let src_dir = crate::testutil::scratch_dir();
    let src = git2::Repository::init(src_dir.path()).expect("init source");
    seed_commit(&src);
    let src_url = src_dir.path().to_string_lossy().replace('\\', "/");

    let sup_dir = crate::testutil::scratch_dir();
    let sup = git2::Repository::init(sup_dir.path()).expect("init superproject");
    seed_commit(&sup);
    let info = add_submodule(sup_dir.path(), &src_url, "vendor/sub").expect("add submodule");
    (sup_dir, src_dir, info.name)
}

/// P82 AC#3: `force=false` on a CLEAN submodule proceeds WITHOUT `-f` and
/// reports `Deinitialized`.
#[test]
fn deinit_clean_no_force_omits_dash_f() {
    let (sup, _src, name) = seed_superproject_with_submodule();
    let runner = RecordRunner::default();
    let outcome = deinit_submodule(sup.path(), &runner, &name, false).expect("deinit");
    assert_eq!(outcome, SubmoduleDeinitOutcome::Deinitialized);
    let calls = runner.calls.borrow();
    assert_eq!(calls.len(), 1);
    assert!(!calls[0].contains(&"-f".to_string()), "clean deinit must omit -f");
}

/// P82 AC#2: `force=false` on a DIRTY submodule returns `DirtyNeedsForce` and
/// invokes the runner ZERO times (no mutation).
#[test]
fn deinit_dirty_no_force_refuses_without_running() {
    let (sup, _src, name) = seed_superproject_with_submodule();
    // Dirty the submodule worktree with an untracked file.
    std::fs::write(sup.path().join("vendor/sub/dirty.txt"), "wip\n").expect("write");
    let runner = RecordRunner::default();
    let outcome = deinit_submodule(sup.path(), &runner, &name, false).expect("deinit");
    assert_eq!(outcome, SubmoduleDeinitOutcome::DirtyNeedsForce);
    assert_eq!(runner.calls.borrow().len(), 0, "must not run git when refusing");
}

/// P82 AC#4: `force=true` on a DIRTY submodule discards and runs WITH `-f`.
#[test]
fn deinit_dirty_force_runs_with_dash_f() {
    let (sup, _src, name) = seed_superproject_with_submodule();
    std::fs::write(sup.path().join("vendor/sub/dirty.txt"), "wip\n").expect("write");
    let runner = RecordRunner::default();
    let outcome = deinit_submodule(sup.path(), &runner, &name, true).expect("deinit");
    assert_eq!(outcome, SubmoduleDeinitOutcome::Deinitialized);
    let calls = runner.calls.borrow();
    assert!(calls[0].contains(&"-f".to_string()), "forced deinit must pass -f");
}

/// P82 AC#2/#4: `remove_submodule` mirrors the deinit dirty/force semantics
/// across BOTH shell-outs.
#[test]
fn remove_dirty_no_force_refuses_then_force_runs_with_dash_f() {
    let (sup, _src, name) = seed_superproject_with_submodule();
    std::fs::write(sup.path().join("vendor/sub/dirty.txt"), "wip\n").expect("write");

    let refuse_runner = RecordRunner::default();
    let refused = remove_submodule(sup.path(), &refuse_runner, &name, false).expect("remove");
    assert_eq!(refused, SubmoduleRemoveOutcome::DirtyNeedsForce);
    assert_eq!(refuse_runner.calls.borrow().len(), 0);

    let force_runner = RecordRunner::default();
    let removed = remove_submodule(sup.path(), &force_runner, &name, true).expect("remove");
    assert_eq!(removed, SubmoduleRemoveOutcome::Removed);
    let calls = force_runner.calls.borrow();
    assert_eq!(calls.len(), 2, "deinit + rm");
    assert!(calls.iter().all(|c| c.contains(&"-f".to_string())), "both forced");
}

/// F-A7-10: a failed clone rolls back the add-setup residue (.gitmodules
/// entry, config, dirs) so a retry with a good url succeeds instead of
/// hitting "already exists".
#[test]
fn add_submodule_rolls_back_on_clone_failure() {
    let sup_dir = crate::testutil::scratch_dir();
    let d = sup_dir.path();
    let repo = git2::Repository::init(d).expect("init superproject");
    seed_commit(&repo);

    // A url that fails fast via the local transport (no network).
    let bad_url = d
        .join("definitely-missing-source")
        .to_string_lossy()
        .replace('\\', "/");
    let err = add_submodule(d, &bad_url, "vendor/sub");
    assert!(err.is_err(), "clone from a missing source must fail");

    // Residue is gone: no .gitmodules entry, no registered submodule.
    assert!(
        repo.find_submodule("vendor/sub").is_err(),
        ".gitmodules entry must be rolled back"
    );
    assert!(
        !d.join("vendor").join("sub").exists(),
        "partial checkout dir must be rolled back"
    );

    // Retry with a valid LOCAL source now succeeds (no Exists collision).
    let src_dir = crate::testutil::scratch_dir();
    let src = git2::Repository::init(src_dir.path()).expect("init source");
    seed_commit(&src);
    let src_url = src_dir.path().to_string_lossy().replace('\\', "/");
    let info = add_submodule(d, &src_url, "vendor/sub").expect("retry succeeds");
    assert_eq!(info.path, "vendor/sub");
}

/// Minimal deterministic commit so a repo has a HEAD (used by the rollback
/// test's superproject + source repos).
fn seed_commit(repo: &git2::Repository) {
    let mut cfg = repo.config().expect("config");
    cfg.set_str("user.name", "Test User").expect("name");
    cfg.set_str("user.email", "test@example.com")
        .expect("email");
    drop(cfg);
    let wd = repo.workdir().expect("workdir");
    std::fs::write(wd.join("seed.txt"), "seed\n").expect("write");
    let mut index = repo.index().expect("index");
    index.add_path(Path::new("seed.txt")).expect("add");
    index.write().expect("write index");
    let tree_oid = index.write_tree().expect("tree");
    let tree = repo.find_tree(tree_oid).expect("find tree");
    let sig = git2::Signature::now("Test User", "test@example.com").expect("sig");
    repo.commit(Some("HEAD"), &sig, &sig, "seed\n", &tree, &[])
        .expect("commit");
}

/// P60d: `add_submodule` rejects blank url/path and traversing paths BEFORE
/// opening the repo (so no git/network is touched on obviously bad input).
#[test]
fn add_submodule_rejects_bad_url_and_path() {
    let dir = crate::testutil::scratch_dir();
    match add_submodule(dir.path(), "   ", "vendor/x") {
        Err(AppError::InvalidName(_)) => {}
        other => panic!("blank url ⇒ InvalidName, got {other:?}"),
    }
    match add_submodule(dir.path(), "https://example.com/x.git", "  ") {
        Err(AppError::InvalidName(_)) => {}
        other => panic!("blank path ⇒ InvalidName, got {other:?}"),
    }
    match add_submodule(dir.path(), "https://example.com/x.git", "../escape") {
        Err(AppError::Other(_)) => {}
        other => panic!("traversing path ⇒ Other (validate_rel_path), got {other:?}"),
    }
}
