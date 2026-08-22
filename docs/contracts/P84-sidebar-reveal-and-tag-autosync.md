# P84 — Sidebar click-to-reveal-in-graph + automatic tag sync

Two independent features. Feature 1 is pure frontend. Feature 2 adds one core fn + one
command, wired into the fetch flow. Keep them separable — they can ship in either order.

---

## Feature 1 — Sidebar click-to-reveal-in-graph

### Decisions

- **Single-click = reveal** (scroll graph to the ref's target row + 1s flash). Double-click =
  checkout (unchanged). Enter/Space = existing `useSidebarTreeItem` primary (unchanged).
- **Reveal DOES set `selectedIndex`** (recommended): the right panel shows that commit, matching
  GitKraken. The flash is an *additional* transient cue on top of the persistent selection.
  Rationale: a reveal with no selection change leaves the right panel stale and confusing; and we
  already own `setSelectedIndex` in the container.
- **oid→row resolution lives in the container**, off `layout.nodes`. Rows never touch the graph.
- **Ref-name is the primary key, oid the fallback.** Every branch/remote/tag row's identifier
  already equals a `RefLabel.name` on some `GraphNode.refs` entry (`"main"`, `"origin/main"`,
  `"v1.0"`). Stashes are not ref-labelled in the graph, so stash rows reveal by **oid**.
- **Graceful degrade:** target not found in the current (possibly truncated / not-yet-streamed)
  layout → **no-op** (no scroll, no selection change) plus a transient toast
  `"<name> isn't in the loaded history yet"`. Never throw.

### Data flow

```
row onClick ──onReveal(target)──▶ Sidebar (passes through) ──▶ container.handleReveal(target)
      container: idx = resolveRevealIndex(target, layout)
        idx === null → toast, return
        idx !== null → setSelectedIndex(idx); setFlash({ index: idx, oid }); graphRef scrolls
      GraphCanvas: renders flashIndex ring for ~1s, then container clears it
```

### New shared type (add to `src/ipc/types/graph.ts` or a new `src/graph/reveal.ts`)

```ts
export type RevealTarget =
  | { kind: 'ref'; name: string }   // RefLabel.name: "main" | "origin/main" | "v1.0"
  | { kind: 'oid'; oid: string };   // full 40-hex (stashes)
```

### Row prop additions (rows.tsx + TagsSection.tsx) — MINIMAL ADDITIVE HOOK

`rows.tsx` is being edited concurrently (icon migration). Add exactly **one optional prop per
row** and **one `onClick` handler** on the existing `<li>`. Do not restructure. Do not touch
`useSidebarTreeItem`, `onDoubleClick`, or `onContextMenu`.

```ts
// added to BranchRow / RemoteRow / StashRow / TagRow props (all optional):
onReveal?(target: RevealTarget): void;
```

Per-row `onClick` (added to the existing `<li {...item} …>`):

- `BranchRow`:  `onClick={() => onReveal?.({ kind: 'ref', name: branch.name })}`
- `RemoteRow`:  `onClick={() => onReveal?.({ kind: 'ref', name })}`  (name is `origin/…`)
- `TagRow`:     `onClick={() => onReveal?.({ kind: 'ref', name })}`
- `StashRow`:   `onClick={() => onReveal?.({ kind: 'oid', oid })}`  (uses the row's rendered oid)

Notes:
- A double-click fires `onClick` twice then `onDoubleClick`; reveal is idempotent and cheap, so
  the extra reveal before a checkout is harmless. Do **not** add click/dblclick debouncing.
- `ConfiguredRemoteRow` / `WorktreeRow` / `SubmoduleRow` / `DetachedHeadRow` get **no** `onReveal`
  (not commit-targeting refs). DetachedHeadRow could reveal its oid later — out of scope.
- `onReveal` is optional so the concurrent branch and existing tests compile unchanged.

### Sidebar.tsx threading

Add `onReveal?(target: RevealTarget): void` to `SidebarProps`. Thread it verbatim into each
`BranchRow` / `RemoteRow` / `StashRow` and into `<TagsSection onReveal={onReveal} …>`
(TagsSection forwards to its inner `TagRow`s). One prop, passed straight through.

### Container (RepoWorkspace.tsx) — resolution + flash lifecycle

Build the lookup once per layout (memo):

```ts
// resolveRevealIndex: pure, no IPC.
function buildRevealIndex(layout: GraphLayout | null): {
  byRef: Map<string, number>;   // RefLabel.name → row index
  byOid: Map<string, number>;   // GraphNode.id → row index
} { /* iterate layout.nodes; for each node record byOid[node.id]=i and
       for each ref in node.refs record byRef[ref.name]=i (first wins) */ }

function resolveRevealIndex(t: RevealTarget, idx: RevealIndex): number | null {
  return t.kind === 'ref' ? idx.byRef.get(t.name) ?? null
                          : idx.byOid.get(t.oid) ?? null;
}
```

Flash state + handler:

```ts
const [flashIndex, setFlashIndex] = useState<number | null>(null);
const flashTimer = useRef<ReturnType<typeof setTimeout> | null>(null);

const handleReveal = useCallback((t: RevealTarget) => {
  const i = resolveRevealIndex(t, revealIndex);
  if (i === null) { pushToast(`${labelOf(t)} isn't in the loaded history yet`); return; }
  setSelectedIndex(i);
  // GraphCanvas already auto-scrolls selectedIndex into view (GraphCanvas.tsx ~L514),
  // so no imperative scroll call is required; selection drives the scroll.
  setFlashIndex(i);
  if (flashTimer.current) clearTimeout(flashTimer.current);
  flashTimer.current = setTimeout(() => setFlashIndex(null), 1000);
}, [revealIndex, pushToast]);
// clear timer on unmount.
```

Pass `onReveal={handleReveal}` to `<Sidebar>` and `flashIndex={flashIndex}` down through
`WorkspaceGraphPane` to `<GraphCanvas>`.

> **Reuse existing auto-scroll**: GraphCanvas already scrolls a newly non-null `selectedIndex`
> into view (the effect at ~L514 using `scrollRowIntoView`). Setting `selectedIndex` is therefore
> sufficient to scroll; do not add a second scroll path. If a reveal targets the already-selected
> row, still set `flashIndex` (selection is unchanged so no scroll fires — acceptable, the row is
> already in view).

### GraphCanvas prop addition

```ts
// GraphCanvasProps:
/** P84: row index to flash briefly on click-to-reveal; null = no flash. A new
 *  non-null value (re)starts the highlight; the container clears it after ~1s. */
