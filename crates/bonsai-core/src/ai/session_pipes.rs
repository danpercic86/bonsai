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

/// Per-line byte cap for anything the child writes (security audit 2026-08-18,
/// M4). NDJSON puts no bound on a line, and two reachable paths produce huge ones:
/// `--replay-user-messages` echoes our whole payload (up to `ai_bulk_max_bytes`,
/// 4 MB) on ONE line, and a tool-result line carries whatever `Read` returned — a
/// hostile repo can plant a multi-GB file. Everything downstream is already
/// truncated (`MAX_EVENT_TEXT`, `MAX_PARTIAL_TEXT`); this was the only unbounded
/// allocation left. 8 MB clears the legitimate 4 MB replay with room to spare.
const MAX_LINE_BYTES: usize = 8 * 1024 * 1024;

/// What one capped read produced.
enum Line {
    /// A complete line (newline stripped), within the cap.
    Text(String),
    /// The line exceeded [`MAX_LINE_BYTES`] and was DROPPED, not truncated: a
    /// truncated NDJSON prefix is unparseable noise, and forwarding 8 MB of it
    /// would defeat the point. `usize` = bytes read before giving up.
    TooLong(usize),
    /// EOF, or an unrecoverable read error — same handling either way (the child's
    /// exit status is what the session judges the run by).
    End,
}

/// Read one line, refusing to grow the buffer past `cap`. `Take` is what enforces
/// it: `read_until` alone will happily allocate a gigabyte.
fn read_capped_line<R: BufRead>(reader: &mut R, cap: usize, buf: &mut Vec<u8>) -> Line {
    buf.clear();
    // cap + 1: reading one byte PAST the cap is how "over the cap" is detected
    // without mistaking an exactly-cap-sized line for an over-long one.
    match reader.by_ref().take(cap as u64 + 1).read_until(b'\n', buf) {
        Ok(0) | Err(_) => Line::End,
        Ok(n) => {
            if n > cap && buf.last() != Some(&b'\n') {
                // Still mid-line: throw the rest away so the NEXT line is read from
                // a newline boundary and stays parseable.
                let dropped = n + discard_to_newline(reader);
                return Line::TooLong(dropped);
            }
            Line::Text(String::from_utf8_lossy(buf).trim_end_matches(['\n', '\r']).to_string())
        }
    }
}

/// Consume up to and including the next `\n`, allocating NOTHING (the whole point
/// — the line we are skipping may be gigabytes). Returns the bytes discarded.
fn discard_to_newline<R: BufRead>(reader: &mut R) -> usize {
    let mut dropped = 0usize;
    loop {
        let available = match reader.fill_buf() {
            Ok([]) => return dropped, // EOF
            Ok(b) => b,
            Err(_) => return dropped,
        };
        match available.iter().position(|b| *b == b'\n') {
            Some(i) => {
                reader.consume(i + 1);
                return dropped + i + 1;
            }
            None => {
                let n = available.len();
                reader.consume(n);
                dropped += n;
            }
        }
    }
}

/// Forward `src` line by line into the funnel, then signal EOF. Lossy UTF-8 by
/// design: one undecodable byte must not cost us the rest of the run's output.
/// Bounded per line ([`MAX_LINE_BYTES`]) — an over-long line is dropped with ONE
/// note on the funnel, never silently. D16 is unaffected: all of this blocks on
/// this thread, never on the session loop.
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
                let line = match read_capped_line(&mut reader, MAX_LINE_BYTES, &mut buf) {
                    Line::End => break,
                    Line::Text(line) => line,
                    // Not JSON, so `classify_line` shows it verbatim in the dock —
                    // which is the intent: the user learns output was lost.
                    Line::TooLong(bytes) => format!(
                        "… dropped one over-long line from Claude \
                         ({bytes} bytes, cap {MAX_LINE_BYTES})"
                    ),
                };
                if tx.send(wrap(line)).is_err() {
                    return; // receiver gone: the run already ended
                }
            }
        }
        let _ = tx.send(eof);
    });
}

#[cfg(test)]
#[path = "session_pipes_tests.rs"]
mod tests;
