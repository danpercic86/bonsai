//! Branch listing: the sidebar snapshot (M5 contract §2).

use std::path::Path;

use crate::error::AppError;
use crate::git::repo::read_head_info;

use super::{ci_cmp, open_repo_at, BranchInfo, BranchesSnapshot, RemoteBranchInfo};

/// Blocking. One snapshot of local branches, remote-tracking branches, tags,
/// HEAD. Unborn repo: empty lists (or whatever exists), `head.unborn == true`
/// — `Ok`, not `Err`. Non-UTF-8 ref names are skipped with an eprintln,
/// never an error.
pub fn list_refs(workdir: &Path) -> Result<BranchesSnapshot, AppError> {
    let repo = open_repo_at(workdir)?;
    let head = read_head_info(&repo)?;

    let mut local = Vec::new();
    for item in repo.branches(Some(git2::BranchType::Local))? {
        let (branch, _) = item?;
        let name = match branch.name()? {
            Some(n) => n.to_string(),
            None => {
                eprintln!("bonsai: skipping local branch with non-UTF-8 name");
                continue;
            }
        };
        let is_head = branch.is_head();

        // Tip oid; direct local branches always have a target — the `continue`
        // is a defensive skip, consistent with the non-UTF-8 skip above.
        let local_oid = match branch.get().target() {
            Some(oid) => oid,
            None => {
                eprintln!("bonsai: skipping symbolic/targetless local branch");
                continue;
            }
        };
        let tip = local_oid.to_string();

        // Upstream shorthand; None when unset or the upstream ref is gone.
        let upstream_branch = branch.upstream().ok();
        let upstream = upstream_branch
            .as_ref()
            .and_then(|u| u.name().ok().flatten().map(str::to_string));

        // Ahead/behind is best-effort (contract §2.1): any lookup error
        // degrades to None — never fail the whole snapshot for it. Reuse
        // `local_oid` (already read above) rather than calling target() twice.
        let (ahead, behind) = match &upstream {
            Some(_) => {
                let upstream_oid = upstream_branch.as_ref().and_then(|u| u.get().target());
                match upstream_oid.map(|u| repo.graph_ahead_behind(local_oid, u)) {
                    Some(Ok((a, b))) => (u32::try_from(a).ok(), u32::try_from(b).ok()),
                    _ => (None, None),
                }
            }
            None => (None, None),
        };

        local.push(BranchInfo {
            name,
            is_head,
            upstream,
            ahead,
            behind,
            tip,
        });
    }
    local.sort_by(|a, b| ci_cmp(&a.name, &b.name));

    let mut remote = Vec::new();
    for item in repo.branches(Some(git2::BranchType::Remote))? {
        let (branch, _) = item?;
        // Skip symbolic entries — that is "<remote>/HEAD".
        if branch.get().symbolic_target().ok().flatten().is_some() {
            continue;
        }
        let name = match branch.name()? {
            Some(n) => n.to_string(),
            None => {
                eprintln!("bonsai: skipping remote branch with non-UTF-8 name");
                continue;
            }
        };
        let tip = match branch.get().target() {
            Some(oid) => oid.to_string(),
            None => {
                eprintln!("bonsai: skipping targetless remote branch");
                continue;
            }
        };
        remote.push(RemoteBranchInfo { name, tip });
    }
    remote.sort_by(|a, b| ci_cmp(&a.name, &b.name));

    let mut tags: Vec<String> = repo
        .tag_names(None)?
        .iter()
        .filter_map(Result::ok)
        .flatten()
        .map(str::to_string)
        .collect();
    tags.sort_by(|a, b| ci_cmp(a, b));

    Ok(BranchesSnapshot {
        local,
        remote,
        tags,
        head,
    })
}
