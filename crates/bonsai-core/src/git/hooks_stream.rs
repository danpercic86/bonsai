//! P87 streaming hook variants (split from `hooks.rs` for the ~500-line limit).
//!
//! `run_hook_streaming` / `run_hook_nonblocking_streaming` are [`super::hooks`]'s
//! `run_hook` / `run_hook_nonblocking` PLUS an optional [`GitActivityRecorder`]:
//! when present they emit a `RunningHook` phase, stream the hook's stdout/stderr
//! lines through the exec seam, and record a `hook_done`. The classification
//! (Ok / Git infra / [`AppError::HookRejected`]) and the combined-output body are
//! **byte-for-byte unchanged** whether lines streamed or not; `activity == None`
//! is the exact pre-P87 buffered path. All shared helpers live in `hooks.rs`.

use std::path::Path;

use crate::error::AppError;
use crate::git::activity::{GitActivityRecorder, GitPhaseKind, GitStream};
use crate::git::exec::{GitExec, LineSink};
use crate::git::hooks::{
    build_hook_run_args, combined_output, is_git_infra_failure, is_unknown_subcommand, plan_hook,
    write_stdin_tempfile, HookName, HookPlan, HookRunInfo, TempStdin,
};

/// Adapter driving a [`GitActivityRecorder`] from the exec seam's [`LineSink`]
/// (P87 §3). `line` runs only on the caller thread, so no `Sync` bound is needed.
struct RecorderSink<'a>(&'a dyn GitActivityRecorder);

impl LineSink for RecorderSink<'_> {
    fn line(&self, stream: GitStream, line: &str) {
        self.0.line(stream, line);
    }
}

/// See the module doc. `activity == None` ≡ [`super::hooks::run_hook`].
pub fn run_hook_streaming(
    exec: &dyn GitExec,
    workdir: &Path,
    hook: HookName,
    args: &[String],
    stdin: Option<&[u8]>,
    activity: Option<&dyn GitActivityRecorder>,
) -> Result<(), AppError> {
    if matches!(plan_hook(workdir, hook), HookPlan::Skip) {
        return Ok(()); // no hook file, no core.hooksPath ⇒ nothing to run (no phase/events)
    }
    if let Some(a) = activity {
        a.phase(GitPhaseKind::RunningHook, Some(hook.as_str()));
    }
    // `git hook run` does not forward its own stdin to the hook; a hook that
    // reads stdin (pre-push) gets it via a temp file passed as --to-stdin. The
    // handle deletes the file on drop, AFTER exec completes.
    let stdin_tmp = match stdin {
        Some(bytes) => Some(write_stdin_tempfile(hook, bytes)?),
        None => None,
    };
    let argv = build_hook_run_args(hook, args, stdin_tmp.as_ref().map(TempStdin::path));
    let argv_ref: Vec<&str> = argv.iter().map(String::as_str).collect();
    // Streaming path only when a recorder is present; otherwise the exact
    // pre-P87 buffered call. Either way `out` is the SAME full GitOutput.
    let out = match activity {
        Some(a) => exec.exec_streaming(&argv_ref, workdir, None, &[], &RecorderSink(a))?,
        None => exec.exec(&argv_ref, workdir, None, &[])?,
    };
    if let Some(a) = activity {
        a.hook_done(hook.as_str(), out.code, out.success);
    }
    if out.success {
        return Ok(());
    }
    if is_unknown_subcommand(&out.stderr) {
        // Git < 2.36 has no `hook` subcommand. `plan_hook` already confirmed a
        // hook exists (else Skip), so refuse rather than bypass it silently.
        return Err(AppError::Git(format!(
            "hook execution needs Git ≥ 2.36 (this git cannot run the '{}' hook). \
             Upgrade git, or disable hooks (unset bonsai.runHooks / use Skip hooks).",
            hook.as_str()
        )));
    }
    if is_git_infra_failure(&out.stderr) {
        // git itself failed BEFORE the hook ran (F-A4-5) — not a rejection.
        return Err(AppError::Git(format!(
            "git could not run the {} hook: {}",
            hook.as_str(),
            combined_output(&out.stdout, &out.stderr)
        )));
    }
    Err(AppError::HookRejected(format!(
        "{} hook failed:\n{}",
        hook.as_str(),
        combined_output(&out.stdout, &out.stderr)
    )))
}

