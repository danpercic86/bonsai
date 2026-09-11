//! Unit tests for the PURE halves of [`super`] — the MCP-only write
//! preconditions (audit 2026-09-11). No repo, no git2: each decision function
//! takes already-fetched data, which is why it is split that way. The
//! end-to-end wiring (a real repo, a real `tools/call`) is covered by
//! `tests/mcp_stdio_4.rs`.

use super::*;
use bonsai_core::git::status::{FileStatus, StatusEntry};

fn entry(path: &str, status: FileStatus) -> StatusEntry {
    StatusEntry {
        path: path.to_string(),
        orig_path: None,
        status,
    }
}

fn renamed(path: &str, orig: &str) -> StatusEntry {
    StatusEntry {
        path: path.to_string(),
        orig_path: Some(orig.to_string()),
        status: FileStatus::Renamed,
    }
}

/// A snapshot with one entry in each list plus a staged rename.
fn snapshot() -> StatusSnapshot {
    StatusSnapshot {
        staged: vec![
            entry("staged.txt", FileStatus::Modified),
            renamed("new/name.txt", "old/name.txt"),
        ],
        unstaged: vec![entry("edited.txt", FileStatus::Modified)],
        untracked: vec![entry("fresh.txt", FileStatus::Untracked)],
        conflicted: vec![entry("clash.txt", FileStatus::Conflicted)],
    }
}

// -------------------------------------------------- stage: status membership

/// Every list contributes: a path from staged / unstaged / untracked /
/// conflicted is accepted, and so is a whole batch of them.
#[test]
fn paths_in_any_status_list_are_accepted() {
    let snap = snapshot();
    for p in ["staged.txt", "edited.txt", "fresh.txt", "clash.txt"] {
        ensure_paths_in_snapshot(&snap, &[p.to_string()])
            .unwrap_or_else(|e| panic!("{p} must be accepted: {e:?}"));
    }
    let batch = vec![
        "staged.txt".to_string(),
        "edited.txt".to_string(),
        "fresh.txt".to_string(),
        "clash.txt".to_string(),
    ];
    assert!(ensure_paths_in_snapshot(&snap, &batch).is_ok());
}

/// Both sides of a rename are stageable — the NEW `path` and the OLD
/// `orig_path` (staging the old side is how the deletion half is staged).
#[test]
fn both_sides_of_a_rename_are_accepted() {
    let snap = snapshot();
    assert!(ensure_paths_in_snapshot(&snap, &["new/name.txt".to_string()]).is_ok());
    assert!(ensure_paths_in_snapshot(&snap, &["old/name.txt".to_string()]).is_ok());
}

/// The point of the guard: a path status never offered (a gitignored `.env`,
/// an unchanged tracked file, a made-up path) is refused as `invalidName`, and
/// the message names the offending path and says nothing was staged.
#[test]
fn path_absent_from_status_is_refused_as_invalid_name() {
    let snap = snapshot();
    for p in [".env", "unchanged.txt", "nope/never.txt"] {
        match ensure_paths_in_snapshot(&snap, &[p.to_string()]) {
            Err(AppError::InvalidName(m)) => {
                assert!(m.contains(p), "message must name the path: {m}");
                assert!(m.contains("bonsai_get_status"), "must point at status: {m}");
                assert!(m.contains("Nothing was staged"), "must say so: {m}");
            }
            other => panic!("expected InvalidName for {p}, got {other:?}"),
        }
    }
}

/// One bad path refuses the WHOLE batch (all-or-nothing), and the refusal names
/// the bad path, not the good one.
#[test]
fn one_absent_path_refuses_the_whole_batch() {
    let snap = snapshot();
    let batch = vec!["staged.txt".to_string(), ".env".to_string()];
    match ensure_paths_in_snapshot(&snap, &batch) {
        Err(AppError::InvalidName(m)) => assert!(m.contains(".env"), "{m}"),
        other => panic!("expected InvalidName, got {other:?}"),
    }
}

/// Exact matching: a directory prefix of an untracked file is NOT a stageable
/// path (status recurses untracked dirs, so real rows are always files), and
/// case/separator variants are not silently accepted.
#[test]
fn near_miss_paths_are_refused() {
    let snap = snapshot();
    for p in ["fresh", "new", "new/", "New/Name.txt", "./fresh.txt"] {
        assert!(
            ensure_paths_in_snapshot(&snap, &[p.to_string()]).is_err(),
            "{p} must not pass as a status path"
        );
    }
}

/// An empty snapshot (clean repo) accepts nothing but an empty batch.
#[test]
fn empty_batch_is_ok_and_clean_repo_refuses_everything() {
    let clean = StatusSnapshot::default();
    assert!(ensure_paths_in_snapshot(&clean, &[]).is_ok());
    assert!(ensure_paths_in_snapshot(&clean, &["a.txt".to_string()]).is_err());
}

