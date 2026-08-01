//! P26 agent-asset bundle integration tests (contract §9/§10, §11 AI gate).
//!
//! The in-crate unit tests in `assets/bundle.rs` already cover the parse /
//! serialize / validate units exhaustively. This binary exercises the FULL
//! lifecycle end-to-end on a REAL on-disk `.claude/` tree — the oracle the units
//! cannot reach: create → scan → read round-trips, atomic edits that preserve
//! unknown keys with no temp remnant, the load-bearing complex-frontmatter
//! re-guard, skill-directory-recursive vs single-file delete semantics, and the
//! validation verdicts as they land through `save`/`scan`.
//!
//! The bundle core is fs-only (no git repo needed), so every fixture is a plain
//! scratch dir under `D:\Temp\bonsai-scratch` (C: is full) via
//! `common::scratch_dir`.

mod common;

use std::path::Path;

use bonsai_core::assets::{
    delete_agent_asset, read_agent_asset, save_agent_asset, scan_agent_assets, AgentAssetInput,
    AgentAssetKind, FrontmatterField, IssueSeverity,
};
use bonsai_core::error::AppError;
use common::scratch_dir;

/// Write raw bytes to `root/rel`, creating parent dirs (for hand-authored
/// fixtures the app itself would not normally write, e.g. complex YAML).
fn write(root: &Path, rel: &str, bytes: &[u8]) {
    let full = root.join(rel);
    if let Some(parent) = full.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(full, bytes).unwrap();
}

/// Build an `AgentAssetInput` from flat `(key, value)` pairs + a body.
fn input(kind: AgentAssetKind, name: &str, fm: &[(&str, &str)], body: &str) -> AgentAssetInput {
    AgentAssetInput {
        kind,
        name: name.to_string(),
        frontmatter: fm
            .iter()
            .map(|(k, v)| FrontmatterField {
                key: (*k).to_string(),
                value: (*v).to_string(),
            })
            .collect(),
        body: body.to_string(),
    }
}

/// Recursively assert no `*.bonsai-tmp` file lingers under `root` (the atomic
/// temp+rename must clean up after itself, mirroring the P24 `assets_cli` check).
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

/// §9/§10 area 1 — create → scan → read round-trip across all three kinds on a
/// real on-disk `.claude/` tree. `save_agent_asset` a new skill (dir + SKILL.md),
/// agent, and command; `scan_agent_assets` lists all three `valid:true` with the
/// right kinds/paths/sort order; `read_agent_asset` returns byte-consistent
/// frontmatter/body for each.
#[test]
fn create_scan_read_round_trip_all_kinds() {
    let dir = scratch_dir();
    let root = dir.path();

    // A skill owns its `<name>/` directory; save must create it.
    save_agent_asset(
        root,
        input(
            AgentAssetKind::Skill,
            "code-review",
            &[("name", "code-review"), ("description", "Reviews code")],
            "\n# Code review\n\nDo the review.\n",
        ),
    )
    .unwrap();
    save_agent_asset(
        root,
        input(
            AgentAssetKind::Agent,
            "test-runner",
            &[
                ("name", "test-runner"),
                ("description", "Runs tests"),
                ("tools", "Bash"),
                ("model", "inherit"),
            ],
            "\nYou run tests.\n",
        ),
    )
    .unwrap();
    save_agent_asset(
        root,
        input(
            AgentAssetKind::Command,
            "changelog",
            &[("description", "Update changelog"), ("argument-hint", "<version>")],
            "\nUpdate the changelog for $ARGUMENTS.\n",
        ),
    )
    .unwrap();

    // The mapped files exist on disk with the expected layout.
    assert!(root.join(".claude/skills/code-review/SKILL.md").is_file());
    assert!(root.join(".claude/agents/test-runner.md").is_file());
    assert!(root.join(".claude/commands/changelog.md").is_file());
    assert!(no_tmp_remnant(root), "no *.bonsai-tmp remnant after creates");

    // scan lists all three, sorted (skill < agent < command), all valid.
    let inv = scan_agent_assets(root).unwrap();
    assert_eq!(inv.assets.len(), 3, "exactly the three created assets");
    assert!(
        inv.assets.iter().all(|a| a.exists && a.validation.valid),
        "every created asset exists and is valid"
    );
    assert_eq!(inv.assets[0].kind, AgentAssetKind::Skill);
    assert_eq!(inv.assets[0].name, "code-review");
    assert_eq!(inv.assets[0].path, ".claude/skills/code-review/SKILL.md");
    assert_eq!(inv.assets[1].kind, AgentAssetKind::Agent);
    assert_eq!(inv.assets[1].name, "test-runner");
    assert_eq!(inv.assets[1].path, ".claude/agents/test-runner.md");
    assert_eq!(inv.assets[2].kind, AgentAssetKind::Command);
    assert_eq!(inv.assets[2].name, "changelog");
    assert_eq!(inv.assets[2].path, ".claude/commands/changelog.md");

    // read returns byte-consistent frontmatter/body for each (matches scan).
    for scanned in &inv.assets {
        let read = read_agent_asset(root, scanned.kind, &scanned.name).unwrap();
        assert_eq!(read.frontmatter, scanned.frontmatter, "frontmatter consistent");
        assert_eq!(read.body, scanned.body, "body consistent");
        assert_eq!(read.path, scanned.path);
        assert!(read.exists && read.validation.valid);
    }

    // Spot-check the agent's parsed frontmatter values + verbatim body.
    let agent = read_agent_asset(root, AgentAssetKind::Agent, "test-runner").unwrap();
    assert_eq!(
        agent
            .frontmatter
            .iter()
            .map(|f| (f.key.as_str(), f.value.as_str()))
            .collect::<Vec<_>>(),
        vec![
            ("name", "test-runner"),
            ("description", "Runs tests"),
            ("tools", "Bash"),
            ("model", "inherit"),
        ]
    );
    assert_eq!(agent.body, "\nYou run tests.\n");
}

