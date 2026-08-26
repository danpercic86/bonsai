//! Branch deletion (§9.1(9)) — the TOCTOU tip-recheck guard and the blocking
//! `delete_branches` entry point. Split out of `stale.rs`; behavior unchanged.
//! Re-exported from the module root so existing paths resolve unchanged.

use super::*;

/// F-A7-3 (TOCTOU guard): re-read the branch tip at delete time. Returns
/// `Some(Failed row)` when the tip no longer matches the freshly-scanned
/// `expected` oid (the branch moved between scan and delete — do NOT delete),
/// `None` when it is unchanged and safe to delete.
pub(crate) fn recheck_tip(
    branch: &git2::Branch,
    name: &str,
    expected: git2::Oid,
) -> Option<BranchDeleteResult> {
    match branch.get().target() {
        Some(now) if now == expected => None,
        Some(now) => Some(BranchDeleteResult {
            name: name.to_string(),
            status: BranchDeleteStatus::Failed,
            message: Some(format!(
                "tip moved since scan ({} -> {}); not deleted — re-run the scan",
                short_oid(expected),
                short_oid(now)
            )),
        }),
        None => Some(BranchDeleteResult {
            name: name.to_string(),
            status: BranchDeleteStatus::Failed,
            message: Some("tip changed since scan (no longer a direct ref); not deleted".to_string()),
        }),
    }
}

/// Blocking. Deletes each caller-supplied name that is STILL safe, refusing the
/// current branch, the base branch, and anything not in a freshly-recomputed
/// stale set (defense-in-depth — NEVER trusts the client). Deletes directly via
/// git2 `Branch::delete()` (OPEN #10 — `branches::delete_branch`'s merged-into-HEAD
/// guard would wrongly block a branch merged into the base while a different
/// branch is checked out). Returns a per-branch result; a per-branch failure is
/// reported, NEVER a whole-call error. `base` mirrors `find_stale_branches` so
/// the safe set is recomputed against the same base. Each branch's tip is
/// re-read immediately before deletion and the delete refused if it moved
/// since the recompute (F-A7-3). Errors (whole-call): `git` (bad base / bare)
/// | `noRepo` (command layer).
pub fn delete_branches(
    workdir: &Path,
    names: &[String],
    base: Option<&str>,
) -> Result<Vec<BranchDeleteResult>, AppError> {
    // Recompute the safe set + base identity from scratch — the load-bearing
    // guard. Carry each safe branch's SCANNED tip oid into the delete loop so
    // a tip that moves between scan and delete is refused (F-A7-3).
    let (report, protected) = stale_scan(workdir, base)?;
    let safe: HashMap<&str, git2::Oid> = report
        .branches
        .iter()
        .filter_map(|b| git2::Oid::from_str(&b.tip).ok().map(|oid| (b.name.as_str(), oid)))
        .collect();

    let repo = open_repo_at(workdir)?;
    let current = read_head_info(&repo)?.branch_name;

    let mut results = Vec::with_capacity(names.len());
    for name in names {
        if current.as_deref() == Some(name.as_str()) {
            results.push(BranchDeleteResult {
                name: name.clone(),
                status: BranchDeleteStatus::SkippedCurrent,
                message: Some("checked-out branch".to_string()),
            });
            continue;
        }
        // Base by name OR resolved identity (F-A7-1/F-A7-4) OR default branch.
        if name == &report.base || protected.contains(name.as_str()) {
            results.push(BranchDeleteResult {
                name: name.clone(),
                status: BranchDeleteStatus::SkippedBase,
                message: Some("base/default branch".to_string()),
            });
            continue;
        }
        let Some(&expected_tip) = safe.get(name.as_str()) else {
            results.push(BranchDeleteResult {
                name: name.clone(),
                status: BranchDeleteStatus::SkippedNotStale,
                message: Some("not detected as stale".to_string()),
            });
            continue;
        };

        match repo.find_branch(name, git2::BranchType::Local) {
            Err(e) if e.code() == git2::ErrorCode::NotFound => {
                results.push(BranchDeleteResult {
                    name: name.clone(),
                    status: BranchDeleteStatus::SkippedNotFound,
                    message: Some("not found".to_string()),
                });
            }
            Err(e) => {
                results.push(BranchDeleteResult {
                    name: name.clone(),
                    status: BranchDeleteStatus::Failed,
                    message: Some(e.message().to_string()),
                });
            }
            Ok(mut branch) => {
                // F-A7-3: refuse if the tip moved since the scan above.
                if let Some(row) = recheck_tip(&branch, name, expected_tip) {
                    results.push(row);
                    continue;
                }
                match branch.delete() {
                    Ok(()) => results.push(BranchDeleteResult {
                        name: name.clone(),
                        status: BranchDeleteStatus::Deleted,
                        // F-A7-5: record the deleted tip for recovery.
                        message: Some(format!("was at {}", short_oid(expected_tip))),
                    }),
                    Err(e) => results.push(BranchDeleteResult {
                        name: name.clone(),
                        status: BranchDeleteStatus::Failed,
                        message: Some(e.message().to_string()),
                    }),
                }
            }
        }
    }
    Ok(results)
}

