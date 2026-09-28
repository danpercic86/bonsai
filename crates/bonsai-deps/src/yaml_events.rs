//! Bounded YAML event reader shared by the pnpm / yarn-berry adapters and
//! the YAML registry configs (contract §4.2) `[HIGH-2]`.
//!
//! - Only `yaml_rust2::parser::Parser` events are consumed; `YamlLoader` is
//!   never used.
//! - The event loop is driven HERE with `Parser::next_token`, not through
//!   `Parser::load` + an `EventReceiver`: `load` recurses once per nesting
//!   level (`load_node → load_mapping → load_node`) and a receiver cannot
//!   abort it, so a nesting bomb would exhaust the stack before a receiver's
//!   depth check ran. Driving the (iterative) parser ourselves lets every cap
//!   stop the parse at the first offending event.
//! - Any alias, and any node carrying an anchor, is `UnsupportedFeature`
//!   (so an alias bomb is rejected at its first anchor, before any
//!   expansion). Depth > [`YAML_MAX_DEPTH`] or nodes > [`YAML_MAX_NODES`] is
//!   `LimitExceeded`. Complex (non-scalar) mapping keys are
//!   `UnsupportedFeature`.
//! - A `keep` predicate on the key path decides which subtrees are
//!   materialised; everything else is walked (and counted) but not stored.
//! - Errors carry `(kind, line, col)` only — the scanner's message text is
//!   never read `[MEDIUM-1]`.

use yaml_rust2::parser::{Event, Parser};
use yaml_rust2::scanner::Marker;

use crate::model::ParseErrorKind;

pub const YAML_MAX_DEPTH: usize = 64;
pub const YAML_MAX_NODES: usize = 2_000_000;

/// Path element used for sequence items in `keep` paths.
pub(crate) const SEQ_ITEM: &str = "-";

/// A materialised YAML node. Mapping order is preserved; duplicate keys are
/// kept as they appear (callers take the first).
///
/// `Debug` exists for test assertions. The callers' `keep` predicates stop
/// credential scalars from being materialised at all, and a `YNode` is never
/// logged or put in an error.
/// `[NEW-2]` no Debug outside tests; no Clone (nothing needs one).
#[cfg_attr(test, derive(Debug, PartialEq))]
pub(crate) enum YNode {
    Scalar(String),
    /// Items are materialised for the tests; no adapter reads a sequence yet.
    Seq(#[cfg_attr(not(test), allow(dead_code))] Vec<YNode>),
    Map(Vec<(String, YNode)>),
    /// A subtree the `keep` predicate rejected.
    Skipped,
}

impl YNode {
    pub(crate) fn get(&self, key: &str) -> Option<&YNode> {
        match self {
            YNode::Map(m) => m.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    pub(crate) fn as_str(&self) -> Option<&str> {
        match self {
            YNode::Scalar(s) => Some(s),
            _ => None,
        }
    }

    pub(crate) fn entries(&self) -> &[(String, YNode)] {
        match self {
            YNode::Map(m) => m,
            _ => &[],
        }
    }
}

/// A located YAML failure. No text: `[MEDIUM-1]`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct YamlError {
    pub(crate) kind: ParseErrorKind,
    pub(crate) line: Option<u32>,
    pub(crate) col: Option<u32>,
}

impl YamlError {
    fn at(kind: ParseErrorKind, m: &Marker) -> Self {
        YamlError {
            kind,
            line: u32::try_from(m.line()).ok(),
            col: u32::try_from(m.col().saturating_add(1)).ok(),
        }
    }
}

enum Frame {
    Map {
        entries: Vec<(String, YNode)>,
        key: Option<String>,
    },
    Seq {
        items: Vec<YNode>,
    },
}

/// Per-document builder state.
struct Builder<'k> {
    keep: &'k dyn Fn(&[String]) -> bool,
    stack: Vec<Frame>,
    /// Key path of the node about to be produced (one entry per open frame).
    path: Vec<String>,
    /// >0 while inside a subtree that is being skipped.
    skip_depth: usize,
    nodes: usize,
    done: Option<YNode>,
}

enum Slot {
    /// The next node is a mapping key.
    Key,
    /// The next node is a value at `path`.
    Value,
}

impl Builder<'_> {
    fn depth(&self) -> usize {
        self.stack.len() + self.skip_depth
    }

    fn slot(&self) -> Slot {
        match self.stack.last() {
            Some(Frame::Map { key: None, .. }) => Slot::Key,
            _ => Slot::Value,
        }
    }

    /// Path of the value about to be placed.
    fn value_path(&self) -> Vec<String> {
        let mut p = self.path.clone();
        match self.stack.last() {
            Some(Frame::Map { key: Some(k), .. }) => p.push(k.clone()),
            Some(Frame::Seq { .. }) => p.push(SEQ_ITEM.to_string()),
            _ => {}
        }
        p
    }

    fn place(&mut self, node: YNode) {
        match self.stack.last_mut() {
            Some(Frame::Map { entries, key }) => {
                if let Some(k) = key.take() {
                    entries.push((k, node));
                }
            }
            Some(Frame::Seq { items }) => items.push(node),
            None => self.done = Some(node),
        }
    }

