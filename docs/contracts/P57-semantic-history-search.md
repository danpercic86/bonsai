# P57 — Semantic commit-history search (NL questions answered from real diffs)

Ask a natural-language question about the repo's history and get a prose answer **grounded in real
commit diffs**, plus the ranked commits it was drawn from (each jump-to-graph). This is the
**highest-build-cost** Phase-2 milestone (a per-commit document index) and is sequenced LAST; it
obeys `docs/contracts/phase2-ai-native-overview.md` (C1 grounding, C2 `AiOutputPanel`, C3 `ai_*`
command shape + consent gate + mock parity, C4 local-first, C5 model-tier seam).

References read (verified, not guessed): `crates/bonsai-core/src/git/search.rs` (P50 literal/pickaxe
search + `seed_all_refs` ref-seeding + `GitRunner` idiom), `git/ai_summary.rs` (range walk +
`render_commit_list`/`render_headers` + `AiSummary` idiom the synthesis mirrors), `ai/mod.rs`
(`run_claude`, `RunOpts`, `DEFAULT_MODEL`, `strip_fence`), `ai/payload.rs` (`render_file_diffs`,
`render_headers`, `render_commit_list`, `CommitLine`, `MAX_PAYLOAD_*`), `graph.rs`
(`compute_graph`, `GraphNode`, all-refs walk), `commands/repo.rs::clone_repo` +
`commands/ai.rs` (the **channel** command shape and the AI consent-gate **triple**),
`settings.rs::settings_file` (`app.path().app_config_dir()` app-dir idiom + atomic persist),
`src/ipc/{types,tauri}.ts` + `src/ipc/mock/handlers/{repo,ai}.ts` (channel + AI mock parity),
`docs/contracts/{P50-search-command-palette,P53-ai-why-layer}.md`.

**Adds 4 Tauri commands** (`history_index_build` [channel], `history_index_status`, `history_search`,
`ai_search_history`). P57 is sequenced last, so the exact `generate_handler!` numbering depends on
which of P54/P55/P56 merged first — **verify the final count at integration** (do not hard-code it).
Open questions in §11.

---

## 0. Key decisions (with rationale)

**D1 — v1 retriever is LEXICAL (BM25 over per-commit documents), NOT vector embeddings.** This is the
central call (§11 OQ1). The user-facing "semantic" lift is done by the **already-local `claude`
synthesis** that reads the retrieved commits and answers the question; retrieval's only job is to put
the right candidate commits in front of the model. BM25 over commit message + diff text is a strong
first-stage retriever, is **pure Rust with zero new heavy dependency, zero model download, and runs
fully locally by construction** (privacy = C4; no ONNX/candle runtime; no AppData binary for Defender
ASR to block — MEMORY). A true embedding backend (candle/`fastembed`/local Ollama) can be added later
**behind the same index seam without touching the query/synthesis/IPC/UI layers** — this mirrors the
overview's C5 model-tier seam and is the user's call, not the architect's. **Recommendation: ship
BM25 v1; defer embeddings (flagged, tied to OD1).**

**D2 — The engineering cost is the PERSISTED per-commit document store, which is retriever-agnostic.**
The expensive part is extracting a diff per commit (the jank P50 avoided for pickaxe). We pay it once,
persist it, and maintain it incrementally. This same store feeds a future embedding backend unchanged
(swap "term-frequency vector" for "dense vector"; everything else is identical).

**D3 — The store holds TOKENS + metadata for retrieval; the AI synthesis re-fetches the REAL diff for
only the top-K commits at query time.** Guarantees the answer is grounded in the current real diff
(not a stale excerpt), keeps the store compact, and confines the per-commit diff walk to build time
(retrieval touches no git objects). "Grounded in real diffs" is satisfied literally.

**D4 — Persist under the app data dir, keyed by a hash of the repo path — NOT inside `.git/`.**
`<app_data_dir>/history-index/<fnv-hex(canonical workdir)>/`. Never mutates the user's repo (invariant:
Bonsai does not write into `.git` internals for a cache), centralizes "clear all indexes", and matches
where `settings.json` already lives. It is regenerable derived data (safe to delete). App-data (not
cache-dir) so an OS cache purge does not silently force a slow rebuild mid-session (§11 OQ2).

**D5 — Build is a CHANNEL command; retrieval + synthesis are plain request/response commands.** The
long diff-walk streams `IndexProgress` (invariant: channels = streaming/incremental; overview note
"long index builds likely want a CHANNEL"). Retrieval returns a bounded ranked list (one `invoke`,
like `search_commits`). Synthesis returns bounded prose (C3: AI features are commands, never channels)
— the channel here is the non-AI index build, so it does not violate C3.

**D6 — P57 COMPLEMENTS P50; it does not replace or touch `git/search.rs`.** P50 = deterministic
literal search (substring message/author, `-S`/`-G` pickaxe, pathspec) — "find commits *containing*
this string". P57 = relevance-ranked NL retrieval over precomputed commit documents + optional AI
answer — "find commits *about* this concept / answer this question". Different backends, different
commands; they **share only the results UI** (reveal-in-graph + `matchRows` rings from P50). No edits
to `search.rs`.

**D7 — Retrieval is pure IR and is NOT behind the AI consent gate; only `ai_search_history` is.**
`history_search`/`history_index_build`/`history_index_status` are local git+IR, useful even with AI
off (a "most relevant commits" mode). `ai_search_history` follows the C3 triple + `ai_enabled &&
ai_consented` gate verbatim.

**D8 — Index build uses pure git2 (no `git` CLI).** Diff extraction reuses `git/diff.rs` typed diffs;
unlike P50 path/content it never shells out, so it works even where the `git` binary is absent (§11 OQ8).

---

## 1. Module boundaries / files