flashIndex?: number | null;
```

Rendering: add `flashIndex` to the draw inputs (alongside `selectedIndex`/`matchSet`) and repaint
when it changes (add to the paint effect deps at ~L482). In the row draw pass, when
`row === flashIndex` draw a highlight ring/row-band distinct from selection (spec'd by
ui-designer — reuse a token, e.g. an accent-colored 2px ring pulse). No new geometry math; it is a
per-row style branch keyed on the index. WIP-row offset does not apply (flashIndex is a layout row
index, same basis as selectedIndex).

### Files touched (Feature 1)

- `src/graph/reveal.ts` **(new)** — `RevealTarget` type (or add to `types/graph.ts`).
- `src/components/sidebar/rows.tsx` — **SHARED (concurrent edit)**; additive `onReveal` prop +
  `onClick` on BranchRow/RemoteRow/StashRow only.
- `src/components/sidebar/TagsSection.tsx` — additive `onReveal` prop on TagsSection + TagRow.
- `src/components/Sidebar.tsx` — thread `onReveal` through props.
- `src/components/RepoWorkspace.tsx` — reveal index memo, `handleReveal`, `flashIndex` state,
  wire to Sidebar + GraphCanvas.
- `src/components/repoWorkspace/WorkspaceGraphPane.tsx` — pass `flashIndex` through.
- `src/graph/GraphCanvas.tsx` — `flashIndex` prop + draw + repaint dep.
- `src/graph/draw*.ts` (draw layer) — flash ring branch.

### Acceptance criteria (Feature 1)

1. Single-click a branch/remote/tag row whose tip is in the loaded graph → graph scrolls that row
   into view, `selectedIndex` updates (right panel shows that commit), a highlight appears and
   fades within ~1.2s.
2. Single-click a stash row → reveals its commit by oid.
3. Double-click still checks out; Enter/Space behavior unchanged; context menu unchanged.
4. Reveal of a ref not in the loaded/truncated history → no scroll, no selection change, a toast.
5. Rapid consecutive reveals cancel the prior flash timer (only the latest row flashes).
6. Works under `VITE_MOCK_IPC=1` with fixture layout (no new IPC needed).

---

## Feature 2 — Automatic tag sync from remote

Backend building blocks already exist in `crates/bonsai-core/src/git/tag_sync.rs`
(`list_tag_sync`, `force_refresh_tag`, `delete_remote_tag`, cred chain, peeled-committish rules).
This adds a **non-interactive reconciliation** run on fetch.

### Behavior

On fetch (and optionally on repo open), best-effort:
- **(a) adopt** every `RemoteOnly` tag → create the local tag at the remote's committish.
- **(b) move** a `Stale` tag's local ref onto the remote target **only if** the remote committish
  is a **strict descendant** of the local committish (remote strictly ahead). If local is ahead,
  or they diverged, or ancestry can't be determined → **skip** (report as diverged), leave the
  local tag untouched.
- `InSync` / `LocalOnly` → untouched.
- **Never fails the fetch.** No remote configured, auth failure, or network error → return an
  empty (or partial) report; log; do not propagate as an error.

### New core fn (tag_sync.rs)

```rust
/// Result of one non-interactive auto-sync pass. Compact, camelCase on the wire.
#[derive(Debug, Clone, Default, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TagAutoSyncReport {
    /// The remote actually reconciled ("" when none configured / skipped).
    pub remote: String,
    /// Tag names newly created locally from a remote-only tag.
    pub adopted: Vec<String>,
    /// Tag names whose local ref was fast-forwarded onto the remote target.
    pub moved: Vec<String>,
    /// Stale tags left untouched (local ahead or diverged — not a strict FF).
    pub skipped_diverged: Vec<String>,
}

