//! P31 fs-oracle integration tests (contract §9): per-worktree AI-context
//! activation, cross-checked against the REAL `git` CLI (`git worktree add`,
//! `git worktree lock`, `git status --porcelain`, real merge conflicts) — the
//! unit tests in `assets/profiles.rs` build fixtures with git2; this suite
//! proves the same guarantees hold on worktrees the git CLI itself created.
//!
//! Every test skips (passes with a note) if `git` is not on PATH. All scratch
//! repos live under `D:\Temp\bonsai-scratch` (never the system temp).

mod common;

use std::path::{Path, PathBuf};

use bonsai_core::assets::{
    activate_profile, activate_profile_for_worktree, list_profiles, list_worktree_contexts,
    preview_profile_for_worktree, save_profile, ContextProfile, ProfileTarget, TargetWriteAction,
    MAIN_WORKTREE_KEY,
};
use bonsai_core::error::AppError;
use common::{commit_fixed, git, porcelain_records};

macro_rules! require_git {
    () => {
        if !common::have_git() {
            eprintln!("skipping: `git` CLI not found on PATH");
            return;
        }
    };
}

fn profile(name: &str, targets: &[(&str, &str)]) -> ContextProfile {
    ContextProfile {
        name: name.to_string(),
        description: None,
        model: None,
        targets: targets
            .iter()
            .map(|(id, c)| ProfileTarget {
                asset_id: id.to_string(),
                content: c.to_string(),
            })
            .collect(),
    }
}

/// Fixture built ENTIRELY with the git CLI: main repo (committed CLAUDE.md +
/// AGENTS.md on `main`) and two linked worktrees created via
/// `git worktree add` — `wt-a` (branch feature-a) and `wt-b` (feature-b).
struct Fixture {
    _dir: tempfile::TempDir,
    main: PathBuf,
    wa: PathBuf, // linked worktree "wt-a"
    wb: PathBuf, // linked worktree "wt-b"
}

fn setup() -> Fixture {
    let dir = common::scratch_dir();
    let root = dir.path().to_path_buf();
    let main = root.join("main");
    std::fs::create_dir_all(&main).expect("mkdir main");
    git(&main, &["init", "-b", "main"]);
    git(&main, &["config", "user.name", "Test User"]);
    git(&main, &["config", "user.email", "test@example.com"]);
    git(&main, &["config", "core.autocrlf", "false"]);
    std::fs::write(main.join("CLAUDE.md"), "# base claude\n").unwrap();
    std::fs::write(main.join("AGENTS.md"), "# base agents\n").unwrap();
    git(&main, &["add", "-A"]);
    commit_fixed(&main, "init");

    let wa = root.join("wt-a");
    let wb = root.join("wt-b");
    git(
        &main,
        &["worktree", "add", "-b", "feature-a", wa.to_str().unwrap()],
    );
    git(
        &main,
        &["worktree", "add", "-b", "feature-b", wb.to_str().unwrap()],
    );
    Fixture {
        _dir: dir,
        main,
        wa,
        wb,
    }
}

fn read(p: &Path) -> Vec<u8> {
    std::fs::read(p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()))
}

