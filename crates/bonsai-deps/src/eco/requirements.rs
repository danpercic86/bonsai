//! `requirements*.txt` (E2 PyPI row): an iterative line parser `[HIGH-2]`.
//!
//! - A leading U+FEFF is stripped; lines split on `\r\n`, `\n` and a lone
//!   `\r`; a trailing `\` joins the next line; `#` starts a comment at the
//!   line start or after whitespace (so `#egg=` in a URL is kept) — pip's
//!   own `COMMENT_RE` / line-join rules.
//! - `-r` / `-c` (and long forms) are SKIPPED here (E2); their targets are
//!   followed by the PyPI registry-context loader, which reads them inside
//!   the workdir only and turns any index option it finds there into an
//!   ecosystem-wide private signal.
//! - `-i` / `--index-url` / `--extra-index-url` / `-f` / `--find-links` /
//!   `--no-index` in THIS file route every registry dependency of the file
//!   (E2 "every dep in that file"): a non-public index host ⇒
//!   `Registry(Host(h))`; an index with no host (a path, `${VAR}`, a typo),
//!   find-links to a path, or `--no-index` ⇒ `Registry(Named(""))`, which
//!   `private_reason` reads as CustomRegistry with no host.
//! - `-e` / bare paths / bare URLs ⇒ Path / Git / Url, named by `#egg=`
//!   when present (a nameless local requirement produces no row — it is
//!   never sent either way).

use crate::adapter::{DepSource, Dependency, ParseError, RegistryRef};
use crate::catalog::is_public_host;
use crate::model::{DepScope, Ecosystem, ParseErrorKind};
use crate::registry_ctx::extract_host;

use super::pep508::{parse_req, url_source};

/// The `Named` registry of a hostless / unknown index (never has a host).
pub(crate) const UNNAMED_INDEX: &str = "";

/// One logical line: 1-based number of its first physical line + text.
pub(crate) struct Line {
    pub(crate) no: u32,
    pub(crate) text: String,
}

/// Physical lines: `\r\n`, `\n` and a lone `\r` all end a line.
pub(crate) fn physical_lines(text: &str) -> impl Iterator<Item = &str> {
    let text = text.strip_prefix('\u{FEFF}').unwrap_or(text);
    text.split('\n')
        .flat_map(|l| l.strip_suffix('\r').unwrap_or(l).split('\r'))
}

/// pip `COMMENT_RE`: `(^|\s+)#.*$`.
fn strip_comment(line: &str) -> &str {
    let b = line.as_bytes();
    for (i, c) in b.iter().enumerate() {
        if *c == b'#' && (i == 0 || b[i - 1].is_ascii_whitespace()) {
            return &line[..i];
        }
    }
    line
}

/// Logical lines (comments stripped, `\` continuations joined, blanks
/// dropped).
pub(crate) fn logical_lines(text: &str) -> Vec<Line> {
    let mut out = Vec::new();
    let mut cur: Option<Line> = None;
    for (i, raw) in physical_lines(text).enumerate() {
        let no = u32::try_from(i + 1).unwrap_or(u32::MAX);
        let (body, cont) = match raw.strip_suffix('\\') {
            Some(b) => (b, true),
            None => (raw, false),
        };
        let line = cur.get_or_insert_with(|| Line {
            no,
            text: String::new(),
        });
        line.text.push_str(body);
        if !cont {
            if let Some(l) = cur.take() {
                let t = strip_comment(&l.text).trim().to_string();
                if !t.is_empty() {
                    out.push(Line { no: l.no, text: t });
                }
            }
        }
    }
    if let Some(l) = cur {
        let t = strip_comment(&l.text).trim().to_string();
        if !t.is_empty() {
            out.push(Line { no: l.no, text: t });
        }
    }
    out
}

/// A recognised option line.
pub(crate) enum Opt {
    /// `-r` / `-c`: an include (path as written).
    Include(String),
    /// An index / find-links URL (`-i`, `--index-url`, `--extra-index-url`,
    /// `-f`, `--find-links`).
    Index(String),
    /// `--no-index`.
    NoIndex,
    /// `-e` / `--editable`.
    Editable(String),
    /// Any other option (`--hash`, `--pre`, `--trusted-host`, …).
    Other,
}

const INCLUDE: &[&str] = &["-r", "--requirement", "-c", "--constraint"];
const INDEX: &[&str] = &[
    "-i",
    "--index-url",
    "--extra-index-url",
    "-f",
    "--find-links",
];
const EDITABLE: &[&str] = &["-e", "--editable"];

/// `-x VALUE`, `-xVALUE`, `--long=VALUE`, `--long VALUE`.
fn opt_value<'a>(line: &'a str, names: &[&str]) -> Option<&'a str> {
    for n in names {
        let Some(rest) = line.strip_prefix(n) else {
            continue;
        };
        let v = if n.starts_with("--") {
            match rest.strip_prefix('=') {
                Some(v) => v,
                None if rest.starts_with(char::is_whitespace) => rest,
                None => continue,
            }
        } else {
            rest.strip_prefix('=').unwrap_or(rest)
        };
        return Some(v.trim());
    }
    None
}

