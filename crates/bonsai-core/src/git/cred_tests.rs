//! Credential-ladder unit tests, moved verbatim out of `remote.rs`'s inline
//! `mod tests`: the §6.5 credential guard, the P35 §9/§14.10 `HelperState`
//! machine, the §6.6 `map_remote_err` table, and the `credential_fill`
//! fixtures. Same module tree as an inline `mod`, so the ladder's private
//! internals stay reachable.

use super::*;

// ------------------------------------------------ §6.5 credential guard

/// All three sources allowed → Helper, SshAgent, Default, then None
/// forever (idempotent exhaustion). `next_cred_method` no longer mutates
/// `helper`, so the test drives that transition manually between calls,
/// exactly as `acquire_cred` does (P35 §11).
#[test]
fn cred_guard_full_sequence() {
    let allowed = git2::CredentialType::USER_PASS_PLAINTEXT
        | git2::CredentialType::SSH_KEY
        | git2::CredentialType::DEFAULT;
    let mut attempts = CredAttempts::default();
    assert_eq!(next_cred_method(&mut attempts, allowed), Some(CredMethod::Helper));
    attempts.helper = HelperState::Done; // simulate a fresh fill (terminal)
    assert_eq!(next_cred_method(&mut attempts, allowed), Some(CredMethod::SshAgent));
    assert_eq!(next_cred_method(&mut attempts, allowed), Some(CredMethod::Default));
    assert_eq!(next_cred_method(&mut attempts, allowed), None);
    assert_eq!(next_cred_method(&mut attempts, allowed), None);
}

/// SSH_KEY only → SshAgent once, then None.
#[test]
fn cred_guard_ssh_only() {
    let allowed = git2::CredentialType::SSH_KEY;
    let mut attempts = CredAttempts::default();
    assert_eq!(next_cred_method(&mut attempts, allowed), Some(CredMethod::SshAgent));
    assert_eq!(next_cred_method(&mut attempts, allowed), None);
}

/// Nothing allowed → None immediately.
#[test]
fn cred_guard_empty_allowed() {
    let allowed = git2::CredentialType::empty();
    let mut attempts = CredAttempts::default();
    assert_eq!(next_cred_method(&mut attempts, allowed), None);
}

/// Helper is exhausted once `acquire_cred` marks it `Done` (a fresh fill),
/// even across repeat calls with only USER_PASS_PLAINTEXT allowed.
#[test]
fn cred_guard_single_method_once() {
    let allowed = git2::CredentialType::USER_PASS_PLAINTEXT;
    let mut attempts = CredAttempts::default();
    assert_eq!(next_cred_method(&mut attempts, allowed), Some(CredMethod::Helper));
    attempts.helper = HelperState::Done; // fresh fill (miss/bypass) is terminal
    assert_eq!(next_cred_method(&mut attempts, allowed), None);
    assert_eq!(next_cred_method(&mut attempts, allowed), None);
}

// ---------------------------------- P35 §9/§14.10 HelperState machine

/// (10a) Cache HIT then server rejection → Helper attempted TWICE (the 2nd
/// a cache-bypassing re-fill), then falls through to SshAgent → Default.
#[test]
fn cred_state_hit_then_reject_allows_one_bypass_retry() {
    let allowed = git2::CredentialType::USER_PASS_PLAINTEXT
        | git2::CredentialType::SSH_KEY
        | git2::CredentialType::DEFAULT;
    let mut attempts = CredAttempts::default();
    // 1st Helper attempt returns a CACHED entry.
    assert_eq!(next_cred_method(&mut attempts, allowed), Some(CredMethod::Helper));
    attempts.helper = HelperState::RetryAllowed;
    // Server rejects -> libgit2 re-invokes -> Helper eligible ONCE more (bypass).
    assert_eq!(next_cred_method(&mut attempts, allowed), Some(CredMethod::Helper));
    attempts.helper = HelperState::Done; // bypass fresh fill is terminal
    // Rejected again -> fall through to SshAgent, then Default, then None.
    assert_eq!(next_cred_method(&mut attempts, allowed), Some(CredMethod::SshAgent));
    assert_eq!(next_cred_method(&mut attempts, allowed), Some(CredMethod::Default));
    assert_eq!(next_cred_method(&mut attempts, allowed), None);
}

