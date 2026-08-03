//! Per-worktree AI-context status matrix (P31 contract §4).
//!
//! Joins `git::worktree::list_worktrees` × the shared profile store × a
//! per-worktree `scan_inventory` into one `WorktreeContextStatus` row per
//! worktree. Blocking, runtime-free, unit-testable; the command layer wraps it
//! in `spawn_blocking`. Read-only — writes nothing anywhere.

use std::path::{Path, PathBuf};

use crate::assets::inventory::scan_inventory;
use crate::assets::profiles::{list_profiles, MAIN_WORKTREE_KEY};
use crate::error::AppError;
use crate::git::worktree::list_worktrees;

/// One row of the worktree × AI-context matrix (P31 §4). Wire: camelCase.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorktreeContextStatus {
    /// Store key + command argument: `"@main"` | linked worktree name (D3).
    pub worktree_key: String,
    /// Display name (main basename / linked name).
    pub name: String,
    /// Absolute path, forward slashes.
    pub abs_path: String,
    pub branch: Option<String>,
    pub is_main: bool,
    pub is_current: bool,
    pub locked: bool,
    pub prunable: bool,
    pub valid: bool,
    /// From `worktree_activations` (v1 legacy `active_profile` folded in for
    /// `"@main"` at read time).
    pub active_profile: Option<String>,
    /// D10: drift entries `comparable && exists && !in_sync` in THIS worktree.
    pub drifted_count: u32,
    /// Comparable descriptors with `exists == false` in THIS worktree.
    pub missing_count: u32,
    /// D6: `valid && !prunable && !locked`.
    pub activatable: bool,
    /// Human-readable reason when `!activatable`, else None.
    pub blocked_reason: Option<String>,
}

/// Blocking. `list_worktrees` × shared store × per-worktree `scan_inventory`
/// (the scan is skipped — 0/0 counts — when the row is not activatable, since
/// its working directory may not exist). One call, no per-worktree
/// round-trips. Stale store keys (worktrees since removed) are naturally
/// skipped: the matrix iterates real worktrees only.
pub fn list_worktree_contexts(workdir: &Path) -> Result<Vec<WorktreeContextStatus>, AppError> {
    let worktrees = list_worktrees(workdir)?;
    let store = list_profiles(workdir)?; // shared root (D1/D2)

    let mut out = Vec::with_capacity(worktrees.len());
    for wt in worktrees {
        let key = if wt.is_main {
            MAIN_WORKTREE_KEY.to_string()
        } else {
            wt.name.clone()
        };
        let activatable = wt.valid && !wt.prunable && !wt.locked;
        let blocked_reason = if activatable {
            None
        } else if !wt.valid {
            Some("worktree is invalid (working directory missing or broken)".to_string())
        } else if wt.prunable {
            Some("worktree is stale (prunable)".to_string())
        } else {
            Some(match &wt.lock_reason {
                Some(r) => format!("worktree is locked: {r}"),
                None => "worktree is locked".to_string(),
            })
        };
        let (drifted_count, missing_count) = if activatable {
            scan_counts(&PathBuf::from(&wt.abs_path))?
        } else {
            (0, 0)
        };
        out.push(WorktreeContextStatus {
            active_profile: store.effective_activation(&key).map(str::to_string),
            worktree_key: key,
            name: wt.name,
            abs_path: wt.abs_path,
            branch: wt.branch,
            is_main: wt.is_main,
            is_current: wt.is_current,
            locked: wt.locked,
            prunable: wt.prunable,
            valid: wt.valid,
            drifted_count,
            missing_count,
            activatable,
            blocked_reason,
        });
    }
    Ok(out)
}

