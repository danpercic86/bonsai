# P90 — Per-branch CI Checks view

Right-panel tab showing detailed CI check contexts for the branch selected in the
left sidebar. Per-branch, updates on sidebar branch selection, refreshes on
fetch/pull/push. **Reuses existing forge IPC — no new command.**

## Scope decisions (locked unless flagged)

- **No new Tauri command.** Reuse `forgeCommitStatuses(repoId, [tipSha]) ->
  CommitStatus[]` (already returns full `contexts`, capped at 50) +
  `forgeRepoContext(repoId)` for provider detection. Both already have mock handlers.
- **Own fetch, not the badge cache.** `useForgeSignals` keeps only the `CiBadge`
  summary (rollup+counts), discarding `contexts`. The Checks view needs the full
  `StatusContext[]`, so it issues its own single-sha `forgeCommitStatuses` call.
  This is one commit per branch-selection — cheap, no round-trip storm.
- **Rust owns all forge logic**; this milestone is frontend-only wiring over the
  existing blocking (`spawn_blocking`-wrapped) provider commands, UNLESS the timing
  fields in OQ-1 are accepted.

## OQ-1 — timing fields on `StatusContext` (FLAG FOR USER)

The user asked for "individual check name, state, description, link". All four
already exist in `StatusContext { name, state, description, target_url }`. The user
did NOT explicitly ask for duration/timestamps. Adding `started_at`/`completed_at`
(or a computed `duration_secs`) is **parseable in all 4 providers but not free**:

| Provider  | Wire source for timing                                  |
|-----------|--------------------------------------------------------|
| GitHub    | check-runs `started_at`/`completed_at`; legacy statuses `created_at`/`updated_at` (weaker) |
| GitLab    | pipeline job `started_at`/`finished_at`                 |
| Azure     | commit-status `createdDate`/`updatedDate`               |
| Bitbucket | build-status `created_on`/`updated_on`                  |

Cost: 2 new optional fields on the shared type + TS type + mock fixtures, plus wire
DTO fields and mapping + tests in **4 dto modules**. Legacy statuses give only
coarse timestamps, so the field is inconsistently populated across providers.

**Recommendation: DEFER timing to a P90.1 follow-up.** Ship v1 with the four
existing fields, which fully satisfy the stated requirement. If the user wants
duration now, add `Option<i64>` unix-secs `startedAt`/`completedAt` to
`StatusContext` (nullable everywhere; render "—" when absent). Do not block P90 on it.

## Data flow

```
sidebar branch/remote row click
  -> existing onReveal(RevealTarget{kind:'ref', name})   (already wired)
  -> RepoWorkspace: resolveChecksTarget(target, branchesSnapshot)
       -> ChecksTarget { name, tip, hasUpstream } | null   (local-only / tag => null)
  -> setChecksTarget(...)   (state; does NOT force-switch the active tab)
  -> useBranchChecks(repoId, checksTarget, refreshSeq) effect:
       forgeRepoContext -> if 'unknown' => state 'noForge'
       else forgeCommitStatuses(repoId, [tip]) -> CommitStatus | 'noChecks' | 'error'
```

Selection updates `checksTarget` even when the Checks tab is inactive, so switching
to it shows the latest branch. It does NOT auto-switch tabs (matches PR-tab behavior).

### Refresh trigger on fetch/pull/push

Add a monotonic `forgeChecksRefreshSeq: number` counter in RepoWorkspace, bumped at
**every existing site** that already calls `forgeSignals.refresh('remote', true)`
(the post fetch/pull/push refresh path in `RepoWorkspace.tsx`). Thread the counter
into `useBranchChecks` as a dep; a bump re-runs the effect (force refetch, TTL
bypassed — the view is a user-facing action surface, not decoration). Reuse the
same trigger points; do NOT add a new event.

## Rust surface

No change if OQ-1 deferred (recommended). If accepted, only:

```rust
// crates/bonsai-forge/src/types.rs — StatusContext gains:
pub started_at: Option<i64>,   // unix secs, None when provider omits
pub completed_at: Option<i64>, // unix secs, None when provider omits
```
plus mapping in each `{github,gitlab,azure,bitbucket}/dto.rs` build-context path.

## TypeScript surface

### New shared helper — `src/components/checksPanel/checksTarget.ts` (PURE, unit-tested)

```ts
import type { RevealTarget } from '../../graph/reveal';
import type { BranchesSnapshot } from '../../ipc';

export interface ChecksTarget {
  /** Display name: "main" | "origin/main". */
  name: string;
  /** Full 40-hex tip oid to query forge status for. */
  tip: string;
  /** False for local branches with no configured upstream (drives a hint). */
  hasUpstream: boolean;
}

/** Resolve a sidebar reveal click to a checks target, or null when the click is
 *  not a branch (tag / stash / oid) or the ref is not in the snapshot. */
export function resolveChecksTarget(
  target: RevealTarget,
  branches: BranchesSnapshot | null,
): ChecksTarget | null;
```
Rules: `kind:'oid'` => null. `kind:'ref'`: match `branches.local` by `name`
(hasUpstream = `upstream !== null`, tip from `BranchInfo.tip`); else match
`branches.remote` by `name` (hasUpstream = true, tip from `RemoteBranchInfo.tip`);
tag names (not in either list) => null.

### Container hook — `src/components/checksPanel/useBranchChecks.ts`

