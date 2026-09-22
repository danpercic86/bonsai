//! Shared P115 test fixtures: the platform-shaped path helpers, the `Fake`
//! filesystem stand-in behind the [`Probe`] seam, the 2026-09-22 evidence
//! settings, and the phase-1 seam injectors. Kept out of the test files
//! themselves so each stays readable (and under the size limit).

use std::collections::HashSet;
use std::io::ErrorKind;
use std::path::{Path, MAIN_SEPARATOR};

use super::*;
use crate::settings::{ForgeAccountRecord, RecentRepo, RepoForgeOverride};

/// The volume the contract's evidence uses (`D:\Repos\…`), in the platform's
/// own shape — on Unix the same tree is `/Repos/…`. Fixture paths must be built
/// through [`fx`] so `Path::components` sees a rooted path everywhere: a
/// literal `D:\Repos\x` has no root on Linux and would classify `Unreachable`.
#[cfg(windows)]
pub(super) const VOLUME: &str = "D:";
#[cfg(not(windows))]
pub(super) const VOLUME: &str = "";

/// `fx("Repos/alpha")` ⇒ `D:\Repos\alpha` (Windows) / `/Repos/alpha` (Unix).
pub(super) fn fx(rel: &str) -> String {
    let mut out = String::from(VOLUME);
    for seg in rel.split('/') {
        out.push(MAIN_SEPARATOR);
        out.push_str(seg);
    }
    out
}

/// Same path, always written with FORWARD slashes — the evidence file's
/// worktree entry. On Windows this is the separator-folding case (AC2); on
/// Unix it is simply the normal shape.
pub(super) fn fx_slash(rel: &str) -> String {
    format!("{VOLUME}/{rel}")
}

/// The volume root (`D:\` / `/`).
pub(super) fn fx_root() -> String {
    format!("{VOLUME}{MAIN_SEPARATOR}")
}

/// A filesystem stand-in for [`Probe`]: every path in `live` **and all of its
/// ancestors** exists; everything else fails with `kind`. Logs every probed
/// path so probe economy is assertable per path and per root (AC13).
pub(super) struct Fake {
    live: HashSet<String>,
    kind: ErrorKind,
    /// One entry per call, in order, as `norm` keys.
    pub(super) probes: Vec<String>,
}

impl Fake {
    /// Absent paths report `NotFound` — the shape of a moved folder, and also
    /// what Windows reports for an unmounted drive or a dead share, which is
    /// exactly why the gate probes the root separately.
    pub(super) fn new(live: &[String]) -> Self {
        Self::with_kind(live, ErrorKind::NotFound)
    }

    pub(super) fn with_kind(live: &[String], kind: ErrorKind) -> Self {
        let mut set = HashSet::new();
        for p in live {
            for ancestor in Path::new(p).ancestors() {
                set.insert(norm(&ancestor.to_string_lossy()));
            }
        }
        Self {
            live: set,
            kind,
            probes: Vec::new(),
        }
    }

    pub(super) fn probe(&mut self, p: &Path) -> std::io::Result<()> {
        let key = norm(&p.to_string_lossy());
        let ok = self.live.contains(&key);
        self.probes.push(key);
        if ok {
            Ok(())
        } else {
            Err(std::io::Error::from(self.kind))
        }
    }

    pub(super) fn calls(&self) -> usize {
        self.probes.len()
    }

    /// How many times `path` itself reached the probe.
    pub(super) fn count(&self, path: &str) -> usize {
        let key = norm(path);
        self.probes.iter().filter(|p| **p == key).count()
    }
}

/// The 2026-09-22 evidence, in the platform's shape: `…/Repos` was moved to
/// `…/Data/Repos`, leaving 5 dead recents (one written with forward slashes),
/// a dead forge pin whose live repo has none, and `hooksAckRepos` carrying both
/// the old and the new path.
pub(super) fn evidence_settings() -> Settings {
    let recent_repos = vec![
        RecentRepo {
            path: fx("Data/Repos/ham-digi-backend"),
            last_opened: 1_700_000_900,
        },
        RecentRepo {
            path: fx("Repos/ham-digi-backend"),
            last_opened: 1_700_000_800,
        },
        RecentRepo {
            path: fx("Data/Repos/bonsai"),
            last_opened: 1_700_000_700,
        },
        RecentRepo {
            path: fx("Repos/bonsai"),
            last_opened: 1_700_000_600,
        },
        RecentRepo {
            path: fx("Repos/ham-digi-frontend"),
            last_opened: 1_700_000_500,
        },
        RecentRepo {
            path: fx("Repos/legacy-tools"),
            last_opened: 1_700_000_400,
        },
        RecentRepo {
            path: fx_slash("Repos/.worktrees/ham-digi-backend-hotfix"),
            last_opened: 1_700_000_300,
        },
    ];
    Settings {
        recent_repos,
        hooks_ack_repos: vec![
            fx("Repos/ham-digi-backend"),
            fx("Data/Repos/ham-digi-backend"),
            fx("Data/Repos/bonsai"),
        ],
        repo_forge_overrides: vec![RepoForgeOverride {
            repo_path: fx("Repos/ham-digi-backend"),
            account_id: PINNED_ACCOUNT.to_string(),
        }],
        forge_accounts: vec![account(PINNED_ACCOUNT, "github.com")],
        open_repos: vec![fx("Data/Repos/ham-digi-backend")],
        active_repo: Some(fx("Data/Repos/ham-digi-backend")),
        ..Settings::default()
    }
}

