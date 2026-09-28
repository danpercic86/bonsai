//! Bounded XML pull reader shared by the NuGet adapter (project files,
//! `Directory.*.props`, `packages.config`) and the `NuGet.Config` extractor
//! (contract §4.2) `[HIGH-2]`.
//!
//! - Only `quick_xml::Reader` pull events are consumed (no serde, no DOM).
//! - Any `<!DOCTYPE` (and so any `<!ENTITY`) is `UnsupportedFeature`:
//!   rejected by a byte pre-scan before the reader is built, and again if
//!   the reader ever yields `Event::DocType`. Entities are NEVER expanded:
//!   text and attribute values resolve only the five predefined entities
//!   and numeric character references; any other `&name;` is
//!   `UnsupportedFeature`. (NuGet itself prohibits DTDs — checked locally
//!   with the .NET 10 SDK: "For security reasons DTD is prohibited".)
//! - Depth > [`XML_MAX_DEPTH`], events > [`XML_MAX_NODES`] or one value
//!   (attribute or accumulated text) > [`XML_MAX_VALUE`] bytes is
//!   `LimitExceeded`.
//! - The sink decides what is materialised: attributes are decoded only
//!   when the sink asks for one by name ([`Elem::attr`]), and text only
//!   inside elements for which [`XmlSink::wants_text`] is true. Everything
//!   else (e.g. `<packageSourceCredentials>`) is walked and counted but
//!   never copied out `[MEDIUM-1]`.
//! - Errors carry `(kind, line, col)` only; a quick-xml error is never
//!   `Display`ed `[MEDIUM-1]`.

use quick_xml::events::{BytesStart, Event};
use quick_xml::Reader;

use crate::adapter::line_col;
use crate::model::ParseErrorKind;

pub(crate) const XML_MAX_DEPTH: usize = 64;
pub(crate) const XML_MAX_NODES: usize = 2_000_000;
/// No registry URL, package id or version comes near this.
pub(crate) const XML_MAX_VALUE: usize = 64 * 1024;

/// A located XML failure. No text `[MEDIUM-1]`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct XmlError {
    pub(crate) kind: ParseErrorKind,
    pub(crate) line: Option<u32>,
    pub(crate) col: Option<u32>,
}

impl XmlError {
    pub(crate) fn new(kind: ParseErrorKind) -> Self {
        XmlError {
            kind,
            line: None,
            col: None,
        }
    }
}

/// One start (or empty) element, handed to the sink. Attribute values are
/// decoded lazily, one by name.
pub(crate) struct Elem<'a, 'b> {
    start: &'b BytesStart<'a>,
}

impl Elem<'_, '_> {
    /// The local name (namespace prefix dropped).
    pub(crate) fn name(&self) -> &str {
        std::str::from_utf8(self.start.local_name().into_inner()).unwrap_or("")
    }

    /// The unescaped value of the first attribute whose local name equals
    /// `key` ASCII-case-insensitively. A malformed attribute list, a bad
    /// entity or an oversize value is an error.
    /// Two attributes matching `key` (even differing only in case) are
    /// ambiguous ⇒ `InvalidSyntax`.
    pub(crate) fn attr(&self, key: &str) -> Result<Option<String>, XmlError> {
        let mut found = None;
        for a in self.start.attributes() {
            let a = a.map_err(|_| XmlError::new(ParseErrorKind::InvalidSyntax))?;
            if !a
                .key
                .local_name()
                .as_ref()
                .eq_ignore_ascii_case(key.as_bytes())
            {
                continue;
            }
            if found.is_some() {
                return Err(XmlError::new(ParseErrorKind::InvalidSyntax));
            }
            if a.value.len() > XML_MAX_VALUE {
                return Err(XmlError::new(ParseErrorKind::LimitExceeded));
            }
            let raw = std::str::from_utf8(&a.value)
                .map_err(|_| XmlError::new(ParseErrorKind::InvalidSyntax))?;
            found = Some(unescape(raw)?);
        }
        Ok(found)
    }
}

/// What the walk reports. `stack` holds the local names of the OPEN
/// elements, outermost first; in `start` it excludes the new element, in
/// `wants_text` / `text` / `end` it includes the element concerned.
pub(crate) trait XmlSink {
    fn start(&mut self, stack: &[String], elem: &Elem<'_, '_>) -> Result<(), XmlError>;
    fn wants_text(&self, _stack: &[String]) -> bool {
        false
    }
    /// The whole (unescaped) text of an element that `wants_text`,
    /// delivered once, when it closes.
    fn text(&mut self, _stack: &[String], _text: &str) -> Result<(), XmlError> {
        Ok(())
    }
    fn end(&mut self, _stack: &[String]) -> Result<(), XmlError> {
        Ok(())
    }
}

/// Resolves the five predefined entities and `&#N;` / `&#xN;`. Anything
/// else ⇒ `UnsupportedFeature` (never expanded).
pub(crate) fn unescape(raw: &str) -> Result<String, XmlError> {
    let mut out = String::with_capacity(raw.len());
    let mut rest = raw;
    while let Some(i) = rest.find('&') {
        out.push_str(&rest[..i]);
        let tail = &rest[i + 1..];
        let end = tail
            .find(';')
            .ok_or(XmlError::new(ParseErrorKind::InvalidSyntax))?;
        out.push(resolve_ref(&tail[..end])?);
        rest = &tail[end + 1..];
    }
    out.push_str(rest);
    Ok(out)
}

