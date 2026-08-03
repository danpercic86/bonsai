# P29 — Repo-Health Dashboard (Theme D, item D1)

Status: contract. Read-only insight overlay ("📊 Health") aggregating repo metrics.
Style/precedent: P24 (assets overlay), P27 (worktrees), P28 (digest). Backend composes
SHIPPED primitives; **nothing here writes to the repo — no ODB writes, no config writes,
no worktree touches. READ-ONLY is a hard invariant.**

---

## 1. Decisions (accept-defaults mode — all resolved here)

| # | Decision | Choice + rationale |
|---|----------|--------------------|
| D1 | Metric set | Four sections exactly: `stats`, `branches`, `workingState`, `structure` (§3). Everything in the task's candidate groups is included EXCEPT: repo size "on disk" is split into two cheap numbers (workdir file count/bytes with a cap, `.git` dir bytes with a cap) — no recursive uncapped walks. |
| D2 | Command shape | ONE command `get_repo_health(repoId)` returning all four sections in a single round-trip (IPC invariant: no per-metric round-trips). |
| D3 | Parallelism | **Sequential** inside one `spawn_blocking`. git2 `Repository` is not `Sync`; each section opens its own repo handle anyway, but parallel threads buy little (sections are I/O-bound on the same disk) and add failure modes. Keep it simple; per-section elapsed ms is reported so slowness is visible. |
| D4 | Failure isolation | Each section is `Section<T> { data, error, elapsedMs }`. One section erroring/capping never fails the whole command. The whole command errors ONLY for `NoRepo`/join errors. |
| D5 | Caps (perf budgets) | §5 table. Every capped metric carries a `capped: true` flag on the wire so the UI shows "≥" instead of "=". |
| D6 | Largest blobs | Top 10 by blob size via ODB scan (`odb.foreach` reading header size only — `read_header`, never full `read`), capped at 500k objects. Reported as oid+size; blob→path mapping is NOT computed (a rev→tree walk to name blobs is O(history) — out of scope v1; documented UI copy: "blob <short-oid>"). Exception: we ALSO report top 10 largest **worktree files** (path+size) from the workdir walk, which is what users usually want. |
| D7 | Contributor window | Distinct author emails and commit count over the last 30 days (and total commits on HEAD, capped), from the same single revwalk pass. |
| D8 | Stash count | Included — `Repository::stash_foreach` count is O(reflog of refs/stash), cheap. |
| D9 | Stale branches | Reuse `find_stale_branches(workdir, None)` verbatim; if base resolution fails (`AppError::Git`, e.g. no main/master and detached HEAD), the branches section still succeeds with `stale: null` + `staleError` string — stale info is a sub-metric, not the section. |
| D10 | AI-asset drift | Reuse `assets::scan_inventory(workdir, None)`; report `driftedCount` + `inSync` only (full detail lives in the AI Assets panel). |
| D11 | Refresh policy | Panel fetches on open + on `repo-changed` (debounced by the existing event pipeline) + manual Refresh button — mirrors `AiAssetsPanel`. NOT fetched while the panel is closed (this scan is heavier than status). |
| D12 | Header button | `📊 Health`, placed next to the existing `🤖 AI Assets` button; overlay wired through the same `globalModalOpen` mechanism in `App.tsx`. |
| D13 | Large-file threshold | Worktree files ≥ 10 MiB counted as `largeFileCount` (warn badge when > 0). Threshold is a Rust const `LARGE_FILE_BYTES = 10 * 1024 * 1024`, not user-configurable in v1. |
| D14 | Module placement | `crates/bonsai-core/src/health.rs` at the **core root** (not `git/`) because it composes `git::*` AND `assets::*`. Runtime-free (no Tauri types), like every other core module. |
| D15 | Sub-increments | Three passes P29a/b/c (§9). Do NOT merge b+c: the mock fixture + types are review-worthy on their own and keep the UI diff small. |

