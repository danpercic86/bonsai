//! NuGet adapter (E2 NuGet row): `*.csproj` / `*.fsproj` / `*.vbproj`
//! `PackageReference`, `Directory.Packages.props` `PackageVersion` (CPM),
//! `Directory.Build.props`, and `packages.config`; `packages.lock.json`
//! (`packages.<project>.lock.json` first) next to a project file.
//!
//! File names compare ASCII-case-insensitively (NuGet / MSBuild on Windows).
//! Every registry dependency is `Registry(Default)`; which feeds actually
//! serve it is the registry context's job (the NuGet.Config chain,
//! `<clear/>`, disabled sources, `packageSourceMapping`, RestoreSources).
//!
//! Declared names `[HIGH-1 item 3]`: `<PackageId>`, `<AssemblyName>` and the
//! project file stem (NuGet's default package id) — the stem needs the
//! path, hence [`EcosystemAdapter::declared_names_at`].

use crate::adapter::{
    file_name, join, parent_dir, DepSource, Dependency, EcosystemAdapter, ParseError, ParseInput,
    ParsedGroup, RegistryRef,
};
use crate::discover::FileSet;
use crate::model::{DepScope, Ecosystem};
use crate::xml_events::XmlError;

use super::nuget_lock::{self, NugetLock};
use super::nuget_proj::{self, Item, ItemKind};

pub struct NugetAdapter;

const PROJECT_EXTS: &[&str] = &[".csproj", ".fsproj", ".vbproj"];
pub(crate) const PACKAGES_PROPS: &str = "Directory.Packages.props";
pub(crate) const BUILD_PROPS: &str = "Directory.Build.props";
pub(crate) const PACKAGES_CONFIG: &str = "packages.config";
const LOCKFILE: &str = "packages.lock.json";

fn ends_with_ci(s: &str, suffix: &str) -> bool {
    s.len() > suffix.len()
        && s.is_char_boundary(s.len() - suffix.len())
        && s[s.len() - suffix.len()..].eq_ignore_ascii_case(suffix)
}

/// A `*.csproj` / `*.fsproj` / `*.vbproj` file name.
pub(crate) fn is_project_file(name: &str) -> bool {
    PROJECT_EXTS.iter().any(|e| ends_with_ci(name, e))
}

/// Any NuGet manifest file name.
pub(crate) fn is_nuget_manifest(name: &str) -> bool {
    is_project_file(name)
        || [PACKAGES_PROPS, BUILD_PROPS, PACKAGES_CONFIG]
            .iter()
            .any(|n| n.eq_ignore_ascii_case(name))
}

/// `packages.lock.json` or `packages.<project>.lock.json`.
fn is_lock_name(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower == LOCKFILE || (lower.starts_with("packages.") && lower.ends_with(".lock.json"))
}

fn xml_error(path: &str, e: XmlError) -> ParseError {
    ParseError::at(path, e.kind, e.line, e.col)
}

/// A plain `1.2.3[-pre]` (no range / float / property) is what NuGet
/// resolves to (lowest applicable, E2); anything else is declared only.
fn plain_version(v: &str) -> Option<String> {
    let v = v.trim();
    let ok = v.chars().next().is_some_and(|c| c.is_ascii_digit())
        && v.chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '+'));
    ok.then(|| v.to_string())
}

fn dependency(item: Item, lock: Option<&NugetLock>) -> Dependency {
    // An unresolved MSBuild property is not a version.
    let declared = item.version.filter(|v| !v.contains("$("));
    let locked = lock.and_then(|l| l.resolved(&item.id));
    let resolved = locked
        .map(|e| e.version.clone())
        .filter(|v| !v.is_empty())
        .or_else(|| declared.as_deref().and_then(plain_version));
    let source = match locked {
        Some(e) if e.source == DepSource::Workspace => DepSource::Workspace,
        _ => DepSource::Registry(RegistryRef::Default),
    };
    let scope = if item.dev {
        DepScope::Dev
    } else {
        DepScope::Normal
    };
    // `packages.config` versions are exact.
    let resolved = match item.kind {
        ItemKind::PackagesConfig => resolved.or_else(|| declared.clone()),
        _ => resolved,
    };
    // [P125d review, MEDIUM-D1] OSV matches NuGet ids case-sensitively,
    // and `packages.lock.json` keys carry the canonical nuspec casing even
    // when the project writes the id differently (VERIFIED, §0.1). A locked
    // dependency therefore takes the lockfile's casing.
    // TFMs disagreeing on the casing ⇒ keep the project's (and the row
    // reads "not verified" on zero hits, `check/rows.rs`).
    let name = lock
        .and_then(|l| l.canonical(&item.id))
        .map_or(item.id.clone(), str::to_string);
    Dependency {
        name,
        scope,
        declared,
        resolved,
        source,
    }
}

impl EcosystemAdapter for NugetAdapter {
    fn ecosystem(&self) -> Ecosystem {
        Ecosystem::Nuget
    }

    fn is_manifest(&self, file_name: &str) -> bool {
        is_nuget_manifest(file_name)
    }

    fn is_lockfile(&self, file_name: &str) -> bool {
        is_lock_name(file_name)
    }

    /// Project files only, same directory: `packages.<stem>.lock.json`,
    /// then `packages.lock.json`.
    fn lockfile_for(&self, manifest: &str, files: &FileSet) -> Option<String> {
        let name = file_name(manifest);
        if !is_project_file(name) {
            return None;
        }
        let dir = parent_dir(manifest);
        let stem = name.rsplit_once('.').map_or(name, |(s, _)| s);
        [format!("packages.{stem}.lock.json"), LOCKFILE.to_string()]
            .into_iter()
            .map(|n| join(dir, &n))
            .find(|p| files.contains(p))
    }

    fn declared_names(&self, manifest: &str) -> Vec<String> {
        nuget_proj::read(manifest)
            .map(|d| d.names)
            .unwrap_or_default()
    }

    fn declared_names_at(&self, path: &str, manifest: &str) -> Vec<String> {
        let mut out = self.declared_names(manifest);
        let name = file_name(path);
        if is_project_file(name) {
            if let Some((stem, _)) = name.rsplit_once('.') {
                out.push(stem.to_string());
            }
        }
        out
    }

    fn parse(&self, input: &ParseInput<'_>) -> Result<ParsedGroup, ParseError> {
        let path = input.manifest_path;
        let data = nuget_proj::read(input.manifest).map_err(|e| xml_error(path, e))?;
        let lock = match input.lockfile {
            Some((lp, text)) => Some(nuget_lock::parse(lp, text)?),
            None => None,
        };
        let direct = data
            .items
            .into_iter()
            .map(|i| dependency(i, lock.as_ref()))
            .collect();
        Ok(ParsedGroup {
            direct,
            lock_entries: lock.map(NugetLock::into_entries).unwrap_or_default(),
            has_lockfile: input.lockfile.is_some(),
        })
    }
}

#[cfg(test)]
#[path = "nuget_tests.rs"]
mod tests;
