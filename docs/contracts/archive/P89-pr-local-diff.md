# P89 — PR files & local diff view — Contract

Show a PR's changed-files list + per-file diffs, with **locally-computed** `+/−/files`
counts (base…head three-dot). Forge API counts are kept only as a fallback. Auto-fetch the
PR's base+head into the local repo so fork/un-fetched PRs work.

Invariants: Rust owns all git + diff math; IPC carries precomputed compact data; git2 in
`spawn_blocking`; per-file hunks fetched on demand (never all at once); every command mocked.

---

## 1. Module boundaries / responsibilities

| File | Responsibility (P89 delta) |
|---|---|
| `crates/bonsai-forge/src/types.rs` | + `PrRefs`, re-use `bonsai_core` `FetchTarget`. |
| `crates/bonsai-forge/src/provider.rs` | + `fn pr_refs(&self, number: u64) -> Result<PrRefs, AppError>`. |
| `crates/bonsai-forge/src/{github,gitlab,bitbucket,azure}/*` | impl `pr_refs`: map forge PR payload → `PrRefs` (table §3). DTO adds where noted. |
| `crates/bonsai-core/src/git/pr_diff.rs` **(new)** | `FetchTarget`, `PrEndpoints`, `PrDiffStats`; `fetch_pr_endpoints`, `pr_diff_headers`, `pr_file_diff`. Local fetch + merge-base + tree diff. Reuses `collect_headers`, `collect_file_diff`, `build_diff_options`, `apply_find_similar`, `maybe_annotate`, `acquire_cred`. |
| `crates/bonsai-core/src/git/mod.rs` | `pub mod pr_diff;` + re-exports. |
| `src-tauri/src/commands/forge.rs` | + `forge_pr_diff`, `forge_pr_file_diff` commands (+ `_inner`). |
| `src-tauri/src/commands/mod.rs` / `lib.rs` invoke_handler | register the two commands. |
| `src/ipc/types/forge.ts` | + `PrRefs?` (internal), `PrDiffStats`. |
| `src/ipc/types/ipc-api-forge.ts`, `src/ipc/tauri/forge.ts`, `src/ipc/mock/handlers/forge.ts`, `src/ipc/fixtures/forge.ts` | + `forgePrDiff`, `forgePrFileDiff` (real + mock + fixtures). |
| `src/components/PrDetailView.tsx` (+ small child files) | render local counts, changed-files list, states; open file diff in existing viewer. |
| Diff viewer reused: **`DiffView` / `DiffViewSplit`** (via `DiffOverlay` or inline, ui-designer decides). Reuses `FileDiff`/`Hunk` types unchanged. |

---

## 2. Rust interface contracts

### 2a. bonsai-core `git/pr_diff.rs`

```rust
/// A neutral fetch instruction: bring `resolve` (an oid or dest ref) local.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FetchTarget {
    /// Fork/head clone URL to fetch from; `None` = use the repo's origin remote.
    pub url: Option<String>,
    /// Refspec, e.g. "+refs/pull/42/head:refs/bonsai/pr/42/head". Empty when the
    /// oid is expected already-local (base on same repo, or cache hit).
    pub refspec: String,
    /// Revision to resolve to an oid AFTER the fetch — normally the commit SHA.
    pub resolve: String,
}

#[derive(Debug, Clone)]
pub struct PrEndpoints { pub base_oid: String, pub head_oid: String }

/// Result of the local base…head diff (three-dot). Wire type (camelCase).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrDiffStats {
    pub additions: u32,
    pub deletions: u32,
    pub changed_files: u32,
    /// merge-base(base,head) — the OLD side of the diff. "" if unrelated histories.
    pub merge_base_oid: String,
    pub base_oid: String,
    pub head_oid: String,
    /// Sorted path-ascending; headers only (hunks fetched per file).
    pub files: Vec<FileDiffHeader>,
}

/// Blocking. Fetch base+head endpoints. For each target: if `refspec` non-empty,
/// fetch it (anonymous remote when `url` is Some, else the origin remote) with the
/// shared `acquire_cred` ladder; then resolve `resolve` to an oid. OFFLINE FALLBACK:
/// if the fetch fails BUT `resolve` already resolves locally, use the cached oid
/// (log, no error). Fetch failure with no local oid → propagate (Network/AuthFailed).
pub fn fetch_pr_endpoints(
    workdir: &Path, base: &FetchTarget, head: &FetchTarget,
) -> Result<PrEndpoints, AppError>;

/// Blocking. merge_base(base,head); diff_tree_to_tree(merge_base_tree, head_tree)
/// (unrelated histories → empty tree as old side); find_similar; collect_headers
/// → counts + file list. Bad oid → AppError::Git.
pub fn pr_diff_headers(
    workdir: &Path, base_oid: &str, head_oid: &str,
) -> Result<PrDiffStats, AppError>;

/// Blocking. Hunks for ONE file of the merge_base…head diff. `merge_base_oid` ""
/// ⇒ empty tree old side. No matching delta → AppError::Git. Reuses build_diff_
/// options + collect_file_diff + maybe_annotate, mirroring `commit_file_diff`.
pub fn pr_file_diff(
    workdir: &Path, merge_base_oid: &str, head_oid: &str,
    path: &str, orig_path: Option<&str>, full_context: bool, intraline: bool,
) -> Result<FileDiff, AppError>;
```

