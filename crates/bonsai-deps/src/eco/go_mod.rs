//! Go adapter (E2 Go row): `go.mod` is both the manifest and the lock
//! (Go ≥ 1.17 lists every module in the build list, the indirect ones
//! marked `// indirect`); `go.sum` is ignored.
//!
//! An iterative line lexer (no recursion, `[HIGH-2]`): `//` comments,
//! `"…"` / `` `…` `` quoted paths, `verb ( … )` blocks. `require` without
//! `// indirect` is a direct dependency whose declared version IS the
//! resolved one (MVS). `replace X => ./dir` ⇒ `Path`; `replace X => Y vN`
//! ⇒ the dependency is queried as `Y@vN` (so `GOPRIVATE` matching runs on
//! the path that would be sent).

use crate::adapter::{
    DepSource, Dependency, EcosystemAdapter, LockEntry, ParseError, ParseInput, ParsedGroup,
    RegistryRef,
};
use crate::discover::FileSet;
use crate::model::{DepScope, Ecosystem, ParseErrorKind};

pub struct GoAdapter;

pub(crate) const MANIFEST: &str = "go.mod";

/// First Go version whose go.mod lists the full module graph.
const FULL_GRAPH_MINOR: u32 = 17;

struct Require {
    path: String,
    version: String,
    indirect: bool,
}

struct Replace {
    old: String,
    old_version: Option<String>,
    new: String,
    new_version: Option<String>,
}

#[derive(Default)]
struct GoMod {
    module: Option<String>,
    go_minor: Option<u32>,
    requires: Vec<Require>,
    replaces: Vec<Replace>,
}

/// Where a lex error sits (1-based line).
struct Bad(u32);

/// One line's tokens and its trailing `//` comment (trimmed).
fn lex(line: &str, no: u32) -> Result<(Vec<String>, &str), Bad> {
    let mut toks = Vec::new();
    let b = line.as_bytes();
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b' ' | b'\t' | b'\r' => i += 1,
            b'/' if b.get(i + 1) == Some(&b'/') => return Ok((toks, line[i + 2..].trim())),
            b'(' | b')' => {
                toks.push((b[i] as char).to_string());
                i += 1;
            }
            b'=' if b.get(i + 1) == Some(&b'>') => {
                toks.push("=>".to_string());
                i += 2;
            }
            b'"' => {
                let mut s = String::new();
                let mut j = i + 1;
                loop {
                    match b.get(j) {
                        None => return Err(Bad(no)),
                        Some(b'"') => break,
                        Some(b'\\') => {
                            let c = line
                                .get(j + 1..)
                                .and_then(|r| r.chars().next())
                                .ok_or(Bad(no))?;
                            s.push(c);
                            j += 1 + c.len_utf8();
                        }
                        Some(_) => {
                            let c = line
                                .get(j..)
                                .and_then(|r| r.chars().next())
                                .ok_or(Bad(no))?;
                            s.push(c);
                            j += c.len_utf8();
                        }
                    }
                }
                toks.push(s);
                i = j + 1;
            }
            b'`' => {
                let rest = &line[i + 1..];
                let end = rest.find('`').ok_or(Bad(no))?;
                toks.push(rest[..end].to_string());
                i += end + 2;
            }
            _ => {
                let start = i;
                while i < b.len()
                    && !matches!(b[i], b' ' | b'\t' | b'\r' | b'(' | b')' | b'"' | b'`')
                    && !(b[i] == b'/' && b.get(i + 1) == Some(&b'/'))
                    && !(b[i] == b'=' && b.get(i + 1) == Some(&b'>'))
                {
                    i += 1;
                }
                toks.push(line[start..i].to_string());
            }
        }
    }
    Ok((toks, ""))
}

/// Go's `isIndirect`: the comment is `indirect` or starts `indirect;`.
fn is_indirect(comment: &str) -> bool {
    comment == "indirect" || comment.starts_with("indirect;")
}

/// `1.21` / `1.21.3` ⇒ Some(21) when the major is 1.
fn go_minor(v: &str) -> Option<u32> {
    let mut parts = v.split('.');
    (parts.next()? == "1").then_some(())?;
    parts.next()?.parse().ok()
}

