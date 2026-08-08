//! CLI-oracle integration tests for commit signing + verification (P58).
//!
//! Extracted from the inline `signing.rs` `#[cfg(test)]` module (P58b: keeps
//! that file under the ~500-line limit) and extended with the P58b verify
//! oracle. Two tiers:
//!   * PURE (always run): `resolve_signing` config resolution + camelCase wire
//!     shapes (guard the TS mirror) + the wholesale-failure degrade (fake exec).
//!   * ORACLE (guarded by `have_git()` / `have_ssh_keygen()`): drives the real
//!     `git` binary against a scratch repo — SSH signing is hermetic (an
//!     ephemeral ed25519 key with an EMPTY passphrase needs no agent). GPG is
//!     the USER CHECKPOINT. `TMP`/`TEMP=D:\Temp` via `common::scratch_dir`.

mod common;

use std::path::{Path, PathBuf};
use std::process::Command;

use bonsai_core::error::AppError;
use bonsai_core::git::commit::{amend_commit, create_commit};
use bonsai_core::git::exec::{GitExec, GitOutput, SpawnGitExec};
use bonsai_core::git::signing::{
    resolve_signing, signing_status, verify_commits, CommitVerification, SignFormat, SigningStatus,
    VerifyResults, VerifyStatus,
};
use bonsai_core::git::stage::stage_paths;
use common::{git, git_ok, init_repo};

macro_rules! require_git {
    () => {
        if !common::have_git() {
            eprintln!("skipping: `git` CLI not found on PATH");
            return;
        }
    };
}
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

// ---- pure: resolve_signing (config resolution) -------------------------------
fn isolated_config(entries: &[(&str, &str)]) -> (tempfile::TempDir, git2::Config) {
    let dir = common::scratch_dir();
    let file = dir.path().join("gitconfig");
    std::fs::write(&file, "").expect("config file");
    let mut cfg = git2::Config::open(&file).expect("open config");
    for (k, v) in entries {
        cfg.set_str(k, v).expect("set entry");
    }
    (dir, cfg)
}

#[test]
fn resolve_signing_none_follows_gpgsign_and_override_wins() {
    let (_d, off) = isolated_config(&[]);
    assert!(!resolve_signing(&off, None).sign, "unset gpgsign ⇒ off");
    let (_d, on) = isolated_config(&[("commit.gpgsign", "true")]);
    assert!(resolve_signing(&on, None).sign, "gpgsign=true ⇒ on");
    assert!(!resolve_signing(&on, Some(false)).sign, "Some(false) overrides true");
    let (_d, off) = isolated_config(&[("commit.gpgsign", "false")]);
    assert!(resolve_signing(&off, Some(true)).sign, "Some(true) overrides false");
}

#[test]
fn resolve_signing_format_and_key() {
    let (_d, cfg) =
        isolated_config(&[("gpg.format", "ssh"), ("user.signingkey", "  /keys/id_ed25519  ")]);
    let r = resolve_signing(&cfg, None);
    assert_eq!(r.format, SignFormat::Ssh);
    assert_eq!(r.key.as_deref(), Some("/keys/id_ed25519"), "trimmed");

    let (_d, cfg) = isolated_config(&[("user.signingkey", "   ")]);
    let r = resolve_signing(&cfg, None);
    assert_eq!(r.format, SignFormat::Openpgp, "unset gpg.format ⇒ openpgp default");
    assert_eq!(r.key, None, "whitespace key ⇒ None");
}