### 2b. bonsai-forge

```rust
// types.rs
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrRefs {
    pub base_oid: String,   // target/base tip sha (from PR payload)
    pub head_oid: String,   // source/head tip sha (from PR payload)
    pub base_fetch: bonsai_core::git::pr_diff::FetchTarget,
    pub head_fetch: bonsai_core::git::pr_diff::FetchTarget,
}

// provider.rs — add to trait:
/// One network call: read the PR's base/head tips + fetch plan. Fork heads carry
/// the fork clone URL in `head_fetch.url`. Requires the PR-read scope only.
fn pr_refs(&self, number: u64) -> Result<PrRefs, AppError>;
```

`FileDiffHeader` already exists in `bonsai_core::git::diff` (wire type) and is reused verbatim.

---

## 3. Per-forge ref mapping

`resolve` is always the **oid** (all four forges return base/head SHAs in the PR payload),
so bonsai-core resolves by SHA regardless of ref-naming quirks. The refspec only needs to
make that SHA reachable locally.

| Forge | base_fetch | head_fetch (same-repo) | head_fetch (fork) | DTO add |
|---|---|---|---|---|
| **GitHub** | url=None, `+refs/heads/<base>:refs/bonsai/pr/<n>/base`, resolve=base_oid | url=None, `+refs/pull/<n>/head:refs/bonsai/pr/<n>/head` | same (`refs/pull/N/head` works for forks) | none (has head/base sha) |
| **GitLab** | url=None, `+refs/heads/<base>:…/base`, resolve=base_oid | url=None, `+refs/merge-requests/<n>/head:…/head` | same (MR head ref covers forks) | none (diff_refs has base/head sha) |
| **Azure DevOps** | url=None, `+<targetRefName>:…/base`, resolve=base_oid (=`lastMergeTargetCommit`) | url=None, `+refs/pull/<n>/merge:…/merge` then resolve head_oid (=`lastMergeSourceCommit`) | url=fork remoteUrl, `+<sourceRefName>:…/head` | parse `lastMergeSourceCommit`/`lastMergeTargetCommit`, `forkSource`/`repository.remoteUrl`, `isFork` |
| **Bitbucket** | url=None, `+refs/heads/<dest>:…/base`, resolve=base_oid | url=None, `+refs/heads/<src>:…/head` | url=source repo clone URL, `+refs/heads/<src>:…/head` | parse `source.commit.hash`, `destination.commit.hash`, `source.repository.links.clone` (fork) |

Notes: (a) when the head SHA is already local (cache hit / same-repo already fetched),
`fetch_pr_endpoints` skips the network per the offline-fallback rule. (b) local dest refs
`refs/bonsai/pr/<n>/*` are namespaced so they never collide with user refs; they are
force-updated (`+`) and left in the repo (harmless, cheap; cleanup is Polish).

---

## 4. IPC surface

Commands = request/response (no events; PR diff never mutates the local repo besides
fetching objects — do NOT emit `repo-changed`).

```ts
// ipc-api-forge.ts
/** Auto-fetch base+head, then compute the base…head diff LOCALLY. Errors:
 *  noRepo | forgeUnsupported | noRemote | forgeApi | forgeRateLimited |
 *  networkError | authFailed | git. */
forgePrDiff(repoId: string, number: number): Promise<PrDiffStats>;

/** Hunks for ONE file of the PR diff. Oids come from a prior forgePrDiff (no
 *  network, no refetch). Errors: noRepo | git. */
forgePrFileDiff(
  repoId: string, mergeBaseOid: string, headOid: string,
  path: string, origPath: string | null,
  fullContext: boolean, intraline: boolean,
): Promise<FileDiff>;
```

```ts
// forge.ts (types)
export interface PrDiffStats {
  additions: number;
  deletions: number;
  changedFiles: number;
  mergeBaseOid: string; // "" for unrelated histories
  baseOid: string;
  headOid: string;
  files: FileDiffHeader[]; // sorted path-asc, headers only
}
```

Rust command signatures (mirror the `forge_get_pr` house shape):

```rust
#[tauri::command]
pub async fn forge_pr_diff(app, state, repo_id: String, number: u64)
    -> Result<PrDiffStats, AppError>;
// _inner: resolve key → spawn_blocking { provider.pr_refs(number)?;
//   pr_diff::fetch_pr_endpoints(&workdir, &refs.base_fetch, &refs.head_fetch)?;
//   pr_diff::pr_diff_headers(&workdir, &ep.base_oid, &ep.head_oid) }

#[tauri::command]
pub async fn forge_pr_file_diff(app, state, repo_id: String, merge_base_oid: String,
    head_oid: String, path: String, orig_path: Option<String>,
    full_context: bool, intraline: bool) -> Result<FileDiff, AppError>;
// _inner: spawn_blocking { pr_diff::pr_file_diff(...) } — pure local, no provider.
```

