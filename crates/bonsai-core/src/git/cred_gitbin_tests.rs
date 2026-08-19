//! P70: the credential ladder's "git is not available" diagnostics — split out
//! of `remote.rs` (already 4× the soft file limit) so the tests can grow without
//! it. Same module tree as an inline `mod`, so it keeps private access to the
//! ladder internals (`acquire_cred_with`, `exhausted_error`, the sentinels).
//!
//! Scope: the ONE distinction P70 exists to make — "git itself could not be
//! launched" vs "authentication genuinely failed" — asserted at both the
//! `exhausted_error` verdict and the `map_remote_err` mapping.

use super::*;

/// P70 (orchestrator correction to contract §3.1): a missing `git` must NOT
/// short-circuit the credential ladder. An SSH remote authenticates
/// entirely inside libgit2 (`ssh_key_from_agent` / `default`) and never
/// needs `git.exe`, so such a user — whose fetch works today — must not
/// start seeing "Git is not available".
#[test]
fn git_missing_does_not_break_non_helper_credential_rungs() {
    let attempts = RefCell::new(CredAttempts::default());
    let cred = acquire_cred_with(
        None,
        &attempts,
        "ssh://git@example.com/o/r.git",
        Some("git"),
        git2::CredentialType::SSH_KEY | git2::CredentialType::DEFAULT,
        true, // git is unresolvable
    );
    assert!(
        cred.is_ok(),
        "SSH/default rungs must still produce a credential with git absent: {:?}",
        cred.err().map(|e| e.message().to_string())
    );
    assert!(
        attempts.borrow().helper_git_unavailable.is_none(),
        "the Helper rung was never eligible, so nothing is recorded"
    );

    // Same with the Helper rung ALSO offered: it fails instantly (no spawn)
    // and the ladder still lands on a working rung.
    let attempts = RefCell::new(CredAttempts::default());
    let cred = acquire_cred_with(
        None,
        &attempts,
        "https://example.com/o/r.git",
        Some("git"),
        git2::CredentialType::USER_PASS_PLAINTEXT
            | git2::CredentialType::SSH_KEY
            | git2::CredentialType::DEFAULT,
        true,
    );
    assert!(cred.is_ok(), "a later rung still succeeds with git absent");
    assert!(
        attempts.borrow().helper_git_unavailable.is_some(),
        "the Helper rung recorded WHY it failed, for the exhausted verdict"
    );
}

/// P70: when the Helper rung is the ONLY option and git cannot be launched,
/// the exhausted ladder yields the honest sentinel — which `map_remote_err`
/// turns into `GitNotFound`, never `AuthFailed`. No git is spawned.
#[test]
fn git_missing_with_only_helper_rung_yields_git_not_found() {
    let attempts = RefCell::new(CredAttempts::default());
    let err = match acquire_cred_with(
        None,
        &attempts,
        "https://example.com/o/r.git",
        None,
        git2::CredentialType::USER_PASS_PLAINTEXT,
        true,
    ) {
        Ok(_) => panic!("no rung can succeed when only Helper is allowed and git is missing"),
        Err(e) => e,
    };
    assert!(err.message().contains(GIT_MISSING_MSG), "{}", err.message());

    match map_remote_err(err, "origin") {
        AppError::GitNotFound(m) => {
            assert!(m.contains("NOT an authentication failure"), "{m}");
            assert!(!m.contains("cached credentials for this remote"), "{m}");
        }
        other => panic!("expected GitNotFound, got {other:?}"),
    }
}

/// P70 §6.1 #16 — the SSH-only exhaustion guard, and the single most
/// important regression pin in this milestone: with git unresolvable, an
/// exhaustion reached WITHOUT the Helper rung ever being offered must keep
/// the pre-P70 auth verdict. If this ever flips to `GitNotFound`, an SSH
/// user with a genuinely bad key is told "Git is not available", which is
/// exactly the misdiagnosis P70 exists to remove — in mirror image.
///
/// `allowed = USERNAME` is what forces true exhaustion: `SSH_KEY` would
/// always succeed, because `Cred::ssh_key_from_agent` only CONSTRUCTS a
/// credential (the agent is contacted later, inside libgit2's transport),
/// so it cannot fail here on any machine.
#[test]
fn ssh_only_exhaustion_with_git_missing_is_auth_failed_not_git_not_found() {
    let attempts = RefCell::new(CredAttempts::default());
    let err = match acquire_cred_with(
        None,
        &attempts,
        "ssh://git@example.com/o/r.git",
        Some("git"),
        git2::CredentialType::USERNAME, // no Helper, no SshAgent, no Default
        true,                           // git is unresolvable
    ) {
        Ok(_) => panic!("no rung is compatible with USERNAME — the ladder must exhaust"),
        Err(e) => e,
    };
    assert!(
        attempts.borrow().helper_git_unavailable.is_none(),
        "the Helper rung was never offered, so nothing may be recorded"
    );
    assert!(
        err.message().contains(CRED_EXHAUSTED_MSG),
        "expected the ordinary exhaustion sentinel, got {}",
        err.message()
    );
    assert!(
        !err.message().contains(GIT_MISSING_MSG),
        "{}",
        err.message()
    );
    match map_remote_err(err, "origin") {
        AppError::AuthFailed(m) => assert!(m.contains("authentication failed for 'origin'"), "{m}"),
        other => panic!("expected AuthFailed, got {other:?}"),
    }
}

