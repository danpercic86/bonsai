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

use super::transport::{OutboundRequest, Transport, DEPSDEV_BODY_CAP};

/// §7.5: more versions than this ⇒ `Service(TooLarge)`.
pub const MAX_VERSIONS: usize = 20_000;

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

/// Maps a deps.dev response. `Ok(None)` ⇒ Unknown (404, or a
/// `versionKey.name` that is not the package asked for).
pub(crate) fn map_package(
    p: &PublicPackage,
    status: u16,
    body: &[u8],
) -> Result<Option<Vec<RegistryVersion>>, CheckError> {
    match status {
        200 => {}
        404 => return Ok(None),
        s => return Err(status_error(s)),
    }
    let dto: PackageDto =
        serde_json::from_slice(body).map_err(|_| CheckError::Service(ServiceFault::BadJson))?;
    if dto.versions.len() > MAX_VERSIONS {
        return Err(CheckError::Service(ServiceFault::TooLarge));
    }
    if dto
        .versions
        .iter()
        .any(|v| !same_name(p.ecosystem(), p.name(), &v.version_key.name))
    {
        return Ok(None);
    }
    Ok(Some(
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

/// §5.1 step 8: one GET. `Ok(None)` ⇒ Unknown.
pub fn fetch_versions(
    t: &dyn Transport,
    p: &PublicPackage,
) -> Result<Option<Vec<RegistryVersion>>, CheckError> {
    let resp = t.send(&OutboundRequest::depsdev_package(p), DEPSDEV_BODY_CAP)?;
    map_package(p, resp.status, &resp.body)
}
