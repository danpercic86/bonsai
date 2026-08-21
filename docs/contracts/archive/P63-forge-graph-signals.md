# P63 — Forge signals on the commit graph (PR + CI badges)

Second Phase-4 milestone. Shared conventions: `docs/contracts/phase4-forge-overview.md`. Data
foundation: `docs/contracts/P62-forge-foundation.md` — P63 CONSUMES P62's `CommitStatus`/`CheckRollup`
and the `forge_*` commands; it does NOT redesign them and adds NO provider code (`combined_status` is
already implemented in `bonsai-forge`, per overview §F2 — P63 only wires an IPC command to it + renders).

Reuse verbatim (READ, do not reinvent):
- Async fill-in pattern — `src/components/repoWorkspace/useCommitVerification.ts` (req-id last-wins guard,
  debounced fetch, cache → new map identity → repaint). P63's `useForgeSignals` mirrors it.
- Ref-band layout — `src/graph/refLabels.ts` (`layoutRefLabels`/`LaidRefLabel`/`drawRefLabelAt`; the
  ahead/behind `chip` is the exact precedent for a per-branch adornment that reserves advance + can be
  popped into the "+n" overflow). Single source of truth for draw AND hit-test.
- Badge glyph pattern — `src/graph/drawRowText.ts` `drawBadge`/`drawGoodBadge`/... (P58 verified badge)
  and `src/graph/verifyBadge.ts` (pure status→visual classifier). P63's CI dot copies this shape.
- Display bag — `src/graph/rightColumns.ts` `GraphDisplayOptions` (already carries the LEFT-band
  `showAheadBehind`+`branchStats`; P63 rides its per-branch maps in alongside them).
- Prefs plumbing — `GraphPrefs` in `src/ipc/types.ts` + `src-tauri/src/settings.rs` (struct+Default+
  round-trip tests), defaults in `src/App.tsx` and `src/ipc/mock/persistence.ts`, toggles in
  `src/components/SettingsGraphSection.tsx`. P51/P58 added SHA/author/date/ahead-behind/compact/signature.

## 1. Goal & scope

Light TWO forge-driven, **branch-tip-scoped** signals on the canvas graph, each **individually
toggleable** and **suppressed in compact mode**:

- **PR badge** — a small trailing pill `#<num>` on a branch pill, tinted by PR state
  (open/draft/merged/closed). Click → open P62's right-pane PR panel to that PR's detail.
- **CI badge** — a small status dot on a branch pill: success/failure/pending/neutral (none ⇒ nothing),
  from P62's `CommitStatus`/`CheckRollup`.

Both fill in ASYNC from a per-branch status cache (short TTL, refreshed on fetch/pull + focus + manual
refresh); rows paint immediately and badges appear when data arrives — the fetch NEVER blocks canvas
paint. IN scope: 2 new `GraphPrefs` toggles, the per-row badge model + canvas layout/hit-test, the
`useForgeSignals` cache, ONE new IPC command (CI status), mock+fixture parity. OUT of scope: any new
provider/GitHub code, per-commit (non-tip) CI, auto-polling, merged/closed PR fetch (badge model
supports them; v1 fetches open only — OQ-3), the PR panel itself (P62), AI descriptions (P64).

**Command-count delta (RELATIVE):** **+1** command (`forge_commit_statuses`) over P62's end state;
PR badges reuse P62 `forge_list_prs`; gating reuses P62 `forge_repo_context`. No new events, no
channels. RECOUNT `generate_handler!` in `src-tauri/src/lib.rs` at implementation and fix the `TODO.md`
line (P62 lands it at ~154 ⇒ P63 ~155).

## 2. Module boundaries

### New files
| File | Responsibility |
|---|---|
| `src/graph/forgeBadges.ts` | PURE badge subsystem: `PrBadge`/`CiBadge` model, `prBadgeVisual`/`ciBadgeVisual` classifiers (state/rollup → colors+glyph), width helpers, draw fns, and `branchSignals(entity,node,display)` gate. No React, no IPC. Mirrors `verifyBadge.ts`+`drawRowText.ts` glyph helpers. |
| `src/components/repoWorkspace/useForgeSignals.ts` | The status cache hook (§7): builds `prByBranch`+`ciBySha`, TTL, refresh triggers, req-id guard. Mirrors `useCommitVerification.ts`. |

