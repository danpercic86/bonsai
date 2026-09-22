//! P115 phase 1 — the transient-absence gate and the comparison keys.
//!
//! Split out of `prune.rs` to keep both files reviewable: this half is "what is
//! true of a path", the other half is "what we do about it". Everything here
//! runs against a lock-free `load_from` snapshot; no `SETTINGS_IO` is held.

use std::collections::{HashMap, HashSet};
use std::path::{Component, Path, PathBuf};

use crate::settings::Settings;

/// What a probe says about one persisted path. Only [`Self::ConfirmedGone`]
/// authorizes any mutation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathState {
    /// The full path resolved — the repo is there.
    Live,
    /// The volume/share root is reachable AND the first failing ancestor below
    /// it failed with `NotFound`: the directory was moved or deleted.
    ConfirmedGone,
    /// Anything else — unmounted volume, unreachable share, permission denied,
    /// rootless path. ALWAYS kept, never pruned, never migrated.
    Unreachable,
}

/// Probe seam (testability): `Ok(())` == the path exists and is reachable.
/// The production probe is `|p| std::fs::metadata(p).map(drop)`.
pub type Probe<'a> = &'a mut dyn FnMut(&Path) -> std::io::Result<()>;

/// Everything phase 2 needs to know about the filesystem and the forges. Built
/// entirely in phase 1 ([`classify_all`] then
/// [`resolve_candidate_hosts`](super::resolve_candidate_hosts)); consumed
/// read-only by [`prune_stale_paths`](super::prune_stale_paths), which performs
/// no I/O of any kind.
#[derive(Debug, Clone, Default)]
pub struct PathFacts {
    /// `norm(path)` -> state, for EVERY path [`collect_paths`] returned.
    pub states: HashMap<String, PathState>,
    /// `norm(path)` -> canonical key, for `Live` paths only (§4.1 live side).
    /// Absent when `fs::canonicalize` failed — such a path can dedupe and block
    /// by `norm`, but can never be a migration target.
    pub canonical: HashMap<String, String>,
    /// Canonical key -> lowercased forge host of that repo's `origin`.
    /// Populated ONLY when at least one override is `ConfirmedGone`, and only
    /// for candidates whose basename matches a dead override's (§5.1). An
    /// absent key, a resolve error and an empty host are all "no migration".
    pub hosts: HashMap<String, String>,
}

impl PathFacts {
    /// A key ABSENT from `states` is `Live`: never mutate a path that was not
    /// probed (covers entries added between the two phases).
    pub fn state(&self, path: &str) -> PathState {
        self.states
            .get(&norm(path))
            .copied()
            .unwrap_or(PathState::Live)
    }

    /// The live side's comparison key (§4.1), or `None` for a dead path or one
    /// that would not canonicalize.
    pub fn key(&self, path: &str) -> Option<&String> {
        self.canonical.get(&norm(path))
    }
}

/// Classifies one persisted path through the transient-absence gate (§3):
/// **root-reachability first, then a `NotFound` ancestor chain**. The
/// discriminator is that a *move* leaves the volume root alive (`D:\` mounted,
/// `D:\Repos` gone) while a *missing volume* does not.
///
/// **The error kind is consulted only BELOW a reachable root.** The root probe
/// rejects on *any* kind, deliberately: Rust's Windows `decode_error_kind` maps
/// `ERROR_BAD_NETPATH`, `ERROR_BAD_NET_NAME` and `ERROR_INVALID_DRIVE` to
/// `ErrorKind::NotFound`, so a down server or an unmounted drive letter *does*
/// surface as `NotFound`. A kind-only gate would prune those entries; the root
/// probe is what makes the kind meaningful.
pub fn classify(path: &str, probe: Probe<'_>) -> PathState {
    let p = Path::new(path);
    if probe(p).is_ok() {
        return PathState::Live;
    }
    // A relative or otherwise rootless path has no volume to prove reachable.
    let Some(root) = root_of(p) else {
        return PathState::Unreachable;
    };
    if probe(&root).is_err() {
        return PathState::Unreachable; // kind deliberately IGNORED here
    }
    // Walk root -> leaf; the first component that fails decides.
    let mut cur = root;
    for comp in p.components() {
        match comp {
            Component::Prefix(_) | Component::RootDir => continue,
            other => cur.push(other.as_os_str()),
        }
        if let Err(e) = probe(&cur) {
            return if e.kind() == std::io::ErrorKind::NotFound {
                PathState::ConfirmedGone
            } else {
                PathState::Unreachable
            };
        }
    }
    // Nothing failed on the way down, so this walk found no evidence the path
    // is gone and `classify` reports `Live`.
    //
    // Read this as a rule, not as a promise about a recreated folder: under
    // [`classify_all`] the probe is memoized, so the FIRST probe of a path is
    // authoritative and a folder that reappears mid-pass still classifies
    // `ConfirmedGone` (that is inside §5.3's accepted race — it is pruned once
    // and `record_recent` puts it back). This branch is still reachable there
    // for a path containing `.` or doubled separators, which `Path::components`
    // normalizes out of the rebuilt `cur`, giving the walk a different memo key
    // than the leaf probe used.
    PathState::Live
}

/// The `Prefix` + `RootDir` components of `p` joined (`D:\`,
/// `\\server\share\`, `/`), or `None` when `p` has no root component at all
/// (relative, or Windows drive-relative like `D:foo`).
fn root_of(p: &Path) -> Option<PathBuf> {
    let mut root = PathBuf::new();
    let mut rooted = false;
    for comp in p.components() {
        match comp {
            Component::Prefix(prefix) => root.push(prefix.as_os_str()),
            Component::RootDir => {
                rooted = true;
                root.push(Component::RootDir.as_os_str());
            }
            _ => break,
        }
    }
    rooted.then_some(root)
}

