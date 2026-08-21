/** Which field(s) commit search examines (P50). `all` = message OR author. */
export type SearchField = 'all' | 'message' | 'author' | 'path' | 'content';
/** Which field actually matched a result row. */
export type MatchedField = 'message' | 'author' | 'path' | 'content';

/** A commit/content search request (P50a). Mirrors the Rust `SearchQuery`
 *  (camelCase) EXACTLY. `regex` applies to CONTENT only (v1): false = `-S`
 *  literal, true = `-G` regex; ignored for message/author/path. `caseSensitive`
 *  false ⇒ case-insensitive. `maxResults` 0 ⇒ backend default cap (1000),
 *  clamped to it. `scopeRef` null ⇒ all refs; a ref/oid ⇒ walk only that scope.
 *  (Date scope `since`/`until` is deferred — not part of the v1 wire type.) */
export interface SearchQuery {
  text: string;
  field: SearchField;
  regex: boolean;
  caseSensitive: boolean;
  maxResults: number;
  scopeRef: string | null;
}

/** One matched commit (P50a). Mirrors the Rust `SearchMatch` (camelCase)
 *  EXACTLY. `oid` is full 40-hex (feeds revealCommitByOid). `snippet` is the
 *  matched pathspec for Path mode, absent otherwise (serde skip when None). */
export interface SearchMatch {
  oid: string;
  summary: string;
  authorName: string;
  authorTs: number;
  matched: MatchedField;
  snippet?: string;
}

/** Commit-search response (P50a): capped, newest-first matches + a `truncated`
 *  flag when a cap or scan bound was hit ("there may be more"). */
export interface SearchResults {
  matches: SearchMatch[];
  truncated: boolean;
}

// ---- P57: semantic commit-history search (BM25 index) ----------------------

/** Build phase of `historyIndexBuild` (P57a). Mirrors the Rust `IndexPhase`
 *  (lowercase camelCase wire values). */
export type IndexPhase = 'counting' | 'extracting' | 'writing' | 'done';

/** One streamed build-progress tick (P57a). Mirrors the Rust `IndexProgress`
 *  (camelCase). `total`/`newCommits` are 0 until the counting phase completes;
 *  `processed` climbs during extraction. */
export interface IndexProgress {
  phase: IndexPhase;
  /** Commits documented so far THIS build. */
  processed: number;
  /** Commits to document THIS build (0 until counted). */
  total: number;
  /** Of `total`, how many were newly-added (incremental). */
  newCommits: number;
}

/** Cheap status of the persisted history index (P57a). Mirrors the Rust
 *  `IndexStatus` (camelCase). `built` is true iff an index file exists AND parsed
 *  at the current schema; `stale` means the current ref tips differ from the last
 *  build's; `newCommits` counts reachable commits not yet indexed (0 when fresh).
 *  `headOid`/`builtAt` are null before the first successful build. */
export interface IndexStatus {
  built: boolean;
  indexedCommits: number;
  headOid: string | null;
  stale: boolean;
  newCommits: number;
  schema: number;
  builtAt: number | null;
  /** Commits skipped as UNREADABLE (corrupt/missing objects) by the build that
   *  returned this status; always 0 from `historyIndexStatus` (only a build can
   *  skip — skipped oids are retried next build). Audit #2 §3.3. */
  skippedCommits: number;
}

/** Retrieval query for `historySearch` (P57b). Mirrors the Rust `HistoryQuery`
 *  (camelCase). `topK` 0 ⇒ the backend default (DEFAULT_TOP_K = 20), clamped to
 *  MAX_TOP_K = 50. */
export interface HistoryQuery {
  text: string;
  topK: number;
}

/** One relevance-ranked commit from `historySearch` (P57b). Mirrors the Rust
 *  `HistoryHit` (camelCase). Overlaps P50's `SearchMatch` so the results UI reuses
 *  `revealCommitByOid` + the graph match rings. `score` is BM25 relevance,
 *  descending. */
export interface HistoryHit {
  oid: string;
  summary: string;
  authorName: string;
  authorTs: number;
  score: number;
}

/** Ranked retrieval results (P57b). Mirrors the Rust `HistorySearchResults`
 *  (camelCase). `indexStale` is true when no usable index exists yet (UI offers
 *  Build). */
export interface HistorySearchResults {
  hits: HistoryHit[];
  indexStale: boolean;
  indexedCommits: number;
}

/** AI answer grounded in retrieved commits (P57c). Mirrors the Rust
 *  `HistoryAnswer` (camelCase). `text` is fence-stripped prose; `cited` are the
 *  short-oids the answer references (best-effort, for UI emphasis); `retrieved`
 *  is the commit set fed to the model (drives the results list + reveal). */
export interface HistoryAnswer {
  text: string;
  cited: string[];
  retrieved: HistoryHit[];
  costUsd: number | null;
}