### Modified files (senior-dev edits; architect only designs)
| File | Change |
|---|---|
| `src/graph/rightColumns.ts` | Extend `GraphDisplayOptions` with `showPrBadge`,`showCiStatus`,`prByBranch`,`ciBySha` (§3). Right-column geometry UNCHANGED (badges live in the LEFT band). |
| `src/graph/refLabels.ts` | `LaidRefLabel` gains `signals`; `layoutRefLabels` computes+reserves signal advance after the chip; `drawRefLabelAt` draws them (§5). |
| `src/graph/metrics.ts` | Add signal geometry constants (§5). |
| `src/graph/GraphCanvas.tsx` | New prop `onOpenPr`; `handleClick`+`computeHoverTarget` signal hit-tests; `TooltipState` gains `pr`/`ci` (§6). `display` already flows to `layoutRefLabels` ⇒ no map prop needed. |
| `src/components/RepoWorkspace.tsx` | Instantiate `useForgeSignals`; extend `graphDisplay` memo (with compact gate); wire refresh triggers; `onOpenPr` → set P62 `rightPaneTab='prs'` + `prNav` state; thread props down. |
| `src/components/WorkspaceGraphPane.tsx` | Pass-through `onOpenPr` (maps already ride inside `display`). |
| `src/components/SettingsGraphSection.tsx` | 2 checkboxes under a "Forge signals" subheading + a "requires a connected forge" note. |
| `src/components/PrPanel.tsx` (P62) | Add input seam `openToPr?: PrNavRequest \| null` (§6) — **cross-milestone dependency, see OQ-6**. |
| `src/ipc/types.ts` | `GraphPrefs.showPrBadge`/`showCiStatus`; `forgeCommitStatuses` on `IpcApi`; `PrNavRequest`. |
| `src/ipc/tauri.ts` | `forgeCommitStatuses` invoke wrapper. |
| `src/ipc/mock/handlers/forge.ts` (P62) | Add `forgeCommitStatuses` handler (§9). |
| `src/ipc/fixtures/forge.ts` (P62) | Add `commitStatusFor(sha)` fixtures; ensure PR `sourceBranch` values match mock `GraphLayout` branch names (§9). |
| `src/App.tsx`, `src/ipc/mock/persistence.ts` | Default `showPrBadge:false`,`showCiStatus:false`. |
| `src-tauri/src/commands/forge.rs` | `forge_commit_statuses` triple (§4). |
| `src-tauri/src/commands/mod.rs`, `src-tauri/src/lib.rs` | Register the command. |
| `src-tauri/src/settings.rs` | `GraphPrefs { show_pr_badge, show_ci_status }` + Default + extend the round-trip tests. |

## 3. Types

### 3.1 `GraphPrefs` additions (TS `src/ipc/types.ts` + Rust `settings.rs`)
```ts
// GraphPrefs (add):
/** P63: PR-state badge on branch-tip pills. Default false (network+auth-gated). */
showPrBadge: boolean;
/** P63: CI/build-status dot on branch-tip pills. Default false (network+auth-gated). */
showCiStatus: boolean;
```
```rust
// settings.rs GraphPrefs (add; #[serde(default)] already set):
/// P63: PR-state badge on branch-tip pills. Default false.
pub show_pr_badge: bool,
/// P63: CI/build-status dot on branch-tip pills. Default false.
pub show_ci_status: bool,
// Default: show_pr_badge: false, show_ci_status: false,
```
**Default OFF (justified):** forge signals need a network round-trip AND a stored PAT; for the many
sessions with no token or a non-GitHub origin they are inert, so ON-by-default would be a dead toggle
and would fire surprise API calls (rate limits). The signature badge is ON because it is local+free;
forge badges are opt-in. Extend the settings.rs off/on round-trip tests (lines ~949/1001) with both.

### 3.2 `GraphDisplayOptions` additions (`rightColumns.ts`)
```ts
// GraphDisplayOptions (add — same "rides along for the LEFT band" precedent as branchStats):
/** P63: draw the PR badge (already AND-ed with !compact by the caller). */
showPrBadge: boolean;
/** P63: draw the CI dot (already AND-ed with !compact by the caller). */
showCiStatus: boolean;
/** P63: branch SHORT-name → PR badge (from open PR list). Empty map ok. */
prByBranch: ReadonlyMap<string, PrBadge>;
/** P63: commit sha → CI badge (branch-tip shas only). Empty map ok. */
ciBySha: ReadonlyMap<string, CiBadge>;
```
The caller (RepoWorkspace) enforces the compact rule at assembly: `showPrBadge:
graphPrefs.showPrBadge && !graphPrefs.compact` (same for CI). So `forgeBadges` only checks the two
booleans — no compact plumbing into the pure layer. All other `GraphDisplayOptions` construction sites
(GraphCanvas self-test ~L727, tests) add the four fields (false + empty Maps).

