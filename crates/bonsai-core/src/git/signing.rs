//! Commit signing (P58a).
//!
//! git2 has no "sign this commit" call, so signing follows mechanism **C** (P58
//! D1/OQ1): git2 assembles the tree + identity + guards (in `commit.rs` /
//! `merge.rs`), then the `git` binary produces the SIGNED object via
//! `git commit-tree -S` and moves HEAD via `git update-ref`. Both SSH and
//! OpenPGP are signed by git itself (respecting `gpg.format`, `user.signingkey`,
//! and the `gpg.program` / `gpg.ssh.program` overrides), so the unsigned path
//! stays 100% git2 and byte-identical to pre-P58.
//!
//! P58a covers signing + the read-only [`signing_status`] indicator. Signature
//! VERIFICATION (`verify_commits`) lands in P58b.

use std::path::Path;

use crate::error::AppError;
use crate::git::exec::GitExec;

/// `gpg.format` — how the commit is signed. Serializes lowercase (`ssh` /
/// `openpgp`) to match the TS `SignFormat` mirror.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SignFormat {
    Ssh,
    Openpgp,
}

/// Effective signing config for the commit-box indicator/toggle (P58 D6). Wire
/// shape: camelCase; `key` omitted when unset.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SigningStatus {
    /// Effective `commit.gpgsign` (git default false).
    pub enabled: bool,
    /// `gpg.format`; `None` when unset (git's own default is openpgp).
    pub format: Option<SignFormat>,
    /// `user.signingkey` set + non-empty (after trim).
    pub has_key: bool,
    /// `user.signingkey` for display (path or key id); omitted when unset.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
}

/// Internal (never on the wire): resolved once per commit by [`resolve_signing`].
#[derive(Debug, Clone, PartialEq)]
pub struct SigningConfig {
    pub sign: bool,
    pub format: SignFormat,
    pub key: Option<String>,
}

/// Resolve whether/how to sign. `override_sign`: `None` ⇒ follow effective
/// `commit.gpgsign` (git default false); `Some(b)` ⇒ `b` (per-commit toggle,
/// P58 D3). `format` from `gpg.format` (default [`SignFormat::Openpgp`]); `key`
/// from `user.signingkey` (trimmed, non-empty). Never fails — a missing key is
/// surfaced later by [`create_signed_commit`] (ssh) or left to git (openpgp).
pub fn resolve_signing(cfg: &git2::Config, override_sign: Option<bool>) -> SigningConfig {
    let sign = match override_sign {
        Some(b) => b,
        None => cfg.get_bool("commit.gpgsign").unwrap_or(false),
    };
    SigningConfig {
        sign,
        format: read_format(cfg).unwrap_or(SignFormat::Openpgp),
        key: read_key(cfg),
    }
}

/// `gpg.format` as a [`SignFormat`], or `None` when unset/empty. Anything that
/// isn't `ssh` (incl. `openpgp`, `x509`) maps to [`SignFormat::Openpgp`] — the
/// only format needing a hard key gate is ssh (OQ2).
fn read_format(cfg: &git2::Config) -> Option<SignFormat> {
    match cfg.get_string("gpg.format").ok().as_deref().map(str::trim) {
        None | Some("") => None,
        Some("ssh") => Some(SignFormat::Ssh),
        Some(_) => Some(SignFormat::Openpgp),
    }
}

