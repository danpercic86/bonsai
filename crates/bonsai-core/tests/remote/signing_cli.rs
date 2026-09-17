//! CLI-oracle integration tests for commit signing + verification (P58).
//!
//! Extracted from the inline `signing.rs` `#[cfg(test)]` module (P58b: keeps
//! that file under the ~500-line limit) and extended with the P58b verify
//! oracle. Two tiers:
//!   * PURE (always run): `resolve_signing` config resolution + camelCase wire
//!     shapes (guard the TS mirror) + the wholesale-failure degrade (fake exec).
//!   * ORACLE (guarded by `have_git()` / `have_ssh_keygen()`): lives in
//!     `signing_oracle_cli.rs`, split out to keep each file under the
//!     ~500-line limit.

use std::path::Path;

use crate::common;
use bonsai_core::error::AppError;
use bonsai_core::git::exec::{GitExec, GitOutput};
use bonsai_core::git::signing::{
    resolve_signing, verify_commits, CommitVerification, SignFormat, SigningStatus, VerifyResults,
    VerifyStatus,
};

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
    assert!(
        !resolve_signing(&on, Some(false)).sign,
        "Some(false) overrides true"
    );
    let (_d, off) = isolated_config(&[("commit.gpgsign", "false")]);
    assert!(
        resolve_signing(&off, Some(true)).sign,
        "Some(true) overrides false"
    );
}

#[test]
fn resolve_signing_format_and_key() {
    let (_d, cfg) = isolated_config(&[
        ("gpg.format", "ssh"),
        ("user.signingkey", "  /keys/id_ed25519  "),
    ]);
    let r = resolve_signing(&cfg, None);
    assert_eq!(r.format, SignFormat::Ssh);
    assert_eq!(r.key.as_deref(), Some("/keys/id_ed25519"), "trimmed");

    let (_d, cfg) = isolated_config(&[("user.signingkey", "   ")]);
    let r = resolve_signing(&cfg, None);
    assert_eq!(
        r.format,
        SignFormat::Openpgp,
        "unset gpg.format ⇒ openpgp default"
    );
    assert_eq!(r.key, None, "whitespace key ⇒ None");
}

// ---- pure: camelCase wire shapes (guard the TS mirror in ipc/types.ts) -------
#[test]
fn wire_shapes_match_ts_mirror() {
    // SignFormat — lowercase.
    assert_eq!(serde_json::to_value(SignFormat::Ssh).unwrap(), "ssh");
    assert_eq!(
        serde_json::to_value(SignFormat::Openpgp).unwrap(),
        "openpgp"
    );

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
    assert_eq!(
        serde_json::to_value(VerifyStatus::GoodUnknown).unwrap(),
        "goodUnknown"
    );
    assert_eq!(
        serde_json::to_value(VerifyStatus::ExpiredKey).unwrap(),
        "expiredKey"
    );
    assert_eq!(
        serde_json::to_value(VerifyStatus::CannotCheck).unwrap(),
        "cannotCheck"
    );
    assert_eq!(
        serde_json::to_value(VerifyStatus::Unsigned).unwrap(),
        "unsigned"
    );

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
    let vr = serde_json::to_value(VerifyResults {
        verifications: vec![],
    })
    .unwrap();
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
    assert!(r
        .verifications
        .iter()
        .all(|v| v.status == VerifyStatus::CannotCheck));
    assert!(r
        .verifications
        .iter()
        .all(|v| v.signer.is_none() && v.key.is_none()));
    assert_eq!(
        r.verifications[0].oid,
        "a".repeat(40),
        "order + oid preserved"
    );
}
