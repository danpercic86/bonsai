# P52 — Adopt git's on-disk commit-graph file

Write/refresh `.git/objects/info/commit-graph` (via `git commit-graph write`) so libgit2 skips
re-parsing commit objects — accelerating the layout revwalk (`graph::compute_graph`), blame/file
history, and above all the repo-health **branches** scan (stale + ahead/behind merge-base), whose
regression currently fails `health::tests::perf_ceiling_on_20k_fixture`.

References read (verified, not guessed): `crates/bonsai-core/src/graph.rs`
(`compute_graph` → `layout_walk` git2 revwalk), `crates/bonsai-core/src/health.rs`
(`collect_branches` → `find_stale_branches` + `repo.graph_ahead_behind`; the perf test), `crates/
bonsai-core/src/git/search.rs` (the P50 `GitRunner` + `SpawnGitRunner` shell-out idiom),
`crates/bonsai-core/src/git/blame.rs::file_history` (git2 per-commit diff), `crates/bonsai-core/
src/fixture.rs` (`generate_fixture` mempack→packfile; `ensure_default_fixture` cache — writes NO
commit-graph today), `crates/bonsai-core/tests/perf_gate.rs`, `crates/bonsai-core/src/git/mod.rs`
(`relax_odb_hash_verification`), `src-tauri/src/commands/repo.rs::open_repo` (warm-on-open
fire-and-forget precedent), `src-tauri/src/commands/remotes.rs` (`fetch`/`pull`),
`src-tauri/src/scheduler.rs::execute_job` (autoFetch), `src-tauri/src/watcher.rs` (`.git/objects/**`
filtered), `src-tauri/src/settings.rs` (additive `#[serde(default)]` pattern).

**Tauri command count: unchanged.** No new command, no new IPC surface, no new event, no channel.
Recommended config decision needs **zero** frontend / TS / mock work (see D3).

---

## 0. Key decisions (with rationale)

**D1 — libgit2 consumes the commit-graph UNCONDITIONALLY; Bonsai only has to WRITE it.**
Verified against libgit2 v1.8.1 source (the version git2 0.21 / libgit2-sys 0.17 vendors):
`git_commit_list_parse` calls `git_odb__get_commit_graph_file(&cgraph_file, walk->odb)` and uses it
`if (cgraph_file)` with **no `core.commitGraph` config check anywhere**. The commit-graph is a pure
ODB-level optimization: present ⇒ used (parents/generation/commit-time read from the graph, no zlib
inflate, no object lookup), absent ⇒ transparent fallback to the ODB. Consequences:
- The **existing** git2 revwalk in `layout_walk` (graph.rs) and the merge-base machinery behind
  `find_stale_branches` + `repo.graph_ahead_behind` (health.rs) speed up the moment the file exists.
  No code change to graph.rs / health.rs is required for the file to be consumed.
- **Bonsai does NOT need to set `core.commitGraph=true`** (see D3).

**D2 — Write via a best-effort `git` shell-out, reusing the P50 `GitRunner` idiom.** git2/libgit2
exposes no "write commit-graph" API, so we shell `git commit-graph write --reachable --changed-paths`
(the roadmap command). Reuse `search::{GitRunner, SpawnGitRunner}` (capture output,
`GIT_TERMINAL_PROMPT=0`, `CREATE_NO_WINDOW`) verbatim — no second spawn implementation. Any failure
(git absent, non-zero exit) is **swallowed** → the op never blocks and never errors the user.

**D3 — Do NOT write `core.commitGraph` to the user's config, and add NO setting (always-on).**
- No config write: libgit2 ignores it (D1); the real `git` we shell out to defaults it `true` since
  git 2.24; writing it would mutate the user's repo Local config for zero benefit. (If a reviewer
  insists on belt-and-suspenders, setting Local `core.commitGraph=true` is harmless — but it is
  unnecessary and is *not* in this contract's plan. Flagged as OQ1.)
- No setting: the commit-graph is a standard, non-destructive artifact `git gc` writes on its own
  (`gc.writeCommitGraph` defaults true). Writing it best-effort on open/fetch is behaviourally
  indistinguishable from ordinary git maintenance — no data risk, no UI-visible change, clean
  degrade with no git. A toggle would add settings + clamp + TS mirror + mock + back-compat tests
  for a feature with no downside to leaving on. **Recommend always-on, no toggle.** (Optional toggle
  design in Appendix A if the orchestrator wants opt-out; flagged as OQ2.)

