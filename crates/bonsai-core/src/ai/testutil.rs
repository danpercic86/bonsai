//! Shared stub-CLI harness for the `ai::*` unit tests (P13 harness, extracted in
//! P68a so `tests.rs` and `session_tests.rs` drive the same fixture the same way).

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread;
use std::time::{Duration, Instant};

use super::{AiRunEvent, AiRunEventKind, CLAUDE_BIN_ENV};

/// Which behaviour `claude_stub.{cmd,sh}` should act out.
pub const STUB_MODE_ENV: &str = "BONSAI_STUB_MODE";
/// `stream_slow` and `stream_hang_stdin` TICK this file (append one line about once
/// a second) for as long as they live, so a test proves the child really died
/// (P68 §10.1) with [`assert_child_is_dead`] — no assumption about how long the
/// kill took, which a one-shot marker needed and got wrong under load.
pub const STUB_MARKER_ENV: &str = "BONSAI_STUB_MARKER";

/// One stub tick, plus slack: how long to wait for a survivor to give itself away.
const MARKER_TICK: Duration = Duration::from_millis(1600);

/// Serialize env-mutating tests: `BONSAI_CLAUDE_BIN` / `BONSAI_STUB_MODE` are
/// process-global and the stub inherits them, so parallel tests would race.
pub fn env_lock() -> MutexGuard<'static, ()> {
    static LOCK: Mutex<()> = Mutex::new(());
    LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

/// Windows runs the `.cmd` stub directly (`Command::new` routes `.cmd` through
/// cmd.exe automatically). macOS/Linux use the POSIX `.sh` twin, with the
/// executable bit forced on at test time — git doesn't reliably preserve the mode
/// bit across clones/platforms.
pub fn stub_path() -> PathBuf {
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    if cfg!(windows) {
        fixtures.join("claude_stub.cmd")
    } else {
        let path = fixtures.join("claude_stub.sh");
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

pub fn set_mode(mode: &str) {
    std::env::set_var(CLAUDE_BIN_ENV, stub_path());
    std::env::set_var(STUB_MODE_ENV, mode);
    std::env::remove_var(STUB_MARKER_ENV);
}

/// `set_mode` + the survival-marker path for the cancel/reap assertions. Removes a
/// stale file from an earlier run so the first tick is unambiguous.
pub fn set_mode_with_marker(mode: &str, marker: &Path) {
    set_mode(mode);
    let _ = std::fs::remove_file(marker);
    std::env::set_var(STUB_MARKER_ENV, marker);
}

/// Prove the stub child (and its `ping`/`sleep` grandchild) is gone: delete whatever
/// it ticked while it was alive, then require the file to STAY gone for longer than
/// one tick. Call only AFTER the run returned — the session reaps the direct child
/// before that, so no tick can still be in flight.
///
/// Deliberately not "the marker never appeared": a loaded machine can let the child
/// tick once before the kill lands, which says nothing about whether it survived.
pub fn assert_child_is_dead(marker: &Path) {
    let _ = std::fs::remove_file(marker);
    thread::sleep(MARKER_TICK);
    assert!(
        !marker.exists(),
        "the killed child (or its grandchild) is still alive and ticked {marker:?}"
    );
}

/// A per-test marker path under the temp dir (the pid keeps concurrent test binaries
/// from colliding).
pub fn marker_path(tag: &str) -> PathBuf {
    std::env::temp_dir().join(format!("bonsai-p68a-{tag}-{}.marker", std::process::id()))
}

/// Thread-safe event collector standing in for the Tauri Channel. Shared by the
/// streaming test files (`session_tests`, `session_io_tests`).
#[derive(Clone, Default)]
pub struct Sink(Arc<Mutex<Vec<AiRunEvent>>>);

impl Sink {
    fn lock(&self) -> MutexGuard<'_, Vec<AiRunEvent>> {
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }
    pub fn push(&self, ev: AiRunEvent) {
        self.lock().push(ev);
    }
    pub fn len(&self) -> usize {
        self.lock().len()
    }
    pub fn events(&self) -> Vec<AiRunEvent> {
        self.lock().clone()
    }
    pub fn kinds(&self) -> Vec<AiRunEventKind> {
        self.lock().iter().map(|e| e.kind).collect()
    }
    pub fn texts(&self) -> Vec<String> {
        self.lock().iter().filter_map(|e| e.text.clone()).collect()
    }
    pub fn has_text(&self, needle: &str) -> bool {
        self.texts().iter().any(|t| t.contains(needle))
    }
    pub fn of_kind(&self, kind: AiRunEventKind) -> Vec<AiRunEvent> {
        self.lock().iter().filter(|e| e.kind == kind).cloned().collect()
    }
}

/// Poll `cond` every 25 ms until it holds or `timeout` elapses.
pub fn wait_until(mut cond: impl FnMut() -> bool, timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if cond() {
            return true;
        }
        thread::sleep(Duration::from_millis(25));
    }
    cond()
}