---

## 2. Module boundaries

| File | Responsibility |
|------|----------------|
| `crates/bonsai-core/src/health.rs` (NEW) | `collect_repo_health(workdir) -> RepoHealth` + per-section collectors + all caps/consts + unit tests. Composes `git::{stale, worktree, submodule, status, opstate, branches}` and `assets::scan_inventory`. Zero new git primitives beyond the ODB/dir scans defined here. |
| `crates/bonsai-core/src/lib.rs` | `pub mod health;` |
| `src-tauri/src/commands.rs` | `get_repo_health` command + `_inner` (pattern of `list_worktrees`, commands.rs:2167). Register in `generate_handler!` in `lib.rs`. |
| `src/ipc/types.ts` | TS mirrors of §4 + `getRepoHealth` on the IPC interface. |
| `src/ipc/tauri.ts` | `invoke('get_repo_health', { repoId })`. |
| `src/ipc/mock.ts` | Fixture `RepoHealth` per §7 (must keep compiling — mandatory harness). |
| `src/components/RepoHealthPanel.tsx` (NEW) | Overlay panel, §8. Renders only; no logic beyond formatting. |
| `src/App.tsx` | Header button + open-state + `globalModalOpen` wiring (copy the 🤖 pattern). |

---

## 3. Metric set (per section)

**stats** — one revwalk from HEAD (topological not required; plain push_head), capped:
total commit count on HEAD (capped), commits in last 30 days, distinct author emails in
last 30 days, distinct author emails overall-within-cap; ODB object count + top-10 blobs
(D6); workdir file count + total bytes + top-10 largest files + large-file count (one
capped walk skipping `.git` and honoring nothing else — ignored files ARE counted, they
occupy disk); `.git` dir byte size (capped walk).

**branches** — local count, remote-tracking count, tag count (`references_glob` counts);
current-branch name + ahead/behind vs upstream (reuse the `branches.rs` logic:
`graph_ahead_behind`, best-effort None); detached-HEAD flag; stale rollup via
`find_stale_branches` (mergedCount, goneUpstreamCount, D9).

