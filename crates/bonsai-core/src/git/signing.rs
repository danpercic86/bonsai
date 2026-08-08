//! Commit signing (P58a) + signature verification (P58b).
//!
//! git2 has no "sign this commit" call, so signing follows mechanism **C** (P58
//! D1/OQ1): git2 assembles the tree + identity + guards (in `commit.rs` /
//! `merge.rs`), then the `git` binary produces the SIGNED object via
//! `git commit-tree -S` and moves HEAD via `git update-ref`. Both SSH and
//! OpenPGP are signed by git itself (respecting `gpg.format`, `user.signingkey`,
//! and the `gpg.program` / `gpg.ssh.program` overrides), so the unsigned path
//! stays 100% git2 and byte-identical to pre-P58.
//!
//! P58a covers signing + the read-only [`signing_status`] indicator; P58b adds
//! [`verify_commits`]: ONE `git log --format=%G?` subprocess verifies a bounded
//! batch of oids, authoritative for BOTH ssh and openpgp. The git-driven
//! behavior lives in the CLI-oracle integration file `tests/signing_cli.rs`.

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
/// `git update-ref` (P58 D1). BLOCKING. The caller supplies the written tree,
/// resolved identity (passed via `GIT_AUTHOR_*`/`GIT_COMMITTER_*`, dates in git's
/// raw format for byte-exact parity), verbatim `message` on stdin, and `old_head`
/// (`Some` ⇒ update-ref CAS; `None` ⇒ unborn). Errors: [`AppError::ConfigMissing`]
/// (ssh + no `user.signingkey`); [`AppError::Git`] (signer failure / CAS race /
/// bad oid). NEVER prompts (via [`crate::git::exec::SpawnGitExec`]).
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

/// Format a git2 time as git's internal `<unix-seconds> <±HHMM>` date, accepted
/// by `git commit-tree` via `GIT_AUTHOR_DATE`/`GIT_COMMITTER_DATE` (the exact
/// form git stores — no locale/month-name math — so both dates are preserved).
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

// ---- P58b: signature verification ---------------------------------------------

/// `git log --format=%G?` verdict, one per commit — authoritative for BOTH ssh
/// and openpgp (git owns the trust check; git2/libgit2 only prove presence).
/// Serializes camelCase to match the TS `VerifyStatus` mirror.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum VerifyStatus {
    /// `G` — valid signature from a trusted/known signer.
    Good,
    /// `U` — valid, signer identity not established (e.g. ssh key not in `allowedSigners`).
    GoodUnknown,
    /// `B` — bad signature.
    Bad,
    /// `X` — good signature that has expired.
    Expired,
    /// `Y` — good signature made by an expired key.
    ExpiredKey,
    /// `R` — good signature made by a revoked key.
    Revoked,
    /// `E` — cannot check (missing key, no `allowedSignersFile`, gpg/ssh absent).
    CannotCheck,
    /// `N` (or any unrecognized code) — no signature.
    Unsigned,
}

/// One commit's verification verdict (P58b). Wire shape camelCase; `signer` /
/// `key` are omitted when git reported them empty (e.g. unsigned commits).
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommitVerification {
    /// Full 40-hex oid, echoing the request — the frontend keys its badge by it.
    pub oid: String,
    pub status: VerifyStatus,
    /// `%GS` — signer name/identity; `None` when git reported it empty.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signer: Option<String>,
    /// `%GK` — key id / fingerprint; `None` when git reported it empty.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
}

/// Result of [`verify_commits`]: one [`CommitVerification`] per RESOLVABLE
/// requested oid, in request order. Oids dropped as non-hex or beyond the batch
/// cap are omitted — the frontend keeps them "unchecked".
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VerifyResults {
    pub verifications: Vec<CommitVerification>,
}

/// Upper bound on oids verified per call (argv sanity — the frontend sends only
/// visible graph rows). Oids beyond this are dropped.
pub const MAX_VERIFY_BATCH: usize = 512;

/// Fixed leading args (before the oid list) produced by [`build_verify_args`].
const VERIFY_ARG_PREFIX: usize = 3;