/// `user.signingkey`, trimmed; `None` when unset or empty.
fn read_key(cfg: &git2::Config) -> Option<String> {
    cfg.get_string("user.signingkey")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// Read-only signing status for the UI (P58 D6). Opens the repo at `workdir`.
/// `format` is `None` when `gpg.format` is unset (distinct from the internal
/// `resolve_signing`, which collapses unset → openpgp).
pub fn signing_status(workdir: &Path) -> Result<SigningStatus, AppError> {
    let repo = open_repo_at(workdir)?;
    let cfg = repo.config()?.snapshot()?;
    let enabled = cfg.get_bool("commit.gpgsign").unwrap_or(false);
    let key = read_key(&cfg);
    Ok(SigningStatus {
        enabled,
        format: read_format(&cfg),
        has_key: key.is_some(),
        key,
    })
}

/// Create a SIGNED commit object via `git commit-tree -S` and move HEAD via
/// `git update-ref` (P58 D1). BLOCKING. The caller has already run every guard,
/// written the tree, and resolved the identity signatures — this only assembles
/// the two plumbing spawns.
///
/// `author`/`committer` supply identity via `GIT_AUTHOR_*` / `GIT_COMMITTER_*`
/// env; their `when()` is passed as `GIT_AUTHOR_DATE`/`GIT_COMMITTER_DATE` in
/// git's raw internal format so both dates match the git2 path exactly (an
/// amend passes HEAD's original author, preserving its date). `message` is used
/// verbatim on stdin (already CRLF-normalized + trimmed + trailing `\n`).
/// `old_head`: `Some` ⇒ update-ref CAS old value; `None` ⇒ unborn (creates the
/// branch the symref points at).
///
/// Errors: [`AppError::ConfigMissing`] (ssh + no `user.signingkey`, named);
/// [`AppError::Git`] (signer failure / stale-old-oid CAS race / non-utf8 oid).
/// NEVER prompts (via [`crate::git::exec::SpawnGitExec`]).
#[allow(clippy::too_many_arguments)]
pub fn create_signed_commit(
    exec: &dyn GitExec,
    workdir: &Path,
    tree: git2::Oid,
    parents: &[git2::Oid],
    author: &git2::Signature<'_>,
    committer: &git2::Signature<'_>,
    message: &str,
    old_head: Option<git2::Oid>,
    reflog_summary: &str,
) -> Result<git2::Oid, AppError> {
    // ssh REQUIRES user.signingkey (git errors cryptically without it) — surface
    // a clear ConfigMissing naming the key BEFORE spawning so no object is
    // written (OQ2). openpgp/x509 fall back to git's committer-email selection.
    let signing = {
        let repo = open_repo_at(workdir)?;
        resolve_signing(&repo.config()?.snapshot()?, Some(true))
    };
    if signing.format == SignFormat::Ssh && signing.key.is_none() {
        return Err(config_missing_key());
    }

    // ---- git commit-tree -S <tree> [-p <parent>…]  (message on stdin) ----
    let tree_hex = tree.to_string();
    let parent_hexes: Vec<String> = parents.iter().map(git2::Oid::to_string).collect();
    let mut args: Vec<&str> = vec!["commit-tree", tree_hex.as_str(), "-S"];
    for p in &parent_hexes {
        args.push("-p");
        args.push(p.as_str());
    }

    let author_name = lossy(author.name_bytes());
    let author_email = lossy(author.email_bytes());
    let author_date = git_raw_date(&author.when());
    let committer_name = lossy(committer.name_bytes());
    let committer_email = lossy(committer.email_bytes());
    let committer_date = git_raw_date(&committer.when());
    let env: [(&str, &str); 6] = [
        ("GIT_AUTHOR_NAME", author_name.as_str()),
        ("GIT_AUTHOR_EMAIL", author_email.as_str()),
        ("GIT_AUTHOR_DATE", author_date.as_str()),
        ("GIT_COMMITTER_NAME", committer_name.as_str()),
        ("GIT_COMMITTER_EMAIL", committer_email.as_str()),
        ("GIT_COMMITTER_DATE", committer_date.as_str()),
    ];

    let out = exec.exec(&args, workdir, Some(message.as_bytes()), &env)?;
    if !out.success {
        return Err(AppError::Git(format!(
            "commit signing failed: {}",
            tail_chars(out.stderr.trim(), 400)
        )));
    }
    let new_oid = git2::Oid::from_str(out.stdout.trim())
        .map_err(|e| AppError::Git(format!("`git commit-tree` returned an invalid oid: {e}")))?;

    // ---- git update-ref -m <reflog> HEAD <newoid> [<oldoid>] ----
    // git-exact HEAD/branch move: follows the symref, creates the branch on an
    // unborn HEAD, and the <oldoid> CAS aborts if HEAD moved under us.
    let new_hex = new_oid.to_string();
    let old_hex = old_head.map(|o| o.to_string());
    let mut uargs: Vec<&str> = vec!["update-ref", "-m", reflog_summary, "HEAD", new_hex.as_str()];
    if let Some(o) = old_hex.as_deref() {
        uargs.push(o);
    }
    let uout = exec.exec(&uargs, workdir, None, &[])?;
    if !uout.success {
        return Err(AppError::Git(format!(
            "failed to move HEAD after signing: {}",
            tail_chars(uout.stderr.trim(), 400)
        )));
    }
    Ok(new_oid)
}

// ---- helpers ------------------------------------------------------------------

/// ConfigMissing naming `user.signingkey` (mirrors `resolve_signature`'s shape).
fn config_missing_key() -> AppError {
    AppError::ConfigMissing(
        "commit signing requires a key: user.signingkey is not set. \
         Run: git config user.signingkey <key>"
            .to_string(),
    )
}

/// Format a git2 time as git's internal `<unix-seconds> <±HHMM>` date, which
/// `git commit-tree` accepts via `GIT_AUTHOR_DATE`/`GIT_COMMITTER_DATE`. Chosen
/// over RFC 2822 (no locale, no weekday/month-name math — the exact form git
/// itself stores) so the signed path preserves author + committer dates
/// byte-for-byte against the git2 path.
fn git_raw_date(when: &git2::Time) -> String {
    let secs = when.seconds();
    let off = when.offset_minutes();
    let sign = if off < 0 { '-' } else { '+' };
    let abs = off.abs();
    format!("{secs} {sign}{:02}{:02}", abs / 60, abs % 60)
}

fn lossy(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

/// Char-safe last-`max` characters of `s` (panic-free stderr tail).
fn tail_chars(s: &str, max: usize) -> String {
    let chars: Vec<char> = s.chars().collect();
    let start = chars.len().saturating_sub(max);
    chars[start..].iter().collect()
}

/// Open the repo at `workdir` with `NO_SEARCH` (same as every other git/ module).
fn open_repo_at(workdir: &Path) -> Result<git2::Repository, AppError> {
    Ok(git2::Repository::open_ext(
        workdir,
        git2::RepositoryOpenFlags::NO_SEARCH,
        std::iter::empty::<&std::ffi::OsStr>(),
    )?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    // ---------------------------------------------------------- guards
    fn have_git() -> bool {
        Command::new("git").arg("--version").output().is_ok()
    }
    fn have_ssh_keygen() -> bool {
        Command::new("ssh-keygen").arg("--help").output().is_ok()
            || Command::new("ssh-keygen").arg("-A").output().is_ok()
    }

    // ---------------------------------------------------------- isolated config
    fn isolated_config(entries: &[(&str, &str)]) -> (tempfile::TempDir, git2::Config) {
        let dir = crate::testutil::scratch_dir();
        let file = dir.path().join("gitconfig");
        std::fs::write(&file, "").expect("config file");
        let mut cfg = git2::Config::open(&file).expect("open config");
        for (k, v) in entries {
            cfg.set_str(k, v).expect("set entry");
        }
        (dir, cfg)
    }

    // ---------------------------------------------------------- pure units
    #[test]
    fn resolve_signing_none_follows_gpgsign() {
        let (_d, off) = isolated_config(&[]);
        assert!(!resolve_signing(&off, None).sign, "unset gpgsign ⇒ off");
        let (_d, on) = isolated_config(&[("commit.gpgsign", "true")]);
        assert!(resolve_signing(&on, None).sign, "gpgsign=true ⇒ on");
    }

    #[test]
    fn resolve_signing_override_wins_over_config() {
        let (_d, on) = isolated_config(&[("commit.gpgsign", "true")]);
        assert!(!resolve_signing(&on, Some(false)).sign, "Some(false) overrides true");
        let (_d, off) = isolated_config(&[("commit.gpgsign", "false")]);
        assert!(resolve_signing(&off, Some(true)).sign, "Some(true) overrides false");
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

        // Unset format ⇒ Openpgp default; empty key ⇒ None.
        let (_d, cfg) = isolated_config(&[("user.signingkey", "   ")]);
        let r = resolve_signing(&cfg, None);
        assert_eq!(r.format, SignFormat::Openpgp);
        assert_eq!(r.key, None, "whitespace key ⇒ None");
    }

    #[test]
    fn git_raw_date_formats_offset() {
        assert_eq!(git_raw_date(&git2::Time::new(1_000_000_000, 120)), "1000000000 +0200");
        assert_eq!(git_raw_date(&git2::Time::new(0, -300)), "0 -0500");
        assert_eq!(git_raw_date(&git2::Time::new(42, 0)), "42 +0000");
    }

    // ---------------------------------------------------------- wire shapes
    #[test]
    fn sign_format_serializes_lowercase() {
        assert_eq!(serde_json::to_value(SignFormat::Ssh).unwrap(), "ssh");
        assert_eq!(serde_json::to_value(SignFormat::Openpgp).unwrap(), "openpgp");
    }

    #[test]
    fn signing_status_wire_shape_camel_case() {
        let v = serde_json::to_value(SigningStatus {
            enabled: true,
            format: Some(SignFormat::Ssh),
            has_key: true,
            key: Some("/k".to_string()),
        })
        .expect("json");
        assert_eq!(v["enabled"], true);
        assert_eq!(v["format"], "ssh");
        assert_eq!(v["hasKey"], true);
        assert_eq!(v["key"], "/k");
        // key omitted when None (skip_serializing_if).
        let none = serde_json::to_value(SigningStatus {
            enabled: false,
            format: None,
            has_key: false,
            key: None,
        })
        .expect("json");
        assert!(none.get("key").is_none());
        assert_eq!(none["format"], serde_json::Value::Null);
    }

    // ---------------------------------------------------------- fixtures
    fn init_repo(dir: &Path) -> git2::Repository {
        let repo = git2::Repository::init_opts(
            dir,
            git2::RepositoryInitOptions::new().initial_head("main"),
        )
        .expect("init");
        let mut cfg = repo.config().expect("config");
        cfg.set_str("user.name", "Sig Tester").expect("name");
        cfg.set_str("user.email", "sig@example.com").expect("email");
        cfg.set_bool("core.autocrlf", false).expect("autocrlf");
        repo
    }

    fn stage_write(dir: &Path, name: &str, body: &str) {
        std::fs::write(dir.join(name), body).expect("write file");
        crate::git::stage::stage_paths(dir, &[name.to_string()]).expect("stage");
    }

    fn head_oid(dir: &Path) -> Option<String> {
        let repo = open_repo_at(dir).ok()?;
        let oid = repo.head().ok()?.peel_to_commit().ok()?.id();
        Some(oid.to_string())
    }

    /// Raw commit object text (`git cat-file -p`) — used to assert the
    /// presence/absence of the `gpgsig` header.
    fn commit_object(dir: &Path, oid: &str) -> String {
        let out = Command::new("git")
            .args(["cat-file", "-p", oid])
            .current_dir(dir)
            .output()
            .expect("cat-file");
        assert!(out.status.success(), "cat-file failed: {}", String::from_utf8_lossy(&out.stderr));
        String::from_utf8_lossy(&out.stdout).into_owned()
    }

    fn git_ok(dir: &Path, args: &[&str]) -> std::process::Output {
        Command::new("git").args(args).current_dir(dir).output().expect("git")
    }

    /// Generate an ephemeral ed25519 key (EMPTY passphrase ⇒ no agent, hermetic),
    /// write an `allowed_signers` naming the committer email, and set the ssh
    /// signing config (`commit.gpgsign` per `gpgsign`). Forward-slash paths so
    /// git + ssh-keygen agree on Windows. Returns the private-key path string.
    fn setup_ssh_signing(dir: &Path, gpgsign: bool) -> String {
        let fwd = |p: std::path::PathBuf| p.to_string_lossy().replace('\\', "/");
        let key = fwd(dir.join("id_ed25519"));
        let out = Command::new("ssh-keygen")
            .args(["-t", "ed25519", "-N", "", "-C", "sig@example.com", "-f", &key, "-q"])
            .output()
            .expect("ssh-keygen");
        assert!(out.status.success(), "keygen: {}", String::from_utf8_lossy(&out.stderr));

        let pubtext = std::fs::read_to_string(dir.join("id_ed25519.pub")).expect("pub key");
        let mut it = pubtext.split_whitespace();
        let ktype = it.next().unwrap_or_default();
        let kdata = it.next().unwrap_or_default();
        let signers = dir.join("allowed_signers");
        std::fs::write(&signers, format!("sig@example.com {ktype} {kdata}\n")).expect("signers");

        let repo = open_repo_at(dir).expect("open");
        let mut cfg = repo.config().expect("cfg");
        cfg.set_str("gpg.format", "ssh").expect("format");
        cfg.set_str("user.signingkey", &key).expect("signingkey");
        cfg.set_str("gpg.ssh.allowedSignersFile", &fwd(signers)).expect("allowed");
        cfg.set_bool("commit.gpgsign", gpgsign).expect("gpgsign");
        key
    }

    // ---------------------------------------------------------- SSH oracle
    #[test]
    fn oracle_ssh_sign_creates_verifiable_commit() {
        if !have_git() || !have_ssh_keygen() {
            eprintln!("skipping: git / ssh-keygen not available");
            return;
        }
        use crate::git::commit::create_commit;
        let dir = crate::testutil::scratch_dir();
        let d = dir.path();
        init_repo(d);
        // Unsigned base first (proves the byte-identical path AND gives an oid to
        // move from).
        stage_write(d, "base.txt", "base\n");
        create_commit(d, "base", None).expect("base commit");
        let base = head_oid(d).expect("base head");
        assert!(!commit_object(d, &base).contains("gpgsig"), "base must be unsigned");

        setup_ssh_signing(d, false);
        stage_write(d, "a.txt", "alpha\n");
        let res = create_commit(d, "signed subject", Some(true)).expect("signed commit");

        // HEAD moved to the new signed commit.
        assert_eq!(head_oid(d).as_deref(), Some(res.oid.as_str()));
        assert_ne!(res.oid, base);
        assert_eq!(res.branch.as_deref(), Some("main"));
        // gpgsig header present (SSH signature).
        let obj = commit_object(d, &res.oid);
        assert!(obj.contains("gpgsig"), "signed commit must carry a gpgsig header:\n{obj}");
        // git agrees the signature is good + trusted (allowed_signers names us).
        let verify = git_ok(d, &["verify-commit", &res.oid]);
        assert!(verify.status.success(), "verify-commit: {}", String::from_utf8_lossy(&verify.stderr));
        let g = git_ok(d, &["log", "--format=%G?", "-1", &res.oid]);
        assert_eq!(String::from_utf8_lossy(&g.stdout).trim(), "G", "%G? must be Good");
        // reflog records the commit move.
        let reflog = git_ok(d, &["reflog", "-1"]);
        assert!(
            String::from_utf8_lossy(&reflog.stdout).contains("commit:"),
            "reflog: {}",
            String::from_utf8_lossy(&reflog.stdout)
        );
        // committer identity == resolve_signature.
        let repo = open_repo_at(d).expect("open");
        let head = repo.head().unwrap().peel_to_commit().unwrap();
        assert_eq!(head.committer().email().ok(), Some("sig@example.com"));
    }

    #[test]
    fn oracle_ssh_amend_preserves_author_and_resigns() {
        if !have_git() || !have_ssh_keygen() {
            return;
        }
        use crate::git::commit::{amend_commit, create_commit};
        let dir = crate::testutil::scratch_dir();
        let d = dir.path();
        let repo = init_repo(d);
        stage_write(d, "a.txt", "one\n");
        create_commit(d, "orig subject", None).expect("commit");
        let orig = repo.head().unwrap().peel_to_commit().unwrap();
        let orig_author_when = orig.author().when();
        let orig_author_email = orig.author().email().ok().map(str::to_string);

        setup_ssh_signing(d, false);
        // Message-only signed amend.
        let res = amend_commit(d, "amended subject", Some(true)).expect("amend");
        let repo2 = open_repo_at(d).unwrap();
        let head = repo2.head().unwrap().peel_to_commit().unwrap();
        assert_eq!(head.author().email().ok().map(str::to_string), orig_author_email, "author preserved");
        assert_eq!(head.author().when().seconds(), orig_author_when.seconds(), "author date preserved");
        assert_eq!(head.message().ok(), Some("amended subject\n"));
        assert!(commit_object(d, &res.oid).contains("gpgsig"), "amend must re-sign");
    }

    #[test]
    fn config_gates_decide_signing() {
        if !have_git() || !have_ssh_keygen() {
            return;
        }
        use crate::git::commit::create_commit;
        let dir = crate::testutil::scratch_dir();
        let d = dir.path();
        init_repo(d);
        setup_ssh_signing(d, false); // gpg.format=ssh + key, commit.gpgsign=false

        // (a) sign=None + gpgsign=false ⇒ UNSIGNED, byte-identical (no header).
        stage_write(d, "a.txt", "a\n");
        let a = create_commit(d, "a", None).expect("a");
        assert!(!commit_object(d, &a.oid).contains("gpgsig"), "None+off ⇒ unsigned");

        // (b) commit.gpgsign=true + sign=None ⇒ SIGNED.
        {
            let repo = open_repo_at(d).unwrap();
            repo.config().unwrap().set_bool("commit.gpgsign", true).unwrap();
        }
        stage_write(d, "b.txt", "b\n");
        let b = create_commit(d, "b", None).expect("b");
        assert!(commit_object(d, &b.oid).contains("gpgsig"), "gpgsign=true ⇒ signed");

        // (c) sign=Some(false) overrides gpgsign=true ⇒ UNSIGNED.
        stage_write(d, "c.txt", "c\n");
        let c = create_commit(d, "c", Some(false)).expect("c");
        assert!(!commit_object(d, &c.oid).contains("gpgsig"), "Some(false) overrides ⇒ unsigned");
    }

    #[test]
    fn ssh_signing_without_key_is_config_missing() {
        if !have_git() {
            return;
        }
        use crate::git::commit::create_commit;
        let dir = crate::testutil::scratch_dir();
        let d = dir.path();
        let repo = init_repo(d);
        stage_write(d, "base.txt", "base\n");
        create_commit(d, "base", None).expect("base");
        let base = head_oid(d).expect("base head");
        // ssh format, NO user.signingkey.
        repo.config().unwrap().set_str("gpg.format", "ssh").unwrap();

        stage_write(d, "a.txt", "a\n");
        let err = create_commit(d, "signed", Some(true)).expect_err("must be ConfigMissing");
        match err {
            AppError::ConfigMissing(m) => assert!(m.contains("user.signingkey"), "names the key: {m}"),
            other => panic!("expected ConfigMissing, got {other:?}"),
        }
        assert_eq!(head_oid(d).as_deref(), Some(base.as_str()), "no commit created");
    }

    // ---------------------------------------------------------- signing_status
    #[test]
    fn signing_status_reads_config() {
        let dir = crate::testutil::scratch_dir();
        let d = dir.path();
        let repo = init_repo(d);
        // Unset ⇒ disabled, no format, no key.
        let s = signing_status(d).expect("status");
        assert!(!s.enabled);
        assert_eq!(s.format, None);
        assert!(!s.has_key);
        assert_eq!(s.key, None);

        {
            let mut cfg = repo.config().unwrap();
            cfg.set_str("gpg.format", "ssh").unwrap();
            cfg.set_str("user.signingkey", "/keys/id").unwrap();
            cfg.set_bool("commit.gpgsign", true).unwrap();
        }
        let s = signing_status(d).expect("status");
        assert!(s.enabled);
        assert_eq!(s.format, Some(SignFormat::Ssh));
        assert!(s.has_key);
        assert_eq!(s.key.as_deref(), Some("/keys/id"));
    }
}
