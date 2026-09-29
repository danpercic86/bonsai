//! `[MEDIUM-1]` key-filtered extractors for NuGet registry signals (§4.4,
//! E3 NuGet row), through the bounded XML walker (`xml_events.rs`):
//! - `NuGet.Config`: `packageSources/add@key,value` + `<clear/>`,
//!   `disabledPackageSources/add@key,value` + `<clear/>`,
//!   `packageSourceMapping/packageSource@key/package@pattern` + `<clear/>`,
//!   and the presence of `fallbackPackageFolders/add` + `<clear/>`.
//!   No other section is looked at: `packageSourceCredentials`, `apikeys`
//!   and `config` attributes are walked (and counted) but never decoded.
//! - project files / `Directory.Build.props`: the text of
//!   `RestoreSources` / `RestoreAdditionalProjectSources`.
//!
//! Behaviour checked locally with the .NET 10 SDK (`dotnet nuget list
//! source` / `config paths` on scratch configs): section names are
//! case-SENSITIVE (a `<PackageSources>` section is ignored), `add` is
//! accepted in any case, `<clear/>` drops every farther level, a disabled
//! key matches case-sensitively with a case-insensitive `true`, `<remove>`
//! is ignored, and a second `packageSources` section in one file changes
//! the result unpredictably. So: a section or `clear` element whose name
//! differs only in case, or a repeated section, is AMBIGUOUS ⇒ `Closed`.

use crate::catalog::is_public_host;
use crate::model::{Ecosystem, ParseErrorKind};
use crate::xml_events::{walk, Elem, XmlError, XmlSink};

use super::{extract_host, Closed};

/// `[P125h §3]` what one package-source value points at, judged on its TEXT
/// alone (the value is never opened: a UNC share causes no SMB traffic).
/// `[MEDIUM-1]` holds no path text — only a URL host is kept.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum FeedRef {
    /// An http(s) feed: its lowercase host.
    Host(String),
    /// A folder / file / UNC package source.
    LocalPath,
    /// A `%VAR%` NuGet does expand; we do not (it may be a URL). Also any
    /// `%` (percent-encoding) and userinfo in front of a PUBLIC host
    /// (`[P125h2 LOW-H1]`): private, with no host shown.
    Unresolved,
}

/// `scheme:` per RFC 3986 (`ALPHA *( ALPHA / DIGIT / "+" / "-" / "." ) ":"`).
fn has_scheme(v: &str) -> bool {
    let Some((scheme, _)) = v.split_once(':') else {
        return false;
    };
    let mut chars = scheme.chars();
    chars.next().is_some_and(|c| c.is_ascii_alphabetic())
        && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
}

/// `C:\…` / `C:/…`.
fn is_drive_path(v: &str) -> bool {
    let b = v.as_bytes();
    b.len() >= 3 && b[0].is_ascii_alphabetic() && b[1] == b':' && matches!(b[2], b'\\' | b'/')
}

/// `[P125h2 LOW-H1]` an http(s) value whose authority carries userinfo
/// (`user@host`): the text before the `@` may be an expansion or an
/// obfuscation, so a PUBLIC host after it is never trusted (a private one
/// stays private, and is shown bare — the userinfo is never kept).
fn has_userinfo(v: &str) -> bool {
    let lower = v.to_ascii_lowercase();
    let Some(rest) = lower
        .strip_prefix("https://")
        .or_else(|| lower.strip_prefix("http://"))
    else {
        return false;
    };
    let end = rest.find(['/', '?', '#', '\\']).unwrap_or(rest.len());
    rest[..end].contains('@')
}