/// P70 regression guard: an exhausted ladder where git RAN fine (the helper
/// simply had nothing) keeps the pre-P70 auth copy — both `exhausted_error`
/// verdicts and both `map_remote_err` arms.
#[test]
fn exhausted_verdict_splits_git_missing_from_genuine_auth_failure() {
    let missing = exhausted_error(Some("The system cannot find the file specified."));
    assert!(missing.message().contains(GIT_MISSING_MSG));
    assert!(matches!(
        map_remote_err(missing, "origin"),
        AppError::GitNotFound(_)
    ));

    let empty_helper = exhausted_error(None);
    assert_eq!(empty_helper.message(), CRED_EXHAUSTED_MSG);
    match map_remote_err(empty_helper, "origin") {
        AppError::AuthFailed(m) => {
            assert!(m.contains("authentication failed for 'origin'"), "{m}");
        }
        other => panic!("expected AuthFailed, got {other:?}"),
    }

    // A plain git2 Auth error (no sentinel) is still AuthFailed.
    let plain = git2::Error::new(
        git2::ErrorCode::Auth,
        git2::ErrorClass::Http,
        "401 Unauthorized",
    );
    assert!(matches!(
        map_remote_err(plain, "origin"),
        AppError::AuthFailed(_)
    ));
}

// ===========================================================================
// P70 §6.1 #13 / #14 / #18 — the REAL out-of-process spawn paths.
//
// Everything above this line drives the ladder through INJECTED seams (a bool
// for `git_missing`, a `FillFn` for the cache). That proves the wiring, but it
// never actually tries to launch a process, so `credential_fill`'s genuine
// `cmd.spawn() -> Err(e)` arm — the one that covers the mid-session
// stale-git-path race — stayed unexecuted. The tests below close that: they
// point the REAL resolver at a REAL (or deliberately absent) executable via
// `BONSAI_GIT_BIN` and let the code spawn for real.
//
// Why a child process. `BONSAI_GIT_BIN` is read by `HostGitEnv::var`, i.e. the
// process environment, and `git_bin()` memoises the answer. Setting it in-
// process would (a) be a cross-test hazard — the whole crate's unit tests share
// one process and run in parallel, which is exactly why the `GitEnv` seam
// exists (contract §7.1) — and (b) usually be a no-op, because the cache is
// already warm. So the parent test re-execs the TEST BINARY ITSELF
// (`current_exe()`) with the variable set via `Command::env`, running exactly
// one `#[ignore]`d child test. Same spirit as the `BONSAI_CLAUDE_BIN`
// precedent, with zero `std::env` mutation in this process.
// ===========================================================================

/// Set by the parent on the re-exec so a child test knows it is being driven.
/// Without it (e.g. a bare `cargo test -- --ignored` sweep) the child no-ops
/// instead of failing on an unset `BONSAI_GIT_BIN`.
const CHILD_ENV: &str = "BONSAI_P70_SPAWN_CHILD";

/// The stub reads this and appends one line per invocation — a spawn counter
/// that survives a process boundary (§6.1 #18).
const STUB_MARKER_ENV: &str = "BONSAI_GIT_STUB_MARKER";

/// A launchable `git` stand-in that exits 0 with empty stdout. Windows runs the
/// `.cmd` directly; the POSIX twin gets its executable bit forced on at test
/// time (git does not reliably preserve the mode bit) — the same helper shape
/// as `ai::testutil::stub_path`.
fn silent_git_stub() -> std::path::PathBuf {
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    if cfg!(windows) {
        fixtures.join("git_stub_silent.cmd")
    } else {
        let path = fixtures.join("git_stub_silent.sh");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if let Ok(meta) = std::fs::metadata(&path) {
                let mut perms = meta.permissions();
                perms.set_mode(perms.mode() | 0o111);
                let _ = std::fs::set_permissions(&path, perms);
            }
        }
        path
    }
}