/// (10b) Fresh MISS then rejection → Helper is NOT retried.
#[test]
fn cred_state_fresh_miss_not_retried() {
    let allowed = git2::CredentialType::USER_PASS_PLAINTEXT
        | git2::CredentialType::SSH_KEY
        | git2::CredentialType::DEFAULT;
    let mut attempts = CredAttempts::default();
    assert_eq!(next_cred_method(&mut attempts, allowed), Some(CredMethod::Helper));
    attempts.helper = HelperState::Done; // fresh fill (from_cache=false) -> terminal
    assert_eq!(next_cred_method(&mut attempts, allowed), Some(CredMethod::SshAgent));
    assert_eq!(next_cred_method(&mut attempts, allowed), Some(CredMethod::Default));
    assert_eq!(next_cred_method(&mut attempts, allowed), None);
}

/// (F-A5-b) `evict_fresh_on_auth_fail` returns the error unchanged and only
/// touches the cache on an AuthFailed with a recorded fresh-fill url. The
/// eviction targets the process-global cache (a no-op for an unknown key),
/// so we assert the identity + no-panic contract across all three arms.
#[test]
fn evict_fresh_on_auth_fail_is_identity_and_scoped() {
    let dir = crate::testutil::scratch_dir();
    let repo = git2::Repository::init(dir.path()).expect("init");

    // Non-auth error: returned unchanged even with a fresh-fill url recorded.
    let attempts = RefCell::new(CredAttempts {
        fresh_fill_url: Some("https://host.example/repo.git".to_string()),
        ..CredAttempts::default()
    });
    let out = evict_fresh_on_auth_fail(&repo, &attempts, AppError::NetworkError("x".into()));
    assert!(matches!(out, AppError::NetworkError(_)));

    // AuthFailed + fresh-fill url: eviction fires (no-op on the global cache
    // for this test url), error still returned unchanged.
    let out = evict_fresh_on_auth_fail(&repo, &attempts, AppError::AuthFailed("y".into()));
    assert!(matches!(out, AppError::AuthFailed(_)));

    // AuthFailed + NO fresh-fill url (a cache HIT op): nothing to evict.
    let attempts = RefCell::new(CredAttempts::default());
    let out = evict_fresh_on_auth_fail(&repo, &attempts, AppError::AuthFailed("z".into()));
    assert!(matches!(out, AppError::AuthFailed(_)));
}

/// (10c) Exhaustion (nothing allowed) returns the CRED_EXHAUSTED sentinel
/// error WITHOUT touching the cache (Helper arm never reached).
#[test]
fn cred_exhaustion_returns_sentinel_error() {
    let attempts = RefCell::new(CredAttempts::default());
    let err = match acquire_cred(
        None,
        &attempts,
        "https://example.com/repo.git",
        None,
        git2::CredentialType::empty(),
    ) {
        Ok(_) => panic!("expected exhaustion error, got a credential"),
        Err(e) => e,
    };
    assert_eq!(err.class(), git2::ErrorClass::Callback);
    assert!(err.message().contains(CRED_EXHAUSTED_MSG));
}

// -------------------------------------------------- §6.6 map_remote_err

fn g2err(code: git2::ErrorCode, class: git2::ErrorClass, msg: &str) -> git2::Error {
    git2::Error::new(code, class, msg)
}

/// Callback class + exhaustion sentinel → authFailed (row 1; matches by
/// class+message regardless of code).
#[test]
fn map_cred_exhausted_to_auth_failed() {
    for code in [git2::ErrorCode::Auth, git2::ErrorCode::GenericError] {
        let e = g2err(code, git2::ErrorClass::Callback, CRED_EXHAUSTED_MSG);
        let mapped = map_remote_err(e, "origin");
        match mapped {
            AppError::AuthFailed(m) => {
                assert!(m.contains("'origin'"), "context missing: {m}");
                assert!(m.contains("credential helper"), "guidance missing: {m}");
            }
            other => panic!("expected AuthFailed, got {other:?}"),
        }
    }
}

/// Auth code under any class (Http, Ssh, Net) → authFailed (row 2).
#[test]
fn map_auth_code_to_auth_failed() {
    for class in [
        git2::ErrorClass::Http,
        git2::ErrorClass::Ssh,
        git2::ErrorClass::Net,
    ] {
        let e = g2err(git2::ErrorCode::Auth, class, "401");
        match map_remote_err(e, "origin") {
            AppError::AuthFailed(m) => assert!(m.contains("'origin'")),
            other => panic!("expected AuthFailed for class {class:?}, got {other:?}"),
        }
    }
}

