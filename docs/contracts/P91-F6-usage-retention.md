# P91 F6 — `usage.json`: always-on, 90-day window, deletable (NORMATIVE AMENDMENT)

**Owner:** architect · **Written:** 2026-09-11 · **Amended:** 2026-09-14 (implementation review —
§2, §4.2, §5/§6, §7, acceptance 9 + new 12, flags F6-C/F6-D)
**Status:** implementation in review (2026-09-14); this file is the normative record of the ruling
**Amends:** `docs/contracts/P91-observability.md` §6.1, §6 Commands, §8, §8.1, §8.2, §10, §13
**Renders to:** `docs/contracts/P91-privacy-copy-ui.md` §6 (ui-designer, authoritative for all copy)
**Authority:** user ruling 2026-09-11, `TODO.md` → `USER DECISION LEDGER` row 3, plus the follow-up
`lifetime` ruling of the same date (§3.5 below).

> **Why this is a separate file rather than edits inside `P91-observability.md`.** That file is
> ~2000 lines — **3.3× the ~600-line lean-contract cap** — and the standing instruction is to split
> per sub-increment rather than grow it further; every spawn pointed at it re-pays its whole token
> cost. Secondarily, the architect role has a `Write` tool only (no `Edit`), so amending ten
> passages in place means re-emitting all of it and risking the silent loss of one of its 33
> ratified decision rows. **This file is normative and overrides every sentence it quotes as
> SUPERSEDED.** §2's pointers have since been spliced into that file.

---

## 1. The ruling, as three separable promises

| # | Promise | Mechanism |
|---|---|---|
| R1 | Collection is **always on**, independent of Dev mode, from first launch | unchanged — §8 already ships this; **not** re-opened |
| R2 | The per-day profile is retained **90 days**, not 400 | §3 |
| R3 | The data is **deletable**, and the remedy targets the **`metrics` folder** | §4 |

R1 is why gating was rejected: a future Statistics page is the stated reason the data exists, and a
Dev-mode gate would leave that page empty for most users. R2 and R3 are independent promises and
**both must hold**: "kept 90 days" does not imply "clearable", and "clearable" does not imply a
bounded window.

**Dated user decision — 2026-09-11 (reversal, recorded because it overturns a ratified section).**
`P91-observability.md` §10 and §13 row 6 ratified "ship `metrics_reset`, expose no UI… there is no
way to destroy history from a surface that cannot yet display it", and §6.1/§8 ratified that
`logs_delete_all` never touches `metrics/`. **The user reversed the premise: the data must be
deletable now.** The reversal is narrower than §10's wording suggests — **no `metrics_reset` row is
added** (§5), so §10's "no destroy affordance of its own" survives; what changes is that the existing
`logs_delete_all` affordance now covers metrics.

---

## 2. Passages superseded — ten listed, **NOT exhaustive**

| Location | Superseded text | Replaced by |
|---|---|---|
| §6.1 purge-scope bullet (`:782-785`) | "The command never touches `metrics/`, `settings.json`, or anything outside those two directories." | §4.2/§4.3 — scope adds `<app_config_dir>/metrics/`. `settings.json` and everything else stay out. |
| §6 Commands note (`:952-955`) | "`metrics_reset` … no UI exposes it in P91 … (Contrast `logs_delete_all`, which *is* user-exposed — logs carry privacy-relevant content; metrics structurally cannot.)" | §5 — the command and its no-UI status survive; the parenthetical's reasoning does not. |
| §8 key-namespace bullet (`:1423-1425`) | "which is why they are not covered by `logs_delete_all`" | §4 — they are covered. The *other* half of that sentence (metrics structurally cannot contain repo content ⇒ no redaction pass) is **unchanged and still ratified**. |
| §8 `MetricsSnapshot.days` comment (`:1404`) | "retained: 400 days" | §3 — 90 days. |
| §8.1 ceiling (`:1494-1495`) | "400 days ≈ 1.2 MB worst case" | §3.5 — recomputed at 90. |
| §8.2 G3 (`:1575`) and the corrected ceiling (`:1597-1601`) | "400-day→`lifetime` roll-up", "≈ 4.3 MB over 400 days" | §3.5. |
| §10 row `dev.delete-logs` (`:1810`) | scope is logs + exports | §4.3 — scope is logs + exports + the metrics folder. §10's "No row for `metrics_reset` in P91" (`:1817`) is **still correct**. |
| §13 row 4 (`:1934`), row 6 (`:1936`), row 29 (`:1959`) | "no automatic deletion", "expose no UI", "400 × 3 × 513" | §3.3 (age-based pruning IS automatic deletion **of metrics**; the §6 prohibition on automatic **log** deletion is untouched), §5, §3.5. |
| §12 increment 1 (`:1891`) | "`log_export_session(None)` writes into …" | the command is **zero-arity** (§13 row 27); read `log_export_session()`. |
| §6 tree comment (`:674`) | "`exports/` … # §6.2 — **default** target for `log_export_session`" | the **only** target; there is no other. |

**This table is a starting point, not a completed checklist (added 2026-09-14).** The splice pass
found **three further** stale passages contradicting the F6 ruling, all since corrected in place:
the **file-table row** (`:44`), the **`SAVE_LOCK` paragraph** (now `:1708`), and `:795`, which still
asserted the command "never touches `metrics/`" — false since `logs_delete_all` clears the metrics
folder and resets the in-memory state. Assume more may exist; a reader amending that file should
grep it for `metrics`, `400`, and `un-deletable` rather than trusting this list.
**The line numbers above are pre-splice and have all shifted** — grep `SUPERSEDED` in
`P91-observability.md` for current positions. The table is deliberately not renumbered.