fn directive(
    m: &mut GoMod,
    verb: &str,
    args: &[String],
    comment: &str,
    no: u32,
) -> Result<(), Bad> {
    match verb {
        "module" => m.module = args.first().cloned(),
        "go" => m.go_minor = args.first().and_then(|v| go_minor(v)),
        "require" => match args {
            [path, version] => m.requires.push(Require {
                path: path.clone(),
                version: version.clone(),
                indirect: is_indirect(comment),
            }),
            _ => return Err(Bad(no)),
        },
        "replace" => {
            let arrow = args.iter().position(|a| a == "=>").ok_or(Bad(no))?;
            let (left, right) = (&args[..arrow], &args[arrow + 1..]);
            let (old, old_version) = match left {
                [o] => (o.clone(), None),
                [o, v] => (o.clone(), Some(v.clone())),
                _ => return Err(Bad(no)),
            };
            let (new, new_version) = match right {
                [n] => (n.clone(), None),
                [n, v] => (n.clone(), Some(v.clone())),
                _ => return Err(Bad(no)),
            };
            m.replaces.push(Replace {
                old,
                old_version,
                new,
                new_version,
            });
        }
        // exclude, retract, toolchain, godebug, tool, ignore, …: no deps.
        _ => {}
    }
    Ok(())
}

fn parse_mod(text: &str) -> Result<GoMod, Bad> {
    let text = text.strip_prefix('\u{FEFF}').unwrap_or(text);
    let mut m = GoMod::default();
    let mut block: Option<String> = None;
    let mut last = 0;
    for (i, line) in text.split('\n').enumerate() {
        let no = u32::try_from(i + 1).unwrap_or(u32::MAX);
        last = no;
        let (toks, comment) = lex(line, no)?;
        let Some(first) = toks.first() else { continue };
        if let Some(verb) = &block {
            if toks.len() == 1 && first == ")" {
                block = None;
            } else {
                let verb = verb.clone();
                directive(&mut m, &verb, &toks, comment, no)?;
            }
            continue;
        }
        match &toks[1..] {
            [open] if open == "(" => block = Some(first.clone()),
            args => directive(&mut m, first, args, comment, no)?,
        }
    }
    if block.is_some() {
        return Err(Bad(last));
    }
    Ok(m)
}

/// `(name, version, source)` for a required module after `replace`.
fn apply_replace(m: &GoMod, r: &Require) -> (String, Option<String>, DepSource) {
    let exact = m
        .replaces
        .iter()
        .find(|x| x.old == r.path && x.old_version.as_deref() == Some(r.version.as_str()));
    let any = || {
        m.replaces
            .iter()
            .find(|x| x.old == r.path && x.old_version.is_none())
    };
    match exact.or_else(any) {
        None => (
            r.path.clone(),
            Some(r.version.clone()),
            DepSource::Registry(RegistryRef::Default),
        ),
        // No version on the right: a local directory (Go rejects anything
        // else), never sent.
        Some(x) => match &x.new_version {
            None => (r.path.clone(), None, DepSource::Path),
            Some(v) => (
                x.new.clone(),
                Some(v.clone()),
                DepSource::Registry(RegistryRef::Default),
            ),
        },
    }
}

impl EcosystemAdapter for GoAdapter {
    fn ecosystem(&self) -> Ecosystem {
        Ecosystem::Go
    }

    fn is_manifest(&self, file_name: &str) -> bool {
        file_name == MANIFEST
    }

    /// go.mod is its own lock; `go.sum` is ignored (E2).
    fn is_lockfile(&self, _file_name: &str) -> bool {
        false
    }

    fn lockfile_for(&self, _manifest: &str, _files: &FileSet) -> Option<String> {
        None
    }

    fn declared_names(&self, manifest: &str) -> Vec<String> {
        parse_mod(manifest)
            .ok()
            .and_then(|m| m.module)
            .into_iter()
            .collect()
    }

    fn parse(&self, input: &ParseInput<'_>) -> Result<ParsedGroup, ParseError> {
        let m = parse_mod(input.manifest).map_err(|Bad(line)| {
            ParseError::at(
                input.manifest_path,
                ParseErrorKind::InvalidSyntax,
                Some(line),
                None,
            )
        })?;
        let mut direct = Vec::new();
        let mut lock_entries = Vec::new();
        for r in &m.requires {
            let (name, version, source) = apply_replace(&m, r);
            if !r.indirect {
                direct.push(Dependency {
                    name: name.clone(),
                    scope: DepScope::Normal,
                    declared: Some(r.version.clone()),
                    resolved: version.clone(),
                    source: source.clone(),
                });
            }
            if let Some(version) = version {
                lock_entries.push(LockEntry {
                    name,
                    version,
                    source,
                });
            }
        }
        Ok(ParsedGroup {
            direct,
            lock_entries,
            // Before Go 1.17 the indirect part of the graph is incomplete.
            has_lockfile: m.go_minor.is_some_and(|n| n >= FULL_GRAPH_MINOR),
        })
    }
}

#[cfg(test)]
#[path = "go_mod_tests.rs"]
mod tests;