**Counts strategy (recommended):** separate `forgePrDiff` command; leave `PrDetail`
forge counts as untouched fallback. `PrDetailView` prefers `PrDiffStats` once loaded and
falls back to `detail.additions/…` while the local diff is pending or failed.

---

## 5. Auto-fetch flow, caching, errors

- **When:** the frontend calls `forgePrDiff` on PR open (after / alongside `forgeGetPr`),
  in a separate effect so the body/mergeable render immediately and the diff streams in.
- **Caching / debounce:** file clicks call `forgePrFileDiff` with the oids from the loaded
  `PrDiffStats` — **no refetch**. Reopening the same PR: frontend keeps the last
  `PrDiffStats` keyed by PR number and reuses it if `summary.headSha` is unchanged; only
  refetches when head advanced or on manual refresh. (Backend fetch is also idempotent and
  cheap when up-to-date.) *Optional* backend 60 s TTL guard keyed by `(repoId, number)` —
  **OQ-1**, recommend skipping (frontend guard suffices).
- **Offline / fetch-fail:** `fetch_pr_endpoints` falls back to cached local oids when
  present; only errors (`networkError`/`authFailed`) when the oid is unreachable. Frontend
  surfaces the error inline in the files section (see §6), body/counts-fallback still shown.

---

## 6. Frontend states (data contract for ui-designer)

`PrDetailView` files section is a small state machine:
- `idle` → `loading` (spinner "computing diff…") while `forgePrDiff` pending.
- `ready`: show local `+X / −Y / N files` (replacing forge counts) + changed-files list.
  Each row: path, status glyph, `+adds/−dels`, binary marker. Click → open file diff in
  reused viewer via `forgePrFileDiff`.
- `empty`: `files.length === 0` → "No changes between base and head".
- `error`: fetch-failed/offline/unresolved → inline message + Retry button (re-invokes
  `forgePrDiff`); forge counts remain as fallback in the header.
- Per-file diff has its own loading/error inside the viewer (existing pattern).

ui-designer contract `docs/contracts/P89-ui.md` owns visuals; this section fixes only the
states + which data each needs.

---

## 7. Sub-increments (one fresh-context pass each)

**P89a — backend ref-resolution + local diff engine + IPC.**
Files: `crates/bonsai-core/src/git/pr_diff.rs` (new), `git/mod.rs`;
`crates/bonsai-forge/src/types.rs`, `provider.rs`, and the four forge impls' `pr_refs`
(+ DTO fields per §3); `src-tauri/src/commands/forge.rs`, command registration.
Deliver: `forge_pr_diff` + `forge_pr_file_diff` working against GitHub + one non-GitHub
forge; unit test `pr_diff_headers`/`pr_file_diff` on a scratch repo (fork simulated by a
2nd remote). *(If the four forge `pr_refs` impls make this too large, split the non-GitHub
three into P89a2.)*

**P89b — frontend types + IPC wiring + mock.**
Files: `src/ipc/types/forge.ts`, `ipc-api-forge.ts`, `src/ipc/tauri/forge.ts`,
`src/ipc/mock/handlers/forge.ts`, `src/ipc/fixtures/forge.ts`. Deliver: `forgePrDiff`/
`forgePrFileDiff` in real + mock; fixture `PrDiffStats` + per-file `FileDiff` so the panel
renders in the browser harness (incl. an `?forge=` variant for the error/empty states).

**P89c — PrDetailView files list + per-file diff view.**
Files: `src/components/PrDetailView.tsx` + new child components (files-list, file-row,
diff-open) each in its own file (≤500 lines); reuse `DiffView`/`DiffViewSplit`. Deliver:
the §6 state machine, local counts override, click-to-diff.

---

## 8. Acceptance criteria (mirror TODO P89)

1. Opening a PR shows correct `+X / −Y / N files` computed locally, matching
   `git diff <merge-base>..<head>` (verified against a scratch fixture).
2. Changed-files list renders; selecting a file shows its diff in the existing viewer.
3. Works when head is a fork branch / not yet fetched (auto-fetch brings it local).
4. Graceful states: fetching, fetch-failed/offline, base/head unresolved, empty diff.
5. Forge-agnostic: each forge supplies `PrRefs`; the diff path is shared bonsai-core code.
6. No Rust/React boundary violation; heavy git2 in `spawn_blocking`; all files ≤ ~500 lines;
   every new command served by the mock layer.

---

## 9. Open questions

- **OQ-1** Backend TTL fetch guard: recommend NOT adding (frontend headSha guard + cheap
  idempotent fetch suffice). Confirm.
- **OQ-2** Azure head via `refs/pull/N/merge` gives the server merge commit, not the raw
  source tip; we resolve `lastMergeSourceCommit` after fetching, which is correct for
  base…head. If a forge omits `lastMergeSourceCommit` (very old TFS), fall back to fork
  `sourceRefName` fetch. Acceptable? (recommend yes.)
- **OQ-3** Leaving `refs/bonsai/pr/<n>/*` refs in the repo (no cleanup in v1) — recommend
  yes; cleanup is Polish. Confirm.