### 3.3 Badge model (`forgeBadges.ts`)
```ts
import type { PrState, CheckRollup } from '../ipc';

/** Per-branch PR signal (subset of P62 PrSummary; title for the tooltip). */
export interface PrBadge { number: number; title: string; state: PrState; isDraft: boolean; url: string; }
/** Per-tip CI signal (subset of P62 CommitStatus; counts for the tooltip). */
export interface CiBadge { rollup: CheckRollup; passed: number; failed: number; pending: number; total: number; }

/** Pill visual for a PR state (mirrors refLabels.entityStyle shape). draft ⇒ open+outline. */
export function prBadgeVisual(pr: PrBadge, theme: Theme):
  { label: string; fill: string; text: string; border: string | null };
//   open   → filled green;  draft → grey outline;  merged → filled purple;  closed → filled red.
//   label  → `#${pr.number}`.

/** CI dot visual, or null when NOTHING draws (rollup 'none'). Copies verifyBadgeKind's null pattern. */
export function ciBadgeVisual(rollup: CheckRollup, theme: Theme):
  { glyph: 'check' | 'x' | 'dot' | 'dash'; color: string } | null;
//   success→green check; failure|error→red x; pending→amber dot; neutral→grey dash; none→null.

/** PURE gate: a branch entity's signals from the display maps (nulls when off/absent/not a branch). */
export function branchSignals(
  entity: RefEntity, node: GraphNode, display: GraphDisplayOptions,
): { pr: PrBadge | null; ci: CiBadge | null };
//   pr = display.showPrBadge && entity.kind==='branch' ? display.prByBranch.get(entity.name) ?? null : null
//   ci = display.showCiStatus && entity.kind==='branch' ? display.ciBySha.get(node.id)      ?? null : null
```

### 3.4 `LaidRefLabel.signals` (`refLabels.ts`) + `PrNavRequest` (`types.ts`)
```ts
// LaidRefLabel (add): laid-out signal geometry for draw + hit-test, or null.
signals: { ci: { badge: CiBadge; cx: number } | null;
           pr: { badge: PrBadge; x: number; w: number } | null } | null;

