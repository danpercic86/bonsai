//! P73 — integration guard for the FAILED-fresh-clone rollback in
//! `update_submodule` (contract `docs/contracts/P73-submodule-reconnect.md`
//! §4-§5). The rollback exists to clear the residue of a genuinely failed FRESH
//! clone; it must NEVER delete content the user put in the submodule folder
//! before it was ever cloned.
//!
//! All submodules use a LOCAL `file://` URL (no creds, no network). Scratch on
//! D:. Skips (passes with a note) w/o `git`.

use std::path::Path;

use bonsai_core::git::submodule::{
    add_submodule, list_submodules, sync_submodule, update_submodule, SubmoduleStatus,
};
use crate::common;
use crate::common::{commit_fixed, file_url, git, init_repo, scratch_dir};

macro_rules! require_git {
    () => {
        if !common::have_git() {
            eprintln!("skipping: `git` CLI not found on PATH");
            return;
        }
    };
}

const SUB_PATH: &str = "vendor/sub";
const USER_CONTENT: &str = "MY OWN NOTES - NOT TRACKED BY ANY REPO\n";

/// Upstream sub-repo with one commit touching `lib.txt`. Returns (dir, url).
fn build_sub() -> (tempfile::TempDir, String) {
    let dir = init_repo();
    let p = dir.path();
    std::fs::write(p.join("lib.txt"), "sub v1\n").expect("write lib.txt");
    git(p, &["add", "-A"]);
    commit_fixed(p, "sub v1");
    let url = file_url(p);
    (dir, url)
}

/// Superproject with the submodule added at `SUB_PATH` and committed.
fn build_super_with_sub(url: &str) -> tempfile::TempDir {
    let dir = init_repo();
    let p = dir.path();
    std::fs::write(p.join("top.txt"), "super\n").expect("write top.txt");
    git(p, &["add", "-A"]);
    commit_fixed(p, "super: initial");
    add_submodule(p, url, SUB_PATH).expect("add_submodule");
    git(p, &["add", "-A"]);
    commit_fixed(p, "super: add submodule");
    dir
}

/// A FRESH clone of the superproject: the submodule is registered in
/// `.gitmodules` but has never been cloned — no `.git/modules/<name>`, no
/// gitlink, `WD_UNINITIALIZED`. Returns (parent_dir, worktree_path).
fn fresh_clone_of(super_dir: &Path) -> (tempfile::TempDir, std::path::PathBuf) {
    let parent = scratch_dir();
    git(parent.path(), &["clone", &file_url(super_dir), "work"]);
    let work = parent.path().join("work");
    (parent, work)
}

/// REGRESSION (reviewer MUST-FIX 1): the failed-fresh-clone rollback must not
/// delete the user's own files.
///
/// Scenario: fresh clone of a superproject whose submodule was never cloned, and
/// the user has dropped an uncommitted `vendor/sub/lib.txt` into the folder.
/// `update_submodule` clones the module gitdir fine, then its SAFE checkout
/// correctly refuses (`lib.txt` would be clobbered) → the update errors out on
/// the fresh-clone path with `WD_UNINITIALIZED` set and the workdir already
/// existing. A contents-only cleanup keyed on those two facts alone would eat
/// `lib.txt`; the `workdir_was_empty` snapshot flag is what prevents it.
#[test]
fn failed_fresh_clone_rollback_preserves_user_files() {
    require_git!();
    let (_sub, url) = build_sub();
    let super_dir = build_super_with_sub(&url);
    let (_parent, work) = fresh_clone_of(super_dir.path());

    // Precondition: never cloned — no cached module gitdir.
    let modules = work.join(".git").join("modules").join(SUB_PATH);
    assert!(!modules.exists(), "precondition: no cached module gitdir yet");

    // The user's own file sits where the submodule will be checked out.
    let sub_wd = work.join(SUB_PATH);
    std::fs::create_dir_all(&sub_wd).expect("mkdir submodule folder");
    let user_file = sub_wd.join("lib.txt");
    std::fs::write(&user_file, USER_CONTENT).expect("write user file");
    let before = std::fs::read(&user_file).expect("read user file");

    let res = update_submodule(&work, SUB_PATH);
    let err = match res {
        Err(e) => e.to_string(),
        Ok(()) => panic!("the SAFE checkout must refuse to clobber the user's file"),
    };
    // ...and it must be THE checkout refusal, not some unrelated earlier failure
    // (a bare `is_err()` would be satisfied by e.g. a clone or config error).
    let low = err.to_lowercase();
    assert!(
        low.contains("conflict") || low.contains("checkout") || low.contains("already has files"),
        "the failure must name the checkout conflict, got: {err}"
    );

    // THE assertion: the user's bytes are untouched.
    assert!(user_file.exists(), "the user's file must still exist after the refused update");
    assert_eq!(
        std::fs::read(&user_file).expect("read user file after"),
        before,
        "the user's file must be byte-identical after the refused update",
    );

    // ...and the repo is left COHERENT, not half-torn: if libgit2's clone wrote a
    // gitlink, the gitdir it points at must still be there (deleting it would
    // leave a dangling link that makes even `git submodule status` fail).
    if sub_wd.join(".git").exists() {
        assert!(
            work.join(".git").join("modules").join(SUB_PATH).exists(),
            "a gitlink was left behind, so its module gitdir must survive too"
        );
    }
    let status = std::process::Command::new("git")
        .args(["submodule", "status", "--", SUB_PATH])
        .current_dir(&work)
        .output()
        .expect("run git submodule status");
    assert!(
        status.status.success(),
        "`git submodule status` must still work after the refusal: {}",
        String::from_utf8_lossy(&status.stderr)
    );
}

