//! Shared `git` shell-out seam (P58 D1).
//!
//! A richer sibling of [`crate::git::search`]'s `GitRunner`: [`GitExec`] lets a
//! caller pass **stdin** and extra **env** pairs and inspect the FULL
//! [`GitOutput`] (status code + stdout + stderr) rather than only
//! stdout-or-error. Signing (P58) drives `git commit-tree -S` / `git update-ref`
//! through it; P59 (hooks) reuses the same seam.
//!
//! [`SpawnGitExec`] NEVER prompts — `GIT_TERMINAL_PROMPT=0` gates the terminal
//! prompt, `GIT_ASKPASS`/`SSH_ASKPASS` are cleared, AND `-c core.askpass=`
//! neutralizes a *configured* askpass helper (the env vars alone do NOT cover a
//! `core.askpass` set in git config — see `remote.rs::credential_fill`), so the
//! askpass GUI path can't pop a window — and it suppresses the transient console
//! window on Windows. A locked agent (encrypted key, no agent) or a
//! credential-requiring push therefore fails fast to captured stderr, never hangs.

use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

use crate::error::AppError;

/// Captured result of one `git` invocation. `success`/`code` mirror the child's
/// exit status; `stdout`/`stderr` are captured utf8-lossy.
#[derive(Debug, Clone)]
pub struct GitOutput {
    pub success: bool,
    pub code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

/// Injected so argv / stdin / env assembly stays unit-testable without launching
/// git; the oracle tests drive the real [`SpawnGitExec`] against a fixture repo.
pub trait GitExec {
    /// Run `git <args>` in `cwd` with optional `stdin` bytes and extra `env`
    /// pairs. Returns the captured [`GitOutput`] on ANY exit status (a non-zero
    /// exit is NOT an error — the caller inspects `success`/`stderr`); only a
    /// spawn / I/O failure surfaces as [`AppError::Git`].
    fn exec(
        &self,
        args: &[&str],
        cwd: &Path,
        stdin: Option<&[u8]>,
        env: &[(&str, &str)],
    ) -> Result<GitOutput, AppError>;
}

/// Production executor: capture stdout+stderr+status, never prompt, no console
/// window flash on Windows.
pub struct SpawnGitExec;

impl GitExec for SpawnGitExec {
    fn exec(
        &self,
        args: &[&str],
        cwd: &Path,
        stdin: Option<&[u8]>,
        env: &[(&str, &str)],
    ) -> Result<GitOutput, AppError> {
        let mut cmd = Command::new("git");
        // `-c core.askpass=` (before the subcommand) neutralizes a CONFIGURED
        // askpass helper — GIT_TERMINAL_PROMPT=0 + clearing the askpass ENV vars
        // do NOT cover a `core.askpass` set in git config (see
        // remote.rs::credential_fill); a credential-requiring push (P59b
        // force-with-lease) would otherwise hit it as a hidden GUI prompt / hang.
        // `args` is left untouched so the subcmd extraction + builder tests hold.
        cmd.arg("-c")
            .arg("core.askpass=")
            .args(args)
            .current_dir(cwd)
            // Never block on an interactive prompt (terminal OR askpass GUI): a
            // locked signer / credential-requiring push must fail fast to captured
            // stderr (never-prompt policy, mirrors remote.rs::credential_fill).
            .env("GIT_TERMINAL_PROMPT", "0")
            .env_remove("GIT_ASKPASS")
            .env_remove("SSH_ASKPASS")
            .stdin(if stdin.is_some() {
                Stdio::piped()
            } else {
                Stdio::null()
            })
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        // Caller env is applied AFTER the never-prompt defaults so explicit
        // intent (e.g. GIT_AUTHOR_*) wins; signing never passes the askpass keys.
        for (k, v) in env {
            cmd.env(k, v);
        }
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            cmd.creation_flags(CREATE_NO_WINDOW);
        }

        let subcmd = args.first().copied().unwrap_or("");
        let mut child = cmd
            .spawn()
            .map_err(|e| AppError::Git(format!("failed to run `git {subcmd}`: {e}")))?;
        if let Some(bytes) = stdin {
            let write_res = match child.stdin.as_mut() {
                Some(s) => s
                    .write_all(bytes)
                    .map_err(|e| AppError::Git(format!("failed to write `git {subcmd}` stdin: {e}"))),
                None => Err(AppError::Git(format!("failed to open `git {subcmd}` stdin"))),
            };
            if let Err(e) = write_res {
                let _ = child.wait(); // reap instead of leaving a zombie
                return Err(e);
            }
        }
        // wait_with_output drops stdin (EOF) before reading, so the child sees
        // the full request even though we never explicitly closed the handle.
        let output = child
            .wait_with_output()
            .map_err(|e| AppError::Git(format!("failed to wait on `git {subcmd}`: {e}")))?;
        Ok(GitOutput {
            success: output.status.success(),
            code: output.status.code(),
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn have_git() -> bool {
        let ok = Command::new("git").arg("--version").output().is_ok();
        if !ok && std::env::var("BONSAI_REQUIRE_GIT_STRICT").as_deref() == Ok("1") {
            panic!("BONSAI_REQUIRE_GIT_STRICT=1: `git` CLI required on PATH but not found");
        }
        ok
    }

    /// Smoke test the real seam: `git --version` succeeds and its banner is
    /// captured on stdout (proves capture + status wiring end-to-end).
    #[test]
    fn spawn_git_exec_captures_version() {
        if !have_git() {
            eprintln!("skipping: `git` CLI not found");
            return;
        }
        let out = SpawnGitExec
            .exec(&["--version"], Path::new("."), None, &[])
            .expect("git --version");
        assert!(out.success);
        assert_eq!(out.code, Some(0));
        assert!(out.stdout.contains("git version"), "stdout: {}", out.stdout);
    }

    /// A non-zero exit is captured (not an `Err`): `git` with a bogus subcommand
    /// returns `success = false` + a stderr tail for the caller to map.
    #[test]
    fn spawn_git_exec_nonzero_is_captured_not_err() {
        if !have_git() {
            return;
        }
        let out = SpawnGitExec
            .exec(&["not-a-real-subcommand"], Path::new("."), None, &[])
            .expect("spawn ok even on non-zero exit");
        assert!(!out.success);
        assert_ne!(out.code, Some(0));
    }

    /// stdin bytes reach the child: `git hash-object --stdin` hashes exactly the
    /// piped content (deterministic blob oid for "hi\n").
    #[test]
    fn spawn_git_exec_pipes_stdin() {
        if !have_git() {
            return;
        }
        let out = SpawnGitExec
            .exec(
                &["hash-object", "--stdin"],
                Path::new("."),
                Some(b"hi\n"),
                &[],
            )
            .expect("hash-object");
        assert!(out.success, "stderr: {}", out.stderr);
        // git blob sha1 of "hi\n" is stable.
        assert_eq!(out.stdout.trim(), "45b983be36b73c0788dc9cbcb76cbb80fc7bb057");
    }
}