/// §9/§10 area 2 — an edit preserves unknown/preserved frontmatter keys IN ORDER
/// and is atomic (no `.bonsai-tmp` remnant anywhere). Save an agent with a known
/// field AND an unknown `color: blue`; re-read shows both, in the original order,
/// after editing the known field.
#[test]
fn edit_preserves_unknown_keys_in_order_and_is_atomic() {
    let dir = scratch_dir();
    let root = dir.path();

    // Initial save carries an unknown key `color` between two known keys.
    save_agent_asset(
        root,
        input(
            AgentAssetKind::Agent,
            "helper",
            &[
                ("name", "helper"),
                ("color", "blue"),
                ("description", "original"),
            ],
            "\nSystem prompt.\n",
        ),
    )
    .unwrap();

    // Load, edit only `description`, carry EVERY field (incl. unknown `color`).
    let loaded = read_agent_asset(root, AgentAssetKind::Agent, "helper").unwrap();
    let mut fm = loaded.frontmatter.clone();
    for f in fm.iter_mut() {
        if f.key == "description" {
            f.value = "updated".to_string();
        }
    }
    save_agent_asset(
        root,
        AgentAssetInput {
            kind: AgentAssetKind::Agent,
            name: "helper".to_string(),
            frontmatter: fm,
            body: loaded.body.clone(),
        },
    )
    .unwrap();

    // Re-read: both keys survive, in the ORIGINAL insertion order, edit applied.
    let re = read_agent_asset(root, AgentAssetKind::Agent, "helper").unwrap();
    assert_eq!(
        re.frontmatter
            .iter()
            .map(|f| (f.key.as_str(), f.value.as_str()))
            .collect::<Vec<_>>(),
        vec![
            ("name", "helper"),
            ("color", "blue"),
            ("description", "updated"),
        ],
        "unknown key preserved in order; known field edited"
    );
    // Atomic: no temp remnant anywhere under the workdir.
    assert!(
        !root.join(".claude/agents/helper.md.bonsai-tmp").exists(),
        "no sibling temp file"
    );
    assert!(no_tmp_remnant(root), "no *.bonsai-tmp anywhere");
}

