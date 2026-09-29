//! IPC + cache DTOs (contract §2.1), `summarize`, and `clean_display` (§7.5).
//!
//! Every `String` field that comes from the repo or a response is passed through
//! [`clean_display`] by the code that builds these values `[LOW-3]`. Private and
//! local names DO appear here (and in the cache) by design: D7's boundary is the
//! socket, not the local view `[MEDIUM-2]`.

use serde::{Deserialize, Serialize};

mod display;
mod summary;

pub use display::{clean_display, MAX_DISPLAY, MAX_ID_DISPLAY, MAX_SUMMARY_DISPLAY};
pub use summary::summarize;

/// `CachedReport::schema` value written by this build (§6.5).
pub const CACHE_SCHEMA: u32 = 1;
/// Caps `[LOW-3]` (§7.5).
pub const MAX_MANIFESTS: usize = 2000;
pub const MAX_PARSE_BYTES: u64 = 256 * 1024 * 1024;
pub const MAX_ADVISORIES: usize = 5000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Ecosystem {
    Npm,
    Cargo,
    Pypi,
    Go,
    Nuget,
}

impl Ecosystem {
    pub const ALL: [Ecosystem; 5] = [
        Ecosystem::Npm,
        Ecosystem::Cargo,
        Ecosystem::Pypi,
        Ecosystem::Go,
        Ecosystem::Nuget,
    ];
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum UpdateKind {
    Patch,
    Minor,
    Major,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Critical,
    High,
    Medium,
    Low,
    Unknown,
}

impl Severity {
    /// Rank for "max severity" comparisons: Critical is highest, Unknown lowest.
    pub fn rank(self) -> u8 {
        match self {
            Severity::Critical => 4,
            Severity::High => 3,
            Severity::Medium => 2,
            Severity::Low => 1,
            Severity::Unknown => 0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Relation {
    Direct,
    Transitive,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DepScope {
    Normal,
    Dev,
    Build,
    Optional,
    Peer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum LocalReason {
    Path,
    Git,
    Workspace,
    Url,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PrivateReason {
    CustomRegistry,
    ScopedRegistry,
    GoPrivate,
    SourceMapping,
    /// `[HIGH-1]` fail-closed: a registry signal could not be read or resolved.
    ConfigUnreadable,
    /// P125h §3: a folder / file / UNC package source; `registry_host` is
    /// always `None`.
    LocalFeed,
}

/// Why a public row is `Unknown` (P125h §0). v1 JSON `{"kind":"unknown"}`
/// has no reason and reads as [`UnknownReason::NotFound`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum UnknownReason {
    /// deps.dev 404, or a `versionKey.name` that is not the package asked for.
    #[default]
    NotFound,
    /// The deps.dev body exceeded `DEPSDEV_BODY_CAP`, or listed more than
    /// `MAX_VERSIONS` versions (P125h §2).
    TooLarge,
}

/// Why a `Checked` / `Unknown` row's `vulns_checked` is false (P125h §0, §4c).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum VulnsUnverifiedReason {
    /// No resolved version, so OSV was not asked.
    NoVersion,
    /// `[MEDIUM-D1]` a NuGet id in an unconfirmed casing with zero OSV hits.
    NameCasing,
    /// ≥ 1 OSV hit for this package failed the id shape check and was
    /// dropped (P125h §1).
    MalformedAdvisory,
    /// ≥ 1 of its advisory records exceeded `OSV_VULN_BODY_CAP` (P125h §2).
    ResponseTooLarge,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Availability {
    /// deps.dev knew it.
    Checked,
    /// deps.dev did not answer usably (see [`UnknownReason`]). OSV is still
    /// queried when a resolved version exists, so an Unknown row CAN carry
    /// vulns `[UI-e]`.
    Unknown {
        #[serde(default)]
        reason: UnknownReason,
    },
    Local {
        reason: LocalReason,
    },
    Private {
        reason: PrivateReason,
        /// `[UI-d]` host only (from `extract_host`). Display only; never sent.
        #[serde(rename = "registryHost")]
        registry_host: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Advisory {
    pub id: String,
    pub aliases: Vec<String>,
    /// Plain text; never rendered as markdown/HTML `[LOW-3]`.
    pub summary: Option<String>,
    pub severity: Severity,
    pub cvss_score: Option<f32>,
    pub modified: String,
    pub url: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DepRow {
    pub name: String,
    pub ecosystem: Ecosystem,
    pub relation: Relation,
    pub scope: DepScope,
    pub declared: Option<String>,
    pub current: Option<String>,
    pub latest: Option<String>,
    pub update: Option<UpdateKind>,
    pub availability: Availability,
    /// false ⇒ the UI must not claim "no vulnerabilities".
    pub vulns_checked: bool,
    pub vulns: Vec<Advisory>,
    pub registry_url: Option<String>,
    /// Why `vulns_checked` is false; `None` on v1 cache rows (P125h §0).
    #[serde(default)]
    pub vulns_unverified: Option<VulnsUnverifiedReason>,
}

impl DepRow {
    /// The P125h §0 row invariant: a `Checked` / `Unknown` row is verified
    /// iff it has no unverified reason; a `Local` / `Private` row is never
    /// verified and carries no reason (its availability explains it).
    pub fn verdict_consistent(&self) -> bool {
        match self.availability {
            Availability::Checked | Availability::Unknown { .. } => {
                self.vulns_checked == self.vulns_unverified.is_none()
            }
            Availability::Local { .. } | Availability::Private { .. } => {
                !self.vulns_checked && self.vulns_unverified.is_none()
            }
        }
    }

    /// The highest severity among this row's advisories, if any.
    pub fn max_severity(&self) -> Option<Severity> {
        self.vulns
            .iter()
            .map(|a| a.severity)
            .max_by_key(|s| s.rank())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ParseErrorKind {
    TooLarge,
    NotAFile,
    Unreadable,
    InvalidSyntax,
    UnsupportedFeature,
    UnsupportedVersion,
    LimitExceeded,
    Internal,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum GroupState {
    Ok,
    NoLockfile,
    /// `[MEDIUM-1]` `message` is RENDERED BY RUST from `category` + `line:col`
    /// using a fixed table. It never contains parser output or file content.
    ParseError {
        file: String,
        category: ParseErrorKind,
        line: Option<u32>,
        col: Option<u32>,
        message: String,
    },
    NotTracked {
        label: String,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManifestGroup {
    pub manifest_path: String,
    pub lockfile_path: Option<String>,
    /// None ⇔ NotTracked, or the `MAX_MANIFESTS` overflow `ParseError`
    /// group (`<N more manifests>`, `LimitExceeded`) `[P125d review]`.
    pub ecosystem: Option<Ecosystem>,
    pub state: GroupState,
    pub rows: Vec<DepRow>,
    /// `[UI-d]` Public transitive lock entries queried against OSV.
    pub transitive_checked: u32,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SeverityCounts {
    pub critical: u32,
    pub high: u32,
    pub medium: u32,
    pub low: u32,
    pub unknown: u32,
}

/// Computed ONLY by [`summarize`]; counts ROWS (except `not_tracked` and
/// `parse_errors`, which count groups).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DepSummary {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
    pub vulnerable: u32,
    pub vulnerable_by_severity: SeverityCounts,
    pub up_to_date: u32,
    pub unknown: u32,
    pub local: u32,
    pub private: u32,
    pub not_tracked: u32,
    pub parse_errors: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DepReport {
    pub repo_id: String,
    pub checked_at_ms: i64,
    /// Empty ⇒ "No dependency manifests found".
    pub groups: Vec<ManifestGroup>,
    pub summary: DepSummary,
    /// P125h §4b: a user/system config base dir is on a network share.
    /// `false` on v1 cache reports.
    #[serde(default)]
    pub network_profile: bool,
}

/// Persisted only (cache.rs); never on the wire.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileStamp {
    pub path: String,
    pub len: u64,
    pub mtime_ms: i64,
}

/// Persisted only (cache.rs); never on the wire.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CachedReport {
    pub schema: u32,
    pub repo_id: String,
    pub stamps: Vec<FileStamp>,
    pub report: DepReport,
}

#[cfg(test)]
#[path = "model_tests.rs"]
mod tests;
