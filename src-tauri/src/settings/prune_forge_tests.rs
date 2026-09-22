//! P115 rule (6) — positive forge identity, and phase 1b's resolver economy.
//!
//! Why this rule exists (the audit's F1): basename-only migration can land a
//! pin on a repo whose `origin` is on a different host. `resolve_account`
//! filters accounts by host *before* the `account_id` lookup, so such a pin
//! resolves to nothing; `ForgeAccountSwitcher` gates both of its clearing
//! affordances on `accountSource === 'override'`, which that pin can never
//! produce; and rule (1) can never revisit it, because it now names a `Live`
//! path. The result would be a permanent, invisible, unremovable wrong binding
//! — the exact loss P115 was written to prevent.

use std::path::Path;

use super::fixtures::*;
use super::*;
use crate::settings::{RecentRepo, RepoForgeOverride};

/// One dead pin on `…/Repos/api`, one live `…/Data/Repos/api` to migrate onto,
/// and a `forge_accounts` record on `account_host`.
fn host_case(account_host: &str) -> (Settings, String) {
    let live = fx("Data/Repos/api");
    let s = Settings {
        recent_repos: vec![RecentRepo {
            path: live.clone(),
            last_opened: 1,
        }],
        repo_forge_overrides: vec![RepoForgeOverride {
            repo_path: fx("Repos/api"),
            account_id: PINNED_ACCOUNT.to_string(),
        }],
        forge_accounts: vec![account(PINNED_ACCOUNT, account_host)],
        ..Settings::default()
    };
    (s, live)
}

/// Phase 1a + 1b with an injected resolver, then phase 2. Returns the report
/// and how many times the resolver was invoked (AC22).
fn run_with_resolver(
    s: &mut Settings,
    live: &[String],
    resolve: &mut dyn FnMut(&Path) -> Option<String>,
) -> (PruneReport, usize) {
    let mut probe_live = live.to_vec();
    probe_live.push(fx_root());
    let mut fake = Fake::new(&probe_live);
    let mut facts = facts_for(s, &mut fake);

    let mut calls = 0usize;
    resolve_candidate_hosts(s, &mut facts, &mut |p| {
        calls += 1;
        resolve(p)
    });
    let acks_before = s.hooks_ack_repos.clone();
    let report = prune_stale_paths(s, &facts);
    assert_acks_subset(&acks_before, &s.hooks_ack_repos);
    (report, calls)
}

#[test]
fn a_same_host_candidate_migrates() {
    // AC21: the evidence case, asserted through the `hosts` map rather than
    // assumed — this is AC3's precondition.
    let (mut s, live) = host_case("github.com");
    let (report, calls) = run_with_resolver(&mut s, std::slice::from_ref(&live), &mut |_| {
        Some("github.com".to_string())
    });

    assert_eq!(calls, 1, "exactly one repo opened for one dead pin");
    assert_eq!(report.overrides_migrated, 1);
    assert_eq!(report.overrides_kept_dead, 0);
    assert_eq!(s.repo_forge_overrides[0].repo_path, live);
    assert_eq!(s.repo_forge_overrides[0].account_id, PINNED_ACCOUNT);
}

#[test]
fn a_cross_host_candidate_blocks_the_migration() {
    // AC18: the pinned account is on github.com, the candidate's origin is on
    // gitlab.com. Migrating here would create the unremovable pin described at
    // the top of this file.
    let (mut s, live) = host_case("github.com");
    let before = s.repo_forge_overrides.clone();
    let (report, _) = run_with_resolver(&mut s, std::slice::from_ref(&live), &mut |_| {
        Some("gitlab.com".to_string())
    });

    assert_eq!(report.overrides_migrated, 0);
    assert_eq!(report.overrides_kept_dead, 1);
    assert_eq!(s.repo_forge_overrides, before, "byte-identical");
}