```ts
export type ChecksState =
  | { kind: 'idle' }                                   // no branch selected
  | { kind: 'loading'; target: ChecksTarget }
  | { kind: 'noForge'; target: ChecksTarget }          // provider === 'unknown'
  | { kind: 'noChecks'; target: ChecksTarget }         // fetched, contexts empty
  | { kind: 'error'; target: ChecksTarget; message: string }
  | { kind: 'loaded'; target: ChecksTarget; status: CommitStatus };

export function useBranchChecks(deps: {
  repoId: string;
  target: ChecksTarget | null;
  /** Bumped on fetch/pull/push to force a refetch. */
  refreshSeq: number;
  /** Only fetch while the Checks tab is active (avoid work for a hidden panel). */
  active: boolean;
}): { state: ChecksState; refresh(): void };
```
Behavior: last-wins `reqId` guard + 300 ms debounce (mirror `useForgeSignals`).
`target === null` => `idle`. Effect deps: `repoId`, `target?.tip`, `refreshSeq`,
`active`. `noChecks` when the returned `CommitStatus.contexts.length === 0`
(regardless of `total`, since the view lists contexts). Errors are surfaced (this
is a user surface, unlike the silent badge cache) — one inline error state, no toast.
`hasUpstream === false` is a soft hint rendered on `noChecks`, not a separate state.

### Right-panel tab — `WorkspaceRightPanel.tsx`

```ts
rightPaneTab: 'work' | 'prs' | 'checks';        // widen existing union
onSelectRightPaneTab(tab: 'work' | 'prs' | 'checks'): void;
```
Add a third `role="tab"` button (label owned by ui-designer). Render `<ChecksPanel>`
under `rightPaneTab === 'checks'`. RepoWorkspace `useState<'work'|'prs'|'checks'>`.
Thread `repoId`, `checksTarget`, `checksRefreshSeq` down; construct `active =
rightPaneTab === 'checks'`.

## Component boundary (each file < ~200 lines)

- `src/components/checksPanel/ChecksPanel.tsx` — container. Calls `useBranchChecks`,
  switches on `state.kind`, composes the presentational children. Owns nothing else.
- `src/components/checksPanel/useBranchChecks.ts` — the state/effect/IPC hook (above).
- `src/components/checksPanel/checksTarget.ts` — pure resolver (above).
- `src/components/checksPanel/ChecksSummary.tsx` — header: branch name, rollup pill,
  passed/failed/pending counts (from `CommitStatus`), short tip oid, manual Refresh
  button (calls `refresh()`).
- `src/components/checksPanel/ChecksList.tsx` — maps `contexts` to rows.
- `src/components/checksPanel/ChecksListItem.tsx` — one context: state icon, `name`,
  `description`, external link when `target_url !== null` (opens via existing external
  open path; no in-app nav).
- `src/components/checksPanel/ChecksEmptyState.tsx` — renders idle / noForge /
  noChecks / error variants (ui-designer styles copy). `loading` = a skeleton here or
  in ChecksPanel.

## Mock IPC

No new handler. `src/ipc/mock/handlers/forge.ts` `forgeCommitStatuses` already
serves fixtures from `src/ipc/fixtures/forge.ts`. **Add fixture coverage** so the
browser harness exercises every state: a tip sha with mixed contexts (pass+fail+
pending, some with `description` + `target_url`, some null), a tip sha returning an
empty `contexts` (noChecks), and ensure at least one sidebar branch tip in the mock
branches snapshot maps to each. `forgeRepoContext` mock already returns a known
provider; a noForge fixture toggle is optional (state reachable via unit test).

## States to handle (state machine = `ChecksState` above)

| State    | Trigger                                            |
|----------|----------------------------------------------------|
| idle     | no branch selected yet                             |
| loading  | fetch in flight for the current target             |
| noForge  | `forgeRepoContext.provider === 'unknown'`          |
| noChecks | fetch ok, `contexts.length === 0` (+ upstream hint) |
| error    | `forgeCommitStatuses` rejects                      |
| loaded   | fetch ok, `contexts.length > 0`                    |

## Acceptance criteria

1. Selecting a local or remote branch in the sidebar makes the Checks tab show that
   branch's checks (name, state, description, link) for its tip commit.
2. Selecting a different branch re-queries and replaces the view; selecting a tag or
   stash leaves the last branch (or `idle`) — no crash.
3. A fetch/pull/push refreshes the currently-shown branch to latest status (force,
   TTL bypassed).
4. All six states render without error and are reachable in the mock harness.
5. External check links open externally; rows without `target_url` show no link.
6. `resolveChecksTarget` + the `noChecks`/state-selection logic are unit-tested (pure).
7. `WorkspaceRightPanel.test.tsx` updated for the widened tab union; existing tests green.
8. No new Tauri command; `tsc`, `cargo check`, `pnpm gate` clean.

### AI gate (orchestrator verifies)
- Vitest units for `checksTarget` + `useBranchChecks` state transitions.
- `tsc` + lint + `cargo check` clean; `WorkspaceRightPanel.test.tsx` green.
- Browser harness (`VITE_MOCK_IPC=1`): click a mock branch, screenshot each state
  (loaded/mixed, noChecks, error), confirm tab switches and rows render.

### USER CHECKPOINT (native `pnpm tauri dev`)
- Against a real repo with a configured forge + real CI: selecting branches shows
  correct per-branch checks; links open the real CI pages; fetch/pull/push refreshes.
- noForge / no-upstream branches read sensibly.

## Open questions for the orchestrator
- **OQ-1 (timing fields):** recommend DEFER; confirm with user before senior-dev.
- **OQ-2:** should selecting a branch auto-switch to the Checks tab? Recommend NO
  (matches PR tab; less surprising). ui-designer to confirm placement/label/naming.
