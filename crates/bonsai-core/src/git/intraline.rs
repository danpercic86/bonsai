//! P61a — word-level (intraline) diff highlighting.
//!
//! Pure, dependency-free. Given a [`Hunk`], pairs consecutive del/add runs
//! index-by-index (exactly like the frontend's P46 `pairSplitRows`) and, for
//! each paired row, computes the CHANGED sub-ranges between the old and new
//! line via a hand-rolled word-level LCS token diff. Only paired rows receive
//! `spans`; context lines and surplus (unpaired) del/add lines keep `spans =
//! []` (the add/del tint already conveys "all new/removed").
//!
//! Offsets are Unicode SCALAR VALUES (code points / `char`s), `[start, len]`,
//! ascending + non-overlapping — chosen over UTF-16 units for a natural Rust
//! implementation (`char_indices`); a multibyte unit test guards the contract.

use crate::git::diff::{DiffLine, Hunk, LineKind};

/// Skip the O(n·m) token diff for any line longer than this (code points) —
/// avoids blowups on minified / single-line files. The line keeps `spans = []`.
pub const MAX_INTRALINE_CHARS: usize = 2000;

/// One token: a maximal run of one character class, with its code-point span.
struct Token<'a> {
    text: &'a str,
    char_start: u32,
    char_len: u32,
}

/// Word-level token class. `Word` and `Space` runs coalesce; every other
/// character is its own 1-char token (so `foo(bar)` -> `foo` `(` `bar` `)`).
#[derive(PartialEq, Eq, Clone, Copy)]
enum Class {
    Word,
    Space,
    Other,
}

fn class(c: char) -> Class {
    if c == '_' || c.is_alphanumeric() {
        Class::Word
    } else if c.is_whitespace() {
        Class::Space
    } else {
        Class::Other
    }
}

/// Split `s` into maximal single-class runs, tracking the code-point index so
/// each token carries `[char_start, char_len]`. `Other` chars never coalesce.
fn tokenize(s: &str) -> Vec<Token<'_>> {
    let mut out: Vec<Token<'_>> = Vec::new();
    let mut start_byte = 0usize;
    let mut start_cp = 0u32;
    let mut cur: Option<Class> = None;
    let mut cp = 0u32;
    for (byte, c) in s.char_indices() {
        let cls = class(c);
        // A new token begins on a class change, and always for `Other`.
        let boundary = match cur {
            None => true,
            Some(prev) => prev != cls || cls == Class::Other,
        };
        if boundary {
            if cur.is_some() {
                out.push(Token {
                    text: &s[start_byte..byte],
                    char_start: start_cp,
                    char_len: cp - start_cp,
                });
            }
            start_byte = byte;
            start_cp = cp;
            cur = Some(cls);
        }
        cp += 1;
    }
    if cur.is_some() {
        out.push(Token {
            text: &s[start_byte..],
            char_start: start_cp,
            char_len: cp - start_cp,
        });
    }
    out
}

/// Standard LCS/Myers over token TEXT equality (O(n·m), fine for one line).
/// Returns `(a_deleted, b_inserted)` boolean masks: a token is "changed" when
/// it is not part of the longest common subsequence. The backtrack tie-break
/// (`>=` prefers advancing OLD) matches the mock's `diffOps`.
fn lcs_marks(a: &[Token<'_>], b: &[Token<'_>]) -> (Vec<bool>, Vec<bool>) {
    let n = a.len();
    let m = b.len();
    // dp[i][j] = LCS length of a[i..] vs b[j..].
    let mut dp = vec![vec![0u32; m + 1]; n + 1];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            dp[i][j] = if a[i].text == b[j].text {
                dp[i + 1][j + 1] + 1
            } else {
                dp[i + 1][j].max(dp[i][j + 1])
            };
        }
    }
    let mut a_del = vec![true; n];
    let mut b_ins = vec![true; m];
    let mut i = 0usize;
    let mut j = 0usize;
    while i < n && j < m {
        if a[i].text == b[j].text {
            a_del[i] = false;
            b_ins[j] = false;
            i += 1;
            j += 1;
        } else if dp[i + 1][j] >= dp[i][j + 1] {
            i += 1;
        } else {
            j += 1;
        }
    }
    (a_del, b_ins)
}

