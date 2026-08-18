//! The per-line cap on child output (security audit 2026-08-18, M4). A
//! `#[path]`-included child module of [`super`] so it can reach `read_capped_line`
//! and `Line` without widening either, following the `session_drain_tests`
//! convention; kept in its own file so `session_pipes.rs` stays focused.
//!
//! What must hold: an over-long line is DROPPED (not truncated into unparseable
//! JSON), it costs exactly one note on the funnel, and the reader resynchronises
//! on the next newline so the FOLLOWING line still arrives intact. Without the
//! resync a single hostile line would poison the rest of the run's output.

use std::io::Cursor;
use std::sync::mpsc::channel;

use super::*;

/// `read_capped_line` with a small cap — the sequencing is what matters, and an
/// 8 MB fixture per case would only make the suite slow.
fn lines(input: &str, cap: usize) -> Vec<String> {
    let mut reader = std::io::BufReader::new(Cursor::new(input.as_bytes().to_vec()));
    let mut buf = Vec::new();
    let mut out = Vec::new();
    loop {
        match read_capped_line(&mut reader, cap, &mut buf) {
            Line::End => return out,
            Line::Text(t) => out.push(t),
            Line::TooLong(n) => out.push(format!("<too long: {n}>")),
        }
    }
}

#[test]
fn a_line_within_the_cap_is_unchanged_and_newlines_are_stripped() {
    assert_eq!(lines("a\nbb\r\nccc\n", 8), vec!["a", "bb", "ccc"]);
    // Exactly the cap (plus its newline) is NOT over it — an off-by-one here would
    // start dropping legitimate output.
    assert_eq!(lines("12345678\nx\n", 8), vec!["12345678", "x"]);
    // A last line without a trailing newline still arrives.
    assert_eq!(lines("a\nlast", 8), vec!["a", "last"]);
}

/// THE regression: a hostile/huge line must not be truncated into the stream, and
/// must not take the next line with it.
#[test]
fn an_over_cap_line_is_dropped_once_and_the_next_line_still_parses() {
    let over = "x".repeat(50);
    let input = format!("before\n{over}\n{{\"type\":\"result\"}}\n");
    // cap 24: wide enough for the short lines, far below the over-long one.
    let got = lines(&input, 24);
    assert_eq!(got.len(), 3, "exactly ONE note for the dropped line: {got:?}");
    assert_eq!(got[0], "before");
    assert!(got[1].starts_with("<too long: "), "{got:?}");
    // Resynchronised on the newline: the following line is byte-identical.
    assert_eq!(got[2], "{\"type\":\"result\"}");
    // No fragment of the dropped line leaked through.
    assert!(!got.iter().any(|l| l.contains("xxxx")), "{got:?}");
}

#[test]
fn several_over_cap_lines_in_a_row_each_cost_exactly_one_note() {
    let big = "y".repeat(40);
    let got = lines(&format!("{big}\n{big}\na\n"), 8);
    assert_eq!(got.len(), 3, "{got:?}");
    assert!(got[0].starts_with("<too long: ") && got[1].starts_with("<too long: "), "{got:?}");
    assert_eq!(got[2], "a");
}

/// An over-long line that never ends (EOF mid-line) must still terminate — the
/// discard loop's EOF branch, which is what stops the reader thread from spinning.
#[test]
fn an_unterminated_over_cap_line_ends_the_reader_instead_of_spinning() {
    let got = lines(&"z".repeat(100), 8);
    assert_eq!(got.len(), 1, "{got:?}");
    assert!(got[0].starts_with("<too long: 100>") || got[0].starts_with("<too long: "), "{got:?}");
}

/// End to end through the real thread and the real 8 MB constant: the funnel gets
/// the truncation note, then the next line, then EOF — and nothing 8 MB long.
#[test]
fn spawn_reader_survives_an_over_cap_line_and_keeps_the_next_one() {
    let mut input = vec![b'x'; MAX_LINE_BYTES + 10];
    input.push(b'\n');
    input.extend_from_slice(b"{\"type\":\"result\"}\n");
    let (tx, rx) = channel();
    spawn_reader(Some(Cursor::new(input)), tx, Msg::Out, Msg::OutEof);

    let mut texts = Vec::new();
    let mut saw_eof = false;
    for msg in rx {
        match msg {
            Msg::Out(t) => texts.push(t),
            Msg::OutEof => {
                saw_eof = true;
                break;
            }
            _ => panic!("unexpected message kind"),
        }
    }
    assert!(saw_eof, "the reader must always signal EOF");
    assert_eq!(texts.len(), 2, "{texts:?}");
    assert!(texts[0].contains("dropped one over-long line"), "{}", texts[0]);
    assert!(texts[0].len() < 200, "the note itself must be short: {}", texts[0].len());
    assert_eq!(texts[1], "{\"type\":\"result\"}");
}