/// Verify `oids` in ONE `git log --no-walk` subprocess (P58 D2). BLOCKING. Each
/// oid is validated as 40-hex (non-hex dropped) and capped at [`MAX_VERIFY_BATCH`]
/// by [`build_verify_args`]. An empty / all-invalid set returns `Ok(empty)`
/// WITHOUT spawning. A wholesale git failure (non-zero exit) degrades EVERY
/// resolvable requested oid to [`VerifyStatus::CannotCheck`] rather than
/// erroring (so a missing gpg/ssh toolchain still renders); only a spawn / I/O
/// failure surfaces as [`AppError::Git`].
pub fn verify_commits(
    exec: &dyn GitExec,
    workdir: &Path,
    oids: &[String],
) -> Result<VerifyResults, AppError> {
    let args = build_verify_args(oids);
    // No resolvable oid ⇒ nothing to check; do not spawn git.
    if args.len() <= VERIFY_ARG_PREFIX {
        return Ok(VerifyResults {
            verifications: Vec::new(),
        });
    }
    let argv: Vec<&str> = args.iter().map(String::as_str).collect();
    let out = exec.exec(&argv, workdir, None, &[])?;
    if !out.success {
        // Degrade the resolvable requested oids (argv tail) to CannotCheck.
        let verifications = args[VERIFY_ARG_PREFIX..]
            .iter()
            .map(|oid| CommitVerification {
                oid: oid.clone(),
                status: VerifyStatus::CannotCheck,
                signer: None,
                key: None,
            })
            .collect();
        return Ok(VerifyResults { verifications });
    }
    Ok(VerifyResults {
        verifications: parse_verify_output(&out.stdout),
    })
}

/// Assemble the `git log` argv (P58 D2): a fixed `log --no-walk=unsorted
/// --format=…` prefix ([`VERIFY_ARG_PREFIX`] entries) then the 40-hex oids
/// (non-hex dropped, capped at [`MAX_VERIFY_BATCH`], order preserved). The
/// `%x1f` US separator cannot collide with oid/signer/key text. Pure.
fn build_verify_args(oids: &[String]) -> Vec<String> {
    let mut args = vec![
        "log".to_string(),
        "--no-walk=unsorted".to_string(),
        "--format=%H%x1f%G?%x1f%GS%x1f%GK".to_string(),
    ];
    args.extend(
        oids.iter()
            .filter(|o| is_hex40(o))
            .take(MAX_VERIFY_BATCH)
            .cloned(),
    );
    args
}

/// Parse `git log --format=%H%x1f%G?%x1f%GS%x1f%GK` output: one commit per line,
/// four US-separated fields (oid, code, signer, key). Empty signer/key ⇒ `None`;
/// blank / oid-less lines are skipped. Pure.
fn parse_verify_output(stdout: &str) -> Vec<CommitVerification> {
    stdout
        .lines()
        .filter_map(|line| {
            let mut fields = line.splitn(4, '\u{1f}');
            let oid = fields.next().filter(|o| !o.is_empty())?;
            let code = fields.next().unwrap_or("");
            let signer = fields.next().unwrap_or("");
            let key = fields.next().unwrap_or("");
            Some(CommitVerification {
                oid: oid.to_string(),
                status: map_status_code(code.chars().next().unwrap_or('N')),
                signer: non_empty(signer),
                key: non_empty(key),
            })
        })
        .collect()
}

/// Map a `%G?` code char to a [`VerifyStatus`] (P58 D2). Unrecognized ⇒
/// [`VerifyStatus::Unsigned`] (git only emits the documented set). Pure.
fn map_status_code(c: char) -> VerifyStatus {
    match c {
        'G' => VerifyStatus::Good,
        'U' => VerifyStatus::GoodUnknown,
        'B' => VerifyStatus::Bad,
        'X' => VerifyStatus::Expired,
        'Y' => VerifyStatus::ExpiredKey,
        'R' => VerifyStatus::Revoked,
        'E' => VerifyStatus::CannotCheck,
        _ => VerifyStatus::Unsigned, // 'N' and anything unexpected
    }
}

/// `true` when `s` is exactly 40 ASCII-hex chars (a SHA-1 oid). Anything else is
/// dropped before spawning git (never hard-fail on caller input).
fn is_hex40(s: &str) -> bool {
    s.len() == 40 && s.bytes().all(|b| b.is_ascii_hexdigit())
}

