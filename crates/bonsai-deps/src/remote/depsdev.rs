//! deps.dev stable v3 `GetPackage` (contract §0.1, §5.1 step 8, §7.5).
//!
//! The DTO deserializes only `versions[].versionKey.{name,version}`,
//! `isDefault` and `isDeprecated`; `links`, `registries`, `url`,
//! `deprecatedReason` and every unknown field are ignored. deps.dev maps a
//! crates.io / PyPI yank to `isDeprecated: true` (P125d live probe, §0.1),
//! so `select_latest` never picks a yanked version.

use serde::Deserialize;

use crate::check::error::{status_error, CheckError, ServiceFault};
use crate::model::Ecosystem;
use crate::privacy::{normalize, PublicPackage};

use super::transport::{OutboundRequest, Transport, TransportError, DEPSDEV_BODY_CAP};

/// §7.5 / P125h §2: more versions than this ⇒ that package is
/// [`VersionsLookup::TooLarge`] (`Unknown { reason: TooLarge }`), not an abort.
pub const MAX_VERSIONS: usize = 20_000;

/// One deps.dev lookup outcome (P125h §2).
#[derive(Clone, PartialEq, Eq)]
pub enum VersionsLookup {
    Found(Vec<RegistryVersion>),
    /// 404, or a `versionKey.name` that is not the package asked for.
    NotFound,
    /// The body exceeded `DEPSDEV_BODY_CAP`, or listed more than
    /// [`MAX_VERSIONS`] versions. A degraded response: it counts toward the
    /// check's breaker.
    TooLarge,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PackageDto {
    #[serde(default)]
    versions: Vec<VersionDto>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct VersionDto {
    version_key: VersionKeyDto,
    #[serde(default)]
    is_default: bool,
    #[serde(default)]
    is_deprecated: bool,
}

#[derive(Deserialize)]
struct VersionKeyDto {
    #[serde(default)]
    name: String,
    #[serde(default)]
    version: String,
}

/// One registry version as `version::select_latest` needs it.
#[derive(Clone, PartialEq, Eq)]
pub struct RegistryVersion {
    pub version: String,
    pub is_default: bool,
    pub is_deprecated: bool,
}

/// deps.dev canonicalises names: PyPI to PEP 503 (`Django` ⇒ `django`),
/// NuGet to lowercase (`Newtonsoft.Json` ⇒ `newtonsoft.json`) — P125d
/// live probe. npm / Cargo / Go names come back as sent.
fn same_name(eco: Ecosystem, sent: &str, got: &str) -> bool {
    match eco {
        Ecosystem::Pypi => normalize(eco, sent) == normalize(eco, got),
        Ecosystem::Nuget => sent.eq_ignore_ascii_case(got),
        Ecosystem::Npm | Ecosystem::Cargo | Ecosystem::Go => sent == got,
    }
}

/// Maps a deps.dev response.
pub(crate) fn map_package(
    p: &PublicPackage,
    status: u16,
    body: &[u8],
) -> Result<VersionsLookup, CheckError> {
    match status {
        200 => {}
        404 => return Ok(VersionsLookup::NotFound),
        s => return Err(status_error(s)),
    }
    let dto: PackageDto =
        serde_json::from_slice(body).map_err(|_| CheckError::Service(ServiceFault::BadJson))?;
    if dto.versions.len() > MAX_VERSIONS {
        return Ok(VersionsLookup::TooLarge);
    }
    if dto
        .versions
        .iter()
        .any(|v| !same_name(p.ecosystem(), p.name(), &v.version_key.name))
    {
        return Ok(VersionsLookup::NotFound);
    }
    Ok(VersionsLookup::Found(
        dto.versions
            .into_iter()
            .map(|v| RegistryVersion {
                version: v.version_key.version,
                is_default: v.is_default,
                is_deprecated: v.is_deprecated,
            })
            .collect(),
    ))
}

/// §5.1 step 8: one GET. An oversize body is a per-package
/// [`VersionsLookup::TooLarge`], not an error (P125h §2).
pub fn fetch_versions(t: &dyn Transport, p: &PublicPackage) -> Result<VersionsLookup, CheckError> {
    match t.send(&OutboundRequest::depsdev_package(p), DEPSDEV_BODY_CAP) {
        Ok(resp) => map_package(p, resp.status, &resp.body),
        Err(TransportError::TooLarge) => Ok(VersionsLookup::TooLarge),
        Err(e) => Err(e.into()),
    }
}