#[test]
fn a_missing_account_record_blocks_the_migration() {
    // AC19: the pin names an account that no longer exists. There is no binding
    // left to preserve (`remove_forge_account` already drops overrides when an
    // account is deleted), so this is a stale leftover — kept, never deleted.
    let (mut s, live) = host_case("github.com");
    s.forge_accounts.clear();
    let before = s.repo_forge_overrides.clone();
    let (report, _) = run_with_resolver(&mut s, std::slice::from_ref(&live), &mut |_| {
        Some("github.com".to_string())
    });

    assert_eq!(report.overrides_migrated, 0);
    assert_eq!(report.overrides_kept_dead, 1);
    assert_eq!(s.repo_forge_overrides, before);
    assert_eq!(s.repo_forge_overrides.len(), 1, "AC6: never deleted");
}

#[test]
fn every_resolver_failure_blocks_the_migration() {
    // AC20, three independent cases. `None` is what the production wrapper
    // yields for BOTH `Err(AppError::NoRemote)` (a repo with no `origin`) and
    // any other `Err`; `Some("")` is the documented degradation for an origin
    // `bonsai_forge` cannot parse (`lib.rs:212`).
    let cases: [(&str, Option<String>); 2] = [
        ("Err — no origin, or the repo would not open", None),
        ("Ok(\"\") — unparseable origin", Some(String::new())),
    ];
    for (what, answer) in cases {
        let (mut s, live) = host_case("github.com");
        let before = s.repo_forge_overrides.clone();
        let (report, _) =
            run_with_resolver(&mut s, std::slice::from_ref(&live), &mut |_| answer.clone());

        assert_eq!(report.overrides_migrated, 0, "{what}");
        assert_eq!(report.overrides_kept_dead, 1, "{what}");
        assert_eq!(s.repo_forge_overrides, before, "{what}");
    }
}

#[test]
fn an_empty_host_is_never_a_wildcard() {
    // AC20's explicit leg: an empty resolved host must not compare equal to an
    // account whose host is somehow also empty. Injected straight into `hosts`,
    // since phase 1b refuses to store an empty host in the first place.
    let (mut s, live) = host_case("");
    let mut fake = Fake::new(&[live.clone(), fx_root()]);
    let mut facts = facts_for(&s, &mut fake);
    set_host(&mut facts, &live, "");

    let before = s.repo_forge_overrides.clone();
    let acks_before = s.hooks_ack_repos.clone();
    let report = prune_stale_paths(&mut s, &facts);
    assert_acks_subset(&acks_before, &s.hooks_ack_repos);
    assert_eq!(report.overrides_migrated, 0);
    assert_eq!(report.overrides_kept_dead, 1);
    assert_eq!(s.repo_forge_overrides, before);
}

#[test]
fn phase_1b_opens_nothing_when_no_override_is_dead() {
    // AC22's fast path: dead recents and dead acks are NOT a reason to open a
    // repository. The overwhelmingly common launch resolves zero hosts.
    let mut s = Settings {
        recent_repos: vec![
            RecentRepo {
                path: fx("Repos/gone"),
                last_opened: 1,
            },
            RecentRepo {
                path: fx("Data/Repos/api"),
                last_opened: 2,
            },
        ],
        hooks_ack_repos: vec![fx("Repos/gone")],
        ..Settings::default()
    };
    let live = vec![fx("Data/Repos/api")];
    let (report, calls) = run_with_resolver(&mut s, &live, &mut |_| Some("github.com".to_string()));

    assert_eq!(calls, 0, "AC22: no override is dead ⇒ no repo is opened");
    assert_eq!(report.recents_pruned, 1);
    assert_eq!(report.hooks_acks_pruned, 1);
}