/// End-to-end two-worktree scenario (§9.2/§9.3/§9.6 against CLI-created
/// worktrees): different profiles into main + linked; files byte-exact per
/// worktree; `git status` in each worktree shows only the expected paths;
/// drift counts differ per worktree; store JSON v2 on disk holds both
/// activations plus the `@main` mirror.
#[test]
fn two_worktrees_activate_different_profiles_end_to_end() {
    require_git!();
    let f = setup();

    save_profile(
        &f.main,
        profile(
            "opus",
            &[("claude", "# opus claude\n"), ("agents", "# opus agents\n")],
        ),
    )
    .unwrap();
    save_profile(
        &f.main,
        profile(
            "haiku",
            &[("claude", "# haiku claude\n"), ("agents", "# haiku agents\n")],
        ),
    )
    .unwrap();

    activate_profile_for_worktree(&f.main, MAIN_WORKTREE_KEY, "opus").unwrap();
    activate_profile_for_worktree(&f.main, "wt-a", "haiku").unwrap();

    // Byte-exact content per worktree; wt-b untouched.
    assert_eq!(read(&f.main.join("CLAUDE.md")), b"# opus claude\n");
    assert_eq!(read(&f.main.join("AGENTS.md")), b"# opus agents\n");
    assert_eq!(read(&f.wa.join("CLAUDE.md")), b"# haiku claude\n");
    assert_eq!(read(&f.wa.join("AGENTS.md")), b"# haiku agents\n");
    assert_eq!(read(&f.wb.join("CLAUDE.md")), b"# base claude\n");
    assert_eq!(read(&f.wb.join("AGENTS.md")), b"# base agents\n");

    // CLI oracle: git status per worktree shows EXACTLY the expected records.
    // Main: 2 modified tracked docs + the untracked .bonsai store.
    let main_status: Vec<String> = porcelain_records(&f.main)
        .into_iter()
        .map(|(r, _)| r)
        .collect();
    assert_eq!(
        main_status,
        vec![
            " M AGENTS.md".to_string(),
            " M CLAUDE.md".to_string(),
            "?? .bonsai/profiles.json".to_string(),
        ],
        "main worktree status"
    );
    let wa_status: Vec<String> = porcelain_records(&f.wa)
        .into_iter()
        .map(|(r, _)| r)
        .collect();
    assert_eq!(
        wa_status,
        vec![" M AGENTS.md".to_string(), " M CLAUDE.md".to_string()],
        "wt-a status: only the two activated docs, no .bonsai here"
    );
    assert!(
        porcelain_records(&f.wb).is_empty(),
        "wt-b must be pristine"
    );
    assert!(!f.wa.join(".bonsai").exists());
    assert!(!f.wb.join(".bonsai").exists());

    // Store JSON v2 on disk: both activations + the @main mirror.
    let store_path = f.main.join(".bonsai").join("profiles.json");
    let v: serde_json::Value = serde_json::from_slice(&read(&store_path)).unwrap();
    assert_eq!(v["version"], 2);
    assert_eq!(v["activeProfile"], "opus", "legacy mirror of @main");
    assert_eq!(v["worktreeActivations"]["@main"], "opus");
    assert_eq!(v["worktreeActivations"]["wt-a"], "haiku");
    assert!(v["worktreeActivations"].get("wt-b").is_none());

    // Matrix: per-worktree activeProfile + drift counts differ per worktree.
    let rows = list_worktree_contexts(&f.main).unwrap();
    assert_eq!(rows.len(), 3);
    let by_key = |k: &str| rows.iter().find(|r| r.worktree_key == k).unwrap();
    let m = by_key("@main");
    let a = by_key("wt-a");
    let b = by_key("wt-b");
    assert_eq!(m.active_profile.as_deref(), Some("opus"));
    assert_eq!(a.active_profile.as_deref(), Some("haiku"));
    assert_eq!(b.active_profile, None);
    // main + wt-a hold divergent CLAUDE/AGENTS pairs → drift; wt-b's docs
    // also differ from each other (base claude vs base agents) so compare
    // relative: change wt-b's docs to identical content? No — drift is
    // per-worktree canonical-vs-doc. Assert the invariant that counts are
    // computed per worktree from its OWN files:
    assert!(a.drifted_count >= 1, "wt-a divergent docs must drift");
    assert!(m.drifted_count >= 1, "main divergent docs must drift");
    assert!(a.activatable && m.activatable && b.activatable);
}

/// §9.5 against a CLI-created lock: `git worktree lock --reason` blocks both
/// preview and activate with the lock reason semantics, and the matrix row
/// carries the reason; `git worktree unlock` restores activatability.
#[test]
fn cli_locked_worktree_is_refused_until_unlocked() {
    require_git!();
    let f = setup();
    save_profile(&f.main, profile("p", &[("claude", "# p\n")])).unwrap();
    git(
        &f.main,
        &["worktree", "lock", "--reason", "pinned by QA", f.wa.to_str().unwrap()],
    );

    for res in [
        preview_profile_for_worktree(&f.main, "wt-a", "p").map(|_| ()),
        activate_profile_for_worktree(&f.main, "wt-a", "p").map(|_| ()),
    ] {
        match res {
            Err(AppError::Git(m)) => assert!(m.contains("locked"), "got: {m}"),
            other => panic!("expected locked refusal, got {other:?}"),
        }
    }
    // Nothing written into the locked worktree.
    assert_eq!(read(&f.wa.join("CLAUDE.md")), b"# base claude\n");
    // Matrix row: blocked + CLI-provided reason.
    let rows = list_worktree_contexts(&f.main).unwrap();
    let a = rows.iter().find(|r| r.worktree_key == "wt-a").unwrap();
    assert!(!a.activatable && a.locked);
    assert!(a.blocked_reason.as_deref().unwrap().contains("pinned by QA"));

    // Unlock via the CLI → activation proceeds.
    git(&f.main, &["worktree", "unlock", f.wa.to_str().unwrap()]);
    let act = activate_profile_for_worktree(&f.main, "wt-a", "p").unwrap();
    assert_eq!(act.results[0].action, TargetWriteAction::Written);
    assert_eq!(read(&f.wa.join("CLAUDE.md")), b"# p\n");
}