/// `None` for an empty field (git prints empty `%GS`/`%GK` for unsigned).
fn non_empty(s: &str) -> Option<String> {
    (!s.is_empty()).then(|| s.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::exec::GitOutput;

    #[test]
    fn git_raw_date_formats_offset() {
        assert_eq!(git_raw_date(&git2::Time::new(1_000_000_000, 120)), "1000000000 +0200");
        assert_eq!(git_raw_date(&git2::Time::new(0, -300)), "0 -0500");
        assert_eq!(git_raw_date(&git2::Time::new(42, 0)), "42 +0000");
    }

    // ---- verify: build_verify_args / parse / map (pure, git-free) ---------
    #[test]
    fn build_verify_args_prefix_and_drops_non_hex() {
        let good = "a".repeat(40);
        let upper = "A".repeat(40); // uppercase is still 40-hex
        let mixed = "0123456789abcdef0123456789abcdef01234567".to_string();
        let oids = vec![
            good.clone(),
            "not-hex".to_string(),
            "#".to_string(),
            String::new(),
            "b".repeat(39), // too short
            upper.clone(),
            mixed.clone(),
        ];
        let args = build_verify_args(&oids);
        assert_eq!(args[0], "log");
        assert_eq!(args[1], "--no-walk=unsorted");
        assert_eq!(args[2], "--format=%H%x1f%G?%x1f%GS%x1f%GK");
        assert_eq!(&args[VERIFY_ARG_PREFIX..], &[good, upper, mixed][..], "only 40-hex, in order");
    }

    #[test]
    fn build_verify_args_caps_at_max_batch() {
        let oids = vec!["a".repeat(40); MAX_VERIFY_BATCH + 50];
        assert_eq!(build_verify_args(&oids).len(), VERIFY_ARG_PREFIX + MAX_VERIFY_BATCH);
    }

    #[test]
    fn parse_verify_output_splits_maps_and_empties_to_none() {
        let us = '\u{1f}';
        let stdout = format!(
            "{a}{us}G{us}Ada Lovelace{us}KEY1\n{b}{us}N{us}{us}\n{c}{us}U{us}ssh-user{us}SHA256:xyz\n",
            a = "1".repeat(40),
            b = "2".repeat(40),
            c = "3".repeat(40),
        );
        let v = parse_verify_output(&stdout);
        assert_eq!(v.len(), 3);
        assert_eq!(v[0].oid, "1".repeat(40));
        assert_eq!(v[0].status, VerifyStatus::Good);
        assert_eq!(v[0].signer.as_deref(), Some("Ada Lovelace"));
        assert_eq!(v[0].key.as_deref(), Some("KEY1"));
        assert_eq!(v[1].status, VerifyStatus::Unsigned);
        assert_eq!(v[1].signer, None, "empty %GS ⇒ None");
        assert_eq!(v[1].key, None, "empty %GK ⇒ None");
        assert_eq!(v[2].status, VerifyStatus::GoodUnknown);
        assert_eq!(v[2].signer.as_deref(), Some("ssh-user"));
        assert_eq!(v[2].key.as_deref(), Some("SHA256:xyz"));
        // blank / oid-less lines skipped.
        assert!(parse_verify_output("\n\n").is_empty());
    }

    #[test]
    fn map_status_code_full_table() {
        use VerifyStatus::*;
        assert_eq!(map_status_code('G'), Good);
        assert_eq!(map_status_code('U'), GoodUnknown);
        assert_eq!(map_status_code('B'), Bad);
        assert_eq!(map_status_code('X'), Expired);
        assert_eq!(map_status_code('Y'), ExpiredKey);
        assert_eq!(map_status_code('R'), Revoked);
        assert_eq!(map_status_code('E'), CannotCheck);
        assert_eq!(map_status_code('N'), Unsigned);
        assert_eq!(map_status_code('?'), Unsigned, "unrecognized ⇒ Unsigned");
    }

    // ---- verify_commits: empty / all-invalid never spawns (P58 §8) --------
    /// Panics if invoked — proves `verify_commits` does not spawn git when no
    /// oid survives 40-hex validation. (The wholesale-failure → CannotCheck
    /// degrade is covered hermetically in `tests/signing_cli.rs`.)
    struct PanicExec;
    impl GitExec for PanicExec {
        fn exec(
            &self,
            _args: &[&str],
            _cwd: &Path,
            _stdin: Option<&[u8]>,
            _env: &[(&str, &str)],
        ) -> Result<GitOutput, AppError> {
            panic!("verify_commits must not spawn git for an empty/all-invalid set");
        }
    }

    #[test]
    fn verify_commits_empty_or_all_invalid_does_not_spawn() {
        assert!(verify_commits(&PanicExec, Path::new("."), &[]).unwrap().verifications.is_empty());
        let junk = vec!["not-hex".to_string(), "#".to_string(), "b".repeat(39)];
        assert!(verify_commits(&PanicExec, Path::new("."), &junk).unwrap().verifications.is_empty());
    }
}