    fn count(&mut self, m: &Marker) -> Result<(), YamlError> {
        self.nodes += 1;
        if self.nodes > YAML_MAX_NODES {
            return Err(YamlError::at(ParseErrorKind::LimitExceeded, m));
        }
        Ok(())
    }

    fn scalar(&mut self, s: String, m: &Marker) -> Result<(), YamlError> {
        self.count(m)?;
        if self.skip_depth > 0 {
            return Ok(());
        }
        match self.slot() {
            Slot::Key => {
                if let Some(Frame::Map { key, .. }) = self.stack.last_mut() {
                    *key = Some(s);
                }
            }
            Slot::Value => {
                let node = if (self.keep)(&self.value_path()) {
                    YNode::Scalar(s)
                } else {
                    YNode::Skipped
                };
                self.place(node);
            }
        }
        Ok(())
    }

    fn start(&mut self, is_map: bool, m: &Marker) -> Result<(), YamlError> {
        self.count(m)?;
        if self.depth() + 1 > YAML_MAX_DEPTH {
            return Err(YamlError::at(ParseErrorKind::LimitExceeded, m));
        }
        if self.skip_depth > 0 {
            self.skip_depth += 1;
            return Ok(());
        }
        if matches!(self.slot(), Slot::Key) {
            // `? [a, b]: x` — a complex key. The adapters never need one.
            return Err(YamlError::at(ParseErrorKind::UnsupportedFeature, m));
        }
        let path = self.value_path();
        if !(self.keep)(&path) {
            self.place(YNode::Skipped);
            self.skip_depth = 1;
            return Ok(());
        }
        self.path = path;
        self.stack.push(if is_map {
            Frame::Map {
                entries: Vec::new(),
                key: None,
            }
        } else {
            Frame::Seq { items: Vec::new() }
        });
        Ok(())
    }

    fn end(&mut self) {
        if self.skip_depth > 0 {
            self.skip_depth -= 1;
            return;
        }
        let node = match self.stack.pop() {
            Some(Frame::Map { entries, .. }) => YNode::Map(entries),
            Some(Frame::Seq { items }) => YNode::Seq(items),
            None => return,
        };
        self.path.pop();
        self.place(node);
    }
}

/// Parses every document in `text`. `keep(path)` is asked for each value
/// (container or scalar) before it is stored; `path` holds the mapping keys
/// from the document root, with [`SEQ_ITEM`] for sequence items.
pub(crate) fn parse_documents(
    text: &str,
    keep: &dyn Fn(&[String]) -> bool,
) -> Result<Vec<YNode>, YamlError> {
    // yaml-rust2 keeps a leading U+FEFF INSIDE the first key (pinned by the
    // `leading_bom_*` tests), which would hide it from every `keep` path.
    let text = text.strip_prefix('\u{FEFF}').unwrap_or(text);
    let mut parser = Parser::new_from_str(text);
    let mut docs = Vec::new();
    let mut total_nodes = 0usize;
    let mut b: Option<Builder<'_>> = None;
    loop {
        let (ev, mark) = parser
            .next_token()
            .map_err(|e| YamlError::at(ParseErrorKind::InvalidSyntax, e.marker()))?;
        match ev {
            Event::StreamEnd => break,
            Event::DocumentStart => {
                b = Some(Builder {
                    keep,
                    stack: Vec::new(),
                    path: Vec::new(),
                    skip_depth: 0,
                    nodes: total_nodes,
                    done: None,
                });
            }
            Event::DocumentEnd => {
                if let Some(done) = b.take() {
                    total_nodes = done.nodes;
                    docs.push(done.done.unwrap_or(YNode::Scalar(String::new())));
                }
            }
            Event::Alias(_) => {
                return Err(YamlError::at(ParseErrorKind::UnsupportedFeature, &mark))
            }
            Event::Scalar(_, _, anchor, _)
            | Event::SequenceStart(anchor, _)
            | Event::MappingStart(anchor, _)
                if anchor != 0 =>
            {
                return Err(YamlError::at(ParseErrorKind::UnsupportedFeature, &mark));
            }
            Event::Scalar(s, ..) => builder(&mut b, &mark)?.scalar(s, &mark)?,
            Event::SequenceStart(..) => builder(&mut b, &mark)?.start(false, &mark)?,
            Event::MappingStart(..) => builder(&mut b, &mark)?.start(true, &mark)?,
            Event::SequenceEnd | Event::MappingEnd => builder(&mut b, &mark)?.end(),
            Event::StreamStart | Event::Nothing => {}
        }
    }
    Ok(docs)
}

fn builder<'a, 'k>(
    b: &'a mut Option<Builder<'k>>,
    mark: &Marker,
) -> Result<&'a mut Builder<'k>, YamlError> {
    b.as_mut()
        .ok_or_else(|| YamlError::at(ParseErrorKind::InvalidSyntax, mark))
}

/// Convenience: the first document, or an empty map for an empty stream.
pub(crate) fn parse_first(
    text: &str,
    keep: &dyn Fn(&[String]) -> bool,
) -> Result<YNode, YamlError> {
    Ok(parse_documents(text, keep)?
        .into_iter()
        .next()
        .unwrap_or(YNode::Map(Vec::new())))
}

#[cfg(test)]
#[path = "yaml_events_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "yaml_corpus_tests.rs"]
mod corpus_tests;
