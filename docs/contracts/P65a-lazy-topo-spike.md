# P65a — Lazy Generation-Number Topo Order — Feasibility Spike (go/no-go)

> Spike memo, **not** an implementation contract. Decides whether first-paint can be made
> O(visible screen) instead of O(total commits). Orchestrator relays to the user BEFORE any build.
> Grounded in: `crates/bonsai-core/src/graph.rs` (`seeded_revwalk`/`collect_seed`),
> `graph/lane.rs` (`LaneWalker`), `graph/stream.rs` (`stream_graph_core`),
> `tests/stream_perf.rs` (the finding), `Cargo.lock` (git2 0.21.0 / libgit2-sys 0.18.7+1.9.6).

## VERDICT: **TRACTABLE — but NOT via git2 alone.**

The lazy walk is implementable and gets first paint to ~O(512·avgdeg·log), independent of total
commits — this is exactly what git's own `--topo-order` does. The blocker is only the **source of
generation numbers**: git2 0.21 / libgit2-sys 0.18 expose **none**. We must read them out-of-band
from the P52 commit-graph file (pure-Rust `gix-commitgraph`, or our own parser), then run the
lazy algorithm in Rust over git2 object access. The equivalence invariant (streamed layout ≡
one-shot layout) is **preserved by construction** because both paths keep sharing the single order
stage → the unchanged `LaneWalker`. What it costs: a one-time **regeneration of every graph
fixture** (the new tie-break differs from libgit2's), and it is best scoped as a **new milestone**,
not a P65a re-open. Recommended path effort: **L**.

---

## 1. git2 0.21 API reality (evidence-backed)

| Question | Finding | Evidence |
|---|---|---|
| Generation numbers on `git2::Commit`? | **No.** | grep `git2-0.21.0/src` for `generation` → 0 hits. `Commit` has `time/author/committer/parents/parent_ids/parent` only. |
| Corrected committer dates exposed? | **No.** | same grep — no generation/corrected-date API at all. |
| Any commit-graph object type in git2? | **No `CommitGraph`, no `commit_graph`.** | grep → 0 hits in the whole crate `src`. |
| Raw FFI via libgit2-sys 0.18? | **Not bound.** `git_commit_graph_*` sys symbols are absent from the crate. | grep `libgit2-sys-0.18.7+1.9.6/lib.rs` for `commit_graph` → 0 hits. |
| Does libgit2 1.9's revwalk do lazy topo itself? | **No** — its topo sort runs the eager `prepare_walk` (full reachable in-degree pass) even with a commit-graph present. | `tests/stream_perf.rs` FINDING: first `Batch` scales with total commits (40k→~0.73s, 120k→~1.37s, 200k→~2.3s), not with `STREAM_FIRST_BATCH`. |
| Would upgrading git2 unlock it? | **No** (as of knowledge cutoff). git2-rs still binds no commit-graph/generation API; the sys API lives in `sys/commit_graph.h` and git2-rs does not wrap it. *Flag: re-check newest git2 before building.* | crates.io / docs.rs `git2::Commit` surface. |

**Access options for generation numbers, ranked:**
1. **`gix-commitgraph`** (pure-Rust, gitoxide component) — opens `.git/objects/info/commit-graph`
   (+ the `commit-graphs/` chain) and exposes per-commit `generation()`. Cleanest; keeps git2 for
   everything else. *Flag: verify it reads git-written split/chain graphs and the v2
   corrected-date/generation chunk, and confirm its dependency footprint.*
2. **Hand-parse the commit-graph file** — more work; must handle split-graph chains + generation v2.
   Only if (1)'s footprint is unacceptable.
3. **Shell out to `git log --topo-order`** — git computes generations internally; see §4(a).
4. **Compute generation numbers on the fly** — requires walking to roots = O(reachable); defeats the
   purpose. Usable ONLY as the "commit not in the graph" branch (git treats those as generation ∞
   and explores them eagerly — small region near tips for a slightly-stale graph).

---

## 2. Algorithm — lazy generation-number topo walk (git's `--topo-order`)

Mirrors git `revision.c` (`init_topo_walk` / `compute_indegrees_to_depth` / `expand_topo_walk`;
Stolee, commit `b454241`). Emits a commit as soon as generation bounds guarantee no unseen child,
with **no full up-front prep**. `indegree(c) == 1 + (unemitted children of c)`; ready ⇔ `== 1`.

```
INPUT : tips[]  (deterministic seed order from collect_seed, deduped, stash W injected)
        repo    (git2 — parent-ids + commit bodies)
        GEN(oid): commit-graph generation (gix-commitgraph, O(1)); else u64::MAX  // git's ∞
OUTPUT: oids in topological order, commit-date priority — pulled lazily by next()

explore_q : max-heap by GEN            // drives the bounded in-degree accumulation
topo_q    : max-heap by (commit_date, oid)   // EMISSION order = today's TIME tiebreak + stable oid
indegree  : HashMap<oid,i32> default 0
min_gen   : u64::MAX

seed:
  for t in tips: indegree[t] += 1; explore_q.push(t); min_gen = min(min_gen, GEN(t))
  compute_indegrees_to_depth(min_gen)
  for t in tips: if indegree[t] == 1 { topo_q.push(t) }   // tip with no walked child = ready

compute_indegrees_to_depth(cutoff):        // only touch commits at/above the frontier generation
  while let Some(c) = explore_q.peek_if(GEN(c) >= cutoff):
    explore_q.pop()
    for p in parents(c) not hidden(I/U):    // git2: find_commit(c).parent_ids()
      if indegree[p] == 0 { explore_q.push(p) }
      indegree[p] += 1

next() -> Option<oid>:                      // one row, in topo order
  let c = topo_q.pop()?;
  for p in parents(c) not hidden:
    if GEN(p) < min_gen { min_gen = GEN(p); compute_indegrees_to_depth(min_gen) }  // deepen
    indegree[p] -= 1
    if indegree[p] == 1 { topo_q.push(p) }  // all children of p emitted → p ready
  Some(c)                                    // hidden I/U filtered downstream, exactly as today
```

**Mapping to git2 0.21:** `parents(c)` = `repo.find_commit(oid)?.parent_ids()` (already used by
`LaneWalker`); with a commit-graph present these parent-ids come from the graph, so explore/indegree
touch only parent-ids + `GEN` — cheap. Commit bodies (author/summary/times) are read **only at emit
time inside `LaneWalker::step`**, unchanged. `next()` replaces the `for oid in revwalk` iterator in
both `layout_walk` and `stream_graph_core` verbatim.

**First-512 cost:** explore/indegree only ever walk commits within a generation window of the tips
(the cutoff keeps descending only as far as `topo_q`'s frontier), so first paint is
~O(512 · avg_degree · log) — output-sensitive, NOT O(total). This is the exact win git measured
when it adopted this for `rev-list --topo-order`. Full stream over 1M rows stays in-process (no
per-row overhead beyond today's).

---

## 3. THE EQUIVALENCE CONSTRAINT (critical)

Today both paths share `seeded_revwalk` (`Sort::TOPOLOGICAL | TIME`) → the same `LaneWalker`.
`LaneWalker` is order-in / lane-out: **change the order and lanes/colors change.**

| Concern | Analysis |
|---|---|
| Still "topological, then commit date"? | **Yes.** Output is topo-valid (child before parent, guaranteed by the indegree gate) and `topo_q` prioritizes by commit date desc — the same basis as libgit2 `TIME`. Product invariant (M2) upheld. |
| Byte-identical to today's `TOPOLOGICAL\|TIME`? | **No — differs only in tie-breaks.** When several concurrently-ready commits share a commit date (sibling branches, rebased/imported ranges, **and our monotonic-per-lane fixtures**), libgit2's heap order vs our `(date,oid)` order can pick a different one. Topology is identical; the *order among date-ties* is not. |
| Runtime equivalence get_graph ≡ stream_graph? | **Preserved by construction.** Both paths consume the SAME single lazy iterator instance feeding the SAME `LaneWalker`. Batching never touches the iterator (same as today). Both run on the same repo state → same commit-graph presence → same order. This is P65a's whole guarantee, and it survives. |
| Blast radius of the tie-break change | **One-time fixture regeneration.** Every test that pins exact lanes/edges/rows on a branchy or equal-timestamp fixture (`graph::tests`, the 31k layout gate, streaming gates) gets new expected values. `searchCommits`→`resolveLayout` and reveal-to-row are **row-index agnostic** (they resolve rows from the delivered layout), so they stay correct as long as both paths agree — which they do. |
| A path where both switch and stay mutually identical? | **Yes — the recommended shape.** Keep `LaneWalker` untouched (order-in/lane-out). Replace ONLY the order stage: a shared `order_oids(repo, tips) -> impl Iterator<Item=oid>` that both `layout_walk` and `stream_graph_core` call in place of `seeded_revwalk`. Both differ from PRE-change output identically; they never differ from each other. Add an equivalence guard test: `get_graph` order == concatenated `stream_graph` rows on a branchy equal-timestamp fixture. |

**Determinism footnote (flag):** with a partially-stale commit-graph, ungraphed tip-region commits
get generation ∞ and are explored eagerly — correct topo output, but same-date ties there *could*
resolve differently than a fully-graphed run. Mitigate by pinning fixture graph-state; runtime
equivalence is unaffected (both paths see the same state).

---

## 4. Fallbacks (ranked)

**(a) Shell out to `git log --topo-order` and parse — SECOND choice / pragmatic.**
Git's C impl does the lazy generation magic for free over the commit-graph we already write (P52).
- Pros: no algorithm to reimplement; fast first paint by construction.
- Cons / risks: (1) introduces a **hard `git`-binary dependency on the core read path** — a real
  architectural regression (libgit2 was chosen to avoid exactly this; P52 only shells out for
  best-effort *maintenance*), forcing a second eager-libgit2 fallback path when git is absent →
  two possible orders → doubled fixture surface. (2) Must reproduce our exact seed (all local +
  remote + tag + HEAD + injected stash `W`, deduped in our order) via positional tip args; git's
  tie-break among positional tips is git's, not ours. (3) Text parsing (`-z`, `%x00` field
  delimiters), non-UTF-8 authors, 120-char summary cap, child-process lifecycle + prompt kill on
  `emit==false`, Windows spawn/AV cost (see ASR memory note). Refs/pills are still computed by our
  `collect_refs`, so only oid+parents+times+author+summary come from git — smaller parse surface.

**(b) Provisional TIME-first screen + reconcile — LAST.**
Seed `Sort::TIME` only (lazy in libgit2) for an instant first 512, render provisionally, compute the
real topo layout in the background, reflow. **User already rejected this.** TIME-only order is not
topo-valid → provisional lanes/edges are wrong and **visibly reflow** (colors/lanes jump) when the
real layout lands — violates "lane colors stay stable while scrolling." High UX risk. Not
recommended.

**(c) Read precomputed generation numbers + run the §2 walk in Rust — FIRST choice (recommended).**
`gix-commitgraph` (or own parser) for `GEN(oid)`; git2 for objects; the §2 lazy walk as the shared
order stage.
- Pros: fully in-process (fast full stream too), **no git-binary hard-dep on the read path**, keeps
  git2 + the shared-engine architecture, we own determinism/tie-break, honors "Rust owns ALL Git
  logic." Ungraphed commits fall back to eager exploration for their (small, tip-local) region only,
  matching git's own graceful degradation → one consistent order regardless of graph presence.
- Cons: reimplement the triple-queue walk (correctness risk — mitigate with a differential test vs
  `git rev-list --topo-order`); add + validate `gix-commitgraph`; the one-time fixture regen (§3).

---

## 5. Effort · risk · recommendation

**Recommend path (c).** It is the only option that keeps first paint O(screen) AND honors the
invariants (Rust owns Git logic; no new runtime binary dependency on the core read path; shared
order stage → equivalence preserved). Use (a) shell-out only if reimplementing §2 is later judged
too risky — accepting the git-binary dependency and a second fallback order.

**Effort: L.** New algorithm + new dep + cross-cutting order change + fixture regeneration + a
differential correctness harness.

**Top risks (recommended path) & mitigations:**
1. Lazy-topo correctness (indegree cutoff subtleties) → differential test asserting our order equals
   `git rev-list --topo-order` on branchy fixtures.
2. `gix-commitgraph` reading git-written split-graph chains + generation semantics → verify against a
   P52-written graph in a spike test before committing to the dep; own-parser is the escape hatch.
3. Fixture blast radius across M2 / search / reveal → one-time regen + the get_graph≡stream_graph
   equivalence guard (§3).
4. Same-date tie divergence graphed-vs-ungraphed → pin fixture graph-state; runtime unaffected.

**Scope / milestone (flag for orchestrator):** best as a **NEW milestone (e.g. P66 "lazy topo
order")**, not a P65a re-open. Rationale: the IPC surface (commands/events/channel + `GraphChunk`
wire shape + `LaneWalker`) is **unchanged**, so committed P65a/P65b code stays as-is at the
boundary — only the internal `seeded_revwalk` helper is replaced and every graph fixture is
regenerated. Because fixture regen reaches into M2's territory (not just P65), it is larger than an
amendment. The deferred `stream_perf.rs` first-batch threshold gets set by this milestone.

---

## Flagged ambiguities (for the orchestrator / user)

- **F1 — generation-number source.** Recommended `gix-commitgraph` (pure Rust). Needs a 1-day spike
  to confirm it reads git-written split/chain graphs + generation v2 and its dep footprint is
  acceptable; else own-parser or shell-out(a). *Decision needed before build.*
- **F2 — git-binary dependency.** Path (a) makes `git` a hard runtime requirement for the graph.
  Recommend NOT taking on that dependency (pick (c)); confirm with user.
- **F3 — accept the one-time fixture regeneration + a changed (but still "topo, then date") row
  order?** Required by any lazy approach; recommend yes, guarded by the equivalence test.
- **F4 — milestone numbering:** new P66 vs re-open P65a. Recommend new P66.
- **F5 — re-verify the newest git2 release** exposes no generation/commit-graph API before building
  (finding is for 0.21.0; unlikely to change but cheap to check).
