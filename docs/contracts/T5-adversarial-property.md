# T5 — Redundancy, Adversarial & Property-Based Pass (contract)

> Campaign: pre-release testing (plan: `~/.claude/plans/the-end-goal-is-misty-crayon.md`, Phase T5).
> Two commits: **T5a** (Rust: proptest suites + corrupt-repo + race/lifecycle) and **T5b**
> (frontend adversarial vitest). Design only — implementers follow this file verbatim.

## 0. Ground rules (repeat verbatim in every subagent prompt)

Identical to `docs/contracts/T2-rust-audit.md` §0 (TMP/TEMP → `D:\Temp`; scratch repos only under
`D:\Temp\bonsai-scratch` via `common::scratch_dir()`; never `cargo test` + `cargo clippy`
concurrently; subagents do not commit; frozen-files list — re-check the current "paused session"
list in `docs/testing-campaign-2026-08/FINDINGS.md` before touching anything; ~500-line file limit;
test-only work must not change app behavior). T5 additions:

- **Shrink artifacts are committed.** When proptest finds a failure it writes
  `crates/bonsai-core/proptest-regressions/<file>.txt` — commit that file with the fix so the
  minimal case replays forever. Do NOT gitignore `proptest-regressions/`.
- **Every property failure ⇒** (a) FINDINGS.md entry (`F-###` format), (b) a plain, deterministic
  `#[test]` reproducing the shrunk minimal case, placed in the SAME `tests/prop_*.rs` file next to
  the property, named `regression_f###_<slug>`.
- Case count comes from the env proptest already reads: local runs `PROPTEST_CASES=256`, CI keeps
  the in-file default. Set the in-file default to **64** via `proptest::prelude::ProptestConfig`
  so CI stays fast without env plumbing.

## 1. Dependency + shared strategy module (T5a, increment 1)

`crates/bonsai-core/Cargo.toml`:

```toml
[dev-dependencies]
proptest = "1"
```

proptest is a dev-dependency of **bonsai-core only** — never `src-tauri`, never bonsai-forge.

