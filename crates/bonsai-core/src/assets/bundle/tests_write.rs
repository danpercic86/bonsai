//! Write-path tests for the agent-asset bundle (P26b §11).

use tempfile::TempDir;

use crate::error::AppError;

use super::test_support::write;
use super::*;

// ---- P26b (write path) -------------------------------------------------

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

// §11 row 7 — save creates a new skill (dir + SKILL.md), agent, command with
// byte-exact content; parent dirs created; no `.bonsai-tmp` remnant; re-scan
// lists them valid.
#[test]
fn save_creates_new_assets() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    let skill = input(
        AgentAssetKind::Skill,
        "code-review",
        &[("name", "code-review"), ("description", "Reviews code")],
        "\n# Code review\n",
    );
    let inv = save_agent_asset(root, skill.clone()).unwrap();
    assert!(
        inv.assets
            .iter()
            .any(|a| a.kind == AgentAssetKind::Skill && a.name == "code-review"),
        "first save's returned inventory already lists the skill"
    );

    // Byte-exact on disk via serialize_asset; parent `<name>/` dir created.
    let skill_path = root.join(".claude/skills/code-review/SKILL.md");
    assert!(skill_path.is_file());
    assert_eq!(
        std::fs::read_to_string(&skill_path).unwrap(),
        serialize_asset(&skill.frontmatter, &skill.body)
    );
    assert!(
        !root
            .join(".claude/skills/code-review/SKILL.md.bonsai-tmp")
            .exists(),
        "no temp remnant"
    );

    save_agent_asset(
        root,
        input(
            AgentAssetKind::Agent,
            "test-runner",
            &[("name", "test-runner"), ("description", "Runs tests")],
            "\nYou run tests.\n",
        ),
    )
    .unwrap();
    save_agent_asset(
        root,
        input(
            AgentAssetKind::Command,
            "changelog",
            &[("description", "Update changelog")],
            "\nUpdate for $ARGUMENTS.\n",
        ),
    )
    .unwrap();
    assert!(root.join(".claude/agents/test-runner.md").is_file());
    assert!(root.join(".claude/commands/changelog.md").is_file());

    // A fresh scan round-trips all three as valid + existing.
    let final_inv = scan_agent_assets(root).unwrap();
    assert_eq!(final_inv.assets.len(), 3);
    assert!(final_inv
        .assets
        .iter()
        .all(|a| a.exists && a.validation.valid));
}

// §11 row 7 — save EDITS (overwrites) an existing asset atomically.
#[test]
fn save_edits_existing_asset_atomically() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    save_agent_asset(
        root,
        input(
            AgentAssetKind::Agent,
            "test-runner",
            &[("name", "test-runner"), ("description", "old")],
            "\nold body\n",
        ),
    )
    .unwrap();
    let inv = save_agent_asset(
        root,
        input(
            AgentAssetKind::Agent,
            "test-runner",
            &[("name", "test-runner"), ("description", "new")],
            "\nnew body\n",
        ),
    )
    .unwrap();
    // Overwritten in place, not duplicated.
    assert_eq!(inv.assets.len(), 1);
    let a = read_agent_asset(root, AgentAssetKind::Agent, "test-runner").unwrap();
    assert_eq!(
        a.frontmatter
            .iter()
            .find(|f| f.key == "description")
            .unwrap()
            .value,
        "new"
    );
    assert_eq!(a.body, "\nnew body\n");
    assert!(!root
        .join(".claude/agents/test-runner.md.bonsai-tmp")
        .exists());
}

// SHOULD-FIX (data-loss fail-open): saving over an existing COMPLEX asset is
// refused with `Other` and leaves the file byte-unchanged; a flat overwrite
// and a fresh create are unaffected.
#[test]
fn save_refuses_overwriting_complex_asset() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    // An on-disk agent with multi-line (sequence) YAML the flat editor can't
    // round-trip. Confirm the scan flags it `complex`.
    let complex_bytes =
        b"---\nname: fancy\ndescription: has a list\ntools:\n  - Read\n  - Bash\n---\n\nbody\n";
    write(root, ".claude/agents/fancy.md", complex_bytes);
    let loaded = read_agent_asset(root, AgentAssetKind::Agent, "fancy").unwrap();
    assert!(loaded.complex, "sequence frontmatter must parse as complex");

    // Attempting to overwrite it from the (flat) editor is refused, writes
    // nothing, and the bytes on disk are untouched.
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
    assert!(matches!(err, AppError::Other(_)));
    assert_eq!(
        std::fs::read(root.join(".claude/agents/fancy.md")).unwrap(),
        complex_bytes.to_vec(),
        "the complex file must be byte-unchanged"
    );

    // Overwriting a FLAT existing asset still works.
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
            "\nnew\n",
        ),
    )
    .unwrap();
    let plain = read_agent_asset(root, AgentAssetKind::Agent, "plain").unwrap();
    assert_eq!(plain.body, "\nnew\n");

    // Creating a brand-new asset is unaffected.
    save_agent_asset(
        root,
        input(
            AgentAssetKind::Command,
            "changelog",
            &[("description", "d")],
            "\nrun\n",
        ),
    )
    .unwrap();
    assert!(root.join(".claude/commands/changelog.md").is_file());
}

