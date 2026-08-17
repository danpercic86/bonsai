//! A minimal `claude`-CLI stand-in that speaks just enough NDJSON to drive the
//! P68 streaming protocol — used as `BONSAI_CLAUDE_BIN` by
//! `tests/ai_stream_bulk_cli.rs`.
//!
//! WHY this exists next to `tests/fixtures/claude_stub.{cmd,sh}`: the batch stub
//! reads one turn with `set /p`, which accepts only ~1 KB of a line and leaves the
//! rest in the pipe — so the NEXT read swallows that residue instead of the reply.
//! It therefore CANNOT exercise the one combination where both serious P68a defects
//! lived: a BULK-sized payload (hundreds of KB, larger than the OS pipe buffer)
//! plus a MID-RUN question. That combination is exactly D16:
//!   (a) the pipe-buffer deadlock — readers must be live before the first write;
//!   (b) the unkillable run — the write must not sit on the session loop thread.
//! A Rust helper is cross-platform by construction, with no `.cmd`/`.sh` twin to
//! diverge, and it can prove the payload arrived IN FULL by reporting its length.
//!
//! A `[[bin]]` target (declared in `Cargo.toml:24-32`) rather than an
//! `examples/` one — deliberately, and do NOT "simplify" it into `examples/`:
//! `cargo test --test ai_stream_bulk_cli` does not build example targets and
//! exposes no `CARGO_BIN_EXE_*` for them, so the test would lose its binary
//! path and skip. A skipped test would make the D16 guard vacuous, which is the
//! one thing this fixture exists to prevent. It is still dev-only: nothing in the
//! app depends on it and it ships in no artifact.
//!
//! Protocol (mode from `BONSAI_ECHO_MODE`, default `bulk_ask`):
//! 1. read ONE line from stdin — the whole first turn as a stream-json `user`
//!    message — and pull the requested paths out of its `===== BONSAI FILE i/n:`
//!    headers;
//! 2. emit `system/init`, an `assistant` text line and (mode `bulk_ask`) a `result`
//!    whose body is the `BONSAI_NEEDS_INPUT:` sentinel;
//! 3. mode `bulk_ask`: read a SECOND line (the user's reply);
//! 4. emit the final `result` carrying one `===== BONSAI RESULT: <path> =====`
//!    block per requested path, each body naming the bytes received — so a test can
//!    assert the FULL payload got through.
//!
//! Modes: `bulk_ask` (default) · `bulk` (no question) · `bulk_missing` (omits the
//! LAST path's block, for the per-file `failed` attribution).

use std::io::{BufRead, Write};

/// Header the bulk payload uses per file (must match `git::ai_resolve_bulk`).
const FILE_MARK: &str = "===== BONSAI FILE ";

fn main() {
    let mode = std::env::var("BONSAI_ECHO_MODE").unwrap_or_else(|_| "bulk_ask".to_string());
    let stdin = std::io::stdin();
    let mut lines = stdin.lock().lines();

    // 1. The whole first turn arrives as ONE (possibly ~400 KB) NDJSON line.
    let Some(Ok(first)) = lines.next() else {
        // Nothing on stdout at all: the session must report the stderr line, which
        // is also the P68a "stderr wins the race" behaviour.
        eprintln!("claude_echo: no turn arrived on stdin");
        std::process::exit(3);
    };
    let payload = user_text(&first);
    let paths = requested_paths(&payload);
    let bytes = payload.len();

    emit(r#"{"type":"system","subtype":"init","session_id":"sess-echo","model":"sonnet","tools":["Read","Grep","Glob"]}"#);
    emit(&assistant_line(&format!("received {bytes} bytes for {} files", paths.len())));

    if mode == "bulk_ask" {
        // 2./3. Ask, then wait for the reply — on an OPEN stdin, which is what makes
        // this a second TURN rather than a second process.
        emit(&result_line("BONSAI_NEEDS_INPUT: which locale wins?", 0.0238));
        match lines.next() {
            Some(Ok(reply)) => {
                let text = user_text(&reply);
                emit(&assistant_line(&format!("answer was {} bytes", text.len())));
            }
            _ => {
                eprintln!("claude_echo: stdin closed while awaiting the reply");
                std::process::exit(4);
            }
        }
    }

    // 4. One result block per path (minus the last one in `bulk_missing`).
    let keep = if mode == "bulk_missing" && !paths.is_empty() {
        paths.len() - 1
    } else {
        paths.len()
    };
    let mut body = String::new();
    for path in paths.iter().take(keep) {
        body.push_str(&format!("===== BONSAI RESULT: {path} =====\n"));
        body.push_str(&format!("ECHO {path} bytes={bytes}\n"));
    }
    emit(&result_line(&body, 0.0263));
}

fn emit(line: &str) {
    let mut out = std::io::stdout();
    let _ = writeln!(out, "{line}");
    let _ = out.flush();
}

/// The `text` of a stream-json `user` message, or the raw line when it is not one
/// (a helper must never panic on input — the test would then see a mystery exit).
fn user_text(line: &str) -> String {
    serde_json::from_str::<serde_json::Value>(line)
        .ok()
        .and_then(|v| {
            v.pointer("/message/content/0/text")
                .and_then(serde_json::Value::as_str)
                .map(str::to_string)
        })
        .unwrap_or_else(|| line.to_string())
}

/// Paths from the payload's `===== BONSAI FILE i/n: <path> =====` headers, in
/// order. A single-file payload has no such header, so fall back to the
/// `FILE: <path>` line of the P13 single-file format.
fn requested_paths(payload: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in payload.lines() {
        if let Some(rest) = line.strip_prefix(FILE_MARK) {
            if let Some((_, tail)) = rest.split_once(": ") {
                let path = tail.trim_end().trim_end_matches('=').trim();
                if !path.is_empty() {
                    out.push(path.to_string());
                }
            }
        } else if out.is_empty() {
            if let Some(path) = line.strip_prefix("FILE: ") {
                out.push(path.trim().to_string());
            }
        }
    }
    out
}

fn assistant_line(text: &str) -> String {
    serde_json::json!({
        "type": "assistant",
        "message": { "content": [{ "type": "text", "text": text }] }
    })
    .to_string()
}

/// A `result` line — byte-compatible with the one-shot envelope (spike §1.3).
fn result_line(body: &str, cost: f64) -> String {
    serde_json::json!({
        "type": "result",
        "subtype": "success",
        "is_error": false,
        "result": body,
        "total_cost_usd": cost,
        "session_id": "sess-echo"
    })
    .to_string()
}