/// Blocking, best-effort, NEVER-fail-the-fetch tag reconciliation.
/// `remote`: None => default ("origin" else first configured). Returns an empty
/// report (Ok) — not Err — when no remote is configured or the remote is
/// unreachable/auth-failed. Only true programming/repo-corruption faults surface
/// as Err; callers on the fetch path may still ignore that.
pub fn auto_sync_tags(
    workdir: &Path,
    remote: Option<&str>,
) -> Result<TagAutoSyncReport, AppError>;
```

### Algorithm (pseudocode)

```
auto_sync_tags(workdir, remote):
  repo = open_repo_at(workdir)
  remote_name = resolve_default_remote(repo, remote)          // NoRemote => return Ok(empty)
     └ on NoRemote error: return Ok(TagAutoSyncReport::default())

  // 1. Fetch all remote tag OBJECTS into a private namespace so both committishes
  //    exist locally for ancestry checks. Force (+) so temp refs always update.
  find_remote(remote_name)                                     // NotFound => Ok(empty)
  fetch(["+refs/tags/*:refs/bonsai-tagsync/*"],
        FetchOptions{ callbacks: cred chain, download_tags: None })
     └ on auth/network err (map_remote_err / evict_fresh_on_auth_fail):
         cleanup_temp_refs(); return Ok(empty-with-remote-name)   // DO NOT propagate

  report = TagAutoSyncReport{ remote: remote_name, .. }

  // 2. Build local + remote(temp) peeled-committish maps.
  local = collect_local_tags(repo)                     // name -> (peeled_oid, annotated)
  temp  = collect_glob(repo, "refs/bonsai-tagsync/*")  // name -> peeled_oid (peel Any)

  // 3. Classify + apply.
  for (name, remote_oid) in temp:
    match local.get(name):
      None:                                            // RemoteOnly -> ADOPT
        create lightweight ref refs/tags/<name> = remote_oid  (validate_tag_name first)
        report.adopted.push(name)
      Some((local_oid, _)):
        if local_oid == remote_oid: continue           // InSync
        // Stale: move ONLY if remote strictly descends from local.
        if repo.graph_descendant_of(remote_oid, local_oid) == Ok(true):
          force-update ref refs/tags/<name> = remote_oid     // FF the local tag
          report.moved.push(name)
        else:                                          // local ahead / diverged / unknown
          report.skipped_diverged.push(name)
  // (LocalOnly tags — in `local` but not `temp` — are ignored here.)

  // 4. Always clean up the temp namespace.
  cleanup_temp_refs(repo)   // delete every ref under refs/bonsai-tagsync/*
  sort report vecs case-insensitively
  return Ok(report)