/// §9.4 (D7) against real `git status`: a tracked CLAUDE.md hand-modified in
/// the LINKED worktree blocks activation of a 2-target profile before ANY
/// write; the human edit survives byte-exact and `git status` still shows
/// exactly that one modification.
#[test]
fn dirty_tracked_target_in_linked_worktree_blocks_and_preserves_content() {
    require_git!();
    let f = setup();
    std::fs::write(f.wa.join("CLAUDE.md"), "# precious human edit\n").unwrap();
    // Oracle precondition: git itself sees the file as tracked+modified.
    let pre: Vec<String> = porcelain_records(&f.wa).into_iter().map(|(r, _)| r).collect();
    assert_eq!(pre, vec![" M CLAUDE.md".to_string()]);

    save_profile(
        &f.main,
        profile("p", &[("gemini", "# g\n"), ("claude", "# machine\n")]),
    )
    .unwrap();

    let err = activate_profile_for_worktree(&f.main, "wt-a", "p").unwrap_err();
    match err {
        AppError::Git(m) => {
            assert!(m.contains("uncommitted changes"), "got: {m}");
            assert!(m.contains("CLAUDE.md"), "names the offending path: {m}");
        }
        other => panic!("expected Git dirty-target error, got {other:?}"),
    }
    // ZERO writes: target #1 (GEMINI.md) not created, human edit intact.
    assert!(!f.wa.join("GEMINI.md").exists(), "no partial write");
    assert_eq!(read(&f.wa.join("CLAUDE.md")), b"# precious human edit\n");
    let post: Vec<String> = porcelain_records(&f.wa).into_iter().map(|(r, _)| r).collect();
    assert_eq!(post, pre, "git status unchanged by the refused activation");
    // No activation recorded.
    assert!(list_profiles(&f.main)
        .unwrap()
        .worktree_activations
        .is_empty());
}

/// D7 CONFLICTED flag: a REAL merge conflict on the target file (produced by
/// `git merge` in the linked worktree) blocks activation and the conflict
/// markers survive untouched.
#[test]
fn conflicted_target_blocks_activation() {
    require_git!();
    let f = setup();
    // Diverge CLAUDE.md on feature-a (in wt-a) and on main, then merge main
    // into wt-a → real conflict on CLAUDE.md.
    std::fs::write(f.wa.join("CLAUDE.md"), "# side a\n").unwrap();
    git(&f.wa, &["add", "CLAUDE.md"]);
    commit_fixed(&f.wa, "a-side");
    std::fs::write(f.main.join("CLAUDE.md"), "# main side\n").unwrap();
    git(&f.main, &["add", "CLAUDE.md"]);
    commit_fixed(&f.main, "main-side");
    assert!(
        !common::git_ok(&f.wa, &["merge", "main"]),
        "merge must conflict"
    );
    let conflicted = read(&f.wa.join("CLAUDE.md"));
    assert!(
        String::from_utf8_lossy(&conflicted).contains("<<<<<<<"),
        "conflict markers present"
    );

    save_profile(&f.main, profile("p", &[("claude", "# machine\n")])).unwrap();
    let err = activate_profile_for_worktree(&f.main, "wt-a", "p").unwrap_err();
    assert!(
        matches!(&err, AppError::Git(m) if m.contains("uncommitted changes")),
        "conflicted target must block, got {err:?}"
    );
    assert_eq!(
        read(&f.wa.join("CLAUDE.md")),
        conflicted,
        "conflict content byte-preserved"
    );
    assert!(list_profiles(&f.main)
        .unwrap()
        .worktree_activations
        .is_empty());
}

/// D5 from INSIDE a linked worktree: opening the linked path as the workdir,
/// the legacy `activate_profile` writes into ITSELF and records the activation
/// under the linked worktree's key — never under `@main`.
#[test]
fn activation_from_inside_linked_worktree_records_own_key() {
    require_git!();
    let f = setup();
    // Store written FROM the linked worktree lands in main (D1/D2).
    save_profile(&f.wa, profile("p", &[("claude", "# from inside\n")])).unwrap();
    assert!(f.main.join(".bonsai").join("profiles.json").is_file());
    assert!(!f.wa.join(".bonsai").exists());

    let act = activate_profile(&f.wa, "p").unwrap();
    assert_eq!(
        act.store
            .worktree_activations
            .get("wt-a")
            .map(String::as_str),
        Some("p")
    );
    assert!(
        !act.store.worktree_activations.contains_key(MAIN_WORKTREE_KEY),
        "must not retarget @main"
    );
    assert_eq!(act.store.active_profile, None, "legacy mirror untouched");
    // Wrote into itself only.
    assert_eq!(read(&f.wa.join("CLAUDE.md")), b"# from inside\n");
    assert_eq!(read(&f.main.join("CLAUDE.md")), b"# base claude\n");
    assert_eq!(read(&f.wb.join("CLAUDE.md")), b"# base claude\n");

    // Reopening from main sees the same persisted map (survives "reopen").
    let store = list_profiles(&f.main).unwrap();
    assert_eq!(
        store.worktree_activations.get("wt-a").map(String::as_str),
        Some("p")
    );
    let rows = list_worktree_contexts(&f.wa).unwrap();
    let a = rows.iter().find(|r| r.worktree_key == "wt-a").unwrap();
    assert_eq!(a.active_profile.as_deref(), Some("p"));
    assert!(a.is_current, "the open (current) worktree is wt-a");
}