/// NotFastForward (push negotiation) → pushRejected (row 3).
#[test]
fn map_not_fast_forward_to_push_rejected() {
    let e = g2err(
        git2::ErrorCode::NotFastForward,
        git2::ErrorClass::Reference,
        "cannot push non-fastforwardable reference",
    );
    match map_remote_err(e, "origin") {
        AppError::PushRejected(m) => assert!(m.contains("never force-pushes"), "{m}"),
        other => panic!("expected PushRejected, got {other:?}"),
    }
}

/// Non-auth Net / Http / Ssh class → networkError with context +
/// underlying message (rows 4-6).
#[test]
fn map_transport_classes_to_network_error() {
    for class in [
        git2::ErrorClass::Net,
        git2::ErrorClass::Http,
        git2::ErrorClass::Ssh,
    ] {
        let e = g2err(git2::ErrorCode::GenericError, class, "failed to resolve address");
        match map_remote_err(e, "origin") {
            AppError::NetworkError(m) => {
                assert!(m.contains("'origin'"), "context missing: {m}");
                assert!(m.contains("failed to resolve address"), "cause missing: {m}");
            }
            other => panic!("expected NetworkError for class {class:?}, got {other:?}"),
        }
    }
}

/// Anything else → plain git error with the original message (row 7).
#[test]
fn map_other_to_git() {
    let e = g2err(
        git2::ErrorCode::GenericError,
        git2::ErrorClass::None,
        "something odd",
    );
    match map_remote_err(e, "origin") {
        AppError::Git(m) => assert_eq!(m, "something odd"),
        other => panic!("expected Git, got {other:?}"),
    }
}

// --------------- credential_fill (2026-08-04 addendum §A.6)
//
// NOTE: unlike the rest of this suite, these fixtures use plain
// `tempfile::tempdir()` rather than `crate::testutil::scratch_dir()` —
// `scratch_dir()` is hardcoded to the Windows-only `D:\Temp\bonsai-scratch`
// path and panics on macOS/Linux. This substitution is scoped to this
// block only; `scratch_dir()` itself is untouched.
//
// Hermeticity: every test repo FIRST resets `credential.helper` to empty
// (git's documented way to clear the inherited system/global helper list,
// e.g. Git Credential Manager) so `credential_fill` can never fall through
// to a real credential manager and pop an interactive GUI/terminal prompt
// during `cargo test`. Fixtures are `!`-prefixed inline shell commands,
// which git runs via its bundled `sh` on every platform (including
// Git-for-Windows) — no executable bit, shebang, or `.sh` file needed.

use std::process::Command;

fn have_git() -> bool {
    let ok = Command::new("git").arg("--version").output().is_ok();
    if !ok && std::env::var("BONSAI_REQUIRE_GIT_STRICT").as_deref() == Ok("1") {
        panic!("BONSAI_REQUIRE_GIT_STRICT=1: `git` CLI required on PATH but not found");
    }
    ok
}