// §11 row 8 — save preserves unknown/preserved frontmatter keys on edit.
#[test]
fn save_edit_preserves_unknown_keys() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    // Pre-drop an agent carrying an unknown `color: blue` key.
    write(
        root,
        ".claude/agents/test-runner.md",
        b"---\nname: test-runner\ndescription: old\ncolor: blue\n---\n\nbody\n",
    );
    // Load, edit `description`, carry every field (incl. `color`) through.
    let loaded = read_agent_asset(root, AgentAssetKind::Agent, "test-runner").unwrap();
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
            name: "test-runner".to_string(),
            frontmatter: fm,
            body: loaded.body.clone(),
        },
    )
    .unwrap();

    let re = read_agent_asset(root, AgentAssetKind::Agent, "test-runner").unwrap();
    assert_eq!(
        re.frontmatter
            .iter()
            .find(|f| f.key == "color")
            .unwrap()
            .value,
        "blue",
        "unknown key survives the round-trip"
    );
    assert_eq!(
        re.frontmatter
            .iter()
            .find(|f| f.key == "description")
            .unwrap()
            .value,
        "updated"
    );
}

// §11 row 9 — bad names reject (nothing written); a missing required field
// still WRITES and the inventory flags it invalid.
#[test]
fn save_rejects_bad_names_but_writes_missing_required() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    for bad in ["", "a/b", "a\\b", "..", "a:b", "-x"] {
        let err = save_agent_asset(root, input(AgentAssetKind::Agent, bad, &[], "b")).unwrap_err();
        assert!(
            matches!(err, AppError::InvalidName(_)),
            "name {bad:?} should be InvalidName"
        );
    }
    // Nothing written by the rejected saves.
    assert!(!root.join(".claude/agents").exists());

    // Missing the required `description` -> still writes; flagged invalid.
    let inv = save_agent_asset(
        root,
        input(
            AgentAssetKind::Agent,
            "incomplete",
            &[("name", "incomplete")],
            "\nbody\n",
        ),
    )
    .unwrap();
    assert!(root.join(".claude/agents/incomplete.md").is_file());
    let a = inv.assets.iter().find(|a| a.name == "incomplete").unwrap();
    assert!(!a.validation.valid);
    assert!(a
        .validation
        .issues
        .iter()
        .any(|i| i.severity == IssueSeverity::Error && i.message.contains("description")));
}

// §11 row 10 — delete removes the whole skill dir; agent/command remove just
// the file; others untouched; absent target is a no-op Ok.
#[test]
fn delete_removes_skill_dir_and_single_files() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    write(
        root,
        ".claude/skills/code-review/SKILL.md",
        b"---\nname: code-review\n---\n\nbody\n",
    );
    // A supporting file beside SKILL.md must go with the dir.
    write(
        root,
        ".claude/skills/code-review/helper.py",
        b"print('hi')\n",
    );
    write(
        root,
        ".claude/agents/test-runner.md",
        b"---\nname: test-runner\ndescription: d\n---\n\nbody\n",
    );
    write(root, ".claude/commands/changelog.md", b"body\n");

    // Skill delete -> the WHOLE `<name>/` dir is gone (incl. helper.py).
    let inv = delete_agent_asset(root, AgentAssetKind::Skill, "code-review").unwrap();
    assert!(!root.join(".claude/skills/code-review").exists());
    assert!(inv.assets.iter().all(|a| a.name != "code-review"));
    // Other assets untouched.
    assert!(root.join(".claude/agents/test-runner.md").is_file());

    // Agent delete -> just the file; the `agents/` dir remains.
    delete_agent_asset(root, AgentAssetKind::Agent, "test-runner").unwrap();
    assert!(!root.join(".claude/agents/test-runner.md").exists());
    assert!(root.join(".claude/agents").is_dir());

    // Command delete -> just the file; inventory now empty.
    let inv2 = delete_agent_asset(root, AgentAssetKind::Command, "changelog").unwrap();
    assert!(!root.join(".claude/commands/changelog.md").exists());
    assert!(inv2.assets.is_empty());

    // Deleting an absent asset is a no-op Ok.
    let inv3 = delete_agent_asset(root, AgentAssetKind::Skill, "gone").unwrap();
    assert!(inv3.assets.is_empty());
}

// §11 row 11 — path-escape defense: save + delete reject before any fs op.
#[test]
fn save_delete_reject_path_escapes() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    for bad in ["../x", "a/b", "a\\b", ".."] {
        assert!(
            matches!(
                save_agent_asset(root, input(AgentAssetKind::Skill, bad, &[], "b")),
                Err(AppError::InvalidName(_))
            ),
            "save({bad:?}) should reject"
        );
        assert!(
            matches!(
                delete_agent_asset(root, AgentAssetKind::Agent, bad),
                Err(AppError::InvalidName(_))
            ),
            "delete({bad:?}) should reject"
        );
    }
    // The name guard fires first, so nothing was created/removed.
    assert!(!root.join(".claude").exists());
}
