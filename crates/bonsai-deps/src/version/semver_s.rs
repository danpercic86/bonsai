//! `semver_s` (npm, Cargo): `semver::Version` after trimming a leading `v` /
//! `=` (npm loose). SemVer 2 order with build metadata ignored (E1).

use semver::{BuildMetadata, Version};

use super::{lower_bound_scan, strip_lower_op, VersionScheme};

pub struct SemverScheme;

impl SemverScheme {
    fn parse_loose(s: &str) -> Option<Version> {
        let t = s.trim().trim_start_matches(['v', '=']).trim();
        let mut v = Version::parse(t).ok()?;
        // semver's Ord tie-breaks on build metadata; E1 says build is ignored.
        v.build = BuildMetadata::EMPTY;
        Some(v)
    }

    /// A partial version as a range floor: `1` / `1.x` ⇒ 1.0.0, `1.2` ⇒ 1.2.0.
    /// A pre-release / build suffix is only kept on a complete `X.Y.Z`.
    fn parse_partial(s: &str) -> Option<Version> {
        let t = s.trim().trim_start_matches(['v', '=']).trim();
        if let Some(v) = Self::parse_loose(t) {
            return Some(v);
        }
        let core = t.split(['-', '+']).next().unwrap_or(t);
        let mut nums = [0u64; 3];
        let mut seen = 0usize;
        for part in core.split('.') {
            if seen == 3 {
                return None;
            }
            if matches!(part, "x" | "X" | "*") {
                break;
            }
            nums[seen] = part.parse().ok()?;
            seen += 1;
        }
        if seen == 0 {
            return None;
        }
        Some(Version::new(nums[0], nums[1], nums[2]))
    }
}

impl VersionScheme for SemverScheme {
    type V = Version;

    fn parse(&self, s: &str) -> Option<Version> {
        Self::parse_loose(s)
    }

    fn is_prerelease(&self, v: &Version) -> bool {
        !v.pre.is_empty()
    }

    fn segments(&self, v: &Version) -> (u64, u64, u64) {
        (v.major, v.minor, v.patch)
    }

    /// npm (`^1.2`, `~1.2.3`, `>=1 <2`, `1.x`, `a || b`, `1.2.3 - 2.0.0`) and
    /// Cargo (`1.2`, `^1`, `=1.2.3`, `>=1, <2`) constraint floors.
    fn lower_bound(&self, constraint: &str) -> Option<Version> {
        lower_bound_scan(
            constraint,
            |c| c.is_whitespace() || c == ',',
            |clause| Self::parse_partial(strip_lower_op(clause)?),
        )
    }
}

#[cfg(test)]
#[path = "semver_s_tests.rs"]
mod tests;
