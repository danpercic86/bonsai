# P57 — Semantic commit-history search — USER CHECKPOINT checklist (native-only)

These items require the native Tauri window, a **real `claude` CLI**, a **real (largish) repo**, and
human judgement — they CANNOT be self-declared by the orchestrator. The AI gate only proves the
**algorithm + persistence** (Rust unit tests: BM25 ranking, tokenizer, per-commit doc extraction,
build / incremental / schema-invalidation / staleness / round-trip, retrieval, and the AI-synthesis
grounding shape) and the **mock-driven** UI wiring (browser harness with canned `historyHandlers`).
The native checkpoint is about **a real index build on real history, real on-disk persistence across
a restart, and a real-model answer grounded in real diffs** — not whether the panel exists.

> **Browser-harness limit (why this is native-only):** the "Ask history" overlay + build/retrieve/
> ask flow IS drivable in `pnpm dev` (`VITE_MOCK_IPC=1`) against `historyHandlers`, but (a) the mock
> fixtures carry **no diffs**, so its ranking is a naive token-overlap stand-in (UI plumbing only —
> the real BM25 lives in `bonsai-core::git::history_index`), and (b) in the orchestrator's automated
> environment the graph pane renders **headless (0×0)**, so the live index-build progress bar, real
> ranked results, match rings, and the real grounded answer must be perceived in the native window.