**D4 — Fire-and-forget, off the UI path, at three trigger classes.** The write runs in
`spawn_blocking` and is **never awaited** (mirrors the warm-on-open credential block in
`open_repo`). Triggers: (a) **repo open** — once; (b) **fetch/pull commands** — after success;
(c) **scheduler autoFetch** — only when refs actually advanced. Plain (non-`--split`) `--reachable`
rewrites the whole graph each time; the cadence is infrequent enough (open + fetch-with-changes)
that a full rewrite off-thread is fine. `--split` is a future large-repo optimization (OQ3).

**D5 — The write does NOT trip the file watcher.** The file lands at
`.git/objects/info/commit-graph`, and `watcher.rs` already filters every write under `.git/objects`
(`git_internals_filtered` test asserts it). So the maintenance write causes **no spurious
`repo-changed`**. (Fetch already writes refs/packs under `.git`, which the watcher debounces; the
commit-graph write is strictly quieter.)

**D6 — `--changed-paths` Bloom filters help ONLY the shelled `git log` path search, not libgit2.**
libgit2 1.8 reads the base commit-graph (generation numbers + inline commit metadata) but does **not**
read the changed-path Bloom filters. So Bloom filters accelerate `git/search.rs` **path** mode
(`git log -- <pathspec>`, shelled) but do **not** speed up git2 `blame::file_history` or git2 blame
(both go through libgit2). Keep `--changed-paths` anyway — it is cheap relative to the base write and
is a real win for path search — but the health-gate / layout speedups come from the **base graph**,
not the Bloom filters. (OQ4: drop `--changed-paths` to shave write time? Recommend keep.)

---

## 1. Module boundaries / files

**New**
- `crates/bonsai-core/src/git/maintenance.rs` — the whole feature: `CommitGraphOutcome`,
  `commit_graph_args()`, `write_commit_graph(workdir, &dyn GitRunner)`,
  `write_commit_graph_best_effort(workdir)`, and `#[cfg(test)]` unit + CLI-oracle tests. ~150 lines.

**Edited (backend only — no frontend touch)**
- `crates/bonsai-core/src/git/mod.rs` — add `pub mod maintenance;`.
- `src-tauri/src/commands/repo.rs` — in the `open_repo` command's `if info.is_repo && !info.bare`
  block, add a fire-and-forget graph-write `spawn_blocking` (beside the warm-on-open block). §3.1.
- `src-tauri/src/commands/remotes.rs` — `fetch` and `pull` commands: after the inner call returns
  `Ok`, fire-and-forget the graph write (fetch gated on `updated_refs > 0`). §3.2.
- `src-tauri/src/scheduler.rs` — in `execute_job`'s AutoFetch success arm, when `updated > 0`,
  fire-and-forget the graph write. §3.3.
- `crates/bonsai-core/src/fixture.rs` — `ensure_default_fixture` writes the commit-graph **once**
  (existence-guarded, best-effort, `have_git`) so ALL gate consumers measure with it present. §6.1.
- `crates/bonsai-core/src/health.rs` — the perf test asserts the graph file is present and keeps its
  budget asserts (now expected to pass). §6.3.
- `crates/bonsai-core/tests/perf_gate.rs` — comment only; it benefits automatically via the fixture.

---

## 2. Core — `crates/bonsai-core/src/git/maintenance.rs`