/// `[P125h §3]` classify one package-source value (trimmed by the caller).
/// `None` ⇒ the caller fails closed. The arm order is load-bearing:
/// `[P125h2 LOW-H1]` every expansion / userinfo check runs BEFORE the host is
/// trusted, so `https://%FEED%@api.nuget.org/` or `https://$(F)@api.nuget.org/`
/// can never pass as the public host it merely ends in.
pub(crate) fn feed_ref(v: &str) -> Option<FeedRef> {
    // An MSBuild expansion could expand to a URL we cannot see ⇒ closed.
    if v.contains("$(") || v.contains("@(") || v.contains("%(") {
        return None;
    }
    // A `%VAR%` NuGet expands (percent-encoding is indistinguishable from
    // it on text alone, so it is treated the same: private, no host).
    if v.contains('%') {
        return Some(FeedRef::Unresolved);
    }
    let host = extract_host(v);
    if has_userinfo(v) {
        return Some(match host {
            Some(h) if !is_public_host(Ecosystem::Nuget, &h) => FeedRef::Host(h),
            _ => FeedRef::Unresolved,
        });
    }
    if let Some(h) = host {
        return Some(FeedRef::Host(h));
    }
    let lower = v.to_ascii_lowercase();
    let local_prefix = lower.starts_with("file:")
        || ["\\", "//", "/", "./", "../", ".\\", "..\\"]
            .iter()
            .any(|p| v.starts_with(p));
    if local_prefix || is_drive_path(v) || !has_scheme(v) {
        return Some(FeedRef::LocalPath);
    }
    // ftp:, any other scheme, `C:relative`, `https://` with no host.
    None
}

/// What one `NuGet.Config` file says. Keys as written; hosts only.
#[derive(Default)]
pub(crate) struct NugetFile {
    /// `packageSources/add` (key, feed). `[P125c2 review]` a feed with no
    /// URL host is kept as a NON-PUBLIC source, so mapping can still scope
    /// it; `[P125h §3]` a value `feed_ref` rejects ⇒ `Closed`.
    pub(crate) sources: Vec<(String, FeedRef)>,
    pub(crate) sources_clear: bool,
    /// `disabledPackageSources/add` (key, value is `true`).
    pub(crate) disabled: Vec<(String, bool)>,
    pub(crate) disabled_clear: bool,
    /// `packageSourceMapping` (source key, lowercase id pattern).
    pub(crate) mapping: Vec<(String, String)>,
    pub(crate) mapping_clear: bool,
    /// `[P125c2 review]` `fallbackPackageFolders/add` count: local folders
    /// consulted for EVERY package (not subject to source mapping).
    pub(crate) fallback: usize,
    pub(crate) fallback_clear: bool,
}

const ROOT: &str = "configuration";
const SOURCES: &str = "packageSources";
const DISABLED: &str = "disabledPackageSources";
const MAPPING: &str = "packageSourceMapping";
const FALLBACK: &str = "fallbackPackageFolders";
const SECTIONS: [&str; 4] = [SOURCES, DISABLED, MAPPING, FALLBACK];
const MAP_SOURCE: &str = "packageSource";
const MAP_PACKAGE: &str = "package";

/// A closed extraction, carried through the XML walker.
fn closed() -> XmlError {
    XmlError::new(ParseErrorKind::InvalidSyntax)
}

/// `name == exact`; a case-only mismatch is ambiguous ⇒ error.
fn exact(name: &str, want: &str) -> Result<bool, XmlError> {
    if name == want {
        Ok(true)
    } else if name.eq_ignore_ascii_case(want) {
        Err(closed())
    } else {
        Ok(false)
    }
}

fn required(e: &Elem<'_, '_>, attr: &str) -> Result<String, XmlError> {
    e.attr(attr)?.ok_or_else(closed)
}

#[derive(Default)]
struct ConfigSink {
    out: NugetFile,
    seen: [bool; 4],
    /// The `packageSource@key` being read inside `packageSourceMapping`.
    map_key: Option<String>,
}

impl ConfigSink {
    fn section(&mut self, name: &str) -> Result<(), XmlError> {
        for (i, s) in SECTIONS.iter().enumerate() {
            if exact(name, s)? {
                if self.seen[i] {
                    return Err(closed());
                }
                self.seen[i] = true;
            }
        }
        Ok(())
    }

    fn item(&mut self, section: &str, e: &Elem<'_, '_>) -> Result<(), XmlError> {
        let name = e.name();
        if exact(name, "clear")? {
            match section {
                SOURCES => self.out.sources_clear = true,
                DISABLED => self.out.disabled_clear = true,
                FALLBACK => self.out.fallback_clear = true,
                _ => self.out.mapping_clear = true,
            }
            return Ok(());
        }
        match section {
            SOURCES | DISABLED if name.eq_ignore_ascii_case("add") => {
                let key = required(e, "key")?;
                let value = required(e, "value")?;
                if section == SOURCES {
                    // [P125c2 review] a feed with no URL host (a folder, a
                    // UNC share, `%VAR%`) is a hostless PRIVATE source; an
                    // empty value (checked BEFORE `feed_ref`, which would
                    // read it as a bare relative dir) or one `feed_ref`
                    // rejects is malformed ⇒ closed.
                    let value = value.trim();
                    if value.is_empty() {
                        return Err(closed());
                    }
                    let feed = feed_ref(value).ok_or_else(closed)?;
                    self.out.sources.push((key, feed));
                } else {
                    let on = value.trim().eq_ignore_ascii_case("true");
                    self.out.disabled.push((key, on));
                }
            }
            FALLBACK if name.eq_ignore_ascii_case("add") => {
                required(e, "value")?;
                self.out.fallback += 1;
            }
            MAPPING if exact(name, MAP_SOURCE)? => {
                self.map_key = Some(required(e, "key")?);
            }
            // NuGet ignores other children (`<remove>` verified).
            _ => {}
        }
        Ok(())
    }
}