/// §9/§10 area 3 — THE COMPLEX RE-GUARD (load-bearing data-loss safety). A
/// hand-authored agent with genuinely complex frontmatter (a YAML block list)
/// reads as `complex:true` with an Error issue; attempting to `save` over it with
/// a flat payload returns `AppError::Other` (the backend re-guard) AND leaves the
/// on-disk file byte-UNCHANGED. Saving a NEW name and overwriting a FLAT existing
/// file both still succeed.
#[test]
fn complex_frontmatter_reguard_refuses_lossy_overwrite() {
    let dir = scratch_dir();
    let root = dir.path();

    // Genuinely complex: a YAML block-sequence under `tools:` the flat parser
    // cannot round-trip. Hand-authored (the editor could never produce it).
    let complex_bytes =
        b"---\nname: fancy\ndescription: has a list\ntools:\n  - Read\n  - Bash\n---\n\nSystem prompt.\n";
    write(root, ".claude/agents/fancy.md", complex_bytes);

    // read flags it complex + Error, so the editor opens it read-only.
    let loaded = read_agent_asset(root, AgentAssetKind::Agent, "fancy").unwrap();
    assert!(loaded.complex, "block-sequence frontmatter is complex");
    assert!(!loaded.validation.valid, "complex asset is invalid");
    assert!(
        loaded.validation.issues.iter().any(|i| i.severity
            == IssueSeverity::Error
            && i.message.contains("multi-line YAML")),
        "complex asset carries the multi-line-YAML Error"
    );

    // The re-guard: a flat overwrite of the SAME name is refused with `Other`
    // and writes nothing — the on-disk bytes are unchanged.
    let err = save_agent_asset(
        root,
        input(
            AgentAssetKind::Agent,
            "fancy",
            &[("name", "fancy"), ("description", "clobbered")],
            "\nnew body\n",
        ),
    )
    .unwrap_err();
    assert!(
        matches!(err, AppError::Other(_)),
        "flat overwrite of a complex asset must be AppError::Other, got {err:?}"
    );
    assert_eq!(
        std::fs::read(root.join(".claude/agents/fancy.md")).unwrap(),
        complex_bytes.to_vec(),
        "the complex file must be byte-UNCHANGED after a refused save"
    );
    assert!(no_tmp_remnant(root), "the refused save leaves no temp remnant");

    // A brand-NEW (non-existent) name is unaffected by the re-guard.
    save_agent_asset(
        root,
        input(
            AgentAssetKind::Agent,
            "brand-new",
            &[("name", "brand-new"), ("description", "fresh")],
            "\nbody\n",
        ),
    )
    .unwrap();
    assert!(root.join(".claude/agents/brand-new.md").is_file());

    // Overwriting a FLAT existing file still succeeds.
    write(
        root,
        ".claude/agents/plain.md",
        b"---\nname: plain\ndescription: old\n---\n\nold\n",
    );
    save_agent_asset(
        root,
        input(
            AgentAssetKind::Agent,
            "plain",
            &[("name", "plain"), ("description", "new")],
            "\nnew body\n",
        ),
    )
    .unwrap();
    let plain = read_agent_asset(root, AgentAssetKind::Agent, "plain").unwrap();
    assert_eq!(plain.body, "\nnew body\n");
    assert_eq!(
        plain
            .frontmatter
            .iter()
            .find(|f| f.key == "description")
            .unwrap()
            .value,
        "new"
    );
}

