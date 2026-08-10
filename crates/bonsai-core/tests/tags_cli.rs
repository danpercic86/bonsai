//! P22 CLI-oracle tag tests (contract §8.1).
//!
//! Every "remote" is a LOCAL BARE repo (`git init --bare`) referenced by a
//! plain path — the local transport needs NO network and NO credentials. All
//! scratch repos live under `D:\Temp\bonsai-scratch`.
//!
//! Each test skips (passes with a note) if `git` is not on PATH.

mod common;

use std::path::Path;

use bonsai_core::error::AppError;
use bonsai_core::git::branches::list_refs;
use bonsai_core::git::tags::{create_tag, delete_tag, push_tag};
use common::{commit_fixed, git, git_ok, init_repo};

macro_rules! require_git {
    () => {
        if !common::have_git() {
            eprintln!("skipping: `git` CLI not found on PATH");
            return;
        }
    };
}

fn path_str(p: &Path) -> String {
    p.to_string_lossy().into_owned()
}

/// Repo with two commits (C1, C2) on `main`; returns (dir, oidC1, oidC2).
fn repo_two_commits() -> (tempfile::TempDir, String, String) {
    let dir = init_repo();
    let path = dir.path();
    std::fs::write(path.join("a.txt"), "one\n").expect("write a.txt");
    git(path, &["add", "-A"]);
    commit_fixed(path, "first");
    let c1 = git(path, &["rev-parse", "HEAD"]);
    std::fs::write(path.join("b.txt"), "two\n").expect("write b.txt");
    git(path, &["add", "-A"]);
    commit_fixed(path, "second");
    let c2 = git(path, &["rev-parse", "HEAD"]);
    (dir, c1, c2)
}

// ---------------------------------------------------------- §8.1.1 lightweight

/// Lightweight tag points STRAIGHT at the commit (`cat-file -t` == `commit`);
/// equals `git tag <name> <oid>` on a twin.
#[test]
fn lightweight_tag_parity() {
    require_git!();
    let (dir, c1, _c2) = repo_two_commits();
    let path = dir.path();

    create_tag(path, "lw", &c1, None, false, false).expect("create lightweight tag");

    // Points straight at the commit — no tag object.
    assert_eq!(git(path, &["cat-file", "-t", "lw"]), "commit");
    assert_eq!(git(path, &["rev-parse", "refs/tags/lw"]), c1);

    // Twin oracle: `git tag lw2 <oid>` yields the same ref target.
    git(path, &["tag", "lw2", &c1]);
    assert_eq!(
        git(path, &["rev-parse", "refs/tags/lw"]),
        git(path, &["rev-parse", "refs/tags/lw2"]),
    );
}

// ----------------------------------------------------------- §8.1.2 annotated

/// Annotated tag creates a real tag OBJECT (`cat-file -t` == `tag`) whose
/// target/message match `git tag -a -m` on a twin.
#[test]
fn annotated_tag_parity() {
    require_git!();
    let (dir, c1, _c2) = repo_two_commits();
    let path = dir.path();

    create_tag(path, "ann", &c1, Some("release notes".to_string()), false, false)
        .expect("create annotated tag");

    // A real tag object, not a straight ref.
    assert_eq!(git(path, &["cat-file", "-t", "ann"]), "tag");
    // Peels to the commit it targets.
    assert_eq!(git(path, &["rev-parse", "ann^{commit}"]), c1);

    // Twin oracle via the CLI.
    git(
        path,
        &["tag", "-a", "ann2", "-m", "release notes", &c1],
    );

    // Same peeled target and same subject (git normalizes the message).
    let ours = git(
        path,
        &[
            "for-each-ref",
            "--format=%(*objectname) %(contents:subject)",
            "refs/tags/ann",
        ],
    );
    let twin = git(
        path,
        &[
            "for-each-ref",
            "--format=%(*objectname) %(contents:subject)",
            "refs/tags/ann2",
        ],
    );
    assert_eq!(ours, twin, "annotated tag object target+subject must match CLI");
    assert!(ours.starts_with(&c1), "peeled target must be C1: {ours}");
    assert!(ours.contains("release notes"), "subject must be present: {ours}");
}