#[test]
fn phase_1b_resolves_only_basename_matching_candidates() {
    // AC22's second half: rule (3) can never pick a candidate with a different
    // basename, so opening it would be pure cost.
    let (mut s, live) = host_case("github.com");
    s.recent_repos.push(RecentRepo {
        path: fx("Data/Repos/unrelated"),
        last_opened: 3,
    });
    let live = vec![live, fx("Data/Repos/unrelated")];
    let mut resolved: Vec<String> = Vec::new();
    let (report, calls) = run_with_resolver(&mut s, &live, &mut |p| {
        resolved.push(p.to_string_lossy().to_string());
        Some("github.com".to_string())
    });

    assert_eq!(calls, 1);
    assert_eq!(
        resolved,
        vec![live[0].clone()],
        "only the name-matching candidate"
    );
    assert_eq!(report.overrides_migrated, 1);
}

/// The literal production closure from `lib.rs`'s setup block.
fn production_resolve(workdir: &Path) -> Option<String> {
    bonsai_forge::resolve_forge_identity(workdir)
        .ok()
        .map(|(host, _owner, _kind)| host)
}

#[test]
fn the_production_resolver_declines_a_directory_that_is_not_a_repo() {
    // AC20's "any other Err" leg through the LITERAL production closure: a real
    // directory with no git repository at all. Proves the wrapper maps the
    // error away rather than panicking or yielding a host.
    let dir = tempfile::TempDir::new().expect("temp dir");
    assert_eq!(
        production_resolve(dir.path()),
        None,
        "no repo ⇒ no host ⇒ no migration"
    );
}

#[test]
fn the_production_resolver_declines_a_repo_with_no_origin() {
    // AC20's `AppError::NoRemote` leg, as a REAL one — a genuine repository
    // that simply has no `origin`. Through the seam it is indistinguishable
    // from any other error (both are `None`), so this is the only place the
    // variant itself is exercised.
    let dir = tempfile::TempDir::new().expect("temp dir");
    git2::Repository::init(dir.path()).expect("init fixture repo");
    assert_eq!(
        production_resolve(dir.path()),
        None,
        "a repo with no origin has no forge identity to migrate onto"
    );
}

#[test]
fn rule_four_blocks_across_two_spellings_of_one_directory() {
    // AC24. `read_repo_info` persists the RAW opened path, so two strings for
    // one directory are routine (`\\?\D:\x` vs `D:\x`, 8.3, mapped drive vs
    // UNC). `norm` cannot fold those — the canonical key can. Without it the
    // dead pin would migrate onto a repo that ALREADY has a pin, and
    // `resolve_account`'s `.find()` would then let list order decide which one
    // wins. The canonical keys are injected through the phase-1 seam rather
    // than requiring a real 8.3 name.
    let primary = fx("Data/Repos/backend");
    let alias = fx("Mapped/backend"); // same directory, different spelling
    let mut s = Settings {
        recent_repos: vec![
            RecentRepo {
                path: primary.clone(),
                last_opened: 2,
            },
            RecentRepo {
                path: alias.clone(),
                last_opened: 1,
            },
        ],
        repo_forge_overrides: vec![
            RepoForgeOverride {
                repo_path: fx("Repos/backend"),
                account_id: PINNED_ACCOUNT.to_string(),
            },
            RepoForgeOverride {
                repo_path: alias.clone(),
                account_id: "gitHub:github.com:other".to_string(),
            },
        ],
        forge_accounts: vec![account(PINNED_ACCOUNT, "github.com")],
        ..Settings::default()
    };
    let mut fake = Fake::new(&[primary.clone(), alias.clone(), fx_root()]);
    let mut facts = facts_for(&s, &mut fake);
    // Both spellings resolve to the same directory.
    facts.canonical.insert(norm(&alias), canon_of(&primary));
    set_host(&mut facts, &primary, "github.com");

    let before = s.repo_forge_overrides.clone();
    let acks_before = s.hooks_ack_repos.clone();
    let report = prune_stale_paths(&mut s, &facts);
    assert_acks_subset(&acks_before, &s.hooks_ack_repos);

    assert_eq!(report.overrides_migrated, 0, "rule (4) blocks");
    assert_eq!(report.overrides_kept_dead, 1);
    assert_eq!(s.repo_forge_overrides, before);
}