/// See the module doc. `activity == None` ≡ [`super::hooks::run_hook_nonblocking`].
/// A hook that did NOT run (Git < 2.36 unknown subcommand, or a spawn failure)
/// emits no `hook_done`.
pub fn run_hook_nonblocking_streaming(
    exec: &dyn GitExec,
    workdir: &Path,
    hook: HookName,
    args: &[String],
    activity: Option<&dyn GitActivityRecorder>,
) -> HookRunInfo {
    if matches!(plan_hook(workdir, hook), HookPlan::Skip) {
        return HookRunInfo { ran: false, success: true, output: String::new() };
    }
    if let Some(a) = activity {
        a.phase(GitPhaseKind::RunningHook, Some(hook.as_str()));
    }
    let argv = build_hook_run_args(hook, args, None);
    let argv_ref: Vec<&str> = argv.iter().map(String::as_str).collect();
    let result = match activity {
        Some(a) => exec.exec_streaming(&argv_ref, workdir, None, &[], &RecorderSink(a)),
        None => exec.exec(&argv_ref, workdir, None, &[]),
    };
    match result {
        Ok(out) if out.success => {
            if let Some(a) = activity {
                a.hook_done(hook.as_str(), out.code, true);
            }
            HookRunInfo {
                ran: true,
                success: true,
                output: combined_output(&out.stdout, &out.stderr),
            }
        }
        Ok(out) if is_unknown_subcommand(&out.stderr) => HookRunInfo {
            ran: false,
            success: false,
            output: format!(
                "failed to run the {} hook: this git has no `hook run` subcommand (Git ≥ 2.36 required)",
                hook.as_str()
            ),
        },
        Ok(out) => {
            if let Some(a) = activity {
                a.hook_done(hook.as_str(), out.code, false);
            }
            HookRunInfo {
                ran: true,
                success: false,
                output: combined_output(&out.stdout, &out.stderr),
            }
        }
        // Audit #2 §3.3: carry the spawn/I/O error so the caller can surface it —
        // an empty output here was indistinguishable from "no hook installed".
        Err(e) => HookRunInfo {
            ran: false,
            success: false,
            output: format!("failed to run the {} hook: {e}", hook.as_str()),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::exec::SpawnGitExec;
    use std::path::PathBuf;
    use std::process::Command;

    // ---- oracle helpers (git ≥ 2.36 only) ---------------------------------

    fn oracle_ready() -> bool {
        let out = match Command::new("git").arg("--version").output() {
            Ok(o) if o.status.success() => o,
            _ => {
                if std::env::var("BONSAI_REQUIRE_GIT_STRICT").as_deref() == Ok("1") {
                    panic!("BONSAI_REQUIRE_GIT_STRICT=1: `git` CLI required on PATH but not found");
                }
                eprintln!("skipping hooks_stream oracle: `git` not found");
                return false;
            }
        };
        let s = String::from_utf8_lossy(&out.stdout);
        let ver = s.split_whitespace().nth(2).unwrap_or("");
        let mut it = ver.split('.');
        let maj: u32 = it.next().and_then(|x| x.parse().ok()).unwrap_or(0);
        let min: u32 = it.next().and_then(|x| x.parse().ok()).unwrap_or(0);
        if maj > 2 || (maj == 2 && min >= 36) {
            true
        } else {
            eprintln!("skipping hooks_stream oracle: git < 2.36 (no `git hook run`)");
            false
        }
    }

    fn init_repo(dir: &Path) -> git2::Repository {
        let repo = git2::Repository::init(dir).expect("init");
        let mut cfg = repo.config().expect("config");
        cfg.set_str("user.name", "Hook Tester").expect("name");
        cfg.set_str("user.email", "hooks@example.com").expect("email");
        cfg.set_bool("core.autocrlf", false).expect("autocrlf");
        drop(cfg);
        repo
    }

    fn hooks_dir(repo: &git2::Repository) -> PathBuf {
        repo.commondir().join("hooks")
    }

    fn write_hook(dir: &Path, name: &str, body: &str) {
        std::fs::create_dir_all(dir).expect("mkdir hooks");
        let path = dir.join(name);
        std::fs::write(&path, body.replace("\r\n", "\n")).expect("write hook");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(&path).expect("meta").permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(&path, perms).expect("chmod");
        }
    }

    /// A recorder that logs every callback as a compact string so a test can
    /// assert the exact activity sequence.
    #[derive(Default)]
    struct RecordingRecorder {
        events: std::sync::Mutex<Vec<String>>,
    }
    impl RecordingRecorder {
        fn snapshot(&self) -> Vec<String> {
            self.events.lock().expect("lock").clone()
        }
    }
    impl GitActivityRecorder for RecordingRecorder {
        fn phase(&self, kind: GitPhaseKind, hook: Option<&str>) {
            self.events
                .lock()
                .expect("lock")
                .push(format!("phase:{kind:?}:{}", hook.unwrap_or("-")));
        }
        fn line(&self, stream: GitStream, line: &str) {
            self.events
                .lock()
                .expect("lock")
                .push(format!("line:{stream:?}:{line}"));
        }
        fn hook_done(&self, hook: &str, code: Option<i32>, success: bool) {
            self.events
                .lock()
                .expect("lock")
                .push(format!("hookDone:{hook}:{code:?}:{success}"));
        }
    }

    /// A PASSING blocking hook streams its output lines, records a
    /// `hook_done{success:true}`, and opens with a `RunningHook` phase — and it
    /// still returns `Ok(())`.
    #[test]
    fn passing_hook_streams_lines_and_hook_done() {
        if !oracle_ready() {
            return;
        }
        let dir = crate::testutil::scratch_dir();
        let repo = init_repo(dir.path());
        write_hook(
            &hooks_dir(&repo),
            "pre-commit",
            "#!/bin/sh\necho first line\necho second line\nexit 0\n",
        );
        let rec = RecordingRecorder::default();
        run_hook_streaming(&SpawnGitExec, dir.path(), HookName::PreCommit, &[], None, Some(&rec))
            .expect("passing hook ⇒ Ok");
        let events = rec.snapshot();
        assert_eq!(events.first().map(String::as_str), Some("phase:RunningHook:pre-commit"));
        // `git hook run` routes a hook's own stdout/stderr to the child's stderr,
        // so assert the line TEXT streamed, not which captured stream carried it.
        assert!(
            events.iter().any(|e| e.starts_with("line:") && e.ends_with(":first line")),
            "first hook line must stream: {events:?}"
        );
        assert!(
            events.iter().any(|e| e.starts_with("line:") && e.ends_with(":second line")),
            "second hook line must stream: {events:?}"
        );
        assert!(
            events.iter().any(|e| e == "hookDone:pre-commit:Some(0):true"),
            "a success hook_done must be recorded: {events:?}"
        );
    }

    /// A FAILING blocking hook still produces the FULL combined output in
    /// `HookRejected` (byte-identical to the buffered `None` path) AND records a
    /// `hook_done{success:false}` — the two paths are independent (§9).
    #[test]
    fn failing_hook_keeps_full_hook_rejected_output_and_matches_buffered() {
        if !oracle_ready() {
            return;
        }
        let dir = crate::testutil::scratch_dir();
        let repo = init_repo(dir.path());
        write_hook(
            &hooks_dir(&repo),
            "pre-commit",
            "#!/bin/sh\necho stdout-tell\necho stderr-tell >&2\nexit 3\n",
        );
        let rec = RecordingRecorder::default();
        let streamed =
            run_hook_streaming(&SpawnGitExec, dir.path(), HookName::PreCommit, &[], None, Some(&rec))
                .expect_err("failing hook ⇒ HookRejected");
        match &streamed {
            AppError::HookRejected(m) => {
                assert!(m.starts_with("pre-commit hook failed:"), "prefix: {m}");
                assert!(m.contains("stdout-tell"), "stdout in body: {m}");
                assert!(m.contains("stderr-tell"), "stderr in body: {m}");
            }
            other => panic!("expected HookRejected, got {other:?}"),
        }
        assert!(
            rec.snapshot().iter().any(|e| e == "hookDone:pre-commit:Some(3):false"),
            "a failed hook_done must be recorded"
        );

        // Byte-identity: the SAME hook through the None (buffered) path yields the
        // identical HookRejected message.
        let buffered =
            run_hook_streaming(&SpawnGitExec, dir.path(), HookName::PreCommit, &[], None, None)
                .expect_err("buffered path also rejects");
        match (streamed, buffered) {
            (AppError::HookRejected(s), AppError::HookRejected(b)) => {
                assert_eq!(s, b, "streamed + buffered HookRejected bodies must match");
            }
            other => panic!("both paths must reject identically: {other:?}"),
        }
    }
}
