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

use crate::model::ParseErrorKind;
use crate::xml_events::{walk, Elem, XmlError, XmlSink};

use super::{extract_host, Closed};

/// What one `NuGet.Config` file says. Keys as written; hosts only.
#[derive(Default)]
pub(crate) struct NugetFile {
    /// `packageSources/add` (key, host). `[P125c2 review]` host `None` = a
    /// feed with no URL host (a local folder, a UNC share, `%VAR%`): kept
    /// as a NON-PUBLIC source, so mapping can still scope it.
    pub(crate) sources: Vec<(String, Option<String>)>,
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
                    // empty value is malformed ⇒ closed.
                    if value.trim().is_empty() {
                        return Err(closed());
                    }
                    self.out.sources.push((key, extract_host(&value)));
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
    hosts: Vec<String>,
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
            // Any other property / item / metadata expansion, a folder or a
            // share has no URL host ⇒ fail closed.
            let host = extract_host(v).ok_or_else(closed)?;
            self.hosts.push(host);
        }
        Ok(())
    }
}

/// The RestoreSources hosts of one project file / `Directory.Build.props`.
pub(crate) fn restore_sources(text: &str) -> Result<Vec<String>, Closed> {
    let mut sink = RestoreSink::default();
    walk(text, &mut sink).map_err(|_| Closed)?;
    Ok(sink.hosts)
}