```rust
//! Best-effort git commit-graph maintenance (P52). Shells
//! `git commit-graph write --reachable --changed-paths` to (re)write
//! `.git/objects/info/commit-graph`. libgit2 (v1.8, git2 0.21) consumes that
//! file UNCONDITIONALLY when present (no core.commitGraph gate), so the git2
//! revwalk in `graph::compute_graph` and the merge-base/ahead-behind in
//! `health` get faster for free. Best-effort: git absent or a non-zero exit is
//! Skipped cleanly — libgit2 still works without the file, so we NEVER error.

use std::path::Path;

use crate::git::search::{GitRunner, SpawnGitRunner};

/// Repo-relative path of the single-file commit-graph (used by tests + the
/// fixture existence-guard). Non-`--split` writes land here.
pub const COMMIT_GRAPH_REL: &str = ".git/objects/info/commit-graph";

/// Outcome of a write attempt. Best-effort ⇒ never an `Err`. Trigger sites
/// discard it (`let _ = …`); tests assert `Written` under a `have_git` guard.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommitGraphOutcome {
    /// `git commit-graph write` ran and exited 0 (the file may still be absent
    /// for an unborn / commit-less repo — git writes nothing then).
    Written,
    /// git not on PATH, spawn failure, or non-zero exit. String = a short
    /// reason for optional debug logging; not surfaced to the user.
    Skipped(String),
}

/// The exact argv (injection-free — no user input). Pure; unit-tested.
/// `["commit-graph", "write", "--reachable", "--changed-paths"]`.
pub fn commit_graph_args() -> Vec<String>;

/// Blocking. (Re)writes the commit-graph for the repo at `workdir` via `runner`
/// (`runner.run(&commit_graph_args(), workdir)`). NEVER returns `Err`:
/// `Ok(_)` → `Written`, `Err(_)` → `Skipped(msg)`.
pub fn write_commit_graph(workdir: &Path, runner: &dyn GitRunner) -> CommitGraphOutcome;

/// Convenience for the fire-and-forget trigger sites: runs with the real
/// `SpawnGitRunner` so callers never import from `search`.
pub fn write_commit_graph_best_effort(workdir: &Path) -> CommitGraphOutcome;
```

Notes for the implementer:
- `write_commit_graph` = `match runner.run(&commit_graph_args(), workdir) { Ok(_) => Written,
  Err(e) => Skipped(e.to_string()) }`. Nothing else. No panics, no `?`.
- `SpawnGitRunner::run` sets `current_dir(workdir)`; git resolves `.git` from there. Bonsai only
  opens non-bare working copies, so `workdir` is always the repo root — correct cwd.
- Keep the file under the ~500-line limit (it is tiny).

---

## 3. Trigger sites (exact wiring)

All three are **fire-and-forget** `spawn_blocking` — never `await`ed — so the write never delays the
op or its response. `use bonsai_core::git::maintenance;` at each site.

### 3.1 Repo open — `src-tauri/src/commands/repo.rs` (`open_repo` command)
Inside the existing `if info.is_repo && !info.bare { … }` block, **after** the warm-on-open
credential `spawn_blocking` (same shape, same rationale):
```rust
// P52: (re)write the commit-graph so libgit2's revwalk/merge-base skip
// re-parsing commit objects. Fire-and-forget, best-effort, off the UI path
// (like warm-on-open). No error path: write_commit_graph_best_effort never Errs.
let cg_workdir = info.path.clone();
tauri::async_runtime::spawn_blocking(move || {
    let _ = maintenance::write_commit_graph_best_effort(std::path::Path::new(&cg_workdir));
});
```
Placed in the command, NOT `open_repo_inner`, so the unit-tested core spawns no subprocess.

### 3.2 Fetch / Pull — `src-tauri/src/commands/remotes.rs`
Wrap the command bodies (keep `fetch_inner`/`pull_inner` untouched — testable cores stay
subprocess-free):
```rust
#[tauri::command]
pub async fn fetch(state: State<'_, AppState>, repo_id: String) -> Result<FetchResult, AppError> {
    let result = fetch_inner(state.inner(), &repo_id).await?;
    if result.remotes.iter().any(|r| r.updated_refs > 0) {        // only when refs advanced
        if let Ok(path) = repo_path(state.inner(), &repo_id) {
            tauri::async_runtime::spawn_blocking(move || {
                let _ = maintenance::write_commit_graph_best_effort(&path);
            });
        }
    }
    Ok(result)
}
```
`pull`: same pattern after `pull_inner` returns `Ok`. Gate on the pull having advanced if
`PullResult` exposes it; otherwise unconditional is acceptable (pull is user-initiated + low
frequency, and a no-op rewrite is harmless).