/// Normalized comparison key for a **dead** persisted path — canonicalization
/// is unavailable there by construction (§4.1 splits the two sides: live paths
/// compare by [`canon_key`]).
///
/// `same_repo_path` is deliberately NOT used anywhere in P115: it canonicalizes
/// and degrades to `eq_ignore_ascii_case` when it cannot, which is every dead
/// path (`commands/repo.rs:385`).
///
///   1. trim trailing `/` and `\`
///   2. Windows only: replace `/` with `\` (on Unix `\` is a legal filename
///      character, so folding it there would merge two different files)
///   3. Windows only: `to_ascii_lowercase()` — F4. On Linux and case-sensitive
///      APFS `/x/api` and `/x/Api` are *different directories*; folding them
///      would apply one verdict to both, and for an override that means a live
///      repo's pin becoming migration-eligible with nothing actually gone.
pub(super) fn norm(path: &str) -> String {
    let trimmed = path.trim_end_matches(['/', '\\']);
    #[cfg(windows)]
    {
        trimmed.replace('/', "\\").to_ascii_lowercase()
    }
    #[cfg(not(windows))]
    {
        trimmed.to_string()
    }
}

/// Last path segment of `norm(path)` (after the final separator), or the whole
/// key when it has no separator. `norm` has already folded separators to the
/// platform's own, so splitting on `MAIN_SEPARATOR` is exact.
pub(super) fn basename(path: &str) -> String {
    let key = norm(path);
    match key.rsplit_once(std::path::MAIN_SEPARATOR) {
        Some((_, last)) => last.to_string(),
        None => key,
    }
}

/// Comparison key for a **live** path: `fs::canonicalize`'s string form, which
/// folds what `norm` cannot — `\\?\D:\x` vs `D:\x`, 8.3 short names,
/// mapped-drive vs UNC, NFC/NFD. `None` when canonicalization fails.
///
/// It is a comparison key ONLY: the string written into `repo_path` on
/// migration is always the persisted candidate, never this `\\?\` form.
fn canon_key(path: &str) -> Option<String> {
    std::fs::canonicalize(path)
        .ok()
        .map(|p| p.to_string_lossy().to_string())
}

/// Every distinct path P115 must know the state of: `recent_repos[*].path`,
/// `hooks_ack_repos[*]`, `repo_forge_overrides[*].repo_path`, and
/// `open_repos[*]` (the last are migration CANDIDATES only — §1.4 never mutates
/// them). Deduplicated by `norm`; returns the original strings, one per
/// distinct key, so the probe sees exactly what was persisted.
///
/// Blank entries are skipped: probing `""` can only ever yield `Unreachable`
/// and would pollute `unreachable_skipped`.
pub fn collect_paths(s: &Settings) -> Vec<String> {
    let all = s
        .recent_repos
        .iter()
        .map(|r| r.path.as_str())
        .chain(s.hooks_ack_repos.iter().map(String::as_str))
        .chain(s.repo_forge_overrides.iter().map(|o| o.repo_path.as_str()))
        .chain(s.open_repos.iter().map(String::as_str));

    let mut seen: HashSet<String> = HashSet::new();
    let mut out: Vec<String> = Vec::new();
    for path in all {
        if path.trim().is_empty() {
            continue;
        }
        if seen.insert(norm(path)) {
            out.push(path.to_string());
        }
    }
    out
}

/// Phase 1a — all filesystem I/O, no lock held. Fills `states` + `canonical`.
///
/// **Roots first (§5.3).** The distinct volume/share roots are probed once, up
/// front, and every path under a root that failed is short-circuited to
/// `Unreachable` *without* probing the full path. Under §3 such a path is
/// `Unreachable` whatever the full probe would say (the root check precedes the
/// kind check and rejects on any kind), so this is pure bookkeeping — but it
/// turns N dead `\\server\…` entries from N multi-second network timeouts per
/// launch into one, and `hooks_ack_repos`/`repo_forge_overrides`/`open_repos`
/// are uncapped.
///
/// The probe handed to [`classify`] is **memoized by path** so the root it
/// re-probes on its failure branch costs nothing: each distinct path, root
/// included, reaches the caller's probe at most once.
pub fn classify_all(paths: &[String], probe: Probe<'_>) -> PathFacts {
    let mut cache: HashMap<String, Option<std::io::ErrorKind>> = HashMap::new();
    let mut memo = |p: &Path| -> std::io::Result<()> {
        let key = norm(&p.to_string_lossy());
        if let Some(cached) = cache.get(&key) {
            return match cached {
                None => Ok(()),
                Some(kind) => Err(std::io::Error::from(*kind)),
            };
        }
        let result = probe(p);
        cache.insert(key, result.as_ref().err().map(|e| e.kind()));
        result
    };

    let mut dead_roots: HashSet<String> = HashSet::new();
    for path in paths {
        if let Some(root) = root_of(Path::new(path)) {
            if memo(&root).is_err() {
                dead_roots.insert(norm(&root.to_string_lossy()));
            }
        }
    }

    let mut facts = PathFacts::default();
    for path in paths {
        let key = norm(path);
        if facts.states.contains_key(&key) {
            continue;
        }
        let root_dead = root_of(Path::new(path))
            .is_some_and(|r| dead_roots.contains(&norm(&r.to_string_lossy())));
        let state = if root_dead {
            PathState::Unreachable
        } else {
            classify(path, &mut memo)
        };
        if state == PathState::Live {
            if let Some(canon) = canon_key(path) {
                facts.canonical.insert(key.clone(), canon);
            }
        }
        facts.states.insert(key, state);
    }
    facts
}
