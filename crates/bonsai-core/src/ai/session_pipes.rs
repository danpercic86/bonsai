//! The three I/O threads around one streaming child and the single mpsc funnel
//! they report on (P68 §A). Split from [`super::session`] so that module is only
//! the state machine: NOTHING here interprets a line (that is `super::stream`) or
//! decides anything about the run.
//!
//! Why threads at all: `git2`-style blocking I/O on the loop thread is exactly the
//! failure this milestone removes. stdout, stderr and stdin each get their own
//! thread, so no single pipe can stop the loop from polling cancel/the watchdog.

use std::io::{BufRead, BufReader, Read, Write};
use std::process::ChildStdin;
use std::sync::mpsc::{Receiver, Sender};
use std::thread;

/// What arrived on the funnel. Two reader threads plus the writer thread, one
/// mpsc, so stdout and stderr interleave in real time without either blocking the
/// child and a failed write is reported without blocking the loop.
///
/// NOTE (the S1 race): `Out*` and `Err*` come from DIFFERENT senders, so mpsc
/// gives NO ordering guarantee between the last stderr line and `OutEof`. The
/// session therefore drains a bounded grace before composing a failure message.
pub(super) enum Msg {
    Out(String),
    Err(String),
    OutEof,
    ErrEof,
    /// The writer thread could not deliver a turn (`io::Error`, rendered).
    WriteErr(String),
}

/// A pending stdin write, handed to the writer thread. Sending is never blocking:
/// only the writer thread ever touches [`ChildStdin`], so a child that refuses to
/// drain its stdin cannot stop the loop from polling cancel / the watchdog.
/// Dropping the last `Sender` closes the child's stdin (EOF).
pub(super) type WriteTx = Sender<String>;

/// One NDJSON `user` message, newline-terminated. serde_json does the escaping —
/// never hand-build this line.
pub(super) fn turn_line(text: &str) -> String {
    let line = serde_json::json!({
        "type": "user",
        "message": { "role": "user", "content": [{ "type": "text", "text": text }] }
    });
    format!("{line}\n")
}

/// Queue `text` for the writer thread. Fails only when that thread is already
/// gone (a previous write failed, or stdin was closed for a one-shot run) — an
/// actual write failure arrives later as [`Msg::WriteErr`].
pub(super) fn send_write(writer: &mut Option<WriteTx>, text: String) -> std::io::Result<()> {
    let closed = || {
        std::io::Error::new(std::io::ErrorKind::BrokenPipe, "Claude's stdin is already closed")
    };
    let Some(tx) = writer.as_ref() else {
        return Err(closed());
    };
    if tx.send(text).is_err() {
        *writer = None;
        return Err(closed());
    }
    Ok(())
}

/// Own [`ChildStdin`] in ONE dedicated thread and write whatever arrives on
/// `reqs`. This is why a child that never drains its stdin cannot make a run
/// unkillable: `write_all` blocks HERE, not on the loop thread, so cancel and the
/// watchdog keep being polled (the non-streaming sibling `super::run_process`
/// uses a writer thread for the same reason). Errors go back through the same
/// funnel as the readers; dropping every `Sender` closes stdin (EOF).
pub(super) fn spawn_writer(stdin: Option<ChildStdin>, reqs: Receiver<String>, tx: Sender<Msg>) {
    thread::spawn(move || {
        let mut stdin = stdin;
        while let Ok(text) = reqs.recv() {
            let res = match stdin.as_mut() {
                Some(si) => si.write_all(text.as_bytes()).and_then(|()| si.flush()),
                None => Err(std::io::Error::new(
                    std::io::ErrorKind::BrokenPipe,
                    "Claude has no stdin pipe",
                )),
            };
            if let Err(e) = res {
                let _ = tx.send(Msg::WriteErr(e.to_string()));
                return; // `stdin` drops here -> the child sees EOF
            }
        }
    });
}

/// Forward `src` line by line into the funnel, then signal EOF. Lossy UTF-8 by
/// design: one undecodable byte must not cost us the rest of the run's output.
pub(super) fn spawn_reader<R: Read + Send + 'static>(
    src: Option<R>,
    tx: Sender<Msg>,
    wrap: fn(String) -> Msg,
    eof: Msg,
) {
    thread::spawn(move || {
        if let Some(r) = src {
            let mut reader = BufReader::new(r);
            let mut buf = Vec::new();
            loop {
                buf.clear();
                match reader.read_until(b'\n', &mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(_) => {
                        let line = String::from_utf8_lossy(&buf)
                            .trim_end_matches(['\n', '\r'])
                            .to_string();
                        if tx.send(wrap(line)).is_err() {
                            return; // receiver gone: the run already ended
                        }
                    }
                }
            }
        }
        let _ = tx.send(eof);
    });
}