/// Re-exec this test binary for exactly one `#[ignore]`d child test, with
/// `BONSAI_GIT_BIN` = `git_bin` and a fresh spawn-marker path. Returns the
/// marker path so the PARENT can assert whether anything was launched.
///
/// The `1 passed` assertion is load-bearing: libtest exits 0 after running
/// ZERO tests when `--exact` matches nothing, so a renamed child would silently
/// turn this into a no-op test.
fn run_child(child_test: &str, git_bin: &Path) -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempfile::tempdir().expect("tempdir");
    let marker = dir.path().join("spawned.log");
    // `module_path!()` is `bonsai_core::git::cred::gitbin_tests`; libtest
    // names tests without the crate prefix. Derived rather than hard-coded so a
    // module rename cannot silently disable these tests.
    let module = module_path!()
        .split_once("::")
        .map(|(_, rest)| rest)
        .unwrap_or(module_path!());
    let filter = format!("{module}::{child_test}");
    let exe = std::env::current_exe().expect("current_exe");
    let out = std::process::Command::new(exe)
        .args([
            filter.as_str(),
            "--exact",
            "--ignored",
            "--nocapture",
            "--test-threads",
            "1",
        ])
        .env(crate::gitbin::GIT_BIN_ENV, git_bin)
        .env(STUB_MARKER_ENV, &marker)
        .env(CHILD_ENV, "1")
        .output()
        .expect("re-exec the test binary");
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
    assert!(
        out.status.success(),
        "child test `{filter}` failed\n--- stdout ---\n{stdout}\n--- stderr ---\n{stderr}"
    );
    assert!(
        stdout.contains("1 passed"),
        "child test `{filter}` did not RUN (filter mismatch?)\n{stdout}"
    );
    (dir, marker)
}

/// True when this process is the re-exec'd child. A child test reached any
/// other way (a bare `--ignored` sweep) no-ops rather than failing.
fn child_ready() -> bool {
    std::env::var(CHILD_ENV).is_ok()
}

// ---------------------------------------------------------------- #13

/// P70 §6.1 #13 — parent half. `BONSAI_GIT_BIN` points at a path that does not
/// exist, so the resolver hands `Command` an unlaunchable program and the spawn
/// fails for real.
#[test]
fn credential_fill_with_unlaunchable_git_is_git_unavailable() {
    let missing = tempfile::tempdir().expect("tempdir");
    let bogus = missing.path().join(if cfg!(windows) {
        "no-such-git.exe"
    } else {
        "no-such-git"
    });
    assert!(!bogus.exists(), "the fixture path must NOT exist");
    let (_dir, marker) = run_child("child_unlaunchable_git", &bogus);
    assert!(
        !marker.exists(),
        "nothing may have been launched — the program does not exist"
    );
}

/// P70 §6.1 #13 — child half. Runs with `BONSAI_GIT_BIN` = a nonexistent path.
///
/// Note `git_missing()` is FALSE here: the ladder's Override rung is taken
/// verbatim without an `is_file()` check (contract §2.3 step 1), so this is
/// precisely the "the resolved path went stale mid-session" race that §3.1
/// rule 3 exists for — the arm no injected-bool test can reach.
#[test]
#[ignore = "driven out-of-process by credential_fill_with_unlaunchable_git_is_git_unavailable"]
fn child_unlaunchable_git() {
    if !child_ready() {
        return;
    }
    let bin = crate::gitbin::git_bin();
    assert_eq!(bin.source, crate::gitbin::GitBinSource::Override);
    assert!(!bin.path.exists(), "fixture must be unlaunchable: {bin:?}");
    assert!(
        !crate::gitbin::git_missing(),
        "an Override path is 'resolved' even when it is bogus — this test IS \
         the stale-path race, not the cheap-fail path"
    );

    let url = "https://example.com/o/r.git";
    match credential_fill(None, url) {
        FillOutcome::GitUnavailable(detail) => {
            assert!(!detail.is_empty(), "the io error text is kept for the log")
        }
        other => panic!("expected GitUnavailable from a real spawn failure, got {other:?}"),
    }

    // Op level: the ladder must exhaust with the HONEST verdict. Before P70
    // this produced the "no cached credentials" auth toast.
    let attempts = RefCell::new(CredAttempts::default());
    let err = match acquire_cred_with(
        None,
        &attempts,
        url,
        None,
        git2::CredentialType::USER_PASS_PLAINTEXT,
        crate::gitbin::git_missing(), // false — the runtime spawn failure is the only signal
    ) {
        Ok(_) => panic!("only the Helper rung is allowed, and git cannot be launched"),
        Err(e) => e,
    };
    assert!(
        attempts.borrow().helper_git_unavailable.is_some(),
        "the RUNTIME GitUnavailable must be recorded, not just the cheap-fail path"
    );
    assert!(err.message().contains(GIT_MISSING_MSG), "{}", err.message());
    match map_remote_err(err, "origin") {
        AppError::GitNotFound(m) => {
            assert!(m.contains("NOT an authentication failure"), "{m}");
            assert!(!m.contains("cached credentials for this remote"), "{m}");
        }
        other => panic!("expected GitNotFound, got {other:?}"),
    }
}