/// D10: per-worktree drift = P24 `scan_inventory(<worktree root>, None)`
/// verbatim (canonical auto-picked per worktree). Returns
/// `(drifted, missing)` over the comparable set.
fn scan_counts(wt_root: &Path) -> Result<(u32, u32), AppError> {
    let inv = scan_inventory(wt_root, None)?;
    let drifted = inv
        .drift
        .entries
        .iter()
        .filter(|e| e.comparable && e.exists && !e.in_sync)
        .count() as u32;
    let missing = inv
        .drift
        .entries
        .iter()
        .filter(|e| e.comparable && !e.exists)
        .count() as u32;
    Ok((drifted, missing))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assets::profiles::{activate_profile_for_worktree, save_profile, ContextProfile, ProfileTarget};
    use crate::assets::taxonomy::descriptors;

    /// Scratch git fixture: main repo (committed CLAUDE.md) + two linked
    /// worktrees "feature-x" / "feature-y".
    fn git_fixture() -> (tempfile::TempDir, PathBuf, PathBuf, PathBuf) {
        let dir = crate::testutil::scratch_dir();
        let repo_dir = dir.path().join("repo");
        let repo = git2::Repository::init(&repo_dir).unwrap();
        let mut cfg = repo.config().unwrap();
        cfg.set_str("user.name", "Test").unwrap();
        cfg.set_str("user.email", "test@example.com").unwrap();
        std::fs::write(repo_dir.join("CLAUDE.md"), b"# base\n").unwrap();
        let mut idx = repo.index().unwrap();
        idx.add_path(Path::new("CLAUDE.md")).unwrap();
        idx.write().unwrap();
        let tree = repo.find_tree(idx.write_tree().unwrap()).unwrap();
        let sig = repo.signature().unwrap();
        let head = repo
            .commit(Some("HEAD"), &sig, &sig, "init", &tree, &[])
            .unwrap();
        let commit = repo.find_commit(head).unwrap();
        repo.branch("feature/x", &commit, false).unwrap();
        repo.branch("feature/y", &commit, false).unwrap();
        let wx = crate::git::worktree::add_worktree(&repo_dir, "feature/x").unwrap();
        let wy = crate::git::worktree::add_worktree(&repo_dir, "feature/y").unwrap();
        (
            dir,
            repo_dir,
            PathBuf::from(wx.abs_path),
            PathBuf::from(wy.abs_path),
        )
    }

    fn profile(name: &str, targets: Vec<(&str, &str)>) -> ContextProfile {
        ContextProfile {
            name: name.to_string(),
            description: None,
            model: None,
            targets: targets
                .into_iter()
                .map(|(id, c)| ProfileTarget {
                    asset_id: id.to_string(),
                    content: c.to_string(),
                })
                .collect(),
        }
    }

    fn comparable_total() -> u32 {
        descriptors().iter().filter(|d| d.comparable()).count() as u32
    }

    // §9.6 — matrix: keys, counts, activeProfile, blocked rows.
    #[test]
    fn matrix_reports_keys_counts_and_activations() {
        let (_dir, main, wx, _wy) = git_fixture();
        // feature-x: drifted (two comparable docs with different content).
        std::fs::write(wx.join("CLAUDE.md"), b"# a\n").unwrap();
        std::fs::write(wx.join("AGENTS.md"), b"# b\n").unwrap();
        // Activations: "@main" via profile p, feature-x via q.
        save_profile(&main, profile("p", vec![("gemini", "# g\n")])).unwrap();
        save_profile(&main, profile("q", vec![("gemini", "# g2\n")])).unwrap();
        activate_profile_for_worktree(&main, MAIN_WORKTREE_KEY, "p").unwrap();
        activate_profile_for_worktree(&main, "feature-x", "q").unwrap();

        let rows = list_worktree_contexts(&main).unwrap();
        assert_eq!(rows.len(), 3);
        let by_key = |k: &str| rows.iter().find(|r| r.worktree_key == k).unwrap();

        let m = by_key("@main");
        assert!(m.is_main && m.activatable && m.blocked_reason.is_none());
        assert_eq!(m.active_profile.as_deref(), Some("p"));
        // main: CLAUDE.md + GEMINI.md exist and differ → drifted ≥ 1.
        assert!(m.drifted_count >= 1);
        assert_eq!(m.missing_count, comparable_total() - 2);

        let x = by_key("feature-x");
        assert_eq!(x.name, "feature-x");
        assert_eq!(x.active_profile.as_deref(), Some("q"));
        assert!(x.drifted_count >= 1, "divergent docs drift");
        assert_eq!(x.missing_count, comparable_total() - 3);

        let y = by_key("feature-y");
        assert_eq!(y.active_profile, None);
        assert_eq!(y.drifted_count, 0, "single comparable doc is in sync");
        assert_eq!(y.missing_count, comparable_total() - 1);
        assert_eq!(y.branch.as_deref(), Some("feature/y"));
    }

    // §3 — v1 legacy activeProfile folds into the "@main" row at read time,
    // without rewriting the store.
    #[test]
    fn legacy_active_profile_folds_into_main_row() {
        let (_dir, main, _wx, _wy) = git_fixture();
        let path = main.join(".bonsai").join("profiles.json");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let v1 = br#"{ "version": 1, "profiles": [], "activeProfile": "legacy" }"#;
        std::fs::write(&path, v1).unwrap();

        let rows = list_worktree_contexts(&main).unwrap();
        let m = rows.iter().find(|r| r.worktree_key == "@main").unwrap();
        assert_eq!(m.active_profile.as_deref(), Some("legacy"));
        // Read-only: file untouched.
        assert_eq!(std::fs::read(&path).unwrap(), v1);
    }

    // §9.5 — locked / invalid rows are not activatable and carry a reason.
    #[test]
    fn blocked_rows_report_reason_and_skip_scan() {
        let (_dir, main, wx, _wy) = git_fixture();
        crate::git::worktree::lock_worktree(&main, "feature-y", Some("pinned for QA")).unwrap();
        std::fs::remove_dir_all(&wx).unwrap(); // feature-x → invalid/prunable

        let rows = list_worktree_contexts(&main).unwrap();
        let by_key = |k: &str| rows.iter().find(|r| r.worktree_key == k).unwrap();

        let y = by_key("feature-y");
        assert!(!y.activatable && y.locked);
        assert!(y.blocked_reason.as_deref().unwrap().contains("pinned for QA"));
        assert_eq!((y.drifted_count, y.missing_count), (0, 0), "scan skipped");

        let x = by_key("feature-x");
        assert!(!x.activatable);
        assert!(x.blocked_reason.is_some());
        assert_eq!((x.drifted_count, x.missing_count), (0, 0));
    }

    /// P31 §4 wire shape: camelCase keys, exactly the contract fields.
    #[test]
    fn wire_shape_is_camel_case() {
        let row = WorktreeContextStatus {
            worktree_key: "@main".to_string(),
            name: "repo".to_string(),
            abs_path: "/x/repo".to_string(),
            branch: Some("main".to_string()),
            is_main: true,
            is_current: true,
            locked: false,
            prunable: false,
            valid: true,
            active_profile: Some("opus".to_string()),
            drifted_count: 1,
            missing_count: 4,
            activatable: true,
            blocked_reason: None,
        };
        let v = serde_json::to_value(&row).expect("json");
        assert_eq!(
            v,
            serde_json::json!({
                "worktreeKey": "@main",
                "name": "repo",
                "absPath": "/x/repo",
                "branch": "main",
                "isMain": true,
                "isCurrent": true,
                "locked": false,
                "prunable": false,
                "valid": true,
                "activeProfile": "opus",
                "driftedCount": 1,
                "missingCount": 4,
                "activatable": true,
                "blockedReason": null
            })
        );
    }
}
