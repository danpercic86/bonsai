//! Shared fixtures for the P8 §7 autostash-merge behavioral matrix
//! (`autostash_tests`). Scratch repos built with git2 (deterministic, no
//! network, no CLI), plus the optional real-`git` autostash oracle.

use std::path::Path;

/// Init a scratch repo with a deterministic identity + autocrlf off.
pub(super) fn p8_init(dir: &Path) -> git2::Repository {
    let repo = git2::Repository::init(dir).expect("init repo");
    let mut cfg = repo.config().expect("config");
    cfg.set_str("user.name", "Test User").expect("name");
    cfg.set_str("user.email", "test@example.com").expect("email");
    cfg.set_bool("core.autocrlf", false).expect("autocrlf");
    drop(cfg);
    repo
}

/// Stage + commit `files` on the CURRENT branch (moves HEAD + worktree).
pub(super) fn p8_commit(dir: &Path, msg: &str, files: &[(&str, &str)]) {
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

/// Build a commit on `refname` from `parent`'s tree with the given top-level
/// file additions/modifications, WITHOUT moving HEAD or touching the
/// worktree. Creates `refname` if absent. Used to advance a `topic` branch
/// (FF fixtures) or to build a divergent tip (non-FF fixtures).
pub(super) fn p8_commit_on_ref(
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

pub(super) fn p8_head_oid(repo: &git2::Repository) -> git2::Oid {
    repo.head()
        .expect("HEAD")
        .peel_to_commit()
        .expect("peel")
        .id()
}

/// Enumerate the stash stack via a FRESH handle (state is read from disk).
pub(super) fn p8_stash_count(dir: &Path) -> usize {
    let mut repo = git2::Repository::open(dir).expect("open");
    let mut n = 0usize;
    repo.stash_foreach(|_i, _msg, _oid| {
        n += 1;
        true
    })
    .expect("stash_foreach");
    n
}

pub(super) fn p8_read(dir: &Path, name: &str) -> String {
    std::fs::read_to_string(dir.join(name)).expect("read file")
}

/// Optional CLI oracle for row 2: build the identical fixture in a fresh
/// scratch repo, run real `git merge --autostash topic`, return the
/// resulting `feature.txt` + restored `unrelated.txt` worktree contents.
/// Returns None if `git` is not runnable so the test degrades to git2-only
/// assertions. Commit oids are intentionally NOT returned: they cannot match
/// across two independently-built repos (timestamp-dependent hashes).
pub(super) fn p8_git_cli_autostash_ff_oracle() -> Option<(String, String)> {
    use std::process::Command;
    let dir = crate::testutil::scratch_dir();
    let p = dir.path();
    let git = |args: &[&str]| -> Option<std::process::Output> {
        Command::new("git").current_dir(p).args(args).output().ok()
    };
    // Probe git availability first.
    let probe = git(&["--version"])?;
    if !probe.status.success() {
        return None;
    }
    let run = |args: &[&str]| -> bool {
        git(args).map(|o| o.status.success()).unwrap_or(false)
    };
    if !run(&["init", "-q"]) {
        return None;
    }
    run(&["config", "user.name", "Test User"]);
    run(&["config", "user.email", "test@example.com"]);
    run(&["config", "core.autocrlf", "false"]);
    std::fs::write(p.join("a.txt"), "base\n").ok()?;
    std::fs::write(p.join("unrelated.txt"), "orig\n").ok()?;
    run(&["add", "-A"]);
    run(&["commit", "-q", "-m", "base"]);
    // Capture the real default branch (master/main), don't assume.
    let default_branch = {
        let o = git(&["rev-parse", "--abbrev-ref", "HEAD"])?;
        String::from_utf8_lossy(&o.stdout).trim().to_string()
    };
    // topic = base + feature.txt, built without moving HEAD.
    run(&["branch", "topic"]);
    run(&["checkout", "-q", "topic"]);
    std::fs::write(p.join("feature.txt"), "feature\n").ok()?;
    run(&["add", "-A"]);
    run(&["commit", "-q", "-m", "topic advance"]);
    run(&["checkout", "-q", &default_branch]);
    // Dirty unstaged edit, then autostash FF.
    std::fs::write(p.join("unrelated.txt"), "locally edited\n").ok()?;
    if !run(&["merge", "--autostash", "--ff-only", "topic"]) {
        return None;
    }
    let feature = std::fs::read_to_string(p.join("feature.txt")).ok()?;
    let unrelated = std::fs::read_to_string(p.join("unrelated.txt")).ok()?;
    Some((feature, unrelated))
}