/// The conflicted subset is exactly the conflicted-list paths (either side of a
/// rename), and nothing else — it is what gets the extra marker gate, so a
/// false positive would make ordinary staging read the worktree for no reason
/// and a false negative would leave the bypass open.
#[test]
fn conflicted_subset_picks_only_conflicted_paths() {
    let mut snap = snapshot();
    snap.conflicted.push(renamed("moved.txt", "was.txt"));
    let paths: Vec<String> = [
        "staged.txt",
        "clash.txt",
        "fresh.txt",
        "moved.txt",
        "was.txt",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    assert_eq!(
        conflicted_subset(&snap, &paths),
        vec!["clash.txt", "moved.txt", "was.txt"]
    );
    assert!(
        conflicted_subset(&StatusSnapshot::default(), &paths).is_empty(),
        "no conflicts ⇒ no extra gate"
    );
}

// -------------------------------------------------------- conflict markers

/// Each of the three marker lines is caught, at the start of the file or in the
/// middle, and the refusal is `unresolvedConflicts` naming the path.
#[test]
fn marker_text_is_refused_as_unresolved_conflicts() {
    let bodies = [
        "<<<<<<< HEAD\nours\n=======\ntheirs\n>>>>>>> topic\n",
        "clean start\n=======\nmore\n",
        "a\nb\n>>>>>>> topic\n",
    ];
    for body in bodies {
        match ensure_no_conflict_markers("a.txt", body) {
            Err(AppError::UnresolvedConflicts(m)) => {
                assert!(m.contains("a.txt"), "must name the path: {m}");
                assert!(m.contains("conflict markers"), "{m}");
            }
            other => panic!("expected UnresolvedConflicts, got {other:?}"),
        }
    }
}

/// Clean merged text passes — including text that merely MENTIONS markers
/// mid-line (the predicate is line-prefix based, matching the frontend's
/// `/^(<{7}|={7}|>{7})/`), and an empty file.
#[test]
fn clean_text_passes() {
    for body in [
        "merged line\nsecond\n",
        "",
        "  <<<<<<< indented is not a marker line\n",
        "text with ======= inside the line\n",
    ] {
        ensure_no_conflict_markers("a.txt", body)
            .unwrap_or_else(|e| panic!("{body:?} must pass: {e:?}"));
    }
}

// ------------------------------------------------------------- commit hooks

/// No commit hooks ⇒ no refusal (the common case; behaviour unchanged).
#[test]
fn no_hooks_is_allowed() {
    assert!(hooks_refusal(&[]).is_ok());
}

/// Hooks present ⇒ refused, naming every hook and BOTH remedies, so the model
/// can relay an actionable message instead of retrying blindly. The kind is
/// `hooksNotPermitted`, NOT the untyped `other` (review 2026-09-11): the caller
/// branches on kinds, and a refusal that looks like a generic failure invites a
/// blind retry loop.
#[test]
fn hooks_present_refuses_and_names_them_and_the_remedies() {
    match hooks_refusal(&["pre-commit", "commit-msg"]) {
        Err(e @ AppError::HooksNotPermitted(_)) => {
            let m = e.to_string();
            assert_eq!(
                serde_json::to_value(&e)
                    .ok()
                    .and_then(|v| v.get("kind").and_then(|k| k.as_str().map(str::to_string))),
                Some("hooksNotPermitted".to_string()),
                "the wire `kind` the model branches on"
            );
            assert!(m.contains("pre-commit"), "{m}");
            assert!(m.contains("commit-msg"), "{m}");
            assert!(m.contains("--allow-hooks"), "must name the consent flag: {m}");
            assert!(m.contains("bonsai.runHooks"), "must name the opt-out: {m}");
            assert!(m.contains("nothing was committed"), "{m}");
        }
        other => panic!("expected HooksNotPermitted, got {other:?}"),
    }
}

/// The MERGE-scoped refusal (review 2026-09-11 MUST-FIX) says what a merge
/// refusal actually means: nothing merged, committed OR stashed. It must never
/// speak of "the commit", because a refused merge produced none.
#[test]
fn merge_hooks_refusal_describes_the_merge_and_leaves_nothing_behind() {
    assert!(merge_hooks_refusal(&[]).is_ok(), "no hook ⇒ no refusal");
    match merge_hooks_refusal(&["commit-msg"]) {
        Err(AppError::HooksNotPermitted(m)) => {
            assert!(m.contains("commit-msg"), "{m}");
            assert!(
                m.contains("the merge was refused"),
                "must name the operation refused: {m}"
            );
            assert!(
                m.contains("nothing was merged, committed or stashed"),
                "must state that NOTHING happened: {m}"
            );
            assert!(m.contains("--allow-hooks"), "must name the consent flag: {m}");
            assert!(m.contains("bonsai.runHooks"), "must name the opt-out: {m}");
        }
        other => panic!("expected HooksNotPermitted, got {other:?}"),
    }
}

/// The gate handed to core is `Gate` only where disclosure is needed; a server
/// that may run hooks gets plain `Run`, so its merges are byte-identical to
/// before this guard existed.
#[test]
fn merge_hooks_gate_is_only_installed_when_disclosure_is_needed() {
    use bonsai_core::git::merge::MergeHookGate;
    assert!(matches!(
        merge_hooks_gate(true),
        MergeHookGate::Gate(_)
    ));
    assert!(matches!(merge_hooks_gate(false), MergeHookGate::Run));
}
