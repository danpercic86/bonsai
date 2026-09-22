//! P115 phase 2: the pure prune/migrate pass over the evidence shape, rules
//! (3)-(5), and the end-to-end run over a real `settings.json`. Rule (6) and
//! the phase-1b resolver live in `prune_forge_tests.rs`; the gate itself in
//! `prune_tests.rs`. Fixtures come from `prune_fixtures.rs`.

use super::fixtures::*;
use super::*;
use crate::settings::{RecentRepo, RepoForgeOverride};

/// Phase 1a + 1b + phase 2, as the setup call site chains them, in memory.
/// Asserts AC7's ⊆ property on every fixture it runs.
fn run_pass(s: &mut Settings, fake: &mut Fake) -> PruneReport {
    let facts = evidence_facts(s, fake);
    let acks_before = s.hooks_ack_repos.clone();
    let report = prune_stale_paths(s, &facts);
    assert_acks_subset(&acks_before, &s.hooks_ack_repos);
    report
}

// ----------------------------------------------------------- the evidence

#[test]
fn the_evidence_file_is_pruned_and_the_pin_is_migrated() {
    // AC1, AC2, AC3, AC6, AC7, AC12, AC21 over the 2026-09-22 shape.
    let mut s = evidence_settings();
    let before_len = s.repo_forge_overrides.len();
    let before_open = s.open_repos.clone();
    let before_active = s.active_repo.clone();
    let report = run_pass(&mut s, &mut evidence_probe());

    // AC1 + AC2: all 5 dead recents gone (including the forward-slash worktree
    // entry), the live ones untouched and in their original relative order.
    assert_eq!(report.recents_pruned, 5);
    let paths: Vec<&str> = s.recent_repos.iter().map(|r| r.path.as_str()).collect();
    assert_eq!(
        paths,
        vec![
            fx("Data/Repos/ham-digi-backend").as_str(),
            fx("Data/Repos/bonsai").as_str()
        ]
    );
    assert_eq!(s.recent_repos[0].last_opened, 1_700_000_900);
    assert_eq!(s.recent_repos[1].last_opened, 1_700_000_700);
    assert!(
        !s.recent_repos
            .iter()
            .any(|r| r.path == fx_slash("Repos/.worktrees/ham-digi-backend-hotfix")),
        "AC2: the forward-slash entry folds to the same key as its siblings"
    );

    // AC3 + AC6: the pin moved to the repo's new location, as the PERSISTED
    // string (never a `\\?\` canonical form); nothing deleted.
    assert_eq!(report.overrides_migrated, 1);
    assert_eq!(report.overrides_kept_dead, 0);
    assert_eq!(s.repo_forge_overrides.len(), before_len, "AC6");
    assert_eq!(
        s.repo_forge_overrides[0].repo_path,
        fx("Data/Repos/ham-digi-backend")
    );
    assert_eq!(s.repo_forge_overrides[0].account_id, PINNED_ACCOUNT);

    // AC7: the dead ack is gone, the live ones survive, none was added.
    assert_eq!(report.hooks_acks_pruned, 1);
    assert_eq!(
        s.hooks_ack_repos,
        vec![fx("Data/Repos/ham-digi-backend"), fx("Data/Repos/bonsai")]
    );

    // AC12: the session is read-only for this pass.
    assert_eq!(s.open_repos, before_open);
    assert_eq!(s.active_repo, before_active);

    assert!(report.changed());
    assert_eq!(report.unreachable_skipped, 0);
}

#[test]
fn a_second_pass_changes_nothing() {
    // AC10: idempotence. The fixture migrates cleanly, so the second report is
    // exactly `PruneReport::default()`.
    let mut s = evidence_settings();
    run_pass(&mut s, &mut evidence_probe());
    let after_first = s.clone();

    let second = run_pass(&mut s, &mut evidence_probe());
    assert_eq!(second, PruneReport::default());
    assert!(!second.changed());
    assert_eq!(s, after_first);
}

#[test]
fn a_confirmed_gone_session_is_still_never_mutated() {
    // AC12 proper: `open_repos` and `active_repo` name paths the pass has
    // positively proven gone, and still may not be touched — the frontend's
    // session persist self-heals them (§1.4) and a restore-open that fails
    // simply adds no tab.
    let mut s = session_gone_settings();
    let before_open = s.open_repos.clone();
    let before_active = s.active_repo.clone();
    let report = run_pass(&mut s, &mut evidence_probe());

    assert_eq!(report.recents_pruned, 1);
    assert_eq!(report.hooks_acks_pruned, 1);
    assert_eq!(s.open_repos, before_open, "AC12: byte-identical");
    assert_eq!(s.active_repo, before_active, "AC12: byte-identical");
}

// ------------------------------------------------------- migration rules §4