```

Implementation notes:
- **Adopt/move write refs directly** (`repo.reference("refs/tags/<name>", oid, force, log)`) off
  the already-fetched objects — no second network round-trip. Annotated tags: point the local ref
  at the same peeled committish the temp ref resolves to (lightweight local tag), consistent with
  how `force_refresh_tag` overwrites. (Preserving the annotated tag *object* would need
  `+refs/tags/*:refs/tags/*` directly; **recommend the lightweight-committish form** for v1 — it
  keeps the graph correct, which is all the UI shows. Flag: if annotated-object fidelity is later
  required, switch temp-namespace adoption to copy the tag object.)
- `cleanup_temp_refs` must run on every exit path after the fetch (use a guard / helper). Leaving
  `refs/bonsai-tagsync/*` around would pollute the ref list.
- `graph_descendant_of(remote, local)` returns `Ok(false)` when equal (already filtered) and can
  error on missing objects → treat any non-`Ok(true)` as skip.

### IPC surface

Add a standalone command **and** wire it into fetch. **Recommendation: do both.**

- **Standalone command** `auto_sync_tags` — lets the UI trigger it on repo open and surface the
  report (toast: "Adopted 2 tags, moved 1, skipped 1 diverged"). Also independently testable.
- **Fetch integration** — call `auto_sync_tags` after `fetch_all` succeeds, best-effort,
  **inside** `fetch` (not `fetch_inner`, to keep the runtime-free core pure), folding the result
  into the fetch response so the sidebar refreshes tags in the same round-trip. Justification: tag
  sync is exactly "what changed upstream", which is the fetch's job; making it implicit means the
  user gets correct tags without a manual step, and best-effort means it can never break fetch.

Command (`src-tauri/src/commands/tags.rs`):

```rust
#[tauri::command]
pub async fn auto_sync_tags(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    remote: Option<String>,
) -> Result<tag_sync::TagAutoSyncReport, AppError> { … spawn_blocking(auto_sync_tags) … }
// + auto_sync_tags_inner (runtime-free) mirroring the existing pattern.
```

Register in `src-tauri/src/commands/mod.rs` `generate_handler!`.

Fetch wiring (`src-tauri/src/commands/remotes.rs::fetch`): after `fetch_inner` succeeds, run
`tag_sync::auto_sync_tags(&path, None)` in `spawn_blocking`, `let _ =` the result (or fold into
`FetchResult` — see below), never `?`. Do it in the same place the commit-graph rewrite is fired.

**Fold-into-FetchResult option (recommended):** add an optional field to `FetchResult`:

```rust
// src-tauri FetchResult (and TS FetchResult):
#[serde(skip_serializing_if = "Option::is_none")]
pub tag_auto_sync: Option<TagAutoSyncReport>,   // TS: tagAutoSync?: TagAutoSyncReport
```

so the frontend can toast + refresh the tags list from the same response. If touching
`FetchResult` is undesirable mid-branch, fall back to the standalone command called by the
frontend right after `fetch` resolves. **Flag for orchestrator:** pick fold-in vs. separate call.

### TS types (`src/ipc/types/tags.ts` or alongside `TagSyncReport`)

```ts
export interface TagAutoSyncReport {
  remote: string;
  adopted: string[];
  moved: string[];
  skippedDiverged: string[];
}
```

IPC binding (`src/ipc/tauri/tags.ts`):

```ts
autoSyncTags(repoId: string, remote: string | null): Promise<TagAutoSyncReport> {
  return invoke<TagAutoSyncReport>('auto_sync_tags', { repoId, remote });
},
```

### Mock handler (`src/ipc/mock/handlers/tagSync.ts`)

Add `autoSyncTags(repoId, remote)`:
- `await delay(400)`; `requireRepo`; honor `remoteTrigger`:
  `authfail`/`network` → **return an empty report** (`{ remote, adopted:[], moved:[], skippedDiverged:[] }`)
  NOT a throw (auto-sync is best-effort — mirrors Rust never-fail).
- Otherwise mutate the live `reportFor(repoId, remote)` fixture: for each `remote-only` entry →
  set `localOid = remoteOid`, status `in-sync`, collect into `adopted`; for each `stale` entry
  where the fixture marks the remote as a descendant (add a fixture flag, e.g.
  `remoteDescends: true`) → `localOid = remoteOid`, status `in-sync`, collect into `moved`; other
  `stale` → `skippedDiverged`. Return the report.
- If `resolveRemote` finds no remote → return empty report (not throw).
- Extend `src/ipc/fixtures/tagSync.ts` with at least one remote-only, one FF-able stale, and one
  diverged stale entry so the harness exercises all three buckets.

### Files touched (Feature 2)

- `crates/bonsai-core/src/git/tag_sync.rs` — `TagAutoSyncReport` + `auto_sync_tags` +
  `cleanup_temp_refs` helper.
- `crates/bonsai-core/src/git/tag_sync_tests.rs` — new tests (below).
- `src-tauri/src/commands/tags.rs` — `auto_sync_tags` command + inner.
- `src-tauri/src/commands/mod.rs` — register handler.
- `src-tauri/src/commands/remotes.rs` — best-effort call in `fetch` (+ optional `FetchResult`
  field).
- `src-tauri` FetchResult definition (if folding in) — **SHARED with M6 fetch shape.**
- `src/ipc/types/tags.ts` (+ `types/index` re-export) — `TagAutoSyncReport`; optional
  `FetchResult.tagAutoSync`.
- `src/ipc/tauri/tags.ts` — `autoSyncTags` binding.
- `src/ipc/mock/handlers/tagSync.ts` + `src/ipc/fixtures/tagSync.ts` — mock + fixtures.
- `src/ipc/mock/index` / `IpcApi` — register `autoSyncTags`.
- `src/components/RepoWorkspace.tsx` — on fetch success (and optionally repo open), refresh tags
  + toast the report counts.

### Acceptance criteria (Feature 2)

1. Fetch against a remote with a new tag → that tag exists locally afterward (adopted); appears in
   the sidebar Tags section.
2. A local tag the remote moved forward (remote strictly ahead) → local tag fast-forwards to the
   remote target after fetch (moved).
3. A local tag ahead of / diverged from the remote → local tag unchanged; reported as
   `skippedDiverged`.
4. `InSync` and `LocalOnly` tags are never modified.
5. No remote configured → fetch still succeeds; report is empty; no error surfaced.
6. Auth/network failure during auto-sync → fetch result still returns; report empty/partial; no
   thrown error; `refs/bonsai-tagsync/*` left clean.
7. `refs/bonsai-tagsync/*` never persists after any auto_sync_tags call (success or failure).
8. Harness (`VITE_MOCK_IPC=1`): `autoSyncTags` returns a report with all three buckets populated
   from fixtures; auth/network trigger returns empty (no throw).

### Test checklist (Feature 2, `tag_sync_tests.rs`, against local bare-repo remotes)

- adopt: remote-only tag → created locally at remote committish.
- move-FF: stale tag, remote strict descendant → local ref updated; in `moved`.
- skip-diverged: stale, local ahead → untouched; in `skippedDiverged`.
- skip-diverged: stale, siblings (no ancestry) → untouched; in `skippedDiverged`.
- in-sync + local-only: untouched, absent from all three vecs.
- annotated remote tag adopted → local peeled committish matches remote peeled committish.
- no-remote repo → `Ok(default)`, empty.
- temp namespace cleaned on both success and simulated-fetch-failure paths.

---

## Open items flagged for orchestrator

1. **F1 reveal == selection**: recommend yes (selection + flash). Confirm.
2. **F2 fetch integration shape**: fold `tagAutoSync` into `FetchResult` (recommended) vs. a
   separate frontend `autoSyncTags` call after `fetch`. `FetchResult` is a shared M6 shape.
3. **F2 annotated-tag fidelity**: v1 adopts as lightweight local tags at the peeled committish.
   Confirm that's acceptable (vs. copying the annotated tag object).
4. **F2 on repo open**: spec allows an optional auto-sync on open. Recommend deferring to fetch
   only for v1 to avoid a network hit on every open; expose the standalone command so it's easy to
   add later.