### 3.3 Scheduler autoFetch — `src-tauri/src/scheduler.rs` (`execute_job`)
In the `JobKind::AutoFetch` → `Ok(Ok(fr))` arm, right where `updated` is computed:
```rust
let updated: u32 = fr.remotes.iter().map(|r| r.updated_refs).fold(0, u32::saturating_add);
if updated > 0 {
    let cg_path = path.clone();  // `path` is still owned here
    tauri::async_runtime::spawn_blocking(move || {
        let _ = bonsai_core::git::maintenance::write_commit_graph_best_effort(&cg_path);
    });
}
RunResult::Success { updated_refs: Some(updated), emit_repo_changed: updated > 0 }
```
Gating on `updated > 0` avoids a pointless rewrite on the (common) no-op auto-fetch tick.

---

## 4. Config decision (D3, normative)

- **No `core.commitGraph` write.** libgit2 ignores it (D1); shelled `git` defaults it true.
- **No new setting / IPC / TS / mock.** Always-on best-effort maintenance.
- If OQ1/OQ2 flip, Appendix A gives the additive toggle design; it is out of scope otherwise.

---

## 5. `--changed-paths` analysis (D6, normative)

| Consumer | Backend | Base graph helps? | Bloom filters (`--changed-paths`) help? |
|---|---|---|---|
| `graph::compute_graph` revwalk | git2/libgit2 | **Yes** (gen numbers, inline meta) | No |
| `health` stale + `graph_ahead_behind` (merge-base) | git2/libgit2 | **Yes** (`paint_down_to_common`) | No |
| `health` stats revwalk | git2/libgit2 | **Yes** | No |
| `search.rs` **path** mode (`git log -- <pathspec>`) | shelled `git` | Yes | **Yes** |
| `search.rs` content pickaxe (`-S`/`-G`) | shelled `git` | Yes | No (pickaxe) |
| `blame::file_history`, git2 blame | git2/libgit2 | Yes (walk) | No (libgit2 ignores Bloom) |

Keep `--changed-paths`: cheap relative to the base write, and the only win for path search.

---

## 6. Perf: fixture + gates + the health-gate fix

### 6.1 `ensure_default_fixture` writes the graph once (`crates/bonsai-core/src/fixture.rs`)
The fixture is a cached, immutable synthetic history whose refs never change, so write the graph
**once when it is missing** — this covers both a fresh generation AND a pre-P52 cached fixture, and
avoids a full rewrite on every call. Restructure the tail of `ensure_default_fixture` (do NOT leave
the early cache-hit `return` bypassing this):
```rust
// … after resolving repo_path and generating on cache-miss (existing logic),
// on BOTH the cache-hit and fresh paths, before returning:
let cg = repo_path.join("objects/info/commit-graph");        // repo_path is the workdir; .git resolved by git
let cg_git = repo_path.join(".git/objects/info/commit-graph");
if !cg.exists() && !cg_git.exists() {
    // Best-effort, have_git-guarded inside write_commit_graph_best_effort.
    let _ = crate::git::maintenance::write_commit_graph_best_effort(&repo_path);
    eprintln!("[fixture] commit-graph write attempted");
}
Ok(repo_path)
```
(The fixture is a normal non-bare repo → the file lands at `.git/objects/info/commit-graph`; the
double check is just defensive.) `git` absent ⇒ no file ⇒ the gate degrades to the old, slower path
(acceptable; the CLI-oracle tests already require git on the dev/CI machine).

### 6.2 `perf_gate.rs` — no code change
`layout_31k_under_500ms` calls `ensure_default_fixture`, so it now measures `compute_graph` **with**
the graph present (faster; still asserts < 500 ms — more margin). Add a one-line comment noting the
fixture now carries a commit-graph. (This changes the M2 gate's *baseline* favourably — flag in the
increment report, it is an improvement not a regression.)

### 6.3 `health.rs` — assert the graph is present, keep the budgets (`perf_ceiling_on_20k_fixture`)
After `ensure_default_fixture`, assert the file exists (proves setup), then keep the existing best-of-3
measurement and the `best_stats < 1500` / `best_total < 2000` asserts:
```rust
let repo_path = crate::fixture::ensure_default_fixture().expect("fixture");
if have_git() {                                              // graph only exists when git wrote it
    assert!(repo_path.join(".git/objects/info/commit-graph").exists(),
            "P52: fixture must carry a commit-graph for the perf measurement");
}
// … unchanged warm-up + best-of-3 + budget asserts …
```

