// Split out of the former monolithic mock.ts (pure refactor; no behavior change).
import { MOCK_ASSET_CONTENT, mockAgentAssets, mockInventory, mockProfiles } from '../fixtures/aiAssets';
import { INITIAL_BRANCHES, MOCK_OID } from '../fixtures/branches';
import { makeMockConfigStore } from '../fixtures/config';
import type { MockConfigStore, MockIdentityFixture } from '../fixtures/config';
import { initialMainRs } from '../fixtures/diffs';
import type { ThreeWay } from '../fixtures/diffs';
import type { MockCommit } from '../fixtures/graph';
import { mockHash } from '../fixtures/profiles';
import { STALE_SEED } from '../fixtures/staleBranches';
import { seedStashes } from '../fixtures/stashes';
import { INITIAL_STATUS } from '../fixtures/status';
import { seedSubmodules } from '../fixtures/submodules';
import { seedOpState } from './opStateSeed';
import { worktreesFor } from './worktreeState';
import type { AgentAsset, AiAssetInventory, AppError, BranchesSnapshot, ConflictEntry, ConflictFile, ProfileStore, RemoteInfo, RepoInfo, RepoOpState, StaleBranch, StaleReport, StashEntry, StatusSnapshot, SubmoduleInfo } from '../types';

export const MOCK_REPO_PATH = 'C:\\mock\\bonsai-fixture';

export function delay(ms = 150): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}
// P20 §8.4 demo trigger: a cherry-pick / revert of a commit whose oid ends in
// this suffix pauses with a conflict (mirrors the mergeBranch `name.includes
// ('conflict')` convention, keyed on oid). Any other oid commits cleanly.
export const PICK_REVERT_CONFLICT_OID_SUFFIX = 'c0ffee';

// P47 demo trigger: a cherry-pick / revert of a commit whose oid ends in this
// suffix commits cleanly but conflicts when the autostash is re-applied, so the
// harness can exercise the `stashPopConflicts` outcome (stash retained).
export const STASH_POP_CONFLICT_OID_SUFFIX = 'deadbe';

// P23b §7.2: interactive-rebase conflict demo. A Start pauses on a conflict when
// EITHER `?rebase=conflict` is set (seeded per repo below) OR one of the plan's
// replayed oids ends in this suffix (an explicit fixture marker) — mirroring the
// cherry-pick/revert oid-suffix convention. The pause reuses the merge-conflict
// fixture (src/auth.ts) and drives the EXISTING OpBanner rebase branch.
export const INTERACTIVE_REBASE_CONFLICT_OID_SUFFIX = PICK_REVERT_CONFLICT_OID_SUFFIX;

/** P23b: pending interactive-rebase plan retained while paused on a conflict, so
 *  the reused rebaseContinue/Skip/Abort can finish/restore the rewritten commits.
 *  `rewritten` is newest-first (ready to unshift onto the mock graph). */
export interface InteractivePlan {
  headName: string;
  ontoOid: string;
  rewritten: MockCommit[];
  /** oids of the ORIGINAL range commits (base..old-HEAD) — removed from the mock
   *  commit list on finish so the rewritten commits truly REPLACE them. */
  originalOids: string[];
  totalSteps: number;
}

/** P39: stateful git-bisect mock. `chain` is a synthetic linear candidate list
 *  oldest→newest (index 0 = the seeded good bound, last = the seeded bad bound).
 *  `lo`/`hi` are the moving known-good / known-bad boundary indices; the testable
 *  window is the strictly-between indices minus `skipped`. When the window
 *  collapses the bad bound (`chain[hi]`) is the first-bad commit. */
export interface MockBisect {
  chain: string[];
  lo: number;
  hi: number;
  /** chain index under test, or null in a terminal (found / cannotDetermine) phase. */
  current: number | null;
  /** skipped candidate oids. */
  skipped: string[];
  firstBad: string | null;
}
/** Mutate `state.inventory` so `assetId`'s mapped file now holds `content`
 *  (exists:true, hashes derived from the content). Drift recomputes from these
 *  hashes, giving the browser harness the full drift→activate→in-sync loop. */
export function applyMockWrite(state: MockRepoState, assetId: string, path: string, content: string): void {
  state.assetContent[path] = content;
  const asset = state.inventory.assets.find((a) => a.id === assetId);
  if (asset) {
    asset.exists = true;
    asset.files = [
      {
        path,
        size: content.length,
        contentHash: mockHash(content),
        normalizedHash: mockHash(content),
        modified: 1_753_900_000,
      },
    ];
  }
}

