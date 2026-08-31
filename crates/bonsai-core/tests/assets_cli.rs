//! P24 AI-asset integration tests (contract §11 / §12 AI gate).
//!
//! These strengthen the in-crate unit tests with the oracles the units cannot
//! reach:
//!   1. a REAL `git hash-object` cross-check of the inventory content hash (the
//!      unit test hashes via git2 — the same lib prod uses — so it cannot catch
//!      a systematic git2 misuse; the CLI can);
//!   2. an end-to-end `save_profile` → `preview_profile` (writes nothing) →
//!      `activate_profile` (writes real files) round-trip, statted on disk;
//!   3. drift recomputed against reality after a write;
//!   4. path-safety rejection + rules-dir member listing.
//!
//! The assets core is fs-only (no git repo needed), so most tests use a plain
//! scratch dir; only test #1 shells out to `git` (skips gracefully if absent).
//! Every scratch dir lives under `D:\Data\Temp\bonsai-scratch` (C: is full) via
//! `common::scratch_dir`.

mod common;

use std::path::Path;

use bonsai_core::assets::{
    activate_profile, list_profiles, preview_profile, read_asset, save_profile, scan_inventory,
    ContextProfile, ProfileTarget, TargetWriteAction,
};
use bonsai_core::error::AppError;
use common::{git, scratch_dir};

macro_rules! require_git {
    () => {
        if !common::have_git() {
            eprintln!("skipping: `git` CLI not found on PATH");
            return;
        }
    };
}

fn write(root: &Path, rel: &str, bytes: &[u8]) {
    let full = root.join(rel);
    if let Some(parent) = full.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(full, bytes).unwrap();
}

fn target(asset_id: &str, content: &str) -> ProfileTarget {
    ProfileTarget {
        asset_id: asset_id.to_string(),
        content: content.to_string(),
    }
}

fn profile(name: &str, targets: Vec<ProfileTarget>) -> ContextProfile {
    ContextProfile {
        name: name.to_string(),
        description: None,
        model: None,
        targets,
    }
}

/// §11 row 2 — REAL `git hash-object` oracle for the raw content hash. The unit
/// test hashes via `git2::Oid::hash_object` (prod's own path); the external CLI
/// is an INDEPENDENT oracle that catches systematic git2 misuse.
///
/// `-c core.autocrlf=false` forces raw-byte hashing (Windows global config often
/// enables autocrlf) so the CLI blob oid matches the raw bytes on disk exactly.
#[test]
fn content_hash_matches_real_git_hash_object() {
    require_git!();
    let dir = scratch_dir();
    let root = dir.path();
    // Known bytes with an internal newline (LF only — no CR to convert).
    let bytes = b"# Oracle test\nThese are known bytes for hash-object.\n";
    write(root, "CLAUDE.md", bytes);

    let inv = scan_inventory(root, None).unwrap();
    let claude = inv.assets.iter().find(|a| a.id == "claude").unwrap();
    assert!(claude.exists);
    assert_eq!(claude.files.len(), 1);

    // Independent oracle: the actual git CLI's blob oid for the same file.
    let cli_oid = git(root, &["-c", "core.autocrlf=false", "hash-object", "CLAUDE.md"]);
    assert_eq!(
        claude.files[0].content_hash, cli_oid,
        "inventory contentHash must equal `git hash-object` of the raw bytes"
    );
    assert_eq!(claude.files[0].size, bytes.len() as u64);
}

/// §11 rows 8+9 (integration) — the full activation write-path against a real
/// filesystem: preview writes nothing; activate creates both mapped files with
/// byte-exact content, sets `active_profile`, persists a round-trippable
/// `.bonsai/profiles.json`, leaves NO `.bonsai-tmp` remnant; re-activate is all
/// Unchanged.
#[test]
fn activate_writes_real_files_end_to_end() {
    let dir = scratch_dir();
    let root = dir.path();

    // Two single-file targets whose content differs from anything on disk
    // (neither file exists yet).
    let claude_body = "# Opus profile\nrich context here\n";
    let agents_body = "# Agents flavor\nsame guidance, codex tone\n";
    save_profile(
        root,
        profile(
            "opus-rich",
            vec![target("claude", claude_body), target("agents", agents_body)],
        ),
    )
    .unwrap();

    // --- preview: reports created/changed, writes NOTHING ---
    let preview = preview_profile(root, "opus-rich").unwrap();
    assert_eq!(preview.len(), 2);
    assert!(preview.iter().all(|e| e.changed), "new files all change");
    assert!(
        preview.iter().all(|e| e.current.is_none()),
        "no current content for absent files"
    );
    // Stat the dir: the mapped files must NOT exist after a mere preview.
    assert!(!root.join("CLAUDE.md").exists(), "preview must not write CLAUDE.md");
    assert!(!root.join("AGENTS.md").exists(), "preview must not write AGENTS.md");

    // --- activate: writes the real files ---
    let act = activate_profile(root, "opus-rich").unwrap();
    let action_of = |id: &str| {
        act.results
            .iter()
            .find(|r| r.asset_id == id)
            .unwrap()
            .action
    };
    assert_eq!(action_of("claude"), TargetWriteAction::Created);
    assert_eq!(action_of("agents"), TargetWriteAction::Created);

    // Byte-exact content landed on disk.
    assert_eq!(
        std::fs::read(root.join("CLAUDE.md")).unwrap(),
        claude_body.as_bytes()
    );
    assert_eq!(
        std::fs::read(root.join("AGENTS.md")).unwrap(),
        agents_body.as_bytes()
    );

    // active_profile set + store persisted + round-trips from disk.
    assert_eq!(act.store.active_profile.as_deref(), Some("opus-rich"));
    let store_path = root.join(".bonsai").join("profiles.json");
    assert!(store_path.is_file(), ".bonsai/profiles.json must exist");
    let reloaded = list_profiles(root).unwrap();
    assert_eq!(reloaded.active_profile.as_deref(), Some("opus-rich"));
    assert_eq!(reloaded.profiles.len(), 1);
    assert_eq!(reloaded.profiles[0].name, "opus-rich");
    assert_eq!(reloaded.profiles[0].targets.len(), 2);

    // No `.bonsai-tmp` remnant left by the atomic temp+rename anywhere.
    assert!(!root.join("CLAUDE.md.bonsai-tmp").exists());
    assert!(!root.join("AGENTS.md.bonsai-tmp").exists());
    assert!(!root.join(".bonsai").join("profiles.json.bonsai-tmp").exists());
    // Belt-and-suspenders: scan the workdir + .bonsai for any stray temp file.
    assert!(no_tmp_remnant(root), "no *.bonsai-tmp anywhere under the workdir");

    // Re-activate: identical content → all Unchanged.
    let again = activate_profile(root, "opus-rich").unwrap();
    assert!(
        again
            .results
            .iter()
            .all(|r| r.action == TargetWriteAction::Unchanged),
        "second activation of the same profile is a no-op"
    );
}

