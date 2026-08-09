//! T2 Area 7 — tag HARDENING extensions (split from `tags_cli.rs`).
//!
//! unicode/slash names (+push), tag-of-tag peeling, tags on blob/tree, short-oid
//! rejection, multi-MB annotated message, index-lock independence, and the
//! F-A7-8 `tag.gpgSign` divergence (documented v1 limitation: annotated tags are
//! never signed). Scratch on D:. Skips (passes with a note) w/o `git`.

mod common;

use std::path::Path;

use bonsai_core::error::AppError;
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

/// Repo with one commit on `main`; returns (dir, head_oid).
fn repo_one_commit() -> (tempfile::TempDir, String) {
    let dir = init_repo();
    let p = dir.path();
    std::fs::write(p.join("a.txt"), "one\n").unwrap();
    git(p, &["add", "-A"]);
    commit_fixed(p, "first");
    let head = git(p, &["rev-parse", "HEAD"]);
    (dir, head)
}

// ------------------------------------------------- unicode + slash names

/// Unicode and slash-bearing tag names create valid refs and push to a bare
/// file:// remote.
#[test]
fn unicode_and_slash_tag_names_create_and_push() {
    require_git!();
    let (dir, head) = repo_one_commit();
    let path = dir.path();

    for name in ["release/v1", "v1-München", "级别/日本語"] {
        create_tag(path, name, &head, None, false).unwrap_or_else(|e| panic!("create {name}: {e:?}"));
        assert!(git_ok(path, &["rev-parse", "--verify", &format!("refs/tags/{name}")]),
            "ref refs/tags/{name} exists");
    }

    // Push the slash-name tag to a local bare remote.
    let bare = path.parent().unwrap().join(format!(
        "{}-tags.git", path.file_name().unwrap().to_string_lossy()));
    git(path.parent().unwrap(), &["init", "--bare", "-b", "main", &path_str(&bare)]);
    git(path, &["remote", "add", "origin", &path_str(&bare)]);
    git(path, &["push", "origin", "main"]);
    push_tag(path, "origin", "release/v1", false).expect("push slash tag");
    assert!(git_ok(&bare, &["show-ref", "--verify", "refs/tags/release/v1"]),
        "slash tag present on the remote");
    std::fs::remove_dir_all(&bare).ok();
}

// --------------------------------------------------------- tag-of-tag peel

/// An annotated tag pointing at ANOTHER annotated tag object peels through to
/// the underlying commit.
#[test]
fn annotated_tag_of_tag_peels_to_commit() {
    require_git!();
    let (dir, head) = repo_one_commit();
    let path = dir.path();

    create_tag(path, "inner", &head, Some("inner".into()), false).expect("inner");
    let inner_obj = git(path, &["rev-parse", "refs/tags/inner"]); // the tag OBJECT oid
    assert_eq!(git(path, &["cat-file", "-t", &inner_obj]), "tag", "inner is a tag object");

    create_tag(path, "outer", &inner_obj, Some("outer".into()), false).expect("outer");
    assert_eq!(git(path, &["cat-file", "-t", "refs/tags/outer"]), "tag", "outer is a tag object");
    // Peels through both tags to the commit.
    assert_eq!(git(path, &["rev-parse", "refs/tags/outer^{commit}"]), head);
}

// ----------------------------------------------------- tag on blob / tree

/// A lightweight tag may point at a blob or a tree (git allows tagging any
/// object) — pin this documented behavior.
#[test]
fn lightweight_tag_on_blob_and_tree() {
    require_git!();
    let (dir, head) = repo_one_commit();
    let path = dir.path();

    let blob = git(path, &["rev-parse", "HEAD:a.txt"]);
    let tree = git(path, &["rev-parse", &format!("{head}^{{tree}}")]);

    create_tag(path, "blobtag", &blob, None, false).expect("tag a blob");
    assert_eq!(git(path, &["cat-file", "-t", "refs/tags/blobtag"]), "blob");

    create_tag(path, "treetag", &tree, None, false).expect("tag a tree");
    assert_eq!(git(path, &["cat-file", "-t", "refs/tags/treetag"]), "tree");
}

// ------------------------------------------------------- short-oid target

/// A short (abbreviated) oid is NOT a valid full target — `create_tag` requires
/// a 40-hex oid and rejects the abbreviation cleanly.
#[test]
fn short_oid_target_rejected() {
    require_git!();
    let (dir, head) = repo_one_commit();
    let path = dir.path();
    let short = &head[..7];
    match create_tag(path, "shorty", short, None, false) {
        Err(AppError::Git(m)) => assert!(m.contains("not a valid commit id") || m.contains("cannot"),
            "short oid rejected: {m}"),
        other => panic!("short oid must be a clean Git error, got {other:?}"),
    }
    assert!(!git_ok(path, &["rev-parse", "--verify", "refs/tags/shorty"]), "no tag created");
}

// ------------------------------------------------ multi-MB annotated message

/// A multi-MB annotated tag message round-trips into the tag object.
#[test]
fn multi_mb_annotated_message() {
    require_git!();
    let (dir, head) = repo_one_commit();
    let path = dir.path();

    let marker = "UNIQUE-END-MARKER";
    let mut msg = "x".repeat(2 * 1024 * 1024);
    msg.push('\n');
    msg.push_str(marker);
    create_tag(path, "big", &head, Some(msg.clone()), false).expect("create big-message tag");

    assert_eq!(git(path, &["cat-file", "-t", "refs/tags/big"]), "tag");
    let body = git(path, &["for-each-ref", "--format=%(contents)", "refs/tags/big"]);
    assert!(body.len() >= 2 * 1024 * 1024, "message preserved at size: {}", body.len());
    assert!(body.contains(marker), "message tail survives");
}

// ----------------------------------------------- index.lock independence

/// Tag creation/deletion do NOT touch the index, so an existing `.git/index.lock`
/// neither blocks them nor is disturbed by them.
#[test]
fn index_lock_does_not_affect_tags() {
    require_git!();
    let (dir, head) = repo_one_commit();
    let path = dir.path();
    let lock = path.join(".git").join("index.lock");
    std::fs::write(&lock, b"").expect("create lock");

    create_tag(path, "locktag", &head, Some("m".into()), false).expect("create despite index.lock");
    assert!(lock.exists(), "create must not remove the index lock");
    delete_tag(path, "locktag").expect("delete despite index.lock");
    assert!(lock.exists(), "delete must not remove the index lock");

    std::fs::remove_file(&lock).ok();
}

// --------------------------------------------- F-A7-8 tag.gpgSign divergence

/// DOCUMENTED v1 limitation (F-A7-8): `tag.gpgSign=true` is IGNORED — bonsai's
/// annotated tags are never signed. Pin the current behavior so a future change
/// is a deliberate, test-visible decision.
#[test]
fn tag_gpgsign_is_ignored_v1_limitation() {
    require_git!();
    let (dir, head) = repo_one_commit();
    let path = dir.path();
    git(path, &["config", "tag.gpgSign", "true"]);

    create_tag(path, "wouldsign", &head, Some("release".into()), false)
        .expect("annotated tag created (unsigned) despite tag.gpgSign=true");

    // The tag object carries NO PGP signature block.
    let body = git(path, &["cat-file", "-p", "refs/tags/wouldsign"]);
    assert!(!body.contains("-----BEGIN PGP SIGNATURE-----"),
        "annotated tag is unsigned (tag.gpgSign ignored — F-A7-8): {body:?}");
}
