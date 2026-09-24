//! [`ActivityTarget`]: the sanitized, capped run-target identifier. A child
//! module of `git::activity` (split for size, P119 §2.2), so it calls the
//! parent's private `strip_control_chars` / `truncate_chars` and the one-funnel
//! property holds: every constructor goes through [`ActivityTarget::new`].

use super::{strip_control_chars, truncate_chars};

/// Cap on a run target, in CHARS. Git's practical ref bound is far under this,
/// so 255 never truncates a real name; it exists so a hostile ref cannot park a
/// megabyte string in the frontend's 200-run store (FU-1 §3.6-4).
pub const MAX_ACTIVITY_TARGET_CHARS: usize = 255;

/// Length of the short commit oid a `Commit` target carries.
const SHORT_OID_CHARS: usize = 7;

/// A run's target ref: a RAW git identifier, sanitized and capped. NEVER a human
/// phrase — the frontend derives all copy (`all remotes`, prepositions) from the
/// category (FU-1 §3.3).
///
/// The inner `String` is private and there is NO public constructor: outside this
/// crate the only ways to obtain one are
/// [`crate::git::activity_target::resolve_activity_target`] and
/// [`crate::git::activity_target::arg_activity_target`], so the command layer
/// cannot invent a target at all (FU-1 §6, guarantee 1 — structural at the crate
/// boundary).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActivityTarget(String);

impl ActivityTarget {
    /// THE funnel: the same [`strip_control_chars`] + [`truncate_chars`] rule
    /// `activity_line` applies, one implementation (FU-1 §6.1). `None` when
    /// nothing survives sanitation.
    fn new(raw: &str) -> Option<Self> {
        let clean = strip_control_chars(raw).trim().to_string();
        if clean.is_empty() {
            return None;
        }
        Some(ActivityTarget(truncate_chars(
            &clean,
            MAX_ACTIVITY_TARGET_CHARS,
        )))
    }

    /// A remote name (`origin`).
    pub(crate) fn remote(remote: &str) -> Option<Self> {
        Self::new(remote)
    }

    /// `remote/branch` (`origin/main`). `branch` may be a short name or
    /// `refs/heads/<x>`; the prefix is stripped. Each part is validated
    /// SEPARATELY, so a blank part yields `None` rather than `"/main"`.
    pub(crate) fn remote_branch(remote: &str, branch: &str) -> Option<Self> {
        let rp = Self::remote(remote)?;
        let bp = Self::branch(branch)?;
        Self::new(&format!("{}/{}", rp.as_str(), bp.as_str()))
    }

    /// A branch SHORT name; a `refs/heads/` prefix is stripped here so no call
    /// site can leak a full refname.
    pub(crate) fn branch(branch: &str) -> Option<Self> {
        Self::new(branch.strip_prefix("refs/heads/").unwrap_or(branch))
    }

    /// A tag SHORT name; a `refs/tags/` prefix is stripped. P119.
    pub(crate) fn tag(tag: &str) -> Option<Self> {
        Self::new(tag.strip_prefix("refs/tags/").unwrap_or(tag))
    }

    /// Any ref: strips exactly ONE of `refs/heads/`, `refs/remotes/`,
    /// `refs/tags/` (never two in sequence). P119.
    pub(crate) fn any_ref(r: &str) -> Option<Self> {
        let short = r
            .strip_prefix("refs/heads/")
            .or_else(|| r.strip_prefix("refs/remotes/"))
            .or_else(|| r.strip_prefix("refs/tags/"))
            .unwrap_or(r);
        Self::new(short)
    }

    /// A commit's 7-char short oid. `Some` iff the input is at least 7 chars and
    /// ALL ASCII hex — a revspec like `HEAD~2` is not an oid and yields `None`
    /// rather than being passed through. P119.
    pub(crate) fn commit(oid: &str) -> Option<Self> {
        if oid.len() < SHORT_OID_CHARS || !oid.bytes().all(|b| b.is_ascii_hexdigit()) {
            return None;
        }
        // All-ASCII was just checked, so byte slicing is on a char boundary.
        Self::new(&oid[..SHORT_OID_CHARS])
    }

    /// A stash entry, `stash@{N}`. P119.
    pub(crate) fn stash(index: usize) -> Option<Self> {
        Self::new(&format!("stash@{{{index}}}"))
    }

    /// A raw name/path identifier (submodule, worktree, file path). It MAY
    /// contain spaces; "never a phrase" for this constructor is backed by the
    /// call-site table + tests, not the type (P119 §2.4).
    pub(crate) fn name(raw: &str) -> Option<Self> {
        Self::new(raw)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn into_string(self) -> String {
        self.0
    }
}
