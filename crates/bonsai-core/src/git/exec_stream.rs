//! P87 streaming exec seam: [`SpawnGitExec::exec_streaming`]'s body.
//!
//! Split from `exec.rs` (file-size discipline, contract §6). Reuses `exec.rs`'s
//! `build_command` + the shared 64 MiB counter and returns a [`GitOutput`]
//! **byte-identical** to buffered `exec` for the same child (for output under the
//! cap) — the ONLY addition is incremental per-line delivery to a [`LineSink`].
//!
//! Two reader threads own the byte accumulation + a `\n` splitter and hand
//! COMPLETE lines to the caller thread over an mpsc channel; `sink.line` runs
//! only on the caller thread, so a `LineSink` need not be `Sync`. The two full
//! byte buffers rebuild the same `GitOutput` `exec` would.

use std::io::{Read, Write};
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{channel, Sender};
use std::sync::Arc;

use crate::error::AppError;
use crate::git::activity::GitStream;
use crate::git::exec::{build_command, GitOutput, LineSink, MAX_OUTPUT_BYTES};

/// One complete output line (newline excluded), tagged by its source stream.
struct LineMsg {
    stream: GitStream,
    bytes: Vec<u8>,
}

/// Read `r` to EOF like `exec::read_capped` (same SHARED-counter cap so the
/// combined capture is bounded at `cap`), while ALSO splitting the buffered
/// bytes on `\n` and sending each complete line to `tx`. The full byte buffer is
/// returned so the caller can rebuild the exact `GitOutput`. Always drains to EOF
/// (no pipe-full deadlock). Past the cap we stop buffering AND stop emitting
/// lines — the op errors on overflow anyway.
fn read_streaming<R: Read>(
    mut r: R,
    stream: GitStream,
    counter: &AtomicUsize,
    cap: usize,
    tx: &Sender<LineMsg>,
) -> std::io::Result<(Vec<u8>, bool)> {
    let mut buf = Vec::new();
    let mut overflow = false;
    let mut line_start = 0usize; // index into `buf` where the current unsent line begins
    let mut chunk = [0u8; 16 * 1024];
    loop {
        let n = r.read(&mut chunk)?;
        if n == 0 {
            // EOF: flush a trailing partial line (no terminating `\n`). Skipped
            // on overflow — that output is moot (the op returns an error).
            if !overflow && line_start < buf.len() {
                let _ = tx.send(LineMsg {
                    stream,
                    bytes: buf[line_start..].to_vec(),
                });
            }
            return Ok((buf, overflow));
        }
        let total = counter.fetch_add(n, Ordering::Relaxed) + n;
        if total > cap {
            // FIRST crossing keeps the portion still under the shared cap; after
            // that count + drain but stop buffering (bounded memory) — identical
            // to `exec::read_capped`, so the buffered `GitOutput` matches.
            let already = total - n;
            if already < cap {
                let take = (cap - already).min(n);
                buf.extend_from_slice(&chunk[..take]);
            }
            overflow = true;
        } else {
            buf.extend_from_slice(&chunk[..n]);
        }
        if !overflow {
            // Split every complete line now buffered; `line_start` marches
            // forward so each byte is scanned ~once overall.
            while let Some(nl_off) = buf[line_start..].iter().position(|&b| b == b'\n') {
                let end = line_start + nl_off; // index of the '\n'
                let _ = tx.send(LineMsg {
                    stream,
                    bytes: buf[line_start..end].to_vec(),
                });
                line_start = end + 1;
            }
        }
    }
}