// ---- pure: camelCase wire shapes (guard the TS mirror in ipc/types.ts) -------
#[test]
fn wire_shapes_match_ts_mirror() {
    // SignFormat — lowercase.
    assert_eq!(serde_json::to_value(SignFormat::Ssh).unwrap(), "ssh");
    assert_eq!(serde_json::to_value(SignFormat::Openpgp).unwrap(), "openpgp");

    // SigningStatus — camelCase; `key` omitted when None.
    let s = serde_json::to_value(SigningStatus {
        enabled: true,
        format: Some(SignFormat::Ssh),
        has_key: true,
        key: Some("/k".to_string()),
    })
    .unwrap();
    assert_eq!(s["enabled"], true);
    assert_eq!(s["format"], "ssh");
    assert_eq!(s["hasKey"], true);
    assert_eq!(s["key"], "/k");
    let none = serde_json::to_value(SigningStatus {
        enabled: false,
        format: None,
        has_key: false,
        key: None,
    })
    .unwrap();
    assert!(none.get("key").is_none(), "key omitted when None");
    assert_eq!(none["format"], serde_json::Value::Null);

    // VerifyStatus — camelCase.
    assert_eq!(serde_json::to_value(VerifyStatus::Good).unwrap(), "good");
    assert_eq!(serde_json::to_value(VerifyStatus::GoodUnknown).unwrap(), "goodUnknown");
    assert_eq!(serde_json::to_value(VerifyStatus::ExpiredKey).unwrap(), "expiredKey");
    assert_eq!(serde_json::to_value(VerifyStatus::CannotCheck).unwrap(), "cannotCheck");
    assert_eq!(serde_json::to_value(VerifyStatus::Unsigned).unwrap(), "unsigned");

    // CommitVerification — camelCase; signer/key omitted when None.
    let cv = serde_json::to_value(CommitVerification {
        oid: "abc".to_string(),
        status: VerifyStatus::Good,
        signer: None,
        key: None,
    })
    .unwrap();
    assert_eq!(cv["oid"], "abc");
    assert_eq!(cv["status"], "good");
    assert!(cv.get("signer").is_none() && cv.get("key").is_none());
    let cv2 = serde_json::to_value(CommitVerification {
        oid: "abc".to_string(),
        status: VerifyStatus::GoodUnknown,
        signer: Some("Ada".to_string()),
        key: Some("KEY".to_string()),
    })
    .unwrap();
    assert_eq!(cv2["signer"], "Ada");
    assert_eq!(cv2["key"], "KEY");

    // VerifyResults wraps `verifications`.
    let vr = serde_json::to_value(VerifyResults { verifications: vec![] }).unwrap();
    assert!(vr["verifications"].is_array());
}

// ---- pure: wholesale-failure degrade (fake exec, no git needed) --------------
#[test]
fn verify_commits_wholesale_failure_degrades_to_cannot_check() {
    struct FailExec;
    impl GitExec for FailExec {
        fn exec(
            &self,
            _args: &[&str],
            _cwd: &Path,
            _stdin: Option<&[u8]>,
            _env: &[(&str, &str)],
        ) -> Result<GitOutput, AppError> {
            Ok(GitOutput {
                success: false,
                code: Some(128),
                stdout: String::new(),
                stderr: "fatal: bad object".to_string(),
            })
        }
    }
    let oids = vec!["a".repeat(40), "b".repeat(40)];
    let r = verify_commits(&FailExec, Path::new("."), &oids).expect("degrade, never Err");
    assert_eq!(r.verifications.len(), 2);
    assert!(r.verifications.iter().all(|v| v.status == VerifyStatus::CannotCheck));
    assert!(r.verifications.iter().all(|v| v.signer.is_none() && v.key.is_none()));
    assert_eq!(r.verifications[0].oid, "a".repeat(40), "order + oid preserved");
}

// ---- oracle helpers ----------------------------------------------------------
fn cat(dir: &Path, oid: &str) -> String {
    git(dir, &["cat-file", "-p", oid])
}

fn stage_write(dir: &Path, name: &str, body: &str) {
    std::fs::write(dir.join(name), body).expect("write file");
    stage_paths(dir, &[name.to_string()]).expect("stage");
}

/// Generate an ephemeral ed25519 key (EMPTY passphrase ⇒ hermetic, no agent),
/// write an `allowed_signers` naming the committer email, and set the ssh
/// signing config. Forward-slash paths so git + ssh-keygen agree on Windows.
fn setup_ssh_signing(dir: &Path, gpgsign: bool) -> String {
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
    git(dir, &["config", "commit.gpgsign", if gpgsign { "true" } else { "false" }]);
    key
}

// ---- oracle: signing_status --------------------------------------------------
#[test]
fn signing_status_reads_config() {
    require_git!();
    let dir = init_repo();
    let d = dir.path();
    let s = signing_status(d).expect("status");
    assert!(!s.enabled);
    assert_eq!(s.format, None);
    assert!(!s.has_key);
    assert_eq!(s.key, None);

    git(d, &["config", "gpg.format", "ssh"]);
    git(d, &["config", "user.signingkey", "/keys/id"]);
    git(d, &["config", "commit.gpgsign", "true"]);
    let s = signing_status(d).expect("status");
    assert!(s.enabled);
    assert_eq!(s.format, Some(SignFormat::Ssh));
    assert!(s.has_key);
    assert_eq!(s.key.as_deref(), Some("/keys/id"));
}

