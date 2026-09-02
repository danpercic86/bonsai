//! P91 — observability: structured Dev-mode logs (§1 module map).
//!
//! This barrel holds only wiring: the managed [`ObsState`], the log-directory
//! resolver, and the one function that turns `DevSettings` into a running (or
//! stopped) sink. Schema lives in [`record`], redaction in [`redact`], the
//! non-blocking queue in [`sink`], bytes-on-disk in [`writer`].
//!
//! **Two guarantees this module exists to keep:**
//! 1. *Nothing here can block a git or UI path.* Producers `try_send`; only
//!    lifecycle calls (start/stop) touch IO, and they run on `spawn_blocking` or
//!    the exit hook.
//! 2. *Nothing here deletes a log file automatically except §6 pruning at session
//!    start.* Turning Dev mode OFF stops the sink and leaves every byte on disk
//!    (decision 4) — see [`stop`].

pub mod anomaly;
pub mod fs_perm;
pub mod histogram;
pub mod invoke_shim;
pub mod metrics;
mod metrics_cmds;
pub mod metrics_file;
mod metrics_keys;
mod metrics_map;
pub mod phase;
pub mod raw_args;
pub mod record;
pub mod redact;
pub mod scrub;
pub mod sink;
pub mod strict;
pub mod trace;
pub mod writer;
mod writer_files;

pub use trace::{emit_logged, TraceMeta};

#[cfg(test)]
#[path = "tests_redact.rs"]
mod tests_redact;

#[cfg(test)]
#[path = "tests_writer.rs"]
mod tests_writer;

#[cfg(test)]
#[path = "tests_sink.rs"]
mod tests_sink;

#[cfg(test)]
#[path = "tests_purge.rs"]
mod tests_purge;

#[cfg(test)]
#[path = "tests_strict.rs"]
mod tests_strict;

#[cfg(test)]
#[path = "tests_raw_args.rs"]
mod tests_raw_args;

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use bonsai_core::error::AppError;

pub use metrics::MetricsState;
pub use record::{LogLevel, LogRecord, RedactionMode};
pub use sink::Sink;

use crate::settings::DevSettings;

/// The managed observability state (§1: "`ObsState` held in `AppState`").
///
/// `None` ⇒ Dev mode is off and there is no writer thread at all — the §11
/// "Dev mode OFF" budget is met structurally, not by a fast path.
#[derive(Default)]
pub struct ObsState {
    session: Mutex<Option<Arc<Sink>>>,
}

impl ObsState {
    fn lock(&self) -> std::sync::MutexGuard<'_, Option<Arc<Sink>>> {
        self.session
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// A handle to the live sink, if any. The `Arc` is cloned OUT under the lock
    /// so no caller ever holds the mutex across an enqueue or a write.
    pub fn sink(&self) -> Option<Arc<Sink>> {
        self.lock().clone()
    }

    /// True while a session is recording.
    pub fn is_enabled(&self) -> bool {
        self.lock().is_some()
    }

    /// TEST ONLY — production goes through [`apply_dev_settings`], which needs an
    /// `AppHandle` to resolve the config dir.
    #[cfg(test)]
    pub fn from_sink(sink: Sink) -> Self {
        ObsState {
            session: Mutex::new(Some(Arc::new(sink))),
        }
    }
}

/// `<app_config_dir>/logs` — sibling to `settings.json` (§6).
pub fn logs_dir(app: &tauri::AppHandle) -> Result<PathBuf, AppError> {
    use tauri::Manager;
    let dir = app
        .path()
        .app_config_dir()
        .map_err(|e| AppError::Other(format!("cannot resolve app config dir: {e}")))?;
    Ok(dir.join("logs"))
}

/// `<app_config_dir>/metrics` — where §8 `usage.json` lives, a sibling of
/// `logs/` and `settings.json`.
pub fn metrics_dir(app: &tauri::AppHandle) -> Result<PathBuf, AppError> {
    use tauri::Manager;
    let dir = app
        .path()
        .app_config_dir()
        .map_err(|e| AppError::Other(format!("cannot resolve app config dir: {e}")))?;
    Ok(dir.join("metrics"))
}

/// A session id that is guaranteed NOT to be all digits.
///
/// That matters: [`writer::session_group`] splits a rotation part off the file
/// name by looking for a trailing all-numeric segment, so a numeric session id
/// would make `bonsai-<stamp>-1234.jsonl` parse as "part 1234 of session
/// `bonsai-<stamp>`" and prune the wrong files. The `s` prefix removes the
/// ambiguity by construction rather than by luck.
fn new_session_id() -> String {
    let n: u32 = rand::random();
    format!("s{n:08x}")
}

