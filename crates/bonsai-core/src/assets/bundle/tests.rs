//! Read-path tests for the agent-asset bundle (§11).

use tempfile::TempDir;

use crate::error::AppError;

use super::test_support::write;
use super::validate::validate;
use super::*;

// §11 row 1 — empty / absent `.claude/` -> empty inventory (not an error).
#[test]
fn scan_empty_returns_no_assets() {
    let tmp = TempDir::new().unwrap();
    let inv = scan_agent_assets(tmp.path()).unwrap();
    assert!(inv.assets.is_empty());
    // A `.claude/` with no agent-asset sub-dirs is still empty.
    std::fs::create_dir_all(tmp.path().join(".claude")).unwrap();
    assert!(scan_agent_assets(tmp.path()).unwrap().assets.is_empty());
}

// §11 row 2 — scan all kinds, sorted + filtered.
#[test]
fn scan_all_kinds_sorted_and_filtered() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    write(
        root,
        ".claude/skills/code-review/SKILL.md",
        b"---\nname: code-review\ndescription: Reviews code\n---\n\n# Code review\n",
    );
    write(
        root,
        ".claude/agents/test-runner.md",
        b"---\nname: test-runner\ndescription: Runs tests\ntools: Bash\nmodel: inherit\n---\n\nYou run tests.\n",
    );
    write(
        root,
        ".claude/commands/changelog.md",
        b"---\ndescription: Update changelog\nargument-hint: <version>\n---\n\nUpdate for $ARGUMENTS.\n",
    );
    // A skill dir WITHOUT SKILL.md is skipped.
    std::fs::create_dir_all(root.join(".claude/skills/empty-skill")).unwrap();
    // A stray non-.md file in commands is ignored.
    write(root, ".claude/commands/notes.txt", b"ignore me\n");

    let inv = scan_agent_assets(root).unwrap();
    assert_eq!(
        inv.assets.len(),
        3,
        "3 assets, empty-skill + notes.txt skipped"
    );

    // Sort order: skill < agent < command, then name.
    assert_eq!(inv.assets[0].kind, AgentAssetKind::Skill);
    assert_eq!(inv.assets[0].name, "code-review");
    assert_eq!(inv.assets[0].path, ".claude/skills/code-review/SKILL.md");
    assert_eq!(inv.assets[1].kind, AgentAssetKind::Agent);
    assert_eq!(inv.assets[1].name, "test-runner");
    assert_eq!(inv.assets[1].path, ".claude/agents/test-runner.md");
    assert_eq!(inv.assets[2].kind, AgentAssetKind::Command);
    assert_eq!(inv.assets[2].name, "changelog");
    assert_eq!(inv.assets[2].path, ".claude/commands/changelog.md");

    // Parsed frontmatter + body of the agent.
    let agent = &inv.assets[1];
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
    assert!(agent.validation.valid, "complete agent is valid");
    assert!(inv.assets.iter().all(|a| a.exists));
}

// §11 row 3 — parse: order + unknown keys preserved; no-fence + unterminated.
#[test]
fn parse_preserves_order_and_unknown_keys() {
    let raw = "---\nname: foo\ncolor: blue\ndescription: hi\n---\nbody line\n";
    let (fields, body, complex) = parse_frontmatter(raw);
    assert!(!complex);
    assert_eq!(
        fields
            .iter()
            .map(|f| (f.key.as_str(), f.value.as_str()))
            .collect::<Vec<_>>(),
        vec![("name", "foo"), ("color", "blue"), ("description", "hi")]
    );
    assert_eq!(body, "body line\n", "body is verbatim after the fence");

    // No fence at all (common for commands) -> body is the whole file.
    let (f2, b2, c2) = parse_frontmatter("Just a prompt body.\n");
    assert!(f2.is_empty() && !c2);
    assert_eq!(b2, "Just a prompt body.\n");

    // Opening fence with no closing `---` -> not frontmatter.
    let (f3, b3, c3) = parse_frontmatter("---\nname: foo\nno close here\n");
    assert!(f3.is_empty() && !c3);
    assert_eq!(b3, "---\nname: foo\nno close here\n");

    // A bare `key:` line yields an empty value; `key: value` keeps the value.
    let (f4, _, _) = parse_frontmatter("---\nmodel:\ntools: Read, Write\n---\n");
    assert_eq!(
        f4[0],
        FrontmatterField {
            key: "model".into(),
            value: String::new()
        }
    );
    assert_eq!(f4[1].value, "Read, Write");
}