// ---------------------------------------------------------- #14 + #18

/// P70 §6.1 #14 (+ the hard half of #18) — parent. Same override mechanism,
/// but pointed at a stub that RUNS and exits 0 with no output. The marker file
/// is the proof that the two child assertions are about different things: it
/// must exist afterwards (the stub really was launched at least once), which is
/// what makes the "zero spawns" assertion inside the child meaningful.
#[test]
fn silent_stub_git_yields_no_credentials_not_git_unavailable() {
    let stub = silent_git_stub();
    assert!(stub.is_file(), "stub fixture missing: {}", stub.display());
    let (_dir, marker) = run_child("child_silent_stub_git", &stub);
    assert!(
        marker.exists(),
        "the stub must have been spawned at least once — otherwise the \
         NoCredentials verdict would be vacuous"
    );
}

/// P70 §6.1 #14 + #18 — child. Three assertions, in order, sharing one stub:
///
/// 1. **#18, unconditionally and for real**: with `git_missing == true` the
///    Helper rung performs ZERO spawns (the marker file is still absent after a
///    full `acquire_cred_with` run) yet still records WHY it failed, so the
///    ladder can carry on to SshAgent. This is the guard that keeps an
///    ssh-agent user working when git falls off PATH.
/// 2. **#14**: the same stub, spawned for real, yields `NoCredentials` — git
///    ran and had nothing. The marker now exists, proving it really launched.
/// 3. The exhaustion that follows keeps the UNCHANGED pre-P70 auth copy.
#[test]
#[ignore = "driven out-of-process by silent_stub_git_yields_no_credentials_not_git_unavailable"]
fn child_silent_stub_git() {
    if !child_ready() {
        return;
    }
    let bin = crate::gitbin::git_bin();
    assert_eq!(bin.source, crate::gitbin::GitBinSource::Override);
    assert!(bin.path.is_file(), "stub must be launchable: {bin:?}");
    let marker = std::path::PathBuf::from(
        std::env::var(STUB_MARKER_ENV).expect("parent sets the marker path"),
    );
    assert!(!marker.exists(), "marker starts absent");

    let url = "https://example.com/o/r.git";

    // (1) #18 — the Helper rung must not spawn when git is known-missing.
    let attempts = RefCell::new(CredAttempts::default());
    let err = match acquire_cred_with(
        None,
        &attempts,
        url,
        None,
        git2::CredentialType::USER_PASS_PLAINTEXT,
        true, // git_missing
    ) {
        Ok(_) => panic!("only the Helper rung is allowed"),
        Err(e) => e,
    };
    assert!(
        !marker.exists(),
        "the Helper rung MUST fail cheaply with zero spawns when git is missing"
    );
    assert!(
        attempts.borrow().helper_git_unavailable.is_some(),
        "it still records why, for the exhausted verdict"
    );
    assert!(err.message().contains(GIT_MISSING_MSG), "{}", err.message());

    // (2) #14 — the stub, actually launched, is `git ran and had nothing`.
    assert_eq!(
        credential_fill(None, url),
        FillOutcome::NoCredentials,
        "a git that exits 0 with no output is NoCredentials, never GitUnavailable"
    );
    assert!(
        marker.exists(),
        "the stub must have been spawned — otherwise (2) proves nothing"
    );

    // (3) …and exhaustion after it keeps the pre-P70 auth copy verbatim.
    let attempts = RefCell::new(CredAttempts::default());
    let err = match acquire_cred_with(
        None,
        &attempts,
        url,
        None,
        git2::CredentialType::USER_PASS_PLAINTEXT,
        false, // git resolves fine — it simply has nothing cached
    ) {
        Ok(_) => panic!("the stub never returns credentials"),
        Err(e) => e,
    };
    assert!(
        attempts.borrow().helper_git_unavailable.is_none(),
        "git RAN — nothing about launching may be recorded"
    );
    assert_eq!(err.message(), CRED_EXHAUSTED_MSG);
    match map_remote_err(err, "origin") {
        AppError::AuthFailed(m) => assert!(m.contains("authentication failed for 'origin'"), "{m}"),
        other => panic!("expected the unchanged AuthFailed copy, got {other:?}"),
    }
}
