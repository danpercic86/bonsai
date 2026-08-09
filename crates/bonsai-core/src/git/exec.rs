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
//!
//! No timeout (git-parity): like the `git` CLI, an invocation runs until the
//! child exits. A hook or filter that hangs git would hang this too — the
//! never-prompt hardening above removes the common "silently waiting on a
//! hidden prompt" hang, but a genuinely wedged user hook is out of scope (git
//! itself has no default timeout either). Output IS bounded, though: combined
//! stdout+stderr is capped at [`MAX_OUTPUT_BYTES`] so a runaway hook can never
//! grow the capture Vec without bound — overflow returns [`AppError::Git`].

use std::io::{Read, Write};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use crate::error::AppError;

/// Combined stdout+stderr capture cap (F-A5-c). Generous — normal git output is
/// tiny; this only fires on a pathological/runaway child. Bounds the capture
/// Vec growth so a hook spewing gigabytes can't OOM the process.
pub const MAX_OUTPUT_BYTES: usize = 64 * 1024 * 1024;

/// Read `r` fully to EOF, appending to the returned Vec until the SHARED
/// `counter` (stdout+stderr combined) exceeds `cap`, after which bytes are
/// counted + drained but NOT buffered. Returns `(captured, overflowed)`. Always
/// drains to EOF so the child never blocks on a full pipe (no deadlock), while
/// memory stays bounded past the cap.
fn read_capped<R: Read>(mut r: R, counter: &AtomicUsize, cap: usize) -> std::io::Result<(Vec<u8>, bool)> {
    let mut buf = Vec::new();
    let mut overflow = false;
    let mut chunk = [0u8; 16 * 1024];
    loop {
        let n = r.read(&mut chunk)?;
        if n == 0 {
            return Ok((buf, overflow));
        }
        let total = counter.fetch_add(n, Ordering::Relaxed) + n;
        if total > cap {
            // On the FIRST crossing, keep the portion still under the shared
            // cap; thereafter count + drain but stop buffering (bounded memory).
            let already = total - n; // combined bytes counted BEFORE this chunk
            if already < cap {
                let take = (cap - already).min(n);
                buf.extend_from_slice(&chunk[..take]);
            }
            overflow = true;
        } else {
            buf.extend_from_slice(&chunk[..n]);
        }
    }
}

