//! Untracked-file safety of the autostash carry (branch-switch data-loss
//! regressions).
//!
//! `checkout_branch_autostash` stashes with `INCLUDE_UNTRACKED`, so brand-new
//! files leave the worktree for the duration of the switch. libgit2's
//! untracked-restore phase of `git_stash_apply` can silently FAIL to write
//! those blobs back (binary collision, file-vs-directory collision, textual
//! collision resolved into markers) while leaving NO conflict entry in the
//! index — the naive "Ok(()) && !has_conflicts => drop" rule then destroys the
//! only remaining copy. Every test here asserts the safety invariant:
//!
//!   the stash is dropped ONLY when every carried untracked blob is
//!   byte-identical on disk afterwards.

use std::path::Path;

use super::*;
use crate::git::stash::{list_stashes, ApplyStashOutcome};

fn u_init(dir: &Path) -> git2::Repository {
    let repo =
        git2::Repository::init_opts(dir, git2::RepositoryInitOptions::new().initial_head("main"))
            .expect("init repo");
    let mut cfg = repo.config().expect("config");
    cfg.set_str("user.name", "Test User").expect("name");
    cfg.set_str("user.email", "test@example.com").expect("email");
    cfg.set_bool("core.autocrlf", false).expect("autocrlf");
    drop(cfg);
    repo
}

fn u_commit(dir: &Path, msg: &str, files: &[(&str, &str)]) {
    use crate::git::stage::stage_paths;
    for (name, content) in files {
        std::fs::write(dir.join(name), content).expect("write file");
    }
    stage_paths(dir, &files.iter().map(|(n, _)| n.to_string()).collect::<Vec<_>>())
        .expect("stage");
    crate::git::commit::create_commit(dir, msg, None, false).expect("commit");
}

/// Branch `name` off HEAD without moving the worktree.
fn u_branch(repo: &git2::Repository, name: &str) {
    let head = repo.head().expect("head").peel_to_commit().expect("commit");
    repo.branch(name, &head, false).expect("branch");
}

/// The carried paths a partial apply could not restore, or `[]`.
fn unrestored(outcome: &ApplyStashOutcome) -> &[String] {
    match outcome {
        ApplyStashOutcome::AppliedPartially { unrestored } => unrestored,
        _ => &[],
    }
}

/// A brand-new BINARY file whose path the target branch tracks. The carried
/// bytes cannot be merged, so they must NOT be silently replaced by the
/// branch's version with the stash dropped.
#[test]
fn binary_untracked_collision_retains_stash() {
    let td = tempfile::tempdir().expect("tempdir");
    let dir = td.path();
    let repo = u_init(dir);
    u_commit(dir, "base", &[("base.txt", "base\n")]);
    u_branch(&repo, "feat");
    checkout_branch(dir, "feat").expect("co feat");
    std::fs::write(dir.join("bin.dat"), [0u8, 1, 2, 3, 0, 9]).expect("write");
    crate::git::stage::stage_paths(dir, &["bin.dat".to_string()]).expect("stage");
    crate::git::commit::create_commit(dir, "feat bin", None, false).expect("commit");
    checkout_branch(dir, "main").expect("co main");

    std::fs::write(dir.join("bin.dat"), [9u8, 9, 9, 0, 7]).expect("write mine");

    let res = checkout_branch_autostash(dir, "feat").expect("switch");
    let apply = res.apply.expect("stashed => apply outcome");
    assert_eq!(
        unrestored(&apply),
        ["bin.dat".to_string()],
        "the unrestorable binary must be named: {apply:?}"
    );
    assert_eq!(
        list_stashes(dir).expect("list").len(),
        1,
        "the stash is the ONLY copy of the user's bytes — it must be retained"
    );
}

/// A brand-new file sitting where the target branch has a DIRECTORY. The file
/// is necessarily removed to make room; the stash must survive.
#[test]
fn untracked_file_vs_target_directory_retains_stash() {
    let td = tempfile::tempdir().expect("tempdir");
    let dir = td.path();
    let repo = u_init(dir);
    u_commit(dir, "base", &[("base.txt", "base\n")]);
    u_branch(&repo, "feat");
    checkout_branch(dir, "feat").expect("co feat");
    std::fs::create_dir_all(dir.join("thing")).expect("mkdir");
    u_commit(dir, "feat dir", &[("thing/a.txt", "a\n")]);
    checkout_branch(dir, "main").expect("co main");

    std::fs::write(dir.join("thing"), "my new file\n").expect("write");

    let res = checkout_branch_autostash(dir, "feat").expect("switch");
    let apply = res.apply.expect("stashed => apply outcome");
    assert_eq!(unrestored(&apply), ["thing".to_string()], "{apply:?}");
    assert_eq!(
        list_stashes(dir).expect("list").len(),
        1,
        "stash retained — the new file exists nowhere else"
    );
}