// ---- oracle: SSH signing -----------------------------------------------------
#[test]
fn oracle_ssh_sign_creates_verifiable_commit() {
    require_git_ssh!();
    let dir = init_repo();
    let d = dir.path();
    // Unsigned base first (also gives an oid to move HEAD from).
    stage_write(d, "base.txt", "base\n");
    create_commit(d, "base", None).expect("base commit");
    let base = git(d, &["rev-parse", "HEAD"]);
    assert!(!cat(d, &base).contains("gpgsig"), "base must be unsigned");

    setup_ssh_signing(d, false);
    stage_write(d, "a.txt", "alpha\n");
    let res = create_commit(d, "signed subject", Some(true)).expect("signed commit");

    assert_eq!(git(d, &["rev-parse", "HEAD"]), res.oid, "HEAD moved to the signed commit");
    assert_ne!(res.oid, base);
    assert_eq!(res.branch.as_deref(), Some("main"));
    assert!(cat(d, &res.oid).contains("gpgsig"), "signed commit must carry a gpgsig header");
    assert!(git_ok(d, &["verify-commit", &res.oid]), "git verify-commit must pass");
    assert_eq!(git(d, &["log", "--format=%G?", "-1", &res.oid]), "G", "%G? must be Good");
    assert!(git(d, &["reflog", "-1"]).contains("commit:"), "reflog records the commit");
}

#[test]
fn oracle_ssh_amend_preserves_author_and_resigns() {
    require_git_ssh!();
    let dir = init_repo();
    let d = dir.path();
    stage_write(d, "a.txt", "one\n");
    create_commit(d, "orig subject", None).expect("commit");
    let orig_author = git(d, &["log", "--format=%an <%ae>", "-1"]);
    let orig_adate = git(d, &["log", "--format=%at", "-1"]);

    setup_ssh_signing(d, false);
    let res = amend_commit(d, "amended subject", Some(true)).expect("amend");
    assert_eq!(git(d, &["log", "--format=%an <%ae>", "-1"]), orig_author, "author preserved");
    assert_eq!(git(d, &["log", "--format=%at", "-1"]), orig_adate, "author date preserved");
    assert_eq!(git(d, &["log", "--format=%s", "-1"]), "amended subject");
    assert!(cat(d, &res.oid).contains("gpgsig"), "amend must re-sign");
}

#[test]
fn config_gates_decide_signing() {
    require_git_ssh!();
    let dir = init_repo();
    let d = dir.path();
    setup_ssh_signing(d, false); // gpg.format=ssh + key, commit.gpgsign=false

    // (a) sign=None + gpgsign=false ⇒ UNSIGNED (byte-identical: no gpgsig header).
    stage_write(d, "a.txt", "a\n");
    let a = create_commit(d, "a", None).expect("a").oid;
    assert!(!cat(d, &a).contains("gpgsig"), "None + gpgsign=false ⇒ unsigned");

    // (b) commit.gpgsign=true + sign=None ⇒ SIGNED.
    git(d, &["config", "commit.gpgsign", "true"]);
    stage_write(d, "b.txt", "b\n");
    let b = create_commit(d, "b", None).expect("b").oid;
    assert!(cat(d, &b).contains("gpgsig"), "gpgsign=true ⇒ signed");

    // (c) sign=Some(false) overrides gpgsign=true ⇒ UNSIGNED.
    stage_write(d, "c.txt", "c\n");
    let c = create_commit(d, "c", Some(false)).expect("c").oid;
    assert!(!cat(d, &c).contains("gpgsig"), "Some(false) overrides ⇒ unsigned");
}