/// Classifies an option line (one starting with `-`).
pub(crate) fn option(line: &str) -> Opt {
    if let Some(v) = opt_value(line, INCLUDE) {
        return Opt::Include(v.to_string());
    }
    if let Some(v) = opt_value(line, INDEX) {
        return Opt::Index(v.to_string());
    }
    if line == "--no-index" || line.starts_with("--no-index ") {
        return Opt::NoIndex;
    }
    if let Some(v) = opt_value(line, EDITABLE) {
        return Opt::Editable(v.to_string());
    }
    Opt::Other
}

/// The `#egg=<name>` fragment of a URL / path requirement.
fn egg_name(s: &str) -> Option<&str> {
    let frag = s.split_once("#egg=")?.1;
    let name = frag.split(['&', ' ']).next()?.trim();
    (!name.is_empty()).then_some(name)
}

/// A line that is a local path / URL rather than a PEP 508 name.
fn is_location(s: &str) -> bool {
    s.starts_with(['.', '/', '~', '\\'])
        || s.contains("://")
        || s.contains(['/', '\\'])
        || s.get(1..3) == Some(":\\")
        || is_archive(s)
}

/// Archive suffixes pip installs as a local file (`foo-1.0.tar.gz`).
const ARCHIVES: &[&str] = &[".whl", ".tar.gz", ".tgz", ".tar.bz2", ".zip"];

/// `[P125c2 review]` a bare archive file name is a location (Local), never a
/// registry name — `foo-1.0.tar.gz` would otherwise parse as a PEP 508 name.
fn is_archive(s: &str) -> bool {
    let file = s
        .split(['#', ';'])
        .next()
        .unwrap_or(s)
        .split_whitespace()
        .next()
        .unwrap_or("")
        .to_ascii_lowercase();
    ARCHIVES.iter().any(|a| file.ends_with(a))
}

/// The source of an `-e` value or a bare location.
fn location_source(s: &str) -> DepSource {
    if s.contains("://") || s.starts_with("git+") {
        url_source(s)
    } else {
        DepSource::Path
    }
}

/// The requirement part of a line: per-requirement options (` --hash=…`)
/// are cut at the first whitespace-preceded `-`.
fn requirement_part(line: &str) -> &str {
    let b = line.as_bytes();
    for i in 1..b.len() {
        if b[i] == b'-' && b[i - 1].is_ascii_whitespace() {
            return line[..i].trim_end();
        }
    }
    line
}

/// How the file's own index options route its registry dependencies.
fn file_route(indexes: &[String], no_index: bool) -> RegistryRef {
    if no_index {
        return RegistryRef::Named(UNNAMED_INDEX.to_string());
    }
    let mut private: Option<String> = None;
    for raw in indexes {
        match extract_host(raw) {
            Some(h) if is_public_host(Ecosystem::Pypi, &h) => {}
            Some(h) => {
                private.get_or_insert(h);
            }
            None => return RegistryRef::Named(UNNAMED_INDEX.to_string()),
        }
    }
    private.map_or(RegistryRef::Default, RegistryRef::Host)
}

fn row(
    name: &str,
    req_spec: Option<String>,
    resolved: Option<String>,
    source: DepSource,
) -> Dependency {
    Dependency {
        name: name.to_string(),
        scope: DepScope::Normal,
        declared: req_spec,
        resolved,
        source,
    }
}

/// Parses one requirements file. A line that is neither an option, a
/// location nor a PEP 508 requirement ⇒ `InvalidSyntax` at that line (no
/// text `[MEDIUM-1]`).
pub(crate) fn parse(path: &str, text: &str) -> Result<Vec<Dependency>, ParseError> {
    let mut deps = Vec::new();
    let mut indexes = Vec::new();
    let mut no_index = false;
    for line in logical_lines(text) {
        let t = line.text.as_str();
        if t.starts_with('-') {
            match option(t) {
                Opt::Index(v) => indexes.push(v),
                Opt::NoIndex => no_index = true,
                Opt::Editable(v) => {
                    if let Some(n) = egg_name(&v) {
                        deps.push(row(n, None, None, location_source(&v)));
                    }
                }
                Opt::Include(_) | Opt::Other => {}
            }
            continue;
        }
        let req = requirement_part(t);
        if is_archive(req) && !req.contains('@') {
            if let Some(n) = egg_name(req) {
                deps.push(row(n, None, None, location_source(req)));
            }
        } else if let Some(r) = parse_req(req) {
            let resolved = r.resolved();
            deps.push(row(&r.name, r.spec, resolved, r.source));
        } else if is_location(req) {
            if let Some(n) = egg_name(req) {
                deps.push(row(n, None, None, location_source(req)));
            }
        } else {
            return Err(ParseError::at(
                path,
                ParseErrorKind::InvalidSyntax,
                Some(line.no),
                None,
            ));
        }
    }
    let route = file_route(&indexes, no_index);
    if route != RegistryRef::Default {
        for d in &mut deps {
            if d.source == DepSource::Registry(RegistryRef::Default) {
                d.source = DepSource::Registry(route.clone());
            }
        }
    }
    Ok(deps)
}
