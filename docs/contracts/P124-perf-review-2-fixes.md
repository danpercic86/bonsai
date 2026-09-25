# P124 — Fixes from the second /perf-review (session `s1d3f226f`)

Evidence (read-only, do NOT redo): `D:\Data\Temp\bonsai-perf-review\s1d3f226f.md`; frozen log
`D:\Data\Temp\bonsai-perf-review\s1d.jsonl` (grep only). Board: `TODO.md` `## P124`.
Builds on `docs/contracts/P121-perf-review-fixes.md` (P121a TTL/coalescing, P121d ignore sampling).

**No new IPC commands, events or channels. No payload changes.** One obs value added
(watcher record `suppressReason: "ignored"`, Rust-emitted). Mock IPC changes: P124d only.

**Order by value:** a → b → c → d. **P124a2 (batched CI lookup) is recommended DEFERRED.**

| Pass | Sub-increments | Files (disjoint) |
|---|---|---|
| 1 | P124a + P124d | `src/components/repoWorkspace/useForgeSignals*`, new `forgeCiCache.ts`; new `src/ipc/focusGate.ts`, `src/ipc/tauri/app.ts`, `src/ipc/mock/handlers/session.ts`, `src/ipc/types/ipc-api.ts` (doc) |
| 2 | P124b + P124c | `crates/bonsai-core/src/git/ignore.rs`, `src-tauri/src/watcher/*`, `src-tauri/src/obs/record.rs` + `src/obs/types.ts` (docs); `RepoWorkspace.tsx`, new `repoWorkspace/useDiffSlot.ts`, `e2e/perf/interactions*.ts`, `scripts/perf-baseline.json`, `scripts/file-size-baseline.json` |

**StrictMode in vitest:** off by default (P121 convention); all tests below are non-StrictMode.

**P120 lab:** ONE new interaction, `background-refresh-diff-open` (P124c) — the only
lab-reproducible item (a needs network, b real notify, d the Tauri double focus). Its
`reactCommits` ceiling is **hand-added** (= 2, as `background-refresh`) so the lab fails today;
other metrics are seeded by `pnpm lint:perf -- --update-baseline`. No interaction-count assert
exists; update the "seven scripted interactions" comments (`interactions.ts:2`,
`interactions.spec.ts:2`) to eight. An existing ceiling that falls → regenerate deliberately.

---

## P124a — `none` CI verdicts never settle (error)

### Root cause (verified)

- No statuses/checks ⇒ `CheckRollup::None` (`crates/bonsai-forge/src/rollup.rs:72-73`, all
  providers). `ciTtlFor` (`useForgeSignals.ts:60-71`) gives `none` the 60 s `TTL_MS`, so every
  non-forced trigger (focus `RepoWorkspace.tsx:1020`; its `full` round re-streams the graph →
  `refresh('graph')` `:1043`) re-requests every `none` tip > 60 s old, forever (log `02e12749`
  1475→2901, 14667→15882; `e18d503e` 30541→33086).
- `none` is transient right after a push (CI not registered yet); a push is already a **forced**
  refresh (`bumpForgeAndChecks`, `:1088-1091`).
- `GraphNode.committerTs` exists (`src/ipc/types/graph.ts:25`) but not for open-PR head shas, and
  committer age ≠ push age — see Q1. `useForgeSignals.ts` is 402 lines → extract pure helpers first.

### Decision — streak rule ("settled `none`")

A `none` verdict becomes long-lived once observed as `none` at two points ≥ `NONE_SETTLE_MS` apart;
the unsettled TTL is stretched so the second observation lands exactly then. Result: exactly one
re-check per `none` sha, then 30 min. Forced paths (manual, fetch/pull/push, enable) still bypass
every TTL. P113a untouched (`isForgeSuppressed` per run; `noteForgeRateLimit` on
`stoppedBy`/catch). The forced-rerun fold (`useForgeSignals.ts:337`) stays (P121a test 3).

### Design — new `src/components/repoWorkspace/forgeCiCache.ts` (PURE)

Move verbatim from `useForgeSignals.ts`: `TTL_MS`, `TTL_TERMINAL_MS`, `CiEntry` (now exported),
`branchTipShas`, `isFresh`, `ciTtlFor`, `collectCiShas`, `rebuildCiCache`. The hook imports them by
name (no re-export from the hook); `useForgeSignals.test.ts` imports from `./forgeCiCache`.

