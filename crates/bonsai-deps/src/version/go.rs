//! `go` (E1): `vMAJOR.MINOR.PATCH[-pre][+incompatible]`. Pseudo-versions
//! (`vX.Y.Z-yyyymmddhhmmss-abcdef123456`) are SemVer pre-releases, so they
//! parse as pre-release naturally. `+incompatible` is ignored for order.

use semver::{BuildMetadata, Version};

use super::VersionScheme;

pub struct GoScheme;

impl VersionScheme for GoScheme {
    type V = Version;

    fn parse(&self, s: &str) -> Option<Version> {
        let t = s.trim().strip_prefix('v')?;
        let mut v = Version::parse(t).ok()?;
        v.build = BuildMetadata::EMPTY;
        Some(v)
    }

    fn is_prerelease(&self, v: &Version) -> bool {
        !v.pre.is_empty()
    }

    fn segments(&self, v: &Version) -> (u64, u64, u64) {
        (v.major, v.minor, v.patch)
    }

    /// go.mod `require` lines carry an exact (minimum) version.
    fn lower_bound(&self, constraint: &str) -> Option<Version> {
        self.parse(constraint)
    }
}

#[cfg(test)]
#[path = "go_tests.rs"]
mod tests;