/** How a repo's HEAD / graph are shaped (seeded once at open). */
export type RepoKind = 'default' | 'detached' | 'unborn';
export type GraphFixture = 'default' | '20k' | 'detached';

export interface MockRepoState {
  /** Path exactly as passed to openRepo (what the UI shows); repoId is the key. */
  path: string;
  kind: RepoKind;
  /** Which fixture getGraph/getCommitDiff serve for this repo. */
  graphFixture: GraphFixture;
  /** P40: stateful per-level git-config store (Local | Global). commit()
   *  consults it for identity so the identity-gap fix demos end-to-end. The
   *  `?fixture=noconfig` intent lives in how this is seeded (identity dropped). */
  config: MockConfigStore;
  /** `?remote=` failure trigger (authfail | network | rejected | conflict). */
  remoteTrigger: string | null;
  /** P59a: `?hooks=fail` — a blocking pre-commit hook rejects commit/amend/merge
   *  (unless skipHooks or `bonsai.runHooks` is false). A `#hookfail` message
   *  sentinel triggers it per-call regardless of this flag. */
  hooksFail: boolean;
  /** P59a-2: `?hooks=failpush` — a blocking pre-push hook rejects push/forcePush
   *  (unless skipHooks or `bonsai.runHooks` is false). Drives the push-side
   *  HookOutputDialog + "Push anyway (skip hooks)" retry in the harness. */
  hooksFailPush: boolean;

  status: StatusSnapshot;
  headOid: string;
  branches: BranchesSnapshot;
  headBranch: string;
  fetched: boolean;
  commits: MockCommit[];
  opState: RepoOpState;
  conflicts: ConflictEntry[];
  conflictTexts: Map<string, ConflictFile>;
  /** Stash stack, index 0 (most recent) first (P9 §6.5). */
  stashes: StashEntry[];
  /** Submodules with classified status (P19 §5). Default repo only. */
  submodules: SubmoduleInfo[];
  /** Configured remotes (P22 §5.3). Seeded with `origin` for every usable repo. */
  remotes: RemoteInfo[];
  /** P17: the one live three-way (head/index/workdir) model file, `src/main.rs`.
   *  Partial stage/unstage mutate `index`; getStatus/getWorkdirFileDiff derive
   *  the file's section membership + diffs from it. */
  mainRs: ThreeWay;
  /** P23b: `?rebase=conflict` — startInteractiveRebase pauses on a conflict. */
  interactiveConflictDemo: boolean;
  /** P23b: pending interactive-rebase plan while paused on a conflict; null when
   *  no interactive rebase is mid-flight. The reused rebaseContinue/Skip/Abort
   *  key on this to finish (prepend the rewritten commits) or restore. */
  interactive: InteractivePlan | null;
  /** P39: in-progress git-bisect state; null when no bisect is active. */
  bisect: MockBisect | null;
  /** P24: AI-asset inventory (drift recomputed per `listAiAssets` call). */
  inventory: AiAssetInventory;
  /** P24b: per-repo mapped-file content (seeded from the canned bodies), mutated
   *  by `activateProfile` so preview/read/drift reflect writes. */
  assetContent: Record<string, string>;
  /** P24b: the context-profile store (stateful CRUD + activation). */
  profiles: ProfileStore;
  /** P26: managed agent assets (skills / subagents / slash commands). Stateful
   *  so P26b's create/edit/delete round-trips in the harness. */
  agentAssets: AgentAsset[];
}
export const repos = new Map<string /* repoId */, MockRepoState>();

export function query(name: string): string | null {
  return new URLSearchParams(window.location.search).get(name);
}

// P13: `?ai=off` simulates "no claude on PATH" — read once at module init,
// composable with `?op=merge` / `?fixture=`. Availability probes honour it;
// aiResolveConflict is independent (it gates on the conflict kind, not this).
export const AI_OFF = query('ai') === 'off';
/** Strips Git conflict markers, keeping BOTH sides' body lines — a plausible
 *  markerless merged body for the mock AI proposal (P13, derived from the
 *  fixture's `conflictTexts[path].text`). */
export function stripConflictMarkers(text: string): string {
  return text
    .split('\n')
    .filter((line) => {
      const t = line.trimStart();
      return !(t.startsWith('<<<<<<<') || t.startsWith('=======') || t.startsWith('>>>>>>>'));
    })
    .join('\n');
}

/**
 * Mock stand-in for the backend's `read_repo_info` workdir canonicalization:
 * normalize separators + strip a trailing slash, preserving case (matches the
 * real backend, whose ONLY case-insensitive step is the dedupe scan below).
 */
