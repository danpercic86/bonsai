//! P115 phase 1: the transient-absence gate, the string keys, root-first
//! probing, and path collection. Phase 2 lives in `prune_migration_tests.rs`,
//! rule (6) in `prune_forge_tests.rs`; fixtures in `prune_fixtures.rs`.
//!
//! Every gate case runs through the injected [`Probe`] seam — no test needs a
//! real unmounted drive, and the suite passes on all three CI platforms.

use std::io::ErrorKind;
use std::path::MAIN_SEPARATOR;

use super::fixtures::*;
use super::*;
use crate::settings::{RecentRepo, RepoForgeOverride};

// ------------------------------------------------------------- norm/basename

#[test]
fn norm_trims_trailing_separators() {
    assert_eq!(
        norm(&format!("{}{MAIN_SEPARATOR}", fx("Repos/Alpha"))),
        norm(&fx("Repos/Alpha")),
        "a trailing separator is not a different repo"
    );
}

#[cfg(windows)]
#[test]
fn norm_folds_forward_slashes_on_windows_only() {
    // The evidence file's worktree entry is written with forward slashes; it
    // must land on the same key as its backslash siblings (AC2). On Unix `\`
    // is a legal filename character, so this folding is Windows-only by design.
    assert_eq!(norm("D:/Repos/x"), norm("D:\\Repos\\x"));
    assert_eq!(norm("D:/Repos/x"), "d:\\repos\\x");
}

#[test]
fn basename_is_the_last_segment_of_the_normalized_key() {
    assert_eq!(
        basename(&fx("Data/Repos/ham-digi-backend")),
        "ham-digi-backend"
    );
    assert_eq!(
        basename(&fx_slash("Data/Repos/bonsai")),
        "bonsai",
        "a forward-slash entry still ends at its last segment"
    );
    assert_eq!(
        basename(&format!("{}{MAIN_SEPARATOR}", fx("Data/Repos/bonsai"))),
        "bonsai"
    );
    assert_eq!(basename(""), "");
}

// ------------------------------------------------------------------ the gate

#[test]
fn a_live_path_is_live_after_exactly_one_probe() {
    let live = fx("Data/Repos/bonsai");
    let mut fake = Fake::new(std::slice::from_ref(&live));
    assert_eq!(classify(&live, &mut |p| fake.probe(p)), PathState::Live);
    assert_eq!(fake.calls(), 1, "a live path must not walk its ancestors");
}

#[test]
fn a_moved_folder_under_a_reachable_root_is_confirmed_gone() {
    // AC1's precondition: `D:\` mounted, `D:\Repos` gone.
    let mut fake = evidence_probe();
    assert_eq!(
        classify(&fx("Repos/ham-digi-backend"), &mut |p| fake.probe(p)),
        PathState::ConfirmedGone
    );
}

#[test]
fn an_unmounted_volume_is_unreachable_even_though_it_reports_not_found() {
    // AC8. Windows maps ERROR_INVALID_DRIVE / ERROR_BAD_NETPATH to NotFound,
    // so a kind-only gate would have pruned every entry on an unplugged drive.
    // Nothing exists here — not even the root.
    let mut fake = Fake::new(&[]);
    assert_eq!(
        classify(&fx("repos/x"), &mut |p| fake.probe(p)),
        PathState::Unreachable
    );
}

#[test]
fn a_share_error_below_a_live_root_is_unreachable() {
    // AC9: root Ok, first failing component is not NotFound.
    for kind in [
        ErrorKind::PermissionDenied,
        ErrorKind::TimedOut,
        ErrorKind::ConnectionRefused,
    ] {
        let mut fake = Fake::with_kind(&[fx_root()], kind);
        assert_eq!(
            classify(&fx("Repos/secret"), &mut |p| fake.probe(p)),
            PathState::Unreachable,
            "{kind:?} must never authorize a mutation"
        );
    }
}

#[test]
fn a_rootless_path_is_unreachable() {
    let mut fake = Fake::new(&[]);
    assert_eq!(
        classify("some/relative/path", &mut |p| fake.probe(p)),
        PathState::Unreachable
    );
    assert_eq!(classify("", &mut |p| fake.probe(p)), PathState::Unreachable);
}

#[test]
fn classify_reports_live_when_its_walk_finds_no_failure() {
    // Pins `classify`'s OWN contract, not production behaviour: the walk found
    // no failing ancestor, so it reports `Live`. Through `classify_all` the
    // probe is memoized and this branch is normally unreachable (the leaf's
    // first answer is authoritative) — hence the deliberately non-memoized
    // probe here.
    let target = fx("Data/Repos/bonsai");
    let mut calls = 0usize;
    let state = classify(&target, &mut |_p| {
        calls += 1;
        if calls == 1 {
            Err(std::io::Error::from(ErrorKind::NotFound))
        } else {
            Ok(())
        }
    });
    assert_eq!(state, PathState::Live);
}

// -------------------------------------------------------- collect + classify

#[test]
fn collect_paths_dedupes_by_norm_and_probes_once() {
    // AC13: the same repo in all three lists plus the session is one probe.
    let repo = fx("Data/Repos/bonsai");
    let s = Settings {
        recent_repos: vec![RecentRepo {
            path: repo.clone(),
            last_opened: 1,
        }],
        hooks_ack_repos: vec![format!("{repo}{MAIN_SEPARATOR}")], // trailing sep
        repo_forge_overrides: vec![RepoForgeOverride {
            repo_path: repo.clone(),
            account_id: "a".into(),
        }],
        open_repos: vec![repo.clone()],
        ..Settings::default()
    };

    let paths = collect_paths(&s);
    assert_eq!(
        paths,
        vec![repo.clone()],
        "one distinct key, original string"
    );

    let mut fake = Fake::new(std::slice::from_ref(&repo));
    let facts = classify_all(&paths, &mut |p| fake.probe(p));
    assert_eq!(fake.count(&repo), 1, "one probe per distinct path");
    assert_eq!(fake.count(&fx_root()), 1, "the root is probed exactly once");
    assert_eq!(fake.calls(), 2, "root + path, nothing else");
    assert_eq!(facts.states.get(&norm(&repo)), Some(&PathState::Live));
}