/// The account the evidence file's pin names, and its host.
pub(super) const PINNED_ACCOUNT: &str = "gitHub:github.com:dan";

/// A `forge_accounts` record — rule (6) needs one to read the pinned host from.
pub(super) fn account(account_id: &str, host: &str) -> ForgeAccountRecord {
    ForgeAccountRecord {
        account_id: account_id.to_string(),
        keychain_key: account_id.to_string(),
        host: host.to_string(),
        kind: bonsai_forge::ForgeKind::GitHub,
        login: Some("dan".to_string()),
        avatar_url: None,
    }
}

/// Everything under `…/Data/Repos` resolves; everything under `…/Repos` is
/// `NotFound`; the volume root is reachable (the "folder was moved" shape).
pub(super) fn evidence_probe() -> Fake {
    Fake::new(&[
        fx("Data/Repos/ham-digi-backend"),
        fx("Data/Repos/bonsai"),
        fx_root(),
    ])
}

/// The canonical key a real filesystem would hand back for `path`. Fixture
/// paths do not exist on disk, so `fs::canonicalize` cannot supply one — and
/// without a key nothing can migrate (§4.1). Injecting through `PathFacts`
/// (whose fields are `pub` precisely so phase 1 is seamable) keeps the
/// migration rules testable without a real `\\?\` or 8.3 name.
pub(super) fn canon_of(path: &str) -> String {
    format!("canon::{}", norm(path))
}

/// Phase 1a as the setup call site runs it, plus the canonical keys the real
/// filesystem would have supplied for every `Live` path.
pub(super) fn facts_for(s: &Settings, fake: &mut Fake) -> PathFacts {
    let paths = collect_paths(s);
    let mut facts = classify_all(&paths, &mut |p| fake.probe(p));
    for path in &paths {
        if facts.state(path) == PathState::Live {
            facts.canonical.insert(norm(path), canon_of(path));
        }
    }
    facts
}

/// Rule (6)'s phase-1b output for one candidate.
pub(super) fn set_host(facts: &mut PathFacts, path: &str, host: &str) {
    facts.hosts.insert(canon_of(path), host.to_string());
}

/// AC7, as a property over every fixture: the ack list may only shrink — no
/// path is ever added, rewritten or migrated into it.
pub(super) fn assert_acks_subset(before: &[String], after: &[String]) {
    let before: HashSet<String> = before.iter().map(|p| norm(p)).collect();
    for path in after {
        assert!(
            before.contains(&norm(path)),
            "hooks_ack_repos gained an entry — acks are never added or migrated"
        );
    }
}

/// Phase 1a + 1b for the evidence fixture: the moved repo's `origin` is still
/// on the pinned account's host, which is what rule (6) requires.
pub(super) fn evidence_facts(s: &Settings, fake: &mut Fake) -> PathFacts {
    let mut facts = facts_for(s, fake);
    set_host(&mut facts, &fx("Data/Repos/ham-digi-backend"), "github.com");
    set_host(&mut facts, &fx("Data/Repos/bonsai"), "github.com");
    facts
}

/// AC12's fixture: a session whose tabs name `ConfirmedGone` paths. The audit
/// found that clause unexercised — every earlier fixture used live or
/// unreachable session paths, so "never mutated" was never actually tested
/// against a path the pass was otherwise allowed to act on.
pub(super) fn session_gone_settings() -> Settings {
    Settings {
        recent_repos: vec![
            RecentRepo {
                path: fx("Repos/ghost"),
                last_opened: 10,
            },
            RecentRepo {
                path: fx("Data/Repos/bonsai"),
                last_opened: 20,
            },
        ],
        hooks_ack_repos: vec![fx("Repos/ghost")],
        open_repos: vec![fx("Repos/ghost"), fx("Repos/ghost-2")],
        active_repo: Some(fx("Repos/ghost-2")),
        ..Settings::default()
    }
}