/// A dead override plus whatever live repos the caller wants to offer it, with
/// the `forge_accounts` record rule (6) reads the pinned host from.
fn migration_case(
    dead_override: &str,
    others: Vec<RepoForgeOverride>,
    live: &[String],
) -> Settings {
    let mut overrides = vec![RepoForgeOverride {
        repo_path: dead_override.to_string(),
        account_id: PINNED_ACCOUNT.to_string(),
    }];
    overrides.extend(others);
    Settings {
        recent_repos: live
            .iter()
            .map(|p| RecentRepo {
                path: p.clone(),
                last_opened: 1,
            })
            .collect(),
        repo_forge_overrides: overrides,
        forge_accounts: vec![account(PINNED_ACCOUNT, "github.com")],
        ..Settings::default()
    }
}

/// Phase 1 for a `migration_case`, with every live candidate's `origin`
/// resolving to the pinned account's host — so a declined migration below can
/// only be rules (3)-(5) declining it, never a missing host.
fn case_facts(s: &Settings, fake: &mut Fake) -> PathFacts {
    let mut facts = facts_for(s, fake);
    let live: Vec<String> = s
        .recent_repos
        .iter()
        .map(|r| r.path.clone())
        .chain(s.open_repos.iter().cloned())
        .collect();
    for path in live {
        if facts.state(&path) == PathState::Live {
            set_host(&mut facts, &path, "github.com");
        }
    }
    facts
}

fn run_case(s: &mut Settings, fake: &mut Fake) -> PruneReport {
    let facts = case_facts(s, fake);
    let acks_before = s.hooks_ack_repos.clone();
    let report = prune_stale_paths(s, &facts);
    assert_acks_subset(&acks_before, &s.hooks_ack_repos);
    report
}

#[test]
fn two_live_candidates_with_the_same_name_block_the_migration() {
    // AC4: ambiguous ⇒ the pin stays exactly where it is.
    let live = vec![fx("Data/Repos/backend"), fx("Work/backend")];
    let mut s = migration_case(&fx("Repos/backend"), vec![], &live);
    let mut fake = Fake::new(&[live[0].clone(), live[1].clone(), fx_root()]);
    let report = run_case(&mut s, &mut fake);

    assert_eq!(report.overrides_migrated, 0);
    assert_eq!(report.overrides_kept_dead, 1);
    assert!(
        !report.changed(),
        "an observation must not rewrite the file"
    );
    assert_eq!(s.repo_forge_overrides[0].repo_path, fx("Repos/backend"));
}

#[test]
fn a_live_repos_own_pin_is_never_overwritten() {
    // AC5 / rule (4).
    let live = vec![fx("Data/Repos/backend")];
    let mine = RepoForgeOverride {
        repo_path: live[0].clone(),
        account_id: "gitLab:gitlab.com:other".to_string(),
    };
    let mut s = migration_case(&fx("Repos/backend"), vec![mine], &live);
    let mut fake = Fake::new(&[live[0].clone(), fx_root()]);
    let report = run_case(&mut s, &mut fake);

    assert_eq!(report.overrides_migrated, 0);
    assert_eq!(report.overrides_kept_dead, 1);
    assert_eq!(s.repo_forge_overrides[0].repo_path, fx("Repos/backend"));
    assert_eq!(
        s.repo_forge_overrides[1].account_id, "gitLab:gitlab.com:other",
        "the live pin is untouched"
    );
}

#[test]
fn two_dead_overrides_sharing_a_basename_both_stay() {
    // Rule (5): which `backend` the live one is cannot be decided, so neither
    // moves — and neither is deleted.
    let live = vec![fx("Data/Repos/backend")];
    let twin = RepoForgeOverride {
        repo_path: fx("Archive/backend"),
        account_id: "gitHub:github.com:other".to_string(),
    };
    let mut s = migration_case(&fx("Repos/backend"), vec![twin], &live);
    let mut fake = Fake::new(&[live[0].clone(), fx_root()]);
    let report = run_case(&mut s, &mut fake);

    assert_eq!(report.overrides_migrated, 0);
    assert_eq!(report.overrides_kept_dead, 2);
    assert_eq!(s.repo_forge_overrides.len(), 2, "AC6: never deleted");
    assert_eq!(s.repo_forge_overrides[0].repo_path, fx("Repos/backend"));
    assert_eq!(s.repo_forge_overrides[1].repo_path, fx("Archive/backend"));
}

#[test]
fn no_candidate_keeps_the_dead_override_in_place() {
    // Rule (3), zero matches: the repo has not been reopened anywhere yet. The
    // pin is the only record of the user's choice, so it waits.
    let live = vec![fx("Data/Repos/bonsai")];
    let mut s = migration_case(&fx("Repos/ghost"), vec![], &live);
    let mut fake = Fake::new(&[live[0].clone(), fx_root()]);
    let report = run_case(&mut s, &mut fake);

    assert_eq!(report.overrides_migrated, 0);
    assert_eq!(report.overrides_kept_dead, 1);
    assert_eq!(s.repo_forge_overrides[0].repo_path, fx("Repos/ghost"));
}

