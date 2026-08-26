use super::*;
use std::sync::{Mutex, MutexGuard};

use crate::git::commit::create_commit;
use crate::git::stage::stage_paths;

/// Serialize env-mutating tests: `BONSAI_CLAUDE_BIN` is process-global, so
/// parallel tests that touch it would race (mirrors `ai::mod` / ai_branch_name).
fn env_lock() -> MutexGuard<'static, ()> {
    static LOCK: Mutex<()> = Mutex::new(());
    LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

/// git2-init a scratch repo with identity + autocrlf off (mirrors `ai_explain`).
fn init_scratch() -> tempfile::TempDir {
    let dir = crate::testutil::scratch_dir();
    let repo = git2::Repository::init(dir.path()).expect("init repo");
    let mut cfg = repo.config().expect("config");
    cfg.set_str("user.name", "Test User").expect("name");
    cfg.set_str("user.email", "test@example.com").expect("email");
    cfg.set_bool("core.autocrlf", false).expect("autocrlf");
    dir
}

fn v(paths: &[&str]) -> Vec<String> {
    paths.iter().map(|s| s.to_string()).collect()
}

/// Asserts the strict partition invariant: `groups ∪ unassigned == changed`,
/// each path exactly once (`changed` has no duplicates, so a sorted-equality
/// check proves BOTH coverage and disjointness).
fn assert_partition(parsed: &ParsedCompose, changed: &[String]) {
    let mut all: Vec<String> = parsed.groups.iter().flat_map(|g| g.files.clone()).collect();
    all.extend(parsed.unassigned.clone());
    all.sort();
    let mut want = changed.to_vec();
    want.sort();
    assert_eq!(all, want, "must be a disjoint cover of `changed`");
}

/// §8.1: junk/prose (no valid JSON) => `groups:[]`, `unassigned == changed`
/// (input order), a single explanatory note. Pure, no CLI.
#[test]
fn parse_unparseable_degrades_to_unassigned() {
    let changed = v(&["a.rs", "b.rs", "c.rs"]);
    let parsed = parse_compose_response("I can't group these, sorry - no JSON here.", &changed);
    assert!(parsed.groups.is_empty(), "no groups on unparseable output");
    assert_eq!(parsed.unassigned, changed, "all files unassigned, in input order");
    assert_eq!(parsed.notes.len(), 1, "exactly one explanatory note");
    assert!(
        parsed.notes[0].contains("could not be parsed"),
        "got {:?}",
        parsed.notes
    );
    assert_partition(&parsed, &changed);
}

/// §8.2: overlap first-wins (+note); unknown path dropped (+note); empty group
/// dropped (+note); uncovered path => `unassigned`; empty-message group KEPT
/// (OQ5); result is a disjoint cover of `changed`.
#[test]
fn parse_normalizes_partition() {
    let changed = v(&["a.rs", "b.rs", "c.rs", "d.rs", "e.rs"]);
    let raw = concat!(
        r#"{"groups":["#,
        r#"{"message":"feat a b","files":["a.rs","b.rs","ghost.rs"]},"#,
        r#"{"message":"dup b plus c","files":["b.rs","c.rs"]},"#,
        r#"{"message":"","files":["d.rs"]},"#,
        r#"{"message":"drop me","files":["ghost2.rs"]}"#,
        r#"]}"#,
    );
    let parsed = parse_compose_response(raw, &changed);

    // 3 groups survive; the all-unknown 4th group is dropped as empty.
    assert_eq!(parsed.groups.len(), 3, "groups: {:?}", parsed.groups);
    assert_eq!(parsed.groups[0].files, v(&["a.rs", "b.rs"]));
    assert_eq!(parsed.groups[0].message, "feat a b");
    // Overlap first-wins: b.rs stays in group 0; group 1 keeps only c.rs.
    assert_eq!(parsed.groups[1].files, v(&["c.rs"]));
    // OQ5: a valid-file group with an EMPTY message is KEPT.
    assert_eq!(parsed.groups[2].files, v(&["d.rs"]));
    assert_eq!(parsed.groups[2].message, "");
    // e.rs was never mentioned => unassigned.
    assert_eq!(parsed.unassigned, v(&["e.rs"]));
    assert_partition(&parsed, &changed);

    let notes = parsed.notes.join(" | ");
    assert!(notes.contains("dropped unknown path ghost.rs"), "{notes}");
    assert!(notes.contains("b.rs") && notes.contains("already assigned"), "{notes}");
    assert!(notes.contains("dropped an empty group"), "{notes}");
}

