//! `pep440` (PyPI, E1): `[N!]N(.N)*[{a|b|rc}N][.postN][.devN][+local]`,
//! case-insensitive, with the PEP 440 normalizations (`alpha/beta/c/pre/
//! preview`, `-`/`_`/`.` separators, implicit numbers, `-N` post). A single
//! forward cursor scan — iterative, no recursion `[HIGH-2]`.

use super::{lower_bound_scan, strip_lower_op, VersionScheme};

pub struct Pep440Scheme;

/// Pre-release key. Variant order is the comparison order: a dev-only
/// release (`1.0.dev1`) sorts before every pre-release, a final release
/// after all of them.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum PreKey {
    DevOnly,
    /// (kind: a=0, b=1, rc=2, number)
    Pre(u8, u64),
    Final,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum PostKey {
    None,
    Post(u64),
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum DevKey {
    Dev(u64),
    None,
}

/// Local segments: alphanumeric sort before numeric (PEP 440).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum LocalSeg {
    Alpha(String),
    Num(u64),
}

/// Field order is the PEP 440 comparison order. `release` has trailing zeros
/// trimmed so `1.0 == 1.0.0`; an empty `local` sorts before any local.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Pep440Version {
    epoch: u64,
    release: Vec<u64>,
    pre: PreKey,
    post: PostKey,
    dev: DevKey,
    local: Vec<LocalSeg>,
}

struct Cursor<'a> {
    s: &'a [u8],
    i: usize,
}

impl Cursor<'_> {
    fn peek(&self) -> Option<u8> {
        self.s.get(self.i).copied()
    }
    fn at_end(&self) -> bool {
        self.i >= self.s.len()
    }
    fn eat_sep(&mut self) -> bool {
        if matches!(self.peek(), Some(b'.' | b'-' | b'_')) {
            self.i += 1;
            true
        } else {
            false
        }
    }
    fn number(&mut self) -> Option<u64> {
        let start = self.i;
        while matches!(self.peek(), Some(b'0'..=b'9')) {
            self.i += 1;
        }
        if start == self.i {
            return None;
        }
        std::str::from_utf8(&self.s[start..self.i])
            .ok()?
            .parse()
            .ok()
    }
    /// Consumes the longest of `words` at the cursor; returns its index.
    fn word(&mut self, words: &[&str]) -> Option<usize> {
        let rest = &self.s[self.i..];
        let (idx, w) = words
            .iter()
            .enumerate()
            .filter(|(_, w)| rest.starts_with(w.as_bytes()))
            .max_by_key(|(_, w)| w.len())?;
        self.i += w.len();
        Some(idx)
    }
    /// `[sep] word [sep] [N]` with implicit number 0; restores on no match.
    fn tagged(&mut self, words: &[&str]) -> Option<(usize, u64)> {
        let save = self.i;
        self.eat_sep();
        let Some(idx) = self.word(words) else {
            self.i = save;
            return None;
        };
        let before_num = self.i;
        self.eat_sep();
        let n = match self.number() {
            Some(n) => n,
            None => {
                self.i = before_num;
                0
            }
        };
        Some((idx, n))
    }
}

/// Pre-release spellings and their normalized kind (a=0, b=1, rc=2).
/// `Cursor::word` takes the longest match, so `preview` beats `pre`.
const PRE_WORDS: [&str; 8] = ["a", "alpha", "b", "beta", "c", "rc", "pre", "preview"];
const PRE_KIND: [u8; 8] = [0, 0, 1, 1, 2, 2, 2, 2];

fn parse_pep440(s: &str) -> Option<Pep440Version> {
    let lower = s.trim().to_ascii_lowercase();
    let mut c = Cursor {
        s: lower.as_bytes(),
        i: 0,
    };
    if c.peek() == Some(b'v') {
        c.i += 1;
    }
    let first = c.number()?;
    let (epoch, mut release) = if c.peek() == Some(b'!') {
        c.i += 1;
        (first, vec![c.number()?])
    } else {
        (0, vec![first])
    };
    while c.peek() == Some(b'.') && matches!(c.s.get(c.i + 1), Some(b'0'..=b'9')) {
        c.i += 1;
        release.push(c.number()?);
    }
    let pre = c
        .tagged(&PRE_WORDS)
        .map(|(idx, n)| (PRE_KIND.get(idx).copied().unwrap_or(2), n));
    let mut post = c.tagged(&["post", "rev", "r"]).map(|(_, n)| n);
    if post.is_none() && c.peek() == Some(b'-') {
        let save = c.i;
        c.i += 1;
        post = c.number();
        if post.is_none() {
            c.i = save;
        }
    }
    let dev = c.tagged(&["dev"]).map(|(_, n)| n);
    let mut local = Vec::new();
    if c.peek() == Some(b'+') {
        c.i += 1;
        let rest = std::str::from_utf8(&c.s[c.i..]).ok()?;
        for seg in rest.split(['.', '-', '_']) {
            if seg.is_empty() || !seg.bytes().all(|b| b.is_ascii_alphanumeric()) {
                return None;
            }
            local.push(match seg.parse::<u64>() {
                Ok(n) if seg.bytes().all(|b| b.is_ascii_digit()) => LocalSeg::Num(n),
                _ => LocalSeg::Alpha(seg.to_string()),
            });
        }
        c.i = c.s.len();
    }
    if !c.at_end() {
        return None;
    }
    while release.len() > 1 && release.last() == Some(&0) {
        release.pop();
    }
    let pre = match (pre, post, dev) {
        (Some((k, n)), _, _) => PreKey::Pre(k, n),
        (None, None, Some(_)) => PreKey::DevOnly,
        _ => PreKey::Final,
    };
    Some(Pep440Version {
        epoch,
        release,
        pre,
        post: post.map_or(PostKey::None, PostKey::Post),
        dev: dev.map_or(DevKey::None, DevKey::Dev),
        local,
    })
}

impl VersionScheme for Pep440Scheme {
    type V = Pep440Version;

    fn parse(&self, s: &str) -> Option<Pep440Version> {
        parse_pep440(s)
    }

    fn is_prerelease(&self, v: &Pep440Version) -> bool {
        matches!(v.pre, PreKey::Pre(..) | PreKey::DevOnly) || matches!(v.dev, DevKey::Dev(_))
    }

    fn segments(&self, v: &Pep440Version) -> (u64, u64, u64) {
        let at = |i: usize| v.release.get(i).copied().unwrap_or(0);
        (at(0), at(1), at(2))
    }

    fn epoch(&self, v: &Pep440Version) -> u64 {
        v.epoch
    }

    /// PEP 440 specifiers (`>=1.0,<2`, `~=1.4.2`, `==1.2.*`, `===1.0`) and
    /// Poetry (`^1.2`, `~1.2`, `a || b`). `!=`, `<`, `<=` have no floor.
    fn lower_bound(&self, constraint: &str) -> Option<Pep440Version> {
        lower_bound_scan(
            constraint,
            |c| c.is_whitespace() || c == ',',
            |clause| {
                let v = strip_lower_op(clause)?;
                let v = v.strip_suffix(".*").unwrap_or(v);
                parse_pep440(v)
            },
        )
    }
}

#[cfg(test)]
#[path = "pep440_tests.rs"]
mod tests;