```ts
import type { CheckRollup, CommitStatus, GraphLayout } from '../../ipc';
import type { CiBadge } from '../../graph/forgeBadges';

export const TTL_MS = 60_000;
export const TTL_TERMINAL_MS = 30 * 60_000;
/** P124a: a `none` observed across at least this span is treated as terminal. */
export const NONE_SETTLE_MS = 2 * 60_000;

export interface CiEntry {
  badge: CiBadge;
  /** When this verdict was fetched (batch landing time). */
  tsMs: number;
  /** P124a: `tsMs` of the FIRST observation in the current unbroken run of `none`
   *  verdicts for this sha; null when the verdict is not `none`. */
  noneSinceMs: number | null;
}

/** SIGNATURE CHANGE: takes the entry (was: the rollup). PURE.
 *  success|failure|error|neutral → TTL_TERMINAL_MS;  pending → TTL_MS
 *  none: span = e.tsMs - (e.noneSinceMs ?? e.tsMs)
 *        span >= NONE_SETTLE_MS ? TTL_TERMINAL_MS : max(TTL_MS, NONE_SETTLE_MS - span)
 *  (span is measured between OBSERVATIONS, never against `now`: a sha is re-checked once,
 *   ≥ NONE_SETTLE_MS after first sight, before it settles). */
export function ciTtlFor(entry: CiEntry): number;

/** SIGNATURE CHANGE: `ttlOf: (entry: CiEntry) => number`.
 *  skip rule: !force && entry !== undefined && isFresh(entry.tsMs, now, ttlOf(entry)) */
export function collectCiShas(
  tips: readonly string[], prHeadShas: readonly string[],
  cached: ReadonlyMap<string, CiEntry>, now: number,
  ttlOf: (entry: CiEntry) => number, force: boolean, max: number,
): string[];

/** Unchanged contract (replace-not-merge, P113a cut-short semantics) PLUS the streak, per fetched s:
 *    p = prev.get(s.sha)
 *    noneSinceMs = s.state !== 'none' ? null
 *      : (p !== undefined && p.badge.rollup === 'none' && p.noneSinceMs !== null) ? p.noneSinceMs : stamp
 *  Carried-over entries keep all fields. */
export function rebuildCiCache(
  prev: ReadonlyMap<string, CiEntry>, currentSet: ReadonlySet<string>,
  requested: ReadonlySet<string>, fetched: readonly CommitStatus[], stamp: number,
): Map<string, CiEntry>;

export function branchTipShas(layout: GraphLayout): string[];               // unchanged
export function isFresh(tsMs: number, now: number, ttlMs: number): boolean; // unchanged
```

Hook: only imports change (the `collectCiShas(..., ciTtlFor, ...)` call already passes the
function). `clearAll` on repo switch/disable drops the streak. Target hook ≤ 280 lines.