// types.ts — external "open PR N" request into P62's PrPanel (seq re-triggers a repeat click).
export interface PrNavRequest { number: number; seq: number; }
```

## 4. IPC surface (+1 command)

Reuses P62 `forge_list_prs` (PR index) + `forge_repo_context` (auth/provider gate). Adds ONE:

| Command (snake) | TS method | Wire request | Response |
|---|---|---|---|
| `forge_commit_statuses` | `forgeCommitStatuses(repoId, shas)` | `{ repoId, shas: string[] }` | `CommitStatus[]` |

**Batch, not per-sha (OQ-1, REC):** one IPC round-trip / one `spawn_blocking` wrapping N
`combined_status` calls, mirroring `verifyCommits`. Best-effort: dedup+cap `shas` at
`MAX_STATUS_BATCH` (100); per-sha `ForgeApi("not found")`/404 ⇒ OMIT that sha from the result; a FATAL
class (`AuthFailed`/`ForgeAuthRequired`/`NetworkError`/`ForgeRateLimited`) or `open()` failure ⇒
propagate the error (so the hook can back off). Returns only resolved shas (order not guaranteed;
frontend keys by `sha`).
```rust
#[tauri::command]
pub async fn forge_commit_statuses(
    state: tauri::State<'_, AppState>, repo_id: String, shas: Vec<String>,
) -> Result<Vec<CommitStatus>, AppError> {
    forge_commit_statuses_inner(state.inner(), &repo_id, shas).await
}
pub(crate) async fn forge_commit_statuses_inner(
    state: &AppState, repo_id: &str, shas: Vec<String>,
) -> Result<Vec<CommitStatus>, AppError> {
    let workdir = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || {
        let p = bonsai_forge::open(&workdir)?;
        collect_statuses(p.as_ref(), &shas) // dedup+cap; omit not-found; propagate fatal
    }).await.map_err(|e| AppError::Other(format!("task join error: {e}")))?
}
```
```ts
// tauri.ts
forgeCommitStatuses(repoId: string, shas: string[]): Promise<CommitStatus[]> {
  return invoke('forge_commit_statuses', { repoId, shas });
}
// IpcApi doc: rejects forgeUnsupported | forgeAuthRequired | forgeRateLimited | networkError | forgeApi.
```
`CommitStatus`/`CheckRollup` already exist in `types.ts` from P62 — no new DTOs.

## 5. Canvas placement + render (LEFT ref band, riding the branch pill — OQ-4)

Badges are per-branch, so they live on the branch PILL in the LEFT ref band (not the right columns —
those are per-commit; a mostly-empty right "signals column" would waste width). They render as a
trailing group after the pill, in order: **[pill] [ahead/behind chip] [CI dot] [PR pill]**. Their
combined width is RESERVED as part of the pill's advance (extending the existing `chipAdvance`), so a
pill and its signals overflow ATOMICALLY into the "+n" chip — a PR/CI badge never floats without its
branch.

`metrics.ts` additions (CSS px):
```
prBadgeMaxWidth: 46,  // fits "#12345"
prBadgePadX: 5,
ciBadgeSize: 11,      // == iconSize; dot/glyph box
signalGap: 6,         // gap before each signal (== chipGap feel)
```
`layoutRefLabels` (per branch entity, after computing `chip`):
```
sig = branchSignals(entity, node, display)
ciW = sig.ci ? signalGap + ciBadgeSize : 0
prW = sig.pr ? signalGap + prBadgeWidth(ctx, sig.pr) : 0   // prBadgeWidth = 2*prBadgePadX + measure("#num"), capped prBadgeMaxWidth
advance = w + chipAdvance(chip) + ciW + prW                 // reserve; counts toward band budget + pop-rewind
signals = { ci: sig.ci ? {badge, cx: <after chip>+signalGap+ciBadgeSize/2} : null,
            pr: sig.pr ? {badge, x: <after ci>+signalGap, w: prBadgeWidth} : null }
```
`drawRefLabelAt` draws the CI dot then PR pill after the existing chip, using
`ciBadgeVisual`/`prBadgeVisual`. CI glyphs copy `drawRowText`'s good/warn glyph style (save/restore so
lineJoin/cap don't leak). PR pill = rounded-rect (`pillHeight`) fill/text/border from `prBadgeVisual`,
label re-measured to the same cap. **Compact:** `display.showPrBadge`/`showCiStatus` arrive already
false (caller AND-ed `!compact`) ⇒ `branchSignals` returns nulls ⇒ zero advance, nothing drawn.

## 6. Hit-testing + PR-panel navigation

Both handlers recompute the row's laid labels with the SAME `layoutRefLabels(...)` call already used
for ref hit-tests (single source of truth ⇒ signal rects come free).

- **Click → PR** (`GraphCanvas.handleClick`): compute `x` (currently only `y`); for the hit row's laid
  labels, if `x` is within any `signals.pr` rect ⇒ `onOpenPr(pr.number)` and RETURN (do NOT select the
  row). Else the existing row-select behavior. New prop:
  `onOpenPr?(number: number): void` (GraphCanvasProps → WorkspaceGraphPane pass-through → RepoWorkspace).
- **RepoWorkspace wiring:** `onOpenPr(n)` ⇒ set P62 `rightPaneTab='prs'` + `setPrNav({ number:n, seq:s+1 })`.
  Thread `prNav` into `PrPanel` as `openToPr` (§3.4). `PrPanel` runs an effect on `openToPr.seq`:
  fetch `forgeGetPr(n)` (+ comments) and show detail; unauthenticated ⇒ falls back to its ForgeConnect
  flow (can't fetch — acceptable). `seq` makes re-clicking the same PR re-navigate.
- **Hover tooltips (SHOULD):** `TooltipState` gains
  `{ kind:'pr'; lines; anchor }` (`["PR #123 (open)", title]`) and
  `{ kind:'ci'; lines; anchor }` (`["Checks: 3 passed, 1 failed, 1 pending"]` from `CiBadge` counts);
  `computeHoverTarget` tests the `signals` rects (precedence after avatar/overflow/shown-pill, before
  the date column); extend `sameTarget` (joined-lines identity).

## 7. Status cache + refresh flow (`useForgeSignals`)

NOT scroll-virtualized (unlike verify): branch tips are a small BOUNDED set (# branches), fetched
wholesale on refresh triggers — no `onVisibleRangeChange`. Fetch failures are **SILENT** (decoration,
not a user action): log only, keep stale maps; no toast. (PR-panel actions still toast — those are
explicit.)
```
useForgeSignals({ repoId, graphDataRef, showPrBadge, showCiStatus, compact }) -> { prByBranch, ciBySha, refresh }

