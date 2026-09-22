# P115 — Pruning stale repo paths from `settings.json`

**Revision 2 (2026-09-22)** — amends rev 1 after the security audit. Changed sections are marked
**[REV 2]**; everything unmarked is rev 1 unchanged and was reviewed with zero MUST-FIX.
Rev 2 touches **§1.3, §4, §5, §9, §10 only**. The §1.1/§1.2 verdicts, the §3 gate, the §6 call
site and threading, and the §7 log-only reporting decision are confirmed and **not reopened**.

Rev 2 changes in one line each:
- **F1** — §4 gains **rule (6)**: an override migrates only when the live candidate's own forge
  host matches the pinned account's host. A cross-host migration is not inert; it is invisible,
  unremovable from the UI, and permanent. §1.3's asymmetry argument is corrected.
- **F2** — the same-host same-basename residual is recorded as **accepted**, with its caveat.
- **F3** — comparison split: **dead side = `norm`, live side = canonical**.
- **F4** — `norm`'s case fold is `#[cfg(windows)]`-gated, mirroring the separator fold.
- **INFO-2** — `classify_all` probes distinct roots first and short-circuits under a dead root.

**Revision 2.1 (2026-09-22)** — post-implementation corrections, marked **[REV 2.1]**. Rev 2 was
implemented and passed both a code re-review and a security re-audit with **zero MUST-FIX**; the
audit confirmed **F1, F3 and F4 closed** and CLEAN-1 (ack safety) not regressed by the `prune.rs`
rewrite. 2.1 fixes what those passes found in the *contract*, not in the policy: the seam type
(`Option`, not `Result`), one genuine correctness gap in rule (4)'s fallback on case-insensitive
filesystems, §5.3's overstated equivalence claim, and rule (5)'s "dead" ambiguity. **No policy
verdict changes.**

Status: rev 2 implemented, re-reviewed and re-audited (both approve). Rev 2.1 corrections being
applied.
`SETTINGS_VERSION` stays **1** (no field added or removed — below the bar the `Settings`
wire-format doc sets at "a field with no safe default").

Evidence this is written against (real `%APPDATA%\com.bonsai.app\settings.json`, 2026-09-22):
`D:\Repos` was moved to `D:\Data\Repos`. 5 `recentRepos` under the dead prefix; one
`repoForgeOverrides` entry for the dead `D:\Repos\ham-digi-backend` while the live
`D:\Data\Repos\ham-digi-backend` has none (the user silently lost a forge-account binding);
`hooksAckRepos` carrying both the old and the new path for two repos.

Not in scope, not a bug: a recents path written with forward slashes. `record_recent` dedups via
`same_repo_path` (`src-tauri/src/commands/repo.rs:385`), which canonicalizes; the difference is
cosmetic.

---

## 1. Policy — the three lists

### 1.1 `recent_repos` — PRUNE (only `ConfirmedGone`)

Drop entries the §3 gate classifies `ConfirmedGone`. Everything else (including `Unreachable`)
is kept verbatim.

Why: recents is a *convenience list of things the user can click*; an entry that cannot be opened
is pure noise, and the cost of a wrong prune is one re-pick from the folder dialog, after which
`record_recent` puts it back at the front. This is the only list where the correct action is
deletion, and the only one with a cap (`MAX_RECENT_REPOS`), so dead entries evict live ones.

### 1.2 `hooks_ack_repos` — PRUNE `ConfirmedGone`, NEVER migrate

Drop `ConfirmedGone` entries. Never rewrite an entry's path, never copy an ack to another path,
under any similarity rule.

