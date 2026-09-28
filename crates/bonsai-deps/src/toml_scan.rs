//! `[HIGH-2]` TOML nesting pre-scan (contract §4.2). Runs before EVERY
//! `toml::from_str` (manifests, lockfiles and Cargo configs), so a nesting
//! bomb is rejected in one linear pass without reaching the parser.
//!
//! The estimate is an UPPER bound on the nesting the parser (and serde's
//! `IgnoredAny`, which recurses) would see: open `[` / `{` containers, plus
//! every dotted-key segment (`a.b.c = 1` nests three tables) of the header
//! and of each key that opens a container. Delimiters inside basic,
//! literal and multi-line strings and in `#` comments are ignored.
//! Iterative, byte-oriented (every delimiter is ASCII, and UTF-8
//! continuation bytes are ≥ 0x80), no recursion, O(n).
//!
//! `toml_parser` 1.x does have its own depth bound (`max_depth`, lifted only
//! by the `unbounded` feature, which this crate never enables); this scan is
//! the contract's belt-and-braces over it.

/// Nesting above this ⇒ `LimitExceeded` (§4.2).
pub(crate) const TOML_MAX_DEPTH: u32 = 64;

/// The document nests deeper than [`TOML_MAX_DEPTH`]; `line` is 1-based.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct TooDeep {
    pub(crate) line: u32,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum State {
    Normal,
    Comment,
    Basic,
    Literal,
    MlBasic,
    MlLiteral,
}

/// The scanner's running state.
struct Scan {
    /// Open containers: the byte (`[` / `{`) and the dotted-key depth of the
    /// key that opened it.
    stack: Vec<(u8, u32)>,
    /// Sum of the key depths on `stack`.
    frame_sum: u32,
    /// Dots seen in the current key.
    cur_dots: u32,
    /// Depth contributed by the current `[header]` / `[[header]]`.
    header_depth: u32,
    in_header: bool,
    /// A key (not a value) is expected / being read.
    key_ctx: bool,
    /// Only whitespace so far on this line.
    line_start: bool,
    line: u32,
}

impl Scan {
    fn depth(&self) -> u32 {
        let open = u32::try_from(self.stack.len()).unwrap_or(u32::MAX);
        open.saturating_add(self.frame_sum)
            .saturating_add(self.header_depth)
            .saturating_add(self.cur_dots)
    }

    fn check(&self) -> Result<(), TooDeep> {
        if self.depth() > TOML_MAX_DEPTH {
            return Err(TooDeep { line: self.line });
        }
        Ok(())
    }

    fn newline(&mut self) {
        self.line = self.line.saturating_add(1);
        self.line_start = true;
        self.in_header = false;
        if self.stack.is_empty() {
            self.key_ctx = true;
            self.cur_dots = 0;
        }
    }

    fn push(&mut self, b: u8) -> Result<(), TooDeep> {
        self.stack.push((b, self.cur_dots));
        self.frame_sum = self.frame_sum.saturating_add(self.cur_dots);
        self.cur_dots = 0;
        // An inline table starts with a key; an array with a value.
        self.key_ctx = b == b'{';
        self.check()
    }

    fn pop(&mut self) {
        if let Some((_, dots)) = self.stack.pop() {
            self.frame_sum = self.frame_sum.saturating_sub(dots);
        }
        self.cur_dots = 0;
        self.key_ctx = false;
    }

    /// One structural byte outside strings and comments.
    fn structural(&mut self, b: u8, next: Option<u8>) -> Result<(), TooDeep> {
        let at_start = self.line_start;
        self.line_start = false;
        match b {
            b'[' if at_start && self.stack.is_empty() && !self.in_header => {
                // `[table]` / `[[array-of-tables]]`: a fresh header.
                self.in_header = true;
                self.key_ctx = true;
                self.cur_dots = 0;
                self.header_depth = 1 + u32::from(next == Some(b'['));
                self.check()?;
            }
            b'[' if self.in_header => {}
            b']' if self.in_header => {
                self.header_depth = self.header_depth.saturating_add(self.cur_dots);
                self.cur_dots = 0;
                self.check()?;
            }
            b'.' if self.key_ctx => {
                self.cur_dots = self.cur_dots.saturating_add(1);
                self.check()?;
            }
            b'=' if self.key_ctx && !self.in_header => self.key_ctx = false,
            b'[' | b'{' => self.push(b)?,
            b']' | b'}' => self.pop(),
            b',' => {
                if matches!(self.stack.last(), Some((b'{', _))) {
                    self.key_ctx = true;
                    self.cur_dots = 0;
                }
            }
            _ => {}
        }
        Ok(())
    }
}

/// `true` when `bytes[i..]` starts with three `q`.
fn triple(bytes: &[u8], i: usize, q: u8) -> bool {
    bytes
        .get(i..i + 3)
        .is_some_and(|w| w.iter().all(|c| *c == q))
}

/// Index just past a closing triple quote at `i`, absorbing the up to two
/// extra quotes TOML allows directly before the delimiter.
fn close_triple(bytes: &[u8], i: usize, q: u8) -> usize {
    let mut j = i + 3;
    let mut extra = 0;
    while extra < 2 && bytes.get(j) == Some(&q) {
        j += 1;
        extra += 1;
    }
    j
}

/// §4.2: `Err` when `text` nests deeper than [`TOML_MAX_DEPTH`].
pub(crate) fn check_depth(text: &str) -> Result<(), TooDeep> {
    let bytes = text.as_bytes();
    let mut s = Scan {
        stack: Vec::new(),
        frame_sum: 0,
        cur_dots: 0,
        header_depth: 0,
        in_header: false,
        key_ctx: true,
        line_start: true,
        line: 1,
    };
    let mut state = State::Normal;
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        match state {
            State::Normal => match b {
                b'\n' => s.newline(),
                b' ' | b'\t' | b'\r' => {}
                b'#' => state = State::Comment,
                b'"' | b'\'' => {
                    s.line_start = false;
                    let multi = triple(bytes, i, b);
                    state = match (b, multi) {
                        (b'"', true) => State::MlBasic,
                        (b'"', false) => State::Basic,
                        (_, true) => State::MlLiteral,
                        (_, false) => State::Literal,
                    };
                    if multi {
                        i += 3;
                        continue;
                    }
                }
                _ => s.structural(b, bytes.get(i + 1).copied())?,
            },
            State::Comment => {
                if b == b'\n' {
                    state = State::Normal;
                    s.newline();
                }
            }
            State::Basic | State::Literal => match b {
                b'\\' if state == State::Basic => i += 1,
                b'\n' => {
                    // Unterminated: the parser reports it; keep counting.
                    state = State::Normal;
                    s.newline();
                }
                b'"' if state == State::Basic => state = State::Normal,
                b'\'' if state == State::Literal => state = State::Normal,
                _ => {}
            },
            State::MlBasic | State::MlLiteral => {
                let q = if state == State::MlBasic { b'"' } else { b'\'' };
                if b == b'\\' && state == State::MlBasic {
                    if bytes.get(i + 1) == Some(&b'\n') {
                        s.line = s.line.saturating_add(1);
                    }
                    i += 2;
                    continue;
                }
                if b == b'\n' {
                    s.line = s.line.saturating_add(1);
                } else if triple(bytes, i, q) {
                    i = close_triple(bytes, i, q);
                    state = State::Normal;
                    continue;
                }
            }
        }
        i += 1;
    }
    Ok(())
}

#[cfg(test)]
#[path = "toml_scan_tests.rs"]
mod tests;