export function mockCanonical(path: string): string {
  return path.replace(/[\\/]+/g, '/').replace(/\/+$/, '');
}

/**
 * Resolve the repoId for a usable open: reuse an existing key that matches
 * case-insensitively (backend dedupe scan → focus), else the fresh canonical.
 */
export function resolveRepoId(path: string): string {
  const canonical = mockCanonical(path);
  for (const key of repos.keys()) {
    if (key.toLowerCase() === canonical.toLowerCase()) return key;
  }
  return canonical;
}

/**
 * Per-repo seeding for distinct tabs. Query params (`?op=merge`, `?op=rebase`,
 * `?fixture=detached|20k|noconfig`, `?remote=…`) seed the DEFAULT repo so
 * single-tab harness flows are unchanged. ADDITIONALLY, a path whose string
 * contains one of these substrings seeds that distinct state so the harness can
 * open multiple tabs with independent states:
 *   'merge'      → paused conflicted merge
 *   'rebase'     → paused conflicted rebase (step 2/3)
 *   'detached'   → detached HEAD + detached graph fixture
 *   'unborn'     → unborn HEAD (usable, empty repo)
 *   'not-a-repo' → non-usable (isRepo:false)   [handled in openRepo, no entry]
 *   'bare'       → non-usable bare repo         [handled in openRepo, no entry]
 * Path substrings win over query params for their own dimension.
 */
export function repoOp(path: string): 'merge' | 'rebase' | null {
  if (path.includes('merge')) return 'merge';
  if (path.includes('rebase')) return 'rebase';
  const q = query('op');
  if (q === 'merge' || q === 'rebase') return q;
  return null;
}

export function repoGraphFixture(path: string): GraphFixture {
  if (path.includes('detached')) return 'detached';
  const q = query('fixture');
  if (q === '20k') return '20k';
  if (q === 'detached') return 'detached';
  return 'default';
}

export function repoKind(path: string, graphFixture: GraphFixture): RepoKind {
  if (path.includes('unborn')) return 'unborn';
  if (graphFixture === 'detached') return 'detached';
  return 'default';
}
/** P69i: which identity seed `?fixture=` asks for (see `MockIdentityFixture`). */
function identityFixture(): MockIdentityFixture {
  const q = query('fixture');
  if (q === 'noconfig') return 'none';
  if (q === 'identitymatch') return 'localMatch';
  return 'global';
}

/** Builds a fresh MockRepoState for a usable repo (default / detached / unborn). */
export function createRepoState(path: string): MockRepoState {
  const graphFixture = repoGraphFixture(path);
  const kind = repoKind(path, graphFixture);
  const state: MockRepoState = {
    path,
    kind,
    graphFixture,
    // Seed a WORKING identity by default; `?fixture=noconfig` drops it so the
    // commit-error / Set-identity flow is demoable (P40 §6.3), and
    // `?fixture=identitymatch` adds a LOCAL one equal to the seeded Work profile
    // (P69i — the only route to identity state 1 and to the §4.5 confirm).
    config: makeMockConfigStore(identityFixture()),
    remoteTrigger: query('remote'),
    hooksFail: query('hooks') === 'fail',
    hooksFailPush: query('hooks') === 'failpush',
    status: structuredClone(INITIAL_STATUS),
    headOid: MOCK_OID,
    branches: structuredClone(INITIAL_BRANCHES),
    headBranch: 'main',
    fetched: false,
    commits: [],
    opState: { kind: 'none' },
    conflicts: [],
    conflictTexts: new Map(),
    stashes: seedStashes(kind, graphFixture),
    submodules: seedSubmodules(kind, graphFixture),
    remotes: [{ name: 'origin', url: 'https://example.com/repo.git' }],
    mainRs: initialMainRs(),
    interactiveConflictDemo: query('rebase') === 'conflict',
    interactive: null,
    bisect: null,
    inventory: structuredClone(mockInventory),
    assetContent: structuredClone(MOCK_ASSET_CONTENT),
    profiles: structuredClone(mockProfiles),
    agentAssets: structuredClone(mockAgentAssets),
  };
  seedOpState(state, repoOp(path));
  // P27 §5 fidelity: opening a linked worktree's path checks out THAT
  // worktree's branch — mirror it by pointing HEAD (and the branches snapshot)
  // at the row's branch. Skipped for the stale row (branch === null).
  const wtRow = worktreesFor(state).find((w) => !w.isMain && w.absPath === path);
  if (wtRow !== undefined && wtRow.branch !== null) {
    state.headBranch = wtRow.branch;
    state.headOid = wtRow.headOid ?? MOCK_OID;
    for (const b of state.branches.local) b.isHead = false;
    const local = state.branches.local.find((b) => b.name === wtRow.branch);
    if (local !== undefined) {
      local.isHead = true;
    } else {
      state.branches.local.push({
        name: wtRow.branch,
        isHead: true,
        upstream: null,
        ahead: null,
        behind: null,
        tip: wtRow.headOid ?? MOCK_OID,
      });
    }
  }
  return state;
}