#[test]
fn one_repo_in_both_recents_and_the_session_is_one_candidate() {
    // The evidence shape: the moved repo is open AND in recents. Counting it
    // twice would make rule (3) call the single real candidate ambiguous.
    let live = fx("Data/Repos/backend");
    let mut s = migration_case(&fx("Repos/backend"), vec![], std::slice::from_ref(&live));
    s.open_repos = vec![live.clone()];
    let mut fake = Fake::new(&[live.clone(), fx_root()]);
    let report = run_case(&mut s, &mut fake);

    assert_eq!(report.overrides_migrated, 1);
    assert_eq!(s.repo_forge_overrides[0].repo_path, live);
}

#[test]
fn a_session_only_repo_is_a_valid_migration_target() {
    // §1.4: `open_repos` is read as a candidate source and never mutated.
    let live = fx("Data/Repos/backend");
    let mut s = migration_case(&fx("Repos/backend"), vec![], &[]);
    s.open_repos = vec![live.clone()];
    s.active_repo = Some(live.clone());
    let mut fake = Fake::new(&[live.clone(), fx_root()]);
    let report = run_case(&mut s, &mut fake);

    assert_eq!(report.overrides_migrated, 1);
    assert_eq!(s.repo_forge_overrides[0].repo_path, live);
    assert_eq!(s.open_repos, vec![live.clone()], "AC12");
    assert_eq!(s.active_repo, Some(live));
}

// --------------------------------------------------------------- the gate

#[test]
fn an_unreachable_volume_mutates_nothing() {
    // AC8 end to end: the drive is unplugged, every probe fails (as NotFound,
    // the way Windows reports a missing drive letter). Nothing may move.
    let mut s = evidence_settings();
    let before = s.clone();
    let mut fake = Fake::new(&[]); // not even the root exists
    let report = run_pass(&mut s, &mut fake);

    assert_eq!(report.recents_pruned, 0);
    assert_eq!(report.hooks_acks_pruned, 0);
    assert_eq!(report.overrides_migrated, 0);
    assert_eq!(report.overrides_kept_dead, 0);
    assert!(report.unreachable_skipped >= 1);
    assert!(!report.changed());
    assert_eq!(s, before, "an offline volume changes nothing at all");
}

#[test]
fn a_permission_error_below_a_live_root_mutates_nothing() {
    // AC9 end to end.
    let mut s = evidence_settings();
    let before = s.clone();
    let mut fake = Fake::with_kind(&[fx_root()], std::io::ErrorKind::PermissionDenied);
    let report = run_pass(&mut s, &mut fake);

    assert!(!report.changed());
    assert_eq!(report.overrides_migrated, 0);
    assert_eq!(s, before);
}

#[test]
fn an_absent_state_is_treated_as_live() {
    // AC14: a path that was never probed (added between the two phases) is
    // never mutated.
    let mut s = evidence_settings();
    let before = s.clone();
    let acks_before = s.hooks_ack_repos.clone();
    let report = prune_stale_paths(&mut s, &PathFacts::default());
    assert_acks_subset(&acks_before, &s.hooks_ack_repos);
    assert_eq!(report, PruneReport::default());
    assert!(!report.changed());
    assert_eq!(s, before);
}

// ------------------------------------------------------------ file-level

/// `D:\Data\Temp` via TMP/TEMP; `tempfile` honors them.
fn temp_settings(s: &Settings) -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempfile::TempDir::new().expect("temp dir");
    let file = dir.path().join("settings.json");
    crate::settings::save_to(&file, s).expect("write fixture");
    (dir, file)
}

#[test]
fn the_pass_round_trips_through_a_real_settings_file() {
    // The setup call site, end to end: lock-free snapshot -> probes -> a single
    // `update_if` whose mutator is pure.
    let (_dir, file) = temp_settings(&evidence_settings());

    let snapshot = crate::settings::load_from(&file);
    let mut fake = evidence_probe();
    let facts = evidence_facts(&snapshot, &mut fake);
    assert!(facts
        .states
        .values()
        .any(|st| *st == PathState::ConfirmedGone));

    let mut report = PruneReport::default();
    let saved = crate::settings::update_if(&file, |s| {
        report = prune_stale_paths(s, &facts);
        report.changed()
    })
    .expect("save");
    assert!(report.changed());

    let reloaded = crate::settings::load_from(&file);
    assert_eq!(reloaded, saved);
    assert_eq!(reloaded.version, crate::settings::SETTINGS_VERSION, "AC15");
    assert_eq!(reloaded.recent_repos.len(), 2);
    assert_eq!(
        reloaded.repo_forge_overrides[0].repo_path,
        fx("Data/Repos/ham-digi-backend")
    );
    assert_eq!(reloaded.open_repos, evidence_settings().open_repos, "AC12");
    assert_acks_subset(
        &evidence_settings().hooks_ack_repos,
        &reloaded.hooks_ack_repos,
    );

    // AC15: the on-disk file still declares version 1.
    let text = std::fs::read_to_string(&file).expect("read back");
    let json: serde_json::Value = serde_json::from_str(&text).expect("parse");
    assert_eq!(json["version"], 1);
}