state: prByBranch: Map<name, PrBadge>, ciBySha: Map<sha, {badge:CiBadge, tsMs:number}>, prTsMs
const TTL_MS = 60_000, DEBOUNCE_MS = 300, MAX = 100
enabled = (showPrBadge || showCiStatus) && !compact

refresh(reason, force=false):
  if !enabled: clear both maps; drop in-flight (reqId++); return
  debounce DEBOUNCE_MS -> runFetch(force)          // coalesces focus+graph-change after a fetch

runFetch(force):
  reqId = ++seq; layout = graphDataRef.current; if layout==null: return
  ctx = await forgeRepoContext(repoId)             // cheap, no network (P62)
  if superseded(reqId): return
  if ctx.provider != 'gitHub': clear both; return  // non-forge origin ⇒ inert, no error
  try:
    if showPrBadge && (force || stale(prTsMs)):
      page = await forgeListPrs(repoId, { state:'open', page:1, perPage:50 })   // OQ-3: open-only
      if superseded: return
      prByBranch = map(page.items, pr => [pr.sourceBranch, {number,title,state,isDraft,url}]) // last wins
      prTsMs = now
    if showCiStatus:
      tips = distinct(node.id for node in layout.nodes if node.refs has local/remoteBranch)  // branch tips
      shas = (tips ∪ openPRheadShas) filter (force || !fresh(ciBySha, TTL))  ; cap MAX ; chunk MAX
      if shas nonempty:
        for chunk: res = await forgeCommitStatuses(repoId, chunk); if superseded: return
                   merge res into ciBySha (tsMs=now) as CiBadge{rollup:state, passed,failed,pending,total}
    setState(new map identities)                    // -> graphDisplay memo -> new display -> repaint
  catch e: log(e)                                   // SILENT; keep stale maps