/// Annotated tag needs a git identity; lightweight does not (§8.1.3).
#[test]
fn annotated_tag_needs_identity() {
    require_git!();
    let dir = common::scratch_dir();
    let path = dir.path();
    git(path, &["init", "-b", "main"]);
    git(path, &["config", "core.autocrlf", "false"]);
    // Force the repo-local identity to EMPTY (local overrides any global
    // user.* on the dev machine); resolve_signature treats blank as missing.
    git(path, &["config", "user.name", ""]);
    git(path, &["config", "user.email", ""]);

    // Commit via env vars so a first commit exists without needing config.
    std::fs::write(path.join("a.txt"), "one\n").expect("write a.txt");
    git(path, &["add", "-A"]);
    common::git_env(
        path,
        &["commit", "-m", "first"],
        &[
            ("GIT_AUTHOR_NAME", "Env User"),
            ("GIT_AUTHOR_EMAIL", "env@example.com"),
            ("GIT_COMMITTER_NAME", "Env User"),
            ("GIT_COMMITTER_EMAIL", "env@example.com"),
            ("GIT_AUTHOR_DATE", common::FIXED_DATE),
            ("GIT_COMMITTER_DATE", common::FIXED_DATE),
        ],
    );
    let c1 = git(path, &["rev-parse", "HEAD"]);

    // Annotated fails with ConfigMissing naming user.email/user.name.
    let err = create_tag(path, "ann", &c1, Some("m".to_string()), false, false)
        .expect_err("annotated must fail without identity");
    match err {
        AppError::ConfigMissing(m) => {
            assert!(
                m.contains("user.email") || m.contains("user.name"),
                "must name a missing identity key: {m}"
            );
        }
        other => panic!("expected ConfigMissing, got {other:?}"),
    }

    // Lightweight still succeeds.
    create_tag(path, "lw", &c1, None, false, false).expect("lightweight tag without identity");
    assert_eq!(git(path, &["cat-file", "-t", "lw"]), "commit");
}

// ------------------------------------------------------------ §8.1.4 duplicate

/// Duplicate create (no force) errors; `force=true` moves the tag.
#[test]
fn duplicate_and_force() {
    require_git!();
    let (dir, c1, c2) = repo_two_commits();
    let path = dir.path();

    create_tag(path, "v", &c1, None, false, false).expect("first create");

    let err = create_tag(path, "v", &c1, None, false, false).expect_err("duplicate must error");
    match err {
        AppError::Git(m) => assert!(m.contains("already exists"), "got: {m}"),
        other => panic!("expected Git already-exists, got {other:?}"),
    }

    // Force moves the tag to C2.
    create_tag(path, "v", &c2, None, true, false).expect("force overwrite");
    assert_eq!(git(path, &["rev-parse", "refs/tags/v"]), c2);
}

// ------------------------------------------------ §8.1.5 bad target / bad name

#[test]
fn bad_target_and_bad_name() {
    require_git!();
    let (dir, _c1, _c2) = repo_two_commits();
    let path = dir.path();

    // Unknown but well-formed 40-hex oid.
    let ghost = "0".repeat(40);
    match create_tag(path, "ghost", &ghost, None, false, false) {
        Err(AppError::Git(_)) => {}
        other => panic!("unknown oid must be Git error, got {other:?}"),
    }

    // Not even hex.
    match create_tag(path, "bad", "not-an-oid", None, false, false) {
        Err(AppError::Git(_)) => {}
        other => panic!("bad oid must be Git error, got {other:?}"),
    }

    // Bad names.
    let good = git(path, &["rev-parse", "HEAD"]);
    for bad in ["", "-x"] {
        match create_tag(path, bad, &good, None, false, false) {
            Err(AppError::InvalidName(_)) => {}
            other => panic!("name {bad:?} must be InvalidName, got {other:?}"),
        }
    }
}

// -------------------------------------------------------------- §8.1.6 delete

#[test]
fn delete_parity() {
    require_git!();
    let (dir, c1, _c2) = repo_two_commits();
    let path = dir.path();

    create_tag(path, "gone", &c1, None, false, false).expect("create");
    assert_eq!(git(path, &["tag", "-l", "gone"]), "gone");

    delete_tag(path, "gone").expect("delete");
    assert_eq!(git(path, &["tag", "-l", "gone"]), "", "tag ref must be gone");

    // Deleting a missing tag errors.
    match delete_tag(path, "gone") {
        Err(AppError::Git(m)) => assert!(m.contains("not found"), "got: {m}"),
        other => panic!("expected Git not-found, got {other:?}"),
    }

    // Blank name → InvalidName.
    match delete_tag(path, "") {
        Err(AppError::InvalidName(_)) => {}
        other => panic!("blank delete must be InvalidName, got {other:?}"),
    }
}

// --------------------------------------------------------------- §8.1.7 push