/// §8.3: more than `MAX_COMPOSE_GROUPS` raw groups => capped; the tail folds
/// into `unassigned` (in `changed` order) with a note; still a disjoint cover.
#[test]
fn parse_caps_groups() {
    let changed: Vec<String> = (0..(MAX_COMPOSE_GROUPS + 2))
        .map(|i| format!("f{i}.rs"))
        .collect();
    let raw_groups: Vec<String> = changed
        .iter()
        .map(|f| format!(r#"{{"message":"m {f}","files":["{f}"]}}"#))
        .collect();
    let raw = format!(r#"{{"groups":[{}]}}"#, raw_groups.join(","));
    let parsed = parse_compose_response(&raw, &changed);

    assert_eq!(parsed.groups.len(), MAX_COMPOSE_GROUPS, "capped at MAX");
    assert_eq!(
        parsed.unassigned,
        vec![
            format!("f{}.rs", MAX_COMPOSE_GROUPS),
            format!("f{}.rs", MAX_COMPOSE_GROUPS + 1),
        ],
        "tail folds into unassigned, in `changed` order"
    );
    assert!(
        parsed.notes.iter().any(|n| n.contains("group limit")),
        "cap note missing: {:?}",
        parsed.notes
    );
    assert_partition(&parsed, &changed);
}

/// §8.4: fenced ```json {…}```, leading/trailing prose, and a bare top-level
/// array all extract + parse to the same single group.
#[test]
fn parse_extracts_fenced_json() {
    let changed = v(&["a.rs"]);
    let want = v(&["a.rs"]);

    let fenced = "```json\n{\"groups\":[{\"message\":\"m\",\"files\":[\"a.rs\"]}]}\n```";
    let p1 = parse_compose_response(fenced, &changed);
    assert_eq!(p1.groups.len(), 1, "fenced json must parse");
    assert_eq!(p1.groups[0].files, want);

    let prose = "Sure, here is the grouping:\n{\"groups\":[{\"message\":\"m\",\"files\":[\"a.rs\"]}]}\nHope that helps!";
    let p2 = parse_compose_response(prose, &changed);
    assert_eq!(p2.groups.len(), 1, "prose-wrapped json must parse");
    assert_eq!(p2.groups[0].files, want);

    // Bare top-level array (no {groups} wrapper) — lenient deserialize path.
    let bare = "[{\"message\":\"m\",\"files\":[\"a.rs\"]}]";
    let p3 = parse_compose_response(bare, &changed);
    assert_eq!(p3.groups.len(), 1, "bare array must parse");
    assert_eq!(p3.groups[0].files, want);
}

/// §8.5: a clean worktree => `NothingToCommit` BEFORE any CLI call.
/// `BONSAI_CLAUDE_BIN` points at a nonexistent path: a regressed spawn would
/// return `AiUnavailable` (binary NotFound) — a DIFFERENT variant — so the
/// precise `NothingToCommit` assertion proves the pre-CLI bail.
#[test]
fn compose_clean_tree_is_nothing_to_commit() {
    let _g = env_lock();
    std::env::set_var(ai::CLAUDE_BIN_ENV, "D:/nonexistent/claude-must-not-spawn.exe");

    let dir = init_scratch();
    let p = dir.path();
    std::fs::write(p.join("base.txt"), "base\n").expect("write");
    stage_paths(p, &["base.txt".into()]).expect("stage");
    create_commit(p, "base", None, false).expect("commit");
    // Worktree is now clean => no change set.

    let err = compose_commits(p, None, RunOpts::default())
        .expect_err("clean tree must fail before any CLI call");
    std::env::remove_var(ai::CLAUDE_BIN_ENV);

    assert!(
        matches!(err, AppError::NothingToCommit),
        "expected NothingToCommit (pre-CLI), got {err:?} — a spawn would be AiUnavailable"
    );
}

/// §8.6 (grounding shape, pure): the payload lists the CHANGED FILES header,
/// the EXACT paths, and the per-file `===== FILE:` blocks, with the path list
/// BEFORE the diffs. The end-to-end stub-echo variant (payload reaches the
/// CLI stdin + the result carries `costUsd`) lives in `tests/ai_compose_cli.rs`
/// (a separate process, so the `BONSAI_CLAUDE_BIN` env cannot race the lib
/// unit tests).
#[test]
fn grounding_shape_lists_paths_and_file_blocks() {
    use crate::git::diff::{DiffLine, Hunk, LineKind};
    use crate::git::status::FileStatus;

    let files = vec![
        FileDiff {
            path: "src/app.rs".into(),
            orig_path: None,
            status: FileStatus::Modified,
            binary: false,
            too_large: false,
            hunks: vec![Hunk {
                old_start: 1,
                old_lines: 1,
                new_start: 1,
                new_lines: 2,
                lines: vec![DiffLine {
                    kind: LineKind::Add,
                    old_no: None,
                    new_no: None,
                    content: "    new_line();".into(),
                    no_newline: false,
                    spans: Vec::new(),
                }],
            }],
        },
        FileDiff {
            path: "README.md".into(),
            orig_path: None,
            status: FileStatus::Untracked,
            binary: false,
            too_large: false,
            hunks: vec![],
        },
    ];
    let changed: Vec<String> = files.iter().map(|f| f.path.clone()).collect();
    let g = render_grounding(&changed, &files);

    assert!(g.contains("WORKING CHANGES (HEAD vs working tree):"), "{g}");
    assert!(
        g.contains("CHANGED FILES (assign each to exactly one group; use these exact paths):"),
        "{g}"
    );
    assert!(g.contains("src/app.rs") && g.contains("README.md"), "{g}");
    assert!(g.contains("DIFFS:"), "{g}");
    assert!(g.contains("===== FILE: src/app.rs (modified) ====="), "{g}");
    assert!(g.contains("+    new_line();"), "{g}");
    let list_idx = g.find("CHANGED FILES").expect("path list");
    let diffs_idx = g.find("DIFFS:").expect("diffs section");
    assert!(list_idx < diffs_idx, "path list must precede the diffs: {g}");
}

/// §8.7: serde casing matches the TS types. `ComposeGroup` (`files`/`message`)
/// round-trips (Serialize + Deserialize); `ComposeProposal`
/// (`groups`/`unassigned`/`notes`/`costUsd`, `None` => `null`). (`ComposePlan`
/// / `ComposeCommit` casing is covered in P54b, where those types are defined.)
#[test]
fn compose_group_wire_shape_is_camel_case() {
    let g = serde_json::to_value(ComposeGroup {
        files: v(&["src/a.rs", "src/b.rs"]),
        message: "feat: add a and b".to_string(),
    })
    .expect("json");
    assert_eq!(
        g,
        serde_json::json!({ "files": ["src/a.rs", "src/b.rs"], "message": "feat: add a and b" })
    );
    // Round-trips (Deserialize) — the edited plan re-enters as the same shape.
    let back: ComposeGroup = serde_json::from_value(g).expect("round-trip");
    assert_eq!(back.files, v(&["src/a.rs", "src/b.rs"]));
    assert_eq!(back.message, "feat: add a and b");

    let p = serde_json::to_value(ComposeProposal {
        groups: vec![ComposeGroup {
            files: v(&["x"]),
            message: "m".to_string(),
        }],
        unassigned: v(&["y"]),
        notes: v(&["n"]),
        cost_usd: Some(0.012),
    })
    .expect("json");
    assert_eq!(
        p,
        serde_json::json!({
            "groups": [{ "files": ["x"], "message": "m" }],
            "unassigned": ["y"],
            "notes": ["n"],
            "costUsd": 0.012
        })
    );

    let p_none = serde_json::to_value(ComposeProposal {
        groups: vec![],
        unassigned: vec![],
        notes: vec![],
        cost_usd: None,
    })
    .expect("json");
    assert_eq!(
        p_none,
        serde_json::json!({ "groups": [], "unassigned": [], "notes": [], "costUsd": null })
    );
}

/// §8.8: the prompt consts MUST be single-line (Windows argv constraint), and
/// `compose_prompt` MUST collapse multi-line user guidance to a single line.
#[test]
fn prompts_are_single_line() {
    for s in [COMPOSE_SYSTEM_PROMPT, COMPOSE_PROMPT] {
        assert!(!s.contains('\n'), "prompt must be single-line: {s:?}");
        assert!(!s.contains('\r'), "prompt must be single-line: {s:?}");
    }
    // None => the bare prompt; blank => no suffix.
    assert_eq!(compose_prompt(None), COMPOSE_PROMPT);
    assert_eq!(compose_prompt(Some("   ")), COMPOSE_PROMPT);
    // Multi-line free-text guidance is collapsed to one line.
    let p = compose_prompt(Some("keep tests separate\nand group docs\r\ntogether"));
    assert!(!p.contains('\n') && !p.contains('\r'), "guidance must be collapsed: {p:?}");
    assert!(
        p.contains("Extra guidance: keep tests separate and group docs together"),
        "got {p:?}"
    );
}