**New (Rust core — split to honor the ~500-line limit; the index is a directory module):**
- `crates/bonsai-core/src/git/history_index/mod.rs` — public API + wire types + the
  `build_index` / `index_status` / `search_history` orchestration + `index_dir_for` path builder +
  consts. Re-exports the submodules' items.
- `crates/bonsai-core/src/git/history_index/doc.rs` — per-commit document extraction (`CommitDoc`,
  `extract_doc`) + `tokenize` (the shared tokenizer) + field-boost. Pure over a `git2::Repository`.
- `crates/bonsai-core/src/git/history_index/bm25.rs` — the inverted index + BM25 scoring
  (`Bm25Index`, `build_stats`, `score_query`). **Pure, no git, no IO — the load-bearing algorithm,
  unit-tested in isolation.**
- `crates/bonsai-core/src/git/history_index/store.rs` — on-disk persistence (`IndexStore`,
  `load`, `save` [atomic tmp+rename, mirroring `settings.rs`], `repo_key` [FNV-1a hex], schema
  invalidation).
- `crates/bonsai-core/src/git/ai_history.rs` — `answer_history` (AI synthesis; mirrors
  `ai_summary.rs`) + `HistoryAnswer` + prompt consts + `parse_cited`. Retrieves via
  `history_index::search_history`, re-fetches real diffs for the top-K, renders the grounding
  payload, calls `run_claude`.

**New (frontend):**
- `src/components/HistorySearchPanel.tsx` — the "Ask history" overlay: index-status line + build
  button (with a progress bar) + question input + Search/Ask actions + inline error. Presentational;
  state via the hook.
- `src/components/HistoryResultsList.tsx` — relevance-ranked hit rows (short-oid · summary · author ·
  rel-date · a score bar); click → reveal in graph. Own file (styled like `SearchResultsList`).
- `src/components/repoWorkspace/useHistorySearch.ts` — state hook (status, building+progress, query,
  hits, answer, reqId last-wins, `matchRows` derivation, reveal wiring). Mirrors `useCommitSearch`.
- `src/ipc/mock/handlers/history.ts` — mock `historyIndexBuild` / `historyIndexStatus` /
  `historySearch` / `aiSearchHistory`.

**Edited:**
- `crates/bonsai-core/src/git/mod.rs` — `pub mod history_index; pub mod ai_history;`.
- `src-tauri/src/commands/history.rs` — the 4 commands (+ `ai_search_history_inner`). (This file
  already hosts `blame`/`file_history`/`reflog`-style read commands — extend it; if it nears the
  ~500-line limit, split the P57 commands into `commands/history_search.rs` instead — flag.)
- `src-tauri/src/commands/shared.rs` — re-export the new core types; add `app_data_root(app)` helper
  (like `settings_file` but `app.path().app_data_dir()`).
- `src-tauri/src/lib.rs` — register the 4 commands in `generate_handler!` (count verified at
  integration, D-note above).
- `src/ipc/types.ts` — the 6 wire types + `IpcApi.{historyIndexBuild,historyIndexStatus,
  historySearch,aiSearchHistory}`.
- `src/ipc/tauri.ts` — the 4 invoke wrappers (build bridges a `Channel<IndexProgress>` exactly like
  `cloneRepo`).