/// A brand-new TEXT file colliding with the target's tracked file: libgit2
/// writes conflict markers but records no index conflict. The content is not
/// lost, but it is not what the user wrote either — retain the stash and say so.
#[test]
fn text_untracked_collision_retains_stash() {
    let td = tempfile::tempdir().expect("tempdir");
    let dir = td.path();
    let repo = u_init(dir);
    u_commit(dir, "base", &[("base.txt", "base\n")]);
    u_branch(&repo, "feat");
    checkout_branch(dir, "feat").expect("co feat");
    u_commit(dir, "feat adds new.txt", &[("new.txt", "from-feat\n")]);
    checkout_branch(dir, "main").expect("co main");

    std::fs::write(dir.join("new.txt"), "my-work\n").expect("write");

    let res = checkout_branch_autostash(dir, "feat").expect("switch");
    let apply = res.apply.expect("stashed => apply outcome");
    assert_eq!(unrestored(&apply), ["new.txt".to_string()], "{apply:?}");
    assert_eq!(list_stashes(dir).expect("list").len(), 1);
}

/// The happy path must stay a clean drop: brand-new files that restore
/// byte-identically leave NO stash behind.
#[test]
fn clean_untracked_carry_still_drops_stash() {
    let td = tempfile::tempdir().expect("tempdir");
    let dir = td.path();
    let repo = u_init(dir);
    u_commit(dir, "base", &[("base.txt", "base\n")]);
    u_branch(&repo, "feat");

    std::fs::write(dir.join("added.txt"), "staged new\n").expect("write");
    crate::git::stage::stage_paths(dir, &["added.txt".to_string()]).expect("stage");
    std::fs::create_dir_all(dir.join("src/deep")).expect("mkdir");
    std::fs::write(dir.join("src/deep/n.txt"), "nested new\n").expect("write");
    std::fs::write(dir.join("base.txt"), "modified\n").expect("write");

    let res = checkout_branch_autostash(dir, "feat").expect("switch");
    assert_eq!(res.apply, Some(ApplyStashOutcome::Applied));
    assert_eq!(list_stashes(dir).expect("list").len(), 0);
    assert_eq!(
        std::fs::read_to_string(dir.join("src/deep/n.txt")).expect("nested"),
        "nested new\n"
    );
    assert!(dir.join("added.txt").exists());
    assert_eq!(
        std::fs::read_to_string(dir.join("base.txt")).expect("base"),
        "modified\n"
    );
}

/// An untracked file identical to the target's tracked version restores
/// byte-identically => clean drop, no false "partial" alarm.
#[test]
fn untracked_identical_to_target_drops_stash() {
    let td = tempfile::tempdir().expect("tempdir");
    let dir = td.path();
    let repo = u_init(dir);
    u_commit(dir, "base", &[("base.txt", "base\n")]);
    u_branch(&repo, "feat");
    checkout_branch(dir, "feat").expect("co feat");
    u_commit(dir, "feat adds", &[("new.txt", "same\n")]);
    checkout_branch(dir, "main").expect("co main");
    std::fs::write(dir.join("new.txt"), "same\n").expect("write");

    let res = checkout_branch_autostash(dir, "feat").expect("switch");
    assert_eq!(res.apply, Some(ApplyStashOutcome::Applied));
    assert_eq!(list_stashes(dir).expect("list").len(), 0);
}

/// A conflicting TRACKED modification retains the stash (pre-existing
/// behaviour) AND still restores the unrelated brand-new files to disk.
#[test]
fn conflicting_tracked_pop_keeps_untracked_on_disk() {
    let td = tempfile::tempdir().expect("tempdir");
    let dir = td.path();
    let repo = u_init(dir);
    u_commit(dir, "base", &[("f.txt", "one\n")]);
    u_branch(&repo, "feat");
    checkout_branch(dir, "feat").expect("co feat");
    u_commit(dir, "feat edits", &[("f.txt", "feat\n")]);
    checkout_branch(dir, "main").expect("co main");

    std::fs::write(dir.join("f.txt"), "mine\n").expect("write");
    std::fs::create_dir_all(dir.join("src/deep")).expect("mkdir");
    std::fs::write(dir.join("src/deep/n.txt"), "nested new\n").expect("write");

    let res = checkout_branch_autostash(dir, "feat").expect("switch");
    assert!(matches!(res.apply, Some(ApplyStashOutcome::Conflicts { .. })));
    assert_eq!(list_stashes(dir).expect("list").len(), 1);
    assert!(dir.join("src/deep/n.txt").exists());
}