/// The orphan cleanup the rollback exists for must still fire: a genuinely
/// failed FRESH clone (bogus URL) leaves no `.git/modules/<key>` behind and no
/// `submodule.<name>.url` registration, so a retry is not wedged.
#[test]
fn failed_fresh_clone_rollback_still_clears_its_own_residue() {
    require_git!();
    let (_sub, url) = build_sub();
    let super_dir = build_super_with_sub(&url);
    let (_parent, work) = fresh_clone_of(super_dir.path());

    // Repoint the submodule at a nonexistent local repo → the clone fails.
    let bogus = file_url(&work.join("does-not-exist"));
    git(&work, &["config", "-f", ".gitmodules", "submodule.vendor/sub.url", &bogus]);

    let res = update_submodule(&work, SUB_PATH);
    assert!(res.is_err(), "a clone from a nonexistent URL must fail: {res:?}");

    // libgit2 keys the clone repodir on the PATH while Bonsai's cleanup keys on
    // the NAME; here both are `vendor/sub`, so probing the single key is enough.
    let dir = work.join(".git").join("modules").join(SUB_PATH);
    assert!(!dir.exists(), "the failed clone's module gitdir must be cleaned up");
    // The `submodule.<name>.{url,update,active}` keys written by `init` are gone
    // (git leaves the now-empty `[submodule "..."]` section header behind, which
    // is inert — a retry re-writes the keys).
    for key in ["url", "update", "active"] {
        let full = format!("submodule.vendor/sub.{key}");
        assert!(
            !common::git_ok(&work, &["config", "--local", "--get", &full]),
            "the registration key written by init must be cleared: {key}",
        );
    }

    // Acceptance criterion 8: the RETRY leg — with the url repointed at the good
    // source, sync + update must now succeed. No reinit residue may block it.
    git(&work, &["config", "-f", ".gitmodules", "submodule.vendor/sub.url", &url]);
    sync_submodule(&work, SUB_PATH).expect("sync after rollback");
    update_submodule(&work, SUB_PATH).expect("retry update after rollback must succeed");
    let row = list_submodules(&work)
        .expect("list_submodules")
        .into_iter()
        .find(|s| s.path == SUB_PATH)
        .expect("the submodule row");
    assert_eq!(row.status, SubmoduleStatus::UpToDate, "row after successful retry");
}

/// P73 backstop (reviewer SHOULD-FIX 2): when `.git/modules/<key>` exists but is
/// NOT an openable repository (an aborted clone left an incomplete dir), the
/// salvage stands down, libgit2 takes its clone branch and refuses with
/// `attempt to reinitialize '<abs path>'`. That raw libgit2 prose must never reach
/// the UI — the user gets the actionable sentence naming the folder to delete.
#[test]
fn incomplete_module_gitdir_reports_leftover_data_not_reinitialize() {
    require_git!();
    let (_sub, url) = build_sub();
    let super_dir = build_super_with_sub(&url);
    let (_parent, work) = fresh_clone_of(super_dir.path());

    // Forge a "looks like a repo, cannot be opened" module gitdir: the layout
    // libgit2's init probe accepts, with a config file it cannot parse.
    let modules = work.join(".git").join("modules").join(SUB_PATH);
    std::fs::create_dir_all(modules.join("objects")).expect("mkdir objects");
    std::fs::create_dir_all(modules.join("refs")).expect("mkdir refs");
    std::fs::write(modules.join("HEAD"), "ref: refs/heads/main\n").expect("write HEAD");
    std::fs::write(modules.join("config"), "[core\nthis is not valid ini\n").expect("write config");

    let err = match update_submodule(&work, SUB_PATH) {
        Err(e) => e.to_string(),
        Ok(()) => panic!("an unopenable module gitdir must not silently succeed"),
    };
    assert!(
        !err.to_lowercase().contains("reinitialize"),
        "the raw libgit2 message must never reach the UI, got: {err}"
    );
    assert!(
        err.contains("Bonsai has leftover data for this submodule that it cannot reuse.")
            && err.contains("\".git/modules/vendor/sub\""),
        "the refusal must name the folder to delete, got: {err}"
    );
    // Bonsai must NOT have deleted it — a locked/permission-blocked but VALID
    // repo must never be destroyed, so the remedy stays the user's call.
    assert!(modules.exists(), "the module gitdir must be left in place");
}