Why: this list gates a **security disclosure** (one-time per-repo "this repo has runnable git
hooks"). The two error directions are not symmetric:
- a wrong *migration* means a repo the user never acknowledged silently inherits an ack, and a
  hook-execution warning never appears — an unrecoverable, invisible loss of consent;
- a wrong *prune* costs exactly one extra disclosure prompt.
Pruning errs in the safe direction, and this is the only uncapped list of the three, so without a
prune it grows monotonically (the evidence file already carries doubled entries).

### 1.3 `repo_forge_overrides` — NEVER delete; migrate only under §4's strong rule **[REV 2]**

A dead override is **kept** unless it migrates. Silent deletion is forbidden in P115.

Why never delete: the harm the evidence shows was *loss of a binding*, not clutter. A dead
override is inert — `resolve_account` (`src-tauri/src/commands/forge_accounts.rs:48-60`) finds it
only via `same_repo_path(&o.repo_path, repo_path)`, so an entry no live repo matches never fires.
Deleting it buys nothing and destroys the only record of which account the user pinned.

**[REV 2] Correction to rev 1's asymmetry argument.** Rev 1 justified migration partly on "a
cross-host mis-migration is inert, because `resolve_account` pre-filters accounts by host". That
is wrong in the way that matters, and the audit verified all three legs:

1. `commands/forge_accounts.rs:33-40` filters `accts` to `host == host_l`, then `:47-56` finds
   the override by path and looks for `account_id` **within that filtered set**. A cross-host
   pin misses and falls through — so `accountSource` is never `'override'` for it.
2. `src/components/ForgeAccountSwitcher.tsx:100-104` (`'Use host default'`, `disabled:
   accountSource !== 'override'`) and `:124-131` (`'Reset to host default'`, only *rendered* when
   `=== 'override'`) gate **both** clearing affordances on a resolution the mis-migrated pin can
   never produce. There is no Settings pane listing `repoForgeOverrides`. The pin is therefore
   **invisible AND unremovable from the UI**; recovery means hand-editing `settings.json`.
3. It is permanent: the override now names a **Live** path, so §4 rule (1) blocks it forever.
   Re-cloning the real repo later cannot bring the pin back.

That is precisely the harm the "never delete" rule exists to prevent, reached by a *migration*.
Hence **rule (6)** in §4: positive forge-identity match, not merely a path-shape guess.

**Post-rule-6, the asymmetry argument is true as stated:** every surviving migration names an
account on the candidate's own forge host, so `resolve_account` resolves it, the switcher shows
"Pinned to this repo", and both clearing affordances are live. One unpin undoes it.

**[REV 2] Accepted residual (F2).** Two repos on the same host — a work and a personal
`github.com` clone sharing a basename, dead pin on one, live twin on the other — pass rule (6),
and the app then acts as the wrong account. This covers **writes**, not only reads
(`forge_create_pr` / merge / close resolve through the same key), which is why it is called out
rather than buried. It is accepted: the outcome is visible ("Pinned to this repo") and one unpin
fixes it, which is inside this contract's stated error budget. Caveat on the record: nothing
distinguishes a machine-made pin from a user-set one, so a user who never pinned anything has no
way to know the pin was not theirs. The closer is a `pinnedBy: "user" | "migration"` marker —
logged as a UI follow-up in §10, **not specified here**.

### 1.4 `open_repos` / `active_repo` — OUT of scope (already self-healing)

`useRepoTabs.ts` persists the **whole live session** (`persistSession` →
`ipc.setSession({ openRepos, activeRepo })`, debounced) on every `tabs`/`activeRepo` change once
`sessionReadyRef` flips after App's launch-reopen effect settles. A restore-open that fails adds
no tab, so the next persist writes the session without it; `active_repo: None` already means
"activate the first still-openable one" (`settings.rs:117`). They converge without P115.
Declared out of scope explicitly — the prune must not mutate either field. (They are still
*read* by §4 as migration candidates.)

---

## 2. Module placement

| File | Change |
|---|---|
| `src-tauri/src/settings/prune.rs` | **new** — everything in §3-§5. One concern: classify paths, prune/migrate, report. |
| `src-tauri/src/settings/prune_tests.rs` | **new** — `#[cfg(test)] #[path = …] mod tests;` from `prune.rs`, mirroring `external_tools_tests.rs`. |
| `src-tauri/src/settings.rs` | `pub mod prune;` + re-export of the pass entry points; doc edits in §8. |
| `src-tauri/src/lib.rs` | the setup call site (§6). |
| `src-tauri/src/commands/tests_repo_session_misc.rs` | comment-only edit (§8.4). |

No frontend change. No new IPC command, event, or channel — **nothing in `src/ipc/mock.ts`
changes** and the browser harness is unaffected.

---

## 3. The transient-absence gate

A bare `!exists()` is rejected: an unplugged drive or an unmounted share would destroy live
settings. A persisted "N consecutive observations" counter is also rejected — it adds a wire
field (pushing at the version-bump bar for no gain) and delays the fix by N launches, while
buying nothing the root probe below already gives.

The gate is **root-reachability first, then a NotFound ancestor chain**. The discriminator is
that a *move* leaves the volume root alive (`D:\` mounted, `D:\Repos` gone), while a *missing
volume* does not.

```rust
/// What a probe says about one persisted path. Only `ConfirmedGone` authorizes any mutation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathState {
    /// The full path resolved — the repo is there.
    Live,
    /// The volume/share root is reachable AND the first failing ancestor below it failed
    /// with `NotFound`: the directory was moved or deleted.
    ConfirmedGone,
    /// Anything else — unmounted volume, unreachable share, permission denied, rootless
    /// path. ALWAYS kept, never pruned, never migrated.
    Unreachable,
}

/// Probe seam (testability): `Ok(())` == the path exists and is reachable.
/// The production probe is `|p| std::fs::metadata(p).map(drop)`.
pub type Probe<'a> = &'a mut dyn FnMut(&Path) -> std::io::Result<()>;

pub fn classify(path: &str, probe: Probe<'_>) -> PathState;
```

```
classify(path, probe):
  p = Path::new(path)
  if probe(p).is_ok(): return Live
  root = the path's Prefix+RootDir components joined
         (Windows: "D:\", "\\\\server\\share\\"; Unix: "/")
  if path has no root component (relative / bare):  return Unreachable
  if probe(root).is_err():                          return Unreachable   // kind IGNORED here
  walk ancestors root -> leaf, probing each:
      first component whose probe fails:
          ErrorKind::NotFound -> return ConfirmedGone
          any other kind      -> return Unreachable
      (no failure found: TOCTOU, the leaf came back) -> return Live
```

**The kind is consulted only BELOW a reachable root.** The root probe rejects on *any* error
kind, deliberately: Rust's Windows `decode_error_kind` maps `ERROR_BAD_NETPATH`,
`ERROR_BAD_NET_NAME` and `ERROR_INVALID_DRIVE` to `ErrorKind::NotFound`, so a down server or a
missing drive letter *does* surface as `NotFound`. A kind-only gate without the root probe would
therefore prune an unmounted drive's entries. The root probe is what makes the kind meaningful.

| Case | Classification | Effect |
|---|---|---|
| (a) `D:\Repos\x` gone, `D:\` mounted | `ConfirmedGone` | recents + hooks-ack entry pruned; override migrated per §4 or kept |
| (b) whole drive `E:\` unmounted | `Unreachable` — root probe fails (as `NotFound`, kind ignored) | nothing touched |
| (c) network share down / timing out | `Unreachable` — root probe fails (any kind) | nothing touched |
| (d) permission denied on a component below a reachable root | `Unreachable` — first failing kind is not `NotFound` | nothing touched |

Probe cost: one `metadata` per distinct path, plus — only on the failure branch — one per
ancestor, bounded by path depth. This is blocking I/O, which is why the pass cannot live in
`load_from` and why §6 keeps it out of the `SETTINGS_IO` critical section.

---

## 4. Matching and migration rules — in precise string terms **[REV 2]**

### 4.1 The comparison split: dead side = `norm`, live side = canonical **[REV 2]**

Rev 1 justified string comparison with "every P115 comparison has a dead path on at least one
side". **That is false for rule (4)**, whose `target` is Live and whose `O2` may be Live too.
`read_repo_info` stores the **raw** opened path (`crates/bonsai-core/src/git/repo.rs:43,64` —
`to_string_lossy()`, not `workdir()`, not `canonicalize`), so two persisted strings naming one
directory are routine. `norm` folds case, separators and a trailing separator — but not
`\\?\D:\x` vs `D:\x`, not 8.3 short names, not mapped-drive vs UNC, not macOS NFC/NFD. A miss
there leaves **two overrides matching one repo** under `same_repo_path`, where `.find()` makes
**list order** decide — and a migrated entry keeps its original index, so it could shadow a pin
the user set long ago.

Therefore:

- **Dead side** (`ConfirmedGone` / `Unreachable` paths): compared by `norm`. Canonicalization is
  impossible by construction — the path is not there.
- **Live side** (candidates, and any Live `O2` in rule (4)): compared by **canonical key**
  (`std::fs::canonicalize`, computed in phase 1 where the I/O already lives). Used for the
  `live_candidates` dedupe and for rule (4).
- The canonical form is a **comparison key only**. The string written into `o.repo_path` on
  migration stays the persisted candidate string `L`, never the `\\?\`-prefixed canonical form.
- Where the two sides must be compared (a dead `O2` against a live `L` in rule (4)), the dead
  side's `norm` key is used and can only ever *block* a migration — the conservative direction.

```rust
/// Normalized comparison key for a DEAD persisted path (canonicalization is unavailable).
///   1. trim trailing `/` and `\`
///   2. #[cfg(windows)] replace `/` with `\`    (on Unix `\` is a legal filename char)
///   3. #[cfg(windows)] `to_ascii_lowercase()`  // [REV 2] F4 — see below
fn norm(path: &str) -> String;

/// Last path segment of `norm(path)` (after the final separator), or `""`.
fn basename(path: &str) -> String;

/// Comparison key for a LIVE path: `fs::canonicalize(path)`'s string form.
/// Computed in phase 1 only; `None` when canonicalization fails (then the path is
/// treated as having no canonical key and can neither dedupe nor block).
fn canon_key(path: &str) -> Option<String>;
```

**[REV 2] F4 — the case fold is now `#[cfg(windows)]`-gated**, mirroring the separator fold, and
for the same reason: rev 1 lowercased unconditionally, but on Linux and case-sensitive APFS
`/x/api` and `/x/Api` are *different directories*. Collapsing them to one key made
`collect_paths` keep the first-seen string and apply one verdict to both — recents damage
cosmetic, acks safe (one extra prompt), but for **overrides it meant a live repo's pin became
migration-eligible with no folder actually gone**, i.e. F1/F2's payload with no move at all.

Consequence on case-**insensitive** non-Windows filesystems (default APFS), stated honestly: two
persisted spellings of one directory may now produce two live candidates, because `realpath` on
macOS does not case-correct. Rule (3) then sees two candidates for the basename and declines to
migrate. That is an *under*-merge — a missed migration, the safe direction — not a wrong one.
F3's canonical live side removes the case where both spellings resolve identically.

### 4.2 Override migration rule **[REV 2 — rule (6) added]**

A dead override `O` migrates to live candidate `L` iff **all six** hold:

1. `state(O.repo_path) == ConfirmedGone` (not merely "probe failed");
2. `L` comes from `{ e.path | e in recent_repos } ∪ open_repos`, restricted to `state(L) == Live`
   and deduplicated by **canonical key**;
3. **exactly one** such `L` has `basename(L) == basename(O.repo_path)` (zero ⇒ no migration; two
   or more ⇒ ambiguous, no migration);
4. no other `repo_forge_overrides` entry `O2 != O` resolves to the same repo as `L` — compared by
   **canonical key** when `O2` is Live, by `norm` otherwise. A live pin is never overwritten.
   **[REV 2.1] The `norm` fallback here compares case-insensitively** (`eq_ignore_ascii_case`),
   which looks like it contradicts F4's `#[cfg(windows)]` gate on `norm` and does not. The two
   uses are different in kind: case folding as a **state key** is dangerous — it applies one
   verdict to two directories, which was F4 — while case folding as a **blocking predicate** is
   safe, because over-blocking only declines a migration and keeps the override. Without this,
   on a case-insensitive *case-preserving* filesystem (default APFS/HFS+) `realpath` returns each
   spelling as given, so `/x/Repos/y` and `/x/repos/y` differ under **both** branches and a second
   override can land on a repo that already has one — after which `resolve_account`'s `.find()`
   lets list order pick the account;
5. no other **dead** override shares that basename (two dead `backend` entries ⇒ both ambiguous,
   neither migrates). **[REV 2.1] "dead" here means `ConfirmedGone` only**, not
   `ConfirmedGone ∪ Unreachable` — matching rule (1), so an `Unreachable` override neither
   migrates nor blocks. §4.1's broader "dead side" wording is about which *comparison* to use
   (`norm` vs canonical), not about which states count here;
6. **[REV 2] positive forge identity.** `hosts[canon_key(L)]` is present, non-empty, and equals
   the `host` field of the `forge_accounts` record whose `account_id == O.account_id`.
   **Every failure mode declines the migration:**
   - no `forge_accounts` record with that `account_id` ⇒ **no migration** (a pin to an account
     that no longer exists has nothing to preserve; `remove_forge_account`
     (`settings/forge_accounts.rs:89-97`) already drops overrides when an account is deleted, so
     this state is a stale leftover, not a binding);
   - `canon_key(L)` absent from `hosts` (identity not resolved, or not attempted) ⇒ **no
     migration**;
   - `resolve_forge_identity` returned `Err` — including `AppError::NoRemote` for a repo with no
     `origin` ⇒ **no migration**;
   - the resolved host is the **empty string** — the documented degradation for an unparseable
     origin (`crates/bonsai-forge/src/lib.rs:212-213`) ⇒ **no migration**. An empty host must
     never be allowed to compare equal to anything.

   Host comparison is on already-lowercased values on both sides (`resolve_forge_identity`
   returns a lowercased host; `account_id`/`ForgeAccountRecord.host` are lowercased at write —
   `settings/forge_accounts.rs:69`, `:119`). Compare with `==`, not `eq_ignore_ascii_case`, so a
   future non-lowercased writer fails closed.

Effect: `O.repo_path := L` (the persisted candidate string verbatim), `O.account_id` unchanged,
list order unchanged.

**Deliberately dropped from the orchestrator's original rule:** "AND the dead path's parent
directory is itself gone". Rule (3) plus rule (6) are the real discriminators, and the parent
clause would block the legitimate "repo deleted and re-cloned elsewhere while its old parent
still exists" case without excluding any collision those rules allow.

**A dead override that does NOT migrate is left exactly as it is** — same index, same strings. It
is inert (§1.3) and it is the only surviving record of the user's pin.

Convergence: the user's file already contains the new path in `recent_repos` (the doubled
`hooksAckRepos` proves the repo was reopened at its new location), so the observed case migrates
on the first run *provided its origin still points at the pinned account's host* — which it does.
A repo not yet reopened anywhere migrates on the launch after it is.

---

## 5. The pass — two phases, probe I/O outside the lock **[REV 2 — phase-1 facts extended]**

**Invariant (reviewer-enforced): no probe I/O, and no git2/remote I/O, ever runs while
`SETTINGS_IO` is held.** A dead UNC path can block `fs::metadata` for tens of seconds; doing that
inside an `update_if` mutator would queue every other settings writer at launch (`record_recent`
from session-restore `open_repo`, `set_session`, UI-settings patches) behind it. Phase 1 gathers
all facts against a lock-free `load_from` snapshot; phase 2 takes the lock and applies them.
**Phase 2 remains pure and performs NO I/O of any kind** — rule (6)'s host lookup is a `HashMap`
read of a value phase 1 computed.

```rust
/// [REV 2] Everything phase 2 needs to know about the filesystem and the forges.
/// Built entirely in phase 1; consumed read-only by phase 2.
#[derive(Debug, Clone, Default)]
pub struct PathFacts {
    /// `norm(path)` -> state, for EVERY path `collect_paths` returned.
    pub states: HashMap<String, PathState>,
    /// `norm(path)` -> canonical key, for `Live` paths only (§4.1 live side).
    /// Absent when `fs::canonicalize` failed.
    pub canonical: HashMap<String, String>,
    /// [REV 2] canonical key -> lowercased forge host of that repo's `origin`.
    /// Populated ONLY when at least one override is `ConfirmedGone`, and only for
    /// candidates whose basename matches a dead override's (§5.1). An absent key,
    /// a resolve error, and an empty host are all "no migration" (§4.2 rule 6).
    pub hosts: HashMap<String, String>,
}

/// Every distinct path P115 must know the state of: `recent_repos[*].path`,
/// `hooks_ack_repos[*]`, `repo_forge_overrides[*].repo_path`, and `open_repos[*]`
/// (the last are migration CANDIDATES only — §1.4 never mutates them).
/// Deduplicated by `norm`; returns the original strings, one per distinct key.
pub fn collect_paths(s: &Settings) -> Vec<String>;

/// Phase 1a — all filesystem I/O, no lock held. Fills `states` + `canonical`.
pub fn classify_all(paths: &[String], probe: Probe<'_>) -> PathFacts;

/// Phase 1b — [REV 2] all forge-identity I/O, no lock held. Fills `facts.hosts`.
/// No-op (and opens no repository) unless some override is `ConfirmedGone`.
/// `resolve` is a seam; production passes
/// `|p| bonsai_forge::resolve_forge_identity(p).ok().map(|(host, _owner, _kind)| host)`.
///
/// **[REV 2.1] The seam returns `Option`, not `Result`.** Rule (6) treats every
/// `Err` identically (decline), so a `Result` would couple `prune.rs` to
/// `bonsai_forge`'s error type for no gain. Consequence to keep in mind when
/// writing tests: `Err(NoRemote)` and any other `Err` collapse to the SAME seam
/// value (`None`), so they cannot be distinguished through the seam — the
/// distinct legs have to be driven through the production wrapper instead.
pub fn resolve_candidate_hosts(
    s: &Settings,
    facts: &mut PathFacts,
    resolve: &mut dyn FnMut(&Path) -> Option<String>,
);

/// Phase 2 — pure. Applies §1 and §4 using `facts`; performs NO I/O.
/// A key ABSENT from `facts.states` is treated as `Live`: never mutate a path that
/// was not probed (covers entries added between the two phases).
/// Mutates `recent_repos`, `hooks_ack_repos`, and `repo_forge_overrides[*].repo_path`
/// only. NEVER touches `open_repos` or `active_repo`.
pub fn prune_stale_paths(s: &mut Settings, facts: &PathFacts) -> PruneReport;

/// What one pass changed. Counts only, no paths — the caller decides what to log.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PruneReport {
    pub recents_pruned: usize,
    pub hooks_acks_pruned: usize,
    pub overrides_migrated: usize,
    /// Dead overrides deliberately left in place (ambiguous, no candidate, or
    /// [REV 2] rule (6) declined).
    pub overrides_kept_dead: usize,
    /// Distinct paths classified `Unreachable` — the "we did nothing on purpose"
    /// counter, so a log line can tell "nothing stale" from "the drive was offline".
    pub unreachable_skipped: usize,
}

impl PruneReport {
    /// `true` iff the pass mutated `Settings` — the `update_if` return value.
    pub fn changed(&self) -> bool;
}
```

### 5.1 Phase 1b — candidate host resolution **[REV 2]**

```
resolve_candidate_hosts(s, facts, resolve):
  dead_basenames = { basename(o.repo_path)
                     | o in s.repo_forge_overrides,
                       facts.state(o.repo_path) == ConfirmedGone }
  if dead_basenames.is_empty(): return          // fast path: opens NO repository

  for p in distinct-by-canonical(live candidates from recent_repos ++ open_repos):
      if basename(p) not in dead_basenames: continue   // rule (3) could never pick it
      match resolve(Path::new(p)):
          Ok(host) if !host.is_empty() -> facts.hosts.insert(canon_key(p), host)
          _                            -> (insert nothing — rule (6) then declines)
```

Two narrowings, both deliberate: the whole phase is skipped when no override is `ConfirmedGone`
(the overwhelmingly common launch opens zero repositories), and within it only basename-matching
candidates are resolved (rule (3) can never select any other), so the typical dead-override case
costs **one** git2 open.

### 5.2 Phase 2 pseudocode **[REV 2 — rule (6) in the migration branch]**

```
prune_stale_paths(s, facts):
  state(p) = facts.states.get(norm(p)).copied().unwrap_or(Live)
  key(p)   = facts.canonical.get(norm(p))            // Some(..) only for Live paths

  live_candidates = distinct-by-key([ p in (s.recent_repos[*].path ++ s.open_repos)
                                      if state(p) == Live ])

  // overrides — never deleted
  for o in s.repo_forge_overrides:
      if state(o.repo_path) != ConfirmedGone: continue
      target = the unique L in live_candidates satisfying rules (3),(4),(5)
      migrate = match target:
          Some(L) => rule6_ok(L, o, s, facts)        // §4.2 rule (6)
          None    => false
      if migrate: o.repo_path = L; report.overrides_migrated += 1
      else:       report.overrides_kept_dead += 1

  before = s.recent_repos.len()
  s.recent_repos.retain(|r| state(r.path) != ConfirmedGone)
  report.recents_pruned = before - s.recent_repos.len()

  before = s.hooks_ack_repos.len()
  s.hooks_ack_repos.retain(|p| state(p) != ConfirmedGone)
  report.hooks_acks_pruned = before - s.hooks_ack_repos.len()

  report.unreachable_skipped = facts.states.values().filter(|st| **st == Unreachable).count()
  return report

rule6_ok(L, o, s, facts):
  acct = s.forge_accounts.iter().find(|a| a.account_id == o.account_id)?   // None => false
  host = facts.hosts.get(key(L)?)?                                        // None => false
  !host.is_empty() && *host == acct.host
```

Ordering inside phase 2: migrate overrides, then prune recents, then prune acks. Migration reads
only `Live` entries and the prune removes only `ConfirmedGone` ones, so the order cannot change
the result; fix it anyway for deterministic tests.

### 5.3 `classify_all` — roots first **[REV 2, INFO-2]**

`classify_all` probes the **distinct roots** of all collected paths once, up front. Any path
whose root probed `Err` is short-circuited to `Unreachable` **without probing the full path**.

**[REV 2.1] This is CONSERVATIVE, not equivalent — rev 2 overstated it.** The claim used to read
"`Unreachable` regardless of what the full-path probe would have said, because the root check
precedes the kind check". That is not what `classify` does: its **first** step is the full-path
probe, which returns `Live` on success *before* the root is ever consulted. So a path whose root
rejects `stat` while the path itself is reachable — an SMB share root or DFS link denying
`FILE_READ_ATTRIBUTES` while a subdirectory is readable — is `Live` under a bare `classify` and
`Unreachable` under roots-first.

Why this is still correct, stated as a direction rather than an equivalence: **the divergence can
only ever produce `Unreachable`, never `ConfirmedGone`**, and only `ConfirmedGone` authorizes a
mutation (§1). Roots-first can therefore cause the pass to do *less*, never to do something wrong.
The one second-order effect is that a vanished candidate can turn a rule-(3) ambiguity into a
unique match — and that match must still clear rule (6).

Anyone relying on a literal equivalence here (e.g. to reorder the probes, or to drop the
short-circuit) must re-derive it; it does not hold.

Why it matters: without it, a user with N dead `\\server\…` entries pays the full multi-second
network timeout **N times on every launch** — and `hooks_ack_repos`, `repo_forge_overrides` and
`open_repos` are uncapped. With it, N entries under one dead root cost one slow probe.

**Idempotence.** `classify` is deterministic for a fixed filesystem. After a pass no
`ConfirmedGone` path remains in `recent_repos` or `hooks_ack_repos`, and every migrated override
names a `Live` path, failing rule (1) next run. A kept dead override re-evaluates rules (3)-(6)
to the same answer against the same live set and the same origins, so it is not migrated. A
second immediate pass returns `PruneReport::default()`, `changed() == false`, and `update_if`
skips the write (AC10).

**The one accepted race:** a path `ConfirmedGone` in phase 1 and re-created before phase 2 is
pruned once; reopening the repo re-adds it via `record_recent`. Nothing is lost.

---

## 6. Call site and threading

**App `setup`, not `get_recent_repos`.** Decisive reason is latency, not purity: `fs::metadata`
on a dead UNC path can block for tens of seconds on Windows, and inside `get_recent_repos` that
would freeze the no-repo empty state — the exact screen the recents list is on. In `setup` it is
fire-and-forget off the UI path.

Placement in `src-tauri/src/lib.rs`, inside `.setup(|app| { … })`, **after** the existing
`Ok(file) => { … }` settings block (i.e. after `apply_dev_settings`, ~line 120) and before the
metrics block:

```rust
// P115: one-shot prune of settings paths whose repo is confirmed gone. Two phases so
// the blocking fs + git2 I/O never runs under SETTINGS_IO (§5 invariant). Non-fatal
// throughout: an unresolvable settings path or a failed save leaves the file as it is.
if let Ok(file) = settings::settings_file(&handle) {
    tauri::async_runtime::spawn_blocking(move || {
        use crate::settings::prune;
        let snapshot = settings::load_from(&file);              // lock-free pure read
        let paths = prune::collect_paths(&snapshot);
        let mut facts =
            prune::classify_all(&paths, &mut |p| std::fs::metadata(p).map(drop));
        if !facts.states.values().any(|st| *st == prune::PathState::ConfirmedGone) {
            return;                                             // common launch: lock never taken
        }
        // [REV 2] phase 1b — opens no repository unless an OVERRIDE is dead.
        prune::resolve_candidate_hosts(&snapshot, &mut facts, &mut |p| {
            bonsai_forge::resolve_forge_identity(p).ok().map(|(host, _owner, _kind)| host)
        });
        let mut report = prune::PruneReport::default();
        let saved = settings::update_if(&file, |s| {
            report = prune::prune_stale_paths(s, &facts);
            report.changed()
        });
        if saved.is_ok() {
            prune::note_prune(&report);                         // never log a prune that failed to save
        }
    });
}
```

- `update_if` (not `update`) so a no-change launch never rewrites `settings.json`; it holds
  `SETTINGS_IO` across load→mutate→save, so a concurrent `record_recent` cannot be lost. The
  mutator is pure, so the lock is held for map lookups and two `retain`s.
- One shot per launch. No retry, no scheduler job.
- **Race, accepted:** the frontend's first `getRecentRepos` may land before the pass finishes and
  show the unpruned list once; any later `refreshRecents` shows the pruned one. A
  `recents-changed` event would close it but is not worth an IPC surface for once-per-launch
  cosmetic staleness — **SHOULD, deferred** (§10).

**Relation to the existing client-side prune** (`src/hooks/useRepoTabs.ts:121`): that one is
*reactive* — an `io` error at open time calls `removeRecentRepo(path)` for the single path the
user just clicked. P115 is the *proactive bulk complement* at launch, for entries the user never
clicks again (exactly how five of them accumulated). Neither replaces the other; do not remove or
change the client prune.

---

## 7. Reporting — log only, NO UI surface

Decision: **`eprintln!`, one line, on change only. No obs record, no toast, no banner.
`ui-designer` is NOT required for P115.**

- Precedent is `note_if_dropped` (`settings/external_tools.rs:121`): `eprintln!` is this crate's
  facade for non-fatal diagnostics.
- The obs sink *is* up at this call site (dev-mode `apply_dev_settings` runs earlier in the same
  `setup`, `lib.rs:101-106`), so the `external_tools.rs` reason for avoiding it does not apply.
  It is still declined: `LogPayload` (`obs/record.rs:195+`) is a closed typed enum, so an obs
  event means a new schema variant plus redaction review for the paths it would carry —
  disproportionate for a once-per-launch housekeeping line.
- **No UI is needed because the policy removes the loss.** §1.3 never deletes an override, so
  P115 loses nothing a user could want back. Recents pruning is self-evident (the list the user
  is looking at *is* the report). The remaining gaps — a dead override kept but not migrated, and
  F2's machine-made pin being indistinguishable from a user's — are **deliberate follow-ups**
  (§10), not P115.

```rust
/// One line, only when something changed. Counts only — NEVER a path (a settings path
/// carries the OS account name, and obs redaction is not in play here).
pub fn note_prune(report: &PruneReport);
// e.g. "bonsai: settings housekeeping — pruned 5 recent repo(s) and 2 hook
//       acknowledgement(s) whose folder is gone, moved 1 forge-account pin to the
//       repo's new location, kept 0 pin(s) whose new location is ambiguous"
```

---

## 8. Doc and test updates this design forces

1. **`settings.rs:444-447`, `record_recent`'s doc.** Today it promises "no entry is ever silently
   dropped" for unresolvable paths. Scope it and cross-reference:
   > `record_recent` itself never drops an entry: its dedupe falls back to an ASCII-case compare
   > when a path cannot be canonicalized. Dropping stale entries is the separate, explicit job of
   > [`settings::prune::prune_stale_paths`] (P115), which runs once at app setup, removes only
   > paths it can prove are gone (§3 gate), and logs what it removed.
2. **`settings.rs:465-478`** — one-line pointer on `hooks_ack_contains` / `set_hooks_ack`: the
   list is pruned of confirmed-gone paths by `prune_stale_paths`, and acks are never migrated
   between paths.
3. **`settings/forge_accounts.rs:136-154`** — one-line pointer on `set_repo_override` /
   `clear_repo_override`: P115 may rewrite `repo_path` in place, never removes an override, and
   only when the new location's `origin` host matches the pinned account's host (§4.2 rule 6).
4. **`src-tauri/src/commands/tests_repo_session_misc.rs:~250-257`** — **correction to the
   milestone brief: this test does NOT break.** It exercises `record_recent` and the
   `remove_recent_repo` body directly and never invokes the pass, so the assertion that
   `D:\Repos\beta` survives stays green and must not be weakened. Only its comment changes, to
   say what it now proves: *`record_recent` and `remove_recent_repo`* keep a non-existent path;
   the app-level lifetime of such an entry is the P115 pass's business.
5. `docs/contracts/INDEX.md` — row added (see `INDEX.md.p115insert`; this agent has `Write` only).

---

## 9. Acceptance criteria

All Rust-side, driven through the injected probe and resolver seams — no test needs a real
unmounted drive or a network remote. Fixture: a `settings.json` matching the 2026-09-22 evidence
(5 recents under `D:\Repos\*`, one override on `D:\Repos\ham-digi-backend`, `hooksAckRepos`
carrying both `D:\Repos\ham-digi-backend` and `D:\Data\Repos\ham-digi-backend`), with a probe
reporting `D:\` and `D:\Data\Repos\*` as `Ok` and every `D:\Repos\*` component as `Err(NotFound)`,
and a resolver reporting `github.com` for the live candidate.

### Rev 1 criteria (unchanged unless marked)

1. **AC1 — recents pruned.** All 5 `D:\Repos\*` recents are removed; every `D:\Data\Repos\*`
   entry survives with `last_opened` unchanged and in the same relative order.
2. **AC2 — forward-slash entry.** The `D:/Repos/.worktrees/...` recents entry is classified and
   pruned by the same rule as its backslash siblings (`norm` folds the separator on Windows).
3. **AC3 — override migrated.** The override's `repo_path` becomes
   `D:\Data\Repos\ham-digi-backend` (the exact persisted string, **not** a `\\?\` canonical
   form); `account_id` byte-identical; `repo_forge_overrides.len()` unchanged.
4. **AC4 — ambiguity blocks migration.** With two live candidates named `backend`, the dead
   `…\backend` override is untouched, `overrides_migrated == 0`, `overrides_kept_dead == 1`.
5. **AC5 — a live pin is never overwritten.** When the live candidate already has its own
   override, the dead one is left untouched (rule 4).
6. **AC6 — no override is ever deleted.** For every fixture and every gate outcome,
   `repo_forge_overrides.len()` after == before.
7. **AC7 — hooks acks.** The dead `D:\Repos\ham-digi-backend` ack is removed, the live one
   survives, and no ack is added to any path. **[REV 2]** The ⊆ invariant
   (`hooks_ack_repos` after ⊆ before, as a set of `norm` keys) is asserted as a **shared property
   helper run on EVERY fixture in this section**, not only on the evidence one — the audit found
   it pinned to a single case.
8. **AC8 — gate: unmounted volume.** Probe returns `Err(NotFound)` for both `E:\` and
   `E:\repos\x`: nothing pruned, nothing migrated, `unreachable_skipped >= 1`. (Guards the "a
   kind-only gate would have pruned this" failure.)
9. **AC9 — gate: permission denied / timeout below a live root.** Root `Ok`, first failing
   component `Err(PermissionDenied)` — and a second case `Err(TimedOut)`: `Unreachable`, nothing
   mutated.
10. **AC10 — idempotence.** Two consecutive passes: the second returns `PruneReport::default()`,
    `changed() == false`, and `Settings` is `==` to the post-first-run value.
11. **AC11 — no write on a clean file.** With all paths `Live`, `classify_all` yields no
    `ConfirmedGone`, `update_if` is never called, and `settings.json`'s mtime is unchanged.
12. **AC12 — session untouched.** `open_repos` and `active_repo` are byte-identical before and
    after. **[REV 2]** The audit found the "even when they name `ConfirmedGone` paths" clause was
    never exercised — every fixture used live or unreachable session paths. AC12 now **requires a
    fixture whose `open_repos` and `active_repo` both name `ConfirmedGone` paths**, asserted
    byte-identical after the pass.
13. **AC13 — probe economy.** For a settings file repeating one path across all three lists,
    `collect_paths` returns it once and it is probed once. **[REV 2]** Also assert each distinct
    **root** is probed at most once, and that N paths under one dead root cost exactly one root
    probe and zero full-path probes (§5.3). Also assert `collect_paths` includes `open_repos`.
14. **AC14 — absent key is Live.** `prune_stale_paths` with an empty `PathFacts` mutates nothing
    and returns `PruneReport::default()`.
15. **AC15 — version.** `version` stays `1` in the written file; it round-trips through
    `load_from` with no other field changed.
16. **AC16 — non-fatal.** A `save_to` failure (read-only settings file) does not panic, does not
    prevent setup from completing, and suppresses the `note_prune` line.
17. **AC17 — existing tests.** `tests_repo_session_misc.rs`'s "non-existent path survives"
    assertion still passes unmodified; the `settings*` test count rises only by the new
    `prune_tests` cases.

### Rev 2 criteria **[REV 2]**

18. **AC18 — cross-host candidate blocks migration.** Dead pin's account host `github.com`, live
    candidate's resolver returns `gitlab.com`: `overrides_migrated == 0`,
    `overrides_kept_dead == 1`, the override's `repo_path` is byte-identical to before.
19. **AC19 — missing account record blocks migration.** The override's `account_id` matches no
    `forge_accounts` entry: no migration, and the override survives unchanged (never deleted —
    AC6 still holds).
20. **AC20 — resolver failures block migration.** Three cases, each independently:
    `Err(AppError::NoRemote)`; any other `Err`; and `Ok("")` (the documented unparseable-origin
    degradation). All three ⇒ no migration, override unchanged. The empty host must not compare
    equal to an account whose host is also somehow empty — assert explicitly.
21. **AC21 — same-host match still migrates.** The evidence case end to end: resolver returns
    `github.com`, the pinned account's `host` is `github.com` ⇒ migrated (this is AC3's
    precondition, asserted through the `hosts` map rather than assumed).
22. **AC22 — rule (6) fast path.** With recents/acks dead but **no** override dead,
    `resolve_candidate_hosts` invokes the resolver **zero** times (assert the call count), and
    with one dead override it is invoked only for basename-matching candidates.
23. **AC23 — case-only siblings on a case-sensitive FS.** `#[cfg(unix)]`: `/x/api` and `/x/Api`
    produce two distinct `norm` keys, get independent verdicts, and a dead `/x/Api` never causes
    any mutation touching `/x/api`. Companion `#[cfg(windows)]` test asserting the same two
    strings *do* collapse to one key.
24. **AC24 — rule (4) across two spellings of one directory.** Two overrides whose `repo_path`
    strings differ (e.g. `D:\a\repo` and its 8.3 or `\\?\` form) but canonicalize to the same
    live directory: the dead one does not migrate onto it. Fixture may inject `canon_key` via the
    phase-1 seam rather than requiring a real 8.3 name.
25. **AC25 — purity.** `prune_stale_paths` is called with a probe/resolver that **panics if
    invoked**, proving phase 2 performs no I/O.

USER CHECKPOINT: none owed beyond the orchestrator's own run — the observable effect (five stale
recents gone at next launch) is verifiable from the settings file. A user holding the evidence
file can confirm the migrated pin shows as "Pinned to this repo" in `ForgeAccountSwitcher` with
both clearing affordances enabled (which, post-rule-6, is now a guaranteed property of every
surviving migration — see §1.3).

---

## 10. Open, deliberately

- **`recents-changed` event** (§6): left OUT. Affects only a once-per-launch first paint. Taking
  it turns P115 into an IPC change (`src/ipc/mock.ts` + `useRepoTabs.ts`) and a second increment.
- **Surfacing kept-dead overrides** (§7): left OUT. A dead pin that never migrated is invisible
  in the UI (there is no pane listing `repoForgeOverrides`). Retention, not loss — but a
  Settings → Forge accounts row marking pins whose repo is missing would close it.
  `ui-designer` first if wanted.
- **[REV 2] `pinnedBy: "user" | "migration"` marker** (F2): left OUT, and it is the closer for
  the accepted same-host residual — a user who never pinned anything currently has no way to tell
  a machine-made pin from their own. Adds a wire field to `RepoForgeOverride` (still additive,
  still no version bump) plus switcher copy. UI follow-up.
- **[REV 2] Launch cost of uncapped lists** (audit INFO-2): §5.3's root-first probing collapses N
  dead entries under one dead root to a single slow probe, which is the cheap 90% fix and is
  **specified, not deferred**. What remains open: `hooks_ack_repos`, `repo_forge_overrides` and
  `open_repos` have no cap at all, so a pathological file with many dead roots still pays per
  root, every launch. A cap (or a persisted "last pruned" stamp to skip launches) is the next
  step if it is ever observed.