/// §9/§10 area 4 — delete semantics on a real tree. A skill delete removes the
/// ENTIRE `.claude/skills/<name>/` directory (incl. a sibling support file); an
/// agent/command delete removes only the `.md`; other assets are untouched; a
/// missing target is a safe no-op Ok.
#[test]
fn delete_removes_skill_dir_vs_single_file() {
    let dir = scratch_dir();
    let root = dir.path();

    // A skill directory with SKILL.md AND a sibling support file.
    write(
        root,
        ".claude/skills/code-review/SKILL.md",
        b"---\nname: code-review\ndescription: d\n---\n\nbody\n",
    );
    write(
        root,
        ".claude/skills/code-review/reference.md",
        b"# supporting material\n",
    );
    write(root, ".claude/skills/code-review/scripts/run.py", b"print('hi')\n");
    // A second skill that must be left alone.
    write(
        root,
        ".claude/skills/keep-me/SKILL.md",
        b"---\nname: keep-me\ndescription: d\n---\n\nbody\n",
    );
    write(
        root,
        ".claude/agents/test-runner.md",
        b"---\nname: test-runner\ndescription: d\n---\n\nbody\n",
    );
    write(root, ".claude/commands/changelog.md", b"body\n");

    // Skill delete -> the WHOLE `<name>/` dir is gone (incl. reference.md + scripts).
    let inv = delete_agent_asset(root, AgentAssetKind::Skill, "code-review").unwrap();
    assert!(
        !root.join(".claude/skills/code-review").exists(),
        "the whole skill directory is removed, not just SKILL.md"
    );
    assert!(inv.assets.iter().all(|a| a.name != "code-review"));
    // Every OTHER asset is untouched.
    assert!(root.join(".claude/skills/keep-me/SKILL.md").is_file());
    assert!(root.join(".claude/agents/test-runner.md").is_file());
    assert!(root.join(".claude/commands/changelog.md").is_file());

    // Agent delete -> only the `.md`; the `agents/` dir survives.
    delete_agent_asset(root, AgentAssetKind::Agent, "test-runner").unwrap();
    assert!(!root.join(".claude/agents/test-runner.md").exists());
    assert!(root.join(".claude/agents").is_dir());

    // Command delete -> only the `.md`.
    delete_agent_asset(root, AgentAssetKind::Command, "changelog").unwrap();
    assert!(!root.join(".claude/commands/changelog.md").exists());

    // A missing target is a safe no-op Ok; inventory now holds just keep-me.
    let after = delete_agent_asset(root, AgentAssetKind::Skill, "never-existed").unwrap();
    assert_eq!(after.assets.len(), 1);
    assert_eq!(after.assets[0].name, "keep-me");
    // The no-op delete created/removed nothing else.
    assert!(root.join(".claude/skills/keep-me/SKILL.md").is_file());
}

/// §9/§10 area 5 — validation + name safety as it lands through the fs surface.
/// A missing required field scans `valid:false` with the right issue; a
/// frontmatter/file name mismatch is flagged (Warning); Windows-reserved and
/// separator/`..` names are rejected by `save` with `InvalidName`, writing
/// NOTHING.
#[test]
fn validation_and_name_safety_through_fs() {
    let dir = scratch_dir();
    let root = dir.path();

    // Agent missing the required `description` -> writes, but scans invalid.
    save_agent_asset(
        root,
        input(AgentAssetKind::Agent, "incomplete", &[("name", "incomplete")], "\nbody\n"),
    )
    .unwrap();
    let inv = scan_agent_assets(root).unwrap();
    let incomplete = inv.assets.iter().find(|a| a.name == "incomplete").unwrap();
    assert!(!incomplete.validation.valid, "missing required field -> invalid");
    assert!(
        incomplete.validation.issues.iter().any(|i| i.severity
            == IssueSeverity::Error
            && i.message.contains("description")),
        "the issue names the missing required field"
    );

    // A frontmatter `name` that differs from the file name -> Warning (still valid).
    save_agent_asset(
        root,
        input(
            AgentAssetKind::Agent,
            "mismatch",
            &[("name", "something-else"), ("description", "d")],
            "\nbody\n",
        ),
    )
    .unwrap();
    let mism = read_agent_asset(root, AgentAssetKind::Agent, "mismatch").unwrap();
    assert!(mism.validation.valid, "name mismatch is only a Warning");
    assert!(
        mism.validation.issues.iter().any(|i| i.severity == IssueSeverity::Warning
            && i.message.contains("differs from the file name")),
        "the name-mismatch Warning is present"
    );

    // Windows reserved name + separator/`..` names are rejected with InvalidName,
    // writing nothing.
    for bad in ["CON", "nul", "a/b", "a\\b", "..", "a:b", "-x", ""] {
        let err = save_agent_asset(root, input(AgentAssetKind::Command, bad, &[], "b"))
            .unwrap_err();
        assert!(
            matches!(err, AppError::InvalidName(_)),
            "save({bad:?}) must be InvalidName, got {err:?}"
        );
    }
    // No commands dir was created by any rejected save (nothing written).
    assert!(
        !root.join(".claude/commands").exists(),
        "rejected saves write nothing"
    );
    // read + delete reject the same unsafe names before any fs touch.
    for bad in ["CON", "../x", "a/b"] {
        assert!(matches!(
            read_agent_asset(root, AgentAssetKind::Agent, bad),
            Err(AppError::InvalidName(_))
        ));
        assert!(matches!(
            delete_agent_asset(root, AgentAssetKind::Agent, bad),
            Err(AppError::InvalidName(_))
        ));
    }
}
