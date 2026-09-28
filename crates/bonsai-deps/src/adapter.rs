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
    fn parse(&self, input: &ParseInput<'_>) -> Result<ParsedGroup, ParseError>;
}

pub static ADAPTERS: &[&dyn EcosystemAdapter] = &[&crate::eco::npm::NpmAdapter];

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
            for n in adapter.declared_names(&text) {
                names.insert(adapter.ecosystem(), &n);
            }
        }
    }
    names
}
