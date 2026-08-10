//! T2 Area 7 — tag HARDENING extensions (split from `tags_cli.rs`).
//!
//! unicode/slash names (+push), tag-of-tag peeling, tags on blob/tree, short-oid
//! rejection, multi-MB annotated message, index-lock independence, and F-A7-8
//! annotated-tag signing (`tag.gpgSign` / explicit `sign` honoured via
//! `git tag -s`; lightweight tags never signed; missing-key ⇒ `ConfigMissing`).
//! Scratch on D:. Skips (passes with a note) w/o `git` / `ssh-keygen`.

mod common;

use std::path::{Path, PathBuf};
use std::process::Command;

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

/// SSH signing is hermetic (an ephemeral ed25519 key with an EMPTY passphrase
/// needs no agent); GPG signing is a USER CHECKPOINT. Skip cleanly when either
/// `git` or `ssh-keygen` is absent (mirrors `signing_cli.rs`).
macro_rules! require_git_ssh {
    () => {
        if !common::have_git() || !have_ssh_keygen() {
            eprintln!("skipping: `git` / `ssh-keygen` not found on PATH");
            return;
        }
    };
}

fn have_ssh_keygen() -> bool {
    Command::new("ssh-keygen").arg("-A").output().is_ok()
        || Command::new("ssh-keygen").arg("--help").output().is_ok()
}