#[test]
fn ssh_signing_without_key_is_config_missing() {
    require_git!(); // ssh-keygen not needed — fails before any signer runs
    let dir = init_repo();
    let d = dir.path();
    stage_write(d, "base.txt", "base\n");
    create_commit(d, "base", None).expect("base");
    let base = git(d, &["rev-parse", "HEAD"]);
    git(d, &["config", "gpg.format", "ssh"]); // ssh format, NO user.signingkey

    stage_write(d, "a.txt", "a\n");
    let err = create_commit(d, "signed", Some(true)).expect_err("must be ConfigMissing");
    match err {
        AppError::ConfigMissing(m) => assert!(m.contains("user.signingkey"), "names the key: {m}"),
        other => panic!("expected ConfigMissing, got {other:?}"),
    }
    assert_eq!(git(d, &["rev-parse", "HEAD"]), base, "no commit created");
}

// ---- oracle: verify_commits --------------------------------------------------
fn find<'a>(res: &'a VerifyResults, oid: &str) -> &'a CommitVerification {
    res.verifications
        .iter()
        .find(|v| v.oid == oid)
        .unwrap_or_else(|| panic!("oid {oid} missing from results"))
}

#[test]
fn oracle_verify_signed_and_unsigned() {
    require_git_ssh!();
    let dir = init_repo();
    let d = dir.path();
    stage_write(d, "base.txt", "base\n");
    create_commit(d, "base", None).expect("base");
    let base = git(d, &["rev-parse", "HEAD"]);

    setup_ssh_signing(d, false); // allowed_signers names the committer ⇒ trusted
    stage_write(d, "a.txt", "alpha\n");
    let signed = create_commit(d, "signed", Some(true)).expect("signed").oid;

    let res = verify_commits(&SpawnGitExec, d, &[base.clone(), signed.clone()]).expect("verify");
    assert_eq!(res.verifications.len(), 2, "both oids resolvable");
    let sv = find(&res, &signed);
    assert!(
        matches!(sv.status, VerifyStatus::Good | VerifyStatus::GoodUnknown),
        "signed ⇒ Good/GoodUnknown, got {:?}",
        sv.status
    );
    assert!(sv.signer.is_some(), "a signed commit carries a signer");
    let bv = find(&res, &base);
    assert_eq!(bv.status, VerifyStatus::Unsigned);
    assert!(bv.signer.is_none() && bv.key.is_none());
}

#[test]
fn oracle_verify_trust_unavailable_never_errs() {
    require_git_ssh!();
    let dir = init_repo();
    let d = dir.path();
    setup_ssh_signing(d, false);
    stage_write(d, "a.txt", "a\n");
    let signed = create_commit(d, "signed", Some(true)).expect("signed").oid;
    // Drop the allowed-signers file ⇒ git cannot establish trust for the SSH
    // signature. The invariant under test: verify_commits still returns Ok (never
    // hard-fails) with a non-`good` verdict — the exact `%G?` is git's call
    // (`N`/`U`/`E` across versions; SSH without an allowed-signers file reports
    // `N`, so we mirror git faithfully rather than assert a single code).
    git(d, &["config", "--unset", "gpg.ssh.allowedSignersFile"]);

    let res = verify_commits(&SpawnGitExec, d, std::slice::from_ref(&signed)).expect("never Err");
    assert_eq!(res.verifications.len(), 1);
    let v = &res.verifications[0];
    assert_ne!(v.status, VerifyStatus::Good, "trust cannot be established ⇒ not Good");
    assert!(
        matches!(
            v.status,
            VerifyStatus::GoodUnknown | VerifyStatus::CannotCheck | VerifyStatus::Unsigned
        ),
        "got {:?}",
        v.status
    );
}

#[test]
fn oracle_verify_bogus_and_empty_omitted() {
    require_git!();
    let dir = init_repo();
    let d = dir.path();
    stage_write(d, "a.txt", "a\n");
    let real = create_commit(d, "a", None).expect("a").oid;

    // Non-hex "oids" are dropped before spawning ⇒ omitted; only `real` resolves.
    let oids = vec![real.clone(), "not-a-real-oid".to_string(), "#".to_string()];
    let res = verify_commits(&SpawnGitExec, d, &oids).expect("verify");
    assert_eq!(res.verifications.len(), 1, "bogus non-hex oids omitted");
    assert_eq!(res.verifications[0].oid, real);
    assert_eq!(res.verifications[0].status, VerifyStatus::Unsigned);

    // Empty request ⇒ empty result (no spawn path also covered by the unit test).
    assert!(verify_commits(&SpawnGitExec, d, &[]).expect("empty").verifications.is_empty());
}