/// Recursively assert no `*.bonsai-tmp` file lingers under `root`.
fn no_tmp_remnant(root: &Path) -> bool {
    fn walk(dir: &Path) -> bool {
        for entry in std::fs::read_dir(dir).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            if path.is_dir() {
                if !walk(&path) {
                    return false;
                }
            } else if path
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.ends_with(".bonsai-tmp"))
            {
                return false;
            }
        }
        true
    }
    walk(root)
}

/// §11 row 4 (integration) — drift reflects reality: an initially-drifted
/// AGENTS.md becomes in-sync after activating a profile that rewrites it to the
/// canonical (CLAUDE.md) content.
#[test]
fn drift_flips_to_in_sync_after_activation() {
    let dir = scratch_dir();
    let root = dir.path();

    let canon = "# Shared canon\nidentical guidance\n";
    write(root, "CLAUDE.md", canon.as_bytes());
    write(root, "AGENTS.md", b"# Stale agents\ndifferent guidance\n");

    // Before: AGENTS.md drifts from the canonical CLAUDE.md.
    let before = scan_inventory(root, None).unwrap().drift;
    assert_eq!(before.canonical_id.as_deref(), Some("claude"));
    let agents_before = before.entries.iter().find(|e| e.asset_id == "agents").unwrap();
    assert!(!agents_before.in_sync, "AGENTS.md starts drifted");
    assert!(!before.in_sync, "report starts out of sync");

    // Activate a profile that rewrites AGENTS.md to the canonical content.
    save_profile(root, profile("sync-up", vec![target("agents", canon)])).unwrap();
    let act = activate_profile(root, "sync-up").unwrap();
    assert_eq!(
        act.results.iter().find(|r| r.asset_id == "agents").unwrap().action,
        TargetWriteAction::Written
    );

    // After: the two files share a normalized hash → mutually in sync.
    let after = scan_inventory(root, None).unwrap().drift;
    assert_eq!(after.canonical_id.as_deref(), Some("claude"));
    let agents_after = after.entries.iter().find(|e| e.asset_id == "agents").unwrap();
    assert!(agents_after.in_sync, "AGENTS.md is in sync after activation");
    assert!(after.in_sync, "report is fully in sync after activation");
    assert_eq!(agents_after.normalized_hash, after.canonical_hash);
}

/// §11 row 4 / §0 safety — `read_asset` rejects `..` / absolute paths (mapped to
/// `Other`), and a rules-dir with two `*.mdc` members lists exactly those two,
/// sorted, ignoring non-matching files.
#[test]
fn path_safety_and_rules_dir_member_listing() {
    let dir = scratch_dir();
    let root = dir.path();

    // Path-escape defense: `..`, POSIX-absolute, and Windows-absolute all rejected
    // as AppError::Other (validate_rel_path), before any read.
    for bad in ["../escape.md", "/etc/passwd", "C:/Windows/system32/drivers/etc/hosts"] {
        let err = read_asset(root, bad).unwrap_err();
        assert!(
            matches!(err, AppError::Other(_)),
            "path {bad:?} must be rejected as Other, got {err:?}"
        );
    }

    // Rules-dir member listing: exactly the two *.mdc, sorted; the .txt ignored.
    write(root, ".cursor/rules/z-last.mdc", b"z rule\n");
    write(root, ".cursor/rules/a-first.mdc", b"a rule\n");
    write(root, ".cursor/rules/README.txt", b"ignore me\n");

    let inv = scan_inventory(root, None).unwrap();
    let cursor = inv.assets.iter().find(|a| a.id == "cursorRules").unwrap();
    assert!(cursor.exists);
    assert_eq!(cursor.files.len(), 2, "only the two *.mdc members count");
    assert_eq!(cursor.files[0].path, ".cursor/rules/a-first.mdc");
    assert_eq!(cursor.files[1].path, ".cursor/rules/z-last.mdc");
    // It is inventoried but NOT drift-comparable (frontmatter dir).
    assert!(
        !inv.drift.entries.iter().any(|e| e.asset_id == "cursorRules"),
        "rules-dir is not in the drift-comparable set"
    );
}