Run via `pnpm tauri dev` against a **REAL repo** (ideally a few thousand+ commits) with the real,
authenticated `claude` binary on PATH and **AI consent enabled** in Settings. Entry points
(contract §5.3): the command palette (`Ctrl/Cmd-K`) → **"Ask history…"** action, and the ✨ button on
the graph pane. Both open the overlay: an index-status line (Prepare / progress bar / "Indexed N
commits" + Rebuild), a question input, a **Search** button (pure retrieval — works with AI off) and
an **Ask AI** button (synthesis — gated), plus a ranked results list.

> This milestone is **read-only** — it NEVER writes into the repo. The index persists to the app
> data dir keyed by repo path (contract D4), not inside `.git/`. Nothing here mutates history, so no
> scratch-repo safety dance is needed; use any real repo you like.

## Already proved by the AI gate (do NOT re-verify manually)

- **BM25 retrieval core, unit-tested (pure — no git, no IO):** `tokenize` splits camelCase/
  snake_case into sub-tokens + keeps the whole identifier, lowercases, drops short tokens/stopwords;
  BM25 ranks the query-bearing doc **above** a longer noise doc and scores non-matching terms 0; the
  BM25+ idf is **non-negative** even for a term in every doc; the **message field-boost** (`MSG_BOOST=3`)
  prefers a message match over an equal diff-only match.
- **Per-commit document extraction, unit-tested:** a merge commit documents vs its **FIRST** parent;
  a file over `MAX_DOC_DIFF_BYTES` is truncated; a binary file is skipped; the root commit diffs vs
  the empty tree. (Tokens + metadata are stored — never raw diff text; contract D3.)
- **Build / persistence / staleness, unit-tested (git2 fixtures):** build-then-search finds the
  expected commit first; an **incremental** rebuild only documents NEW commits (`newCommits == 1`,
  existing docs are not re-extracted); a **schema bump** forces a full rebuild; `index_status` reports
  `stale` + a `newCommits` count after a new ref, and a fresh build clears it; the store **save/load
  round-trips** (atomic tmp+rename; deterministic BTree bytes); the repo key is stable and path-
  normalized (Windows case-fold).
- **AI synthesis, unit-tested (stubbed CLI):** the grounding payload contains `QUESTION:`,
  `RELEVANT COMMITS`, and the `MESSAGE:` / `CHANGES:` detail for the top hit, and returns a
  `HistoryAnswer` with `retrieved` populated; **no index / no relevant commits ⇒ `AiFailed` BEFORE any
  CLI spawn** (a panicking fake bin proves the CLI is never launched); `parse_cited` extracts referenced
  short-oids; the prompts are single-line. `topK == 0` resolves to `DEFAULT_TOP_K` (regression test).
- **Contract + gate, unit-tested:** every wire type serializes **camelCase** (incl. `None → null`);
  `HistoryQuery` deserializes `{ "text": "x" }` with a default `topK`; the C3 consent-gate triple is
  enforced in `ai_search_history_inner` (`ai_enabled && ai_consented`, else `AiUnavailable`).
- **Browser harness (`VITE_MOCK_IPC=1`, canned handlers):** open "Ask history…" → **Prepare** streams
  an `IndexProgress` bar (counting → extracting → writing → done) → status shows "Indexed N commits";
  **Search** returns ranked rows with a descending score bar; the **Ask AI** button is disabled until
  the index is built AND AI is eligible; `?historyFail` rejects the build with a `git` error banner;
  `?ai=off` rejects "Ask AI" with a clear "Claude Code CLI not found" banner; Esc peels the overlay
  (below the P50 search layer) before deselecting. **All against mock fixtures with no diffs — ranking
  and answer text are UI plumbing only.**

So below is strictly what a **real index build + a live model + a real repo** must confirm.

## A. Build the index on real history (progress · persistence · incremental)

- [ ] Open the palette (`Ctrl/Cmd-K`) → **"Ask history…"**; the overlay opens with a **"Prepare
      history search"** button (status = not built). The graph-pane ✨ button opens the same overlay.
- [ ] Click **Prepare** → the progress bar advances smoothly through the phases ("Counting commits…"
      → "Indexing commits N/total" → "Saving index…") and **finishes in an acceptable time** for the
      repo size (this is the one-time diff-extraction cost of contract D2; it is CPU-bound on the
      per-commit first-parent diff). Note the repo's commit count and the wall-clock build time.
- [ ] On completion the status line reads **"Indexed N commits"** with N matching the reachable
      history (all local + remote-tracking branches + tags + HEAD, contract §3.6), and the overlay is
      immediately usable for Search / Ask AI.
- [ ] **Persistence across a restart:** fully quit and relaunch `pnpm tauri dev`, reopen the same repo,
      open "Ask history…" → the status is **already "Indexed N commits"** (NOT "Prepare") with **no
      rebuild** — the on-disk store under the app data dir was reused. (Optional: confirm a
      `store.json` exists under `<app_data_dir>/history-index/<repo-hash>/` and that **nothing** was
      written inside the repo's `.git/`.)
- [ ] **Incremental top-up is fast:** make a few new commits in the repo (any real edits), then in
      the overlay the status shows **stale** with a **"· N new — Rebuild"** affordance; click
      **Rebuild** → it re-documents **only the new commits** (the progress "total" ≈ the number of new
      commits, NOT the whole history) and finishes far faster than the initial build. Afterwards the
      status is fresh again ("Indexed N+ commits", not stale).

## B. A grounded NL answer from real diffs (the headline flow — real model)

Pick two genuine questions about your repo's evolution, e.g. **"why did we move off library X"** or
**"when did the auth flow change"** — questions whose answer lives in commit *intent*, not a literal
keyword.

- [ ] With the index built and AI consent ON, type the question and click **Ask AI**. An answer
      renders in the shared **AI output panel** (titled after the question), written as prose (no
      markdown fences), explaining the **WHY / the evolution** — not a raw diff dump.
- [ ] The answer **cites specific commits by short hash** (e.g. `a1b2c3d`), and those hashes are
      **real commits in this repo** that genuinely bear on the question — spot-check 2–3 by revealing
      them in the graph and reading their message/diff. The answer must be grounded in those commits,
      not invented.
- [ ] **Retrieval beats literal search:** take a concept the answer surfaced and confirm that the P50
      literal search bar (`/`) for the obvious keyword would **miss** at least one of the relevant
      commits (because the commit phrases it differently) — i.e. the semantic retrieval added value
      over substring/pickaxe search. (This is the whole point of P57 vs P50; contract D6.)
- [ ] Ask a question the history genuinely does **not** answer → the model says so **plainly**
      ("the provided commits don't cover that") rather than confabulating a citation.

## C. Ranked results list + reveal-in-graph (real ranking)

- [ ] Click **Search** (pure retrieval, no AI needed) on a real query → the results list shows
      relevance-ranked rows (short-oid · summary · author · relative date · a **score bar** whose
      width tracks the BM25 score, widest at the top hit).
- [ ] The hit commits show **match rings** on the commit graph (reusing the P50 ring pass), and the
      top hit is revealed/scrolled into view automatically.
- [ ] **Clicking a hit row reveals that commit** in the graph (selection + scroll). Closing the
      overlay (Esc / ✕) clears the match rings.
- [ ] The ranking is sensible: the rows most on-topic for the query sit at the top; an empty query
      shows nothing; a query with no matches on a **built** index shows **"No relevant commits"** (not
      an error).

## D. Gating + privacy (real model)

- [ ] **Ask AI is gated:** with **no index built**, the "Ask AI" button is **disabled** (Search may
      still be offered but returns "build the index first" via the empty/rebuild-hint path). Build the
      index → "Ask AI" becomes enabled only when AI is ALSO eligible.
- [ ] **Consent OFF:** turn AI consent OFF in Settings → "Ask AI" is disabled / errors via the consent
      gate (`aiUnavailable`), and **nothing spawns the CLI**; pure **Search still works** (retrieval is
      not behind the AI gate, contract D7). Re-enable consent → Ask AI works again without a restart.
- [ ] **CLI missing:** remove/rename `claude` from PATH (consent ON) → **Ask AI** gives a clear
      "Claude Code CLI not found …" message, not a crash or silent no-op. (Index build + Search are
      git2-only and must still work with no `git`/`claude` binary present — contract D8.)
- [ ] **Local-only (no code leaves the device):** the retriever is **local BM25** over a local on-disk
      index, and the only egress is the local `claude` child process you already authenticated —
      Bonsai opens **no** network connection to any AI endpoint and sends **no** diff anywhere but into
      `claude`'s stdin. (Optional: confirm with a process/network monitor — identical to running
      `claude` yourself.)

## E. Notes / known decisions to observe (NOT blockers)

- [ ] **Retriever is BM25 v1 — embeddings deferred (FOR-USER decision).** v1 retrieval is lexical
      (Okapi BM25 over per-commit message+diff tokens with a message field-boost), NOT vector
      embeddings (contract D1 / OQ1). The "semantic" lift comes from the local `claude` synthesis
      reading the retrieved commits. A true local embedding backend (candle / fastembed+ONNX / local
      Ollama) can slot behind the **same index seam** later without touching query/synthesis/IPC/UI.
      This is flagged in **TODO.md's FOR-USER section** as the user's call — decide whether BM25 is
      sufficient for v1 or an embedding backend should be greenlit (and which runtime). If retrieval
      ever surfaces an obviously-irrelevant top hit for a clearly-worded query, note it here as tuning
      signal (field-boost value / BM25F are documented refinements, OQ4) — not a correctness bug.
- [ ] **Rewritten/GC'd history leaves harmless orphan docs.** After a force-push/rebase, old
      commit docs linger in the store (a commit's content never changes, so a doc is never *wrong*);
      "Rebuild from scratch" cleans them. Only note it if you see a stale commit surface as a hit.

## Sign-off
- [ ] A (real build: smooth progress + acceptable time; index PERSISTS across restart; incremental
      top-up re-documents only new commits and is fast)
- [ ] B (real-model answer grounded in real diffs, cites the right short-hashes, surfaces commits P50
      literal search would miss, and declines gracefully when history lacks the answer)
- [ ] C (ranked results with score bars + match rings; clicking a hit reveals it)
- [ ] D (Ask AI gated on built-index AND AI-eligible; consent-off + CLI-missing states clear; Search
      works with AI off; local BM25 + local claude only — no code leaves the device)
- [ ] E (known decisions observed: BM25 v1 / embeddings deferred — flagged FOR-USER in TODO; orphan
      docs after rewritten history are harmless)