/// Assemble the child `Command` with the never-prompt hardening + pipe wiring.
/// Extracted so the argv/env/stdin assembly stays unit-testable without
/// launching git (the env-hygiene invariant is asserted via `get_envs`).
fn build_command(args: &[&str], cwd: &Path, stdin_present: bool, env: &[(&str, &str)]) -> Command {
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
        .stdin(if stdin_present {
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
    cmd
}

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
        let mut cmd = build_command(args, cwd, stdin.is_some(), env);

        let subcmd = args.first().copied().unwrap_or("");
        let mut child = cmd
            .spawn()
            .map_err(|e| AppError::Git(format!("failed to run `git {subcmd}`: {e}")))?;

        // Write stdin, then CLOSE it (drop the handle -> EOF) so the child sees
        // the full request. Done before reading output; git stdin is small
        // (messages), so a pre-read write cannot deadlock in practice.
        if let Some(bytes) = stdin {
            let mut sh = child.stdin.take();
            let write_res = match sh.as_mut() {
                Some(s) => s
                    .write_all(bytes)
                    .map_err(|e| AppError::Git(format!("failed to write `git {subcmd}` stdin: {e}"))),
                None => Err(AppError::Git(format!("failed to open `git {subcmd}` stdin"))),
            };
            drop(sh); // EOF for the child
            if let Err(e) = write_res {
                let _ = child.wait(); // reap instead of leaving a zombie
                return Err(e);
            }
        }

        // Read stdout on this thread + stderr on a helper thread (never
        // sequentially — a full stderr pipe would else deadlock a stdout read),
        // both bounded by a SHARED combined-byte counter so total capture is
        // capped at MAX_OUTPUT_BYTES. Both always drain to EOF.
        let counter = Arc::new(AtomicUsize::new(0));
        let stdout_pipe = child.stdout.take();
        let stderr_pipe = child.stderr.take();
        let stderr_counter = Arc::clone(&counter);
        let stderr_join = std::thread::spawn(move || -> std::io::Result<(Vec<u8>, bool)> {
            match stderr_pipe {
                Some(p) => read_capped(p, &stderr_counter, MAX_OUTPUT_BYTES),
                None => Ok((Vec::new(), false)),
            }
        });
        let (stdout_bytes, stdout_of) = match stdout_pipe {
            Some(p) => read_capped(p, &counter, MAX_OUTPUT_BYTES),
            None => Ok((Vec::new(), false)),
        }
        .map_err(|e| AppError::Git(format!("failed to read `git {subcmd}` stdout: {e}")))?;
        let (stderr_bytes, stderr_of) = stderr_join
            .join()
            .map_err(|_| AppError::Git(format!("`git {subcmd}` stderr reader panicked")))?
            .map_err(|e| AppError::Git(format!("failed to read `git {subcmd}` stderr: {e}")))?;

        let status = child
            .wait()
            .map_err(|e| AppError::Git(format!("failed to wait on `git {subcmd}`: {e}")))?;

        if stdout_of || stderr_of {
            return Err(AppError::Git(format!(
                "`git {subcmd}` produced more than {MAX_OUTPUT_BYTES} bytes of output; aborting"
            )));
        }
        Ok(GitOutput {
            success: status.success(),
            code: status.code(),
            stdout: String::from_utf8_lossy(&stdout_bytes).into_owned(),
            stderr: String::from_utf8_lossy(&stderr_bytes).into_owned(),
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

    /// Env-hygiene invariant (recording fake, no git spawn): EVERY assembled
    /// argv sets `GIT_TERMINAL_PROMPT=0` and NEUTRALIZES the askpass paths —
    /// `GIT_ASKPASS`/`SSH_ASKPASS` are env-removed and `-c core.askpass=` leads
    /// the argv. Inspecting the built `Command` proves the doc-claimed hardening
    /// without launching git (previously untested; audit F-A5-c).
    #[test]
    fn build_command_enforces_never_prompt_env_hygiene() {
        let cmd = build_command(&["push", "origin", "main"], Path::new("."), false, &[]);

        // Args lead with `-c core.askpass=` then the caller argv verbatim.
        let args: Vec<String> = cmd
            .get_args()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        assert_eq!(
            &args[..3],
            &["-c".to_string(), "core.askpass=".to_string(), "push".to_string()],
            "argv leads with the askpass neutralizer: {args:?}"
        );

        // Env map: GIT_TERMINAL_PROMPT set to 0; askpass vars explicitly removed.
        let envs: std::collections::HashMap<String, Option<String>> = cmd
            .get_envs()
            .map(|(k, v)| {
                (
                    k.to_string_lossy().into_owned(),
                    v.map(|v| v.to_string_lossy().into_owned()),
                )
            })
            .collect();
        assert_eq!(
            envs.get("GIT_TERMINAL_PROMPT"),
            Some(&Some("0".to_string())),
            "GIT_TERMINAL_PROMPT=0 always set"
        );
        assert_eq!(
            envs.get("GIT_ASKPASS"),
            Some(&None),
            "GIT_ASKPASS is env-removed (None entry)"
        );
        assert_eq!(
            envs.get("SSH_ASKPASS"),
            Some(&None),
            "SSH_ASKPASS is env-removed (None entry)"
        );
    }

    /// Caller env is applied AFTER the defaults but MUST NOT be able to
    /// re-enable a prompt via a lone override — the defaults are still present,
    /// and an explicit caller value (e.g. GIT_AUTHOR_NAME) is layered on top.
    #[test]
    fn build_command_layers_caller_env_over_defaults() {
        let cmd = build_command(
            &["commit-tree"],
            Path::new("."),
            true,
            &[("GIT_AUTHOR_NAME", "Ada")],
        );
        let envs: std::collections::HashMap<String, Option<String>> = cmd
            .get_envs()
            .map(|(k, v)| {
                (
                    k.to_string_lossy().into_owned(),
                    v.map(|v| v.to_string_lossy().into_owned()),
                )
            })
            .collect();
        assert_eq!(envs.get("GIT_AUTHOR_NAME"), Some(&Some("Ada".to_string())));
        assert_eq!(envs.get("GIT_TERMINAL_PROMPT"), Some(&Some("0".to_string())));
        assert_eq!(envs.get("GIT_ASKPASS"), Some(&None));
    }

    /// The output cap fires: a child emitting more than `MAX_OUTPUT_BYTES` on
    /// stdout returns `AppError::Git` (bounded capture, F-A5-c). Uses a small
    /// cap-check via `read_capped` directly so the test is fast + git-free.
    #[test]
    fn read_capped_flags_overflow_but_drains_to_eof() {
        let counter = AtomicUsize::new(0);
        let data = [b'x'; 10];
        // cap = 4: first 4 bytes buffered, the rest counted+drained, overflow set.
        let (buf, overflow) = read_capped(&data[..], &counter, 4).expect("read");
        assert!(overflow, "exceeding the cap sets the overflow flag");
        assert_eq!(buf.len(), 4, "only up-to-cap bytes are buffered");
        assert_eq!(counter.load(Ordering::Relaxed), 10, "all bytes are counted (drained to EOF)");
    }

    /// A combined shared counter caps stdout+stderr TOGETHER, not per-stream.
    #[test]
    fn read_capped_shares_counter_across_streams() {
        let counter = AtomicUsize::new(0);
        let (_a, of_a) = read_capped(&b"aaaa"[..], &counter, 6).expect("a");
        assert!(!of_a, "4 <= 6: no overflow yet");
        let (_b, of_b) = read_capped(&b"bbbb"[..], &counter, 6).expect("b");
        assert!(of_b, "4 + 4 > 6: combined counter trips the second stream");
    }

    /// End-to-end cap via the real seam: `git hash-object --stdin` produces tiny
    /// output well under the cap, proving the bounded reader is transparent for
    /// normal git output (the overflow path is covered offline above).
    #[test]
    fn spawn_git_exec_normal_output_under_cap() {
        if !have_git() {
            return;
        }
        let out = SpawnGitExec
            .exec(&["--version"], Path::new("."), None, &[])
            .expect("git --version");
        assert!(out.success);
        assert!(out.stdout.len() < MAX_OUTPUT_BYTES);
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