/// `git config --add <key> <value>` against the repo, asserting success.
fn git_config_add(repo_dir: &Path, key: &str, value: &str) {
    let out = Command::new("git")
        .args(["config", "--add", key, value])
        .current_dir(repo_dir)
        .output()
        .expect("spawn git config");
    assert!(
        out.status.success(),
        "git config --add {key} failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

/// Clears any inherited (system/global — e.g. Git Credential Manager)
/// `credential.helper` entries for this repo by prepending an empty value,
/// git's documented way to reset the helper list. Without this, real `git`
/// consults the system helper FIRST and pops an interactive prompt for the
/// unknown test hosts during `cargo test`.
fn reset_credential_helpers(repo_dir: &Path) {
    git_config_add(repo_dir, "credential.helper", "");
}

/// Inits a scratch git repo via the real `git` CLI (so repo-local
/// `credential.helper` config resolves the same way it would for a real
/// caller) and returns the owning tempdir.
fn credfill_init_repo() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    let out = Command::new("git")
        .args(["init", "-q"])
        .current_dir(dir.path())
        .output()
        .expect("spawn git init");
    assert!(
        out.status.success(),
        "git init failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    dir
}

/// Resets inherited helpers, then registers `helper` as the SOLE helper.
/// `helper` is a `!`-prefixed inline shell command (run via git's bundled
/// `sh` on every platform, including Git-for-Windows), so no executable
/// bit / shebang handling is needed and the fixtures are cross-platform.
fn set_credential_helper(repo_dir: &Path, helper: &str) {
    reset_credential_helpers(repo_dir);
    git_config_add(repo_dir, "credential.helper", helper);
}

/// Inline `!`-helper: responds to `git credential fill` with fixed creds.
const GOOD_HELPER: &str =
    "!f() { echo username=bonsai-test-user; echo password=bonsai-test-pass; }; f";

/// Inline `!`-helper: simulates a broken helper (non-zero exit, no output).
const BAD_EXIT_HELPER: &str = "!f() { exit 1; }; f";

/// Inline `!`-helper: responds but omits the password (simulates a helper
/// that recognizes the request but has no cached secret).
const PARTIAL_HELPER: &str = "!f() { echo username=bonsai-test-user; }; f";

/// (a) Well-formed helper response round-trips through `credential_fill`.
#[test]
fn credential_fill_well_formed_response() {
    if !have_git() {
        return;
    }
    let dir = credfill_init_repo();
    set_credential_helper(dir.path(), GOOD_HELPER);

    let result = credential_fill(Some(dir.path()), "https://example.com/repo.git");
    assert_eq!(
        result,
        FillOutcome::Filled {
            username: "bonsai-test-user".to_string(),
            password: "bonsai-test-pass".to_string(),
        }
    );
}

/// (b) All three failure modes fall through to `NoCredentials` — the
/// helper RAN and had nothing, which is emphatically NOT "git could not be
/// launched" (P70) — without panicking or hanging: non-zero exit, a
/// nonexistent helper binary, and a well-formed-but-incomplete response
/// (missing `password=`).
#[test]
fn credential_fill_failure_modes_return_no_credentials() {
    if !have_git() {
        return;
    }
    let start = std::time::Instant::now();

    let dir = credfill_init_repo();
    set_credential_helper(dir.path(), BAD_EXIT_HELPER);
    assert_eq!(
        credential_fill(Some(dir.path()), "https://example.com/repo.git"),
        FillOutcome::NoCredentials,
        "non-zero exit helper must yield NoCredentials (git DID run)"
    );

    let dir2 = credfill_init_repo();
    set_credential_helper(dir2.path(), "/path/does/not/exist");
    assert_eq!(
        credential_fill(Some(dir2.path()), "https://example.com/repo.git"),
        FillOutcome::NoCredentials,
        "a nonexistent HELPER binary is still `git ran and had nothing`, not \n             GitUnavailable (which means GIT itself could not launch)"
    );

    let dir3 = credfill_init_repo();
    set_credential_helper(dir3.path(), PARTIAL_HELPER);
    assert_eq!(
        credential_fill(Some(dir3.path()), "https://example.com/repo.git"),
        FillOutcome::NoCredentials,
        "response missing password= must yield NoCredentials"
    );

    // Generous ceiling: this case spawns ~9 `git` processes (3 repos ×
    // init + 2 config + fill), which is slow on Windows. The point is to
    // catch an interactive-prompt HANG, which blocks indefinitely — 30s
    // separates "slow subprocess spawns" from "hung waiting on input".
    assert!(
        start.elapsed() < std::time::Duration::from_secs(30),
        "failure-mode cases took too long — possible hang"
    );
}

/// (c) `GIT_TERMINAL_PROMPT=0` prevents an interactive-prompt hang when
/// no `credential.helper` is configured at all. This bounds the hang
/// empirically via wall-clock timing (a genuinely interactive prompt
/// would block indefinitely, not merely run slow) — combined with the
/// source-level `.env("GIT_TERMINAL_PROMPT", "0")` on `credential_fill`'s
/// one `Command::new("git")` construction, this is the practical
/// verification available without spawning and killing a truly-hung
/// child process.
#[test]
fn credential_fill_no_helper_configured_does_not_hang() {
    if !have_git() {
        return;
    }
    let dir = credfill_init_repo();
    // Clear the inherited system/global helper (e.g. GCM) so this "no
    // helper" case truly has none, instead of silently consulting a real
    // credential manager and popping a prompt.
    reset_credential_helpers(dir.path());

    let start = std::time::Instant::now();
    let result = credential_fill(
        Some(dir.path()),
        "https://example-nonexistent-host.invalid/repo.git",
    );
    // A genuine interactive prompt blocks indefinitely; 15s tolerates a
    // slow Windows `git init` + fill while still flagging a real hang.
    assert!(
        start.elapsed() < std::time::Duration::from_secs(15),
        "credential_fill took too long — possible interactive-prompt hang \
         (GIT_TERMINAL_PROMPT not honored?)"
    );
    assert_eq!(result, FillOutcome::NoCredentials);
}