**Code comments to correct in the same increment** (`senior-dev`, not editable here):
`src-tauri/src/obs/metrics_keys.rs:44` states `usage.json` is "durable, is not covered by
`logs_delete_all`… a user-derived key there is permanent, un-deletable". The privacy argument for the
key guard **still stands and must be kept** — a user-derived key would still be written unredacted
and would survive until the user deletes — but "un-deletable" is now false. Same for
`src-tauri/src/obs/metrics.rs:293`, `metrics_map.rs:5`, `metrics_persist.rs:16-17`.

---

## 3. R2 — the 90-day window

### 3.1 The constant

`src-tauri/src/obs/metrics.rs:40` — `pub const RETAIN_DAYS: usize = 90;` (was `400`). This is the
**single home**; no other module may hard-code a window. Every doc comment naming 400
(`metrics.rs:54`, `:56`, `:85`, `:249`, `:260`, `histogram.rs:127`, `metrics_map.rs:8`,
`src/ipc/types/obs.ts:195`) is updated in the same increment.

### 3.2 Age, not bucket count — and why

Today's retention is purely **count-based** (`totals_for`: `while days.len() >= RETAIN_DAYS`). The
app is not used every day, so 90 buckets can span years of calendar time. The signed copy says "kept
for **90 days**", so the rule must be **calendar age**. The count check is retained as a structural
backstop for a clock regression or a hand-edited file, not as the primary mechanism.

```rust
// obs/metrics_clear.rs  (split out of metrics.rs for the 500-line cap — §7)
/// Folds every day bucket older than the retention window into `lifetime`.
/// Returns the number of buckets folded (0 = nothing to do).
///
/// `now_secs` fixes "today". Comparison is LEXICOGRAPHIC: `writer::utc_date`
/// emits zero-padded `YYYY-MM-DD` (`writer_files.rs:147`), so string order IS
/// chronological order and no date parsing is needed.
fn prune_days(file: &mut MetricsFile, now_secs: i64) -> usize;
```

```
prune_days(file, now_secs):
    cutoff = utc_date(now_secs - (RETAIN_DAYS - 1) * 86_400)   # inclusive lower bound
    folded = 0
    # retain(), NOT a prefix drain: `days` is only *expected* to be sorted
    # (metrics.rs:251-256 documents the append-only clock assumption), and a
    # backwards UTC step can append an out-of-order bucket.
    for bucket in file.days where bucket.date < cutoff:
        file.lifetime.merge(&bucket.totals)
        folded += 1
    file.days.retain(|b| b.date >= cutoff)
    return folded
```

Boundary, pinned by test: with `RETAIN_DAYS = 90`, a bucket dated `today − 89` **survives** and one
dated `today − 90` **folds**. 90 days inclusive of today.

### 3.3 Two triggers — on load AND on date change

| Trigger | Site | Why it is needed |
|---|---|---|
| **load** | `MetricsState::init` (`metrics.rs:201-217`), immediately after `metrics_file::load`, before the `sessions` bump | This is the **migration** (§3.4) and the only thing that prunes an install that sat unused for months. `init` already calls `mark_dirty`, so a pruned file is persisted at the first flush with no extra call. |
| **date change** | `totals_for` (`metrics.rs:257-277`), in the `last.date != today` branch, **before** the existing count backstop and before the push | A long session never re-hits `init`. Without this, a session running across midnight keeps buckets past the window. |

Ordering inside `totals_for`: `prune_days(...)` → existing `while days.len() >= RETAIN_DAYS` fold →
push today's bucket. After an age prune the length is ≤ `RETAIN_DAYS - 1` whenever today's bucket is
absent, so the backstop is inert in every healthy case — keep it anyway, and keep its comment, but
re-label it a backstop.

`prune_days` returning > 0 is **not** an anomaly and emits no record.

### 3.4 Migration of existing on-disk files, and the `.bak` hole

An installed build can hold up to 400 day buckets. **The `init` trigger is the migration**: the first
launch of the new build folds every bucket older than 90 days into `lifetime` and persists the
pruned file at the first flush (≤60 s) or on exit. No version bump, no separate migration path,
`METRICS_SCHEMA_VERSION` stays **1** — the shape is unchanged, only the number of elements.

**The hole that migration alone leaves.** `metrics_file::save_locked` (`:142-144`) rotates the
current good primary to `usage.json.bak` **before** committing the new bytes. So the migration's own
save moves the **400-day file into `.bak`**, where `load` (`:31`) will recover it if the primary is
ever torn — silently restoring a 400-day profile the user was told is 90 days.

**Required (6 lines, not optional — the copy is signed):**

```rust
// obs/metrics.rs — Inner, in-memory only, NEVER serialized
/// Set when `usage.json.bak` may hold data `load` must never recover; cleared by
/// the first `persist()` that commits (which removes the `.bak`). Two arming
/// sites: `prune_days` folded ≥1 bucket at load (§3.4), and a FAILED clear
/// commit (§4.2a), after which the `.bak` may hold the pre-clear profile.
bak_stale: bool,
```
`persist()` (`metrics_persist.rs:97-131`) captures `bak_stale` **with** the snapshot (under the state
mutex), and inside the existing `SAVE_LOCK` block, **after** a successful `guard.commit`, removes
`metrics_file::bak_path(&path)` when the captured flag was true; the flag is cleared in the final
state-mutex block alongside the `dirty` clear. This respects §8.3's ratified lock order — the state
mutex is still never taken inside `SAVE_LOCK`.