- `src/ipc/mock.ts` — import + spread `historyHandlers`.
- `src/components/RepoWorkspace.tsx` — wire `useHistorySearch`; render `HistorySearchPanel`; thread
  `matchRows` into `GraphCanvas` (reuse P50's `matchRows` prop — no GraphCanvas change needed);
  render the AI answer in the existing `AiOutputPanel` (C2, reuse the `aiPanel` req-id state).
- `src/components/repoWorkspace/useWorkspaceKeyboard.ts` — add `historyOpenRef` to Esc-layering (just
  below the P50 search layer) and `historyOpen` to the shortcut gate.
- `src/components/paletteActions.ts` — add an "Ask history…" action (group `'action'`) that opens the
  panel; and reuse the palette's dynamic-row pattern.
- `styles.css` — `.history-search`, `.history-hit`, `.history-score-bar`, `.index-progress` classes.

---

## 2. Wire types

### 2.1 Rust — `git/history_index/mod.rs`

```rust
/// Persisted-index schema; bump on ANY format/tokenization change → forces a full rebuild (§3.4).
pub const HISTORY_INDEX_SCHEMA: u32 = 1;
/// Hard cap on commits indexed in one build. Align with `graph::MAX_COMMITS` (same 20k+ horizon).
pub const MAX_INDEX_COMMITS: usize = /* = graph::MAX_COMMITS */ 50_000;
/// Per-commit diff bytes read for TOKENIZATION at build time (never stored raw). Bounds the walk.
pub const MAX_DOC_DIFF_BYTES: usize = 4_096;
/// Default / hard-cap retrieval depth.
pub const DEFAULT_TOP_K: u32 = 20;
pub const MAX_TOP_K: u32 = 50;
/// Of the retrieved top-K, how many are grounded with a FULL real diff in the synthesis payload
/// (the rest ride as a commit list). Keeps the CLI payload bounded.
pub const SYNTH_DIFF_K: usize = 8;

/// Streamed build progress (one per Channel tick). Serialize camelCase.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IndexProgress {
    pub phase: IndexPhase,
    pub processed: u32, // commits documented so far THIS build
    pub total: u32,     // commits to document THIS build (0 until counted)
    pub new_commits: u32, // of `total`, how many were newly-added (incremental)
}

#[derive(Debug, Clone, Copy, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum IndexPhase { Counting, Extracting, Writing, Done }

/// Cheap status driving the UI affordance. Serialize camelCase.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IndexStatus {
    pub built: bool,              // an index file exists and parsed at the CURRENT schema
    pub indexed_commits: u32,     // docs in the store
    pub head_oid: Option<String>, // HEAD (40-hex) at last build
    pub stale: bool,              // current ref tips differ from the last build's tips
    pub new_commits: u32,         // reachable commits not yet in the store (0 when fresh)
    pub schema: u32,              // schema of the on-disk file (for a mismatch note)
    pub built_at: Option<i64>,    // unix secs of last build
}

/// Retrieval query (pure IR; NOT AI). Deserialize camelCase.
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryQuery {
    pub text: String,             // NL / keyword query; empty/whitespace ⇒ Ok(empty), no work.
    #[serde(default)] pub top_k: u32, // 0 ⇒ DEFAULT_TOP_K; clamped to MAX_TOP_K.
}

/// One relevance-ranked commit. Serialize camelCase. Overlaps `SearchMatch` (P50) so the results UI
/// reuses `revealCommitByOid` + `matchRows`.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryHit {
    pub oid: String,        // full 40-hex → revealCommitByOid
    pub summary: String,    // first message line, capped 120
    pub author_name: String,
    pub author_ts: i64,
    pub score: f32,         // BM25 relevance, descending
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistorySearchResults {
    pub hits: Vec<HistoryHit>, // relevance-desc; tie-break author_ts desc
    pub index_stale: bool,     // hint to offer a rebuild
    pub indexed_commits: u32,
}
```

### 2.2 Rust — `git/ai_history.rs`

```rust
/// AI answer grounded in retrieved commits. Serialize camelCase. Prose shape mirrors `AiAnalysis`,
/// plus citations + the retrieved set (for the UI list + reveal).
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryAnswer {
    pub text: String,           // fence-stripped prose answer
    pub cited: Vec<String>,     // short-oids the answer references (best-effort parse of 7-hex tokens
                                // that prefix a retrieved oid), for UI emphasis
    pub retrieved: Vec<HistoryHit>, // the commits fed to the model
    pub cost_usd: Option<f64>,
}
```

### 2.3 TypeScript (`src/ipc/types.ts`)

```ts
export type IndexPhase = 'counting' | 'extracting' | 'writing' | 'done';
export interface IndexProgress { phase: IndexPhase; processed: number; total: number; newCommits: number; }
export interface IndexStatus {
  built: boolean; indexedCommits: number; headOid: string | null;
  stale: boolean; newCommits: number; schema: number; builtAt: number | null;
}
export interface HistoryQuery { text: string; topK: number; } // 0 => backend default (DEFAULT_TOP_K)
export interface HistoryHit {
  oid: string; summary: string; authorName: string; authorTs: number; score: number;
}
export interface HistorySearchResults { hits: HistoryHit[]; indexStale: boolean; indexedCommits: number; }
export interface HistoryAnswer {
  text: string; cited: string[]; retrieved: HistoryHit[]; costUsd: number | null;
}
```

`IpcApi` gains (near `searchCommits`):
```ts
/** Build/refresh the per-commit semantic-search INDEX (BM25 over message+diff), streaming
 *  IndexProgress. Incremental: only commits absent from the store are (re)documented. Writes to the
 *  app data dir keyed by repo — NOT the repo; does NOT emit repo-changed. Rejects git | io | noRepo. */
historyIndexBuild(repoId: string, onProgress: (p: IndexProgress) => void): Promise<IndexStatus>;

/** Cheap status of the persisted index (built?, count, staleness vs current refs). Rejects noRepo. */
historyIndexStatus(repoId: string): Promise<IndexStatus>;

/** Relevance-ranked retrieval over the persisted index (pure IR; NOT AI-gated). Empty/whitespace
 *  `text` ⇒ { hits: [], ... }. If no index exists ⇒ { hits: [], indexStale: true, indexedCommits: 0 }
 *  (UI offers Build). Rejects io | noRepo. */
historySearch(repoId: string, query: HistoryQuery): Promise<HistorySearchResults>;

/** Retrieve top-K then synthesize an NL answer grounded in the REAL diffs of those commits via the
 *  local claude CLI. Read-only; WRITES NOTHING; does NOT emit repo-changed. AI-gated. Rejects
 *  aiUnavailable | aiFailed (no index / no relevant commits / CLI error) | git | noRepo. */
aiSearchHistory(repoId: string, question: string, topK: number): Promise<HistoryAnswer>;
```

`tauri.ts`:
```ts
historyIndexBuild(repoId, onProgress) {
  const ch = new Channel<IndexProgress>(); ch.onmessage = onProgress;
  return invoke('history_index_build', { repoId, onProgress: ch });
},
historyIndexStatus: (repoId) => invoke('history_index_status', { repoId }),
historySearch: (repoId, query) => invoke('history_search', { repoId, query }),
aiSearchHistory: (repoId, question, topK) => invoke('ai_search_history', { repoId, question, topK }),
```

---

## 3. Backend core

### 3.1 Public core signatures (`git/history_index/mod.rs`, `git/ai_history.rs`)

The command layer resolves the app-data base and passes it in; core stays runtime-free and
unit-testable with a `tempfile::TempDir` base (mirrors `settings.rs`'s path-parameterization).

```rust
/// Pure path builder: `base/history-index/<fnv1a_hex(canonical(workdir))>`. No IO.
pub fn index_dir_for(app_data_base: &Path, workdir: &Path) -> PathBuf;

/// Blocking, CPU-heavy (diff per new commit) ⇒ always under spawn_blocking. Loads any existing
/// store; if its schema != HISTORY_INDEX_SCHEMA, starts empty (invalidation). Walks all refs
/// (bounded at MAX_INDEX_COMMITS), documents every reachable oid absent from the store, rebuilds
/// BM25 corpus stats, stamps head/tips/built_at, and atomically writes. Streams progress.
pub fn build_index(
    workdir: &Path,
    index_dir: &Path,
    on_progress: impl FnMut(IndexProgress) + Send,
) -> Result<IndexStatus, AppError>;

/// Blocking, cheap. Reads the manifest (no doc load) + a header-only ref-tip scan to compute
/// `stale`/`new_commits`. Missing/unparsable/schema-mismatched ⇒ `built: false`.
pub fn index_status(workdir: &Path, index_dir: &Path) -> Result<IndexStatus, AppError>;

/// Blocking. Loads the store, tokenizes `query.text`, BM25-scores, returns the top-K hits
/// (relevance-desc, author_ts tie-break). Empty text or no store ⇒ Ok(empty). Touches NO git objects.
pub fn search_history(
    workdir: &Path,
    index_dir: &Path,
    query: &HistoryQuery,
) -> Result<HistorySearchResults, AppError>;

/// Blocking. Retrieves top-K, re-fetches the REAL first-parent diff for the top `SYNTH_DIFF_K`,
/// renders the grounding payload (§3.5), calls `run_claude`, parses citations. No index / no
/// relevant commits ⇒ `AiFailed(...)` BEFORE any CLI call (mirrors `summarize_range`).
pub fn answer_history(
    workdir: &Path,
    index_dir: &Path,
    question: &str,
    top_k: usize,
    opts: RunOpts,
) -> Result<HistoryAnswer, AppError>;
```

### 3.2 Per-commit document (`doc.rs`)

```rust
/// Compact, persisted per-commit document. Stores TOKEN FREQUENCIES + metadata only — never raw
/// diff text (D3). `dl` = total token count (BM25 length norm).
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct CommitDoc {
    pub summary: String,       // first message line, capped 120 (for the hit row)
    pub author_name: String,
    pub author_ts: i64,
    pub dl: u32,               // document length in tokens (post field-boost)
    pub tf: HashMap<String, u16>, // term → frequency (message terms already field-boosted, §3.3)
}

/// Extract a commit's document: full message + changed file paths + a bounded sample of added/
/// removed line text (first-parent diff via `git/diff.rs::collect_file_diffs`, capped at
/// MAX_DOC_DIFF_BYTES, binary files skipped). Merge commits diff to the FIRST parent (matches the
/// app's "commit vs first parent" rule). Root commit diffs vs the empty tree.
pub fn extract_doc(repo: &git2::Repository, oid: git2::Oid) -> Result<CommitDoc, AppError>;

/// Shared tokenizer: lowercase; split on non-alphanumeric; split camelCase / snake_case into
/// sub-tokens (kept ALSO as the whole identifier); drop tokens < 2 chars and a tiny stopword set.
/// Deterministic. Unit-tested.
pub fn tokenize(text: &str) -> Vec<String>;
```

Field boost: message tokens are counted with weight `MSG_BOOST = 3` (a message term contributes 3 to
`tf`/`dl`); diff/path tokens weight 1. Simple, effective; BM25F is a documented refinement (§11 OQ4).

### 3.3 BM25 index + scoring (`bm25.rs`) — the algorithm

Data shape (derived from the doc store at build time; persisted so retrieval never recomputes it):

```rust
pub struct Bm25Index {
    pub n: u32,                     // corpus size (docs)
    pub avgdl: f32,                 // mean document length
    pub df: HashMap<String, u32>,   // document frequency per term
    // Per-doc tf/dl live in the CommitDoc store; postings are derived on load (term → doc indices)
    // OR scored by a linear scan over docs when N ≤ MAX_INDEX_COMMITS (simpler; fast enough at 20–50k).
}
```

```
# Standard Okapi BM25 (k1 = 1.2, b = 0.75).
build_stats(docs):
    n     = len(docs)
    avgdl = mean(doc.dl for doc in docs)          # guard n==0 ⇒ avgdl = 1
    df    = { }
    for doc in docs:
        for term in doc.tf.keys(): df[term] += 1
    return Bm25Index { n, avgdl, df }

idf(term):
    # non-negative BM25+ idf (avoids negative weights for very common terms)
    return ln( 1 + (n - df[term] + 0.5) / (df[term] + 0.5) )

score(query_terms, doc):
    s = 0.0
    for t in unique(query_terms):
        f = doc.tf.get(t, 0);  if f == 0: continue
        s += idf(t) * ( f * (k1 + 1) ) / ( f + k1 * (1 - b + b * doc.dl / avgdl) )
    return s

search(query_text, docs, top_k):
    q = tokenize(query_text);  if q empty: return []
    scored = [ (oid, score(q, doc)) for (oid, doc) in docs if score > 0 ]
    sort scored by (score desc, author_ts desc)      # deterministic tie-break
    return scored[:top_k] → HistoryHit rows (summary/author/ts from the doc)
```

`build_stats` is O(total tokens) and cheap next to extraction; scoring is O(N · |query|) — fine at the
20–50k horizon. (Postings/BM25F/dense-vectors are the documented scale/quality refinements, §11.)

### 3.4 Persistence + incremental build (`store.rs`, `build_index`)

```rust
pub struct IndexStore {
    pub schema: u32,                       // == HISTORY_INDEX_SCHEMA or it's discarded
    pub head_oid: Option<String>,
    pub tip_oids: Vec<String>,             // sorted ref-tip oids at last build (staleness compare)
    pub built_at: Option<i64>,
    pub docs: BTreeMap<String, CommitDoc>, // full-oid hex → doc (BTree ⇒ deterministic serialization)
    pub bm25: Bm25Index,                   // rebuilt from `docs` on every save
}
```
v1 persistence = **serde JSON** (already a dependency), atomic write (tmp + rename, mirroring
`settings.rs`), one `store.json` under `index_dir`. A compact binary (bincode) + a term dictionary are
size/parse-speed refinements (§11 OQ2/OQ4); the JSON size is bounded by MAX_INDEX_COMMITS.

```
build_index(workdir, index_dir, on_progress):
    repo  = open(workdir)                                  # git2 open_ext NO_SEARCH (as everywhere)
    store = store::load(index_dir).filter(|s| s.schema == HISTORY_INDEX_SCHEMA)
            .unwrap_or_else(empty_store)                   # schema bump ⇒ full rebuild (§3.4 invalidation)
    reachable = reachable_oids(&repo, MAX_INDEX_COMMITS)   # header-only all-refs walk (see §3.6)
    todo = [ oid for oid in reachable if !store.docs.contains(hex(oid)) ]   # INCREMENTAL
    on_progress(Counting{ processed:0, total: todo.len, new: todo.len })
    for (i, oid) in todo.enumerate():
        store.docs.insert(hex(oid), doc::extract_doc(&repo, oid)?)          # the expensive diff walk
        if i % PROGRESS_TICK == 0:
            on_progress(Extracting{ processed:i as u32, total: todo.len, new: todo.len })
    store.bm25     = bm25::build_stats(&store.docs)
    store.head_oid = current HEAD hex (None if unborn)
    store.tip_oids = sorted current ref-tip hexes
    store.built_at = now_unix()
    on_progress(Writing{ processed: todo.len, total: todo.len, new: todo.len })
    store::save(index_dir, &store)                          # atomic
    on_progress(Done{ ... })
    return status_from(&store, &repo)                       # stale=false, new_commits=0 right after build
```
**Invalidation / staleness:** schema mismatch ⇒ discard + full rebuild. `index_status` marks `stale`
when current sorted ref-tips ≠ `store.tip_oids`, and estimates `new_commits` (reachable oids not in
`docs`, header-only, bounded). Rewritten/GC'd history (force-push, rebase) leaves orphan docs; they are
harmless (a commit's content never changes so a doc is never *wrong*), and a "Rebuild from scratch"
affordance (deletes `index_dir` first) cleans them — reachable-set filtering at query time is a
documented refinement (§11 OQ7). A commit doc is immutable ⇒ incremental never re-extracts existing.

### 3.5 Synthesis grounding payload (`answer_history`) — normative

Follows C1 (WHY-not-WHAT; labeled uppercase sections; multi-line ⇒ stdin only; `cap_review_payload`):

```
QUESTION:
<question>

RELEVANT COMMITS (most relevant first):
<render_commit_list of ALL retrieved hits>        # short-oid · summary · author

===== TOP MATCHES IN DETAIL =====
for each of the top SYNTH_DIFF_K hits:
  COMMIT <short7>  <summary>
  AUTHOR <name>  <YYYY-MM-DD>
  MESSAGE:
  <full commit message>
  CHANGES:
  <render_file_diffs of that commit's first-parent diff>   # capped by MAX_PAYLOAD_* + overall cap
```
System prompt (single-line; `prompts_are_single_line` test): *"You are answering a developer's question
about a git repository's history, using ONLY the commits provided on standard input. Explain the WHY
— the intent and evolution — and cite the specific commits by their short hash (e.g. a1b2c3d). If the
provided commits do not contain the answer, say so plainly rather than guessing. Be concise. Output
prose only — no markdown code fences."* Positional prompt: *"Answer the question on standard input from
the provided commits, citing commit hashes."* `parse_cited` = best-effort scan for 7-hex tokens that
prefix a `retrieved` oid.

### 3.6 Shared ref-seeding note

`reachable_oids` needs the SAME all-refs seeding as `search::seed_all_refs` and `graph::collect_refs`
(local + remote-tracking [skip `*/HEAD`] + tags-peeled + HEAD). Both existing copies are private.
**Recommendation:** promote one to a shared `git/refs.rs::seed_all_refs(repo, &mut walk)` and have
search.rs / graph.rs / history_index all call it (avoid a third copy). Flag as OQ9 — if the orchestrator
prefers no cross-module churn now, history_index carries a fourth small copy with a TODO.

---

## 4. Commands — `src-tauri/src/commands/history.rs`

`app_data_root(app)` (new in `shared.rs`) resolves `app.path().app_data_dir()`. The command computes
`index_dir` inside `spawn_blocking` via `history_index::index_dir_for(&base, &workdir)` so the core
owns the layout and the inner stays runtime-free.

```rust
// Channel build (mirrors clone_repo). NOT AI-gated. No repo-changed emit (writes app-data, not repo).
#[tauri::command]
pub async fn history_index_build(
    app: tauri::AppHandle, state: tauri::State<'_, AppState>,
    repo_id: String, on_progress: tauri::ipc::Channel<IndexProgress>,
) -> Result<IndexStatus, AppError> {
    let base = app_data_root(&app)?;
    let workdir = repo_path(state.inner(), &repo_id)?;
    tauri::async_runtime::spawn_blocking(move || {
        let dir = history_index::index_dir_for(&base, &workdir);
        history_index::build_index(&workdir, &dir, move |p| { let _ = on_progress.send(p); })
    }).await.map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

// history_index_status / history_search: same shape WITHOUT the channel (repo_path → spawn_blocking →
// history_index::index_status / ::search_history). Read-only; NOT AI-gated.

// AI synthesis: the C3 consent-gate TRIPLE (gate enforced in _inner BEFORE repo_path).
#[tauri::command]
pub async fn ai_search_history(
    app: tauri::AppHandle, state: tauri::State<'_, AppState>,
    repo_id: String, question: String, top_k: u32,
) -> Result<HistoryAnswer, AppError> {
    let file = settings::settings_file(&app)?;
    let base = app_data_root(&app)?;
    ai_search_history_inner(state.inner(), &file, &base, &repo_id, question, top_k).await
}
// _inner: load settings; REFUSE AiUnavailable unless ai_enabled && ai_consented; repo_path;
//         spawn_blocking(ai_history::answer_history(&workdir, &index_dir_for(base,workdir),
//                        &question, top_k.max(1).min(MAX_TOP_K) as usize, RunOpts::default())).
```

Register all 4 in `lib.rs` `generate_handler!`; re-export the core types in `commands/shared.rs`.

---

## 5. Frontend

### 5.1 `useHistorySearch.ts` (state hook — keeps RepoWorkspace lean)

```ts
export function useHistorySearch(deps: {
  repoId: string;
  graphDataRef: { current: GraphLayout | null };     // oid → row for matchRows (reuse P50)
  revealCommitByOid(oid: string): void;
  aiEligible: boolean;                                 // gate the "Ask" action
  runAiAnswer(question: string, topK: number): void;   // routes into the shared AiOutputPanel state
  pushToast(kind: ToastKind, msg: string): void;
}): {
  open: boolean; openPanel(): void; close(): void; openRef: { current: boolean };
  status: IndexStatus | null; refreshStatus(): void;
  building: boolean; progress: IndexProgress | null; build(): void;   // drives historyIndexBuild
  query: HistoryQuery; setText(t: string): void;
  hits: HistoryHit[]; searching: boolean; error: string | null; search(): void;  // historySearch (submit-only)
  matchRows: number[];                                 // hit oids present in the layout → GraphCanvas rings
};
```
Behavior: `reqId` last-wins guard (mirror `useCommitSearch`/`useReadOverlays`). On open →
`refreshStatus`. `build()` calls `ipc.historyIndexBuild(repoId, p => setProgress(p))`, then
`refreshStatus`. **Retrieval is submit-only** (Enter / Search button — never per-keystroke, it reads a
persisted index but still an `invoke`). On results, derive `matchRows` from `graphDataRef` and reveal
the top hit. The **AI answer renders in the existing `AiOutputPanel`** via the RepoWorkspace `aiPanel`
req-id state (C2) — `runAiAnswer` mirrors `runAnalyze` (title e.g. `History: "<question>"`).

### 5.2 `HistorySearchPanel.tsx` + `HistoryResultsList.tsx`
- Panel (overlay on the graph pane): a status line —
  *not built* → "Prepare history search" button;
  *building* → a progress bar (`processed/total`, phase label);
  *built* → "Indexed N commits" + (if `stale`) "· N new — Rebuild";
  a question `<input>` (autofocus), a **Search** button (retrieval) and an **Ask AI** button (disabled
  unless `aiEligible && status.built`), plus inline error text.
- `HistoryResultsList`: `hits` rows (short-oid · summary · author · rel-date · a `score` bar);
  click → `revealCommitByOid`. Empty + built → "No relevant commits". Reuses `SearchResultsList`
  styling.

### 5.3 Entry points + keyboard
- Palette: "Ask history…" (group `'action'`) → `openPanel()`; a graph-pane button beside the P50
  search toggle.
- `useWorkspaceKeyboard`: add `historyOpenRef` to Esc-layering just below the P50 `searchOpenRef`
  layer; add `historyOpen` to the shortcut gate so nav keys are inert while the panel is open. No new
  global accelerator required (opens from the palette/button) — flag if the orchestrator wants one.

---

## 6. Mock (`src/ipc/mock/handlers/history.ts`)

`historyHandlers satisfies Partial<IpcApi>`; `requireRepo(repoId)`; a module-level `mockBuilt` flag so
the build→status→search flow is exercised. Resolve the layout exactly as `mock/handlers/diff.ts::
getGraph` (reuse the P50 `resolveLayout(state)` helper if it exists, else the same resolution).
- `historyIndexBuild`: honor a `#fail` sentinel in… (no text arg) — instead a `?historyFail` URL seam →
  throw `{ kind:'git', message:'Mock: index build failed' }`. Else tick `IndexProgress`:
  `Counting`, then an `Extracting` loop (`await delay(60)` × ~12, `processed` climbing), then
  `Writing`, then `Done`; set `mockBuilt = true`; return
  `{ built:true, indexedCommits: layout.nodes.length, headOid: layout.nodes[0]?.id ?? null,
     stale:false, newCommits:0, schema:1, builtAt: nowSecs() }`.
- `historyIndexStatus`: return `{ built: mockBuilt, indexedCommits: mockBuilt ? layout.nodes.length : 0,
     ... , stale:false, newCommits:0, schema:1, builtAt: mockBuilt ? nowSecs() : null }`.
- `historySearch`: `await delay(120)`; if `!mockBuilt` ⇒ `{ hits:[], indexStale:true, indexedCommits:0 }`.
  Else rank `layout.nodes` by naive token-overlap of `query.text` against `summary` (+ author), map hits
  → `HistoryHit` with a fake descending `score`, slice to `topK`. **Document: fixtures carry no diffs, so
  this is UI-plumbing only** (same caveat as P50's mock).
- `aiSearchHistory`: honor `AI_OFF` (`?ai=off`) → throw `{ kind:'aiFailed', message:'Claude Code CLI not
  found on PATH' }`. Else `await delay(700)`; retrieve as above; return
  `{ text: 'Based on the retrieved commits, … (mock answer).', cited: hits.slice(0,2).map(h=>h.oid.slice(0,7)),
     retrieved: hits, costUsd: 0.01 }`.
Import + spread `historyHandlers` in `mock.ts`.

---

## 7. Test plan (`#[cfg(test)]`)

Reuse the AI/search idioms: `init_scratch`/`mk_commit` with pinned identity + `core.autocrlf=false` +
strictly-increasing `git2::Time`; `claude_stub` via `CLAUDE_BIN_ENV`; `have_git()` guards on any CLI
compare; **Windows test-runner sets `TMP`/`TEMP=D:\Temp`** (MEMORY rule). Persist to a `tempfile::TempDir`
`index_dir`.

**bm25.rs (pure — no git, no IO — the load-bearing tests):**
1. `tokenize_splits_identifiers`: camelCase/snake_case → sub-tokens + whole; lowercasing; stopwords/short
   tokens dropped.
2. `bm25_ranks_relevant_above_noise`: a hand-built 5-doc corpus where the doc containing the query terms
   (esp. in the boosted message field) outranks a longer noise doc; assert the ordered oid list + that
   score is 0 for non-matching terms.
3. `bm25_idf_non_negative` for a term present in every doc.
4. `field_boost_prefers_message_match` over an equal diff-only match.

**doc.rs:**
5. `extract_doc_first_parent_and_bounds`: a merge commit documents vs its FIRST parent; a >MAX_DOC_DIFF_BYTES
   file is truncated; a binary file is skipped; the root commit diffs vs empty.

**store.rs / build_index (git2 fixture):**
6. `build_then_search_finds_expected` (end-to-end, no CLI): build over a 4-commit fixture; `search_history`
   for a term unique to one commit returns that oid first.
7. `incremental_build_only_documents_new`: build; add 1 commit; rebuild → `IndexProgress.new_commits == 1`
   and existing docs are NOT re-extracted (assert via a doc-count delta / an extract counter).
8. `schema_bump_forces_full_rebuild`: write a store with `schema = HISTORY_INDEX_SCHEMA - 1`; build
   re-documents everything.
9. `status_reports_stale_after_new_ref`: build; create a branch/commit; `index_status.stale == true`,
   `new_commits >= 1`; a fresh build ⇒ `stale == false`.
10. `save_load_round_trip` (atomic tmp+rename; `store.json` present; BTree ⇒ deterministic bytes).
11. `repo_key_is_stable_and_path_normalized` (same workdir ⇒ same dir; case-fold on Windows).

**ai_history.rs:**
12. `answer_history_grounding_shape` (stub `success`): payload contains `QUESTION:`, `RELEVANT COMMITS`,
    the `MESSAGE:`/`CHANGES:` detail for the top hit; returns `HistoryAnswer` with `retrieved` populated.
13. `answer_history_no_index_fails_before_cli`: no store ⇒ `AiFailed`, CLI never spawned (panicking fake bin).
14. `parse_cited_extracts_referenced_short_oids`.
15. `prompts_are_single_line`.

**Wire shapes:** `*_wire_shape_is_camel_case` for `IndexProgress`/`IndexStatus`/`HistoryHit`/
`HistorySearchResults`/`HistoryAnswer` (incl. `None`→`null`); `HistoryQuery` deserializes `{ "text": "x" }`
(defaults `topK` 0).

---

## 8. Sub-increments (staged by cost — highest last)

### P57a — Index builder + persistence + status + progress channel (backend + IPC + mock)
Scope: `history_index/{mod,doc,bm25,store}.rs` (types, `extract_doc`, `tokenize`, BM25 stats/scoring,
persistence, `build_index`, `index_status`, `index_dir_for`) + tests §7.1–§7.11; `git/mod.rs`;
`commands/history.rs` `history_index_build` (channel) + `history_index_status`; `shared.rs`
`app_data_root` + re-exports; `lib.rs` registration; `types.ts`/`tauri.ts` (`historyIndexBuild` bridges a
Channel, `historyIndexStatus`); `mock/handlers/history.ts` (build + status) + `mock.ts` spread.
**Acceptance:** (1) `cargo test -p bonsai-core history_index` green incl. BM25 ranking, incremental,
schema invalidation, staleness, round-trip; `cargo build` + `clippy -D warnings` clean; no file over
~500 lines. (2) `tsc`/`pnpm build` clean. (3) Harness: `await ipc.historyIndexStatus('r')` → `built:false`;
`await ipc.historyIndexBuild('r', p=>console.log(p))` streams `IndexProgress` ticks then resolves an
`IndexStatus{built:true}`; status then reports the count; `?historyFail` rejects `{kind:'git'}`.

### P57b — Retrieval command + wire type + mock (pure IR; not AI)
Scope: `history_index::search_history` + `HistoryQuery`/`HistoryHit`/`HistorySearchResults` (tests §7.6,
wire/deserialize); `commands/history.rs` `history_search`; `lib.rs`; `types.ts`/`tauri.ts` `historySearch`;
`mock/handlers/history.ts` `historySearch`.
**Acceptance:** (1) `cargo test` retrieval green (relevant commit ranks first on the fixture);
build/clippy/tsc/pnpm build clean. (2) Harness: after a mock build, `await ipc.historySearch('r',
{text:'feature', topK:0})` resolves ranked `hits` with descending `score`; empty text ⇒ empty; no index ⇒
`{hits:[], indexStale:true}`.

### P57c — AI synthesis + UI (answer grounded in real diffs)
Scope: `git/ai_history.rs` (`answer_history`, `HistoryAnswer`, prompts, `parse_cited`) + tests §7.12–§7.15;
`git/mod.rs`; `commands/history.rs` `ai_search_history` + `_inner` (consent-gate triple); `lib.rs`;
`shared.rs` re-export; `types.ts`/`tauri.ts` `aiSearchHistory`; `mock/handlers/history.ts`
`aiSearchHistory` (`?ai=off` path); `useHistorySearch.ts`, `HistorySearchPanel.tsx`,
`HistoryResultsList.tsx`; `RepoWorkspace` wiring (panel + AiOutputPanel reuse + matchRows);
`useWorkspaceKeyboard` (historyOpen Esc-layer + gate); `paletteActions` "Ask history…"; `styles.css`.
**Acceptance:** (1) `cargo test -p bonsai-core ai_history` green (grounding shape, no-index-fails-before-CLI,
citation parse, single-line prompts); consent gate enforced in `_inner`; build/clippy clean; command count
matches integration. (2) `tsc`/`pnpm build` clean; no file over ~500 lines. (3) Harness (`VITE_MOCK_IPC=1`):
open "Ask history…" → build → ask a question → answer renders in `AiOutputPanel` with cited short-oids;
`HistoryResultsList` shows ranked hits with match rings on the graph; clicking a hit reveals it;
`?ai=off` → error banner; Esc closes the answer panel, then the history panel, before deselecting.

(P57a → P57b → P57c are strictly ordered: b needs a's store, c needs b's retrieval.)

---

## 9. Acceptance criteria (milestone)

- **AI gate:** all P57a/b/c acceptance above; whole-crate `cargo test` green; the browser harness proves
  the full build → status → retrieve → ask flow against `historyHandlers` (incl. `?historyFail` and
  `?ai=off` error paths); the consent gate is enforced in `ai_search_history_inner`; BM25 retrieval
  ranks the semantically-right commit above noise on the Rust fixture; the synthesis payload carries
  real diffs (`CHANGES:` sections) for the top matches (WHY-not-WHAT, C1). Index persists to the app data
  dir (never inside `.git`), is incremental, and invalidates on a schema bump.
- **USER CHECKPOINT** (`docs/contracts/P57-user-checklist.md`) — with a REAL repo + a real `claude` CLI:
  build the index on a real (largish) history — progress is smooth and finishes in acceptable time; ask a
  genuine NL question ("why did we move off library X", "when did the auth flow change") and confirm the
  answer is grounded in real commits and cites the right ones; confirm retrieval surfaces relevant commits
  that P50's literal search would miss; confirm the index persists across an app restart and an incremental
  top-up after new commits is fast; confirm no code leaves the device (local BM25 + local `claude`).

---

## 10. Reuse map

`git/diff.rs` (`collect_file_diffs`/`commit_diff` for extraction + synthesis) · `ai/payload.rs`
(`render_commit_list`/`render_headers`/`render_file_diffs`, `MAX_PAYLOAD_*`) + `cap_review_payload` ·
`ai/mod.rs` (`run_claude`, `RunOpts`, `strip_fence`) · `search::seed_all_refs`/`graph::collect_refs`
ref-seeding (promote to shared, §3.6) · `settings.rs` atomic-write + app-dir idiom · P50 UI
(`revealCommitByOid`, `matchRows` GraphCanvas rings, `SearchResultsList` styling, palette action pattern,
Esc-layering) · `AiOutputPanel` (C2) · `clone_repo` channel idiom (Rust + `tauri.ts` + mock).

---

## 11. Open questions (flag to orchestrator)

- **OQ1 — Retriever backend (THE decision; ties to overview OD1 / C5).** Recommend **BM25 v1**; defer true
  local embeddings (candle `all-MiniLM`, `fastembed`+ONNX, or a local Ollama `nomic-embed-text`) behind the
  same index seam. Rationale: embeddings add a heavy native runtime + a model download to AppData (Defender
  ASR risk — MEMORY), cross-platform build cost, and privacy surface, for a lift the local `claude`
  synthesis already provides. **This is the user's call** — confirm BM25-only for v1, or greenlight an
  embedding backend (and which runtime).
- **OQ2 — Persistence location & format.** Recommend `app_data_dir/history-index/<repo-hash>/store.json`,
  serde JSON, atomic write. Alternatives: `.git/`-adjacent (rejected — touches the user's repo),
  `app_cache_dir` (semantically "derived" but OS-purgeable → surprise rebuilds), bincode+dictionary (size/
  speed, defer). Confirm.
- **OQ3 — Missing-index behavior of `ai_search_history`.** Recommend it returns `AiFailed("history index not
  built — build it first")` before any CLI call, and the UI gates the "Ask" button on `status.built` so this
  is a rare race (NO new `AppError` variant). Alt: add a typed `indexNotReady` kind for a nicer toast
  (enum + TS churn). Confirm reuse-`aiFailed`.
- **OQ4 — Ranking quality knobs.** v1 = plain BM25 + a message field-boost (`MSG_BOOST=3`), linear-scan
  scoring. Recommend shipping as-is; flag BM25F (true per-field norms), postings lists (scale), and dense
  re-rank as refinements. Confirm the field-boost value or ask for tuning.
- **OQ5 — Expose `history_search` (pure retrieval) as its own command?** Recommend YES (independently
  testable, works with AI off, complements P50 as a relevance mode). Alt: fold retrieval into
  `ai_search_history` only (smaller surface, loses the AI-off mode). Confirm.
- **OQ6 — UI home.** Recommend a dedicated `HistorySearchPanel` (own files, ~500-line discipline; heavier
  build/status/answer flow than P50's live bar). Alt: add a "Semantic/Ask" mode to P50's `CommitSearchBar`
  (bloats it). Confirm.
- **OQ7 — Rewritten/GC'd history.** Recommend keep immutable oid-keyed docs + a "Rebuild from scratch"
  affordance; defer query-time reachable-set filtering (extra header-walk per query). Confirm.
- **OQ8 — git2-only extraction (no `git` CLI).** Recommend yes (works without the git binary; consistent with
  graph/diff). Confirm.
- **OQ9 — Shared ref-seeding helper.** Recommend promoting `seed_all_refs` to `git/refs.rs` shared by
  search/graph/history (avoid a fourth copy). Confirm, or accept a small local copy with a TODO for now.
- **OQ10 — Store load per query vs cached in `AppState`.** Recommend load-per-query for v1 (stateless core,
  simplest; fine at ~20–50k). If profiling flags it at large sizes, add an `AppState` LRU keyed by
  repo-key+`built_at`. Confirm defer.
```