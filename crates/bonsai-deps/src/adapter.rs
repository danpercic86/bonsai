//! Adapter surface (contract §4.2): the trait every ecosystem implements,
//! the parse input/output types and the `ADAPTERS` registry.
//!
//! `[MEDIUM-1]` [`ParseError`] has no free-text field. Adapters map parser
//! errors to `(kind, line, col)` and never call `to_string()` / `Display` on a
//! toml / quick-xml / yaml / serde_json error.

use crate::discover::FileSet;
use crate::model::{DepScope, Ecosystem, ParseErrorKind};

/// Where a dependency (or lock entry) resolves from, as the adapter read it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DepSource {
    Registry(RegistryRef),
    Path,
    Git,
    Workspace,
    Url,
}

/// Which registry a `DepSource::Registry` names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegistryRef {
    /// The ecosystem's default registry (subject to config overrides).
    Default,
    /// A named registry, e.g. Cargo `registry = "corp"`, or a pnpm
    /// `namedRegistries` alias protocol (`work:@corp/lib@^2`).
    Named(String),
    /// A lockfile host. `extract_host` output only (lowercase host, no
    /// userinfo/port/path).
    Host(String),
    /// An npm scope (`@corp`) the adapter saw routed to a scope registry.
    Scope(String),
}

pub struct ParseInput<'a> {
    pub manifest_path: &'a str,
    pub manifest: &'a str,
    /// `(path, content)` of the lockfile `lockfile_for` chose.
    pub lockfile: Option<(&'a str, &'a str)>,
    pub files: &'a FileSet,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dependency {
    pub name: String,
    pub scope: DepScope,
    pub declared: Option<String>,
    pub resolved: Option<String>,
    pub source: DepSource,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LockEntry {
    pub name: String,
    pub version: String,
    pub source: DepSource,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ParsedGroup {
    pub direct: Vec<Dependency>,
    pub lock_entries: Vec<LockEntry>,
    pub has_lockfile: bool,
}

/// `[MEDIUM-1]` no free-text field: a file path plus `(kind, line, col)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    pub file: String,
    pub kind: ParseErrorKind,
    pub line: Option<u32>,
    pub col: Option<u32>,
}

impl ParseError {
    pub fn new(file: &str, kind: ParseErrorKind) -> Self {
        ParseError {
            file: file.to_string(),
            kind,
            line: None,
            col: None,
        }
    }

    pub(crate) fn at(
        file: &str,
        kind: ParseErrorKind,
        line: Option<u32>,
        col: Option<u32>,
    ) -> Self {
        ParseError {
            file: file.to_string(),
            kind,
            line,
            col,
        }
    }

    /// Maps a serde_json error by category and position only — its message
    /// (which can quote input) is never read.
    pub(crate) fn from_json(file: &str, e: &serde_json::Error) -> Self {
        let kind = match e.classify() {
            serde_json::error::Category::Io => ParseErrorKind::Internal,
            _ => ParseErrorKind::InvalidSyntax,
        };
        Self::at(
            file,
            kind,
            u32::try_from(e.line()).ok(),
            u32::try_from(e.column()).ok(),
        )
    }

    pub(crate) fn from_yaml(file: &str, e: crate::yaml_events::YamlError) -> Self {
        Self::at(file, e.kind, e.line, e.col)
    }

    /// Maps a toml error by its span only — its `Display` renders the
    /// offending source line (which can hold a token) and is never called.
    pub(crate) fn from_toml(file: &str, text: &str, e: &toml::de::Error) -> Self {
        let (line, col) = e
            .span()
            .map_or((None, None), |span| line_col(text, span.start));
        Self::at(file, ParseErrorKind::InvalidSyntax, line, col)
    }

    /// `[HIGH-2]` the §4.2 TOML depth pre-scan rejected the file.
    pub(crate) fn too_deep(file: &str, e: crate::toml_scan::TooDeep) -> Self {
        Self::at(file, ParseErrorKind::LimitExceeded, Some(e.line), None)
    }
}