/// Starts, stops or restarts the sink to match `dev` (§10 — "takes effect
/// immediately, no restart").
///
/// A running session is RESTARTED when the redaction mode or the level changes:
/// a file never mixes redaction modes (§7.3) and its `session` header states the
/// level, so continuing to append under a changed setting would produce a file
/// whose own header lies about it. Every other change is inert here.
///
/// **The `ObsState` lock is held across the WHOLE transition.** Two concurrent
/// `set_ui_settings` calls (a toggle plus a level change) would otherwise
/// interleave their read/stop/reassign steps and leave two writer threads
/// appending to two files, or a live sink orphaned with no handle to stop it.
/// The lock is never held across a record write — only across this lifecycle
/// step, which is already off the UI thread on `spawn_blocking`.
///
/// Blocking (it opens a file); call from `spawn_blocking` or `setup`.
pub fn apply_dev_settings(
    app: &tauri::AppHandle,
    obs: &ObsState,
    dev: &DevSettings,
) -> Result<(), AppError> {
    let wanted_redaction = if dev.include_raw_names {
        RedactionMode::Raw
    } else {
        RedactionMode::Strict
    };
    let mut slot = obs.lock();
    if !dev.enabled {
        if let Some(s) = slot.take() {
            s.shutdown();
        }
        trace::set_active_sink(None);
        return Ok(());
    }
    if let Some(existing) = slot.as_ref() {
        if existing.redaction() == wanted_redaction && existing.level() == dev.level {
            return Ok(());
        }
    }
    if let Some(s) = slot.take() {
        s.shutdown();
    }
    let dir = logs_dir(app)?;
    let cfg = writer::WriterConfig {
        dir,
        session_id: new_session_id(),
        started_secs: writer::now_secs(),
        app_version: app.package_info().version.to_string(),
        os: std::env::consts::OS.to_string(),
        level: dev.level,
        redaction: wanted_redaction,
        limits: writer::Limits::default(),
    };
    let sink = Arc::new(Sink::start(cfg)?);
    arm_panic_flush(&sink);
    trace::set_active_sink(Some(Arc::clone(&sink)));
    *slot = Some(sink);
    Ok(())
}

/// Weak handle to the live sink, for the panic hook only.
///
/// `Weak` deliberately: an `Arc` here would keep the writer thread alive after
/// Dev mode is switched off, which is exactly the leak §11's "no writer thread
/// spawned" budget forbids.
static PANIC_SINK: Mutex<Option<std::sync::Weak<Sink>>> = Mutex::new(None);

/// Installs (once) a panic hook that flushes buffered records before unwinding
/// (§6). It only ever *requests* a flush — a non-blocking `try_send` — because
/// the panicking thread may BE the writer thread, and a shutdown handshake from
/// there would wait on itself forever.
///
/// **Known limit:** it SCHEDULES a flush rather than performing one, so the
/// writer must still get scheduled to make it good. On an unwinding panic that
/// is reliable; on `panic = "abort"` — or a hard kill — up to one flush window
/// (64 KB buffered, or 1 s) can still be lost. Fixing that would require the
/// hook to write the file itself, which is precisely the self-join deadlock
/// above.
fn arm_panic_flush(sink: &Arc<Sink>) {
    static ARMED: std::sync::OnceLock<()> = std::sync::OnceLock::new();
    *PANIC_SINK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(Arc::downgrade(sink));
    ARMED.get_or_init(|| {
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            let weak = PANIC_SINK
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .clone();
            if let Some(s) = weak.and_then(|w| w.upgrade()) {
                s.request_flush();
            }
            previous(info);
        }));
    });
}

/// Stops the session: flush, ack, join, drop.
///
/// **Deletes nothing.** §6 decision 4 forbids delete-on-disable; the user's whole
/// workflow is exporting the file *after* turning Dev mode back off.
pub fn stop(obs: &ObsState) {
    let taken = obs.lock().take();
    if let Some(s) = taken {
        s.shutdown();
    }
    trace::set_active_sink(None);
}

/// Flush + stop for `RunEvent::ExitRequested` (§6). Safe to call when Dev mode
/// was never on.
pub fn shutdown_on_exit(obs: &ObsState) {
    stop(obs);
}

/// `<app_config_dir>/exports` — where `log_export_session(None)` writes (§6.2).
///
/// **Deliberately NOT `logs/`.** An exported zip of a `raw`-names session is the
/// same privacy artifact as the log it came from; keeping exports in their own
/// app-managed directory is what lets `logs_delete_all` (increment 7) cover them
/// while leaving `logs/` holding nothing but the writer's own `*.jsonl` files, so
/// rotation and pruning never have to reason about foreign file types.
pub fn exports_dir(app: &tauri::AppHandle) -> Result<PathBuf, AppError> {
    use tauri::Manager;
    let dir = app
        .path()
        .app_config_dir()
        .map_err(|e| AppError::Other(format!("cannot resolve app config dir: {e}")))?;
    Ok(dir.join("exports"))
}