#[test]
fn collect_paths_includes_session_tabs_and_skips_blanks() {
    // AC13's second half: `open_repos` is a migration CANDIDATE source, so it
    // must be classified even though §1.4 never mutates it.
    let s = Settings {
        open_repos: vec![fx("Data/Repos/only-open"), String::new(), "   ".into()],
        active_repo: Some(fx("Data/Repos/only-open")),
        ..Settings::default()
    };
    assert_eq!(collect_paths(&s), vec![fx("Data/Repos/only-open")]);
}

#[test]
fn evidence_fixture_classifies_five_dead_and_two_live() {
    let s = evidence_settings();
    let mut fake = evidence_probe();
    let states = facts_for(&s, &mut fake).states;
    let gone = states
        .values()
        .filter(|st| **st == PathState::ConfirmedGone)
        .count();
    let live = states.values().filter(|st| **st == PathState::Live).count();
    assert_eq!(gone, 5, "5 dead repo paths under the moved prefix");
    assert_eq!(live, 2, "the two repos at their new location");
    assert_eq!(
        states
            .values()
            .filter(|st| **st == PathState::Unreachable)
            .count(),
        0
    );
}

// ------------------------------------------------------- §5.3 roots first

#[test]
fn many_paths_under_one_dead_root_cost_one_root_probe() {
    // AC13 / INFO-2: without roots-first, a user with N dead `\server\…`
    // entries pays the full network timeout N times on EVERY launch, and two of
    // the three lists are uncapped. Nothing exists here, not even the root.
    let s = Settings {
        recent_repos: (0..4)
            .map(|i| RecentRepo {
                path: fx(&format!("Repos/dead-{i}")),
                last_opened: i,
            })
            .collect(),
        hooks_ack_repos: vec![fx("Repos/dead-0"), fx("Repos/dead-1")],
        ..Settings::default()
    };
    let mut fake = Fake::new(&[]);
    let facts = facts_for(&s, &mut fake);

    assert_eq!(fake.count(&fx_root()), 1, "one probe for the dead root");
    assert_eq!(fake.calls(), 1, "and no full-path probe at all");
    assert_eq!(facts.states.len(), 4);
    assert!(facts
        .states
        .values()
        .all(|st| *st == PathState::Unreachable));
}

#[test]
fn a_live_root_is_probed_once_across_many_dead_paths() {
    // The gate itself re-probes the root on every failure branch; `classify_all`
    // memoizes by path so that costs nothing beyond the first call.
    let s = Settings {
        recent_repos: (0..3)
            .map(|i| RecentRepo {
                path: fx(&format!("Repos/gone-{i}")),
                last_opened: i,
            })
            .collect(),
        ..Settings::default()
    };
    let mut fake = Fake::new(&[fx_root()]);
    let facts = facts_for(&s, &mut fake);

    assert_eq!(fake.count(&fx_root()), 1, "each distinct root probed once");
    assert_eq!(fake.count(&fx("Repos")), 1, "and each ancestor once");
    assert!(facts
        .states
        .values()
        .all(|st| *st == PathState::ConfirmedGone));
}

// ------------------------------------------------- F4: the case fold is cfg'd

#[cfg(unix)]
#[test]
fn case_only_siblings_stay_distinct_on_a_case_sensitive_fs() {
    // AC23. `/x/api` and `/x/Api` are two different directories on ext4. Folding
    // them to one key would apply one verdict to both — and for an override that
    // means a LIVE repo's pin becoming migration-eligible with nothing gone.
    let dead = fx("x/Api");
    let live = fx("x/api");
    assert_ne!(norm(&dead), norm(&live));

    let s = Settings {
        recent_repos: vec![RecentRepo {
            path: live.clone(),
            last_opened: 1,
        }],
        repo_forge_overrides: vec![RepoForgeOverride {
            repo_path: dead.clone(),
            account_id: "gitHub:github.com:dan".into(),
        }],
        ..Settings::default()
    };
    let mut fake = Fake::new(&[live.clone(), fx_root()]);
    let facts = facts_for(&s, &mut fake);

    assert_eq!(facts.state(&dead), PathState::ConfirmedGone);
    assert_eq!(facts.state(&live), PathState::Live);
    assert_eq!(
        basename(&dead),
        "Api",
        "and the basenames differ too, so rule (3) never pairs them"
    );
}

#[cfg(windows)]
#[test]
fn case_only_siblings_collapse_to_one_key_on_windows() {
    // AC23's companion: NTFS is case-insensitive, so the same two strings are
    // one repo and must share a key (that is what dedupes the evidence file).
    assert_eq!(norm(r"D:\x\Api"), norm(r"D:\x\api"));
    assert_eq!(basename(r"D:\x\Api"), basename(r"D:\x\api"));
}

#[cfg(not(windows))]
#[test]
fn a_backslash_is_a_filename_character_on_unix() {
    // The separator fold is Windows-only, and this is the leg where the guard
    // matters: on Unix `x/a\b` is a FILE named `a\b`, not the path `x/a/b`.
    // Folding it would merge two genuinely different entries under one verdict.
    assert_ne!(norm(r"/x/a\b"), norm("/x/a/b"));
    assert_eq!(basename(r"/x/a\b"), r"a\b");
}