/// Push a tag to a LOCAL bare remote → the tag ref appears in the bare repo;
/// pushing a non-existent local tag errors.
#[test]
fn push_to_bare_remote() {
    require_git!();
    let (dir, c1, _c2) = repo_two_commits();
    let path = dir.path();

    // Bare remote alongside the work repo.
    let bare = dir.path().parent().expect("parent").join(format!(
        "{}-origin.git",
        dir.path().file_name().unwrap().to_string_lossy()
    ));
    git(dir.path().parent().unwrap(), &["init", "--bare", "-b", "main", &path_str(&bare)]);
    // Publish main so the remote is a real repo (not strictly required for tags).
    git(path, &["remote", "add", "origin", &path_str(&bare)]);
    git(path, &["push", "origin", "main"]);

    create_tag(path, "rel", &c1, Some("m".to_string()), false, false).expect("create annotated tag");

    push_tag(path, "origin", "rel", false).expect("push tag to bare");

    // The tag ref now exists in the bare repo.
    assert!(
        git_ok(&bare, &["show-ref", "--tags", "--verify", "refs/tags/rel"]),
        "bare remote must have refs/tags/rel"
    );
    assert_eq!(git(&bare, &["tag", "-l", "rel"]), "rel");

    // Twin oracle: `git push origin rel2` yields the same ref presence.
    create_tag(path, "rel2", &c1, None, false, false).expect("create lightweight");
    git(path, &["push", "origin", "rel2"]);
    assert!(git_ok(&bare, &["show-ref", "--verify", "refs/tags/rel2"]));

    // Pushing a non-existent local tag errors.
    match push_tag(path, "origin", "nope", false) {
        Err(AppError::Git(m)) => assert!(m.contains("not found"), "got: {m}"),
        other => panic!("expected Git not-found, got {other:?}"),
    }

    // Pushing to a non-existent remote → NoRemote.
    match push_tag(path, "nosuch", "rel", false) {
        Err(AppError::NoRemote(_)) => {}
        other => panic!("expected NoRemote, got {other:?}"),
    }

    std::fs::remove_dir_all(&bare).ok();
}

/// Coverage gap: pushing an ANNOTATED tag must transfer the tag OBJECT itself,
/// not just a lightweight ref — the bare remote's `cat-file -t` must read `tag`
/// (the existing push test only asserts ref presence). Peeled target + subject
/// must also survive the transfer.
#[test]
fn push_annotated_tag_transfers_object() {
    require_git!();
    let (dir, c1, _c2) = repo_two_commits();
    let path = dir.path();

    let bare = dir.path().parent().expect("parent").join(format!(
        "{}-annobj.git",
        dir.path().file_name().unwrap().to_string_lossy()
    ));
    git(dir.path().parent().unwrap(), &["init", "--bare", "-b", "main", &path_str(&bare)]);
    git(path, &["remote", "add", "origin", &path_str(&bare)]);
    git(path, &["push", "origin", "main"]);

    create_tag(path, "annrel", &c1, Some("annotated payload".to_string()), false, false)
        .expect("create annotated tag");

    push_tag(path, "origin", "annrel", false).expect("push annotated tag to bare");

    // The bare remote holds a real TAG OBJECT, not a straight commit ref.
    assert_eq!(
        git(&bare, &["cat-file", "-t", "refs/tags/annrel"]),
        "tag",
        "annotated tag must transfer as a tag object on the remote"
    );
    // It peels to C1 and carries the subject across the wire.
    assert_eq!(git(&bare, &["rev-parse", "refs/tags/annrel^{commit}"]), c1);
    let subj = git(
        &bare,
        &["for-each-ref", "--format=%(contents:subject)", "refs/tags/annrel"],
    );
    assert_eq!(subj, "annotated payload", "tag message must survive the push");

    std::fs::remove_dir_all(&bare).ok();
}

// ------------------------------------------------ §8.1.8 list_refs re-surfaces

/// `branches::list_refs().tags` reflects create then delete — proving §2.0
/// (no separate `list_tags` command needed).
#[test]
fn list_refs_resurfaces_tags() {
    require_git!();
    let (dir, c1, _c2) = repo_two_commits();
    let path = dir.path();

    create_tag(path, "surf", &c1, None, false, false).expect("create");
    let snap = list_refs(path).expect("list_refs after create");
    assert!(
        snap.tags.iter().any(|t| t == "surf"),
        "tags must include 'surf' after create: {:?}",
        snap.tags
    );

    delete_tag(path, "surf").expect("delete");
    let snap = list_refs(path).expect("list_refs after delete");
    assert!(
        !snap.tags.iter().any(|t| t == "surf"),
        "tags must omit 'surf' after delete: {:?}",
        snap.tags
    );
}