/// Generate an ephemeral ed25519 key (EMPTY passphrase ⇒ hermetic, no agent),
/// write an `allowed_signers` naming the committer email (`test@example.com`, set
/// by `init_repo`), and configure ssh signing. `gpgsign` seeds `tag.gpgSign`.
/// Forward-slash paths so git + ssh-keygen agree on Windows (mirrors
/// `signing_cli.rs::setup_ssh_signing`).
fn setup_ssh_tag_signing(dir: &Path, tag_gpgsign: bool) {
    let fwd = |p: PathBuf| p.to_string_lossy().replace('\\', "/");
    let key = fwd(dir.join("id_ed25519"));
    let out = Command::new("ssh-keygen")
        .args(["-t", "ed25519", "-N", "", "-C", "test@example.com", "-f", &key, "-q"])
        .output()
        .expect("ssh-keygen");
    assert!(out.status.success(), "keygen: {}", String::from_utf8_lossy(&out.stderr));

    let pubtext = std::fs::read_to_string(dir.join("id_ed25519.pub")).expect("pub key");
    let mut it = pubtext.split_whitespace();
    let ktype = it.next().unwrap_or_default();
    let kdata = it.next().unwrap_or_default();
    let signers = dir.join("allowed_signers");
    std::fs::write(&signers, format!("test@example.com {ktype} {kdata}\n")).expect("signers");

    git(dir, &["config", "gpg.format", "ssh"]);
    git(dir, &["config", "user.signingkey", &key]);
    git(dir, &["config", "gpg.ssh.allowedSignersFile", &fwd(signers)]);
    git(dir, &["config", "tag.gpgSign", if tag_gpgsign { "true" } else { "false" }]);
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
        create_tag(path, name, &head, None, false, false).unwrap_or_else(|e| panic!("create {name}: {e:?}"));
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

    create_tag(path, "inner", &head, Some("inner".into()), false, false).expect("inner");
    let inner_obj = git(path, &["rev-parse", "refs/tags/inner"]); // the tag OBJECT oid
    assert_eq!(git(path, &["cat-file", "-t", &inner_obj]), "tag", "inner is a tag object");

    create_tag(path, "outer", &inner_obj, Some("outer".into()), false, false).expect("outer");
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

    create_tag(path, "blobtag", &blob, None, false, false).expect("tag a blob");
    assert_eq!(git(path, &["cat-file", "-t", "refs/tags/blobtag"]), "blob");

    create_tag(path, "treetag", &tree, None, false, false).expect("tag a tree");
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
    match create_tag(path, "shorty", short, None, false, false) {
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
    create_tag(path, "big", &head, Some(msg.clone()), false, false).expect("create big-message tag");

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

    create_tag(path, "locktag", &head, Some("m".into()), false, false).expect("create despite index.lock");
    assert!(lock.exists(), "create must not remove the index lock");
    delete_tag(path, "locktag").expect("delete despite index.lock");
    assert!(lock.exists(), "delete must not remove the index lock");

    std::fs::remove_file(&lock).ok();
}

// --------------------------------------------- F-A7-8 annotated-tag signing

/// `tag.gpgSign=true` now SIGNS an annotated tag (git parity with `git tag -a`),
/// even when the explicit `sign` flag is false. The signed object carries an SSH
/// signature block and `git tag -v` accepts it (allowed_signers names the tagger).
#[test]
fn tag_gpgsign_true_signs_annotated() {
    require_git_ssh!();
    let (dir, head) = repo_one_commit();
    let path = dir.path();
    setup_ssh_tag_signing(path, /* tag_gpgsign = */ true);

    create_tag(path, "signed", &head, Some("release".into()), false, false)
        .expect("annotated tag signed via tag.gpgSign=true");

    let body = git(path, &["cat-file", "-p", "refs/tags/signed"]);
    assert!(
        body.contains("-----BEGIN SSH SIGNATURE-----"),
        "annotated tag must carry an SSH signature block: {body:?}"
    );
    assert!(git_ok(path, &["tag", "-v", "signed"]), "git tag -v must accept the signed tag");
}

/// The explicit `sign=true` flag signs an annotated tag even when `tag.gpgSign`
/// is unset/false (per-operation opt-in, mirrors the commit `sign` override).
#[test]
fn tag_sign_flag_signs_without_gpgsign() {
    require_git_ssh!();
    let (dir, head) = repo_one_commit();
    let path = dir.path();
    setup_ssh_tag_signing(path, /* tag_gpgsign = */ false);

    create_tag(path, "flagsigned", &head, Some("release".into()), false, true)
        .expect("annotated tag signed via explicit sign flag");

    let body = git(path, &["cat-file", "-p", "refs/tags/flagsigned"]);
    assert!(
        body.contains("-----BEGIN SSH SIGNATURE-----"),
        "explicit sign flag must sign despite tag.gpgSign=false: {body:?}"
    );
    assert!(git_ok(path, &["tag", "-v", "flagsigned"]), "git tag -v must accept the signed tag");
}

/// Signing requested with `gpg.format=ssh` and NO `user.signingkey` fails with a
/// clean `ConfigMissing` naming the key BEFORE any ref is written (mirrors commit
/// signing) — never a silently-unsigned tag. `ssh-keygen` is not needed (fails
/// before any signer runs), so this only requires `git`.
#[test]
fn tag_sign_without_key_is_config_missing() {
    require_git!();
    let (dir, head) = repo_one_commit();
    let path = dir.path();
    git(path, &["config", "gpg.format", "ssh"]); // ssh format, NO user.signingkey

    let err = create_tag(path, "nokey", &head, Some("m".into()), false, true)
        .expect_err("must be ConfigMissing");
    match err {
        AppError::ConfigMissing(m) => {
            assert!(m.contains("user.signingkey"), "names the key: {m}")
        }
        other => panic!("expected ConfigMissing, got {other:?}"),
    }
    assert!(
        !git_ok(path, &["rev-parse", "--verify", "refs/tags/nokey"]),
        "no tag ref must be created on a signing failure"
    );
}

/// LIGHTWEIGHT tags are NEVER signed (git parity), even with `tag.gpgSign=true`
/// AND an explicit `sign=true` — a lightweight tag is a bare ref to the commit,
/// not a tag object.
#[test]
fn lightweight_tag_never_signed() {
    require_git_ssh!();
    let (dir, head) = repo_one_commit();
    let path = dir.path();
    setup_ssh_tag_signing(path, /* tag_gpgsign = */ true);

    create_tag(path, "light", &head, None, false, true).expect("lightweight tag despite sign");

    // The ref points straight at the commit — no intervening tag object.
    let kind = git(path, &["cat-file", "-t", "refs/tags/light"]);
    assert_eq!(kind, "commit", "lightweight tag resolves to the commit, not a tag object");
    assert_eq!(git(path, &["rev-parse", "refs/tags/light"]), head, "ref == commit oid");
}

/// The pre-existing UNSIGNED annotated-tag path is unchanged: with `tag.gpgSign`
/// off and `sign=false`, the git2 tag object carries no signature block.
#[test]
fn unsigned_annotated_tag_unchanged() {
    require_git!();
    let (dir, head) = repo_one_commit();
    let path = dir.path();

    create_tag(path, "plain", &head, Some("release".into()), false, false)
        .expect("unsigned annotated tag");

    let body = git(path, &["cat-file", "-p", "refs/tags/plain"]);
    assert!(!body.contains("-----BEGIN SSH SIGNATURE-----"), "must be unsigned: {body:?}");
    assert!(!body.contains("-----BEGIN PGP SIGNATURE-----"), "must be unsigned: {body:?}");
    let kind = git(path, &["cat-file", "-t", "refs/tags/plain"]);
    assert_eq!(kind, "tag", "annotated ⇒ a real tag object");
}