#[test]
fn a_clean_file_is_never_rewritten() {
    // AC11: every path live ⇒ no ConfirmedGone ⇒ the call site returns before
    // taking SETTINGS_IO at all, and even the `update_if` path would not save.
    let live = vec![fx("Data/Repos/bonsai"), fx("Data/Repos/ham-digi-backend")];
    let clean = Settings {
        recent_repos: live
            .iter()
            .map(|p| RecentRepo {
                path: p.clone(),
                last_opened: 7,
            })
            .collect(),
        hooks_ack_repos: live.clone(),
        open_repos: vec![live[0].clone()],
        ..Settings::default()
    };
    let (_dir, file) = temp_settings(&clean);
    let before_mtime = std::fs::metadata(&file).and_then(|m| m.modified()).ok();
    let before_bytes = std::fs::read(&file).expect("read");

    let snapshot = crate::settings::load_from(&file);
    let mut fake = Fake::new(&[live[0].clone(), live[1].clone(), fx_root()]);
    let facts = facts_for(&snapshot, &mut fake);
    assert!(
        !facts
            .states
            .values()
            .any(|st| *st == PathState::ConfirmedGone),
        "nothing to do"
    );

    let mut report = PruneReport::default();
    crate::settings::update_if(&file, |s| {
        report = prune_stale_paths(s, &facts);
        report.changed()
    })
    .expect("no-op update");
    assert!(!report.changed());
    assert_eq!(std::fs::read(&file).expect("read"), before_bytes);
    assert_eq!(
        std::fs::metadata(&file).and_then(|m| m.modified()).ok(),
        before_mtime,
        "a clean launch must not rewrite settings.json"
    );
}

#[test]
fn a_failed_save_is_non_fatal_and_suppresses_the_log_line() {
    // AC16. The settings path is unwritable (its "parent directory" is a
    // FILE), which is how `save_to` is made to fail on every platform. The
    // call site logs only when the save succeeded, so nothing is announced.
    let dir = tempfile::TempDir::new().expect("temp dir");
    let blocker = dir.path().join("blocker");
    std::fs::write(&blocker, b"not a directory").expect("blocker");
    let file = blocker.join("settings.json");

    let fixture = evidence_settings();
    let mut fake = evidence_probe();
    let facts = evidence_facts(&fixture, &mut fake);

    let mut report = PruneReport::default();
    let saved = crate::settings::update_if(&file, |s| {
        // The file cannot be read either, so seed the mutator with the shape
        // the real load would have produced.
        *s = fixture.clone();
        report = prune_stale_paths(s, &facts);
        report.changed()
    });

    assert!(
        saved.is_err(),
        "unwritable settings path surfaces as an error"
    );
    assert!(report.changed());
    assert!(!file.exists(), "nothing was written");
    // `note_prune` is called only on `saved.is_ok()` — see the lib.rs setup
    // block; a prune that failed to save must never be announced as done.
}

#[test]
fn the_production_probe_classifies_a_real_directory() {
    // Every other gate case runs through the `Fake` seam, which normalizes the
    // strings `classify` builds. This one exercises the literal production
    // closure against a real volume, so the PathBufs `root_of` and the ancestor
    // walk produce are proven to be ones `fs::metadata` accepts.
    let dir = tempfile::TempDir::new().expect("temp dir");
    let repo = dir.path().join("alive").join("repo");
    std::fs::create_dir_all(&repo).expect("create repo");
    let repo_str = repo.to_string_lossy().to_string();

    let probe = |p: &std::path::Path| std::fs::metadata(p).map(drop);
    assert_eq!(classify(&repo_str, &mut { probe }), PathState::Live);

    // The "folder was moved away" shape: the volume root is still reachable and
    // the first failing ancestor is NotFound.
    std::fs::remove_dir_all(dir.path().join("alive")).expect("remove");
    assert_eq!(
        classify(&repo_str, &mut { probe }),
        PathState::ConfirmedGone
    );

    // A rootless path has no volume to prove reachable, whatever the FS says.
    assert_eq!(
        classify("some/relative/path", &mut { probe }),
        PathState::Unreachable
    );
}