/// One reference name (between `&` and `;`).
fn resolve_ref(name: &str) -> Result<char, XmlError> {
    let unsupported = XmlError::new(ParseErrorKind::UnsupportedFeature);
    let c = match name {
        "lt" => '<',
        "gt" => '>',
        "amp" => '&',
        "apos" => '\'',
        "quot" => '"',
        _ => {
            let num = name.strip_prefix('#').ok_or(unsupported)?;
            let code = match num.strip_prefix(['x', 'X']) {
                Some(hex) => u32::from_str_radix(hex, 16),
                None => num.parse::<u32>(),
            }
            .map_err(|_| XmlError::new(ParseErrorKind::InvalidSyntax))?;
            char::from_u32(code)
                .filter(|c| *c != '\0')
                .ok_or(XmlError::new(ParseErrorKind::InvalidSyntax))?
        }
    };
    Ok(c)
}

/// `[HIGH-2]` a case-insensitive `<!DOCTYPE` / `<!ENTITY` anywhere ⇒ reject
/// before parsing (a DTD inside a comment is rejected too: fail closed).
fn has_dtd(text: &str) -> bool {
    text.as_bytes().windows(8).any(|w| {
        w.starts_with(b"<!")
            && (w[2..].eq_ignore_ascii_case(b"DOCTYP") || w[2..].eq_ignore_ascii_case(b"ENTITY"))
    })
}

fn at(text: &str, pos: u64, kind: ParseErrorKind) -> XmlError {
    let (line, col) = line_col(text, usize::try_from(pos).unwrap_or(usize::MAX));
    XmlError { kind, line, col }
}

/// Appends `s` to the open text buffer, enforcing [`XML_MAX_VALUE`].
fn push_text(buf: &mut Option<String>, s: &str) -> Result<(), XmlError> {
    if let Some(b) = buf {
        if b.len().saturating_add(s.len()) > XML_MAX_VALUE {
            return Err(XmlError::new(ParseErrorKind::LimitExceeded));
        }
        b.push_str(s);
    }
    Ok(())
}

/// Walks `text` once, iteratively, feeding `sink`. A leading U+FEFF is
/// stripped.
pub(crate) fn walk(text: &str, sink: &mut dyn XmlSink) -> Result<(), XmlError> {
    let text = text.strip_prefix('\u{FEFF}').unwrap_or(text);
    if has_dtd(text) {
        return Err(XmlError::new(ParseErrorKind::UnsupportedFeature));
    }
    let mut reader = Reader::from_str(text);
    let mut stack: Vec<String> = Vec::new();
    // One text buffer per open element (`Some` when the sink wants it).
    let mut bufs: Vec<Option<String>> = Vec::new();
    let mut nodes = 0usize;
    loop {
        let pos = reader.buffer_position();
        let ev = reader
            .read_event()
            .map_err(|_| at(text, reader.error_position(), ParseErrorKind::InvalidSyntax))?;
        nodes += 1;
        if nodes > XML_MAX_NODES {
            return Err(at(text, pos, ParseErrorKind::LimitExceeded));
        }
        let located = |e: XmlError| {
            if e.line.is_some() {
                e
            } else {
                at(text, pos, e.kind)
            }
        };
        match ev {
            Event::Start(s) => {
                if stack.len() >= XML_MAX_DEPTH {
                    return Err(at(text, pos, ParseErrorKind::LimitExceeded));
                }
                sink.start(&stack, &Elem { start: &s }).map_err(located)?;
                let name = Elem { start: &s }.name().to_string();
                stack.push(name);
                bufs.push(sink.wants_text(&stack).then(String::new));
            }
            Event::Empty(s) => {
                if stack.len() >= XML_MAX_DEPTH {
                    return Err(at(text, pos, ParseErrorKind::LimitExceeded));
                }
                sink.start(&stack, &Elem { start: &s }).map_err(located)?;
                stack.push(Elem { start: &s }.name().to_string());
                sink.end(&stack).map_err(located)?;
                stack.pop();
            }
            Event::End(_) => {
                if let Some(Some(t)) = bufs.pop() {
                    sink.text(&stack, &t).map_err(located)?;
                }
                sink.end(&stack).map_err(located)?;
                if stack.pop().is_none() {
                    return Err(at(text, pos, ParseErrorKind::InvalidSyntax));
                }
            }
            Event::Text(t) => {
                if let Some(buf) = bufs.last_mut() {
                    if buf.is_some() {
                        let s = std::str::from_utf8(&t)
                            .map_err(|_| at(text, pos, ParseErrorKind::InvalidSyntax))?;
                        push_text(buf, s).map_err(located)?;
                    }
                }
            }
            Event::CData(t) => {
                if let Some(buf) = bufs.last_mut() {
                    if buf.is_some() {
                        let s = std::str::from_utf8(&t)
                            .map_err(|_| at(text, pos, ParseErrorKind::InvalidSyntax))?;
                        push_text(buf, s).map_err(located)?;
                    }
                }
            }
            Event::GeneralRef(r) => {
                let name = std::str::from_utf8(&r)
                    .map_err(|_| at(text, pos, ParseErrorKind::InvalidSyntax))?;
                // Resolved even when unwanted, so an unknown entity anywhere
                // is rejected.
                let c = resolve_ref(name).map_err(located)?;
                if let Some(buf) = bufs.last_mut() {
                    push_text(buf, c.encode_utf8(&mut [0u8; 4])).map_err(located)?;
                }
            }
            Event::DocType(_) => return Err(at(text, pos, ParseErrorKind::UnsupportedFeature)),
            Event::Comment(_) | Event::Decl(_) | Event::PI(_) => {}
            Event::Eof => break,
        }
    }
    if stack.is_empty() {
        Ok(())
    } else {
        Err(at(
            text,
            reader.buffer_position(),
            ParseErrorKind::InvalidSyntax,
        ))
    }
}

#[cfg(test)]
#[path = "xml_events_tests.rs"]
mod tests;