#[test]
fn phase_two_performs_no_io() {
    // AC25. `prune_stale_paths` takes neither a probe nor a resolver, so a
    // panicking seam cannot even be handed to it — the compiler enforces the
    // property. What CAN still be tested is that it never reaches the
    // filesystem behind our back: the facts below deliberately CONTRADICT the
    // real filesystem. A directory that genuinely exists is marked
    // `ConfirmedGone`; a path that does not exist at all is marked `Live`.
    // Anything that re-probed would produce the opposite outcome.
    let dir = tempfile::TempDir::new().expect("temp dir");
    let real = dir.path().to_string_lossy().to_string();
    let imaginary = fx("Nowhere/at/all");

    let mut s = Settings {
        recent_repos: vec![
            RecentRepo {
                path: real.clone(),
                last_opened: 1,
            },
            RecentRepo {
                path: imaginary.clone(),
                last_opened: 2,
            },
        ],
        hooks_ack_repos: vec![real.clone(), imaginary.clone()],
        ..Settings::default()
    };
    let mut facts = PathFacts::default();
    facts.states.insert(norm(&real), PathState::ConfirmedGone);
    facts.states.insert(norm(&imaginary), PathState::Live);

    let acks_before = s.hooks_ack_repos.clone();
    let report = prune_stale_paths(&mut s, &facts);
    assert_acks_subset(&acks_before, &s.hooks_ack_repos);

    assert_eq!(report.recents_pruned, 1);
    assert_eq!(report.hooks_acks_pruned, 1);
    assert_eq!(s.recent_repos.len(), 1);
    assert_eq!(
        s.recent_repos[0].path, imaginary,
        "phase 2 followed the facts, not the filesystem"
    );
    assert_eq!(s.hooks_ack_repos, vec![imaginary]);
}

#[test]
fn rule_four_blocks_a_case_variant_of_the_candidates_path() {
    // The macOS hole rule (4)'s `norm` fallback had to close. Default APFS is
    // case-INsensitive but case-PRESERVING, so `…/repos/y` and `…/Repos/y` are
    // one directory whose two persisted spellings `realpath` returns as given —
    // the canonical keys differ, F4 keeps `norm` case-sensitive off Windows, and
    // both branches of rule (4) would miss. The dead pin would then land on a
    // repo that already has one, and `resolve_account`'s `.find()` would let
    // list order pick the winner.
    //
    // Unlike `rule_four_blocks_across_two_spellings_of_one_directory`, this
    // fixture does NOT share a canonical key — that shared key is exactly the
    // premise APFS breaks.
    let candidate = fx("x/repos/y");
    let pinned_spelling = fx("x/Repos/y");
    let mut s = Settings {
        recent_repos: vec![RecentRepo {
            path: candidate.clone(),
            last_opened: 1,
        }],
        repo_forge_overrides: vec![
            RepoForgeOverride {
                repo_path: fx("Gone/y"),
                account_id: PINNED_ACCOUNT.to_string(),
            },
            RepoForgeOverride {
                repo_path: pinned_spelling.clone(),
                account_id: "gitHub:github.com:other".to_string(),
            },
        ],
        forge_accounts: vec![account(PINNED_ACCOUNT, "github.com")],
        ..Settings::default()
    };
    let mut fake = Fake::new(&[candidate.clone(), pinned_spelling.clone(), fx_root()]);
    let mut facts = facts_for(&s, &mut fake);
    set_host(&mut facts, &candidate, "github.com");

    let before = s.repo_forge_overrides.clone();
    let acks_before = s.hooks_ack_repos.clone();
    let report = prune_stale_paths(&mut s, &facts);
    assert_acks_subset(&acks_before, &s.hooks_ack_repos);

    assert_eq!(report.overrides_migrated, 0, "rule (4) blocks");
    assert_eq!(report.overrides_kept_dead, 1);
    assert_eq!(s.repo_forge_overrides, before, "byte-identical");
}