// §11 row 4 — round-trip is a fixed point for flat frontmatter.
#[test]
fn round_trip_flat_frontmatter_is_fixed_point() {
    let raw = "---\nname: foo\ncolor: blue\ndescription: hi\n---\n\nBody text.\n";
    let (fields, body, complex) = parse_frontmatter(raw);
    assert!(!complex);
    let serialized = serialize_asset(&fields, &body);
    // The frontmatter block is byte-stable (canonical `key: value` lines).
    assert_eq!(serialized, raw);
    // Re-parse yields identical fields + body.
    let (fields2, body2, _) = parse_frontmatter(&serialized);
    assert_eq!(fields, fields2);
    assert_eq!(body, body2);

    // Empty frontmatter -> body only, no fence, one trailing newline.
    assert_eq!(serialize_asset(&[], "prompt"), "prompt\n");
    assert_eq!(serialize_asset(&[], "prompt\n"), "prompt\n");
    // A bare `key:` (empty value) serializes without a trailing space.
    let empty_val = vec![FrontmatterField {
        key: "model".into(),
        value: String::new(),
    }];
    assert_eq!(serialize_asset(&empty_val, "b\n"), "---\nmodel:\n---\nb\n");
}

// §11 row 5 — complex-frontmatter detection.
#[test]
fn complex_frontmatter_is_detected_and_errors() {
    // A sequence line under `tools:`.
    let raw = "---\ntools:\n  - Read\n  - Write\n---\nbody\n";
    let (_fields, _body, complex) = parse_frontmatter(raw);
    assert!(complex, "indented sequence items are complex");

    // A top-level `- item` line.
    assert!(parse_frontmatter("---\n- item\n---\n").2);
    // A block-scalar indicator value.
    assert!(parse_frontmatter("---\ndescription: |\n  multi\n---\n").2);
    // But a literal pipe inside a value is NOT a block scalar.
    assert!(!parse_frontmatter("---\ncmd: a | b\n---\n").2);

    // Validation surfaces the complex Error -> invalid.
    let v = validate(AgentAssetKind::Agent, "x", &[], "body", true);
    assert!(!v.valid);
    assert!(v
        .issues
        .iter()
        .any(|i| i.severity == IssueSeverity::Error && i.message.contains("multi-line YAML")));
}

// §11 row 5 — required/recommended/lowercase-hyphen/name-mismatch validation.
#[test]
fn validate_required_and_warning_rules() {
    // Agent missing `description` -> Error, invalid.
    let fields = vec![FrontmatterField {
        key: "name".into(),
        value: "test-runner".into(),
    }];
    let v = validate(AgentAssetKind::Agent, "test-runner", &fields, "body", false);
    assert!(!v.valid);
    assert!(v.issues.iter().any(|i| i.severity == IssueSeverity::Error
        && i.message
            .contains("requires frontmatter field 'description'")));

    // Agent with both required -> valid.
    let full = vec![
        FrontmatterField {
            key: "name".into(),
            value: "test-runner".into(),
        },
        FrontmatterField {
            key: "description".into(),
            value: "runs".into(),
        },
    ];
    assert!(validate(AgentAssetKind::Agent, "test-runner", &full, "b", false).valid);

    // Skill missing description -> valid (recommended, not required).
    let sv = validate(AgentAssetKind::Skill, "code-review", &[], "body", false);
    assert!(sv.valid);

    // Command with no frontmatter + body -> valid.
    assert!(validate(AgentAssetKind::Command, "changelog", &[], "run it", false).valid);

    // `name: Foo_Bar` -> lowercase-hyphen Warning, still valid.
    let warn = validate(AgentAssetKind::Command, "Foo_Bar", &[], "b", false);
    assert!(warn.valid);
    assert!(warn
        .issues
        .iter()
        .any(|i| i.severity == IssueSeverity::Warning && i.message.contains("lowercase")));

    // Frontmatter `name` differing from the on-disk name -> Warning.
    let mism = vec![
        FrontmatterField {
            key: "name".into(),
            value: "other".into(),
        },
        FrontmatterField {
            key: "description".into(),
            value: "d".into(),
        },
    ];
    let mv = validate(AgentAssetKind::Agent, "test-runner", &mism, "b", false);
    assert!(mv.valid, "mismatch is only a Warning");
    assert!(mv
        .issues
        .iter()
        .any(|i| i.message.contains("differs from the file name")));

    // Empty body for a command -> Warning.
    let eb = validate(AgentAssetKind::Command, "changelog", &[], "   \n", false);
    assert!(eb
        .issues
        .iter()
        .any(|i| i.message.contains("body is empty")));
}