/// Body of [`crate::git::exec::SpawnGitExec::exec_streaming`]. See the module
/// doc for the byte-identity guarantee.
pub(crate) fn spawn_exec_streaming(
    args: &[&str],
    cwd: &Path,
    stdin: Option<&[u8]>,
    env: &[(&str, &str)],
    sink: &dyn LineSink,
) -> Result<GitOutput, AppError> {
    let mut cmd = build_command(args, cwd, stdin.is_some(), env);

    let subcmd = args.first().copied().unwrap_or("");
    let mut child = cmd
        .spawn()
        .map_err(|e| crate::gitbin::spawn_error(subcmd, &e))?;

    // Write stdin, then CLOSE it (EOF), exactly as `exec` — before reading output.
    if let Some(bytes) = stdin {
        let mut sh = child.stdin.take();
        let write_res = match sh.as_mut() {
            Some(s) => s
                .write_all(bytes)
                .map_err(|e| AppError::Git(format!("failed to write `git {subcmd}` stdin: {e}"))),
            None => Err(AppError::Git(format!("failed to open `git {subcmd}` stdin"))),
        };
        drop(sh);
        if let Err(e) = write_res {
            let _ = child.wait();
            return Err(e);
        }
    }

    // BOTH streams read on helper threads (never sequentially — a full pipe would
    // deadlock), bounded by ONE shared combined-byte counter, both draining to
    // EOF. Complete lines flow to the caller thread over the channel.
    let counter = Arc::new(AtomicUsize::new(0));
    let (tx, rx) = channel::<LineMsg>();
    let stdout_pipe = child.stdout.take();
    let stderr_pipe = child.stderr.take();

    let stdout_counter = Arc::clone(&counter);
    let stdout_tx = tx.clone();
    let stdout_join = std::thread::spawn(move || -> std::io::Result<(Vec<u8>, bool)> {
        match stdout_pipe {
            Some(p) => read_streaming(p, GitStream::Stdout, &stdout_counter, MAX_OUTPUT_BYTES, &stdout_tx),
            None => Ok((Vec::new(), false)),
        }
    });
    let stderr_counter = Arc::clone(&counter);
    let stderr_tx = tx.clone();
    let stderr_join = std::thread::spawn(move || -> std::io::Result<(Vec<u8>, bool)> {
        match stderr_pipe {
            Some(p) => read_streaming(p, GitStream::Stderr, &stderr_counter, MAX_OUTPUT_BYTES, &stderr_tx),
            None => Ok((Vec::new(), false)),
        }
    });
    // Drop the caller's own handle so `rx` closes once BOTH readers finish.
    drop(tx);

    // Drain on the caller thread: each complete line, in arrival order, to the
    // sink (runs concurrently with the readers — no deadlock, unbounded channel).
    for msg in rx {
        sink.line(msg.stream, String::from_utf8_lossy(&msg.bytes).as_ref());
    }

    // Readers are done (they dropped their senders at EOF); join for the buffers.
    let (stdout_bytes, stdout_of) = stdout_join
        .join()
        .map_err(|_| AppError::Git(format!("`git {subcmd}` stdout reader panicked")))?
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::exec::{GitExec, SpawnGitExec};
    use std::process::Command;
    use std::sync::Mutex;

    fn have_git() -> bool {
        let ok = Command::new("git").arg("--version").output().is_ok();
        if !ok && std::env::var("BONSAI_REQUIRE_GIT_STRICT").as_deref() == Ok("1") {
            panic!("BONSAI_REQUIRE_GIT_STRICT=1: `git` CLI required on PATH but not found");
        }
        ok
    }

    /// Records every `(stream, line)` the sink is handed, in order.
    #[derive(Default)]
    struct CollectSink(Mutex<Vec<(GitStream, String)>>);
    impl LineSink for CollectSink {
        fn line(&self, stream: GitStream, line: &str) {
            self.0.lock().expect("lock").push((stream, line.to_string()));
        }
    }

    /// The seam splits multi-line output into per-line sink calls AND returns a
    /// `GitOutput` byte-identical to what buffered `exec` produces.
    #[test]
    fn streaming_delivers_lines_and_matches_buffered_output() {
        if !have_git() {
            eprintln!("skipping: `git` CLI not found");
            return;
        }
        let cwd = Path::new(".");
        let script = "line1\nline2\nline3";

        // `git stripspace` echoes stdin onto stdout, ensuring a trailing newline,
        // so it yields three complete lines through the splitter.
        let sink = CollectSink::default();
        let streamed = SpawnGitExec
            .exec_streaming(
                &["stripspace"],
                cwd,
                Some(script.as_bytes()),
                &[],
                &sink,
            )
            .expect("exec_streaming");
        let buffered = SpawnGitExec
            .exec(&["stripspace"], cwd, Some(script.as_bytes()), &[])
            .expect("exec");

        // Byte-identical GitOutput.
        assert_eq!(streamed.success, buffered.success);
        assert_eq!(streamed.code, buffered.code);
        assert_eq!(streamed.stdout, buffered.stdout, "stdout must be byte-identical");
        assert_eq!(streamed.stderr, buffered.stderr, "stderr must be byte-identical");

        // Per-line sink delivery on stdout (stripspace emits trailing newline, so
        // three complete lines).
        let lines = sink.0.lock().expect("lock");
        let stdout_lines: Vec<&str> = lines
            .iter()
            .filter(|(s, _)| *s == GitStream::Stdout)
            .map(|(_, l)| l.as_str())
            .collect();
        assert_eq!(stdout_lines, vec!["line1", "line2", "line3"]);
    }

    /// The DEFAULT trait impl (a fake that only impls `exec`) delegates to `exec`
    /// and never touches the sink — proving every existing mock keeps working.
    #[test]
    fn default_impl_delegates_to_exec_and_ignores_sink() {
        struct FakeExec;
        impl GitExec for FakeExec {
            fn exec(
                &self,
                _a: &[&str],
                _c: &Path,
                _s: Option<&[u8]>,
                _e: &[(&str, &str)],
            ) -> Result<GitOutput, AppError> {
                Ok(GitOutput {
                    success: true,
                    code: Some(0),
                    stdout: "buffered-only\n".to_string(),
                    stderr: String::new(),
                })
            }
        }
        let sink = CollectSink::default();
        let out = FakeExec
            .exec_streaming(&["anything"], Path::new("."), None, &[], &sink)
            .expect("default exec_streaming");
        assert_eq!(out.stdout, "buffered-only\n");
        assert!(sink.0.lock().expect("lock").is_empty(), "default impl must not stream lines");
    }
}
