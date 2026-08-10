# P65 — Incremental / paged graph loading: native USER CHECKPOINT checklist

P65 shipped in three sub-increments, all committed:
**P65a** Rust core (shared `LaneWalker` + `stream_graph` channel; `compute_graph` output unchanged),
**P65b** frontend + mock (incremental edge index + stream assembler + `RepoWorkspace.refetchGraph`
switched to `streamGraph`), **P65c** the 200k first-paint latency test.

## AI gate (verified by the orchestrator)
- **Lane-color stability across page boundaries** — `stream_graph_core` streamed at batch sizes
  1/2/3/7/512 assembles to a layout byte-identical to `compute_graph` on every M2 fixture (E1–E6) +
  a mid-size fixture (graph.rs `stream_matches_compute_graph_across_batch_sizes`); the existing E1–E6
  fixture tests are unchanged and still pass. Stability is true by construction (one shared walk).
- **Frontend assembler equivalence** — `streamAssembler` reconstructs a chunk-split fixture deep-equal
  to the un-chunked layout (ordered parents, edges as a set, laneCount, headIndex); `incrementalEdgeIndex`
  matches a from-sorted brute-force oracle. (vitest, 14 tests.)
- **Streaming correctness at scale (NOT a first-paint speed gate — see the finding below)** — `#[ignore]`
  release-gate Rust test `stream_first_batch_under_ms` (`crates/bonsai-core/tests/stream_perf.rs`) builds a
  120k-commit repo and asserts the full stream completes with `total_rows == 120000` and `truncated == false`
  and that delivery is incremental (first batch arrives before the full stream), and PRINTS the measured
  first-batch / full-stream latencies. It deliberately does **not** assert a `< 150 ms` first-batch bound —
  see the finding.

> **FINDING (why there is no first-paint speed gate).** The original contract targeted first paint `< 150 ms`
> on 200k+ repos. That is **not achievable with libgit2's topological revwalk**: `git2`'s
> `Sort::TOPOLOGICAL` runs an eager `prepare_walk` that drains the entire reachable graph before yielding
> row 0, so first-batch latency is **O(total commits)** (measured, release/warm: 40k≈0.73 s, 120k≈1.37 s,
> 200k≈2.3 s), not O(the first 512 rows). Streaming still delivers real wins — lane-stable **progressive**
> render, background scroll-ahead, and no single giant IPC / 100k transfer wall — but the "instant first
> paint on a 1M-commit repo" headline is **NOT met for the topo-ordered walk in P65**. The proper fix (git's
> lazy generation-number `--topo-order`, sourcing generation numbers from the commit-graph file since `git2`
> exposes none) is scoped and deferred to **P66** — see `docs/contracts/P65a-lazy-topo-spike.md`.
- **Wire fidelity** — `GraphChunk` serialization pinned to the TS mirror (serde wire-shape tests).
- **Structural harness pass** (`?fixture=20k`, mock): the streamed load fills the full 20 001-row scroll
  extent, the canvas renders (virtualized), and scrolling the entire range throws no console errors
  (exercises the incremental edge index end-to-end). Command count 156 → **157** (`stream_graph`;
  `get_graph` retained).
- Full gate green: `cargo test --workspace` (pre-P65c) 1607/0/3-ignored + P65a/b/c crate suites,
  `pnpm vitest` 1331/0, `tsc` 0, `pnpm build`, clippy `-D warnings` clean.

## Why the two items below are USER CHECKPOINTs (cannot be self-verified here)
The headless Browser harness composites the pane at 0×0, so `document.visibilityState === "hidden"`
and the browser **pauses `requestAnimationFrame`**. Any frame-timing sweep (`window.__bonsai.scrollSweep`)
therefore cannot progress — it waits on rAF ticks that never fire. Frame-rate smoothness and canvas
pixels both require a **visible window**. (The streamed path renders through the SAME virtualized
`GraphCanvas` as the one-shot path — only the edge index + spacer differ, both O(visible) — so per-frame
work is equivalent to the one-shot path that already meets the 20k gate; but the measurement itself is
native-only.)

## Verify in `pnpm tauri dev`
1. **Large-repo progressive load & scroll (what P65 DOES deliver).** Open a REAL 100k–1M-commit repo. Note:
   the FIRST paint is currently NOT instant — it waits on libgit2's O(n) topo-prep (a few seconds on 200k+;
   this is the deferred-to-P66 finding above, not a bug). What to verify P65 delivers: once the first batch
   lands, the graph paints **progressively** top-down as batches stream in (not one frozen wait for the whole
   history to transfer); scrolling the loaded region is smooth; scrolling DOWN into a still-loading region
   fills in as the stream catches up with no per-scroll IPC round-trip (the walk streams eagerly ahead). A
   faint "loading history…" tail while the frontier catches up is acceptable. (Instant first paint is the P66
   goal.)
2. **20k scroll no-jank (the scrollSweep gate, run natively)** — on `?fixture=20k` in a **visible**
   window (native, or `pnpm dev:mock` in a real browser tab), run
   `await window.__bonsai.scrollSweep(10000)` in devtools; PASS = `maxWindow5Avg <= 33 && over100 <= 3`.
   The streamed path must match the one-shot number.
3. **Lane-color STABILITY while scrolling (the core promise)** — as you scroll a large repo and new
   batches stream in, each branch lane must keep its color; a lane must NOT change color as later rows
   arrive. (This is what the batch-size-invariance AI test proves in the abstract; confirm it visually.)
4. **Progressive fill visual** — watch a large repo load: the graph paints top-down as batches arrive;
   lanes/edges/ref pills are correct throughout; the scrollbar thumb starts near the top (grow-as-you-go,
   `Meta.total = None`) and the extent settles as rows arrive; the scrollbar ultimately reaches the last
   row.
5. **Repo switch mid-stream** — while a large repo is still streaming, switch to another repo. The switch
   is instant, shows no stale rows from the first repo, and the abandoned background walk causes no lag
   (the frontend generation guard drops its chunks; the backend walk stops when the channel drops).
6. **Selection across a re-stream** — select a commit far down (row ≥ 512), then trigger a refresh /
   watcher tick. No crash (this was a fixed regression), and the selection re-points to the same commit
   once its row streams back in.
7. **Truncation (optional, extreme)** — a repo beyond 1,000,000 reachable commits streams up to the cap
   and stops cleanly with a truncated indication; the app stays responsive (no attempt to render beyond
   the cap).
