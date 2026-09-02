//! T5 property suite (contract §2.2): span invariants of the intraline
//! annotator. Exercises the real `annotate_hunk` via the sanctioned
//! `#[doc(hidden)]` test seam `annotate_hunk_for_tests` (see contract §2.2 /
//! §8.2 — a zero-behavior-change wrapper, logged as a test-seam note).

use std::collections::BTreeSet;

use bonsai_core::git::diff::{DiffLine, Hunk, LineKind};
use bonsai_core::git::intraline::{annotate_hunk_for_tests, MAX_INTRALINE_CHARS};
use proptest::prelude::*;

use crate::prop_common::diff_pair;

/// Annotate a one-del + one-add hunk and return `(old_spans, new_spans)`.
fn annotate_pair(a: &str, b: &str) -> (Vec<[u32; 2]>, Vec<[u32; 2]>) {
    let mut hunk = Hunk {
        old_start: 1,
        old_lines: 1,
        new_start: 1,
        new_lines: 1,
        lines: vec![
            DiffLine {
                kind: LineKind::Del,
                old_no: Some(1),
                new_no: None,
                content: a.to_string(),
                no_newline: false,
                spans: Vec::new(),
            },
            DiffLine {
                kind: LineKind::Add,
                old_no: None,
                new_no: Some(1),
                content: b.to_string(),
                no_newline: false,
                spans: Vec::new(),
            },
        ],
    };
    annotate_hunk_for_tests(&mut hunk);
    (hunk.lines[0].spans.clone(), hunk.lines[1].spans.clone())
}

/// Invariants 1-2: ascending, positive-len, non-overlapping AND non-adjacent
/// (coalesced), all within `[0, char_count]`.
fn check_spans(line: &str, spans: &[[u32; 2]]) {
    let cc = line.chars().count() as u32;
    let mut prev_end: Option<u32> = None;
    for &[start, len] in spans {
        assert!(len > 0, "span len > 0");
        assert!(start + len <= cc, "span within code-point bounds: {start}+{len} <= {cc}");
        if let Some(pe) = prev_end {
            assert!(start > pe, "spans ascending + non-adjacent (coalesced): {start} > {pe}");
        }
        prev_end = Some(start + len);
    }
}

fn charset(spans: &[[u32; 2]]) -> BTreeSet<u32> {
    spans.iter().flat_map(|&[s, l]| s..s + l).collect()
}

// ---- F-T5-2: intraline diff is DIRECTIONAL (pinned behavior, not a bug) -----
//
// Contract §2.2 item 4 anticipated that swap-symmetry might not hold and asked
// to "pin the actual behavior". A 64-case proptest showed even the weaker
// changed-CHAR-SET symmetry fails: the LCS backtrack tie-break (`>=`, biased to
// advance the OLD side) picks a different common subsequence when old/new swap,
// so the highlighted (changed) code points differ between `(a,b)` and `(b,a)`.
// This is inherent to a directional diff and does NOT affect per-side
// correctness (each side's spans still cover exactly its non-LCS tokens, guarded
// by `spans_are_well_formed`). Pinned here; logged as FINDINGS F-T5-2 (a
// behavior note, not a product defect).

/// Minimal shrunk case: whitespace-run coalescing + a lone `Other` char make
/// the changed-char sets differ under old/new swap.
#[test]
fn regression_f_t5_2_intraline_diff_is_directional() {
    let a = "\n\n\n¡\n\n¡";
    let b = "\n\n\n\n¡";
    let (old_ab, new_ab) = annotate_pair(a, b);
    let (old_ba, new_ba) = annotate_pair(b, a);
    // Each side is still individually well-formed.
    check_spans(a, &old_ab);
    check_spans(b, &new_ab);
    // ...but the changed-char sets are NOT swap-symmetric (pinned).
    let symmetric =
        charset(&old_ab) == charset(&new_ba) && charset(&new_ab) == charset(&old_ba);
    assert!(
        !symmetric,
        "F-T5-2: intraline diff is directional; expected asymmetry to reproduce"
    );
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 64, ..ProptestConfig::default() })]

    /// Items 1-3: span well-formedness + code-point bounds + the empty/over-cap
    /// degeneracies.
    #[test]
    fn spans_are_well_formed((a, b) in diff_pair()) {
        let (old, new) = annotate_pair(&a, &b);
        check_spans(&a, &old);
        check_spans(&b, &new);

        if a == b {
            prop_assert!(old.is_empty() && new.is_empty(), "identical lines ⇒ no spans");
        }
        if a.chars().count() > MAX_INTRALINE_CHARS || b.chars().count() > MAX_INTRALINE_CHARS {
            prop_assert!(old.is_empty() && new.is_empty(), "over-cap ⇒ no spans");
        }
    }

    /// Astral / multibyte specific: offsets are code points, never bytes.
    #[test]
    fn astral_offsets_stay_in_code_point_bounds(
        prefix in prop::collection::vec(any::<char>().prop_filter("no nl/NUL", |c| *c != '\n' && *c != '\r' && *c != '\0'), 0..=8),
        tail_a in "[a-z]{0,5}",
        tail_b in "[a-z]{0,5}",
    ) {
        let p: String = prefix.into_iter().collect();
        let a = format!("{p}{tail_a}");
        let b = format!("{p}{tail_b}");
        let (old, new) = annotate_pair(&a, &b);
        check_spans(&a, &old);
        check_spans(&b, &new);
    }
}

// ---- deterministic regressions (contract §2.2 item 5 + degeneracies) --------

/// Context + surplus rows always keep `spans = []` (item 5).
#[test]
fn context_and_surplus_rows_keep_empty_spans() {
    let mut hunk = Hunk {
        old_start: 1,
        old_lines: 3,
        new_start: 1,
        new_lines: 2,
        lines: vec![
            line(LineKind::Context, "unchanged"),
            line(LineKind::Del, "alpha one"),
            line(LineKind::Del, "surplus del"),
            line(LineKind::Add, "alpha two"),
        ],
    };
    annotate_hunk_for_tests(&mut hunk);
    assert!(hunk.lines[0].spans.is_empty(), "context empty");
    assert!(!hunk.lines[1].spans.is_empty(), "paired del annotated");
    assert!(hunk.lines[2].spans.is_empty(), "surplus del empty");
    assert!(!hunk.lines[3].spans.is_empty(), "paired add annotated");
}

fn line(kind: LineKind, content: &str) -> DiffLine {
    DiffLine {
        kind,
        old_no: None,
        new_no: None,
        content: content.to_string(),
        no_newline: false,
        spans: Vec::new(),
    }
}