/// 1-based `(line, byte column)` of byte offset `at` in `text`.
pub(crate) fn line_col(text: &str, at: usize) -> (Option<u32>, Option<u32>) {
    let head = text.as_bytes().get(..at).unwrap_or(text.as_bytes());
    let line = head.iter().filter(|b| **b == b'\n').count() + 1;
    let col = head.iter().rev().take_while(|b| **b != b'\n').count() + 1;
    (u32::try_from(line).ok(), u32::try_from(col).ok())
}

pub trait EcosystemAdapter: Sync {
    fn ecosystem(&self) -> Ecosystem;
    fn is_manifest(&self, file_name: &str) -> bool;
    fn is_lockfile(&self, file_name: &str) -> bool;
    /// Same dir, then ancestors ≤ workdir.
    fn lockfile_for(&self, manifest: &str, files: &FileSet) -> Option<String>;
    /// Names this manifest DECLARES for itself `[HIGH-1 item 3]`, e.g.
    /// package.json `name`. `manifest` is the file CONTENT.
    fn declared_names(&self, manifest: &str) -> Vec<String>;
    /// `[P125c2]` [`Self::declared_names`] for an ecosystem whose declared
    /// name can come from the manifest PATH (NuGet: the project file stem
    /// is the default package id). `local_names` calls this; the default
    /// ignores the path.
    fn declared_names_at(&self, _path: &str, manifest: &str) -> Vec<String> {
        self.declared_names(manifest)
    }
    fn parse(&self, input: &ParseInput<'_>) -> Result<ParsedGroup, ParseError>;
}

pub static ADAPTERS: &[&dyn EcosystemAdapter] = &[
    &crate::eco::npm::NpmAdapter,
    &crate::eco::cargo::CargoAdapter,
    &crate::eco::go_mod::GoAdapter,
    &crate::eco::pypi::PypiAdapter,
    &crate::eco::nuget::NugetAdapter,
];

/// The adapter whose manifest this file name is.
pub fn adapter_for_manifest(file_name: &str) -> Option<&'static dyn EcosystemAdapter> {
    ADAPTERS.iter().copied().find(|a| a.is_manifest(file_name))
}

/// The last `/`-separated segment of a repo-relative path.
pub(crate) fn file_name(rel: &str) -> &str {
    rel.rsplit('/').next().unwrap_or(rel)
}

/// The parent directory of a repo-relative path (`""` for the workdir).
pub(crate) fn parent_dir(rel: &str) -> &str {
    rel.rsplit_once('/').map_or("", |(d, _)| d)
}

/// `dir` followed by its ancestors up to and including the workdir (`""`).
pub(crate) fn dir_and_ancestors(dir: &str) -> Vec<&str> {
    let mut out = vec![dir];
    let mut d = dir;
    while !d.is_empty() {
        d = parent_dir(d);
        out.push(d);
    }
    out
}

pub(crate) fn join(dir: &str, name: &str) -> String {
    if dir.is_empty() {
        name.to_string()
    } else {
        format!("{dir}/{name}")
    }
}

/// `[HIGH-1 item 3]` the union of every discovered manifest's declared
/// names (§4.4 `local_names`). Unreadable / unparseable manifests declare
/// nothing (their own group reports the error).
pub fn local_names(workdir_canon: &std::path::Path, files: &FileSet) -> crate::privacy::LocalNames {
    use crate::fsread::{safe_read, ReadOutcome, MAX_FILE_BYTES};
    let mut names = crate::privacy::LocalNames::new();
    for m in files.manifests() {
        let Some(adapter) = adapter_for_manifest(file_name(m)) else {
            continue;
        };
        if let ReadOutcome::Content(text) = safe_read(workdir_canon, m, MAX_FILE_BYTES) {
            for n in adapter.declared_names_at(m, &text) {
                names.insert(adapter.ecosystem(), &n);
            }
        }
    }
    names
}