Per repo: boot batch (`none` #1, TTL 2 min) → first non-forced trigger ≥ 2 min later re-requests
it (`none` #2 → settled) → nothing until 30 min → one batch finds `none` again (streak carried,
stays settled). Steady `pending` tips still poll at 60 s, and only those.

### Files

| File | Change |
|---|---|
| `src/components/repoWorkspace/forgeCiCache.ts` (new) | Pure helpers + streak. |
| `src/components/repoWorkspace/useForgeSignals.ts` | Imports; moved code removed. |
| `src/components/repoWorkspace/useForgeSignals.test.ts` | Import path; `ciTtlFor(entry)` call sites; new cases. |
| `src/components/repoWorkspace/useForgeSignals.none.test.tsx` (new) | Hook tests. |

### Failing tests (fail today)

1. **Hook, fake timers** (harness as `useForgeSignals.coalesce.test.tsx`): `forgeCommitStatuses`
   returns `state:'none'` for all 3 tips. Mount, +300 ms, flush → 1 call. +61 s,
   `refresh('focus')`, +300 ms → **still 1** (today 2). At mount + 2 min 1 s, `refresh('graph')` →
   2 (the one re-check). +3 min, `refresh('focus')` → **still 2** (today 3). At call-2 time +
   30 min + 1 s, `refresh('graph')` → 3.
   **Divergence from the report's check** (1 call after +2 min): that requires settling on first
   sight, which the streak rule rejects (post-push `none` would stick 30 min) — Q1.
2. After settling: `refresh('manual', true)` → +1 call.
3. Mock returns `pending` always: each non-forced trigger > 60 s apart → +1 call (guards the rule).
4. **Pure:** `rebuildCiCache` none(since S) → none ⇒ `S`; none → success ⇒ `null`; success → none
   ⇒ `stamp`; absent → none ⇒ `stamp`. `ciTtlFor({none, tsMs:T, noneSinceMs:T})` =
   `NONE_SETTLE_MS`; `{tsMs:T, noneSinceMs:T-90_000}` = `TTL_MS`;
   `{tsMs:T, noneSinceMs:T-NONE_SETTLE_MS}` = `TTL_TERMINAL_MS`.

### Acceptance

- A `none` sha is requested by non-forced triggers at most twice per 30 min per hook instance.
- Manual refresh, fetch/pull/push, enable/repo switch still refetch everything.
- `useForgeSignals.backoff.test.tsx` and `.coalesce.test.tsx` pass unchanged. No IPC/mock change.

---

## P124a2 — batched CI lookup (P121 Q4) — recommend DEFER

- **GitHub GraphQL would not help this user.** Verified read-only: the three evidence repos
  (`ham-digi-web`, `HamiltonSharedComponents`, `ham-digi-backend`) have `origin` on
  `dev.azure.com` (`.git/config`); the other three open repos were not checked.
- **Azure equivalent:** `POST …/_apis/git/repositories/{repo}/commitsbatch?includeStatuses=true`,
  body `{ "ids": [...] }` (documented; `GitCommitRef.statuses`). Today `azure/mod.rs:263-274` does
  one GET per sha (~90 ms → ~9 s per 100, at the 10 s slow-command cap). Live-probe unknowns:
  (1) does one unknown id (unpushed local tip) 404 the whole batch? (2) all statuses or
  latest-per-context? (3) is `includeStatuses` honoured with `ids` ("may not be combined")?
- The probe needs the user's PAT; the delegation rules forbid a subagent doing it.

**Recommendation:** ship P124a, re-measure. If still a top cost, the USER runs the three probes and
a follow-up contract designs `AzureProvider::commit_statuses` over `commitsbatch` (bonsai-forge
internal, no IPC change, P113a partial-result semantics per chunk).

---

## P124b — all-gitignored watcher bursts drive refreshes (warn)

### Root cause (verified)

- `classify` returns `Worktree` for everything outside `.git`, ignored output included
  (`watcher/classify.rs:33`); the debounce loop always calls `on_change` (`watcher/mod.rs:320`).
  Sampling + `ignore_stats` run only with the obs sink active (`mod.rs:249`, `:315-318`).
- **notify 8.2.0 on Windows gives NO overflow signal** (crate source, `windows.rs`): the completion
  ignores `bytes_written`, the 16 KiB buffer is freshly zeroed per request, Action 0 emits nothing ⇒
  an overflowed completion yields **zero events**; no `Flag::Rescan`/`EventKind::Other`. (The
  non-success error branch — reportedly unwatch — is UNVERIFIED; irrelevant here.)
- So the P121d hazard is: a tracked event lost in one overflowed completion **and** an ignored
  write in a later completion of the same 300 ms window. Today that burst fires and self-heals; a
  drop defers it to the next firing burst / focus / manual refresh. Overflow needs a storm.
- Unnamed in P121d: `is_path_ignored` checks RULES only; a force-added tracked file under an ignore
  rule reports ignored yet changes status. Droppable ⇔ ignored **and not in the index**.

### Decision — drop only small, fully-ignored, untracked bursts, with a heartbeat

All modes. Debounce 300 ms, focus rescan and manual refresh unchanged (the safety net).

```
drop ⇔ ALL of:
  !acc.refs                                  // no Refs path, no notify error tick
  acc.worktree_total > 0
  acc.paths <= DROP_MAX_PATHS                // raw notify paths incl. .git noise; storm proxy
  acc.worktree_total as usize == acc.worktree_paths.len()   // uncapped sample
  stats.all_relative                         // every sampled path has a NON-EMPTY rel form
  stats.ignored_untracked == Some(acc.worktree_total)
  since_last_fire < IGNORED_HEARTBEAT        // bounded staleness
else fire (today's behaviour)
```
`DROP_MAX_PATHS: u32 = 128` (21 of 452 fired bursts in the log had ≥ 100 worktree paths).
`IGNORED_HEARTBEAT = 5 min` since the last `on_change` (initialised at spawn). Every failure fires:
repo open / index read failure, unclassifiable path, submodule-internal path.

**Cost:** the git check runs only for drop-eligible bursts (first four conditions) or with the obs
sink active (keeps P121d's measurement): one `open_ext(NO_SEARCH)` + index read + ≤ 128 lookups
(~4/min in the evidence session) on the debounce thread. Keep P121d's per-call open — a cached
`Repository` would need invalidation for `core.excludesFile` / `info/exclude`; not worth it.

### Design — core (`crates/bonsai-core/src/git/ignore.rs`)

```rust
/// `ignored` = matches an ignore rule; `ignored_untracked` = ignored AND absent from the
/// index at every stage 0..=3 AND no ancestor component is a gitlink (mode 0o160000)
/// index entry — a submodule-internal path never counts as droppable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IgnoreCount { pub ignored: u32, pub ignored_untracked: u32 }

/// Replaces `count_ignored`. None if the repo cannot be opened (NO_SEARCH) or its index
/// cannot be read. An EMPTY rel path counts as not ignored. `/`-normalised lookup (as today).
/// Blocking; debounce thread only.
pub fn count_ignored_paths(workdir: &Path, rel_paths: &[PathBuf]) -> Option<IgnoreCount>;
```

### Design — watcher (`src-tauri/src/watcher/`)

New `burst.rs` (moves `BurstAcc` unchanged, `IGNORE_SAMPLE_CAP`, `IgnoreStats`, `ignore_stats`):
```rust
pub(super) const DROP_MAX_PATHS: u32 = 128;
pub(super) const IGNORED_HEARTBEAT: Duration = Duration::from_secs(300);

pub(super) struct IgnoreStats {
    pub worktree_paths: u32,             // acc.worktree_total
    pub ignore_checked: Option<u32>,     // rel paths checked; None if counting failed
    pub ignored: Option<u32>,
    pub ignored_untracked: Option<u32>,
    pub all_relative: bool,
}
/// Blocking. None when acc.worktree_total == 0.
pub(super) fn ignore_stats(workdir: &Path, acc: &BurstAcc) -> Option<IgnoreStats>;
/// The four git-free drop conditions (decides whether the check is worth running).
pub(super) fn drop_eligible(acc: &BurstAcc) -> bool;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum BurstVerdict { Fire, DropIgnored }
/// PURE — the full rule above.
pub(super) fn burst_verdict(acc: &BurstAcc, stats: Option<&IgnoreStats>,
                            since_last_fire: Duration) -> BurstVerdict;
```
`mod.rs`: the notify callback samples `Worktree` paths **always** (remove the gate at `:249`) but
stops cloning at `IGNORE_SAMPLE_CAP` per tick; `WatchTick` gains `worktree_count: u32` (all
Worktree paths, past the cap) which `BurstAcc::absorb` adds to `worktree_total` — so a huge tick
still reads as capped ⇒ never dropped. At fire:
```
stats   = if drop_eligible(&acc) || active_sink().is_some() { ignore_stats(&workdir, &acc) } else { None }
verdict = burst_verdict(&acc, stats.as_ref(), last_fire.elapsed())
log_watcher(acc.paths, acc.relevant, fired=true, Some(class), stats, suppressed = verdict == DropIgnored)
if verdict == Fire { on_change(class); last_fire = Instant::now() }
```
- `log_watcher` gains `suppressed: bool`; a drop logs `suppressed: true, suppress_reason:
  Some("ignored")`. `obs/anomaly/stability.rs:176` already treats a suppressed firing as "no round".
  `worktreePaths/ignoreChecked/ignored` keep their P121d meaning; no new obs field.
- `obs/record.rs:226-245`: rewrite the doc — `suppressed`/`suppressReason` are set by the frontend
  (`"echo"`) AND the Rust watcher (`"ignored"`). Grep `tests_kind_parity.rs` and
  `.claude/agents/perf-reviewer.md` for anything asserting "Rust never sets it"; fix fixtures only.
  `src/obs/types.ts:106-107` already has `suppressReason?: string` (doc only).

### Files

| File | Change |
|---|---|
| `crates/bonsai-core/src/git/ignore.rs` | `IgnoreCount`, `count_ignored_paths`; tests. |
| `src-tauri/src/watcher/burst.rs` (new) | Acc, stats, eligibility, verdict. |
| `src-tauri/src/watcher/mod.rs` | Always sample (capped); verdict wiring; `last_fire`. ≤ 280 lines. |
| `src-tauri/src/watcher/tests.rs`, `tests_ignore.rs` | Updated + new tests. |
| `src-tauri/src/obs/record.rs`, `src/obs/types.ts` | Doc comments. |

### Failing tests (fail today)

1. **Pure `burst_verdict`** (`tests_ignore.rs`): small all-ignored-untracked burst, fresh
   `last_fire` → `DropIgnored`. Each → `Fire`: one non-ignored path; one ignored-but-tracked path;
   `refs`; error tick; `paths = 129`; capped sample (one 600-path tick); a path equal to the
   workdir root; `stats = None`; `ignored_untracked = None`; `since_last_fire = 5 min`.
2. **Core:** `.gitignore = "*.log\ndist/\n"`, commit a force-added `keep.log` and a gitlink entry
   `sub` (submodule); create `x.log`, `dist/a.js`, `src/a.ts` → `count_ignored_paths([keep.log,
   x.log, dist/a.js, src/a.ts, sub/y.log, ""])` has `ignored_untracked == 2` (x.log, dist/a.js);
   non-repo → `None`.
3. **Integration, real notify** (`tests.rs` temp-repo pattern): `.gitignore = "dist/\n"`; write
   `dist/a.js` 3× → no `on_change` within 1.5 s (today 1). Write `src/a.ts` → one
   `on_change(Worktree)`. Edit `.gitignore` → one `on_change`.
4. Serde: a dropped firing → `"fired":true,"suppressed":true,"suppressReason":"ignored"`.

### Acceptance

- In a session like `s1d3f226f`, small all-ignored bursts emit no `repo-changed`
  (≈ −300 worktree rounds / 115 min; `redundant-refresh` ≈ −60).
- Refs path, error tick, non-ignored / tracked / submodule path, > 128 raw paths, or capped sample →
  fires as today. ≥ 1 `repo-changed` per 5 min of ignored-only churn. Focus/manual/debounce unchanged.

---

## P124c — open diff doubles each background refresh (warn)

### Root cause (verified)

- `fetchDiffSlot` (`RepoWorkspace.tsx:563-576`) always sets `{state:'loading', diff: stale}` then a
  fresh `{state:'ready'}` — two commits per round with a workdir diff open, even when identical.
  `refetchStatus` (`:686-721`) calls it for the open slot every round; status/opState/branches
  already use `keepIfUnchanged` (`:692`, `:638`, `:798`).
- The round's origin is known: `runRefreshRound` passes `silent: !userOrigin` (`:868`);
  `watcher`/`external` (incl. P121b health ticks) are background, focus/manual are user (`:910`).
- `GraphCanvas` (`src/graph/GraphCanvas.tsx:76`, `forwardRef`, no `memo`) and `Sidebar`
  (`Sidebar.tsx:131`, no `memo`) re-render on every container commit; memoising them is out of
  scope (props-identity audit). Moving `diffSlot` out of the container is not feasible here
  (`overlayMeta`, `stageable`, `useCompareMode`, `usePartialStaging`, `usePrFileOverlay`,
  `useCommitActions`, `useImageDiff` consume it) → extract the slot mechanics instead.

### Design — new `src/components/repoWorkspace/useDiffSlot.ts`

```ts
import type { Dispatch, MutableRefObject, SetStateAction } from 'react';
import type { DiffSlot } from '../DiffView';
import type { FileDiff } from '../../ipc';

export interface FetchDiffSlotOpts {
  /** Non-user round (watcher / external / health): when the shown slot has the same key,
   *  skip the 'loading' transition and keep it shown until the result lands. */
  background?: boolean;
}
export interface UseDiffSlot {
  diffSlot: DiffSlot | null;
  setDiffSlot: Dispatch<SetStateAction<DiffSlot | null>>;
  diffSlotRef: MutableRefObject<DiffSlot | null>;
  fileDiffReqId: MutableRefObject<number>;
  fetchDiffSlot(key: string, fetcher: () => Promise<FileDiff>, opts?: FetchDiffSlotOpts): Promise<void>;
}
export function useDiffSlot(): UseDiffSlot;
```
```
fetchDiffSlot(key, fetcher, opts):
  id = ++fileDiffReqId.current
  prev = diffSlotRef.current;  sameKey = prev !== null && prev.key === key
  if !(opts?.background && sameKey):
    setDiffSlot({ key, state:'loading', diff: sameKey ? prev.diff : null, error:null })   // as today
  try:
    diff = await fetcher(); if id !== fileDiffReqId.current: return
    next = { key, state:'ready', diff, error:null }
    setDiffSlot(p => structuralEqual(p, next) ? p : next)     // whole-slot compare
  catch e:
    if id !== fileDiffReqId.current: return
    setDiffSlot({ key, state:'error', diff:null, error: errorMessage(e) })   // unchanged
```
Whole-slot compare ⇒ a foreground loading→ready still commits; only identical ready→ready is a
no-op. `structuralEqual` is O(diff) on parsed JSON — fine.

`RepoWorkspace.tsx`: replace `useState<DiffSlot|null>` (`:387`), `fileDiffReqId` (`:424`),
`diffSlotRef` + assignment (`:426-427`) and `fetchDiffSlot` (`:563-576`) with
`const { diffSlot, setDiffSlot, diffSlotRef, fileDiffReqId, fetchDiffSlot } = useDiffSlot();` at
`:387`. `collapseDiffSlot` stays (needs `setPrOverlayCtx`). `refetchStatus` `:710`:
`fetchDiffSlot(slot.key, () => …, { background: opts?.silent === true })`. All other callers
(partial staging, PR overlay, commit actions, `:1601`) are user-initiated → unchanged. Net
RepoWorkspace change negative; run `pnpm lint:size -- --update-baseline` to lock the reclaim.

Visible effect: background rounds no longer flash the diff loading state (Q5).

### Files

| File | Change |
|---|---|
| `src/components/repoWorkspace/useDiffSlot.ts` (+ `useDiffSlot.test.tsx`) | New hook. |
| `src/components/RepoWorkspace.tsx` | Use hook; `background` at `:710`. |
| `e2e/perf/interactions.ts` (+ `interactions.spec.ts` comment) | New interaction. |
| `scripts/perf-baseline.json` | Hand-add `reactCommits: 2` (msedge); seed the rest. |
| `scripts/file-size-baseline.json` | Regenerated (reclaim). |

### Failing tests (fail today)

1. **Lab** `background-refresh-diff-open` (add to `INTERACTIONS`, sibling of `backgroundRefresh`
   `interactions.ts:200`): `openSettled`; click the unstaged `README.md` row to open its diff;
   `stepUntil` diff ready + `appSettled`; `measure` → `__bonsaiEmitWatcher('fsWorktree')` (accepted
   reason, `mock/watcherHarness.ts:15`); settle on `ipcByCmd.getWorkdirFileDiff >= 1` +
   `appSettled`. Hand-set `reactCommits: 2` fails today. Before fixing the ceiling, confirm the mock
   `getWorkdirFileDiff` (`mock/handlers/diff.ts:36`) returns structurally identical output on
   repeat calls (no timestamps/randomness), else the lab is red for the wrong reason.
2. **Hook** (`useDiffSlot.test.tsx`, render counter): foreground fetch K → ready `S1`. Background
   fetch resolving an equal (fresh-object) diff → `diffSlot === S1`, renders +0, no `loading`
   observed. Background with a changed diff → +1 render, `ready`. Foreground same key → `loading`
   then `ready`. Two overlapping fetches → only the latest lands. (Hook is new; test 1 is the
   failing-today form.)

### Acceptance

- Workdir diff open + identical background worktree round → `RepoWorkspace` commits 0 (so 0
  `GraphCanvas`/`Sidebar` renders); a changed diff → 1 commit.
- User-initiated diff opens/refetches still show loading. RepoWorkspace shrinks. Mock unchanged.

---

## P124d — double window-focus event (warn, cheap)

### Root cause (verified)

`src/ipc/tauri/app.ts:25-29` forwards every `onFocusChanged(true)`; WebView2 delivers two within
22–47 ms (log 15701/15711, 15962/15972). Each runs `RepoWorkspace.tsx:1017-1022` →
`refresh('focus','full')` (openRepo + all slices), forge `refresh('focus')`, checks bump; the
coalescer makes the second a trailing full round → 20 of 22 `dup-ipc`. Whether a `false`
interleaves is not visible (filtered before logging), so an edge trigger alone is insufficient; a
leading-edge time window is robust. The mock (`mock/handlers/session.ts:70-73`) forwards browser
`focus`.

### Design — adapter layer, shared pure gate

New `src/ipc/focusGate.ts`:
```ts
/** Duplicate focus deliveries closer than this collapse into one. */
export const FOCUS_COALESCE_MS = 250;
/** Leading-edge gate. The returned predicate is true for the first `focused === true` and
 *  false for any further `true` within `windowMs` of the last ACCEPTED one. `false` inputs
 *  return false and do NOT reset the window. Pure apart from `now`. */
export function createFocusGate(windowMs?: number, now?: () => number): (focused: boolean) => boolean;
```
- `tauri/app.ts`: `const accept = createFocusGate();` per subscription;
  `onFocusChanged(({ payload }) => { if (accept(payload)) cb(); })`.
- `mock/handlers/session.ts`: same gate around the `window` `focus` listener (`accept(true)`).
- `ipc-api.ts:525` doc: "fires at most once per `FOCUS_COALESCE_MS`". RepoWorkspace untouched.
  A genuine refocus < 250 ms after the last is intentionally merged.

### Files

| File | Change |
|---|---|
| `src/ipc/focusGate.ts` (+ `focusGate.test.ts`) | New. |
| `src/ipc/tauri/app.ts` | Gate. |
| `src/ipc/mock/handlers/session.ts` (+ test) | Gate. |
| `src/ipc/types/ipc-api.ts` | Doc line. |

### Failing tests (fail today)

1. **Mock** (`src/ipc/mock/handlers/session.test.ts`, new or existing; fake timers):
   `onWindowFocus(cb)`; dispatch `focus`, +30 ms, dispatch → `cb` once (today twice); +300 ms,
   dispatch → twice; after unsubscribe → no calls.
2. **Pure** `focusGate.test.ts`, injected `now`: true@0 → true; true@47 → false; false@60,
   true@70 → false; true@300 → true.
3. Recommended (new precedent): `src/ipc/tauri/app.test.ts` with `vi.mock('@tauri-apps/api/window')`
   capturing the `onFocusChanged` handler; `payload:true` twice 47 ms apart → `cb` once.

### Acceptance

- One focus episode → one `full` round (one `openRepo`, one `streamGraph`); focus-pair `dup-ipc`
  → 0 next session. Browser-harness focus still refreshes.

---

## Open questions (user ruling; recommendation first)

1. **P124a `none` rule.** (A, rec.) streak: one re-check ≥ `NONE_SETTLE_MS` after first sight, then
   30 min — safe after a push. (B) A + commit-age shortcut (tip `committerTs` ≥ 1 h → settle on
   first sight): saves the boot re-check, but pushing an older commit whose CI registers seconds
   after the post-push forced refresh shows "no CI" up to 30 min. (C) flat long TTL — rejected.
2. **`NONE_SETTLE_MS` = 2 min** (queued pipelines/check suites show `pending` within seconds);
   5 min if the user's Azure pipelines register slowly.
3. **P124a2 deferred** — evidence repos are Azure DevOps (GitHub GraphQL irrelevant); Azure
   `commitsbatch` needs three live probes with the user's PAT. Re-measure after P124a first.
4. **P124b drop rule.** (A, rec.) drop ≤ 128-raw-path, fully ignored, untracked bursts with a
   5-min heartbeat: worst case a tracked change lost to a Windows buffer overflow inside an
   ignored-only window shows ≤ 5 min late (or at focus/manual). (B) no heartbeat — unbounded until
   the next firing burst/focus. (C) don't drop. Lifts P121d's hold-back on a bounded risk.
5. **P124c visible change:** background rounds no longer flash the diff loading state. Rec. accept
   without a ui-designer pass (the stale diff was already shown during loading).
6. **P120 lab sibling** `background-refresh-diff-open`, hand-set `reactCommits: 2` — rec. yes.

**User rulings 2026-09-25:** Q1 (A) streak rule, no commit-age shortcut. Q2 `NONE_SETTLE_MS` =
2 min. Q3 P124a2 deferred — re-measure after P124a. Q4 (A) drop with the 5-min heartbeat. Q5
accepted without a ui-designer pass. Q6 yes — add `background-refresh-diff-open` with the
hand-set `reactCommits: 2` ceiling.