Cost: the crash-recovery copy is absent for one flush interval after a migration. Accepted — the
only thing that copy could restore is the file the migration exists to discard.

### 3.5 What the window does NOT prune — **ruled by the user, 2026-09-11**

`first_seen`, `sessions` and `lifetime` are **lifetime figures and survive the fold.** The 90-day
window applies to the **per-day profile (`days[]`) only.**

Rationale, recorded so a later session does not "tidy" these into the prune: `first_seen` and
`sessions` are inherently lifetime values (`sessions` is incremented once per launch at
`metrics.rs:210`/`:235`, not per day), and the Statistics page — the user's stated reason for keeping
collection always-on rather than Dev-gated — needs the running total. This makes ui-designer's signed
¶7 clause "The day-by-day detail is kept for 90 days, **and a running total after that**" true as
written.

`lifetime` is **undated aggregate counts of Bonsai's own action names**. It is not a profile of when
anything happened and carries no repo content (§8's structural guarantee is unchanged). The
interaction with R3 is deliberate and both halves hold: **the window never removes the lifetime
figures; the delete action removes them completely** (§4.1). "Retained 90 days" and "cleared by the
delete action" are separate promises.

**Ceilings recomputed** (supersedes §8.1 `:1494-1495` and §8.2 `:1597-1601`): ~212 allow-listed
duration keys ≈ **~11 KB/day**, so **≈ 1.0 MB** over 90 days worst case (a user invoking every
command every day in Dev mode), against 4.3 MB at 400. Worst-case cardinality (§13 row 29) becomes
`90 × 3 × 513` ≈ **139k** keys, not 616k. §8.2's "size-triggered early roll-up at ~8 MB" revisit
trigger is now ~8× further away and stays a revisit trigger, not work.

---

## 4. R3 — deletion

### 4.1 What "deletable" means, exactly

Deleting the aggregate is **two inseparable halves**, and a test that checks only one **passes while
the button does nothing**:

1. **On disk:** the `metrics` folder's files and the folder itself are removed — `usage.json`,
   `usage.json.bak` **and** `usage.json.tmp`. Deleting only `usage.json` is a no-op: the next `load`
   recovers `.bak` (`metrics_file.rs:31`). This is why the user's ruling and every user-facing string
   name the **folder**, never the file.
2. **In memory:** `MetricsState`'s `Inner.file` is replaced by a fresh empty `MetricsFile`. Without
   this, the next flush writes the just-deleted aggregate straight back and the folder reappears with
   the *old* numbers.

**Named requirement, so half an implementation is a visible failure:** acceptance §8 item 5 asserts
the post-delete `metrics_snapshot()` is empty (the in-memory half) **and** that no file remains (the
on-disk half). Either assertion alone is insufficient.

"Empty" means `schema = METRICS_SCHEMA_VERSION`, `first_seen = utc_date(now)`, `sessions = 0`,
`days = []`, `lifetime = MetricTotals::default()` — i.e. exactly what the ratified `reset()` already
produces. The live session is **not** re-counted; `sessions` becomes 1 again at the next launch's
`init`. This is what makes ui-designer's §6.1.1 row "`first_seen` … re-set to today after a delete"
true, and why its copy says "the date the count started" rather than "the date you first used
Bonsai".

### 4.2 The primitive — one door, two modes

```rust
// obs/metrics_purge.rs — the two shared types live with the purge helper
/// §F6 — what a clear leaves behind on disk.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClearMode {
    /// `metrics_reset` — commit a fresh empty `usage.json` in place (§5).
    ResetInPlace,
    /// `logs_delete_all` — remove the metrics files and the folder (§4.3).
    DeleteFiles,
}

/// §F6 — honest counts for the metrics half of a delete. Merged into
/// `LogsDeleteResult` by the command layer.
#[derive(Debug, Default, Clone, Copy)]
pub struct MetricsClearCounts {
    pub deleted_files: u32,
    pub deleted_bytes: u64,
    pub failed_files: u32,
    /// True when the `metrics` directory itself no longer exists.
    pub dir_removed: bool,
}

// obs/metrics_clear.rs
impl MetricsState {
    /// THE door for "forget everything". Blocking (file IO) — call on the blocking
    /// pool. `perf` is the CURRENT `PerfState::snapshot()`; see the re-baseline note.
    pub fn clear(
        &self,
        perf: &PerfCounters,
        now_secs: i64,
        mode: ClearMode,
    ) -> Result<MetricsClearCounts, AppError>;
}
```

```
clear(perf, now_secs, mode):
    # --- 1. in-memory first, under the state mutex ---
    #   `fresh` is CLONED OUT here on purpose: SAVE_LOCK must never re-take the
    #   state mutex to read the file (§8.3's one-lock-order rule).
    (path, fresh, rev) = {
        g = self.lock()
        g.file = MetricsFile{ schema: METRICS_SCHEMA_VERSION,
                              first_seen: utc_date(now_secs),
                              sessions: 0, days: [], lifetime: default }
        g.perf_baseline = perf.clone()      # NOT default() — see below
        g.last_wall_secs = now_secs
        g.bak_stale = false                 # both modes handle the old .bak explicitly
        mark_dirty(&mut g)                  # bumps `rev` — this is the fence token
        g.dirty = false                     # nothing scheduled may re-write the old data
        (g.path.clone(), g.file.clone(), g.rev)
    }
    if path is None: return Ok(MetricsClearCounts{ dir_removed: true, ..default })

    # --- 2. fence every in-flight saver, then touch the disk ---
    guard = metrics_file::begin_save()      # SAVE_LOCK; a saver mid-commit finishes first
    commit_rev.store(rev, Release)          # §8.3's compare-then-commit now DROPS any
                                            # snapshot taken before this clear.
                                            # NEVER rolled back — see §4.2a.
    result = match mode:                    # CAPTURE the Result; do NOT `?` here
        ResetInPlace => guard.commit(&path, &fresh)
                          .map(|_| { remove_file(bak_path(&path));  # else the PRE-clear
                                     MetricsClearCounts::default() })  # profile lives on
        DeleteFiles  => Ok(match path.parent():           # usage.json{,.bak,.tmp} + the dir
                             Some(dir) => purge_metrics_dir(dir)
                             None      => default counts) # cannot happen; must not panic —
                                                          # the in-memory half already won
    drop(guard)                             # §8.3: the state mutex is NEVER taken inside
                                            # SAVE_LOCK, so step 3 comes after this drop

    # --- 3. a failed commit must stay scheduled (§4.2a) ---
    if result is Err:
        g = self.lock()
        g.dirty = true                      # plain assignment, NOT mark_dirty(): the state
        g.bak_stale = true                  # did not change, it just isn't on disk yet
    return result
```

#### 4.2a The failure path — **corrected 2026-09-14** (the pre-correction spec lost the retry)

The shipped `reset()` persisted through `persist()`, which leaves `dirty` **set** on a failed commit
so the next flush retries. Clearing `dirty` before the commit (step 1) and `?`-ing the commit would
leave **memory empty, disk stale, and nothing scheduled to rewrite it** until an unrelated
observation happens to re-mark dirty — and the stale file is what the next launch loads, resurrecting
data the user cleared. Four rules close it:

- **`dirty = true` on `Err`, set with a plain assignment, not `mark_dirty`.** `rev` must not move:
  the memory state did not change, it merely is not on disk. Setting it unconditionally is safe and
  monotone — a concurrent observation that already set it true is unaffected, and the worst case is
  one redundant write of identical bytes.
- **`bak_stale = true` on `Err` as well.** `save_locked` rotates primary→`.bak` *before* writing, so
  a failure at any step can leave the **pre-clear** profile in `usage.json.bak`, which `load`
  recovers. Re-arming the flag makes the eventual successful `persist()` delete it through the
  machinery §3.4 already ratified — no second mechanism.
- **Never roll back `commit_rev`.** The fence's job is to drop pre-clear snapshots, and that must
  hold whether or not our own commit landed — rolling it back would let an in-flight pre-clear
  snapshot rewrite the data the user asked to destroy. It does not block the retry: the retry's
  snapshot carries `rev' >= rev`, so `persist`'s `commit_rev > rev'` test is false.
- **`DeleteFiles` keeps `dirty = false` even when `failed_files > 0`.** It returns `Ok(counts)`, so
  step 3 never fires, and that is intended: a scheduled write of an empty file into a folder the user
  asked to have removed contradicts the intent, and the partial failure is already reported honestly
  through `failed_files` and the §6.1 partial-failure copy. Always-on collection re-creates the
  folder at the next genuine observation. **Do not "fix" this asymmetry.**

**Why the `commit_rev` fence, and what happens to an in-flight writer.** Three cases, all closed by
the two steps above and by machinery §8.3 already ratified:

- **A saver already inside `guard.commit`** — we block on `begin_save()` until it finishes, then
  delete the bytes it just wrote. Correct, and there is no window where the folder is removed
  underneath an open handle: every metrics write happens inside `SAVE_LOCK`.
- **A saver that snapshotted before the clear and has not yet taken `SAVE_LOCK`** — its snapshot's
  `rev` is now `< commit_rev`, so `persist()` (`metrics_persist.rs:116-121`) drops it. This is
  exactly the reset-undone-on-disk failure §13 row 32 exists for; the fence reuses that mechanism
  rather than inventing a second one.
- **A save that starts after the clear** — it snapshots the fresh empty file, and
  `save_locked` **re-creates the directory** (`metrics_file.rs:121-124`, `create_dir_private`). So
  removing the folder is safe: always-on collection re-creates it at the next flush, containing only
  post-delete data. **No writer ever sees a missing directory as an error.**

**`perf_baseline = perf.clone()`, not `PerfCounters::default()` — a real defect in the shipped
`reset()`.** `fold_perf` (`metrics.rs:388-410`) records `delta = snapshot - perf_baseline`, and
`PerfState`'s counters are process-lifetime and are **not** reset. Zeroing the baseline therefore
makes the next fold re-add every pre-click repo-open, graph-walk and status-scan — resurrecting the
counts the user just cleared, about 60 s later, with no test noticing. Both modes must re-baseline to
the current snapshot. Consequence: `metrics_reset` and `logs_delete_all` both need the perf state
(§6).

### 4.3 The folder purge

```rust
// obs/metrics_purge.rs  (NEW, ~60 lines — keeps `metrics.rs` under the 500-line cap)
/// Deletes every FILE in `dir`, then the directory. Non-recursive by design.
pub fn purge_metrics_dir(dir: &Path) -> MetricsClearCounts;
```

```
purge_metrics_dir(dir):
    c = MetricsClearCounts::default()
    for entry in read_dir(dir):              # missing dir => all-zero counts, dir_removed = true
        if entry.file_type().is_dir(): c.failed_files += 1; continue   # never recurse
        size = entry.metadata().len().unwrap_or(0)                     # read immediately before
        remove_file → (deleted_files += 1, deleted_bytes += size) | failed_files += 1
    c.dir_removed = !dir.exists() || remove_dir(dir).is_ok()           # succeeds iff now empty
    return c
```

**Scope: every file directly inside `<app_config_dir>/metrics/`, then the directory.** Not a name
glob — a future metrics file must be covered automatically, and `usage.json*` alone would leave a
stray file that then blocks `remove_dir` and makes "removes that whole folder" false.

**`purge_metrics_dir` is the metrics-side primitive and `writer::purge_scope` is the logs-side one;
neither reaches into the other's directory.** `purge_scope` is logs + exports **only**, because it
also runs on the sink writer thread, which has no access to `MetricsState`. The two halves meet only
at the command layer (§6). This separation is what keeps the metrics delete off the writer thread,
and it is why `tests_purge.rs`'s "`metrics/usage.json` survives `purge_scope`" assertion is
**correct and must not be inverted** (flag F6-D).

**Not `remove_dir_all`, deliberately.** `remove_dir_all` reachable from the webview is a recursive
delete on a path derived by string join; if `metrics_dir` ever resolved to the config root (an empty
join, a refactor), it would take `settings.json` and `logs/` with it. Enumerate-then-`remove_dir`
**cannot** do that, and for a directory that only ever holds `usage.json{,.tmp,.bak}` the two are
behaviourally identical. A subdirectory (never created by Bonsai) is counted in `failed_files` and
left alone — the existing §6.1 partial-failure copy path already covers it. **Do not "simplify" this
to `remove_dir_all`.**

`metrics/` is the **only** addition to the delete scope. `settings.json`, the config root, and
everything outside `logs/`, `exports/` and `metrics/` remain untouched — §6.2's
"exhaustive by construction" property is preserved because all three directories are app-created and
none is caller-supplied.

---

## 5. `metrics_reset` — still distinct, not redundant, still UI-less

**Ruling: keep it.** It is the narrow primitive (`ClearMode::ResetInPlace`) and differs from
`logs_delete_all` on both axes:

| | `metrics_reset` | `logs_delete_all` |
|---|---|---|
| Scope | metrics only | logs + exports + metrics |
| On-disk result | a fresh **empty** `usage.json` | **no** `metrics` folder |
| UI in P91 | none (§10 unchanged) | the `dev.delete-logs` row |
| Lives in | `commands/obs.rs` | `commands/obs_delete.rs` (the one destructive command) |

It stays UI-less: tests need it, and when the Statistics page ships a "clear usage statistics" control
belongs **there**, next to the data. Both commands route through `MetricsState::clear`, so the
in-memory reset, the `commit_rev` fence and the perf re-baseline have exactly one implementation. The
`.bak` removal in `ResetInPlace` is required for the same reason as §3.4: without it a "reset" leaves
the full pre-reset profile in `usage.json.bak`.

---

## 6. IPC surface delta

No new command, no new event, no new channel. Two commands gain state; one response type gains two
fields.

**Managed-state correction (2026-09-14).** `PerfState` is **not** `.manage()`d — `lib.rs:69` manages
`Arc<MetricsState>` only, and `perf: Arc<PerfState>` lives on `AppState` (`state.rs:87`). A
`State<'_, Arc<PerfState>>` parameter would therefore fail at runtime. Both commands take
`State<'_, AppState>` and read `app_state.perf.clone()`, matching the precedent set by
`metrics_snapshot` (`commands/obs.rs:114-118`).

```rust
// src-tauri/src/commands/obs_delete.rs
#[tauri::command]
pub async fn logs_delete_all(
    app: tauri::AppHandle,
    app_state: tauri::State<'_, AppState>,          // NEW — for the §4.2 perf re-baseline
    obs_state: tauri::State<'_, ObsState>,
    metrics: tauri::State<'_, Arc<MetricsState>>,   // NEW
) -> Result<LogsDeleteResult, AppError>;

// src-tauri/src/commands/obs.rs
#[tauri::command]
pub async fn metrics_reset(
    app_state: tauri::State<'_, AppState>,          // NEW — same reason
    metrics: tauri::State<'_, Arc<MetricsState>>,
) -> Result<(), AppError>;
```

Each command does `let perf: Arc<PerfState> = app_state.perf.clone();` **before** `spawn_blocking`
(`State<'_, T>` borrows for the command's lifetime; the closure must be `'static`) and passes
`&perf.snapshot()` into `clear`/`reset`.

The metrics step runs in **both** `logs_delete_all` branches (Dev mode ON → the sink's
`roll_and_purge`; Dev mode OFF → `writer::purge_scope`), on the command's **own** `spawn_blocking` —
**never on the sink writer thread**, which has no access to `MetricsState`. Order: log/export purge
first, then `metrics.clear(&perf.snapshot(), now_secs(), ClearMode::DeleteFiles)`; the counts are
merged. A `clear` error does not discard the log-purge counts — report both halves honestly.

```rust
pub struct LogsDeleteResult {
    // …existing fields unchanged…
    /// §F6 — usage-statistics files removed from `metrics/` (`usage.json`, `.bak`,
    /// `.tmp`). ALSO included in `deleted_files`/`deleted_bytes`, like export zips.
    pub deleted_metrics: u32,
    /// §F6 — the in-memory aggregate was reset AND no usage file remains.
    /// **True even when `deleted_metrics == 0`:** `init` only marks dirty, so on a
    /// launch younger than the first 60 s flush there is nothing on disk yet while
    /// the aggregate is very much live. The copy's "clears your usage counts" is
    /// justified by THIS field, never by the file count.
    pub metrics_cleared: bool,
}
```
```ts
export interface LogsDeleteResult {
  deletedFiles: number; deletedBytes: number; failedFiles: number;
  activeFile: string | null; rolled: boolean;
  deletedExports?: number;
  deletedMetrics: number;     // NEW — required; 0 is meaningful
  metricsCleared: boolean;    // NEW — required
}
```
Both new fields are **required**, not `Option`: `LogsDeleteResult` is a same-build command response,
never persisted, so the §13 row 23 additive-only carve-out (which governs on-disk `LogRecord`s) does
not apply, and an optional `metricsCleared` would let the UI fall back to a misleading default.

**Deliberately NOT added:** `LogSessionInfo.metricsBytes`. The signed confirm copy names no size for
usage counts, so the field would ship unused.

**Mock (`src/ipc/mock/handlers/obs.ts`) — required for the harness to show the new copy truthfully:**
- `logsDeleteAll` clears the module-level metrics fixture (so a following `metricsSnapshot()` returns
  the same empty shape `metricsReset` already returns at `:228-230`) and returns
  `deletedMetrics: 1, metricsCleared: true`. Under `?obsDeleteFail=1` it returns
  `failedFiles: 1, deletedMetrics: 0, metricsCleared: false` so the partial-failure copy path is
  reachable.
- `freshMetrics()` (`:53`) must satisfy the window: `days.length <= 90` and no bucket older than 90
  days relative to the newest. It currently holds fixed dates and nothing prunes.
- **Fixture defect to fix while there:** `freshMetrics()` uses `'cmd.get_status'`. Real `cmd.*` keys
  are **camelCase `IpcApi` method names** (§13 row 28) — `cmd.getStatus`. A snake_case fixture is the
  exact input shape whose acceptance by a validator was indistinguishable from a validator that
  rejected everything.

---

## 7. Lockstep file list (one increment)

| File | Change |
|---|---|
| `src-tauri/src/obs/metrics.rs` | `RETAIN_DAYS = 90`; `Inner.bak_stale`; call `prune_days` in `init` and `totals_for`; `#[path = "metrics_clear.rs"] mod metrics_clear;`; doc comments |
| `src-tauri/src/obs/metrics_clear.rs` | **NEW** — `prune_days`/`load_pruned` (§3.2) + `MetricsState::clear` (§4.2). Split out of `metrics.rs` so the observation hot path and the forget path stay separately readable and neither file crosses the 500-line cap. A **child of `metrics`** (declared from `metrics.rs` via `#[path]`, items `pub(super)`), **not** a sibling module in `obs/mod.rs`. |
| `src-tauri/src/obs/metrics_purge.rs` | **NEW** — `purge_metrics_dir` + `ClearMode` + `MetricsClearCounts` (§4.3) |
| `src-tauri/src/obs/metrics_persist.rs` | `reset()` delegates to `clear(.., ResetInPlace)`; `persist()` honours `bak_stale` inside `SAVE_LOCK`; module doc corrected (no longer "NOT in the `logs_delete_all` scope") |
| `src-tauri/src/obs/metrics_file.rs` | make `bak_path` visible to `metrics_clear.rs` / `metrics_persist.rs` if it is not already |
| `src-tauri/src/obs/metrics_keys.rs`, `metrics_map.rs` | doc comments: keep the privacy rationale, drop "un-deletable" (§2) |
| `src-tauri/src/commands/obs.rs` | `metrics_reset` signature (§6) |
| `src-tauri/src/commands/obs_delete.rs` | `logs_delete_all` signature + the merged counts (§6) |
| `src-tauri/src/obs/mod.rs` | `pub mod metrics_purge;` + re-exports |
| `src/ipc/types/obs.ts` | `LogsDeleteResult` two fields; `days` comment 400 → 90 |
| `src/ipc/mock/handlers/obs.ts` | §6 mock bullets |
| `src/components/settings/devLogMessages.ts` | the toast reads `metricsCleared`/`deletedMetrics`; **copy strings are ui-designer's** (`P91-privacy-copy-ui.md` §6.4) |
| `src-tauri/src/obs/tests_metrics.rs`, `tests_metrics_clear.rs` | §8 — the window, both clear modes, the fence, the failed-commit retry |
| `src-tauri/src/obs/tests_purge.rs` | **unchanged (corrected 2026-09-14).** Its survivor assertion `metrics.join("usage.json").exists()` in `purge_off_removes_every_in_scope_file_and_nothing_else` is **correct**: it pins `purge_scope`'s logs+exports-only scope (§4.3). Add a comment naming `tests_metrics_clear.rs` as the home of the command-level scope; do **not** invert. |

`IpcApi` is unchanged (no new method), so `obs/metrics_cmds.rs`'s 199-name allow-list and its drift
test are untouched.

---

## 8. Acceptance criteria (AI gate)

**Rust** (`cargo nextest -p bonsai`):
1. `prune_days` boundary: a file with buckets at `today-90`, `today-89`, `today-1`, `today` keeps
   three and folds one; the folded bucket's counters appear in `lifetime`; `first_seen` and
   `sessions` are **unchanged** (§3.5).
2. **Migration:** pre-seed a `usage.json` with 400 buckets spanning 400 days, `MetricsState::init`,
   assert `days.len() <= 90`, the oldest survivor is `>= today-89`, and `lifetime` holds the sum of
   every folded bucket. Then force a flush and assert `usage.json.bak` does **not** exist (§3.4).
3. Out-of-order buckets (a backwards clock step) are pruned by age, not position — `retain`, not a
   prefix drain.
4. Date-change trigger: drive `totals_for` across a simulated midnight on a file already at the
   window and assert the length stays ≤ `RETAIN_DAYS`.
5. **Delete, both halves:** after `clear(.., DeleteFiles)` — (a) `metrics_snapshot()` is empty per
   §4.1; (b) the `metrics` directory does not exist; (c) `usage.json.bak` does not exist;
   (d) **the re-baseline**: with `PerfState` already showing N>0 repo opens *before* the clear, drive
   **exactly one** repo open *after* it, then flush, and assert `perf.repo_opens == 1` — not `N+1`.
   Asserting merely "zero" cannot distinguish a correct re-baseline from a fold that never ran;
   (e) `sessions == 0`, `first_seen == today`.
6. **The fence:** using the existing `after_snapshot_hook` seam, inject `clear(.., DeleteFiles)`
   between a saver's snapshot and its commit; assert the saver's write is dropped and the directory
   stays gone. Mirror of the §13 row 32 test.
7. `clear(.., ResetInPlace)` leaves an empty `usage.json`, removes `.bak`, and keeps the directory.
8. `purge_metrics_dir` on a directory containing `usage.json`, `usage.json.bak`, `usage.json.tmp` and
   one subdirectory: `deleted_files == 3`, `failed_files == 1`, `dir_removed == false`; on a clean
   directory `dir_removed == true`; on a missing directory all-zero + `dir_removed == true`.
9. `logs_delete_all` with Dev mode **ON** and **OFF** both return `metricsCleared: true`, and
   `deletedFiles` includes `deletedMetrics`. **The command-level scope is asserted in
   `tests_metrics_clear.rs`.** `tests_purge.rs`'s `metrics/usage.json` survivor assertion is **kept
   as-is** — `purge_scope` is the writer-level primitive and §4.3 forbids it from reaching into
   `metrics/` (corrected 2026-09-14; flag F6-D is WITHDRAWN).
10. `logs_delete_all` on a launch with no `usage.json` yet returns `deletedMetrics: 0` **and**
    `metricsCleared: true` (§6).
11. Grep/compile test: no `remove_dir_all` anywhere under `src-tauri/src/obs/` (§4.3).
12. **The failed-commit retry (§4.2a, added 2026-09-14).** Point the state's path at
    `<tmp>/blocked/usage.json` where `blocked` is a **regular file**, not a directory
    (cross-platform, no permission games), so `save_locked`'s `create_dir_private` fails. Assert
    `clear(.., ResetInPlace)` returns `Err`, and that `dirty == true` **and** `bak_stale == true`
    afterwards. Then **delete the blocking regular file** and call `flush()`: the empty file is
    committed and no `.bak` remains. Asserting only the `Err` passes while the retry is lost.

**Frontend** (`pnpm vitest`): `deleteResultToast` covers `metricsCleared` with `deletedMetrics` 0 and
>0 and with `failedFiles > 0`; the mock's post-delete `metricsSnapshot()` is empty.

**Harness** (`pnpm dev`, `VITE_MOCK_IPC=1`): confirm → delete → the row hint and toast state the
usage counts were cleared, and `?obsDeleteFail=1` shows the partial-failure copy. Visual proof is
ui-designer's gate, not this contract's.

**No USER CHECKPOINT item** — every assertion above is machine-checkable. The privacy panel's
appearance is a ui-designer gate item.

---

## 9. What the copy may and may not claim (for `P91-privacy-copy-ui.md`)

**True after this contract ships** — ui-designer's ¶7, §6.3 and §6.4 strings are all supportable:
- "The day-by-day detail is kept for 90 days, and a running total after that." — §3.2 + §3.5.
- "it stays in a `metrics` folder beside your log files" — unchanged.
- "'Delete logs and usage counts' below clears all of it." — §4.1 clears `days`, `lifetime`,
  `sessions` and `first_seen`.
- "They live in a `metrics` folder beside your log files, and Bonsai removes that whole folder." —
  §4.3 removes the directory itself.
- The delete row being **always enabled** is consistent: `clear` always succeeds and always has an
  in-memory aggregate to clear, whether or not a file exists yet.

**Must NOT be claimed:**
- **No always-on durations.** Only counts, `sessions` and `first_seen` are always-on; the `op.*` /
  `cmd.*` duration histograms and `errors` accumulate **only during Dev-mode sessions** (§8 "always
  on clarified"; `metrics.rs:148-152`). ui-designer's §6.1.1 table is the source of truth over the
  board's F6 note, which overclaimed "and how long".
- **Not "deleted permanently".** Collection is always-on by ruling, so the folder **reappears at the
  next flush (≤60 s) with post-delete data only**. "Clears" is accurate; "stops recording" is not.
  **Flagged for ui-designer** (§10): no copy change is requested — ¶7 already says recording is
  always on — but if a future revision adds "permanently" to the dialog it becomes false.
- **Not "N files deleted" as proof.** `deletedMetrics` can legitimately be 0 on a young launch while
  the clear fully succeeded (§6).

---

## 10. Flags for the orchestrator

- **F6-A — CLOSED.** The §2 pointers are spliced into `P91-observability.md`; §2 now also records the
  three further stale passages the splice found and warns that the list is not exhaustive.
- **F6-B — the shipped `reset()` had a data-resurrection defect** (§4.2, `perf_baseline` zeroed while
  `PerfState` is process-lifetime). It was unreachable from any UI, which is why nobody had seen it,
  and the new delete action would have inherited it. Fixed by the shared primitive.
- **F6-C — `metrics_reset` gains a state parameter: `AppState`** (read as `app_state.perf`), **not**
  `State<Arc<PerfState>>` — nothing manages `Arc<PerfState>` (§6, corrected 2026-09-14). It is
  UI-less, so no user-visible change, but its call sites and tests move.
- **F6-D — WITHDRAWN 2026-09-14 (implementation review; the contract was wrong, the code is right).**
  This flag asked for `tests_purge.rs`'s "`metrics/usage.json` survives" assertion to be inverted.
  Inverting it would require `purge_scope` — the **writer-level** primitive that §4.3 forbids from
  reaching into `metrics/` — to delete metrics, which is exactly what would put the metrics delete on
  the sink writer thread. The metrics half is `MetricsState::clear`, on the command's own task. The
  assertion is correct as it stands; keep it, with a comment pointing at `tests_metrics_clear.rs`.
  The "visible inversion signal" this flag wanted **already exists elsewhere**:
  `src/components/settings/devPrivacyCopy.test.tsx:196-212` inverts "omits the usage-count
  disclosure" into "discloses the usage count". Intent met; only the chosen site was wrong.
- **F6-E — a stray U+202E-class character lives in `docs/contracts/P107-F2-copy-chip-ui.md`**
  (1 occurrence; found while grepping for the same defect in my own files). That is a `*-ui.md`
  file — ui-designer's — so it is not mine to edit. The two files I own here,
  `P87b-FU1-run-target.md` and `P91-observability.md`, are both clean.
- **F6-F — not re-opened, stated so nobody assumes it was.** R1 (always-on collection) and §8's
  "metrics structurally cannot contain repo content, therefore no redaction pass" are both unchanged.
  The key-admission guards (§8.2 G1/G2/G3) keep their full force: deletability is a remedy, not a
  reason to relax a guard that prevents a user-derived key from ever being written.

---

## 11. Rows appended to `P91-observability.md` §13 (already spliced — do not re-add)

| # | Decision | Outcome | Where it lands |
|---|---|---|---|
| 34 | **`usage.json` stays always-on but gains a 90-day window and becomes deletable** (user, 2026-09-11; `TODO.md` ledger row 3) | **RULED — none of the three specced options.** Collection stays always-on and independent of Dev mode, because a future Statistics page is the stated reason the data exists and a Dev gate would leave it empty for most users. `RETAIN_DAYS` 400 → **90**, enforced by **calendar age** (not bucket count) at **two** triggers — `init` (which is also the migration for existing 400-day files) and `totals_for`'s date change. The data becomes deletable through the **existing** `logs_delete_all` affordance; **no new control and no `metrics_reset` row**, so §10's "no destroy affordance of its own" survives while its premise ("no way to destroy history from a surface that cannot display it") is **reversed**. | `P91-F6-usage-retention.md` §1, §3; supersedes §6.1 `:782-785`, §8 `:1423-1425`, §13 rows 4 & 6 |
| 35 | **Lifetime figures survive the 90-day fold** (user, 2026-09-11, second ruling of the same date) | **RULED — KEEP them.** `first_seen`, `sessions` and `lifetime` are lifetime values and are **not** pruned; the window applies to `days[]` only. `sessions` is per-launch, not per-day, and the Statistics page needs the running total. **Do not "tidy" these into the prune.** The delete action still removes them completely — "retained 90 days" and "cleared by the delete action" are **separate promises** and both hold. | `P91-F6-usage-retention.md` §3.5, §4.1 |
| 36 | **Deleting the aggregate is two halves, and the `.bak` is the trap** (architect, 2026-09-11) | **DECIDED.** The remedy targets the **folder**: every file in `metrics/` plus the directory, because `usage.json.bak` restores a deleted `usage.json` on the next `load`. It must **also** reset the in-memory `MetricsState` — otherwise the next flush rewrites the deleted data and a test that only checks the file **passes while the button does nothing**. Ordering reuses §8.3's ratified `rev`/`commit_rev` stamp as a **fence**: clear in memory (bumping `rev`), then under `SAVE_LOCK` store `commit_rev = rev` and touch the disk, so every pre-clear snapshot loses its compare-then-commit. `save_locked` re-creates the directory, so no writer ever sees it missing. **`remove_dir_all` is rejected** (a webview-reachable recursive delete on a joined path); enumerate-then-`remove_dir` is behaviourally identical here and cannot escape. Two consequences recorded: `perf_baseline` must be re-baselined to the **current** snapshot (zeroing it resurrects pre-click counts ~60 s later), and a migration must drop the rotated `.bak` or a 400-day profile survives the window change. | `P91-F6-usage-retention.md` §4, §3.4; supersedes §6.1's purge scope |

The 2026-09-14 corrections (§4.2a's retry, §6's managed state, F6-D's withdrawal) are **refinements
of rows 34–36, not new rulings** — deliberately **no row 37**, so nothing here needs splicing again.
