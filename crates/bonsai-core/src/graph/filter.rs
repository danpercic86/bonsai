//! Graph declutter filter (spec-003).
//!
//! [`GraphFilter`] is the wire-level knob pair threaded through the shared walk
//! seed and the lane walk: `first_parent` simplifies the revwalk AND truncates
//! parent routing; `seed_refs` restricts which refs seed the walk (and which
//! pills survive). All filtering SEMANTICS live here (file-size discipline —
//! `graph.rs` only threads the plan through `collect_refs`/`collect_seed`).
//!
//! Locked semantics (spec-003 plan §Approach):
//! - `seed_refs: None` → today's behavior (all refs), `applied = false`.
//! - `Some([])` → intentional hide-all → HEAD-only seed, `applied = true`.
//! - `Some(non-empty)` matching ZERO existing refs → stale persistence →
//!   fall back to the full graph, `applied = false`.
//! - When `seed_refs` is `Some` (and applied), stash tips are EXCLUDED from
//!   the seed and HEAD is always seeded; if HEAD's branch label was filtered
//!   out, a detached-style `Head` label is synthesized so the HEAD pill always
//!   resolves.

use std::collections::HashSet;

use super::{RefKind, RefLabel, RefMap};

/// Wire-level graph declutter filter (spec-003). `Default` == no filtering ==
/// today's behavior; the struct is `#[serde(default)]` + camelCase so future
/// additive knobs (e.g. `foldLinear`, spec-004) stay wire-compatible.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct GraphFilter {
    pub first_parent: bool,
    /// Full ref names; None = all refs (today's behavior).
    /// Some([]) = hide-all → HEAD-only seed. Some(non-empty) matching zero
    /// existing refs = stale → fallback to None semantics (seedRefsApplied=false).
    pub seed_refs: Option<Vec<String>>,
    /// Spec-004: gates fold-span computation ONLY — it never changes the walk.
    /// Excluded from [`walk_eq`](Self::walk_eq) so toggling fold is a cache Hit.
    pub fold_linear: bool,
}

impl GraphFilter {
    /// WALK-identity equality: `first_parent` + `seed_refs` only. `fold_linear`
    /// is deliberately excluded — it gates per-request span computation, so a
    /// fold toggle must classify as a cache Hit, not a re-walk (spec-004).
    pub fn walk_eq(&self, other: &GraphFilter) -> bool {
        self.first_parent == other.first_parent && self.seed_refs == other.seed_refs
    }
}

/// Whether a whitelist name would actually be ENUMERATED by `collect_refs`,
/// not merely whether a reference of that name exists. The staleness pre-check
/// must mirror the pass, or a name that resolves but is skipped there (the
/// symbolic `refs/remotes/*/HEAD`, a tag peeling to a blob/tree, an
/// unresolvable branch tip) would count as a match and yield `applied = true`
/// with an unintended HEAD-only graph.
fn seedable_ref(repo: &git2::Repository, name: &str) -> bool {
    if name.starts_with("refs/remotes/") && name.ends_with("/HEAD") {
        return false; // collect_refs skips "*/HEAD" remote refs
    }
    match repo.find_reference(name) {
        // collect_refs only seeds refs that peel to a commit.
        Ok(r) => r.peel(git2::ObjectType::Commit).is_ok(),
        Err(_) => false,
    }
}

/// The resolved seed-restriction plan for ONE `collect_refs` pass. Built by
/// [`SeedPlan::new`] BEFORE the pass (staleness is a pre-check, so a stale
/// whitelist never half-drops refs mid-pass).
pub(super) struct SeedPlan {
    /// `None` → keep everything (filter inactive OR stale fallback).
    allow: Option<HashSet<String>>,
    /// True when the seed-ref restriction actually took effect (the
    /// `seedRefsApplied` truth flag on the wire).
    applied: bool,
}

impl SeedPlan {
    /// Resolves `filter.seed_refs` against the repo's existing refs.
    /// Staleness pre-check: a `Some(non-empty)` whitelist where no name
    /// resolves to an existing reference falls back to the full graph.
    pub(super) fn new(repo: &git2::Repository, filter: &GraphFilter) -> SeedPlan {
        match &filter.seed_refs {
            None => SeedPlan {
                allow: None,
                applied: false,
            },
            Some(names) if names.is_empty() => SeedPlan {
                // Intentional hide-all: keep no refs; HEAD is injected below.
                allow: Some(HashSet::new()),
                applied: true,
            },
            Some(names) => {
                let existing: HashSet<String> = names
                    .iter()
                    .filter(|n| seedable_ref(repo, n))
                    .cloned()
                    .collect();
                if existing.is_empty() {
                    // Every name is stale → full-graph fallback.
                    SeedPlan {
                        allow: None,
                        applied: false,
                    }
                } else {
                    SeedPlan {
                        allow: Some(existing),
                        applied: true,
                    }
                }
            }
        }
    }

    /// Whether the restriction took effect (`seedRefsApplied`).
    pub(super) fn applied(&self) -> bool {
        self.applied
    }

    /// Whether a ref with this FULL name (`refs/heads/x`, `refs/remotes/o/x`,
    /// `refs/tags/v1`) seeds the walk and keeps its pill.
    pub(super) fn keep(&self, full_name: &str) -> bool {
        match &self.allow {
            None => true,
            Some(set) => set.contains(full_name),
        }
    }

    /// Stash tips are excluded from the seed whenever a seed restriction is
    /// active (AC3: "exactly X's ancestry plus HEAD's").
    pub(super) fn seed_stashes(&self) -> bool {
        self.allow.is_none()
    }

    /// Synthesizes the detached-style `Head` label on `head_oid` when HEAD is
    /// attached but its branch label was filtered out — the HEAD pill must
    /// always resolve. No-op when the restriction is inactive, HEAD is
    /// detached (the walk already labels it), unborn, or the branch was kept.
    pub(super) fn synthesize_head_label(
        &self,
        labels: &mut RefMap,
        head_oid: Option<git2::Oid>,
        detached: bool,
        head_branch: Option<&str>,
    ) {
        let Some(oid) = head_oid else { return };
        if self.allow.is_none() || detached {
            return;
        }
        let branch_kept = head_branch
            .map(|b| self.keep(&format!("refs/heads/{b}")))
            .unwrap_or(false);
        if !branch_kept {
            labels.entry(oid).or_default().push(RefLabel {
                name: "HEAD".to_string(),
                kind: RefKind::Head,
                is_head: true,
            });
        }
    }
}
