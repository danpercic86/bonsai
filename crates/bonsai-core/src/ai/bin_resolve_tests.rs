//! Tests for spec 001's macOS/Linux `claude` discovery ladder
//! (`docs/specs/001-macos-claude-cli-path/`). A `#[path]`-included child module of
//! [`super`] (`bin_resolve`), following the `session_pipes_tests` convention, so it
//! reaches the module's private helpers directly instead of widening any of them.
//!
//! Env-mutating tests take [`crate::ai::testutil::env_lock`] for their whole
//! duration: `$SHELL`, `BONSAI_PROBE_MODE`, and `BONSAI_PROBE_FAKE_PATH` are
//! process-global and the probe's child inherits them, so parallel tests would
//! otherwise race (same reason the `CLAUDE_BIN_ENV`/`STUB_MODE_ENV` tests in
//! `testutil.rs` already do this).
//!
//! [`login_shell_path_dirs`]'s `OnceLock` cache (AC4: probe at most once per
//! process) is process-global too. Only [`resolve_finds_binary_via_login_shell_probe_when_not_on_current_path`]
//! below calls [`resolve`] / [`login_shell_path_dirs`] — every other `ai::*` test
//! file always sets `CLAUDE_BIN_ENV` first, which short-circuits `resolve_bin()`
//! before it ever reaches this module (see `ai::resolve_bin`), so nothing else in
//! the suite can observe or race this cache.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use super::*;

/// Force the executable bit on, mirroring `testutil::stub_path` — git doesn't
/// reliably preserve the mode bit across clones/platforms.
fn set_executable(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    let meta = std::fs::metadata(path).expect("fixture/temp file must exist");
    let mut perms = meta.permissions();
    perms.set_mode(perms.mode() | 0o111);
    std::fs::set_permissions(path, perms).expect("failed to set executable bit");
}

fn clear_executable(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    let meta = std::fs::metadata(path).expect("temp file must exist");
    let mut perms = meta.permissions();
    perms.set_mode(perms.mode() & !0o111);
    std::fs::set_permissions(path, perms).expect("failed to clear executable bit");
}

/// The `login_shell_probe_stub.sh` fixture, with its executable bit forced on.
fn login_shell_stub_path() -> PathBuf {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/login_shell_probe_stub.sh");
    set_executable(&path);
    path
}

fn restore_env(key: &str, value: Option<String>) {
    match value {
        Some(v) => std::env::set_var(key, v),
        None => std::env::remove_var(key),
    }
}

/// Snapshot + clear the three env vars the login-shell-probe tests touch, so each
/// test can restore exactly what it found rather than assuming a clean slate.
struct EnvSnapshot {
    shell: Option<String>,
    mode: Option<String>,
    fake_path: Option<String>,
}

impl EnvSnapshot {
    fn capture() -> Self {
        Self {
            shell: std::env::var("SHELL").ok(),
            mode: std::env::var("BONSAI_PROBE_MODE").ok(),
            fake_path: std::env::var("BONSAI_PROBE_FAKE_PATH").ok(),
        }
    }

    fn restore(self) {
        restore_env("SHELL", self.shell);
        restore_env("BONSAI_PROBE_MODE", self.mode);
        restore_env("BONSAI_PROBE_FAKE_PATH", self.fake_path);
    }
}

// ---------------------------------------------------------------------------
// find_in / is_executable_file
// ---------------------------------------------------------------------------

#[test]
fn find_in_matches_only_executable_files() {
    let dir = tempfile::tempdir().expect("tempdir");

    let exe_path = dir.path().join("runnable");
    std::fs::write(&exe_path, "#!/bin/sh\necho hi\n").expect("write exe fixture");
    set_executable(&exe_path);

    let non_exe_path = dir.path().join("not_runnable");
    std::fs::write(&non_exe_path, "definitely not executable").expect("write non-exe fixture");
    clear_executable(&non_exe_path);

    assert!(is_executable_file(&exe_path), "executable file must be recognized");
    assert!(
        !is_executable_file(&non_exe_path),
        "a same-named file that exists but isn't executable must never match"
    );
    assert!(!is_executable_file(&dir.path().join("does_not_exist")));

    let dirs = vec![dir.path().to_path_buf()];
    assert_eq!(find_in(&dirs, "runnable"), Some(exe_path), "find_in must return the executable");
    assert_eq!(
        find_in(&dirs, "not_runnable"),
        None,
        "find_in must not match a non-executable file even though it exists on disk"
    );
    assert_eq!(find_in(&dirs, "does_not_exist"), None, "find_in must miss a name absent from every dir");
}

// ---------------------------------------------------------------------------
// probe_login_shell_path
// ---------------------------------------------------------------------------

#[test]
fn probe_login_shell_path_takes_last_non_empty_line() {
    let _guard = crate::ai::testutil::env_lock();
    let snapshot = EnvSnapshot::capture();

    std::env::set_var("SHELL", login_shell_stub_path());
    std::env::remove_var("BONSAI_PROBE_MODE");
    std::env::set_var("BONSAI_PROBE_FAKE_PATH", "/one/bin:/two/bin");

    let result = probe_login_shell_path();

    snapshot.restore();

    let dirs = result.expect("probe must succeed against the stub login shell");
    assert_eq!(
        dirs,
        vec![PathBuf::from("/one/bin"), PathBuf::from("/two/bin")],
        "probe must parse the LAST non-empty stdout line, ignoring the banner line printed first"
    );
}

#[test]
fn probe_login_shell_path_times_out_on_hung_shell() {
    let _guard = crate::ai::testutil::env_lock();
    let snapshot = EnvSnapshot::capture();

    std::env::set_var("SHELL", login_shell_stub_path());
    std::env::set_var("BONSAI_PROBE_MODE", "hang");
    std::env::remove_var("BONSAI_PROBE_FAKE_PATH");

    let start = Instant::now();
    let result = probe_login_shell_path();
    let elapsed = start.elapsed();

    snapshot.restore();

    assert!(result.is_none(), "a hung shell probe must degrade to None, not surface any PATH");
    assert!(
        elapsed < Duration::from_secs(3),
        "probe must bound its wait near SHELL_PROBE_TIMEOUT ({SHELL_PROBE_TIMEOUT:?}), took {elapsed:?} instead"
    );
}

// ---------------------------------------------------------------------------
// resolve() end-to-end
// ---------------------------------------------------------------------------

#[test]
fn resolve_finds_binary_via_login_shell_probe_when_not_on_current_path() {
    let _guard = crate::ai::testutil::env_lock();

    let dir = tempfile::tempdir().expect("tempdir");
    let program = "bonsai-shell-only-program";
    let program_path = dir.path().join(program);
    std::fs::write(&program_path, "#!/bin/sh\necho hi\n").expect("write program fixture");
    set_executable(&program_path);

    // Sanity: the temp dir must not already be reachable via the current
    // process's own inherited PATH, or this test would prove nothing about the
    // login-shell/fallback tiers.
    assert_eq!(
        find_in(&current_path_dirs(), program),
        None,
        "test setup bug: the fixture program must not already be visible on PATH"
    );

    let snapshot = EnvSnapshot::capture();
    std::env::set_var("SHELL", login_shell_stub_path());
    std::env::remove_var("BONSAI_PROBE_MODE");
    std::env::set_var("BONSAI_PROBE_FAKE_PATH", dir.path().to_str().expect("temp dir path must be utf8"));

    let resolved = resolve(program);

    snapshot.restore();

    assert_eq!(
        resolved, program_path,
        "resolve() must find a binary only reachable via the login-shell PATH tier"
    );
}