New shared module `crates/bonsai-core/tests/prop_common/mod.rs` (included by each `prop_*.rs` via
`#[path = "prop_common/mod.rs"] mod prop_common;` — integration tests don't share a crate root):

```rust
/// Bounded random repo shape. Deliberately NOT FixtureSpec: proptest needs
/// per-commit randomness (messages, authors, tag/branch placement), while
/// FixtureSpec is a regular grid. Reuses fixture.rs's CommitFactory pattern
/// (in-memory blob+treebuilder commits, fixed base timestamp + counter*60).
#[derive(Debug, Clone)]
pub struct RepoShape {
    /// 1..=200 total commits (first commit is the root).
    pub commits: Vec<CommitSpec>,
    /// 0..=8 extra branch refs: (name, commit index it points at).
    pub branches: Vec<(String, usize)>,
    /// 0..=8 lightweight tags: (name, commit index).
    pub tags: Vec<(String, usize)>,
    /// Which commit HEAD points at (detached) or which branch (attached).
    pub head: HeadSpec,
}

#[derive(Debug, Clone)]
pub struct CommitSpec {
    /// Parent indices, each < own index (DAG by construction). Empty only
    /// for index 0. len 2 ⇒ merge. Duplicates allowed rarely (adversarial:
    /// duplicate-parent merge, see graph_adversarial.rs).
    pub parents: Vec<usize>,
    /// Message: printable unicode incl. non-ASCII, 0..=200 chars.
    pub message: String,
    /// Timestamp offset — strategies include NON-monotonic offsets (clock skew).
    pub ts_offset: i64,
}

#[derive(Debug, Clone)]
pub enum HeadSpec { Branch(usize), Detached(usize) }

/// proptest Strategy producing RepoShape. Bounds: ≤200 commits, ≤8 branches,
/// ≤8 tags, merge probability ~20%, duplicate-parent probability ~2%.
pub fn repo_shape() -> impl Strategy<Value = RepoShape>;

/// Materialize under common::scratch_dir(): git2::Repository::init, replay
/// CommitSpecs via a CommitFactory-style builder, create refs, set HEAD.
/// Returns (tempdir, repo path). Panics only on infra errors.
pub fn build_repo(shape: &RepoShape) -> (tempfile::TempDir, std::path::PathBuf);

/// Two related text blobs: base = 0..=60 random lines; edited = base with
/// 0..=15 random ops (insert/delete/replace line; intra-line char edits;
/// unicode incl. multibyte + astral chars). Also generates the degenerate
/// pairs: (empty, x), (x, x), (x, empty).
pub fn diff_pair() -> impl Strategy<Value = (String, String)>;

/// Search-token strategies: word tokens `[a-z]{2,12}`, unicode words, mixed
/// case, punctuation-embedded tokens, plus a guaranteed-nonsense token
/// generator ("zq"-prefixed random suffix, asserted absent from the corpus).
pub fn query_token() -> impl Strategy<Value = String>;
```

Branch/tag name strategy: valid git ref chars only (`[a-zA-Z0-9._/-]`, no leading `-`/`.`,
no `..`, no trailing `.lock`) — invalid ref names are T5.3's job, not the generator's.

## 2. Property suites (T5a, increments 2–3; one file each, ~64 cases default)

All files in `crates/bonsai-core/tests/`. Each begins with
`proptest! { #![proptest_config(ProptestConfig { cases: 64, ..Default::default() })] ... }`.

### 2.1 `prop_graph_layout.rs` — `compute_graph` invariants

For every `RepoShape` (built repo → `graph::compute_graph(&path)`), assert:

1. **Node bijection**: `layout.nodes.len() == shape.commits.len()`; every generated oid appears
   exactly once (collect ids into a set, compare against `git2` revwalk of all refs).
2. **Topological order**: for every node `n` and parent index `p in n.parents`: `p > row(n)`
   (child strictly above parent — documented guarantee on `GraphNode.parents`).
3. **Parent truth**: `nodes[p].id` for each listed parent equals the real git parent oid of
   `n.id` (order-preserving: `parents[0]` = first parent). Duplicate-parent merges: deduped or
   duplicated consistently — pin whichever `graph_adversarial.rs::duplicate_parent_merge_is_sane`
   already asserts; do not re-decide.
4. **Lanes dense + bounded**: every `node.lane < layout.lane_count` and every `edge.lane <
   lane_count`; lane set `{0..lane_count}` has no unused lane below the max ever used
   (density: `lane_count == max(used)+1`).
5. **Edges well-formed**: for each edge, `from < to < nodes.len()`; edges sorted ascending by
   `(from, to)` (wire order contract); every `(row(n), p)` parent pair has ≥1 corresponding edge
   and no edge exists without a matching parent link.
6. **head_index**: `Some(i)` where `nodes[i].id` == HEAD oid; detached HEAD ⇒ exactly one
   `RefKind::Head` label with `is_head`.
7. **Determinism**: call `compute_graph` twice; layouts are `==` (derive `PartialEq` exists).
8. **Scroll/color stability under prefix re-layout**: add one commit on top of HEAD (new child),
   recompute. For every OLD commit id, its `lane` in the new layout is unchanged **unless** the
   new tip structurally forced a shift. Lane assignment is deterministic-from-tips, so the honest
   invariant to pin is: *two runs over the identical repo give identical lanes* (item 7), plus
   *appending a commit to the current HEAD branch does not change the lane of any commit that was
   previously in a lane other than HEAD's lane*. If the current algorithm violates the second
   clause, that is a FINDINGS entry + orchestrator decision, not a silent test weakening. (Lane
   colors are derived frontend-side purely from lane index — lane stability IS color stability.)

### 2.2 `prop_intraline.rs` — `annotate_hunk` / `token_diff` spans

`annotate_hunk` is `pub(crate)`; test through the public diff path OR add
`#[cfg(test)] pub` re-export — **recommendation: add a tiny `#[doc(hidden)] pub fn
annotate_hunk_for_tests(hunk: &mut Hunk)` wrapper in `intraline.rs`** (1-line, no behavior
change; log as a test-seam note, not a finding). For every `(a, b)` from `diff_pair()`, build a
synthetic `Hunk` with one del-line `a` + one add-line `b`, annotate, then per side:

1. Spans `[start, len]` satisfy `len > 0`, ascending `start`, **non-overlapping AND non-adjacent**
   (adjacent runs must have been coalesced by `merge_adjacent`).
2. `start + len <= line.chars().count()` (code-point bounds — test astral chars specifically).
3. `a == b` ⇒ both span lists empty. Lines > `MAX_INTRALINE_CHARS` ⇒ empty spans.
4. **Symmetry sanity**: annotating the swapped pair `(b, a)` yields exactly swapped span lists
   (LCS marks are text-equality based; if the tie-break makes this fail, weaken to: the SET of
   changed characters per side is swapped — pin the actual behavior and document which).
5. Context lines and unpaired surplus lines always keep `spans = []`.

Also mirror invariant 1–2 in a frontend twin (see §5.2, `segmentLine`).

### 2.3 `prop_history_index.rs` — BM25 round-trip

Build a corpus from `repo_shape()` messages (or directly a `BTreeMap<String, CommitDoc>` — cheaper;
**recommendation: skip the repo, generate `CommitDoc`s directly** and separately keep ONE
deterministic end-to-end `build_index`→`search_history` test over a small real repo):

1. **Round-trip**: inject a fresh nonsense token into exactly one doc's message → `rank` returns
   that doc first; a token absent from every doc returns zero hits.
2. **idf finite**: for arbitrary tokens (present or absent), `idf()` is finite and ≥ 0
   (`ln(1 + …)` form — assert no NaN/inf even for df=0, df=n).
3. **tf monotonicity**: for a controlled 2-doc corpus with equal `dl`, the doc with more
   occurrences of the query term scores strictly higher (`score` monotone in tf).
4. **rank contract**: results sorted score-desc, ties author_ts-desc; `len <= top_k`.

### 2.4 `prop_status.rs` — porcelain equivalence (gated `require_git!`)

Strategy: an initial committed tree (1..=10 files, nested dirs) + a sequence of 1..=12 mutations
drawn from: create file, modify, delete, rename (delete+create with same content), stage path
(git2 `index.add_path`), unstage path (`reset_default`), each with unicode-capable relative paths
(valid on Windows: no reserved names, no trailing dot/space — generator excludes those). Apply to
a scratch repo, then assert `read_status(&path)` maps 1:1 onto `common::porcelain_records(&path)`
(the existing oracle; reuse the mapping helper from the existing status twin-pair tests — grep
`assert_same_status` / status_cli tests for the established record comparison, do not invent a
new mapping). `require_git!` guard at the top; case count 32 here (each case shells out to git).

### 2.5 `prop_stash_roundtrip.rs` — stash create/apply identity (gated `require_git!` optional; pure git2 is fine)

Strategy: base commit + random dirty state — staged edits, unstaged edits, untracked files
(0..=6 files each, overlapping paths allowed: same file staged-then-modified). Procedure:

1. Snapshot: map of `path → (worktree bytes | absent)` for all files, plus index entries
   (`path → blob oid, stage`) via `repo.index()`.
2. `create_stash(&path, None, StashScope::AllWithUntracked)` (covers untracked; run a second
   configuration with `StashScope::All` and untracked files EXCLUDED from the identity check).
3. Assert worktree is clean post-stash (only tracked baseline remains).
4. `apply_stash(&path, 0, false)` → re-snapshot → **byte-identical worktree AND identical index
   entries (path/oid/stage)** vs step 1.

Skip generated states where `create_stash` legitimately refuses (e.g. nothing to stash) via
`prop_assume!`.

## 3. Corrupt-repo corpus — `tests/corrupt_repo_cli.rs` (T5a, increment 4)

Table-driven matrix. Helper `fn corrupted_repo(c: Corruption) -> (TempDir, PathBuf)` starts from a
healthy 3-commit repo (`common::init_repo` + `commit_fixed`) then tampers:

| ID | Tampering |
|----|-----------|
| C1 | Truncate a loose object file (`.git/objects/xx/yyyy…`) to half length |
| C2 | Overwrite first 8 bytes of a pack file with garbage (only if a pack exists — run `git gc` first under `require_git!`, else `#[ignore]` C2) |
| C3 | `HEAD` = `ref: refs/heads/does-not-exist` (dangling symref — legal git state: unborn-like) |
| C4 | `HEAD` = raw 40-hex oid of a missing object |
| C5 | `refs/heads/x` containing garbage bytes (not hex, not a symref) |
| C6 | `.git/objects/` directory removed entirely |
| C7 | `.git/index` truncated to 10 bytes |
| C8 | `.git/index` = 4 KiB of random bytes |
| C9 | `.git/config` with invalid syntax (`[unclosed` line) |
| C10 | `.git/COMMIT_EDITMSG` = binary garbage (should be a no-op for all ops — pin that) |

For EACH corruption, call the four surfaces **directly on bonsai-core fns** (not through Tauri):
`git2::Repository::open_ext`-based open path used by the app, `status::read_status`,
`graph::compute_graph`, and a commit attempt (`commit::create_commit` or the app's equivalent —
locate by grep, stage one file first where the index still works). Assert per call:

- returns `Ok(coherent degraded output)` **or** `Err(AppError::…)` — never panics
  (wrap in `std::panic::catch_unwind` around each call so one panic fails only its matrix cell
  with a clear label), never hangs (each cell body must complete; if hangs are a realistic risk
  for a cell, run it on a thread with a 30 s watchdog + `panic!` on timeout).
- **Pin the actual behavior per cell** in the assertion (e.g. C3 behaves as unborn HEAD → empty
  graph `Ok`; C1/C4 → `Err`). First run discovers; the committed test asserts the discovered,
  reviewed behavior. Any cell where the behavior is user-hostile (panic, corruption spread,
  lock left behind) ⇒ FINDINGS entry + senior-dev fix.

Also from the plan's T5 list, three extra cells in the same file: bogus `.git/rebase-merge/` dir
(garbage `msgnum`/`onto` files) and bogus `BISECT_LOG` — `opstate` read + status must not panic;
and a path with invalid UTF-8 bytes committed via git2 raw tree entry — `read_status` /
`compute_graph` must not panic (lossy or error, per T2 §1 rules).

## 4. Race / lifecycle — `tests/race_lifecycle_cli.rs` (T5a, increment 4)

1. **Watcher storm during commit**: start the debounced watcher on a scratch repo (reuse the
   existing watcher-test harness — grep `watcher` tests for the setup), spawn a thread writing
   200 files in a loop while the main thread runs `create_commit`. Assert: commit succeeds,
   watcher emits ≥1 debounced signal, no panic in either thread (join + propagate).
2. **Concurrent status + commit**: two threads on one repo — thread A loops `read_status` ×50,
   thread B performs stage+commit ×10. Assert: no panic; every B iteration either succeeds or
   returns a clean `AppError` mentioning the index lock (never a poisoned/corrupted repo — final
   `git fsck`-style check: `read_status` and `compute_graph` succeed after joins, and under
   `require_git!` a real `git fsck --no-dangling` exits 0).
3. **Repo deleted while open**: open repo / take `read_status` baseline → `std::fs::remove_dir_all`
   the whole repo dir (Windows: retry loop for transient sharing violations) → `read_status`,
   `compute_graph`, `create_commit` each return `Err(AppError)`, no panic.

## 5. Frontend adversarial (T5b — vitest, jsdom project from T1)

### 5.1 Persistence garbage (reference only)

Corrupt-localStorage robustness for `src/ipc/mock/persistence.ts` is **owned by T3.4** — do not
duplicate. T5b only ADDS cases T3.4 missed, if any (check the shipped `persistence` test file
first): deeply nested valid-JSON-wrong-shape, `"null"`, 1 MB string, prototype-pollution keys
(`"__proto__"`).

### 5.2 Malformed-DTO tolerance — `src/components/__tests__/adversarial-dto.test.tsx`

Target: the 5 most-rendered DTO consumers, fed hostile-but-typed data through the mock IPC or
direct props. Components must render an error/empty state — **never throw past `ErrorBoundary`,
never white-screen** (assert the boundary fallback OR a graceful empty render; assert
`console.error` spy captures nothing unexpected beyond React's boundary log).

| DTO | Hostile shapes | Consumer under test |
|-----|----------------|---------------------|
| `GraphLayout` | `lane: -1`; `lane: 1e9`; `parents: [99999]` (out of range); `edges` with `from > to`; `laneCount: 0` with nodes; 0-row layout with edges | graph container / extracted pure geometry modules (from T3.5) + `GraphCanvas` mount smoke |
| `StatusSnapshot` | duplicate paths; empty-string path; path of 5,000 chars; unknown status code string | `StatusPanel` |
| `Diff`/hunks | spans out of line bounds; overlapping spans; negative `[start,len]`; hunk with 0 lines | `DiffView` row rendering + `segmentLine` unit twin of §2.2 invariants |
| Commit details | 10,000-char summary; message with `\u0000` and RTL overrides; missing author | commit-details panel |
| Refs/branch list | 1,000 branches; name with `<script>` text (must render as text, XSS check); duplicate names | `Sidebar` list |

`segmentLine(content, spans)` gets a direct fuzz-ish loop (seeded PRNG, 200 iterations — vitest,
no fast-check dependency): random content + random possibly-invalid spans ⇒ output segments
concatenate exactly to `content`, never throw.

### 5.3 Rapid-fire actions — `src/components/__tests__/rapid-fire.test.tsx`

`user-event` double-click on commit submit (exactly one IPC `commit` call — mock spy), palette
open/close/type spam (no state desync, no unhandled rejection), stage/unstage same file 10× fast
(last-wins, spy call count sane).

## 6. File map

New (T5a): `crates/bonsai-core/tests/prop_common/mod.rs`, `tests/prop_graph_layout.rs`,
`tests/prop_intraline.rs`, `tests/prop_history_index.rs`, `tests/prop_status.rs`,
`tests/prop_stash_roundtrip.rs`, `tests/corrupt_repo_cli.rs`, `tests/race_lifecycle_cli.rs`,
`crates/bonsai-core/proptest-regressions/*` (as failures occur).
New (T5b): `src/components/__tests__/adversarial-dto.test.tsx`, `__tests__/rapid-fire.test.tsx`,
`src/utils/intralineSegments.adversarial.test.ts`.
Modified: `crates/bonsai-core/Cargo.toml` (proptest dev-dep),
`crates/bonsai-core/src/git/intraline.rs` (test-seam wrapper only, if §2.2 option taken),
`docs/testing-campaign-2026-08/{FINDINGS,COVERAGE}.md`.

## 7. Acceptance criteria

- `PROPTEST_CASES=256 cargo test -p bonsai-core --test prop_graph_layout --test prop_intraline
  --test prop_history_index --test prop_status --test prop_stash_roundtrip` green locally
  (prop_status at its own 32-case cap); default (64-case) run green in the normal
  `cargo test --workspace`.
- `corrupt_repo_cli` and `race_lifecycle_cli` green; **zero panics** anywhere (catch_unwind cells
  count a panic as failure, not success).
- Every corruption cell's pinned behavior reviewed (reviewer pass) — no cell "passes" by asserting
  nothing.
- vitest suite green incl. new adversarial files; no component throws past ErrorBoundary.
- Any failure found ⇒ FINDINGS.md entry + committed `proptest-regressions` file + deterministic
  regression test.
- clippy -D clean; no new dependency outside `proptest` (frontend: NO fast-check — seeded loops).

## 8. Flagged ambiguities (orchestrator to resolve)

1. **§2.1 item 8 (lane stability under append)** — the exact algorithmic guarantee isn't written
   down anywhere; if the current engine shifts lanes on append, decide: fix the engine (scroll
   stability is a product promise) vs. weaken the property to same-input determinism only.
2. **§2.2 seam** — approve the `#[doc(hidden)]` test wrapper for `annotate_hunk` (my
   recommendation) vs. testing only through the public diff pipeline (slower, more setup).
3. **§3 C2 (pack corruption)** — needs `git gc` (git CLI) to create a pack; accept the
   `require_git!` gate or drop the cell.
4. **§2.5** — if `apply_stash` intentionally does not restore staged-vs-unstaged split exactly
   (libgit2 semantics), the identity check relaxes to worktree-bytes identity + "all changes
   present"; pin whichever the current implementation does and document it.