/** Fresh RepoInfo reflecting the repo's current HEAD (follows checkouts/commits). */
export function buildInfo(state: MockRepoState, path: string): RepoInfo {
  if (state.kind === 'unborn') {
    return {
      path,
      isRepo: true,
      bare: false,
      head: { branchName: 'main', oid: '', detached: false, unborn: true },
    };
  }
  if (state.kind === 'detached') {
    return {
      path,
      isRepo: true,
      bare: false,
      head: { branchName: null, oid: state.headOid, detached: true, unborn: false },
    };
  }
  return {
    path,
    isRepo: true,
    bare: false,
    head: { branchName: state.headBranch, oid: state.headOid, detached: false, unborn: false },
  };
}

/** Looks up an open repo or throws the backend's NoRepo error shape. */
export function requireRepo(repoId: string): MockRepoState {
  const state = repos.get(repoId);
  if (state === undefined) {
    const err: AppError = { kind: 'noRepo', message: 'mock: repository is not open' };
    throw err;
  }
  return state;
}

/**
 * True when `oid` matches a known ref tip: HEAD, a local branch tip, or a
 * remote-tracking branch tip. Tags are `string[]` (no oids) in the mock, so
 * they are not considered here. Lets diff/compare mocks treat ref-pill tips as
 * resolvable commits, mirroring the real backend, instead of throwing.
 */
export function isRefTip(state: MockRepoState, oid: string): boolean {
  return (
    state.branches.head.oid === oid ||
    state.branches.local.some((b) => b.tip === oid) ||
    state.branches.remote.some((r) => r.tip === oid)
  );
}
export function throwAuthFailed(): never {
  const err: AppError = {
    kind: 'authFailed',
    message:
      "authentication failed for 'origin': no usable credentials. Configure a Git " +
      'credential helper (e.g. Git Credential Manager) for HTTPS remotes, or run an ' +
      'SSH agent for SSH remotes.',
  };
  throw err;
}

export function throwNetworkError(): never {
  const err: AppError = {
    kind: 'networkError',
    message: "network error talking to 'origin': failed to resolve address",
  };
  throw err;
}

/**
 * Documented simplification of the git ref-format rules — the backend
 * (`git2::Branch::name_is_valid` + pre-checks) is authoritative.
 */
export function isInvalidBranchName(name: string): boolean {
  const trimmed = name.trim();
  return (
    trimmed === '' ||
    /\s/.test(trimmed) ||
    trimmed.includes('..') ||
    /[~^:?*[\\]/.test(trimmed) ||
    trimmed.includes('@{') ||
    trimmed.startsWith('-') ||
    trimmed.startsWith('/') ||
    trimmed.endsWith('/') ||
    trimmed.endsWith('.lock')
  );
}
/**
 * Recomputes the stale report from the live `state.branches.local`, mirroring the
 * server rules: base = 'main', excludes the base and the current HEAD branch, and
 * only surfaces branches still present that are classified in `STALE_SEED`. A
 * prior `deleteBranches` that removed a local shrinks the report naturally.
 */
export function buildStaleReport(state: MockRepoState): StaleReport {
  const currentName = state.kind === 'detached' ? null : state.headBranch;
  const branches: StaleBranch[] = state.branches.local
    .filter((b) => b.name !== 'main' && b.name !== currentName && b.name in STALE_SEED)
    .map((b) => {
      const seed = STALE_SEED[b.name];
      return {
        name: b.name,
        tip: b.tip,
        lastCommitSummary: `work on ${b.name}`,
        lastCommitAuthor: 'Ada Lovelace',
        lastCommitTime: 1_720_000_000,
        reason: seed.reason,
        merged: seed.merged,
        goneUpstream: seed.goneUpstream,
        upstream: b.upstream,
        ahead: seed.merged ? 0 : (b.ahead ?? 4),
        behind: b.behind ?? 1,
        isCurrent: false,
      };
    })
    .sort((a, b) => a.name.toLowerCase().localeCompare(b.name.toLowerCase()));
  return { base: 'main', baseOid: state.headOid, branches };
}