**Why this should turn the gate green.** The dominant cost is the **branches** section:
`find_stale_branches` computes a merge-base per local branch (~40+ kept `feat-*`/`long-*` refs) against
the base, and `graph_ahead_behind` runs the upstream merge-base — all over deep 20k+ history. Without a
commit-graph each merge-base does per-commit ODB inflate; with one, `paint_down_to_common` uses
generation numbers to cut off early — the classic commit-graph win. The stats revwalk also drops
(`git_commit_list_parse` reads the graph instead of inflating 31k commits). Combined, `best_total`
should fall well under 2000 ms.

**Realistic expectation / caveats (Q6).** The commit-graph does **not** accelerate the stats section's
`odb.foreach` header scan or the workdir/`.git` directory walks — those are `read_dir`/`metadata`
syscalls that Windows Defender scans regardless. The fixture is a single packfile (Defender mmaps it
once, not per object), so the graph's win is concentrated in revwalk + merge-base. If, after
implementing, `best_stats` alone still hovers near 1500 ms because of the ODB/dir scans, that is an
**orthogonal** cost the graph cannot fix — the senior-dev must MEASURE and report per-section timings;
the orchestrator then decides (accept, or a justified budget note). **Do NOT silently raise budgets.**

### 6.4 Optional speedup-quantifier (not a gate)
An `#[ignore]` test may generate a small throwaway fixture (e.g. `main_len: 5_000`), time
`compute_graph` + `collect_branches`, write the commit-graph, time again, and `println!` both — giving
the orchestrator a concrete before/after number without disturbing the cached 31k fixture. Keep it
small; skip if it inflates P52b.

---

## 7. Test plan — `#[cfg(test)]` in `maintenance.rs`

`have_git()` guards every CLI test (skip when git absent, like `search.rs`/`stale.rs`). Windows: the
test-running subagent sets `TMP`/`TEMP` to `D:\Temp` (MEMORY rule). Fixtures use
`crate::testutil::scratch_dir()`.

1. **`commit_graph_args_are_exact`** (pure, no git): `commit_graph_args()` ==
   `["commit-graph", "write", "--reachable", "--changed-paths"]`.
2. **`git_absent_or_failure_skips_cleanly`** (no git): a fake `GitRunner` returning
   `Err(AppError::Git("boom"))` ⇒ `write_commit_graph(dir, &fake) == Skipped(_)`; never panics.
   (Reuse the `FakeGitRunner`/panic-free pattern from `search.rs` tests.)
3. **`write_produces_commit_graph_file`** (have_git): build a small multi-commit repo, call
   `write_commit_graph_best_effort(workdir)` ⇒ outcome `Written` AND
   `workdir/.git/objects/info/commit-graph` exists.
4. **`revwalk_layout_identical_with_and_without_graph`** (have_git) — the load-bearing correctness
   oracle: build a fork+merge fixture, `compute_graph` BEFORE, then `write_commit_graph_best_effort`,
   then `compute_graph` AFTER; assert the two `GraphLayout`s are **equal** (the graph is a pure
   optimization — identical output, only faster).
5. **`branches_scan_identical_with_and_without_graph`** (have_git): `health::collect_branches`
   (or a public shim) before vs after writing the graph ⇒ equal `BranchesSection`. Proves merge-base
   results are unchanged by the graph. (If `collect_branches` stays private, assert via
   `collect_repo_health(..).branches.data`.)
6. **`argv_passed_to_runner_is_exact`** (no git): a recording fake runner captures the argv handed to
   `run`; assert it equals `commit_graph_args()` and cwd == workdir.

---

## 8. Sub-increment split + acceptance

### P52a — maintenance fn + triggers (backend only)
Scope: `git/maintenance.rs` (types, `commit_graph_args`, `write_commit_graph`,
`write_commit_graph_best_effort`, tests §7.1–§7.6); `git/mod.rs` (`pub mod maintenance;`); the three
trigger sites §3.1–§3.3.
**Acceptance:**
1. `cargo test -p bonsai-core maintenance` green incl. §7's arg test, clean-skip test, file-produced
   test, and the **before/after layout-equality** oracle.