// §11 row 4.4 / row 11 — name safety.
#[test]
fn validate_asset_name_rejects_unsafe() {
    for bad in [
        "", "   ", ".", "..", "-x", "a/b", "a\\b", "a:b", "../x", "a\tb", "café",
    ] {
        assert!(
            matches!(validate_asset_name(bad), Err(AppError::InvalidName(_))),
            "name {bad:?} should be InvalidName"
        );
    }
    for good in ["code-review", "test-runner", "foo.bar", "a_b", "x1"] {
        validate_asset_name(good).unwrap_or_else(|e| panic!("must accept {good:?}: {e:?}"));
    }
    // The name check fires before any path is built, so the belt-and-suspenders
    // `validate_rel_path` guard is never reached for `..` via read_agent_asset.
    let tmp = TempDir::new().unwrap();
    assert!(matches!(
        read_agent_asset(tmp.path(), AgentAssetKind::Agent, ".."),
        Err(AppError::InvalidName(_))
    ));
    assert!(matches!(
        read_agent_asset(tmp.path(), AgentAssetKind::Agent, "a/b"),
        Err(AppError::InvalidName(_))
    ));
}

// Windows reserved device names + trailing dot/space are rejected; normal
// names still pass.
#[test]
fn validate_asset_name_rejects_windows_reserved() {
    for bad in [
        "CON", "con", "nul.md", "COM1", "lpt9", "PRN", "aux.txt", "foo.", "foo ", "bar.",
    ] {
        assert!(
            matches!(validate_asset_name(bad), Err(AppError::InvalidName(_))),
            "name {bad:?} should be InvalidName"
        );
    }
    for good in ["my-skill", "code.review", "com", "com10", "lpt", "console"] {
        validate_asset_name(good).unwrap_or_else(|e| panic!("must accept {good:?}: {e:?}"));
    }
}

// read_agent_asset: existing parsed; missing -> exists:false shell.
#[test]
fn read_agent_asset_existing_and_missing() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    write(
        root,
        ".claude/agents/test-runner.md",
        b"---\nname: test-runner\ndescription: Runs tests\n---\n\nYou run tests.\n",
    );
    let a = read_agent_asset(root, AgentAssetKind::Agent, "test-runner").unwrap();
    assert!(a.exists && a.validation.valid);
    assert_eq!(a.name, "test-runner");
    assert_eq!(a.frontmatter[0].value, "test-runner");
    assert_eq!(a.body, "\nYou run tests.\n");

    // Missing -> exists:false shell with a "file does not exist" Error.
    let m = read_agent_asset(root, AgentAssetKind::Skill, "nope").unwrap();
    assert!(!m.exists);
    assert!(m.frontmatter.is_empty() && m.body.is_empty());
    assert!(!m.validation.valid);
    assert!(m
        .validation
        .issues
        .iter()
        .any(|i| i.message.contains("does not exist")));
}

// §11 row 6 — wire shapes: camelCase keys + bare-string enums.
#[test]
fn wire_shapes_are_camel_case() {
    let tmp = TempDir::new().unwrap();
    write(
        tmp.path(),
        ".claude/agents/broken.md",
        b"---\nname: broken\n---\n\nno description here\n",
    );
    let inv = scan_agent_assets(tmp.path()).unwrap();
    let v = serde_json::to_value(&inv).unwrap();

    let asset = &v["assets"][0];
    // Bare-string kind.
    assert_eq!(asset["kind"], "agent");
    assert!(asset.get("name").is_some());
    assert!(asset.get("path").is_some());
    assert!(asset.get("exists").is_some());
    assert!(asset.get("frontmatter").is_some());
    assert!(asset.get("body").is_some());
    assert_eq!(asset["complex"], false);

    let validation = &asset["validation"];
    assert_eq!(validation["valid"], false);
    let issue = &validation["issues"][0];
    // Bare-string severity.
    assert_eq!(issue["severity"], "error");
    assert!(issue.get("message").is_some());

    // FrontmatterField camelCase (key/value are already lowercase).
    let field = &asset["frontmatter"][0];
    assert_eq!(field["key"], "name");
    assert_eq!(field["value"], "broken");

    // Skill/command kinds also serialize bare.
    assert_eq!(
        serde_json::to_value(AgentAssetKind::Skill).unwrap(),
        "skill"
    );
    assert_eq!(
        serde_json::to_value(AgentAssetKind::Command).unwrap(),
        "command"
    );
    assert_eq!(
        serde_json::to_value(IssueSeverity::Warning).unwrap(),
        "warning"
    );
}