/// Coalesce the char spans of consecutive marked (changed) tokens into
/// `[char_start, total_char_len]`. Tokens partition the line gaplessly, so any
/// two consecutive marked tokens touch; a matched token between them breaks the
/// run. Result is ascending + non-overlapping.
fn merge_adjacent(tokens: &[Token<'_>], marks: &[bool]) -> Vec<[u32; 2]> {
    let mut out: Vec<[u32; 2]> = Vec::new();
    for (t, &changed) in tokens.iter().zip(marks) {
        if !changed {
            continue;
        }
        if let Some(last) = out.last_mut() {
            if last[0] + last[1] == t.char_start {
                last[1] += t.char_len;
                continue;
            }
        }
        out.push([t.char_start, t.char_len]);
    }
    out
}

/// Char-diff two lines: `(old_spans removed from `a`, new_spans added to `b`)`.
fn token_diff(a: &str, b: &str) -> (Vec<[u32; 2]>, Vec<[u32; 2]>) {
    let ta = tokenize(a);
    let tb = tokenize(b);
    let (a_del, b_ins) = lcs_marks(&ta, &tb);
    (merge_adjacent(&ta, &a_del), merge_adjacent(&tb, &b_ins))
}

/// Annotate one hunk in place: walk maximal runs of consecutive changed
/// (non-context) lines, pair `dels[i]` with `adds[i]`, and set each paired
/// row's `spans` to its side's changed ranges. Surplus rows and context lines
/// keep `spans = []`.
pub(crate) fn annotate_hunk(hunk: &mut Hunk) {
    let n = hunk.lines.len();
    let mut i = 0;
    while i < n {
        if hunk.lines[i].kind == LineKind::Context {
            i += 1;
            continue;
        }
        let start = i;
        while i < n && hunk.lines[i].kind != LineKind::Context {
            i += 1;
        }
        annotate_run(&mut hunk.lines[start..i]);
    }
}

/// Pair the del/add lines of one changed run index-by-index and char-diff each
/// pair. Lines whose content exceeds [`MAX_INTRALINE_CHARS`] are skipped
/// (spans stay empty).
fn annotate_run(run: &mut [DiffLine]) {
    let dels: Vec<usize> = run
        .iter()
        .enumerate()
        .filter(|(_, l)| l.kind == LineKind::Del)
        .map(|(k, _)| k)
        .collect();
    let adds: Vec<usize> = run
        .iter()
        .enumerate()
        .filter(|(_, l)| l.kind == LineKind::Add)
        .map(|(k, _)| k)
        .collect();
    let pairs = dels.len().min(adds.len());
    for p in 0..pairs {
        let di = dels[p];
        let ai = adds[p];
        if run[di].content.chars().count() > MAX_INTRALINE_CHARS
            || run[ai].content.chars().count() > MAX_INTRALINE_CHARS
        {
            continue; // guard: leave spans empty
        }
        let (old_spans, new_spans) = token_diff(&run[di].content, &run[ai].content);
        run[di].spans = old_spans;
        run[ai].spans = new_spans;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn toks(s: &str) -> Vec<(String, u32, u32)> {
        tokenize(s)
            .into_iter()
            .map(|t| (t.text.to_string(), t.char_start, t.char_len))
            .collect()
    }

    #[test]
    fn tokenize_classes() {
        // Word runs (alphanumeric + `_`) coalesce; punctuation is per-char.
        assert_eq!(
            toks("foo(bar)"),
            vec![
                ("foo".into(), 0, 3),
                ("(".into(), 3, 1),
                ("bar".into(), 4, 3),
                (")".into(), 7, 1),
            ]
        );
        // Whitespace runs coalesce; `_` and digits are word chars.
        assert_eq!(
            toks("x_1  y"),
            vec![
                ("x_1".into(), 0, 3),
                ("  ".into(), 3, 2),
                ("y".into(), 5, 1),
            ]
        );
        // Consecutive punctuation -> separate 1-char tokens.
        assert_eq!(
            toks("=="),
            vec![("=".into(), 0, 1), ("=".into(), 1, 1)]
        );
        assert!(toks("").is_empty());
    }

    #[test]
    fn token_diff_single_token() {
        // A lone changed token -> that token's whole range on each side.
        assert_eq!(token_diff("1", "42"), (vec![[0, 1]], vec![[0, 2]]));
    }

    #[test]
    fn token_diff_multi_token_shared_prefix_suffix() {
        // `const x = 1;` -> `const x = 42;` : only `1`->`42` changes (idx 10).
        let (old, new) = token_diff("const x = 1;", "const x = 42;");
        assert_eq!(old, vec![[10, 1]]);
        assert_eq!(new, vec![[10, 2]]);
        // Shared prefix AND suffix around a single word swap.
        let (old, new) = token_diff("foo bar baz", "foo qux baz");
        assert_eq!(old, vec![[4, 3]]); // "bar"
        assert_eq!(new, vec![[4, 3]]); // "qux"
    }

    #[test]
    fn token_diff_pure_insert_and_delete() {
        // Pure insert: nothing removed from OLD; the trailing " bar" is added.
        let (old, new) = token_diff("foo", "foo bar");
        assert_eq!(old, Vec::<[u32; 2]>::new());
        assert_eq!(new, vec![[3, 4]]); // " bar" (space + word coalesced)
        // Pure delete: mirror image.
        let (old, new) = token_diff("foo bar", "foo");
        assert_eq!(old, vec![[3, 4]]);
        assert_eq!(new, Vec::<[u32; 2]>::new());
    }

    #[test]
    fn token_diff_identical_is_empty() {
        assert_eq!(
            token_diff("same text", "same text"),
            (Vec::<[u32; 2]>::new(), Vec::<[u32; 2]>::new())
        );
    }

    #[test]
    fn token_diff_multibyte_offsets_are_code_points() {
        // Accented char (é = 2 bytes, 1 code point): the changed digit sits at
        // code-point index 5, NOT byte index 6 — guards the code-point contract.
        let (old, new) = token_diff("café 1", "café 2");
        assert_eq!(old, vec![[5, 1]]);
        assert_eq!(new, vec![[5, 1]]);
        // Emoji (👍 = 4 bytes, 1 code point, `Other` class): "ok"->"no" at cp 2.
        let (old, new) = token_diff("👍 ok", "👍 no");
        assert_eq!(old, vec![[2, 2]]);
        assert_eq!(new, vec![[2, 2]]);
    }

    /// Build a Del/Add pair in one hunk and annotate it.
    fn annotate_pair(old: &str, new: &str) -> (Vec<[u32; 2]>, Vec<[u32; 2]>) {
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
                    content: old.to_string(),
                    no_newline: false,
                    spans: Vec::new(),
                },
                DiffLine {
                    kind: LineKind::Add,
                    old_no: None,
                    new_no: Some(1),
                    content: new.to_string(),
                    no_newline: false,
                    spans: Vec::new(),
                },
            ],
        };
        annotate_hunk(&mut hunk);
        (hunk.lines[0].spans.clone(), hunk.lines[1].spans.clone())
    }

    #[test]
    fn annotate_hunk_pairs_and_skips_context() {
        // A context line before and after a Del/Add pair: context stays empty,
        // the pair gets emphasis on the changed token only.
        let mut hunk = Hunk {
            old_start: 1,
            old_lines: 3,
            new_start: 1,
            new_lines: 3,
            lines: vec![
                DiffLine {
                    kind: LineKind::Context,
                    old_no: Some(1),
                    new_no: Some(1),
                    content: "let x = 1;".into(),
                    no_newline: false,
                    spans: Vec::new(),
                },
                DiffLine {
                    kind: LineKind::Del,
                    old_no: Some(2),
                    new_no: None,
                    content: "let y = 2;".into(),
                    no_newline: false,
                    spans: Vec::new(),
                },
                DiffLine {
                    kind: LineKind::Add,
                    old_no: None,
                    new_no: Some(2),
                    content: "let y = 3;".into(),
                    no_newline: false,
                    spans: Vec::new(),
                },
            ],
        };
        annotate_hunk(&mut hunk);
        assert!(hunk.lines[0].spans.is_empty(), "context stays empty");
        assert_eq!(hunk.lines[1].spans, vec![[8, 1]]); // "2"
        assert_eq!(hunk.lines[2].spans, vec![[8, 1]]); // "3"
    }

    #[test]
    fn annotate_hunk_surplus_rows_stay_empty() {
        // Two dels, one add: only the first del pairs with the add; the second
        // del is surplus and keeps spans = [].
        let mut hunk = Hunk {
            old_start: 1,
            old_lines: 2,
            new_start: 1,
            new_lines: 1,
            lines: vec![
                DiffLine {
                    kind: LineKind::Del,
                    old_no: Some(1),
                    new_no: None,
                    content: "alpha 1".into(),
                    no_newline: false,
                    spans: Vec::new(),
                },
                DiffLine {
                    kind: LineKind::Del,
                    old_no: Some(2),
                    new_no: None,
                    content: "beta".into(),
                    no_newline: false,
                    spans: Vec::new(),
                },
                DiffLine {
                    kind: LineKind::Add,
                    old_no: None,
                    new_no: Some(1),
                    content: "alpha 2".into(),
                    no_newline: false,
                    spans: Vec::new(),
                },
            ],
        };
        annotate_hunk(&mut hunk);
        assert_eq!(hunk.lines[0].spans, vec![[6, 1]]); // paired: "1"
        assert!(hunk.lines[1].spans.is_empty(), "surplus del stays empty");
        assert_eq!(hunk.lines[2].spans, vec![[6, 1]]); // paired: "2"
    }

    #[test]
    fn max_intraline_chars_skips_long_lines() {
        // A paired row longer than the cap is skipped (spans stay empty)...
        let long_old = "a".repeat(MAX_INTRALINE_CHARS + 1);
        let long_new = "b".repeat(MAX_INTRALINE_CHARS + 1);
        let (old, new) = annotate_pair(&long_old, &long_new);
        assert!(old.is_empty() && new.is_empty(), "over cap => no spans");
        // ...while a pair AT the cap is still annotated.
        let at_old = format!("{} 1", "a".repeat(MAX_INTRALINE_CHARS - 2));
        let at_new = format!("{} 2", "a".repeat(MAX_INTRALINE_CHARS - 2));
        let (old, new) = annotate_pair(&at_old, &at_new);
        assert!(!old.is_empty() && !new.is_empty(), "at cap => annotated");
    }
}