**workingState** — from `read_status`: staged/unstaged/untracked/conflicted counts;
op state via `read_op_state` (reuse `RepoOpState` wire type verbatim); stash count (D8);
`.gitignore` presence (workdir root file check — placed here, it's a working-tree fact).

**structure** — submodules: total + per-`SubmoduleStatus` counts (reuse
`list_submodules`); worktrees: total + locked + prunable + invalid counts (reuse
`list_worktrees`); AI-asset drift rollup (D10).

---

## 4. Wire types (Rust — serde camelCase; TS mirrors 1:1)

```rust
// crates/bonsai-core/src/health.rs — all #[derive(Debug, Clone, PartialEq, serde::Serialize)]
// + #[serde(rename_all = "camelCase")] unless noted.

/// Per-section envelope (D4). Exactly one of data/error is Some.
pub struct Section<T> {                    // T: serde::Serialize
    pub data: Option<T>,
    pub error: Option<String>,             // human message from AppError::to_string()
    pub elapsed_ms: u32,
}

pub struct RepoHealth {
    pub stats: Section<StatsSection>,
    pub branches: Section<BranchesSection>,
    pub working_state: Section<WorkingStateSection>,
    pub structure: Section<StructureSection>,
    pub generated_at: i64,                 // epoch seconds
}

pub struct StatsSection {
    pub commit_count: u32,     pub commit_count_capped: bool,   // revwalk cap hit
    pub commits_last_30d: u32,                                  // within the same capped walk
    pub authors_last_30d: u32,
    pub authors_total: u32,    // distinct within the capped walk
    pub object_count: u64,     pub object_scan_capped: bool,    // odb.foreach cap
    pub largest_blobs: Vec<BlobStat>,        // top 10 desc by size (D6)
    pub workdir_file_count: u32, pub workdir_bytes: u64, pub workdir_scan_capped: bool,
    pub largest_files: Vec<FileStat>,        // top 10 desc, paths fwd-slash repo-relative
    pub large_file_count: u32,               // files >= LARGE_FILE_BYTES (D13)
    pub git_dir_bytes: u64,    pub git_dir_scan_capped: bool,
}
pub struct BlobStat { pub oid: String /* 40-hex */, pub size: u64 }
pub struct FileStat { pub path: String, pub size: u64 }

pub struct BranchesSection {
    pub local_count: u32, pub remote_count: u32, pub tag_count: u32,
    pub current_branch: Option<String>,      // None = detached/unborn
    pub detached: bool, pub unborn: bool,
    pub ahead: Option<u32>, pub behind: Option<u32>,   // vs upstream, best-effort
    pub upstream: Option<String>,
    pub stale: Option<StaleRollup>,          // None when stale scan failed (D9)
    pub stale_error: Option<String>,
}
pub struct StaleRollup { pub base: String, pub merged_count: u32, pub gone_upstream_count: u32 }

pub struct WorkingStateSection {
    pub staged: u32, pub unstaged: u32, pub untracked: u32, pub conflicted: u32,
    pub op_state: crate::git::opstate::RepoOpState,    // reuse wire type verbatim
    pub stash_count: u32,
    pub has_gitignore: bool,
}

pub struct StructureSection {
    pub submodule_count: u32, pub submodules_uninitialized: u32,
    pub submodules_out_of_sync: u32, pub submodules_modified: u32,
    pub worktree_count: u32,           // includes the synthesized main row
    pub worktrees_locked: u32, pub worktrees_prunable: u32, pub worktrees_invalid: u32,
    pub asset_drifted_count: u32, pub assets_in_sync: bool,
}
```

Core entry point (blocking; command layer wraps in `spawn_blocking`):

```rust
pub fn collect_repo_health(workdir: &Path) -> RepoHealth;
// Never Err: each section collector is Result<T, AppError>, folded into Section<T>.
// Internal collectors (private): collect_stats / collect_branches /
// collect_working_state / collect_structure, each fn(&Path) -> Result<X, AppError>.
```

Command (src-tauri/src/commands.rs, exact pattern of `list_worktrees`):

```rust
#[tauri::command]
pub async fn get_repo_health(
    state: tauri::State<'_, AppState>, repo_id: String,
) -> Result<RepoHealth, AppError>;   // NoRepo via repo_path(); join error → AppError::Other
```

TypeScript (src/ipc/types.ts — mirror every struct; `Section<T>` generic):

```ts
export interface Section<T> { data: T | null; error: string | null; elapsedMs: number }
export interface RepoHealth { stats: Section<StatsSection>; branches: Section<BranchesSection>;
  workingState: Section<WorkingStateSection>; structure: Section<StructureSection>;
  generatedAt: number }
// + StatsSection/BlobStat/FileStat/BranchesSection/StaleRollup/WorkingStateSection/
//   StructureSection mirrors (camelCase). RepoOpState already exists in types.ts — reuse.
// IPC interface addition:
getRepoHealth(repoId: string): Promise<RepoHealth>;
```

Events: none new. Channels: none (payload is small — counts + 20 stat rows).

---

## 5. Perf budgets & caps (Rust consts in health.rs)

| Const | Value | Applies to |
|-------|-------|-----------|
| `REVWALK_CAP` | `100_000` commits | stats revwalk (count/authors/30d). Stop, set `commit_count_capped`. |
| `ODB_SCAN_CAP` | `500_000` objects | `odb.foreach` for object count + largest blobs. `read_header` only (type+size), NEVER `read`. Stop via returning `false`, set `object_scan_capped`. |
| `WORKDIR_WALK_CAP` | `200_000` entries | workdir file count/bytes/largest/large-count. Iterative dir walk, skip `.git`, never follow symlinks/junctions. Early-stop + `workdir_scan_capped`. |
| `GITDIR_WALK_CAP` | `200_000` entries | `.git` byte size. Same walker, `git_dir_scan_capped`. |
| `TOP_N` | `10` | largest blobs and largest files (min-heap of size N, O(n log N)). |
| `LARGE_FILE_BYTES` | `10 MiB` | large-file warn threshold. |

Budget: on the existing 20k-commit fixture (`crates/bonsai-core/src/fixture.rs`
`generate_fixture`), `collect_repo_health` completes in **< 2 s** total and the stats
section alone **< 1.5 s** (asserted with a coarse `Instant` bound in a `#[test]`, not
criterion — this is a ceiling test, not a benchmark). No section may allocate
proportionally to object CONTENT (headers/paths/sizes only).

---

## 6. Error taxonomy & safety

- Existing `AppError` kinds only. Expected per-section: `Git`, `Io`. Whole-command:
  `NoRepo` (unknown repoId), `Other` (join). No new variants.
- READ-ONLY guarantee: health.rs calls only read APIs (`revwalk`, `odb.read_header`,
  `references_glob`, `statuses`, `stash_foreach` — note: takes `&mut Repository` but
  performs no writes — `find_stale_branches`, `list_worktrees`, `list_submodules`,
  `read_op_state`, `scan_inventory`, fs metadata reads). Reviewer MUST verify no call
  to `delete_*`, `stash_save`, `Odb::write`, config setters, or any `std::fs` write.

---

## 7. Mock fixture (src/ipc/mock.ts)

Add a canned `RepoHealth` served by `getRepoHealth` (static; regenerate `generatedAt`
per call). It MUST exercise every warn state:
- `branches.stale = { base: 'main', mergedCount: 3, goneUpstreamCount: 1 }`, `ahead: 2, behind: 5`;
- `structure`: 1 locked worktree, 1 prunable, 1 out-of-sync submodule, 1 uninitialized,
  `assetDriftedCount: 2, assetsInSync: false`;
- `stats`: `largeFileCount: 2`, two `largestFiles` > 10 MiB, `commitCountCapped: true`;
- `workingState`: `conflicted: 1`, `opState: { kind: 'merge', incoming: 'feature/x', message: '...' }`,
  `stashCount: 2`, `hasGitignore: true`;
- ONE section demonstrating the error path: none by default, but the mock must flip
  `stats` to `{ data: null, error: 'simulated slow scan failed', elapsedMs: 1500 }` when
  the mock repo id ends with `-err` (cheap harness hook, same trick as other mocks).

---

## 8. Frontend — RepoHealthPanel

- `RepoHealthPanelProps { open: boolean; onClose(): void; repoId: string }` — mirror
  `AiAssetsPanel` overlay chrome (backdrop, header row, ✕, Esc closes).
- Header button in `App.tsx`: `📊 Health`, next to 🤖; opening sets the same
  `globalModalOpen` guard.
- Fetch on open + on `repoId` change while open + on `repo-changed` while open (D11);
  Refresh button re-fetches; a small "generated <relative time>" caption.
- Four titled sections in order stats → branches → working state → structure. **Each
  section renders independently**: while loading show a per-section skeleton; on
  `section.error` show an inline error row (reuse existing error styling) WITHOUT
  hiding sibling sections.
- Badges: reuse existing chip classes (`asset-chip`, `asset-chip-sync`,
  `asset-chip-drifted`, `asset-chip-muted` — or the app's generic badge classes if the
  reviewer prefers) — green for healthy (0 stale, in-sync, clean), amber/warn for
  stale>0, drifted>0, locked/prunable/invalid>0, out-of-sync/uninitialized>0,
  largeFileCount>0, conflicted>0, detached HEAD, op in progress, capped flags render a
  `≥` prefix on the number plus a muted "(capped)" chip.
- Sizes formatted KiB/MiB/GiB; oids shortened to 7 chars; largest-blob rows labeled
  `blob <shortOid>` (D6).
- No mutations of any kind from this panel.

---

## 9. Sub-increments

- **P29a — core:** `health.rs` + `lib.rs` export + unit tests (§10.1). No Tauri, no TS.
- **P29b — IPC:** command + registration + types.ts/tauri.ts/mock.ts (incl. §7 fixture
  + `-err` hook). Gate: `cargo check`, `tsc`, mock compiles.
- **P29c — UI:** `RepoHealthPanel.tsx` + App.tsx wiring + styles. Gate: harness renders
  warn states.

---

## 10. Acceptance criteria

### 10.1 AI gate
1. **Unit/oracle tests (P29a, in `health.rs` `#[cfg(test)]`)** — fixtures under
   `crate::testutil::scratch_dir()` (D:\Temp\bonsai-scratch), deterministic identity,
   `core.autocrlf=false`, same as `stale.rs` tests:
   - wire-shape test: full `RepoHealth` serializes camelCase incl. nested
     `Section<T>` and `RepoOpState` (`serde_json` assertions);
   - stats: scratch repo with N commits → `commitCount == N` and equals
     `git rev-list --count HEAD` (CLI oracle, skip if git absent); a >10 MiB file →
     appears in `largestFiles` and `largeFileCount == 1`; `objectCount` ≥ commits+trees+blobs;
     cap test with `REVWALK_CAP` shadowed via a testable inner fn taking the cap as a
     parameter (`collect_stats_with_caps`) → `capped: true` and count == cap;
   - branches: local/remote/tag counts vs `git for-each-ref` oracle; merged + gone
     branches → `staleRollup` counts match `find_stale_branches` output; detached HEAD →
     `detached: true`, `currentBranch: null`; unborn repo → section still Ok;
   - workingState: staged/unstaged/untracked/conflicted counts match `read_status`;
     stash created via git2 `stash_save` **in the test fixture only** → `stashCount: 1`;
     `.gitignore` present/absent flag;
   - structure: repo with a locked + a prunable worktree → counts match
     `list_worktrees`; drifted CLAUDE.md/AGENTS.md pair → `assetDriftedCount ≥ 1`;
   - section isolation: a fixture engineered to fail one collector (e.g. stats on a
     deliberately corrupted odb path via the caps-parameterized inner fn returning Err)
     → that `Section.error` set, other three `data` present;
   - **perf ceiling:** `generate_fixture` 20k+ commits → `collect_repo_health` wall
     time < 2 s (mark `#[ignore]`-free but generous bound; single-threaded).
2. `cargo test` green, then `cargo clippy` (SEQUENTIAL — never concurrent, target-dir
   race), `TMP`/`TEMP` = `D:\Temp` for test runs.
3. `tsc`/`pnpm build` green; mock.ts compiles (mandatory harness invariant).
4. Harness (`pnpm dev` + `VITE_MOCK_IPC=1`): screenshot shows the panel with all four
   sections, warn badges for every §7 warn state, capped `≥` rendering, and (via the
   `-err` repo id) one errored section rendering alongside three healthy ones.

### 10.2 USER CHECKPOINT
- Native app (`pnpm tauri dev`): open the panel on a real repo; numbers are plausible
  (spot-check `git rev-list --count HEAD`, branch counts); Refresh works; panel opens
  responsively (< ~2 s to populated) on the user's biggest local repo; no repo
  mutation observed (`git status` unchanged after opening the panel).

---

## 11. Flags for the orchestrator
- D6 (no blob→path mapping for pack blobs) trims scope; if the user wants named large
  blobs, that is a follow-up (requires a tree-diff walk with its own budget).
- D13 threshold (10 MiB) and all §5 caps are consts — confirm silently unless the user
  objects at checkpoint.