impl ConfigSink {
    /// `<package pattern>` inside `packageSourceMapping/packageSource`.
    fn mapped_package(&mut self, e: &Elem<'_, '_>) -> Result<(), XmlError> {
        if exact(e.name(), MAP_PACKAGE)? {
            let pattern = required(e, "pattern")?.trim().to_ascii_lowercase();
            let key = self.map_key.clone().ok_or_else(closed)?;
            self.out.mapping.push((key, pattern));
        }
        Ok(())
    }
}

impl XmlSink for ConfigSink {
    fn start(&mut self, stack: &[String], e: &Elem<'_, '_>) -> Result<(), XmlError> {
        match stack {
            [] => {
                if !exact(e.name(), ROOT)? {
                    return Err(closed());
                }
            }
            [_] => self.section(e.name())?,
            [_, section] if SECTIONS.contains(&section.as_str()) => {
                self.item(section, e)?;
            }
            [_, m, s] if m == MAPPING && s == MAP_SOURCE => self.mapped_package(e)?,
            _ => {}
        }
        Ok(())
    }

    fn end(&mut self, stack: &[String]) -> Result<(), XmlError> {
        if stack.len() == 3 && stack[1] == MAPPING {
            self.map_key = None;
        }
        Ok(())
    }
}

/// One `NuGet.Config`; any XML error, bomb or ambiguity ⇒ `Closed`.
pub(crate) fn nuget_config(text: &str) -> Result<NugetFile, Closed> {
    let mut sink = ConfigSink::default();
    walk(text, &mut sink).map_err(|_| Closed)?;
    Ok(sink.out)
}

fn is_restore_prop(name: &str) -> bool {
    name.eq_ignore_ascii_case("RestoreSources")
        || name.eq_ignore_ascii_case("RestoreAdditionalProjectSources")
}

/// Properties that point restore at a config we do not read ⇒ `Closed`.
fn is_redirect_prop(name: &str) -> bool {
    name.eq_ignore_ascii_case("RestoreConfigFile")
        || name.eq_ignore_ascii_case("RestoreRootConfigDirectory")
}

#[derive(Default)]
struct RestoreSink {
    feeds: Vec<FeedRef>,
}

impl XmlSink for RestoreSink {
    fn start(&mut self, _stack: &[String], e: &Elem<'_, '_>) -> Result<(), XmlError> {
        if is_redirect_prop(e.name()) {
            return Err(closed());
        }
        Ok(())
    }

    fn wants_text(&self, stack: &[String]) -> bool {
        stack.last().is_some_and(|n| is_restore_prop(n))
    }

    fn text(&mut self, _stack: &[String], text: &str) -> Result<(), XmlError> {
        for v in text.split(';').map(str::trim).filter(|v| !v.is_empty()) {
            // Self-references keep what the config chain already gives.
            if v.eq_ignore_ascii_case("$(RestoreSources)")
                || v.eq_ignore_ascii_case("$(RestoreAdditionalProjectSources)")
            {
                continue;
            }
            // `[P125h §3]` a folder / file / UNC feed is a local feed; any
            // other property / item / metadata expansion or an unknown
            // scheme ⇒ fail closed.
            self.feeds.push(feed_ref(v).ok_or_else(closed)?);
        }
        Ok(())
    }
}

/// The RestoreSources feeds of one project file / `Directory.Build.props`.
pub(crate) fn restore_sources(text: &str) -> Result<Vec<FeedRef>, Closed> {
    let mut sink = RestoreSink::default();
    walk(text, &mut sink).map_err(|_| Closed)?;
    Ok(sink.feeds)
}