// RepoWorkspace triggers:
//   fetch/pull success (wrap handleFetch/handlePull at their onFetch/onPull call sites) -> refresh('remote', force=true)
//   onWindowFocus (existing effect)                                                     -> refresh('focus')            // TTL-guarded
//   manual Refresh (refreshAll)                                                         -> refresh('manual', force=true)
//   layout identity change (post fetch/pull/branch-op new graph)                        -> refresh('graph')            // TTL-guarded; covers new tips
//   enabled flips true                                                                  -> refresh('enable')
```
Consumption: `graphDisplay` useMemo lists `prByBranch`,`ciBySha` in its deps ⇒ a completed fetch makes
a NEW `display` identity ⇒ GraphCanvas repaints (deps already include `display`) ⇒ badges fill in.
This is the verify fill-in mechanism, so paint is never blocked and stale rows stay drawn.

## 8. Association rules

- **PR ↔ pill:** keyed by branch SHORT name (`prByBranch.get(entity.name)`, same short-name rule as
  `groupRefs`). Decorates the pill even when the exact head SHA is not fetched. Cross-fork PRs (head in
  a fork) won't match a local name — accepted v1 limitation (OQ-2). Diverged local/remote "main" (two
  entities, two rows) both show the same PR badge — harmless.
- **CI ↔ pill:** keyed by the tip commit `ciBySha.get(node.id)` — precise per-row (two branches at one
  commit share CI, correctly).
- **Scope:** branch entities only (`entity.kind==='branch'`). Tags / detached HEAD / stash get no
  forge badge. Non-tip commits get no CI (rate limits + matches GitKraken).

## 9. Mock + fixtures parity (`?forge=*` — offline harness)

- `src/ipc/mock/handlers/forge.ts`: add `forgeCommitStatuses(repoId, shas)` — `?forge=off` ⇒ throw
  `{kind:'networkError'}`; else map each sha via `commitStatusFor(sha)`, dropping unknowns (best-effort
  parity with the batch contract). `satisfies Partial<IpcApi>`.
- `src/ipc/fixtures/forge.ts`: add `commitStatusFor(sha)` covering all rollups (≥1 each of
  success / failure / pending / none) across the mock `GraphLayout`'s branch-tip shas AND the fixture
  PRs' head shas. **Fixture coordination (REQUIRED):** the P62 fixture PRs' `sourceBranch` values MUST
  equal branch names present in the mock `GraphLayout` refs, or no PR badge can render in the harness.
- No new sentinel required (reuse `?forge=auth` for a warm authenticated state); optionally add
  `?forge=signals` seeding `mockUiSettings.graph.showPrBadge/showCiStatus=true` for a one-shot visual.

## 10. Acceptance criteria

**AI-gate (orchestrator verifies alone):**
- `tsc` + `pnpm build` green. `cargo build` + `cargo clippy -D warnings` green (new command + settings
  fields).
- vitest — `forgeBadges.test.ts`: `prBadgeVisual` per state (open/draft/merged/closed), `ciBadgeVisual`
  per rollup incl. `none→null`, `branchSignals` gate (off / non-branch / present / compact-suppressed),
  width helpers. `refLabels.test.ts`: a branch pill with PR+CI reserves the extra advance and pops into
  "+n" before overlapping; disabled toggles reserve zero.
- Rust — `settings.rs` off/on round-trip asserts the two new fields; `forge_commit_statuses` registered
  in `generate_handler!` (recount + fix TODO.md).
- mock↔real parity: `forgeCommitStatuses` present in BOTH `tauri.ts` and `handlers/forge.ts`.
- Harness (`pnpm dev:mock`, `?forge=auth`): enable **PR badge** + **CI status** in Settings → the graph
  shows `#num` pills (state-colored) + CI dots on the matching branch-tip pills; both toggles PERSIST
  across reload; enabling **Compact** hides both; clicking a PR badge switches the right pane to
  "Pull requests" and opens that PR's detail; `?forge=off` ⇒ badges silently absent, no crash/toast.
  Screenshot the graph with badges as the single visual proof.

**USER CHECKPOINT (native — orchestrator must NOT self-pass):**
- Against a REAL GitHub account (`pnpm tauri dev`, PAT connected via P62): enabling the toggles shows
  real open-PR badges on the correct branch tips with correct state colors, and real CI status
  (green/red/amber) matching GitHub; clicking a PR badge opens the PR panel to that PR; after a fetch
  (and on window-focus return) badges refresh; Compact hides them; a non-GitHub origin shows no badges
  and no errors; a revoked token degrades silently (badges vanish, PR panel still reports the auth
  error on its own actions).

## 11. Open decisions (REC baked in; do NOT block)

- **OQ-1 CI command shape.** REC: batch `forge_commit_statuses(shas)` (one round-trip, mirrors
  `verifyCommits`). Alt: single `forge_commit_status(sha)` (overview's phrasing) — delta +1 either way.
- **OQ-2 PR↔pill match.** REC: branch NAME (`sourceBranch`) — decorates the pill without needing the
  head SHA present. Alt: head SHA (exact but needs the commit fetched). Cross-fork PRs unmatched in v1.
- **OQ-3 PR states fetched.** REC: OPEN only (bounded, actionable; merged/closed branches are usually
  deleted). Badge model already supports all 4 — lighting merged/closed later = feed them into
  `prByBranch` (would need `state:'all'` + latest-per-branch; heavier/unbounded).
- **OQ-4 Placement.** REC: LEFT ref band, riding the branch pill (per-branch semantics, reuses the
  chip/overflow machinery). Alt: a right "signals column" (mostly empty, wastes width) — rejected.
- **OQ-5 Compact.** REC: HIDE both badges in compact (compact = min-clutter/max-density). Alt: shrink.
- **OQ-6 PrPanel nav seam (cross-milestone).** P63 needs an `openToPr?: PrNavRequest | null` input on
  P62's `PrPanel`. FLAG: ensure P62 ships (or is amended to accept) this prop; if P62 is already built,
  this is a small additive P63 edit to `PrPanel.tsx`. REC: `{ number, seq }` (seq re-triggers).
- **OQ-7 Fetch-failure UX.** REC: SILENT for badges (decoration) — log, keep stale, no toast. The PR
  panel keeps toasting its own explicit actions.
