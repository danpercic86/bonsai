# Coverage baseline — testing campaign 2026-08

T1 records baselines only; NO thresholds are enforced yet (contract §1.3/§3).

## Frontend (vitest v8 coverage)

Produced by `pnpm test:coverage` → `coverage/` (text + html + json-summary).
Aggregates the `node` and `dom` projects; excludes tests, `src/test/**`,
`src/ipc/tauri.ts` (Tauri-runtime-only), and `.d.ts`.

Baseline (2026-08-09, T1 increment):

| metric | % | covered/total |
|---|---|---|
| statements | 3.98 | 482/12101 |
| branches | 4.00 | 316/7897 |
| functions | 2.98 | 84/2812 |
| lines | 3.87 | 412/10643 |

(Expected low: pre-T1 the suite was pure-logic `node` tests only; component
coverage starts in Phase T3.)

After T3 (2026-08-09, 109 files / 1317 tests):

| metric | % (all files) |
|---|---|
| statements | 61.18 |
| branches | 54.51 |
| functions | 56.44 |
| lines | 62.71 |

The uncovered remainder is dominated by the `RepoWorkspace.tsx` container (state
wiring — exercised end-to-end by Playwright, not unit tests), the `GraphCanvas.tsx`
imperative paint loop (pure geometry/viewport/hitTest extracted + unit-tested in
T3.6; the `ctx.*` draw calls are covered by e2e specs 02/03), and `ipc/tauri.ts`
(Tauri-runtime-only, coverage-excluded). Component/hook/util/mock logic is broadly
covered; the e2e suite (88 journeys) covers the integrated container paths.

## Rust (cargo llvm-cov)

One-time setup: `rustup component add llvm-tools-preview` then
`cargo install cargo-llvm-cov --locked` (compiled locally — never cargo-binstall).

Baseline run (PowerShell, sequential — NEVER concurrent with other cargo commands;
llvm-cov uses its own profile dir, so expect a full rebuild — do not interleave
with `cargo test`/`clippy`):

```powershell
$env:TMP = 'D:\Temp'; $env:TEMP = 'D:\Temp'
cargo llvm-cov --workspace --summary-only
cargo llvm-cov --workspace --html --output-dir D:\Temp\bonsai-llvm-cov
```

Per-crate line/region % baseline: PENDING (run deferred to the orchestrator —
full-workspace instrumented rebuild + test run).

### T5a — property-based + corrupt-repo + race/lifecycle (2026-08-10)

New bonsai-core integration suites (dev-dep `proptest = "1.6"`):

- `tests/prop_common/mod.rs` — bounded `RepoShape` strategy (≤200 commits, ≤8 branches/tags,
  ~20% merges, ~2% duplicate-parent), `build_repo` (unique tree/commit, auto leaf branches for full
  reachability), `diff_pair`, and the status porcelain-oracle mapping (ported from
  `status_porcelain.rs`).
- `prop_graph_layout.rs` — `compute_graph` node bijection / topological order / parent truth / lane
  density / edge=parent-link set / head_index+detached / same-input determinism. Lane-append
  stability pinned as F-T5-1.
- `prop_intraline.rs` — span well-formedness (ascending, in-bounds code points, coalesced),
  identical/over-cap ⇒ empty, astral offsets, context/surplus rows empty. Swap-asymmetry pinned
  F-T5-2.
- `prop_history_index.rs` — BM25 round-trip (unique token ⇒ sole top hit; absent ⇒ none), idf finite
  ≥0, tf monotonicity, rank sort/cap contract; + one git-backed `build_index`→`search_history` e2e.
- `prop_status.rs` — random create/modify/delete/stage/unstage sequences vs `git status --porcelain`
  (32-cap in-file). fs-rename divergence pinned F-T5-3.
- `prop_stash_roundtrip.rs` — random staged/unstaged/untracked ⇒ create_stash → apply_stash worktree
  byte-identity (AllWithUntracked + All); index split intentionally NOT restored (no REINSTATE_INDEX,
  pinned per contract §8.4).
- `corrupt_repo_cli.rs` — 10-cell matrix + 3 extras (each surface watchdog'd, no panic; behaviors
  pinned; C1 hang = F-T5-4).
- `race_lifecycle_cli.rs` — write-storm-during-commit, concurrent status∥commit, ops-on-deleted-repo.

App-code touched (test-only): `src/git/intraline.rs` gained the `#[doc(hidden)]
annotate_hunk_for_tests` forwarder (zero behavior change).