2. `cargo build` + `cargo clippy -- -D warnings` clean; `cargo test` (whole crate) still green.
3. No new Tauri command; `generate_handler!` unchanged; no TS/mock change (D3).
4. Opening a repo / fetching with new refs writes `.git/objects/info/commit-graph`; with `git`
   removed from PATH the app still opens/fetches with **no error** (best-effort skip — proven at the
   fn level by §7.2, confirmed natively in the USER CHECKPOINT).
5. No file over the ~500-line limit.

### P52b — perf fixture + gates + health-gate fix
Scope: `fixture.rs` (`ensure_default_fixture` §6.1); `health.rs` perf test assertion §6.3;
`perf_gate.rs` comment §6.2; optional quantifier §6.4.
**Acceptance:**
1. `cargo test --release -p bonsai-core -- --nocapture perf_ceiling_on_20k_fixture` **passes** with the
   commit-graph present; the report includes the printed per-section timings.
2. If (1) does NOT pass, the senior-dev reports the measured per-section timings (stats vs branches
   vs total) so the orchestrator can decide — **budgets are not changed without sign-off.**
3. `cargo test --release --test perf_gate -- --ignored --nocapture` still passes (< 500 ms, now with
   the graph); the fixture-generation log shows the commit-graph write was attempted.
4. `ensure_default_fixture` writes the graph at most once (existence-guarded) — reused fixtures do not
   re-pay a full rewrite.

---

## 9. Acceptance criteria (milestone)

- **AI gate:** P52a + P52b acceptance above; the health perf gate is green (or its failure is a
  documented orthogonal-cost decision, not a silent budget bump); before/after layout + branches
  equality proven; browser harness unaffected (no IPC change — sanity `pnpm build`/`tsc` clean).
- **USER CHECKPOINT:** `docs/contracts/P52-user-checklist.md` — native round-trip on a large real repo
  (graph file appears; graph/blame/health feel faster; git-absent degrades with no error; no refresh
  loop on open).

---

## 10. Open questions (flag to orchestrator)

- **OQ1 — `core.commitGraph` Local write?** Recommend **no** (libgit2 ignores it; shelled git defaults
  it true; avoid mutating the user's repo config). Confirm, or opt into a belt-and-suspenders Local
  `core.commitGraph=true` write.
- **OQ2 — Maintenance toggle setting?** Recommend **always-on, no toggle** (D3). Appendix A has the
  additive design if opt-out is wanted.
- **OQ3 — `--split` incremental writes?** Recommend plain `--reachable` for v1 (single file, easy to
  verify, infrequent cadence). `--split` (chain under `objects/info/commit-graphs/`) is a future
  large-repo optimization — would require the test/fixture existence-checks to look for the chain dir.
- **OQ4 — keep `--changed-paths`?** Recommend **keep** (cheap; the only win for path search). Drop only
  if write time on huge repos proves a problem (D6).
- **OQ5 — pull trigger gating?** Recommend gate fetch on `updated_refs > 0` and scheduler autoFetch on
  `updated > 0`; trigger `pull` unconditionally on `Ok` (low frequency). Confirm, or gate pull too if
  `PullResult` exposes movement.

---

## Appendix A — optional maintenance toggle (only if OQ2 flips)

Additive, mirrors P49 `terminalCommand`/P30 `healthRefresh`:
- `settings.rs Settings`: add `pub commit_graph_maintenance: bool` with `#[serde(default = …)]`
  defaulting **true** (add a `fn default_true() -> bool { true }` since bare `#[serde(default)]` on a
  `bool` yields `false`); include in `Settings::default()`; add a back-compat test proving a pre-P52
  file loads it `true`.
- TS mirror in `src/ipc/types.ts` (`commitGraphMaintenance: boolean`) + the settings mock; a
  Settings-panel checkbox under an "Advanced / Performance" group.
- The three trigger sites skip the write when the flag is false. The flag must be threaded to the
  scheduler (it already snapshots settings via `JobsConfig`/`apply_config`) and read in the commands
  from `settings::load_from`. This is the extra surface D3 avoids by defaulting always-on.
