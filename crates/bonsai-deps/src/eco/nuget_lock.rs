//! `packages.lock.json` (E2 NuGet row): `dependencies.<tfm>.<id>` with
//! `type` (Direct | Transitive | CentralTransitive | Project) and
//! `resolved`. serde_json keeps its 128-depth limit (§4.2); errors map by
//! category + position only `[MEDIUM-1]`.

use std::collections::{BTreeMap, BTreeSet};

use serde::Deserialize;

use crate::adapter::{DepSource, LockEntry, ParseError, RegistryRef};
use crate::model::ParseErrorKind;

/// Highest lock `version` understood (NuGet writes 1, or 2 with CPM).
const MAX_LOCK_VERSION: u32 = 2;

#[derive(Deserialize)]
struct LockDto {
    #[serde(default)]
    version: Option<u32>,
    #[serde(default)]
    dependencies: BTreeMap<String, BTreeMap<String, LockDep>>,
}

#[derive(Deserialize)]
struct LockDep {
    #[serde(default, rename = "type")]
    kind: Option<String>,
    #[serde(default)]
    resolved: Option<String>,
}

pub(crate) struct NugetLock {
    entries: Vec<LockEntry>,
    /// Lowercase id ⇒ every exact casing the lockfile writes for it.
    casings: BTreeMap<String, BTreeSet<String>>,
}

impl NugetLock {
    /// The resolved version of `id` (case-insensitive; the first TFM wins).
    pub(crate) fn resolved(&self, id: &str) -> Option<&LockEntry> {
        self.entries
            .iter()
            .find(|e| e.name.eq_ignore_ascii_case(id))
    }

    /// `[P125d review, MEDIUM-D1]` the lockfile's casing of `id` (the
    /// canonical nuspec id, VERIFIED §0.1) — only when every TFM agrees.
    pub(crate) fn canonical(&self, id: &str) -> Option<&str> {
        let set = self.casings.get(&id.to_ascii_lowercase())?;
        match (set.len(), set.iter().next()) {
            (1, Some(c)) => Some(c.as_str()),
            _ => None,
        }
    }

    pub(crate) fn into_entries(self) -> Vec<LockEntry> {
        self.entries
    }
}

pub(crate) fn parse(path: &str, text: &str) -> Result<NugetLock, ParseError> {
    let text = text.strip_prefix('\u{FEFF}').unwrap_or(text);
    let dto: LockDto = serde_json::from_str(text).map_err(|e| ParseError::from_json(path, &e))?;
    if dto.version.is_some_and(|v| v > MAX_LOCK_VERSION) {
        return Err(ParseError::new(path, ParseErrorKind::UnsupportedVersion));
    }
    let mut seen = BTreeSet::new();
    let mut entries = Vec::new();
    let mut casings: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for (id, dep) in dto.dependencies.into_values().flatten() {
        casings
            .entry(id.to_ascii_lowercase())
            .or_default()
            .insert(id.clone());
        let project = dep
            .kind
            .as_deref()
            .is_some_and(|k| k.eq_ignore_ascii_case("Project"));
        // A project reference may carry no `resolved`.
        let Some(version) = dep.resolved.or_else(|| project.then(String::new)) else {
            continue;
        };
        if !seen.insert((id.to_ascii_lowercase(), version.clone())) {
            continue;
        }
        let source = if project {
            DepSource::Workspace
        } else {
            DepSource::Registry(RegistryRef::Default)
        };
        entries.push(LockEntry {
            name: id,
            version,
            source,
        });
    }
    Ok(NugetLock { entries, casings })
}
