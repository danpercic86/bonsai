import {
  CHEAP_TERSE_BODY,
  MOCK_ASSET_CONTENT,
  OPUS_RICH_BODY,
  mockAgentAssets,
  mockInventory,
  mockProfiles,
} from './fixtures/aiAssets';
import { INITIAL_BRANCHES, MOCK_OID } from './fixtures/branches';
import {
  MERGE_AUTH_OURS,
  MERGE_AUTH_TEXT,
  MERGE_AUTH_THEIRS,
  MERGE_README_TEXT,
} from './fixtures/conflicts';
import { MOCK_BRANCH_REFLOGS, MOCK_HEAD_REFLOG } from './fixtures/reflog';
import {
  buildConfigView,
  hasIdentity,
  makeMockConfigStore,
  validateEnumOrThrow,
  validateKeyOrThrow,
  type MockConfigStore,
} from './fixtures/config';
import { mockRepoHealth } from './fixtures/repoHealth';
import {
  asFullContext,
  initialMainRs,
  lineDiff,
  mockCommitDiff,
  mockCommitFileDiff,
  mockCompareDiff,
  mockWorkdirDiff,
  reconstructLines,
} from './fixtures/diffs';
import type { ThreeWay } from './fixtures/diffs';
import {
  buildMockGraph,
  buildMockGraphDetached,
  prependCommits,
  withStashNodes,
} from './fixtures/graph';
import type { MockCommit } from './fixtures/graph';
import { generateLayout20k } from './fixtures/graph20k';
import type {
  AgentAsset,
  AgentAssetInput,
  AgentAssetInventory,
  AgentAssetKind,
  AiAnalysis,
  AiAnalysisMode,
  AiAssetInventory,
  AiAutonomy,
  AiAvailability,
  AiDiffTarget,
  AiDigestRange,
  AiGeneratedAsset,
  AiResolveProposal,
  AiSummary,
  AppError,
  ApplyStashOutcome,
  AssetContent,
  AssetIssue,
  FrontmatterField,
  Validation,
  ContextProfile,
  ProfileActivation,
  ProfilePreviewEntry,
  ProfileStore,
  TargetWriteAction,
  TargetWriteResult,
  DriftReport,
  BlameLine,
  BranchesSnapshot,
  CherrypickOutcome,
  CloneProgress,
  CommitDiff,
  ConfigLevelArg,
  ConfigView,
  IdentityProfile,
  BisectOutcome,
  CommitMessageProposal,
  CommitResult,
  CompareDiff,
  ConflictEntry,
  ConflictFile,
  ConflictResolution,
  CheckoutResult,
  CreateBranchHereResult,
  CreateStashResult,
  StashScope,
  FetchResult,
  FileDiff,
  FileHistoryEntry,
  AutoFetchSettings,
  GraphLayout,
  GraphPrefs,
  HealthRefreshSettings,
  IpcApi,
  JobKind,
  JobStatus,
  JobStatusChangedPayload,
  LineSelection,
  ListView,
  McpStatus,
  MergeOutcome,
  OpenRepoResult,
  PullResult,
  PushResult,
  PaneWidths,
  RebaseOutcome,
  RebaseTodoOp,
  RecentRepo,
  ReflogEntry,
  RemoteInfo,
  RepoChangedPayload,
  RepoHealth,
  RepoInfo,
  RepoOpState,
  ResetMode,
  RevertOutcome,
  SessionState,
  StaleBranch,
  StaleReason,
  StaleReport,
  BranchDeleteResult,
  BranchDeleteStatus,
  StashEntry,
  StatusEntry,
  StatusSnapshot,
  SubmoduleInfo,
  Theme,
  UiSettings,
  UiSettingsPatch,
  Unsubscribe,
  UpdateCheckResult,
  UpdateProgress,
  WorktreeContextStatus,
  WorktreeInfo,
  CopyCandidate,
  CopyPlanEntry,
  CopySelection,
} from './types';
import {
  AUTO_FETCH_INTERVAL_MAX,
  AUTO_FETCH_INTERVAL_MIN,
  AVATAR_RADIUS_MAX,
  AVATAR_RADIUS_MIN,
  DOT_RADIUS_MAX,
  DOT_RADIUS_MIN,
  HEALTH_REFRESH_INTERVAL_MAX,
  HEALTH_REFRESH_INTERVAL_MIN,
  LANE_WIDTH_MAX,
  LANE_WIDTH_MIN,
  ROW_HEIGHT_MAX,
  ROW_HEIGHT_MIN,
} from '../settings/ranges';

const MOCK_REPO_PATH = 'C:\\mock\\bonsai-fixture';

function delay(ms = 150): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

/** Exercises every status render path, incl. a file both staged AND modified. */
const INITIAL_STATUS: StatusSnapshot = {
  staged: [
    { path: 'src/app.rs', origPath: null, status: 'added' },
    // src/main.rs is model-derived (P17 three-way): getStatus appends it to the
    // staged and/or unstaged sections from `state.mainRs`, not this snapshot.
    { path: 'docs/getting-started.md', origPath: 'docs/intro.md', status: 'renamed' },
    { path: 'src/shared/util.rs', origPath: null, status: 'modified' }, // also unstaged below
  ],
  unstaged: [
    { path: 'src/shared/util.rs', origPath: null, status: 'modified' },
    { path: 'README.md', origPath: null, status: 'modified' },
    { path: 'old-config.toml', origPath: null, status: 'deleted' },
    // M4 contract §5: exercise the binary + too-large diff placeholders.
    { path: 'assets/logo.png', origPath: null, status: 'modified' },
    { path: 'data/big-report.csv', origPath: null, status: 'modified' },
  ],
  untracked: [
    { path: 'notes/todo.txt', origPath: null, status: 'untracked' },
    // P3b §3.4: single-child chain — collapses to one "src/git" dir in tree mode.
    { path: 'src/git/status.rs', origPath: null, status: 'untracked' },
    { path: 'scratch.rs', origPath: null, status: 'untracked' },
  ],
  conflicted: [],
};

// P20 §8.4 demo trigger: a cherry-pick / revert of a commit whose oid ends in
// this suffix pauses with a conflict (mirrors the mergeBranch `name.includes
// ('conflict')` convention, keyed on oid). Any other oid commits cleanly.
const PICK_REVERT_CONFLICT_OID_SUFFIX = 'c0ffee';

// P47 demo trigger: a cherry-pick / revert of a commit whose oid ends in this
// suffix commits cleanly but conflicts when the autostash is re-applied, so the
// harness can exercise the `stashPopConflicts` outcome (stash retained).
const STASH_POP_CONFLICT_OID_SUFFIX = 'deadbe';

// P23b §7.2: interactive-rebase conflict demo. A Start pauses on a conflict when
// EITHER `?rebase=conflict` is set (seeded per repo below) OR one of the plan's
// replayed oids ends in this suffix (an explicit fixture marker) — mirroring the
// cherry-pick/revert oid-suffix convention. The pause reuses the merge-conflict
// fixture (src/auth.ts) and drives the EXISTING OpBanner rebase branch.
const INTERACTIVE_REBASE_CONFLICT_OID_SUFFIX = PICK_REVERT_CONFLICT_OID_SUFFIX;

/** P23b: pending interactive-rebase plan retained while paused on a conflict, so
 *  the reused rebaseContinue/Skip/Abort can finish/restore the rewritten commits.
 *  `rewritten` is newest-first (ready to unshift onto the mock graph). */
interface InteractivePlan {
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
interface MockBisect {
  chain: string[];
  lo: number;
  hi: number;
  /** chain index under test, or null in a terminal (found / cannotDetermine) phase. */
  current: number | null;
  /** skipped candidate oids. */
  skipped: string[];
  firstBad: string | null;
}

// ---------------------------------------------------------------------------
// P3e-c: per-repo state. Every stateful flow that used to live in module-level
// singletons now lives inside a MockRepoState, one per open repoId. The map is
// the single source of per-repo truth — there is NO module-level per-repo
// singleton anymore. `openRepo` creates entries lazily; `closeRepo` deletes.
// ---------------------------------------------------------------------------

/** Client-side mirror of the Rust drift algorithm (§4.3) so the `canonical`
 *  override is demonstrable in the browser harness. */
const COMPARABLE_IDS = ['claude', 'agents', 'copilot', 'gemini', 'windsurf', 'cursorLegacy'];
function recomputeDrift(inv: AiAssetInventory, canonical?: string): DriftReport {
  const byId = (id: string) => inv.assets.find((a) => a.id === id);
  const exists = (id: string) => byId(id)?.exists ?? false;
  const nhash = (id: string) => byId(id)?.files[0]?.normalizedHash ?? null;

  let canonicalId: string | null = null;
  if (canonical && COMPARABLE_IDS.includes(canonical) && exists(canonical)) {
    canonicalId = canonical;
  } else {
    // Priority == table order for the comparable set.
    canonicalId = COMPARABLE_IDS.find((id) => exists(id)) ?? null;
  }
  const canonicalHash = canonicalId ? nhash(canonicalId) : null;

  const entries = COMPARABLE_IDS.map((id) => {
    const ex = exists(id);
    const normalizedHash = ex ? nhash(id) : null;
    const inSync = ex && canonicalHash !== null && normalizedHash === canonicalHash;
    return { assetId: id, exists: ex, comparable: true, normalizedHash, inSync };
  });
  const inSync = entries.filter((e) => e.exists).every((e) => e.inSync);
  return { canonicalId, canonicalHash, entries, inSync };
}

// --- P26: agent-asset (skills / subagents / slash commands) fixture ---------

/** Repo-relative path for an agent asset (mirrors Rust `rel_path`, §3.1). */
function agentRelPath(kind: AgentAssetKind, name: string): string {
  switch (kind) {
    case 'skill':
      return `.claude/skills/${name}/SKILL.md`;
    case 'agent':
      return `.claude/agents/${name}.md`;
    case 'command':
      return `.claude/commands/${name}.md`;
  }
}

/** Deterministic (kind order skill<agent<command, then name) sort, matching
 *  `scan_agent_assets`. */
const AGENT_KIND_ORD: Record<AgentAssetKind, number> = { skill: 0, agent: 1, command: 2 };
function sortAgentAssets(assets: AgentAsset[]): AgentAsset[] {
  return [...assets].sort(
    (a, b) => AGENT_KIND_ORD[a.kind] - AGENT_KIND_ORD[b.kind] || a.name.localeCompare(b.name),
  );
}

/** Required frontmatter keys per kind (mirrors Rust `required_keys`, §3.1). */
function agentRequiredKeys(kind: AgentAssetKind): string[] {
  return kind === 'agent' ? ['name', 'description'] : [];
}

/** Recompute an asset's `Validation` on save, mirroring Rust `validate` (§4.5):
 *  required-key Errors + lowercase-hyphen / name-mismatch / empty-body Warnings.
 *  `complex` is never reachable — an `AgentAssetInput` is a flat field list. */
function mockValidateAsset(
  kind: AgentAssetKind,
  name: string,
  frontmatter: FrontmatterField[],
  body: string,
): Validation {
  const issues: AssetIssue[] = [];
  for (const key of agentRequiredKeys(kind)) {
    const present = frontmatter.some((f) => f.key === key && f.value.trim() !== '');
    if (!present) {
      issues.push({ severity: 'error', message: `${kind} requires frontmatter field '${key}'` });
    }
  }
  if (!/^[a-z0-9][a-z0-9-]*$/.test(name)) {
    issues.push({
      severity: 'warning',
      message: 'name should be lowercase letters, digits, and hyphens',
    });
  }
  if (kind === 'skill' || kind === 'agent') {
    const nf = frontmatter.find((f) => f.key === 'name');
    if (nf && nf.value !== '' && nf.value !== name) {
      issues.push({
        severity: 'warning',
        message: `frontmatter name '${nf.value}' differs from the file name '${name}'`,
      });
    }
  }
  if ((kind === 'skill' || kind === 'command') && body.trim() === '') {
    issues.push({ severity: 'warning', message: 'body is empty — nothing will run' });
  }
  const valid = !issues.some((i) => i.severity === 'error');
  return { valid, issues };
}

/** Windows reserved device names (case-insensitive), matched on the base name
 *  before the first `.` — mirrors Rust `WINDOWS_RESERVED_NAMES`. */
const WINDOWS_RESERVED_NAMES = new Set([
  'con', 'prn', 'aux', 'nul',
  'com1', 'com2', 'com3', 'com4', 'com5', 'com6', 'com7', 'com8', 'com9',
  'lpt1', 'lpt2', 'lpt3', 'lpt4', 'lpt5', 'lpt6', 'lpt7', 'lpt8', 'lpt9',
]);

/** Name safety mirror of Rust `validate_asset_name` (§4.4); throws `invalidName`. */
function requireValidAssetName(name: string): void {
  const base = name.split('.')[0];
  const bad =
    name.trim() === '' ||
    name === '.' ||
    name === '..' ||
    name.startsWith('-') ||
    name.includes('/') ||
    name.includes('\\') ||
    name.includes(':') ||
    [...name].some((c) => {
      const code = c.charCodeAt(0);
      return code < 0x20 || (code >= 0x7f && code <= 0x9f);
    }) ||
    [...name].some((c) => !/[A-Za-z0-9._-]/.test(c)) ||
    WINDOWS_RESERVED_NAMES.has(base.toLowerCase()) ||
    name.endsWith('.') ||
    name.endsWith(' ');
  if (bad) {
    const err: AppError = { kind: 'invalidName', message: `invalid asset name: '${name}'` };
    throw err;
  }
}

// --- P24b: profiles fixture + stateful activation helpers -------------------

/** The single-file (profile-target-eligible) descriptor ids → mapped repo path,
 *  mirroring the Rust taxonomy's SingleFile rows. Used to validate targets and
 *  resolve preview/activation paths in the mock. */
const SINGLE_FILE_PATHS: Record<string, string> = {
  claude: 'CLAUDE.md',
  agents: 'AGENTS.md',
  copilot: '.github/copilot-instructions.md',
  gemini: 'GEMINI.md',
  windsurf: '.windsurfrules',
  cursorLegacy: '.cursorrules',
};

/** Deterministic 40-hex mock hash of a string (FNV-1a → repeated to 40 chars).
 *  Not git's SHA-1 — the mock only needs stable equality so drift recomputes
 *  correctly after an activation writes new content. */
function mockHash(content: string): string {
  let h = 0x811c9dc5;
  for (let i = 0; i < content.length; i += 1) {
    h ^= content.charCodeAt(i);
    h = Math.imul(h, 0x01000193) >>> 0;
  }
  const hex = h.toString(16).padStart(8, '0');
  return hex.repeat(5); // 8 * 5 = 40 hex chars
}

/** Mutate `state.inventory` so `assetId`'s mapped file now holds `content`
 *  (exists:true, hashes derived from the content). Drift recomputes from these
 *  hashes, giving the browser harness the full drift→activate→in-sync loop. */
function applyMockWrite(state: MockRepoState, assetId: string, path: string, content: string): void {
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
type RepoKind = 'default' | 'detached' | 'unborn';
type GraphFixture = 'default' | '20k' | 'detached';

interface MockRepoState {
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

const repos = new Map<string /* repoId */, MockRepoState>();

function query(name: string): string | null {
  return new URLSearchParams(window.location.search).get(name);
}

// P13: `?ai=off` simulates "no claude on PATH" — read once at module init,
// composable with `?op=merge` / `?fixture=`. Availability probes honour it;
// aiResolveConflict is independent (it gates on the conflict kind, not this).
const AI_OFF = query('ai') === 'off';

// P42 (INV-2): the update flow's harness seam. `?update=available|none|error`
// read once at module init (mirrors AI_OFF). Default (absent/other) ⇒ 'none'.
const UPDATE_MODE = ((): 'available' | 'none' | 'error' => {
  const q = query('update');
  return q === 'available' || q === 'error' ? q : 'none';
})();
const MOCK_CURRENT_VERSION = '0.1.0';
const MOCK_NEXT_VERSION = '0.2.0';
/** Set true by a successful `checkForUpdate` in `available` mode; gates
 *  `downloadAndInstallUpdate` (mirrors the real `pendingUpdate` handle). */
let mockUpdateReady = false;

/** Strips Git conflict markers, keeping BOTH sides' body lines — a plausible
 *  markerless merged body for the mock AI proposal (P13, derived from the
 *  fixture's `conflictTexts[path].text`). */
function stripConflictMarkers(text: string): string {
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
function mockCanonical(path: string): string {
  return path.replace(/[\\/]+/g, '/').replace(/\/+$/, '');
}

/**
 * Resolve the repoId for a usable open: reuse an existing key that matches
 * case-insensitively (backend dedupe scan → focus), else the fresh canonical.
 */
function resolveRepoId(path: string): string {
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
function repoOp(path: string): 'merge' | 'rebase' | null {
  if (path.includes('merge')) return 'merge';
  if (path.includes('rebase')) return 'rebase';
  const q = query('op');
  if (q === 'merge' || q === 'rebase') return q;
  return null;
}

function repoGraphFixture(path: string): GraphFixture {
  if (path.includes('detached')) return 'detached';
  const q = query('fixture');
  if (q === '20k') return '20k';
  if (q === 'detached') return 'detached';
  return 'default';
}

function repoKind(path: string, graphFixture: GraphFixture): RepoKind {
  if (path.includes('unborn')) return 'unborn';
  if (graphFixture === 'detached') return 'detached';
  return 'default';
}

/** Seeds (or clears) a repo's paused-op state + conflicted status rows. */
function seedOpState(state: MockRepoState, op: 'merge' | 'rebase' | null): void {
  state.conflictTexts = new Map();
  if (op === 'rebase') {
    // Pre-seeded paused conflicted rebase at step 2/3 — the "resolve → continue
    // finishes" demo. `rebaseBranch` is the separate clean-rebase demo path.
    state.opState = {
      kind: 'rebase',
      headName: 'feature/topic',
      onto: '00'.repeat(20), // fixture full oid of the onto tip (base row 0's oid)
      currentStep: 2,
      totalSteps: 3,
    };
    state.conflicts = [
      { path: 'src/auth.ts', kind: 'bothModified', hasBase: true, hasOurs: true, hasTheirs: true },
    ];
    state.conflictTexts.set('src/auth.ts', {
      path: 'src/auth.ts',
      kind: 'bothModified',
      binary: false,
      tooLarge: false,
      missing: false,
      text: MERGE_AUTH_TEXT, // reuse the marker fixture
      ours: MERGE_AUTH_OURS,
      theirs: MERGE_AUTH_THEIRS,
    });
    state.status.conflicted = state.conflicts.map((c) => ({
      path: c.path,
      origPath: null,
      status: 'conflicted',
    }));
    return;
  }
  if (op !== 'merge') {
    state.opState = { kind: 'none' };
    state.conflicts = [];
    return;
  }
  state.opState = {
    kind: 'merge',
    incoming: 'feature/login',
    message: "Merge branch 'feature/login'\n\nConflicts:\n\tsrc/auth.ts\n\tREADME.md",
  };
  // Path-ascending, like the backend's list_conflicts.
  state.conflicts = [
    { path: 'README.md', kind: 'deletedByThem', hasBase: true, hasOurs: true, hasTheirs: false },
    { path: 'src/auth.ts', kind: 'bothModified', hasBase: true, hasOurs: true, hasTheirs: true },
  ];
  state.conflictTexts.set('src/auth.ts', {
    path: 'src/auth.ts',
    kind: 'bothModified',
    binary: false,
    tooLarge: false,
    missing: false,
    text: MERGE_AUTH_TEXT,
    ours: MERGE_AUTH_OURS,
    theirs: MERGE_AUTH_THEIRS,
  });
  // deletedByThem: the worktree keeps OUR version (no markers). ours/theirs are
  // editor-irrelevant for this quick-action kind (§0.5) → '' per §1.4.
  state.conflictTexts.set('README.md', {
    path: 'README.md',
    kind: 'deletedByThem',
    binary: false,
    tooLarge: false,
    missing: false,
    text: MERGE_README_TEXT,
    ours: '',
    theirs: '',
  });
  state.status.conflicted = state.conflicts.map((c) => ({
    path: c.path,
    origPath: null,
    status: 'conflicted',
  }));
  // README.md is conflicted, not plain-modified, while the merge is paused.
  state.status.unstaged = state.status.unstaged.filter((e) => e.path !== 'README.md');
}

/** P20 §8.4: seed a paused cherry-pick / revert with one conflicted path
 *  (src/app.ts), reusing the merge-conflict marker fixture. Mirrors the merge
 *  branch of seedOpState so getOpState + listConflicts + the OpBanner render
 *  from the mock with no extra plumbing. */
function seedPickRevertConflict(state: MockRepoState, kind: 'cherryPick' | 'revert'): void {
  state.opState = { kind };
  state.conflicts = [
    { path: 'src/app.ts', kind: 'bothModified', hasBase: true, hasOurs: true, hasTheirs: true },
  ];
  state.conflictTexts = new Map();
  state.conflictTexts.set('src/app.ts', {
    path: 'src/app.ts',
    kind: 'bothModified',
    binary: false,
    tooLarge: false,
    missing: false,
    text: MERGE_AUTH_TEXT, // reuse the marker fixture
    ours: MERGE_AUTH_OURS,
    theirs: MERGE_AUTH_THEIRS,
  });
  state.status.conflicted = state.conflicts.map((c) => ({
    path: c.path,
    origPath: null,
    status: 'conflicted',
  }));
}

/** Deterministic 40-hex oid for a default-fixture row — MUST match
 *  fixtures/graph.ts `oid(row)` so seeded stash baseOids line up with the graph
 *  pills (index 0 → row 3 `core work 4`; indices 1 & 2 → row 6 `core work 2`). */
function fixtureOid(row: number): string {
  return row.toString(16).padStart(2, '0').repeat(20);
}

/** Seed the DEFAULT repo's stash stack so the sidebar list and the graph pills
 *  (fixtures/graph.ts §6.6) tell the same story. Non-default repos get []. */
function seedStashes(kind: RepoKind, graphFixture: GraphFixture): StashEntry[] {
  if (kind !== 'default' || graphFixture !== 'default') return [];
  const now = Math.floor(Date.now() / 1000);
  return [
    {
      index: 0,
      message: 'WIP on main: polish sidebar',
      oid: randomOid(),
      baseOid: fixtureOid(3), // `core work 4` — carries stash@{0} in the graph
      ts: now - 3600,
    },
    {
      index: 1,
      message: 'WIP on main: extract graph layout helpers',
      oid: randomOid(),
      baseOid: fixtureOid(6), // `core work 2` — carries stash@{1}
      ts: now - 7200,
    },
    {
      // Flagged (message contains 'reserved') so applyStash/popStash exercise the
      // Windows reserved-path recovery flow: first attempt → `reservedPaths`,
      // retry with skipReserved → `appliedSkippingReserved`. See RESERVED_STASH_*.
      index: 2,
      message: 'WIP on main: aspire host scaffolding (reserved-name files)',
      oid: randomOid(),
      baseOid: fixtureOid(6), // `core work 2` — carries stash@{2}
      ts: now - 10800,
    },
  ];
}

/** Mock-only marker: a stash whose message contains this substring is treated as
 *  containing a Windows-reserved path that cannot be checked out (mirrors the
 *  `'conflict'` demo-trigger convention). */
const RESERVED_STASH_MARKER = 'reserved';
/** The reserved path reported/skipped for a flagged fixture stash. */
const RESERVED_STASH_PATHS = ['src/Aspire.AppHost/NUL'];

/** True when this stash entry is the seeded reserved-path fixture. */
function stashHasReserved(entry: StashEntry | undefined): boolean {
  return entry !== undefined && entry.message.includes(RESERVED_STASH_MARKER);
}

/** Seed the DEFAULT repo's submodules so the sidebar section shows every badge
 *  state (P19 §5). Non-default repos get []. */
function seedSubmodules(kind: RepoKind, graphFixture: GraphFixture): SubmoduleInfo[] {
  if (kind !== 'default' || graphFixture !== 'default') return [];
  return [
    {
      name: 'vendor/libcore',
      path: 'vendor/libcore',
      absPath: '/mock/repo/vendor/libcore',
      url: 'https://example.com/libcore.git',
      headOid: fixtureOid(1),
      indexOid: fixtureOid(1),
      wtOid: null,
      status: 'uninitialized',
    },
    {
      name: 'vendor/theme',
      path: 'vendor/theme',
      absPath: '/mock/repo/vendor/theme',
      url: 'https://example.com/theme.git',
      headOid: fixtureOid(2),
      indexOid: fixtureOid(2),
      wtOid: fixtureOid(2),
      status: 'upToDate',
    },
    {
      name: 'docs/spec',
      path: 'docs/spec',
      absPath: '/mock/repo/docs/spec',
      url: 'https://example.com/spec.git',
      headOid: fixtureOid(4),
      indexOid: fixtureOid(4),
      wtOid: randomOid(),
      status: 'outOfSync',
    },
    {
      name: 'tools/ci',
      path: 'tools/ci',
      absPath: '/mock/repo/tools/ci',
      url: 'https://example.com/ci.git',
      headOid: fixtureOid(5),
      indexOid: fixtureOid(5),
      wtOid: fixtureOid(5),
      status: 'modifiedWorkdir',
    },
  ];
}

/** Seed the DEFAULT repo's worktrees so the sidebar section shows every badge:
 *  main, a clean linked, a locked linked, a stale/prunable linked (P27 §5).
 *  Stored rows carry `isCurrent: false`; the flag is computed per viewing repo
 *  at list time (listWorktrees) since all tabs share one repository. */
function seedWorktrees(kind: RepoKind, graphFixture: GraphFixture): WorktreeInfo[] {
  if (kind !== 'default' || graphFixture !== 'default') return [];
  return [
    {
      name: 'repo',
      absPath: '/mock/repo',
      relPath: null,
      branch: 'main',
      headOid: fixtureOid(1),
      locked: false,
      lockReason: null,
      isMain: true,
      isCurrent: false,
      prunable: false,
      valid: true,
    },
    {
      name: 'feature-login',
      absPath: '/mock/.worktrees/repo/feature-login',
      relPath: null,
      branch: 'feature/login',
      headOid: fixtureOid(3),
      locked: false,
      lockReason: null,
      isMain: false,
      isCurrent: false,
      prunable: false,
      valid: true,
    },
    {
      name: 'release-1.2',
      absPath: '/mock/.worktrees/repo/release-1.2',
      relPath: null,
      branch: 'release/1.2',
      headOid: fixtureOid(4),
      locked: true,
      lockReason: 'pinned for QA',
      isMain: false,
      isCurrent: false,
      prunable: false,
      valid: true,
    },
    {
      name: 'hotfix-stale',
      absPath: '/mock/.worktrees/repo/hotfix-stale',
      relPath: null,
      branch: null,
      headOid: null,
      locked: false,
      lockReason: null,
      isMain: false,
      isCurrent: false,
      prunable: true,
      valid: false,
    },
  ];
}

/** Module-level worktree list shared across all default-kind repo states —
 *  matches native, where every open tab views the same repository, so
 *  add/remove/lock/unlock are visible in every tab. Lazily seeded. */
let sharedWorktrees: WorktreeInfo[] | null = null;

/** The worktree list backing a repo state: the shared list for default repos,
 *  a throwaway empty list otherwise (non-default repos have no worktrees). */
function worktreesFor(state: MockRepoState): WorktreeInfo[] {
  if (state.kind !== 'default' || state.graphFixture !== 'default') return [];
  if (sharedWorktrees === null) {
    sharedWorktrees = seedWorktrees('default', 'default');
  }
  return sharedWorktrees;
}

// --- P31 §6: per-worktree instruction files (worktreeKey → relPath → content).
// Module-level shared like `sharedWorktrees` — all default tabs view the same
// repository, so an activation into a linked worktree is visible from every
// tab's matrix. `@main` is NOT stored here: the main worktree's files live in
// each tab's `state.assetContent`/`state.inventory` (the P24 mock map), so
// `@main` activations keep flipping the AiAssetsPanel drift chips.
// Seeds: feature-login has a locally-tweaked (drifted) CLAUDE.md and is
// missing AGENTS.md; release-1.2 is locked (files never scanned);
// hotfix-stale is invalid (empty).
let sharedWorktreeFiles: Map<string, Record<string, string>> | null = null;

function worktreeFilesFor(key: string): Record<string, string> {
  if (sharedWorktreeFiles === null) {
    sharedWorktreeFiles = new Map<string, Record<string, string>>([
      [
        'feature-login',
        {
          'CLAUDE.md': '# CLAUDE.md\n\nfeature-login local tweaks (drifted).\n',
          'GEMINI.md': CHEAP_TERSE_BODY,
        },
      ],
      ['release-1.2', { 'CLAUDE.md': OPUS_RICH_BODY }],
      ['hotfix-stale', {}],
    ]);
  }
  const seeded = sharedWorktreeFiles;
  let files = seeded.get(key);
  if (files === undefined) {
    files = {};
    seeded.set(key, files);
  }
  return files;
}

/** Drift/missing counts over a worktree's file map — the same math as
 *  `recomputeDrift` (canonical = first existing comparable doc), but comparing
 *  content directly instead of hashes (equivalent for the mock). */
function worktreeDriftCounts(files: Record<string, string>): {
  drifted: number;
  missing: number;
} {
  const content = (id: string): string | undefined => files[SINGLE_FILE_PATHS[id]];
  const canonicalId = COMPARABLE_IDS.find((id) => content(id) !== undefined) ?? null;
  const canonical = canonicalId === null ? null : content(canonicalId);
  let drifted = 0;
  let missing = 0;
  for (const id of COMPARABLE_IDS) {
    const c = content(id);
    if (c === undefined) missing += 1;
    else if (canonical !== null && c !== canonical) drifted += 1;
  }
  return { drifted, missing };
}

/** The calling tab's own worktree key (D5): the linked row whose path this
 *  tab has open, else `"@main"`. */
function tabWorktreeKey(state: MockRepoState): string {
  const row = worktreesFor(state).find((w) => !w.isMain && w.absPath === state.path);
  return row === undefined ? '@main' : row.name;
}

/** D6 eligibility guard for the worktree-targeted preview/activate mocks:
 *  throws the backend's refusal messages for unknown / invalid / prunable /
 *  locked worktrees; returns the row otherwise. `"@main"` maps to the main
 *  row, which is always eligible. */
function requireEligibleWorktree(state: MockRepoState, worktreeKey: string): WorktreeInfo {
  const rows = worktreesFor(state);
  if (worktreeKey === '@main') {
    // The main worktree is always eligible — synthesize a row for fixtures
    // without a worktree list (mirrors the backend, where "@main" resolves on
    // any repo).
    return (
      rows.find((w) => w.isMain) ?? {
        name: 'repo',
        absPath: state.path,
        relPath: null,
        branch: state.headBranch,
        headOid: state.headOid,
        locked: false,
        lockReason: null,
        isMain: true,
        isCurrent: true,
        prunable: false,
        valid: true,
      }
    );
  }
  const row = rows.find((w) => !w.isMain && w.name === worktreeKey);
  if (row === undefined) {
    const err: AppError = { kind: 'git', message: `worktree '${worktreeKey}' not found` };
    throw err;
  }
  if (!row.valid) {
    const err: AppError = {
      kind: 'git',
      message: `worktree '${worktreeKey}' is invalid (its working directory is missing or broken)`,
    };
    throw err;
  }
  if (row.prunable) {
    const err: AppError = {
      kind: 'git',
      message: `worktree '${worktreeKey}' is stale (prunable); prune or repair it first`,
    };
    throw err;
  }
  if (row.locked) {
    const err: AppError = {
      kind: 'git',
      message: `worktree '${worktreeKey}' is locked; unlock it first`,
    };
    throw err;
  }
  return row;
}

/** Builds a fresh MockRepoState for a usable repo (default / detached / unborn). */
function createRepoState(path: string): MockRepoState {
  const graphFixture = repoGraphFixture(path);
  const kind = repoKind(path, graphFixture);
  const state: MockRepoState = {
    path,
    kind,
    graphFixture,
    // Seed a WORKING identity by default; `?fixture=noconfig` drops it so the
    // commit-error / Set-identity flow is demoable (P40 §6.3).
    config: makeMockConfigStore(query('fixture') !== 'noconfig'),
    remoteTrigger: query('remote'),
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
function buildInfo(state: MockRepoState, path: string): RepoInfo {
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
function requireRepo(repoId: string): MockRepoState {
  const state = repos.get(repoId);
  if (state === undefined) {
    const err: AppError = { kind: 'noRepo', message: 'mock: repository is not open' };
    throw err;
  }
  return state;
}

// Recents persistence (P1 contract §3.4): localStorage-backed so the harness
// reopen-on-launch story is verifiable — open once, reload, auto-reopen.
const RECENTS_KEY = 'bonsai.mockRecents';
const MAX_RECENTS = 10;

/** Corrupt/missing storage degrades to [] — mirrors the backend's load_from. */
function readRecents(): RecentRepo[] {
  try {
    const raw = window.localStorage.getItem(RECENTS_KEY);
    if (raw === null) return [];
    const parsed: unknown = JSON.parse(raw);
    if (!Array.isArray(parsed)) return [];
    return parsed.filter(
      (r): r is RecentRepo =>
        typeof r === 'object' &&
        r !== null &&
        typeof (r as RecentRepo).path === 'string' &&
        typeof (r as RecentRepo).lastOpened === 'number',
    );
  } catch {
    return [];
  }
}

function writeRecents(list: RecentRepo[]): void {
  try {
    window.localStorage.setItem(RECENTS_KEY, JSON.stringify(list));
  } catch {
    // Best-effort, like the backend's non-fatal save.
  }
}

// Session persistence (P3e contract §6/§8.1): localStorage-backed like recents /
// ui-settings so reopen-all survives a harness reload.
const SESSION_KEY = 'bonsai.mockSession';

/** Corrupt/missing storage degrades to an empty session — mirrors load_from. */
function readSession(): SessionState {
  try {
    const raw = window.localStorage.getItem(SESSION_KEY);
    if (raw === null) return { openRepos: [], activeRepo: null };
    const parsed = JSON.parse(raw) as Partial<SessionState>;
    const openRepos = Array.isArray(parsed.openRepos)
      ? parsed.openRepos.filter((r): r is string => typeof r === 'string')
      : [];
    const activeRepo = typeof parsed.activeRepo === 'string' ? parsed.activeRepo : null;
    return { openRepos, activeRepo };
  } catch {
    return { openRepos: [], activeRepo: null };
  }
}

function writeSession(session: SessionState): void {
  try {
    window.localStorage.setItem(SESSION_KEY, JSON.stringify(session));
  } catch {
    // Best-effort, like the backend's non-fatal save.
  }
}

// UI settings persistence (P2a contract §2.4): mirrors bonsai.mockRecents —
// localStorage-backed so the harness drag/toggle-then-reload story is
// verifiable. Ranges mirror settings.rs's clamp_pane_widths — the ONE place
// the mock duplicates a Rust-side clamp, acceptable because it's a pure
// numeric guard, not git/layout logic (contract §2.4).
const UI_SETTINGS_KEY = 'bonsai.mockUiSettings';
const SIDEBAR_MIN = 180;
const SIDEBAR_MAX = 480;
const RIGHT_PANEL_MIN = 280;
const RIGHT_PANEL_MAX = 640;

const DEFAULT_UI_SETTINGS: UiSettings = {
  theme: 'dark',
  paneWidths: { sidebar: 240, rightPanel: 380 },
  listView: 'tree',
  autoFetch: { enabled: false, intervalMinutes: 5 },
  // P30: backend-scheduler healthRefresh signal; disabled by default.
  healthRefresh: { enabled: false, intervalMinutes: 30 },
  graph: { dotRadius: 4, avatarRadius: 10, rowHeight: 32, laneWidth: 16 },
  // AI assistance (P13): enabled by default, but consent gates the feature.
  aiEnabled: true,
  aiConflictAutonomy: 'proposeReview',
  aiConsented: false,
  // Embedded MCP server (P16): consent gates the enable toggle.
  mcpConsented: false,
  // MCP write consent (P16c): a separate, stronger gate for the write toggle.
  mcpWriteConsented: false,
  // P43: onboarding unseen by default so a fresh browser harness shows it.
  onboardingSeen: false,
  // P42: auto-check-updates-on-launch OFF by default (privacy / opt-in).
  autoCheckUpdates: false,
  // P44: two seeded identity profiles so the harness shows a populated list
  // and Apply is exercisable (fixed string ids).
  profiles: [
    {
      id: 'mock-work',
      label: 'Work',
      userName: 'Mock Fixture User',
      userEmail: 'work@bonsai.dev',
      signingKey: null,
    },
    {
      id: 'mock-personal',
      label: 'Personal',
      userName: 'Mock Personal',
      userEmail: 'me@personal.dev',
      signingKey: 'ABC123',
    },
  ],
};

function clampPaneWidths(w: PaneWidths): PaneWidths {
  return {
    sidebar: Math.min(SIDEBAR_MAX, Math.max(SIDEBAR_MIN, w.sidebar)),
    rightPanel: Math.min(RIGHT_PANEL_MAX, Math.max(RIGHT_PANEL_MIN, w.rightPanel)),
  };
}

/** Mirrors Rust `clamp_auto_fetch` (settings.rs). */
function clampAutoFetch(a: AutoFetchSettings): AutoFetchSettings {
  return {
    enabled: a.enabled,
    intervalMinutes: Math.min(
      AUTO_FETCH_INTERVAL_MAX,
      Math.max(AUTO_FETCH_INTERVAL_MIN, a.intervalMinutes),
    ),
  };
}

/** Mirrors Rust `clamp_health_refresh` (settings.rs, P30). */
function clampHealthRefresh(h: HealthRefreshSettings): HealthRefreshSettings {
  return {
    enabled: h.enabled,
    intervalMinutes: Math.min(
      HEALTH_REFRESH_INTERVAL_MAX,
      Math.max(HEALTH_REFRESH_INTERVAL_MIN, h.intervalMinutes),
    ),
  };
}

/** Mirrors Rust `clamp_graph_prefs` (settings.rs). */
function clampGraphPrefs(g: GraphPrefs): GraphPrefs {
  return {
    dotRadius: Math.min(DOT_RADIUS_MAX, Math.max(DOT_RADIUS_MIN, g.dotRadius)),
    avatarRadius: Math.min(AVATAR_RADIUS_MAX, Math.max(AVATAR_RADIUS_MIN, g.avatarRadius)),
    rowHeight: Math.min(ROW_HEIGHT_MAX, Math.max(ROW_HEIGHT_MIN, g.rowHeight)),
    laneWidth: Math.min(LANE_WIDTH_MAX, Math.max(LANE_WIDTH_MIN, g.laneWidth)),
  };
}

/** Corrupt/missing storage degrades to the default — mirrors load_from. */
function readUiSettings(): UiSettings {
  try {
    const raw = window.localStorage.getItem(UI_SETTINGS_KEY);
    if (raw === null) return structuredClone(DEFAULT_UI_SETTINGS);
    const parsed = JSON.parse(raw) as Partial<UiSettings>;
    const theme: Theme = parsed.theme === 'light' ? 'light' : 'dark';
    const paneWidths = clampPaneWidths({
      sidebar:
        typeof parsed.paneWidths?.sidebar === 'number'
          ? parsed.paneWidths.sidebar
          : DEFAULT_UI_SETTINGS.paneWidths.sidebar,
      rightPanel:
        typeof parsed.paneWidths?.rightPanel === 'number'
          ? parsed.paneWidths.rightPanel
          : DEFAULT_UI_SETTINGS.paneWidths.rightPanel,
    });
    const listView: ListView = parsed.listView === 'flat' ? 'flat' : 'tree';
    const autoFetch = clampAutoFetch({
      enabled:
        typeof parsed.autoFetch?.enabled === 'boolean'
          ? parsed.autoFetch.enabled
          : DEFAULT_UI_SETTINGS.autoFetch.enabled,
      intervalMinutes:
        typeof parsed.autoFetch?.intervalMinutes === 'number'
          ? parsed.autoFetch.intervalMinutes
          : DEFAULT_UI_SETTINGS.autoFetch.intervalMinutes,
    });
    // P30 healthRefresh (additive, like autoFetch): fall back to defaults.
    const healthRefresh = clampHealthRefresh({
      enabled:
        typeof parsed.healthRefresh?.enabled === 'boolean'
          ? parsed.healthRefresh.enabled
          : DEFAULT_UI_SETTINGS.healthRefresh.enabled,
      intervalMinutes:
        typeof parsed.healthRefresh?.intervalMinutes === 'number'
          ? parsed.healthRefresh.intervalMinutes
          : DEFAULT_UI_SETTINGS.healthRefresh.intervalMinutes,
    });
    const graph = clampGraphPrefs({
      dotRadius:
        typeof parsed.graph?.dotRadius === 'number'
          ? parsed.graph.dotRadius
          : DEFAULT_UI_SETTINGS.graph.dotRadius,
      avatarRadius:
        typeof parsed.graph?.avatarRadius === 'number'
          ? parsed.graph.avatarRadius
          : DEFAULT_UI_SETTINGS.graph.avatarRadius,
      rowHeight:
        typeof parsed.graph?.rowHeight === 'number'
          ? parsed.graph.rowHeight
          : DEFAULT_UI_SETTINGS.graph.rowHeight,
      laneWidth:
        typeof parsed.graph?.laneWidth === 'number'
          ? parsed.graph.laneWidth
          : DEFAULT_UI_SETTINGS.graph.laneWidth,
    });
    // P13 AI fields (additive, like autoFetch/graph): fall back to defaults.
    const aiEnabled =
      typeof parsed.aiEnabled === 'boolean' ? parsed.aiEnabled : DEFAULT_UI_SETTINGS.aiEnabled;
    const aiConflictAutonomy: AiAutonomy =
      parsed.aiConflictAutonomy === 'autoResolve' ? 'autoResolve' : 'proposeReview';
    const aiConsented =
      typeof parsed.aiConsented === 'boolean' ? parsed.aiConsented : DEFAULT_UI_SETTINGS.aiConsented;
    // P16 MCP consent (additive, like the AI fields): fall back to default.
    const mcpConsented =
      typeof parsed.mcpConsented === 'boolean'
        ? parsed.mcpConsented
        : DEFAULT_UI_SETTINGS.mcpConsented;
    // P16c MCP write consent (additive): fall back to default.
    const mcpWriteConsented =
      typeof parsed.mcpWriteConsented === 'boolean'
        ? parsed.mcpWriteConsented
        : DEFAULT_UI_SETTINGS.mcpWriteConsented;
    // P43 onboarding seen (additive): fall back to default (false ⇒ show).
    const onboardingSeen =
      typeof parsed.onboardingSeen === 'boolean'
        ? parsed.onboardingSeen
        : DEFAULT_UI_SETTINGS.onboardingSeen;
    // P42 auto-check-updates (additive): fall back to default (false).
    const autoCheckUpdates =
      typeof parsed.autoCheckUpdates === 'boolean'
        ? parsed.autoCheckUpdates
        : DEFAULT_UI_SETTINGS.autoCheckUpdates;
    // P44 identity profiles (additive): degrade to default if absent/malformed.
    const profiles: IdentityProfile[] = Array.isArray(parsed.profiles)
      ? parsed.profiles
      : structuredClone(DEFAULT_UI_SETTINGS.profiles);
    return {
      theme,
      paneWidths,
      listView,
      autoFetch,
      healthRefresh,
      graph,
      aiEnabled,
      aiConflictAutonomy,
      aiConsented,
      mcpConsented,
      mcpWriteConsented,
      onboardingSeen,
      autoCheckUpdates,
      profiles,
    };
  } catch {
    return structuredClone(DEFAULT_UI_SETTINGS);
  }
}

function writeUiSettings(s: UiSettings): void {
  try {
    window.localStorage.setItem(UI_SETTINGS_KEY, JSON.stringify(s));
  } catch {
    // Best-effort, like the backend's non-fatal save.
  }
}

// ---------------------------------------------------------------------------
// P30 §7: background-job mock harness. Stateful per-repo JobStatus + synthetic
// ticks that treat **intervalMinutes as SECONDS** (documented test-speed shim
// so the harness shows activity without waiting minutes). The fixture repo
// simulates failure escalation (backoff) when localStorage
// `bonsaiMockJobFail=1` is set — exactly one backoff toast at failure 3.
// ---------------------------------------------------------------------------

const MOCK_JOB_FAIL_KEY = 'bonsaiMockJobFail';
const BACKOFF_THRESHOLD = 3; // mirrors scheduler.rs
const BACKOFF_MAX_FACTOR = 8;

/** Listener registries: the mock's stand-in for the Tauri event system. */
const repoChangedListeners = new Set<(p: RepoChangedPayload) => void>();
const jobStatusListeners = new Set<(p: JobStatusChangedPayload) => void>();

const jobStatuses = new Map<string, JobStatus[]>();

/** Lazy per-repo seed (contract §7): autoFetch success 2 min ago,
 *  healthRefresh disabled/never run. */
function seedJobStatuses(repoId: string): JobStatus[] {
  let list = jobStatuses.get(repoId);
  if (list === undefined) {
    list = [
      {
        job: 'autoFetch',
        enabled: false,
        lastRunMs: Date.now() - 2 * 60_000,
        lastOutcome: 'success',
        lastError: null,
        consecutiveFailures: 0,
        inBackoff: false,
        nextRunMs: null,
      },
      {
        job: 'healthRefresh',
        enabled: false,
        lastRunMs: null,
        lastOutcome: null,
        lastError: null,
        consecutiveFailures: 0,
        inBackoff: false,
        nextRunMs: null,
      },
    ];
    jobStatuses.set(repoId, list);
  }
  return list;
}

/** Mirrors scheduler.rs `effective_interval_ms` (base for failures 0–2,
 *  base*2^(f-2) for ≥3, capped at 8×). */
function mockEffectiveIntervalMs(baseMs: number, failures: number): number {
  if (failures < BACKOFF_THRESHOLD) return baseMs;
  const factor = Math.min(BACKOFF_MAX_FACTOR, 2 ** (failures - (BACKOFF_THRESHOLD - 1)));
  return baseMs * factor;
}

function mockJobFailEnabled(): boolean {
  try {
    return window.localStorage.getItem(MOCK_JOB_FAIL_KEY) === '1';
  } catch {
    return false;
  }
}

/** Completes one synthetic job run for `repoId`: updates the stateful status,
 *  dispatches `job-status-changed`, then `repo-changed` on refreshing
 *  successes — the same ordering/shape as the Rust scheduler. */
function completeMockJobRun(repoId: string, job: JobKind): void {
  const state = repos.get(repoId);
  if (state === undefined) return;
  const settings = readUiSettings();
  const cfg = job === 'autoFetch' ? settings.autoFetch : settings.healthRefresh;
  const entry = seedJobStatuses(repoId).find((s) => s.job === job);
  if (entry === undefined) return;

  // Failure shim: only autoFetch on the fixture repo escalates.
  const failed =
    job === 'autoFetch' && mockJobFailEnabled() && state.path.includes('bonsai-fixture');
  const now = Date.now();
  const failures = failed ? entry.consecutiveFailures + 1 : 0;
  const inBackoff = failures >= BACKOFF_THRESHOLD;
  const enteredBackoff = failed && failures === BACKOFF_THRESHOLD;
  // Test-speed shim: intervalMinutes as SECONDS.
  const nextRunMs = cfg.enabled
    ? now + mockEffectiveIntervalMs(cfg.intervalMinutes * 1000, failures)
    : null;
  const updatedRefs = !failed && job === 'autoFetch' ? 2 : undefined;
  const error = failed ? 'mock: could not connect to origin (bonsaiMockJobFail=1)' : undefined;

  entry.enabled = cfg.enabled;
  entry.lastRunMs = now;
  entry.lastOutcome = failed ? 'failed' : 'success';
  entry.lastError = error ?? null;
  entry.consecutiveFailures = failures;
  entry.inBackoff = inBackoff;
  entry.nextRunMs = nextRunMs;

  const payload: JobStatusChangedPayload = {
    repoId,
    job,
    outcome: failed ? 'failed' : 'success',
    updatedRefs,
    error,
    consecutiveFailures: failures,
    inBackoff,
    enteredBackoff,
    tsMs: now,
    nextRunMs,
  };
  for (const cb of jobStatusListeners) cb(payload);
  // Rust emits repo-changed on autoFetch success with updatedRefs > 0 and on
  // every healthRefresh success.
  if (!failed && (job === 'healthRefresh' || (updatedRefs ?? 0) > 0)) {
    const rc: RepoChangedPayload = { repoId, reason: 'fs' };
    for (const cb of repoChangedListeners) cb(rc);
  }
}

const jobTimers: { autoFetch: number | null; healthRefresh: number | null } = {
  autoFetch: null,
  healthRefresh: null,
};

/** (Re)arms the synthetic tick timers from the given settings — called at
 *  module init and after every setUiSettings round-trip. */
function applyMockJobTimers(s: UiSettings): void {
  for (const job of ['autoFetch', 'healthRefresh'] as const) {
    const timer = jobTimers[job];
    if (timer !== null) {
      window.clearInterval(timer);
      jobTimers[job] = null;
    }
    const cfg = job === 'autoFetch' ? s.autoFetch : s.healthRefresh;
    if (cfg.enabled) {
      jobTimers[job] = window.setInterval(() => {
        for (const repoId of repos.keys()) completeMockJobRun(repoId, job);
      }, cfg.intervalMinutes * 1000); // minutes-as-seconds shim
    }
  }
}

// Arm timers from persisted settings so a reload keeps ticking.
applyMockJobTimers(readUiSettings());

// Embedded MCP server (P16). In-memory module state — no real socket; the
// harness only verifies the Settings UI wiring. Fake but plausible port/token.
const MOCK_MCP_PORT = 8765;
const MOCK_MCP_TOKEN = 'mock-token-abc123';

const mockMcp: {
  enabled: boolean;
  allowWrite: boolean;
  activeRepo: string | null;
  listeners: Set<(s: McpStatus) => void>;
} = {
  enabled: false,
  allowWrite: false,
  activeRepo: null,
  listeners: new Set(),
};

function mcpStatusOf(): McpStatus {
  const toolCount = mockMcp.allowWrite ? 34 : 14;
  if (!mockMcp.enabled) {
    return {
      enabled: false,
      allowWrite: false,
      port: null,
      url: null,
      token: null,
      toolCount,
    };
  }
  const url = `http://127.0.0.1:${MOCK_MCP_PORT}/mcp`;
  return {
    enabled: true,
    allowWrite: mockMcp.allowWrite,
    port: MOCK_MCP_PORT,
    url,
    token: MOCK_MCP_TOKEN,
    toolCount,
  };
}

/** Upsert at front, dedupe case-insensitively, cap 10 (mirrors record_recent). */
function recordRecent(path: string): void {
  const list = readRecents().filter((r) => r.path.toLowerCase() !== path.toLowerCase());
  list.unshift({ path, lastOpened: Math.floor(Date.now() / 1000) });
  writeRecents(list.slice(0, MAX_RECENTS));
}

function throwAuthFailed(): never {
  const err: AppError = {
    kind: 'authFailed',
    message:
      "authentication failed for 'origin': no usable credentials. Configure a Git " +
      'credential helper (e.g. Git Credential Manager) for HTTPS remotes, or run an ' +
      'SSH agent for SSH remotes.',
  };
  throw err;
}

function throwNetworkError(): never {
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
function isInvalidBranchName(name: string): boolean {
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

function randomOid(): string {
  return Array.from({ length: 40 }, () => '0123456789abcdef'[Math.floor(Math.random() * 16)]).join(
    '',
  );
}

/**
 * P25 §6.3: stale-branch classification seed. Maps a fixture local-branch name
 * to the classification the backend would compute. Branches absent from this map
 * (or the current HEAD / base) are never stale. `experiment-unmerged` is
 * intentionally omitted (NEITHER merged nor gone → excluded from the report).
 */
const STALE_SEED: Record<
  string,
  { reason: StaleReason; merged: boolean; goneUpstream: boolean }
> = {
  'feature/merged-a': { reason: 'merged', merged: true, goneUpstream: false },
  'feature/merged-b': { reason: 'merged', merged: true, goneUpstream: false },
  'feature/gone': { reason: 'goneUpstream', merged: false, goneUpstream: true },
};

/**
 * Recomputes the stale report from the live `state.branches.local`, mirroring the
 * server rules: base = 'main', excludes the base and the current HEAD branch, and
 * only surfaces branches still present that are classified in `STALE_SEED`. A
 * prior `deleteBranches` that removed a local shrinks the report naturally.
 */
function buildStaleReport(state: MockRepoState): StaleReport {
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

function matchesAny(entry: StatusEntry, paths: string[]): boolean {
  return paths.includes(entry.path) || (entry.origPath !== null && paths.includes(entry.origPath));
}

function sortByPath(entries: StatusEntry[]): void {
  entries.sort((a, b) => (a.path < b.path ? -1 : a.path > b.path ? 1 : 0));
}

/** P17: line-array equality for the three-way model (index vs head/workdir). */
function linesEqual(a: string[], b: string[]): boolean {
  return a.length === b.length && a.every((l, i) => l === b[i]);
}

const MAIN_RS_PATH = 'src/main.rs';

/** P17: split a wire selection into add (by newNo) / del (by oldNo) sets;
 *  stray Context elements are ignored (kept in both directions). */
function collectSelection(selection: LineSelection[]): {
  selAdd: Set<number>;
  selDel: Set<number>;
} {
  const selAdd = new Set<number>();
  const selDel = new Set<number>();
  for (const s of selection) {
    if (s.kind === 'add' && s.newNo !== null) selAdd.add(s.newNo);
    else if (s.kind === 'del' && s.oldNo !== null) selDel.add(s.oldNo);
  }
  return { selAdd, selDel };
}

/** Removes matching entries from `from` and returns them. */
function takeMatching(from: StatusEntry[], paths: string[]): StatusEntry[] {
  const taken = from.filter((e) => matchesAny(e, paths));
  const kept = from.filter((e) => !matchesAny(e, paths));
  from.length = 0;
  from.push(...kept);
  return taken;
}

/** Upserts into `into`, deduping by `path` (new entry wins). */
function upsert(into: StatusEntry[], entry: StatusEntry): void {
  const idx = into.findIndex((e) => e.path === entry.path);
  if (idx !== -1) into.splice(idx, 1);
  into.push(entry);
}

/**
 * Completes a paused rebase (shared by rebaseContinue/rebaseSkip): clears the
 * op + conflicted status, moves HEAD, and prepends `steps` plain replayed
 * MockCommits atop the graph so they visibly appear.
 */
function finishRebase(state: MockRepoState, steps: number): RebaseOutcome {
  state.opState = { kind: 'none' };
  state.status.conflicted = [];
  state.headOid = randomOid();
  // commits[0] is the topmost row = the new HEAD tip (prependCommits maps
  // index 0 to headIndex 0), so the tip carries headOid.
  const replayed: MockCommit[] = Array.from({ length: steps }, (_, i) => ({
    oid: i === 0 ? state.headOid : randomOid(),
    summary: `pick: replayed ${steps - i}`,
  }));
  state.commits.unshift(...replayed);
  return { kind: 'rebased', branch: state.headBranch, head: state.headOid, steps };
}

/** P23b: first line of a (possibly multi-line) message. */
function firstLine(msg: string): string {
  return msg.split('\n', 1)[0] ?? '';
}

/**
 * P23b §7.2: apply an interactive-rebase plan (execution order = oldest-first)
 * to the mock commit list DETERMINISTICALLY, producing the rewritten commits.
 * Drops `drop` rows; keeps the array order (reorder); combines `squash`/`fixup`
 * into the preceding kept commit (squash uses `newMessage`'s first line when
 * provided, else concatenates summaries; fixup keeps the predecessor's summary);
 * `reword` applies its `newMessage`'s first line as the summary. Every rewritten
 * commit gets a fresh oid (a rebase always rewrites). Returns NEWEST-first so the
 * result is ready to unshift onto the graph.
 */
function applyInteractivePlan(
  todos: RebaseTodoOp[],
  summaryOf: (oid: string) => string,
): MockCommit[] {
  const result: MockCommit[] = []; // execution order (oldest-first)
  for (const t of todos) {
    if (t.action === 'drop') continue;
    if (t.action === 'squash' || t.action === 'fixup') {
      const prev = result[result.length - 1];
      if (prev === undefined) {
        // squash/fixup as the first applied row is invalid; the editor blocks it
        // and the backend rejects it, but treat it as a pick here defensively.
        result.push({ oid: randomOid(), summary: summaryOf(t.oid) });
        continue;
      }
      if (t.action === 'squash') {
        prev.summary =
          t.newMessage !== null && t.newMessage.trim() !== ''
            ? firstLine(t.newMessage)
            : `${prev.summary}; ${summaryOf(t.oid)}`;
      }
      // fixup keeps the predecessor's summary. Either way the combined commit is
      // rewritten → fresh oid.
      prev.oid = randomOid();
      continue;
    }
    // pick | reword
    const summary =
      t.action === 'reword' && t.newMessage !== null
        ? firstLine(t.newMessage)
        : summaryOf(t.oid);
    result.push({ oid: randomOid(), summary });
  }
  return result.reverse(); // newest-first
}

/**
 * P23b §7.2: finish (or skip-then-finish) a paused interactive rebase — clears
 * the op + conflicted status and prepends the rewritten commits onto the graph
 * so the harness shows the rewritten history. `dropCurrent` (Skip) drops the
 * oldest remaining rewritten commit (the conflicting op).
 */
function finishInteractiveRebase(state: MockRepoState, dropCurrent: boolean): RebaseOutcome {
  const plan = state.interactive;
  state.opState = { kind: 'none' };
  state.status.conflicted = [];
  state.conflicts = [];
  state.conflictTexts = new Map();
  state.interactive = null;
  if (plan === null) {
    // Shouldn't happen (callers check first); fall back to a no-op finish.
    return { kind: 'rebased', branch: state.headBranch, head: state.headOid, steps: 0 };
  }
  // Skip drops the current (conflicting) op — the oldest remaining rewritten row.
  let rewritten = dropCurrent && plan.rewritten.length > 0 ? plan.rewritten.slice(0, -1) : plan.rewritten;
  // Remove the original range commits (base..old-HEAD) so the rewritten commits
  // REPLACE them rather than stacking a duplicate set on top (true rewrite).
  const removed = new Set(plan.originalOids);
  state.commits = state.commits.filter((c) => !removed.has(c.oid));
  state.headOid = randomOid();
  if (rewritten.length > 0) {
    // The topmost row (newest-first index 0) is the new HEAD tip.
    rewritten = rewritten.map((c, i) => (i === 0 ? { ...c, oid: state.headOid } : c));
    state.commits.unshift(...rewritten);
  }
  return { kind: 'rebased', branch: plan.headName, head: state.headOid, steps: rewritten.length };
}

// -------------------------------------------------------------------- P39 bisect
//
// Stateful, deterministic binary search over a synthetic candidate chain so the
// harness walks start → mark → found with the banner counts halving each step.

/** Number of untested candidates strictly between the good/bad bounds, minus
 *  skipped ones (== the `revisionsRemaining` the banner shows). */
function bisectTestable(mb: MockBisect): number[] {
  const out: number[] = [];
  for (let i = mb.lo + 1; i < mb.hi; i++) {
    if (!mb.skipped.includes(mb.chain[i])) out.push(i);
  }
  return out;
}

/** ceil(log2(remaining)); 0 when remaining ≤ 1 — mirrors the Rust estimate. */
function bisectSteps(remaining: number): number {
  if (remaining <= 1) return 0;
  return Math.ceil(Math.log2(remaining));
}

/** Projects the mock bisect state onto the RepoOpState.bisect wire shape and
 *  stores it as the repo's opState (what getOpState returns). */
function bisectProject(state: MockRepoState, mb: MockBisect): void {
  const remaining = mb.firstBad === null ? bisectTestable(mb).length : 0;
  state.opState = {
    kind: 'bisect',
    current: mb.current === null ? null : mb.chain[mb.current],
    bad: mb.chain[mb.hi],
    good: [mb.chain[mb.lo]],
    skipped: [...mb.skipped],
    firstBad: mb.firstBad,
    revisionsRemaining: remaining,
    estimatedSteps: bisectSteps(remaining),
  };
}

/** Picks the next midpoint (or converges) and returns the outcome, mutating the
 *  mock bisect + opState. Shared by start / mark / skip. */
function driveMockBisect(state: MockRepoState, mb: MockBisect): BisectOutcome {
  const testable = bisectTestable(mb);
  if (testable.length === 0) {
    mb.current = null;
    // Any skipped candidate still between the bounds → cannot determine.
    const unresolved = mb.chain
      .slice(mb.lo + 1, mb.hi)
      .some((oid) => mb.skipped.includes(oid));
    if (unresolved) {
      bisectProject(state, mb);
      return { kind: 'cannotDetermine', skipped: [...mb.skipped] };
    }
    // Converge: the bad bound is the first-bad commit.
    mb.firstBad = mb.chain[mb.hi];
    bisectProject(state, mb);
    return { kind: 'found', firstBad: mb.firstBad };
  }
  const mid = testable[Math.floor(testable.length / 2)];
  mb.current = mid;
  mb.firstBad = null;
  bisectProject(state, mb);
  const remaining = testable.length;
  return {
    kind: 'testing',
    current: mb.chain[mid],
    revisionsRemaining: remaining,
    estimatedSteps: bisectSteps(remaining),
  };
}

// P23d §10.2: blame/file-history fixtures. The oids mirror fixtures/graph.ts
// `oid(row)` (row hex, 2 digits, repeated 20×) so reveal-in-graph resolves to a
// real node in the default mock layout. Authors mirror that fixture's `author`
// (even rows Ada, odd rows Grace). The keyed paths are REAL status rows
// (`src/main.rs` shows in both Staged + Changes; `README.md` in Changes) so the
// row-action buttons produce populated views; every other path → git error / [].
const BLAME_FIXTURE_PATHS = new Set(['src/main.rs', 'README.md']);

function mockNodeOid(row: number): string {
  return row.toString(16).padStart(2, '0').repeat(20);
}

const BLAME_NOW = Math.floor(Date.now() / 1000);

const MOCK_BLAME: BlameLine[] = (() => {
  // (row, author, email, summary, lineText) per source line, grouped by commit
  // so consecutive same-oid lines collapse in the gutter (GitHub-blame look).
  const rows: Array<[number, string, string, string, string]> = [
    [1, 'Grace Hopper', 'grace@example.com', 'feat: polish', "import { render } from './render';"],
    [1, 'Grace Hopper', 'grace@example.com', 'feat: polish', ''],
    [5, 'Grace Hopper', 'grace@example.com', 'core work 3', 'export function main() {'],
    [5, 'Grace Hopper', 'grace@example.com', 'core work 3', '  const app = createApp();'],
    [0, 'Ada Lovelace', 'ada@example.com', 'Merge feat and exp', '  app.mount("#root");'],
    [0, 'Ada Lovelace', 'ada@example.com', 'Merge feat and exp', '  return app;'],
    [5, 'Grace Hopper', 'grace@example.com', 'core work 3', '}'],
  ];
  return rows.map(([row, name, email, summary, text], i) => ({
    oid: mockNodeOid(row),
    authorName: name,
    authorEmail: email,
    authorTs: BLAME_NOW - row * 3600,
    summary,
    origLineNo: i + 1,
    finalLineNo: i + 1,
    lineText: text,
  }));
})();

const MOCK_FILE_HISTORY: FileHistoryEntry[] = [
  { row: 0, name: 'Ada Lovelace', email: 'ada@example.com', summary: 'Merge feat and exp' },
  { row: 1, name: 'Grace Hopper', email: 'grace@example.com', summary: 'feat: polish' },
  { row: 5, name: 'Grace Hopper', email: 'grace@example.com', summary: 'core work 3' },
  { row: 8, name: 'Ada Lovelace', email: 'ada@example.com', summary: 'chore: history 19' },
].map(({ row, name, email, summary }) => ({
  oid: mockNodeOid(row),
  summary,
  authorName: name,
  authorEmail: email,
  authorTs: BLAME_NOW - row * 3600,
}));

export const mockIpc: IpcApi = {
  // Idempotent per repoId: a re-open focuses the existing tab (no state reset),
  // matching the real backend + the old single-repo `path !== openedPath` guard.
  async openRepo(path: string): Promise<OpenRepoResult> {
    await delay(150);

    if (path.includes('error')) {
      const err: AppError = { kind: 'io', message: 'mock: path does not exist' };
      throw err;
    }
    // Non-usable opens still return a repoId (for the frontend's error UI) but
    // create NO entry and touch no other tab (contract §4.2).
    if (path.includes('not-a-repo')) {
      return { repoId: mockCanonical(path), info: { path, isRepo: false, bare: false, head: null } };
    }
    if (path.includes('bare')) {
      return {
        repoId: mockCanonical(path),
        info: {
          path,
          isRepo: true,
          bare: true,
          head: { branchName: 'main', oid: '', detached: false, unborn: true },
        },
      };
    }

    // Usable open (isRepo && !bare — unborn included): create/focus an entry.
    const repoId = resolveRepoId(path);
    recordRecent(path);
    let state = repos.get(repoId);
    if (state === undefined) {
      state = createRepoState(path);
      repos.set(repoId, state);
    }
    return { repoId, info: buildInfo(state, path) };
  },

  // P21: clone a remote repo, streaming a few monotonic progress ticks (an
  // object-download phase then a delta-resolve phase, §2.1) so the harness bar
  // animates end-to-end, then return a path the EXISTING openRepo can seed.
  async cloneRepo(
    url: string,
    dest: string,
    onProgress: (p: CloneProgress) => void,
  ): Promise<string> {
    // Failure triggers compose with the M6 messages: `authfail` / `network` in the
    // URL throw the SAME AppErrors after a couple of ticks (exercise the in-dialog
    // error path).
    const failAuth = /authfail/i.test(url);
    const failNet = /network/i.test(url);
    const total = 20;
    for (let i = 1; i <= total; i++) {
      await delay(120);
      onProgress({
        receivedObjects: i,
        totalObjects: total,
        indexedDeltas: 0,
        totalDeltas: 0,
        receivedBytes: i * 4096,
      });
      if (i === 3 && failAuth) throwAuthFailed();
      if (i === 3 && failNet) throwNetworkError();
    }
    for (let i = 1; i <= 10; i++) {
      await delay(80);
      onProgress({
        receivedObjects: total,
        totalObjects: total,
        indexedDeltas: i,
        totalDeltas: 10,
        receivedBytes: total * 4096,
      });
    }
    // The frontend already computed dest = <parent>/<name>; the real backend
    // clones INTO dest and returns its workdir, so mirror that (return dest).
    // openRepo then seeds a normal default repo (dest avoids the reserved
    // 'error'|'not-a-repo'|'bare'|'unborn' substrings for typical URLs).
    return dest;
  },

  // P21: init (or open) a repo at `path`. Return a path containing 'unborn' so
  // createRepoState seeds an EMPTY (unborn) repo — honest: init makes a
  // brand-new repo with no commits.
  async initRepo(path: string): Promise<string> {
    await delay(150);
    return `${path}/new-unborn-repo`;
  },

  closeRepo(repoId: string): Promise<void> {
    // Idempotent: deleting an unknown/already-closed id is a no-op.
    repos.delete(repoId);
    return Promise.resolve();
  },

  async pickFolder(): Promise<string | null> {
    await delay(150);
    return MOCK_REPO_PATH;
  },

  async getStatus(repoId: string): Promise<StatusSnapshot> {
    await delay(150);
    const state = requireRepo(repoId);
    // Fresh copy so callers can't mutate the fixture between fetches.
    const snapshot = structuredClone(state.status);
    // P17: append the model-derived src/main.rs rows. It shows in `staged` when
    // the index differs from HEAD, and in `unstaged` when the workdir differs
    // from the index — so a partial stage/unstage can put it in BOTH sections.
    const { head, index, workdir } = state.mainRs;
    if (!linesEqual(index, head)) {
      snapshot.staged.push({ path: MAIN_RS_PATH, origPath: null, status: 'modified' });
      sortByPath(snapshot.staged);
    }
    if (!linesEqual(workdir, index)) {
      snapshot.unstaged.push({ path: MAIN_RS_PATH, origPath: null, status: 'modified' });
      sortByPath(snapshot.unstaged);
    }
    return snapshot;
  },

  async stage(repoId: string, paths: string[]): Promise<void> {
    await delay(150);
    const state = requireRepo(repoId);
    for (const entry of takeMatching(state.status.unstaged, paths)) {
      upsert(state.status.staged, entry);
    }
    for (const entry of takeMatching(state.status.untracked, paths)) {
      upsert(state.status.staged, { ...entry, status: 'added' });
    }
    sortByPath(state.status.staged);
  },

  async unstage(repoId: string, paths: string[]): Promise<void> {
    await delay(150);
    const state = requireRepo(repoId);
    for (const entry of takeMatching(state.status.staged, paths)) {
      if (entry.status === 'added') {
        upsert(state.status.untracked, { ...entry, status: 'untracked' });
      } else {
        upsert(state.status.unstaged, entry); // status + origPath preserved
      }
    }
    sortByPath(state.status.unstaged);
    sortByPath(state.status.untracked);
  },

  // P17: partial (line-level) staging is modeled ONLY for the live src/main.rs
  // three-way file. Any other path rejects (mirrors the backend rejecting
  // non-model files). Both mutate `state.mainRs.index` via reconstructLines
  // (the SAME rule as the Rust §2.4 reconstruction), return void, and DO NOT
  // emit repo-changed (the frontend refetches imperatively).
  async stagePartial(
    repoId: string,
    path: string,
    _origPath: string | null,
    selection: LineSelection[],
  ): Promise<void> {
    await delay(150);
    const state = requireRepo(repoId);
    if (path !== MAIN_RS_PATH) {
      const err: AppError = {
        kind: 'other',
        message: 'mock: partial staging is only modeled for src/main.rs',
      };
      throw err;
    }
    const { selAdd, selDel } = collectSelection(selection);
    const { index, workdir } = state.mainRs;
    // Stage: recompute index-vs-workdir and move the selected lines index->workdir.
    const { hunks } = lineDiff(index, workdir, MAIN_RS_PATH, 'modified', false);
    state.mainRs.index = reconstructLines('stage', hunks, index, workdir, selAdd, selDel);
  },

  async unstagePartial(
    repoId: string,
    path: string,
    _origPath: string | null,
    selection: LineSelection[],
  ): Promise<void> {
    await delay(150);
    const state = requireRepo(repoId);
    if (path !== MAIN_RS_PATH) {
      const err: AppError = {
        kind: 'other',
        message: 'mock: partial staging is only modeled for src/main.rs',
      };
      throw err;
    }
    const { selAdd, selDel } = collectSelection(selection);
    const { head, index } = state.mainRs;
    // Unstage: recompute head-vs-index and move the selected lines index->head.
    const { hunks } = lineDiff(head, index, MAIN_RS_PATH, 'modified', false);
    state.mainRs.index = reconstructLines('unstage', hunks, head, index, selAdd, selDel);
  },

  // P28: partial discard — same three-way model, but the WORKDIR moves toward
  // the INDEX (side-substituted 'unstage': old=index, new=workdir). The index
  // is never touched; getStatus derives the unstaged row from workdir !== index,
  // so a full discard clears the row naturally. No repo-changed emit.
  async discardPartial(
    repoId: string,
    path: string,
    _origPath: string | null,
    selection: LineSelection[],
  ): Promise<void> {
    await delay(150);
    const state = requireRepo(repoId);
    if (path !== MAIN_RS_PATH) {
      const err: AppError = {
        kind: 'other',
        message: 'mock: partial discard is only modeled for src/main.rs',
      };
      throw err;
    }
    const { selAdd, selDel } = collectSelection(selection);
    const { index, workdir } = state.mainRs;
    // Discard: recompute index-vs-workdir and revert the selected lines in the
    // workdir toward the index ('unstage' = base NEW side, undo toward OLD).
    const { hunks } = lineDiff(index, workdir, MAIN_RS_PATH, 'modified', false);
    state.mainRs.workdir = reconstructLines('unstage', hunks, index, workdir, selAdd, selDel);
  },

  async commit(repoId: string, message: string): Promise<CommitResult> {
    await delay(150);
    const state = requireRepo(repoId);
    if (message.trim() === '') {
      const err: AppError = { kind: 'emptyMessage', message: 'commit message is empty' };
      throw err;
    }
    // Signature resolution happens before the nothing-to-commit check in the
    // backend (contract §2.4 steps 4→6) — mirror that precedence here. P40:
    // read identity from the config store so setting it in Settings clears
    // this error end-to-end (the `?fixture=noconfig` store starts empty).
    if (!hasIdentity(state.config)) {
      const err: AppError = {
        kind: 'configMissing',
        message:
          'git identity not configured: user.name and user.email are not set. ' +
          'Run: git config --global user.name "Your Name" and ' +
          'git config --global user.email "you@example.com"',
      };
      throw err;
    }
    if (state.status.staged.length === 0) {
      const err: AppError = {
        kind: 'nothingToCommit',
        message: 'nothing to commit (index matches HEAD)',
      };
      throw err;
    }
    state.status.staged = [];
    state.headOid = randomOid();
    // M6 contract §5: bump the current branch's ahead count so the harness
    // gets the natural commit → push story (main: 0/0 → ↑1 → push clears).
    const headBranch = state.branches.local.find((b) => b.name === state.headBranch);
    if (headBranch !== undefined && headBranch.upstream !== null) {
      headBranch.ahead = (headBranch.ahead ?? 0) + 1;
    }
    const summary = message.trim().split('\n', 1)[0] ?? '';
    // P1 contract §3.5: the DEFAULT graph fixture gains a synthetic lane-0 row
    // per mock commit (newest first) so the harness shows the commit on top.
    state.commits.unshift({ oid: state.headOid, summary });
    return { oid: state.headOid, summary, branch: state.headBranch };
  },

  async getWorkdirFileDiff(
    repoId: string,
    path: string,
    origPath: string | null,
    staged: boolean,
    fullContext: boolean,
  ): Promise<FileDiff> {
    await delay(150);
    const state = requireRepo(repoId);
    // P17: src/main.rs is the live three-way model — its diff is computed from
    // the current head/index/workdir arrays (staged => head vs index; else
    // index vs workdir), honoring fullContext (true => one whole-file hunk).
    if (path === MAIN_RS_PATH) {
      const { head, index, workdir } = state.mainRs;
      const fd = staged
        ? lineDiff(head, index, MAIN_RS_PATH, 'modified', fullContext)
        : lineDiff(index, workdir, MAIN_RS_PATH, 'modified', fullContext);
      return structuredClone(fd);
    }
    const base = mockWorkdirDiff(path, origPath, staged);
    return structuredClone(fullContext ? asFullContext(base) : base);
  },

  async getCommitDiff(repoId: string, oid: string): Promise<CommitDiff> {
    await delay(150);
    const state = requireRepo(repoId);
    // Route by row index of the ACTIVE fixture layout (contract §5: robust
    // against oid spelling; 20k rows fall through to the generic diff).
    const layout =
      state.graphFixture === '20k'
        ? generateLayout20k()
        : state.graphFixture === 'detached'
          ? buildMockGraphDetached()
          : prependCommits(buildMockGraph(), state.commits);
    const index = layout.nodes.findIndex((n) => n.id === oid);
    if (index === -1) {
      const err: AppError = { kind: 'git', message: 'mock: unknown commit' };
      throw err;
    }
    return structuredClone(mockCommitDiff(index, oid));
  },

  async getCommitFileDiff(
    repoId: string,
    oid: string,
    path: string,
    origPath: string | null,
    fullContext: boolean,
  ): Promise<FileDiff> {
    await delay(150);
    requireRepo(repoId);
    // Commit diffs are read-only: honor fullContext with the best-effort collapse.
    const base = mockCommitFileDiff(oid, path, origPath);
    return structuredClone(fullContext ? asFullContext(base) : base);
  },

  async compareWithHead(repoId: string, oid: string): Promise<CompareDiff> {
    await delay(150);
    const state = requireRepo(repoId);
    // Route by row index of the ACTIVE fixture layout, exactly like getCommitDiff.
    const layout =
      state.graphFixture === '20k'
        ? generateLayout20k()
        : state.graphFixture === 'detached'
          ? buildMockGraphDetached()
          : prependCommits(buildMockGraph(), state.commits);
    // P6: a ref whose tip IS HEAD (e.g. origin/main == main, tip === headOid)
    // compares HEAD-to-itself → "No differences". Handled up front because
    // branch tips are intentionally decoupled from graph-row ids in the mock,
    // so headOid need not appear as a walkable node.
    if (oid === state.headOid) {
      return structuredClone(mockCompareDiff(state.headOid, oid, 0, layout));
    }
    const index = layout.nodes.findIndex((n) => n.id === oid);
    if (index === -1) {
      const err: AppError = { kind: 'git', message: 'mock: unknown commit' };
      throw err;
    }
    // OLD = HEAD (state.headOid), NEW = the right-clicked commit oid.
    return structuredClone(mockCompareDiff(state.headOid, oid, index, layout));
  },

  async compareWithHeadFileDiff(
    repoId: string,
    oid: string,
    path: string,
    origPath: string | null,
    fullContext: boolean,
  ): Promise<FileDiff> {
    await delay(150);
    requireRepo(repoId);
    // A compare file diff has the same FileDiff shape — reuse the commit builder.
    // Read-only: honor fullContext with the best-effort collapse.
    const base = mockCommitFileDiff(oid, path, origPath);
    return structuredClone(fullContext ? asFullContext(base) : base);
  },

  async getGraph(repoId: string): Promise<GraphLayout> {
    await delay(150);
    const state = requireRepo(repoId);
    // Built fresh per call (timestamps relative to now; callers own the copy).
    if (state.graphFixture === '20k') return generateLayout20k();
    if (state.graphFixture === 'detached') return buildMockGraphDetached();
    // Default fixture: synthetic mock-commit rows prepended (P1 §3.5), then the
    // live stash stack injected as offshoot nodes (P10 §3.3) so create/apply/
    // pop/drop reflect visually on the next repo-changed refetch.
    const base = prependCommits(buildMockGraph(), state.commits);
    return withStashNodes(base, state.stashes);
  },

  async listBranches(repoId: string): Promise<BranchesSnapshot> {
    await delay(150);
    const state = requireRepo(repoId);
    const snapshot = structuredClone(state.branches);
    if (state.kind === 'detached') {
      snapshot.head = { branchName: null, oid: state.headOid, detached: true, unborn: false };
      for (const branch of snapshot.local) branch.isHead = false;
    } else {
      snapshot.head = {
        branchName: state.headBranch,
        oid: state.headOid,
        detached: false,
        unborn: false,
      };
    }
    return snapshot;
  },

  async createBranch(repoId: string, name: string): Promise<void> {
    await delay(150);
    const state = requireRepo(repoId);
    if (isInvalidBranchName(name)) {
      const err: AppError = { kind: 'invalidName', message: `invalid branch name: '${name}'` };
      throw err;
    }
    const trimmed = name.trim();
    if (state.branches.local.some((b) => b.name === trimmed)) {
      const err: AppError = {
        kind: 'branchExists',
        message: `branch '${trimmed}' already exists`,
      };
      throw err;
    }
    state.branches.local.push({
      name: trimmed,
      isHead: false,
      upstream: null,
      ahead: null,
      behind: null,
      tip: randomOid(),
    });
    state.branches.local.sort((a, b) => a.name.toLowerCase().localeCompare(b.name.toLowerCase()));
  },

  // P11 §1.3: create a local branch at `oid` and check it out, carrying any
  // dirty worktree across. Stateful so the graph HEAD/new-branch pills move on
  // the next refreshAll. `?branch=cbhconflict` exercises the Conflicts toast.
  async createBranchHere(
    repoId: string,
    name: string,
    oid: string,
  ): Promise<CreateBranchHereResult> {
    await delay(250);
    const state = requireRepo(repoId);
    if (isInvalidBranchName(name)) {
      const err: AppError = { kind: 'invalidName', message: `invalid branch name: '${name}'` };
      throw err;
    }
    const trimmed = name.trim();
    if (state.branches.local.some((b) => b.name === trimmed)) {
      const err: AppError = {
        kind: 'branchExists',
        message: `branch '${trimmed}' already exists`,
      };
      throw err;
    }
    const s = state.status;
    const dirty =
      s.staged.length > 0 ||
      s.unstaged.length > 0 ||
      s.untracked.length > 0 ||
      s.conflicted.length > 0;
    // Add the new branch at `oid` as the checked-out HEAD (unset previous head)
    // + move headBranch/headOid so the graph HEAD pill follows on refreshAll.
    for (const b of state.branches.local) b.isHead = false;
    state.branches.local.push({
      name: trimmed,
      isHead: true,
      upstream: null,
      ahead: null,
      behind: null,
      tip: oid,
    });
    state.branches.local.sort((a, b) => a.name.toLowerCase().localeCompare(b.name.toLowerCase()));
    state.headBranch = trimmed;
    state.headOid = oid;
    state.branches.head = { branchName: trimmed, oid, detached: false, unborn: false };
    if (!dirty) return { stashed: false, apply: null };
    // Simulate carrying work across the switch.
    if (query('branch') === 'cbhconflict') {
      // Carried with markers: worktree stays dirty (synthetic conflict entry)
      // and the stash would be RETAINED — do NOT clear the status.
      upsert(s.conflicted, { path: 'src/app.ts', origPath: null, status: 'conflicted' });
      return { stashed: true, apply: { kind: 'conflicts', paths: ['src/app.ts'] } };
    }
    // Clean carry-over: the changes moved with us — status preserved as-is.
    return { stashed: true, apply: { kind: 'applied' } };
  },

  // P33: dirty-safe switch — auto-stash → switch → auto fast-forward (no fetch)
  // → re-apply stash. Never hard-fails on a dirty tree; a conflicted re-apply is
  // a SUCCESS carrying `apply: {kind:'conflicts'}` (stash RETAINED).
  async checkoutBranch(repoId: string, name: string): Promise<CheckoutResult> {
    await delay(150);
    const state = requireRepo(repoId);
    // P36: deterministic worktree-collision refusal — a reserved fixture branch
    // name simulates the branch being checked out in another worktree.
    if (name === '__wt_locked__') {
      const err: AppError = {
        kind: 'branchCheckedOutElsewhere',
        message: `branch '${name}' is already checked out at '/repo/.worktrees/${name}'`,
      };
      throw err;
    }
    const branch = state.branches.local.find((b) => b.name === name);
    if (branch === undefined) {
      const err: AppError = { kind: 'branchNotFound', message: `branch '${name}' not found` };
      throw err;
    }
    const s = state.status;
    const dirty =
      s.staged.length > 0 ||
      s.unstaged.length > 0 ||
      s.untracked.length > 0 ||
      s.conflicted.length > 0;

    // Move HEAD to the target branch (unset previous head).
    for (const b of state.branches.local) b.isHead = false;
    branch.isHead = true;
    state.headBranch = name;
    state.branches.head = { branchName: name, oid: state.headOid, detached: false, unborn: false };
    // TODO(polish): move the HEAD/branch pills in the mock graph fixture too
    // (contract §5 decision: fixtures stay decoupled from branch state —
    // harness proof is the sidebar dot + header branch name).

    // Auto fast-forward: only when the target tracks an upstream and is strictly
    // behind (behind>0 && ahead==0). `feature/merged-a` is the deterministic FF
    // fixture (ahead 0, behind 3). Diverged (feature/sidebar) or up-to-date
    // (main) → no FF.
    const fastForwarded =
      branch.upstream != null && (branch.behind ?? 0) > 0 && (branch.ahead ?? 0) === 0;
    if (fastForwarded) {
      branch.behind = 0;
    }

    if (!dirty) return { stashed: false, fastForwarded, apply: null };

    // Carried work across the switch. `fix/watcher-debounce` is the designated
    // conflicted re-apply fixture (contract §4.3): worktree stays dirty (stash
    // RETAINED) with a synthetic conflict entry.
    if (name === 'fix/watcher-debounce') {
      upsert(s.conflicted, { path: 'src/app.ts', origPath: null, status: 'conflicted' });
      return { stashed: true, fastForwarded, apply: { kind: 'conflicts', paths: ['src/app.ts'] } };
    }
    // Clean carry-over: the changes moved with us — status preserved as-is.
    return { stashed: true, fastForwarded, apply: { kind: 'applied' } };
  },

  async deleteBranch(repoId: string, name: string): Promise<void> {
    await delay(150);
    const state = requireRepo(repoId);
    const branch = state.branches.local.find((b) => b.name === name);
    if (branch === undefined) {
      const err: AppError = { kind: 'branchNotFound', message: `branch '${name}' not found` };
      throw err;
    }
    if (branch.isHead) {
      const err: AppError = {
        kind: 'git',
        message: `cannot delete '${name}': it is the currently checked-out branch`,
      };
      throw err;
    }
    // Designated unmerged branch (contract §5).
    if (name === 'experiment-unmerged') {
      const err: AppError = {
        kind: 'unmergedBranch',
        message:
          "branch 'experiment-unmerged' is not fully merged into HEAD (tip 1a2b3c4). " +
          'Bonsai v1 does not force-delete; use `git branch -D experiment-unmerged` ' +
          'if you are sure.',
      };
      throw err;
    }
    state.branches.local = state.branches.local.filter((b) => b.name !== name);
  },

  // P6 §3.5: GitKraken-style remote checkout — create/reuse a local tracking
  // branch for the remote-tracking ref and switch to it.
  async checkoutRemoteBranch(repoId: string, name: string): Promise<void> {
    await delay(150);
    const state = requireRepo(repoId);
    const remote = state.branches.remote.find((r) => r.name === name);
    if (remote === undefined) {
      const err: AppError = {
        kind: 'branchNotFound',
        message: `remote-tracking branch '${name}' not found`,
      };
      throw err;
    }
    // Split on the FIRST '/' (remote names contain no '/').
    const slash = name.indexOf('/');
    const localName = slash === -1 ? name : name.slice(slash + 1);
    let local = state.branches.local.find((b) => b.name === localName);
    if (local === undefined) {
      // Create-and-track path: new local tracking branch at the remote tip.
      local = { name: localName, isHead: false, upstream: name, ahead: 0, behind: 0, tip: remote.tip };
      state.branches.local.push(local);
      state.branches.local.sort((a, b) => a.name.toLowerCase().localeCompare(b.name.toLowerCase()));
    }
    // Switch HEAD (same state transition as checkoutBranch).
    for (const b of state.branches.local) b.isHead = false;
    local.isHead = true;
    state.headBranch = local.name;
    state.headOid = local.tip;
    state.branches.head = {
      branchName: local.name,
      oid: state.headOid,
      detached: false,
      unborn: false,
    };
  },

  // P6 §3.5: delete the LOCAL remote-tracking ref only (never touches the server).
  async deleteRemoteBranch(repoId: string, name: string): Promise<void> {
    await delay(150);
    const state = requireRepo(repoId);
    const remote = state.branches.remote.find((r) => r.name === name);
    if (remote === undefined) {
      const err: AppError = {
        kind: 'branchNotFound',
        message: `remote-tracking branch '${name}' not found`,
      };
      throw err;
    }
    state.branches.remote = state.branches.remote.filter((r) => r.name !== name);
  },

  // P25 §6.3: stale-branch cleanup. `listStaleBranches` recomputes the report
  // from the live branch list; `deleteBranches` mirrors the server's re-verified
  // safety rules and MUTATES `state.branches.local` for every deleted name so the
  // harness shows rows disappear + a summary toast.
  async listStaleBranches(repoId: string, _base?: string): Promise<StaleReport> {
    await delay(150);
    const state = requireRepo(repoId);
    return buildStaleReport(state);
  },

  async deleteBranches(
    repoId: string,
    names: string[],
    _base?: string,
  ): Promise<BranchDeleteResult[]> {
    await delay(200);
    const state = requireRepo(repoId);
    const report = buildStaleReport(state);
    const safe = new Set(report.branches.map((b) => b.name));
    const currentName = state.kind === 'detached' ? null : state.headBranch;

    const results: BranchDeleteResult[] = names.map((name) => {
      if (name === currentName) {
        return { name, status: 'skippedCurrent' as BranchDeleteStatus, message: 'checked-out branch' };
      }
      if (name === report.base) {
        return { name, status: 'skippedBase' as BranchDeleteStatus, message: 'base branch' };
      }
      if (!safe.has(name)) {
        return {
          name,
          status: 'skippedNotStale' as BranchDeleteStatus,
          message: 'not detected as stale',
        };
      }
      // Safe → remove it from the live branch list (mutating shrink).
      state.branches.local = state.branches.local.filter((b) => b.name !== name);
      return { name, status: 'deleted' as BranchDeleteStatus, message: null };
    });
    return results;
  },

  // Stateful remote mock (M6 contract §5). Failure triggers via `?remote=`
  // (authfail | network | rejected | conflict), composable with `?fixture=`.
  async fetch(repoId: string): Promise<FetchResult> {
    await delay(400);
    const state = requireRepo(repoId);
    if (state.remoteTrigger === 'authfail') throwAuthFailed();
    if (state.remoteTrigger === 'network') throwNetworkError();
    if (!state.fetched) {
      state.fetched = true;
      // The fetch "discovers" one new upstream commit on main.
      const main = state.branches.local.find((b) => b.name === 'main');
      if (main !== undefined && main.upstream !== null) {
        main.behind = 1;
      }
      return { remotes: [{ remote: 'origin', receivedObjects: 12, updatedRefs: 1 }] };
    }
    return { remotes: [{ remote: 'origin', receivedObjects: 0, updatedRefs: 0 }] };
  },

  async pull(repoId: string): Promise<PullResult> {
    await delay(400);
    const state = requireRepo(repoId);
    if (state.remoteTrigger === 'authfail') throwAuthFailed();
    if (state.remoteTrigger === 'network') throwNetworkError();
    if (state.remoteTrigger === 'conflict') {
      const err: AppError = {
        kind: 'checkoutConflict',
        message:
          'cannot pull: local changes would be overwritten by the update. ' +
          'Commit or discard them first.',
      };
      throw err;
    }
    const branch = state.branches.local.find((b) => b.name === state.headBranch);
    if (branch === undefined) {
      // Detached fixture etc. — button is disabled anyway; stay inert.
      return { kind: 'upToDate' };
    }
    if (branch.upstream === null) {
      const err: AppError = {
        kind: 'noUpstream',
        message: `cannot pull: branch '${branch.name}' has no upstream configured`,
      };
      throw err;
    }
    const ahead = branch.ahead ?? 0;
    const behind = branch.behind ?? 0;
    if (ahead > 0 && behind > 0) {
      // Would not fast-forward: change NOTHING (fetch already "happened").
      return { kind: 'wouldNotFastForward', branch: branch.name, ahead, behind };
    }
    if (behind > 0) {
      const from = state.headOid;
      state.headOid = randomOid();
      branch.behind = 0;
      return { kind: 'fastForwarded', branch: branch.name, from, to: state.headOid };
    }
    return { kind: 'upToDate' };
  },

  async push(repoId: string): Promise<PushResult> {
    await delay(400);
    const state = requireRepo(repoId);
    if (state.remoteTrigger === 'authfail') throwAuthFailed();
    if (state.remoteTrigger === 'network') throwNetworkError();
    if (state.remoteTrigger === 'rejected') {
      const err: AppError = {
        kind: 'pushRejected',
        message:
          'push rejected: the remote contains commits you do not have. ' +
          'Fetch/pull first — Bonsai v1 never force-pushes.',
      };
      throw err;
    }
    const branch = state.branches.local.find((b) => b.name === state.headBranch);
    if (branch === undefined) {
      return { kind: 'upToDate', remote: 'origin', branch: state.headBranch };
    }
    if (branch.upstream === null) {
      // First push of a new branch: push to origin/<name> AND set upstream.
      branch.upstream = `origin/${branch.name}`;
      branch.ahead = 0;
      branch.behind = 0;
      if (!state.branches.remote.some((r) => r.name === branch.upstream)) {
        state.branches.remote.push({ name: `origin/${branch.name}`, tip: branch.tip });
        state.branches.remote.sort((a, b) =>
          a.name.toLowerCase().localeCompare(b.name.toLowerCase()),
        );
      }
      return { kind: 'pushed', remote: 'origin', branch: branch.name, setUpstream: true };
    }
    if ((branch.ahead ?? 0) > 0) {
      branch.ahead = 0;
      return { kind: 'pushed', remote: 'origin', branch: branch.name, setUpstream: false };
    }
    return { kind: 'upToDate', remote: 'origin', branch: branch.name };
  },

  // P37: force-push the current branch WITH A LEASE. `?remote=leasefail` drives
  // the refusal path (the remote moved since the last fetch); otherwise the
  // lease holds and the remote-tracking tip advances to the local tip.
  async forcePush(repoId: string): Promise<PushResult> {
    await delay(400);
    const state = requireRepo(repoId);
    if (state.remoteTrigger === 'authfail') throwAuthFailed();
    if (state.remoteTrigger === 'network') throwNetworkError();
    if (state.remoteTrigger === 'leasefail') {
      const err: AppError = {
        kind: 'pushRejected',
        message:
          "force-push refused: 'origin/" +
          state.headBranch +
          "' has moved on the remote since you last fetched — someone may have pushed. " +
          'Fetch and review before force-pushing again.',
      };
      throw err;
    }
    const branch = state.branches.local.find((b) => b.name === state.headBranch);
    if (branch === undefined || branch.upstream === null) {
      const err: AppError = { kind: 'noUpstream', message: 'cannot force-push: no upstream' };
      throw err;
    }
    // Lease held: force-update the remote-tracking tip to the local tip.
    branch.ahead = 0;
    branch.behind = 0;
    const rt = state.branches.remote.find((r) => r.name === branch.upstream);
    if (rt !== undefined) rt.tip = branch.tip;
    return { kind: 'pushed', remote: 'origin', branch: branch.name, setUpstream: false };
  },

  // Stateful op-state mock (P3c contract §7.2). A repo seeded with a merge/rebase
  // (via `?op=` or a path substring) starts paused; mergeBranch/rebaseBranch are
  // the clean-op demo paths.
  async getOpState(repoId: string): Promise<RepoOpState> {
    await delay(150);
    const state = requireRepo(repoId);
    return structuredClone(state.opState);
  },

  async mergeBranch(repoId: string, name: string): Promise<MergeOutcome> {
    await delay(150);
    const state = requireRepo(repoId);
    if (state.opState.kind !== 'none') {
      const err: AppError = {
        kind: 'operationInProgress',
        message: 'an operation is already in progress — commit or abort it first',
      };
      throw err;
    }
    // P8 demo triggers (keyed on the branch name, like the `?op=` convention)
    // so the browser harness can exercise every new outcome shape:
    //   "stash-conflict" -> stashPopConflicts (repo stays clean, no graph mutation)
    //   "conflict"       -> paused merge with an autostash retained on the stack
    //   "autostash"      -> clean merge that stashed and restored local changes
    if (name.includes('stash-conflict')) {
      return { kind: 'stashPopConflicts', head: randomOid(), paths: ['src/app.ts'] };
    }
    if (name.includes('conflict')) {
      return { kind: 'conflicts', paths: ['src/app.ts', 'README.md'], stashed: true };
    }
    const stashed = name.includes('autostash');
    // Clean-merge demo: auto-committed 2-parent node on top of the graph.
    state.headOid = randomOid();
    state.commits.unshift({
      oid: state.headOid,
      summary: `Merge branch '${name}'`,
      mergeParentBase: 1, // the 'feat' fixture tip
    });
    const headBranch = state.branches.local.find((b) => b.name === state.headBranch);
    if (headBranch !== undefined && headBranch.upstream !== null) {
      headBranch.ahead = (headBranch.ahead ?? 0) + 1;
    }
    return { kind: 'merged', oid: state.headOid, stashed };
  },

  async commitMerge(repoId: string, message: string): Promise<CommitResult> {
    await delay(150);
    const state = requireRepo(repoId);
    if (state.opState.kind !== 'merge') {
      const err: AppError = { kind: 'noOperationInProgress', message: 'no merge in progress' };
      throw err;
    }
    if (state.conflicts.length > 0) {
      const err: AppError = {
        kind: 'unresolvedConflicts',
        message: `cannot commit: ${state.conflicts.length} unresolved conflict(s) remain`,
      };
      throw err;
    }
    if (message.trim() === '') {
      const err: AppError = { kind: 'emptyMessage', message: 'commit message is empty' };
      throw err;
    }
    state.opState = { kind: 'none' };
    state.status.conflicted = [];
    state.headOid = randomOid();
    const summary = message.trim().split('\n', 1)[0] ?? '';
    // Faithful twin: a visible 2-parent merge node on top of the graph
    // (second parent = the 'feat' fixture tip, base row 1).
    state.commits.unshift({ oid: state.headOid, summary, mergeParentBase: 1 });
    const headBranch = state.branches.local.find((b) => b.name === state.headBranch);
    if (headBranch !== undefined && headBranch.upstream !== null) {
      headBranch.ahead = (headBranch.ahead ?? 0) + 1;
    }
    return { oid: state.headOid, summary, branch: state.headBranch };
  },

  async abortMerge(repoId: string): Promise<void> {
    await delay(150);
    const state = requireRepo(repoId);
    if (state.opState.kind !== 'merge') {
      const err: AppError = { kind: 'noOperationInProgress', message: 'no merge in progress' };
      throw err;
    }
    // Restore the pre-merge state.
    state.opState = { kind: 'none' };
    state.conflicts = [];
    state.conflictTexts = new Map();
    state.status.conflicted = [];
  },

  async listConflicts(repoId: string): Promise<ConflictEntry[]> {
    await delay(150);
    const state = requireRepo(repoId);
    return structuredClone(state.conflicts);
  },

  async getConflict(repoId: string, path: string): Promise<ConflictFile> {
    await delay(150);
    const state = requireRepo(repoId);
    const file = state.conflictTexts.get(path);
    if (file === undefined) {
      const err: AppError = { kind: 'git', message: `path '${path}' has no conflict` };
      throw err;
    }
    return structuredClone(file);
  },

  async resolveConflict(
    repoId: string,
    path: string,
    resolution: ConflictResolution,
  ): Promise<void> {
    await delay(150);
    const state = requireRepo(repoId);
    const entry = state.conflicts.find((c) => c.path === path);
    if (entry === undefined) {
      const err: AppError = { kind: 'git', message: `path '${path}' has no conflict` };
      throw err;
    }
    state.conflicts = state.conflicts.filter((c) => c.path !== path);
    state.conflictTexts.delete(path);
    state.status.conflicted = state.status.conflicted.filter((e) => e.path !== path);
    // Taking THEIRS on a deletedByThem conflict accepts their deletion: the
    // file shows up as a staged deletion in the mock lists (contract §7.2).
    if (resolution === 'theirs' && entry.kind === 'deletedByThem') {
      upsert(state.status.staged, { path, origPath: null, status: 'deleted' });
      sortByPath(state.status.staged);
    }
  },

  async resolveConflictText(repoId: string, path: string, content: string): Promise<void> {
    // Backend writes `content` verbatim + stages it (P12 §1.2); the mock only
    // mirrors the resulting state change (the text editor runs for text kinds,
    // so no deletedByThem special-case is needed here).
    void content;
    await delay(150);
    const state = requireRepo(repoId);
    const entry = state.conflicts.find((c) => c.path === path);
    if (entry === undefined) {
      const err: AppError = { kind: 'git', message: `path '${path}' has no conflict` };
      throw err;
    }
    state.conflicts = state.conflicts.filter((c) => c.path !== path);
    state.conflictTexts.delete(path);
    state.status.conflicted = state.status.conflicted.filter((e) => e.path !== path);
  },

  // P13: cheap CLI health probe. `?ai=off` simulates no claude on PATH; never
  // rejects for CLI state (matches the backend's never-Err check_availability).
  async checkAiAvailability(): Promise<AiAvailability> {
    await delay(150);
    if (AI_OFF) {
      return {
        installed: false,
        loggedIn: false,
        version: null,
        detail: 'Claude Code CLI not found on PATH',
      };
    }
    return {
      installed: true,
      loggedIn: true,
      version: '2.1.220',
      detail: 'Claude Code 2.1.220 ready',
    };
  },

  // P13: propose an AI resolution for one conflicted path. Writes NOTHING — the
  // apply step is the existing resolveConflictText (P12). Only text-mergeable
  // kinds (bothModified/bothAdded) are eligible; anything else → aiFailed.
  async aiResolveConflict(repoId: string, path: string): Promise<AiResolveProposal> {
    await delay(600);
    const state = requireRepo(repoId);
    const entry = state.conflicts.find((c) => c.path === path);
    if (entry === undefined || (entry.kind !== 'bothModified' && entry.kind !== 'bothAdded')) {
      const err: AppError = { kind: 'aiFailed', message: 'AI resolution unavailable for this file' };
      throw err;
    }
    const file = state.conflictTexts.get(path);
    // Derive a plausible markerless body from the seeded marker fixture. Do NOT
    // mutate state: the proposal is only applied when the caller feeds it to
    // resolveConflictText (ProposeReview accept / AutoResolve).
    const proposedText = file !== undefined ? stripConflictMarkers(file.text) : '';
    return { path, proposedText, costUsd: 0.012 };
  },

  // P15a: propose a commit message from the staged diff. Writes NOTHING — the
  // caller drops the text into the commit box to edit before committing. `?ai=off`
  // simulates a missing CLI; an empty staged set → nothingToCommit (no CLI call).
  async generateCommitMessage(repoId: string): Promise<CommitMessageProposal> {
    await delay(500);
    const state = requireRepo(repoId);
    if (AI_OFF) {
      const err: AppError = {
        kind: 'aiFailed',
        message: 'Claude Code CLI not found on PATH',
      };
      throw err;
    }
    if (state.status.staged.length === 0) {
      const err: AppError = {
        kind: 'nothingToCommit',
        message: 'nothing to commit (index matches HEAD)',
      };
      throw err;
    }
    return {
      message:
        'feat(sidebar): add branch summary action\n\n' +
        '- wire ai_summarize_range command\n' +
        '- add context-menu entry',
      costUsd: 0.004,
    };
  },

  // P15b: explain/review a diff target (read-only prose). Writes NOTHING. Does
  // NOT enforce the consent gate (matches aiResolveConflict; the frontend gates
  // the affordances). `?ai=off` simulates a missing CLI; else canned prose keyed
  // on `mode`, with a tiny per-target prefix so the panel shows what was analyzed.
  async aiAnalyzeDiff(
    repoId: string,
    target: AiDiffTarget,
    mode: AiAnalysisMode,
  ): Promise<AiAnalysis> {
    await delay(500);
    requireRepo(repoId);
    if (AI_OFF) {
      const err: AppError = {
        kind: 'aiFailed',
        message: 'Claude Code CLI not found on PATH',
      };
      throw err;
    }
    let prefix = '';
    if (target.kind === 'commit') {
      prefix = `Commit ${target.oid.slice(0, 7)}: `;
    } else if (target.kind === 'workdirFile') {
      prefix = `${target.path}: `;
    } else if (target.kind === 'worktree') {
      prefix = 'Working tree: ';
    } else if (target.kind === 'branch') {
      prefix = `Branch ${target.name} vs main: `;
    }
    // P25 B1: worktree/branch review scopes get canned Review prose so the
    // browser harness exercises the same AiOutputPanel plumbing.
    if (mode === 'review' && target.kind === 'worktree') {
      return {
        text:
          prefix +
          'Review: 3 files changed; the new error path in commands.rs lacks a ' +
          'test; otherwise LGTM.',
        costUsd: 0.006,
      };
    }
    if (mode === 'review' && target.kind === 'branch') {
      return {
        text:
          prefix +
          'Review: the branch adds a focused feature; consider squashing the ' +
          'two fixup commits and adding a test for the new base-resolution path. ' +
          'No correctness concerns spotted.',
        costUsd: 0.006,
      };
    }
    const text =
      mode === 'review'
        ? prefix +
          'Review: no blocking issues. Consider a null-check on the new branch ' +
          'lookup in Sidebar.tsx; the added revwalk is unbounded — confirm the ' +
          'AI_SUMMARY_MAX_COMMITS cap is applied. Style LGTM.'
        : prefix +
          'This change adds a "Summarize branch" context-menu action in the sidebar ' +
          'and a matching ai_summarize_range command that gathers base..target ' +
          'commits plus a diffstat and calls the local Claude CLI.';
    return { text, costUsd: 0.006 };
  },

  // P28: AI "what changed" digest over a selectable range (read-only prose).
  // Writes NOTHING. Does NOT enforce the consent gate (matches aiAnalyzeDiff;
  // the frontend gates the affordance). `?ai=off` simulates a missing CLI;
  // else canned prose keyed on `range.kind`, echoing the range so the harness
  // shows what was digested.
  async aiDigest(repoId: string, range: AiDigestRange): Promise<AiAnalysis> {
    await delay(700);
    requireRepo(repoId);
    if (AI_OFF) {
      const err: AppError = {
        kind: 'aiFailed',
        message: 'Claude Code CLI not found on PATH',
      };
      throw err;
    }
    let text: string;
    if (range.kind === 'betweenRefs') {
      text =
        `Digest ${range.from}..${range.to}: Over this range the team landed the ` +
        'worktrees feature (sidebar section, create dialog, lifecycle commands) and ' +
        'hardened the AI review path; most churn is in src-tauri/src and src/components.';
    } else if (range.kind === 'lastDays') {
      text =
        `Digest, last ${range.days} day(s): Mostly polish — mock-harness fixes and ` +
        'docs updates; one behavioral change in the watcher debounce.';
    } else {
      text =
        `Digest since ${range.oid.slice(0, 7)}: Two workstreams — worktree UX and ` +
        'stale-branch cleanup — plus test scaffolding.';
    }
    return { text, costUsd: 0.01 };
  },

  // P15c: summarize the commits/diff unique to `target` vs `base` (read-only
  // prose). Writes NOTHING. Does NOT enforce the consent gate (matches
  // aiAnalyzeDiff; the frontend gates the affordance). `?ai=off` simulates a
  // missing CLI; else a canned summary echoing base/target.
  async aiSummarizeRange(repoId: string, base: string, target: string): Promise<AiSummary> {
    await delay(500);
    requireRepo(repoId);
    if (AI_OFF) {
      const err: AppError = {
        kind: 'aiFailed',
        message: 'Claude Code CLI not found on PATH',
      };
      throw err;
    }
    return {
      text:
        'This branch introduces the P15 in-app AI features: commit-message ' +
        'generation, explain/review of diffs, and branch/range summaries — three ' +
        'thin consumers of the existing run_claude primitive. No new settings or ' +
        'process code; all read-only.',
      base,
      target,
      commitCount: 3,
      costUsd: 0.008,
    };
  },

  // Stateful rebase mock (P3d contract §7.2). A repo seeded with a rebase starts
  // paused (step 2/3); rebaseBranch is the clean-rebase demo path. Shares
  // opState/conflicts/conflictTexts with merge, now per-repo.
  async rebaseBranch(repoId: string, _onto: string): Promise<RebaseOutcome> {
    await delay(150);
    const state = requireRepo(repoId);
    if (state.opState.kind !== 'none') {
      const err: AppError = {
        kind: 'operationInProgress',
        message: 'an operation is already in progress — commit or abort it first',
      };
      throw err;
    }
    // Clean-rebase demo: replay 3 plain commits atop the graph so they appear.
    // commits[0] is the topmost row = the new HEAD tip, so it carries the oid.
    state.headOid = randomOid();
    state.commits.unshift(
      { oid: state.headOid, summary: 'pick: replayed 3' },
      { oid: randomOid(), summary: 'pick: replayed 2' },
      { oid: randomOid(), summary: 'pick: replayed 1' },
    );
    const headBranch = state.branches.local.find((b) => b.name === state.headBranch);
    if (headBranch !== undefined && headBranch.upstream !== null) {
      headBranch.ahead = (headBranch.ahead ?? 0) + 3;
    }
    return { kind: 'rebased', branch: state.headBranch, head: state.headOid, steps: 3 };
  },

  async rebaseContinue(repoId: string): Promise<RebaseOutcome> {
    await delay(150);
    const state = requireRepo(repoId);
    if (state.opState.kind !== 'rebase') {
      const err: AppError = { kind: 'noOperationInProgress', message: 'no rebase in progress' };
      throw err;
    }
    if (state.conflicts.length > 0) {
      const err: AppError = {
        kind: 'unresolvedConflicts',
        message: `cannot continue: ${state.conflicts.length} unresolved conflict(s) remain`,
      };
      throw err;
    }
    // P23b: an interactive rebase finishes by prepending its rewritten commits.
    if (state.interactive !== null) {
      return finishInteractiveRebase(state, false);
    }
    // Advance the current step (so a mid-call getOpState would reflect it), then
    // finish: the seeded demo has no further conflict, so a single continue
    // completes the remaining steps (2/3 → done).
    const totalSteps = state.opState.totalSteps;
    state.opState = { ...state.opState, currentStep: state.opState.currentStep + 1 };
    return finishRebase(state, totalSteps);
  },

  async rebaseSkip(repoId: string): Promise<RebaseOutcome> {
    await delay(150);
    const state = requireRepo(repoId);
    if (state.opState.kind !== 'rebase') {
      const err: AppError = { kind: 'noOperationInProgress', message: 'no rebase in progress' };
      throw err;
    }
    // P23b: skip an interactive op — drop the current (conflicting) op, finish.
    if (state.interactive !== null) {
      return finishInteractiveRebase(state, true);
    }
    // Skip is allowed WITH conflicts — dropping the offending commit resolves it.
    const totalSteps = state.opState.totalSteps;
    state.conflicts = [];
    state.conflictTexts = new Map();
    state.status.conflicted = [];
    state.opState = { ...state.opState, currentStep: state.opState.currentStep + 1 };
    return finishRebase(state, totalSteps);
  },

  async rebaseAbort(repoId: string): Promise<void> {
    await delay(150);
    const state = requireRepo(repoId);
    if (state.opState.kind !== 'rebase') {
      const err: AppError = { kind: 'noOperationInProgress', message: 'no rebase in progress' };
      throw err;
    }
    // Abort rewinds: restore the pre-rebase state, prepend NOTHING. For an
    // interactive rebase this also drops the pending plan (the branch ref was
    // never moved in the real engine).
    state.opState = { kind: 'none' };
    state.conflicts = [];
    state.conflictTexts = new Map();
    state.status.conflicted = [];
    state.interactive = null;
  },

  // P23b §7.2: interactive-rebase plan seed + start. getInteractivePlan returns
  // the commits `baseOid..HEAD` of the active mock layout as all-`pick` todos
  // (oldest-first). startInteractiveRebase applies the plan deterministically
  // (see applyInteractivePlan) and either finishes immediately (rewritten
  // commits prepended) or pauses on a conflict that drives the EXISTING OpBanner.
  async getInteractivePlan(repoId: string, baseOid: string): Promise<RebaseTodoOp[]> {
    await delay(150);
    const state = requireRepo(repoId);
    const layout =
      state.graphFixture === '20k'
        ? generateLayout20k()
        : state.graphFixture === 'detached'
          ? buildMockGraphDetached()
          : prependCommits(buildMockGraph(), state.commits);
    const idx = layout.nodes.findIndex((n) => n.id === baseOid);
    if (idx === -1) {
      const err: AppError = { kind: 'git', message: 'mock: base commit is not in the graph' };
      throw err;
    }
    if (idx === 0) {
      const err: AppError = {
        kind: 'git',
        message: `nothing to rebase: ${baseOid.slice(0, 7)} is HEAD`,
      };
      throw err;
    }
    // Rows above the base (indices 0..idx-1) are newer than it; the replayed
    // range base..HEAD in execution (oldest-first) order is those rows reversed.
    // Cap at the 3 nearest the base for a compact editor.
    const slice = layout.nodes.slice(Math.max(0, idx - 3), idx);
    const oldestFirst = slice.slice().reverse();
    return oldestFirst.map((n) => ({ oid: n.id, action: 'pick', newMessage: null }));
  },

  async startInteractiveRebase(
    repoId: string,
    ontoOid: string,
    todos: RebaseTodoOp[],
  ): Promise<RebaseOutcome> {
    await delay(150);
    const state = requireRepo(repoId);
    if (state.opState.kind !== 'none') {
      const err: AppError = {
        kind: 'operationInProgress',
        message: 'an operation is already in progress — commit or abort it first',
      };
      throw err;
    }
    const kept = todos.filter((t) => t.action !== 'drop');
    if (kept.length === 0) {
      const err: AppError = {
        kind: 'git',
        message: 'nothing to rebase: the plan drops every commit',
      };
      throw err;
    }
    if (kept[0].action !== 'pick' && kept[0].action !== 'reword') {
      const err: AppError = { kind: 'git', message: 'a squash/fixup must follow a pick' };
      throw err;
    }
    const layout =
      state.graphFixture === '20k'
        ? generateLayout20k()
        : state.graphFixture === 'detached'
          ? buildMockGraphDetached()
          : prependCommits(buildMockGraph(), state.commits);
    const summaryOf = (oid: string): string =>
      layout.nodes.find((n) => n.id === oid)?.summary ?? `picked ${oid.slice(0, 7)}`;
    const rewritten = applyInteractivePlan(todos, summaryOf);
    const totalSteps = kept.length;
    // Every replayed commit (base..old-HEAD) is rewritten → remove the originals
    // from the mock commit list so the rewritten set replaces them on finish.
    const originalOids = todos.map((t) => t.oid);

    const conflictTriggered =
      state.interactiveConflictDemo ||
      todos.some(
        (t) => t.action !== 'drop' && t.oid.endsWith(INTERACTIVE_REBASE_CONFLICT_OID_SUFFIX),
      );
    if (conflictTriggered) {
      // Pause on a conflict: seed the merge-conflict fixture + op-state so the
      // EXISTING OpBanner + conflict rows + rebaseContinue/Skip/Abort take over.
      state.opState = {
        kind: 'rebase',
        headName: state.headBranch,
        onto: ontoOid,
        currentStep: 1,
        totalSteps,
      };
      state.conflicts = [
        {
          path: 'src/auth.ts',
          kind: 'bothModified',
          hasBase: true,
          hasOurs: true,
          hasTheirs: true,
        },
      ];
      state.conflictTexts = new Map();
      state.conflictTexts.set('src/auth.ts', {
        path: 'src/auth.ts',
        kind: 'bothModified',
        binary: false,
        tooLarge: false,
        missing: false,
        text: MERGE_AUTH_TEXT,
        ours: MERGE_AUTH_OURS,
        theirs: MERGE_AUTH_THEIRS,
      });
      state.status.conflicted = [{ path: 'src/auth.ts', origPath: null, status: 'conflicted' }];
      state.interactive = {
        headName: state.headBranch,
        ontoOid,
        rewritten,
        originalOids,
        totalSteps,
      };
      return { kind: 'conflicts', paths: ['src/auth.ts'], currentStep: 1, totalSteps };
    }

    // Clean replay: remove the original range commits, then prepend the rewritten
    // commits atop the graph (the top row carries the new HEAD tip) and finish.
    const removedClean = new Set(originalOids);
    state.commits = state.commits.filter((c) => !removedClean.has(c.oid));
    state.headOid = randomOid();
    const prepend =
      rewritten.length > 0
        ? rewritten.map((c, i) => (i === 0 ? { ...c, oid: state.headOid } : c))
        : [];
    if (prepend.length > 0) state.commits.unshift(...prepend);
    const headBranch = state.branches.local.find((b) => b.name === state.headBranch);
    if (headBranch !== undefined && headBranch.upstream !== null) {
      headBranch.ahead = (headBranch.ahead ?? 0) + prepend.length;
    }
    return {
      kind: 'rebased',
      branch: state.headBranch,
      head: state.headOid,
      steps: prepend.length,
    };
  },

  // P39: git bisect — a deterministic binary search over a synthetic candidate
  // chain seeded between the bad and good commits. Progress rides on getOpState
  // (RepoOpState.bisect); mark/skip narrow the window to a `found` result.
  async startBisect(repoId: string, bad: string, good: string[]): Promise<BisectOutcome> {
    await delay(150);
    const state = requireRepo(repoId);
    if (state.opState.kind !== 'none') {
      const err: AppError = {
        kind: 'operationInProgress',
        message: 'an operation is already in progress — commit or abort it first',
      };
      throw err;
    }
    if (good.length === 0 || good[0] === bad) {
      const err: AppError = {
        kind: 'git',
        message: 'nothing to bisect: good and bad must differ',
      };
      throw err;
    }
    // Seed a 6-commit-wide candidate chain: good, s1..s6, bad (oldest→newest).
    const middle = Array.from({ length: 6 }, (_, i) =>
      (i + 1).toString(16).padStart(40, '0'),
    );
    const chain = [good[0], ...middle, bad];
    const mb: MockBisect = {
      chain,
      lo: 0,
      hi: chain.length - 1,
      current: null,
      skipped: [],
      firstBad: null,
    };
    state.bisect = mb;
    return driveMockBisect(state, mb);
  },

  async bisectMark(repoId: string, isGood: boolean): Promise<BisectOutcome> {
    await delay(150);
    const state = requireRepo(repoId);
    const mb = state.bisect;
    if (mb === null || mb.current === null) {
      const err: AppError = { kind: 'noOperationInProgress', message: 'no bisect in progress' };
      throw err;
    }
    if (isGood) {
      mb.lo = mb.current;
    } else {
      mb.hi = mb.current;
    }
    return driveMockBisect(state, mb);
  },

  async bisectSkip(repoId: string): Promise<BisectOutcome> {
    await delay(150);
    const state = requireRepo(repoId);
    const mb = state.bisect;
    if (mb === null || mb.current === null) {
      const err: AppError = { kind: 'noOperationInProgress', message: 'no bisect in progress' };
      throw err;
    }
    const oid = mb.chain[mb.current];
    if (!mb.skipped.includes(oid)) mb.skipped.push(oid);
    return driveMockBisect(state, mb);
  },

  async bisectReset(repoId: string): Promise<void> {
    await delay(150);
    const state = requireRepo(repoId);
    if (state.bisect === null) {
      const err: AppError = { kind: 'noOperationInProgress', message: 'no bisect in progress' };
      throw err;
    }
    state.opState = { kind: 'none' };
    state.bisect = null;
  },

  // P23d §10.2: per-line blame + per-file commit history. Canned fixtures for
  // the designated path `src/app.ts` attributed to the deterministic mock graph
  // commit oids (see fixtures/graph.ts `oid(row)`), so clicking a gutter block /
  // history row reveals a REAL node in the graph. Any other path rejects
  // (blame) / returns [] (history), matching the backend contract.
  async blameFile(repoId: string, path: string, _atOid: string | null): Promise<BlameLine[]> {
    await delay(150);
    requireRepo(repoId);
    if (!BLAME_FIXTURE_PATHS.has(path)) {
      const err: AppError = { kind: 'git', message: `mock: no blame fixture for ${path}` };
      throw err;
    }
    return structuredClone(MOCK_BLAME);
  },

  async fileHistory(repoId: string, path: string, limit: number): Promise<FileHistoryEntry[]> {
    await delay(150);
    requireRepo(repoId);
    if (!BLAME_FIXTURE_PATHS.has(path)) return [];
    return structuredClone(MOCK_FILE_HISTORY).slice(0, Math.max(0, limit) || MOCK_FILE_HISTORY.length);
  },

  // P38: reflog read (stateful-read, mirrors fileHistory). HEAD returns the
  // seeded recovery story; a known local branch returns its reflog; any other
  // ref → [] (never-updated ref), matching the backend contract.
  async readReflog(repoId: string, refName: string): Promise<ReflogEntry[]> {
    await delay(120);
    requireRepo(repoId);
    if (refName === 'HEAD') return structuredClone(MOCK_HEAD_REFLOG);
    const branch = MOCK_BRANCH_REFLOGS[refName];
    return branch ? structuredClone(branch) : [];
  },

  // P40: stateful config store per repo (Local | Global). Validation mirrors
  // the Rust §4.5 shape checks so the harness exercises client + server-shaped
  // `invalidName` errors identically.
  async getConfig(repoId: string, level: ConfigLevelArg): Promise<ConfigView> {
    await delay(80);
    const state = requireRepo(repoId);
    return buildConfigView(state.config, level);
  },

  async setConfig(
    repoId: string,
    level: ConfigLevelArg,
    key: string,
    value: string,
  ): Promise<void> {
    await delay(80);
    const state = requireRepo(repoId);
    validateKeyOrThrow(key);
    validateEnumOrThrow(key, value);
    state.config[level][key.trim()] = value.trim();
  },

  async unsetConfig(repoId: string, level: ConfigLevelArg, key: string): Promise<void> {
    await delay(80);
    const state = requireRepo(repoId);
    validateKeyOrThrow(key);
    delete state.config[level][key.trim()];
  },

  // P44: apply an identity (live in-memory profile fields, NOT a persisted id)
  // to the repo's Local config store, then return the refreshed Local view —
  // full round-trip in the harness. Signing key is written only when set +
  // non-empty (mirrors the Rust core fn); an empty key leaves any existing one
  // untouched.
  async applyIdentityProfile(
    repoId: string,
    userName: string,
    userEmail: string,
    signingKey: string | null,
  ): Promise<ConfigView> {
    await delay(120);
    const state = requireRepo(repoId);
    state.config.local['user.name'] = userName.trim();
    state.config.local['user.email'] = userEmail.trim();
    if (signingKey && signingKey.trim() !== '') {
      state.config.local['user.signingkey'] = signingKey.trim();
    }
    return buildConfigView(state.config, 'local');
  },

  // Stateful stash mock (P9 §6.5). Indices are positional into the mutating
  // stack: every create/pop/drop re-indexes so index 0 stays the most recent.
  async listStashes(repoId: string): Promise<StashEntry[]> {
    await delay(150);
    const state = requireRepo(repoId);
    return structuredClone(state.stashes);
  },

  async createStash(
    repoId: string,
    _message: string | null,
    scope: StashScope,
  ): Promise<CreateStashResult> {
    await delay(150);
    const state = requireRepo(repoId);
    const s = state.status;
    // "Nothing to stash" is scope-specific (mirrors the Rust created:false rule).
    // The mock is file-level coarse: it cannot split a path that is both staged and
    // unstaged; for `staged` it simply clears `staged` and leaves `unstaged` intact.
    const nothing =
      scope === 'staged'
        ? s.staged.length === 0
        : scope === 'all'
          ? s.staged.length === 0 && s.unstaged.length === 0
          : s.staged.length === 0 && s.unstaged.length === 0 && s.untracked.length === 0;
    if (nothing) {
      return { created: false };
    }
    // Push a new stash@{0} and re-index the rest (+1).
    for (const entry of state.stashes) entry.index += 1;
    // P10 §8 risk #3: the new stash's baseOid must match the CURRENT HEAD node's
    // id in the graph so withStashNodes renders a node for it. `state.headOid`
    // (MOCK_OID) does NOT match the default fixture's row-0 id, so derive the
    // head node id from the layout getGraph builds (headIndex is always 0).
    const base = prependCommits(buildMockGraph(), state.commits);
    const headNodeId = base.nodes[base.headIndex ?? 0]?.id ?? state.headOid;
    state.stashes.unshift({
      index: 0,
      message: `WIP on ${state.headBranch}: mock stashed changes`,
      oid: randomOid(),
      baseOid: headNodeId,
      ts: Math.floor(Date.now() / 1000),
    });
    // Post-state per scope: `staged` clears only staged; `all` clears tracked
    // (staged+unstaged) but keeps untracked; `allWithUntracked` clears everything.
    s.staged = [];
    if (scope !== 'staged') s.unstaged = [];
    if (scope === 'allWithUntracked') s.untracked = [];
    return { created: true };
  },

  async applyStash(
    repoId: string,
    index: number,
    skipReserved: boolean,
  ): Promise<ApplyStashOutcome> {
    await delay(150);
    const state = requireRepo(repoId);
    const entry = state.stashes.find((e) => e.index === index);
    // Demo conflict trigger — mirrors the P8 mergeBranch "conflict" convention.
    if (entry !== undefined && entry.message.includes('conflict')) {
      return { kind: 'conflicts', paths: ['src/app.ts'] };
    }
    // Windows reserved-path recovery flow (mirrors the core preflight): first
    // attempt is blocked and applies nothing; retry with skipReserved applies the
    // rest, leaving the stack unchanged either way (apply never drops).
    if (stashHasReserved(entry)) {
      return skipReserved
        ? { kind: 'appliedSkippingReserved', skipped: [...RESERVED_STASH_PATHS] }
        : { kind: 'reservedPaths', paths: [...RESERVED_STASH_PATHS] };
    }
    // Apply leaves the stack unchanged.
    return { kind: 'applied' };
  },

  async popStash(
    repoId: string,
    index: number,
    skipReserved: boolean,
  ): Promise<ApplyStashOutcome> {
    await delay(150);
    const state = requireRepo(repoId);
    const entry = state.stashes.find((e) => e.index === index);
    // Conflict trigger: the entry is RETAINED (libgit2 only drops on clean pop).
    if (entry !== undefined && entry.message.includes('conflict')) {
      return { kind: 'conflicts', paths: ['src/app.ts'] };
    }
    // Reserved-path flow: first attempt blocked; a skipping retry applies the
    // rest but KEEPS the stash (lossless — the reserved blobs live only here).
    if (stashHasReserved(entry)) {
      return skipReserved
        ? { kind: 'appliedSkippingReserved', skipped: [...RESERVED_STASH_PATHS] }
        : { kind: 'reservedPaths', paths: [...RESERVED_STASH_PATHS] };
    }
    // Clean pop: remove the entry, then re-index the survivors.
    state.stashes = state.stashes.filter((e) => e.index !== index);
    state.stashes.forEach((e, i) => (e.index = i));
    return { kind: 'applied' };
  },

  async dropStash(repoId: string, index: number): Promise<void> {
    await delay(150);
    const state = requireRepo(repoId);
    state.stashes = state.stashes.filter((e) => e.index !== index);
    state.stashes.forEach((e, i) => (e.index = i));
  },

  async commitAmend(repoId: string, message: string): Promise<CommitResult> {
    await delay(150);
    const state = requireRepo(repoId);
    if (message.trim() === '') {
      const err: AppError = { kind: 'emptyMessage', message: 'commit message is empty' };
      throw err;
    }
    if (!hasIdentity(state.config)) {
      const err: AppError = {
        kind: 'configMissing',
        message:
          'git identity not configured: user.name and user.email are not set. ' +
          'Run: git config --global user.name "Your Name" and ' +
          'git config --global user.email "you@example.com"',
      };
      throw err;
    }
    // Amend rewrites the tip: new oid, staged content folded in, message-only
    // amend allowed (no nothing-to-commit guard). Replace the top commit's
    // summary in the synthetic lane-0 fixture rows.
    state.status.staged = [];
    state.headOid = randomOid();
    const summary = message.trim().split('\n', 1)[0] ?? '';
    if (state.commits.length > 0) {
      state.commits[0] = { ...state.commits[0], oid: state.headOid, summary };
    } else {
      state.commits.unshift({ oid: state.headOid, summary });
    }
    return { oid: state.headOid, summary, branch: state.headBranch };
  },

  async resetBranch(repoId: string, oid: string, _mode: ResetMode): Promise<void> {
    await delay(150);
    const state = requireRepo(repoId);
    // Visual fidelity: drop synthetic lane-0 rows above the target (inclusive
    // move of HEAD onto `oid`). A plain move of headOid is enough for the
    // harness; guard against an unknown oid by leaving the list untouched.
    const target = state.commits.findIndex((c) => c.oid === oid);
    if (target > 0) {
      state.commits = state.commits.slice(target);
    }
    state.headOid = oid;
  },

  async discardPaths(repoId: string, paths: string[]): Promise<void> {
    await delay(150);
    const state = requireRepo(repoId);
    const drop = new Set(paths);
    state.status.unstaged = state.status.unstaged.filter((e) => !drop.has(e.path));
  },

  // P36: force-discard a mixed set — tracked paths reverted (dropped from
  // unstaged) AND untracked paths deleted (dropped from untracked), mirroring the
  // backend split so the Changes panel reflects a bulk force-discard.
  async discardPathsForce(repoId: string, paths: string[]): Promise<void> {
    await delay(150);
    const state = requireRepo(repoId);
    const drop = new Set(paths);
    state.status.unstaged = state.status.unstaged.filter((e) => !drop.has(e.path));
    state.status.untracked = state.status.untracked.filter((e) => !drop.has(e.path));
  },

  // P20 §5/§8.4 + P47: cherry-pick. An oid ending in the conflict suffix pauses
  // with a conflict (op-state → cherryPick); one ending in the stash-pop suffix
  // commits cleanly but conflicts re-applying the autostash; any other oid
  // commits a new top node. `stashed` mirrors the backend: true when the tracked
  // worktree is dirty (autostash was needed). `message`, when supplied, becomes
  // the new commit's summary (drives the editable-message flow, P47).
  async cherrypickCommit(
    repoId: string,
    oid: string,
    message?: string | null,
  ): Promise<CherrypickOutcome> {
    await delay(150);
    const state = requireRepo(repoId);
    if (state.opState.kind !== 'none') {
      const err: AppError = {
        kind: 'operationInProgress',
        message: 'an operation is already in progress — finish or abort it first',
      };
      throw err;
    }
    const stashed = state.status.unstaged.length > 0 || state.status.staged.length > 0;
    if (oid.endsWith(PICK_REVERT_CONFLICT_OID_SUFFIX)) {
      seedPickRevertConflict(state, 'cherryPick');
      return { kind: 'conflicts', paths: ['src/app.ts'], stashed };
    }
    state.headOid = randomOid();
    const summary =
      message != null && message.length > 0
        ? message.split('\n', 1)[0]
        : `Cherry-pick ${oid.slice(0, 7)}`;
    state.commits.unshift({ oid: state.headOid, summary });
    if (oid.endsWith(STASH_POP_CONFLICT_OID_SUFFIX)) {
      return { kind: 'stashPopConflicts', head: state.headOid, paths: ['src/app.ts'] };
    }
    return { kind: 'committed', oid: state.headOid, stashed };
  },

  async cherrypickContinue(repoId: string): Promise<CherrypickOutcome> {
    await delay(150);
    const state = requireRepo(repoId);
    if (state.opState.kind !== 'cherryPick') {
      const err: AppError = {
        kind: 'noOperationInProgress',
        message: 'no cherry-pick in progress',
      };
      throw err;
    }
    if (state.conflicts.length > 0) {
      const err: AppError = {
        kind: 'unresolvedConflicts',
        message: `cannot continue: ${state.conflicts.length} unresolved conflict(s) remain`,
      };
      throw err;
    }
    state.opState = { kind: 'none' };
    state.status.conflicted = [];
    state.conflictTexts = new Map();
    state.headOid = randomOid();
    state.commits.unshift({ oid: state.headOid, summary: 'Cherry-pick (resolved)' });
    // Continue never re-applies a retained autostash (F5) → stashed: false.
    return { kind: 'committed', oid: state.headOid, stashed: false };
  },

  async cherrypickAbort(repoId: string): Promise<void> {
    await delay(150);
    const state = requireRepo(repoId);
    if (state.opState.kind !== 'cherryPick') {
      const err: AppError = {
        kind: 'noOperationInProgress',
        message: 'no cherry-pick in progress',
      };
      throw err;
    }
    state.opState = { kind: 'none' };
    state.conflicts = [];
    state.conflictTexts = new Map();
    state.status.conflicted = [];
  },

  // P20 §6/§8.4 + P47: revert. Same demo-triggers + autostash plumbing as
  // cherry-pick, but no editable message (revert keeps its deterministic text).
  async revertCommit(repoId: string, oid: string): Promise<RevertOutcome> {
    await delay(150);
    const state = requireRepo(repoId);
    if (state.opState.kind !== 'none') {
      const err: AppError = {
        kind: 'operationInProgress',
        message: 'an operation is already in progress — finish or abort it first',
      };
      throw err;
    }
    const stashed = state.status.unstaged.length > 0 || state.status.staged.length > 0;
    if (oid.endsWith(PICK_REVERT_CONFLICT_OID_SUFFIX)) {
      seedPickRevertConflict(state, 'revert');
      return { kind: 'conflicts', paths: ['src/app.ts'], stashed };
    }
    state.headOid = randomOid();
    state.commits.unshift({ oid: state.headOid, summary: `Revert "${oid.slice(0, 7)}"` });
    if (oid.endsWith(STASH_POP_CONFLICT_OID_SUFFIX)) {
      return { kind: 'stashPopConflicts', head: state.headOid, paths: ['src/app.ts'] };
    }
    return { kind: 'committed', oid: state.headOid, stashed };
  },

  async revertContinue(repoId: string): Promise<RevertOutcome> {
    await delay(150);
    const state = requireRepo(repoId);
    if (state.opState.kind !== 'revert') {
      const err: AppError = { kind: 'noOperationInProgress', message: 'no revert in progress' };
      throw err;
    }
    if (state.conflicts.length > 0) {
      const err: AppError = {
        kind: 'unresolvedConflicts',
        message: `cannot continue: ${state.conflicts.length} unresolved conflict(s) remain`,
      };
      throw err;
    }
    state.opState = { kind: 'none' };
    state.status.conflicted = [];
    state.conflictTexts = new Map();
    state.headOid = randomOid();
    state.commits.unshift({ oid: state.headOid, summary: 'Revert (resolved)' });
    // Continue never re-applies a retained autostash (F5) → stashed: false.
    return { kind: 'committed', oid: state.headOid, stashed: false };
  },

  async revertAbort(repoId: string): Promise<void> {
    await delay(150);
    const state = requireRepo(repoId);
    if (state.opState.kind !== 'revert') {
      const err: AppError = { kind: 'noOperationInProgress', message: 'no revert in progress' };
      throw err;
    }
    state.opState = { kind: 'none' };
    state.conflicts = [];
    state.conflictTexts = new Map();
    state.status.conflicted = [];
  },

  // Stateful submodule mock (P19 §5). init flips uninitialized→upToDate;
  // update brings uninitialized/outOfSync→upToDate; sync is a config no-op.
  async listSubmodules(repoId: string): Promise<SubmoduleInfo[]> {
    await delay(150);
    const state = requireRepo(repoId);
    return structuredClone(state.submodules);
  },

  async initSubmodule(repoId: string, name: string): Promise<void> {
    await delay(150);
    const state = requireRepo(repoId);
    const sub = state.submodules.find((s) => s.name === name);
    // Unknown name → no-op (the mock list is authoritative; unreachable from UI).
    if (sub !== undefined && sub.status === 'uninitialized') {
      sub.status = 'upToDate';
      sub.wtOid = sub.indexOid;
    }
  },

  async updateSubmodule(repoId: string, name: string): Promise<void> {
    await delay(150);
    const state = requireRepo(repoId);
    const sub = state.submodules.find((s) => s.name === name);
    // Init-then-update semantics — clears uninitialized / outOfSync.
    if (sub !== undefined) {
      sub.status = 'upToDate';
      sub.wtOid = sub.indexOid;
    }
  },

  async syncSubmodule(repoId: string, name: string): Promise<void> {
    await delay(150);
    // Sync mutates config (URL propagation), not the listed fields — no-op here.
    // Validate the repo is open (mirrors the real command surface).
    void name;
    requireRepo(repoId);
  },

  // Stateful worktree mock (P27 §5): list + add/remove/lock/unlock over the
  // shared module-level list (all default-kind tabs view one repository, so
  // mutations show up everywhere). Refusal errors mirror the backend's
  // messages so the harness exercises the toast path.
  async listWorktrees(repoId: string): Promise<WorktreeInfo[]> {
    await delay(150);
    const state = requireRepo(repoId);
    const rows = worktreesFor(state);
    // `isCurrent` is per viewing repo: the row whose path this tab has open,
    // falling back to the main row when no row matches.
    const hasMatch = rows.some((w) => w.absPath === state.path);
    return rows.map((w) => ({
      ...structuredClone(w),
      isCurrent: hasMatch ? w.absPath === state.path : w.isMain,
    }));
  },

  async addWorktree(repoId: string, branch: string, name: string): Promise<WorktreeInfo> {
    await delay(150);
    const state = requireRepo(repoId);
    const worktrees = worktreesFor(state);
    // Non-default fixtures have no worktree list — refuse rather than push into
    // a throwaway [] and report a success that listWorktrees never shows.
    if (state.kind !== 'default' || state.graphFixture !== 'default') {
      const err: AppError = {
        kind: 'git',
        message: 'mock: this fixture repo does not support worktrees',
      };
      throw err;
    }
    if (branch.trim() === '') {
      const err: AppError = { kind: 'invalidName', message: 'branch name is empty' };
      throw err;
    }
    // P32 Part A: the slug source is the user-editable `name` (defaults to the
    // branch when blank), NOT the branch. Sanitize, then collision-suffix
    // against existing worktree names. (Branch existence is not enforced — the
    // mock list is authoritative; the real backend rejects unknown branches.)
    const nameSrc = name.trim() === '' ? branch : name;
    const slug = nameSrc
      .replace(/[^A-Za-z0-9._-]+/g, '-')
      .replace(/-{2,}/g, '-')
      .replace(/^[-.]+|[-.]+$/g, '');
    if (slug === '' || slug.includes('..')) {
      const err: AppError = {
        kind: 'invalidName',
        message: `cannot derive a worktree name from '${nameSrc}'`,
      };
      throw err;
    }
    // The branch-uniqueness guard keys off `branch`, independent of `name`.
    if (worktrees.some((w) => w.branch === branch)) {
      const err: AppError = {
        kind: 'git',
        message: `branch '${branch}' is already checked out in another worktree`,
      };
      throw err;
    }
    // Nested per-repo container: `.worktrees/<repo-name>/<leaf>`, where the
    // repo name is the main row's on-disk basename.
    const repoName = worktrees.find((w) => w.isMain)?.name ?? 'repo';
    const taken = new Set(worktrees.map((w) => w.name));
    let leaf = slug;
    for (let i = 2; taken.has(leaf); i += 1) leaf = `${slug}-${i}`;
    const row: WorktreeInfo = {
      name: leaf,
      absPath: `/mock/.worktrees/${repoName}/${leaf}`,
      relPath: null,
      branch,
      headOid: randomOid(),
      locked: false,
      lockReason: null,
      isMain: false,
      isCurrent: false,
      prunable: false,
      valid: true,
    };
    worktrees.push(row);
    return structuredClone(row);
  },

  async removeWorktree(repoId: string, name: string): Promise<void> {
    await delay(150);
    const state = requireRepo(repoId);
    const worktrees = worktreesFor(state);
    const idx = worktrees.findIndex((w) => w.name === name);
    if (idx === -1) {
      const err: AppError = { kind: 'git', message: `worktree '${name}' not found` };
      throw err;
    }
    const wt = worktrees[idx];
    if (wt.isMain) {
      const err: AppError = { kind: 'git', message: 'cannot remove the main worktree' };
      throw err;
    }
    if (wt.absPath === state.path) {
      const err: AppError = {
        kind: 'git',
        message: 'cannot remove the worktree you currently have open',
      };
      throw err;
    }
    if (wt.locked) {
      const err: AppError = { kind: 'git', message: 'worktree is locked; unlock it first' };
      throw err;
    }
    // Dirty is not modeled in the mock — the seeded rows are clean.
    worktrees.splice(idx, 1);
  },

  async lockWorktree(repoId: string, name: string, reason?: string): Promise<void> {
    await delay(150);
    const state = requireRepo(repoId);
    const wt = worktreesFor(state).find((w) => w.name === name);
    if (wt === undefined || wt.isMain) {
      const err: AppError = { kind: 'git', message: `worktree '${name}' not found` };
      throw err;
    }
    if (wt.locked) {
      const err: AppError = { kind: 'git', message: 'worktree is already locked' };
      throw err;
    }
    wt.locked = true;
    const trimmed = reason?.trim() ?? '';
    wt.lockReason = trimmed === '' ? null : trimmed;
  },

  async unlockWorktree(repoId: string, name: string): Promise<void> {
    await delay(150);
    const state = requireRepo(repoId);
    const wt = worktreesFor(state).find((w) => w.name === name);
    if (wt === undefined || wt.isMain) {
      const err: AppError = { kind: 'git', message: `worktree '${name}' not found` };
      throw err;
    }
    if (!wt.locked) {
      const err: AppError = { kind: 'git', message: 'worktree is not locked' };
      throw err;
    }
    wt.locked = false;
    wt.lockReason = null;
  },

  // P32 Part B: copy uncommitted changes into a new worktree. Only the default
  // fixture surfaces candidates; every other fixture returns []. The
  // deterministic seeded conflict is `src/staged-change.ts` (see below), so the
  // harness always exercises the badge + Overwrite/Skip toggle.
  async listCopyCandidates(repoId: string): Promise<CopyCandidate[]> {
    await delay(120);
    const state = requireRepo(repoId);
    if (state.kind !== 'default' || state.graphFixture !== 'default') return [];
    const fixture: CopyCandidate[] = [
      { path: '.claude/skills/new-skill.md', group: 'untracked' },
      { path: '.claude/skills/edited.md', group: 'unstaged' },
      { path: 'src/staged-change.ts', group: 'staged' },
      { path: '.env.local', group: 'ignored' },
    ];
    return structuredClone(fixture);
  },

  async previewWorktreeCopy(
    repoId: string,
    branch: string,
    paths: string[],
  ): Promise<CopyPlanEntry[]> {
    await delay(120);
    requireRepo(repoId);
    if (branch.trim() === '') {
      const err: AppError = { kind: 'branchNotFound', message: 'branch name is empty' };
      throw err;
    }
    // Deterministic conflict: `src/staged-change.ts` (a tracked file the target
    // branch also modified) always conflicts; everything else is clean.
    return paths.map((path) => ({
      path,
      verdict: path === 'src/staged-change.ts' ? 'conflict' : 'clean',
    }));
  },

  async addWorktreeWithChanges(
    repoId: string,
    branch: string,
    name: string,
    selections: CopySelection[],
  ): Promise<WorktreeInfo> {
    // Same guards + row-push as addWorktree; the byte copy is a no-op in the
    // browser mock. `selections` length is observable for the success toast.
    void selections;
    return this.addWorktree(repoId, branch, name);
  },

  // P29: repo health. Static warn-heavy fixture (§7) with a fresh generatedAt
  // per call; repo ids (paths) ending in '-err' flip the stats section to the
  // error envelope so the harness renders one errored section alongside three
  // healthy ones.
  async getRepoHealth(repoId: string): Promise<RepoHealth> {
    await delay(300);
    requireRepo(repoId);
    const health = mockRepoHealth();
    if (repoId.endsWith('-err')) {
      health.stats = { data: null, error: 'simulated slow scan failed', elapsedMs: 1500 };
    }
    return health;
  },

  // Stateful tags mock (P22 §5.3). create/delete mutate state.branches.tags so
  // the sidebar Tags section reflects them after refetchBranches; push is a
  // no-op success (honoring the `?remote=` failure triggers).
  async createTag(
    repoId: string,
    name: string,
    _targetOid: string,
    _message: string | null,
    force: boolean,
  ): Promise<void> {
    await delay(150);
    const state = requireRepo(repoId);
    if (!force && state.branches.tags.includes(name)) {
      const err: AppError = { kind: 'git', message: `tag '${name}' already exists` };
      throw err;
    }
    if (!state.branches.tags.includes(name)) {
      state.branches.tags.push(name);
      state.branches.tags.sort((a, b) => a.toLowerCase().localeCompare(b.toLowerCase()));
    }
  },

  async deleteTag(repoId: string, name: string): Promise<void> {
    await delay(150);
    const state = requireRepo(repoId);
    state.branches.tags = state.branches.tags.filter((t) => t !== name);
  },

  async pushTag(repoId: string, _remote: string, _tagName: string, _force: boolean): Promise<void> {
    await delay(400);
    const state = requireRepo(repoId);
    if (state.remoteTrigger === 'authfail') throwAuthFailed();
    if (state.remoteTrigger === 'network') throwNetworkError();
    if (state.remoteTrigger === 'rejected') {
      const err: AppError = {
        kind: 'pushRejected',
        message:
          'push rejected by remote. Bonsai v1 never force-pushes — fetch/pull first.',
      };
      throw err;
    }
    // Local-only mock: no server, so the push simply succeeds.
  },

  // Stateful remotes mock (P22 §5.3). list/add/remove/rename/set-url mutate a
  // per-repo remotes list; dup/missing throw the appropriate AppError.
  async listRemotes(repoId: string): Promise<RemoteInfo[]> {
    await delay(150);
    const state = requireRepo(repoId);
    return structuredClone(state.remotes);
  },

  async addRemote(repoId: string, name: string, url: string): Promise<void> {
    await delay(150);
    const state = requireRepo(repoId);
    if (state.remotes.some((r) => r.name === name)) {
      const err: AppError = { kind: 'git', message: `remote '${name}' already exists` };
      throw err;
    }
    state.remotes.push({ name, url });
    state.remotes.sort((a, b) => a.name.toLowerCase().localeCompare(b.name.toLowerCase()));
  },

  async removeRemote(repoId: string, name: string): Promise<void> {
    await delay(150);
    const state = requireRepo(repoId);
    if (!state.remotes.some((r) => r.name === name)) {
      const err: AppError = { kind: 'noRemote', message: `remote '${name}' not found` };
      throw err;
    }
    state.remotes = state.remotes.filter((r) => r.name !== name);
    // Mirror libgit2's tracking-ref cleanup: drop `<name>/*` remote-tracking rows.
    state.branches.remote = state.branches.remote.filter(
      (r) => !r.name.startsWith(`${name}/`),
    );
  },

  async renameRemote(repoId: string, name: string, newName: string): Promise<void> {
    await delay(150);
    const state = requireRepo(repoId);
    const entry = state.remotes.find((r) => r.name === name);
    if (entry === undefined) {
      const err: AppError = { kind: 'noRemote', message: `remote '${name}' not found` };
      throw err;
    }
    if (state.remotes.some((r) => r.name === newName)) {
      const err: AppError = { kind: 'git', message: `remote '${newName}' already exists` };
      throw err;
    }
    entry.name = newName;
    state.remotes.sort((a, b) => a.name.toLowerCase().localeCompare(b.name.toLowerCase()));
    // Move the remote-tracking refs `<name>/…` → `<newName>/…`.
    for (const r of state.branches.remote) {
      if (r.name === name || r.name.startsWith(`${name}/`)) {
        r.name = `${newName}${r.name.slice(name.length)}`;
      }
    }
    state.branches.remote.sort((a, b) => a.name.toLowerCase().localeCompare(b.name.toLowerCase()));
  },

  async setRemoteUrl(repoId: string, name: string, url: string): Promise<void> {
    await delay(150);
    const state = requireRepo(repoId);
    const entry = state.remotes.find((r) => r.name === name);
    if (entry === undefined) {
      const err: AppError = { kind: 'noRemote', message: `remote '${name}' not found` };
      throw err;
    }
    entry.url = url;
  },

  async getRecentRepos(): Promise<RecentRepo[]> {
    await delay(150);
    return readRecents();
  },

  async removeRecentRepo(path: string): Promise<RecentRepo[]> {
    await delay(150);
    const list = readRecents().filter((r) => r.path.toLowerCase() !== path.toLowerCase());
    writeRecents(list);
    return list;
  },

  // No backend watcher in the browser harness, but the P30 mock job ticks
  // dispatch repo-changed through this registry (contract §7).
  async onRepoChanged(cb: (p: RepoChangedPayload) => void): Promise<Unsubscribe> {
    repoChangedListeners.add(cb);
    return () => {
      repoChangedListeners.delete(cb);
    };
  },

  // P30: background-job status surface (mock harness §7).
  async getJobStatus(repoId: string): Promise<JobStatus[]> {
    await delay(80);
    requireRepo(repoId);
    const settings = readUiSettings();
    // Reflect the CURRENT config's enabled flags, like the Rust command.
    return seedJobStatuses(repoId).map((s) => ({
      ...s,
      enabled: (s.job === 'autoFetch' ? settings.autoFetch : settings.healthRefresh).enabled,
    }));
  },

  async runJobNow(repoId: string, job: JobKind): Promise<void> {
    await delay(80);
    requireRepo(repoId);
    // Mock runs are instant, so the D10 overlap rejection never triggers here;
    // fire the same synthetic completion the timers use.
    completeMockJobRun(repoId, job);
  },

  async onJobStatusChanged(cb: (p: JobStatusChangedPayload) => void): Promise<Unsubscribe> {
    jobStatusListeners.add(cb);
    return () => {
      jobStatusListeners.delete(cb);
    };
  },

  // Real browser focus event so the harness exercises the refocus-refetch path.
  async onWindowFocus(cb: () => void): Promise<Unsubscribe> {
    window.addEventListener('focus', cb);
    return () => window.removeEventListener('focus', cb);
  },

  async getUiSettings(): Promise<UiSettings> {
    await delay(150);
    return readUiSettings();
  },

  async setUiSettings(patch: UiSettingsPatch): Promise<UiSettings> {
    await delay(150);
    const current = readUiSettings();
    const next: UiSettings = {
      theme: patch.theme ?? current.theme,
      paneWidths:
        patch.paneWidths !== undefined ? clampPaneWidths(patch.paneWidths) : current.paneWidths,
      listView: patch.listView ?? current.listView,
      autoFetch:
        patch.autoFetch !== undefined ? clampAutoFetch(patch.autoFetch) : current.autoFetch,
      healthRefresh:
        patch.healthRefresh !== undefined
          ? clampHealthRefresh(patch.healthRefresh)
          : current.healthRefresh,
      graph: patch.graph !== undefined ? clampGraphPrefs(patch.graph) : current.graph,
      aiEnabled: patch.aiEnabled ?? current.aiEnabled,
      aiConflictAutonomy: patch.aiConflictAutonomy ?? current.aiConflictAutonomy,
      aiConsented: patch.aiConsented ?? current.aiConsented,
      mcpConsented: patch.mcpConsented ?? current.mcpConsented,
      mcpWriteConsented: patch.mcpWriteConsented ?? current.mcpWriteConsented,
      onboardingSeen: patch.onboardingSeen ?? current.onboardingSeen,
      autoCheckUpdates: patch.autoCheckUpdates ?? current.autoCheckUpdates,
      profiles: patch.profiles ?? current.profiles,
    };
    writeUiSettings(next);
    // P30 §7: config round-trip re-arms the synthetic job tick timers.
    applyMockJobTimers(next);
    return next;
  },

  async getSession(): Promise<SessionState> {
    await delay(150);
    return readSession();
  },

  async setSession(session: SessionState): Promise<void> {
    await delay(150);
    writeSession(session);
  },

  // P16: embedded MCP server. No real socket — the harness only proves the
  // Settings UI wiring; canned status mirrors the Rust `McpStatus` shape.
  async setActiveRepo(repoId: string | null): Promise<void> {
    await delay(50);
    mockMcp.activeRepo = repoId;
  },

  async getMcpStatus(): Promise<McpStatus> {
    await delay(100);
    return mcpStatusOf();
  },

  async setMcpEnabled(enabled: boolean): Promise<McpStatus> {
    await delay(150);
    mockMcp.enabled = enabled;
    // Disabling drops the running server's write gate too (a stopped server has
    // no live tools); the setting itself persists via UI settings.
    if (!enabled) mockMcp.allowWrite = false;
    const status = mcpStatusOf();
    // Notify any subscriber, like the backend's `mcp-server-changed` emit.
    for (const cb of mockMcp.listeners) cb(status);
    return status;
  },

  // P16c: flip the write-gate. When the server is running this mirrors the
  // backend BOUNCE (toolCount 14 <-> 34) and re-emits the status; when stopped
  // the flag is remembered so the next enable reflects it.
  async setMcpAllowWrite(allowWrite: boolean): Promise<McpStatus> {
    await delay(150);
    mockMcp.allowWrite = allowWrite;
    const status = mcpStatusOf();
    for (const cb of mockMcp.listeners) cb(status);
    return status;
  },

  async onMcpServerChanged(cb: (s: McpStatus) => void): Promise<Unsubscribe> {
    mockMcp.listeners.add(cb);
    return () => {
      mockMcp.listeners.delete(cb);
    };
  },

  // P16: the harness has no real `claude` CLI, so registration is a no-op that
  // resolves after a short delay (App shows a success toast).
  async registerMcpWithClaude(
    _scope: 'user' | 'local',
    _repoPath: string | null,
  ): Promise<void> {
    await delay(150);
  },

  // P24: AI-asset inventory + drift. Drift is recomputed per call so the
  // optional `canonical` override is demonstrable in the harness.
  async listAiAssets(repoId: string, canonical?: string): Promise<AiAssetInventory> {
    await delay(120);
    const state = requireRepo(repoId);
    return {
      assets: structuredClone(state.inventory.assets),
      drift: recomputeDrift(state.inventory, canonical),
    };
  },

  async readAiAsset(repoId: string, path: string): Promise<AssetContent> {
    await delay(80);
    const state = requireRepo(repoId);
    const content = state.assetContent[path];
    return content !== undefined
      ? { path, exists: true, content }
      : { path, exists: false, content: null };
  },

  // P26: agent-asset (skills / subagents / slash commands) read path.
  async listAgentAssets(repoId: string): Promise<AgentAssetInventory> {
    await delay(100);
    const state = requireRepo(repoId);
    return { assets: sortAgentAssets(structuredClone(state.agentAssets)) };
  },

  async readAgentAsset(
    repoId: string,
    kind: AgentAssetKind,
    name: string,
  ): Promise<AgentAsset> {
    await delay(80);
    const state = requireRepo(repoId);
    requireValidAssetName(name);
    const found = state.agentAssets.find((a) => a.kind === kind && a.name === name);
    if (found) {
      return structuredClone(found);
    }
    // A missing asset resolves to an `exists:false` shell (matches Rust §3).
    return {
      kind,
      name,
      path: agentRelPath(kind, name),
      exists: false,
      frontmatter: [],
      body: '',
      complex: false,
      validation: {
        valid: false,
        issues: [{ severity: 'error', message: 'file does not exist' }],
      },
    };
  },

  // P26b: agent-asset write path (stateful upsert / delete).
  async saveAgentAsset(repoId: string, asset: AgentAssetInput): Promise<AgentAssetInventory> {
    await delay(120);
    const state = requireRepo(repoId);
    // Name guard fires first (throws invalidName on separators / `..` / bad
    // charset) — mirrors the backend; validation issues do NOT block a save.
    requireValidAssetName(asset.name);
    // Structural re-guard (mirrors the Rust save): refuse to overwrite an
    // existing COMPLEX asset — rebuilding from flat fields would drop its YAML.
    const existing = state.agentAssets.find(
      (a) => a.kind === asset.kind && a.name === asset.name,
    );
    if (existing?.complex) {
      const err: AppError = {
        kind: 'other',
        message:
          'cannot overwrite complex YAML frontmatter from the editor — edit this file directly',
      };
      throw err;
    }
    const saved: AgentAsset = {
      kind: asset.kind,
      name: asset.name,
      path: agentRelPath(asset.kind, asset.name),
      exists: true,
      frontmatter: structuredClone(asset.frontmatter),
      body: asset.body,
      // A flat `AgentAssetInput` is never complex (single-line editor values).
      complex: false,
      validation: mockValidateAsset(asset.kind, asset.name, asset.frontmatter, asset.body),
    };
    const idx = state.agentAssets.findIndex(
      (a) => a.kind === asset.kind && a.name === asset.name,
    );
    if (idx >= 0) {
      state.agentAssets[idx] = saved;
    } else {
      state.agentAssets.push(saved);
    }
    return { assets: sortAgentAssets(structuredClone(state.agentAssets)) };
  },

  async deleteAgentAsset(
    repoId: string,
    kind: AgentAssetKind,
    name: string,
  ): Promise<AgentAssetInventory> {
    await delay(120);
    const state = requireRepo(repoId);
    requireValidAssetName(name);
    // Skill vs single-file removal is a no-op distinction in the mock (§6).
    state.agentAssets = state.agentAssets.filter(
      (a) => !(a.kind === kind && a.name === name),
    );
    return { assets: sortAgentAssets(structuredClone(state.agentAssets)) };
  },

  // P24b: context-profile store (stateful CRUD + preview + activate).
  async listProfiles(repoId: string): Promise<ProfileStore> {
    await delay(80);
    const state = requireRepo(repoId);
    return structuredClone(state.profiles);
  },

  async saveProfile(repoId: string, profile: ContextProfile): Promise<ProfileStore> {
    await delay(120);
    const state = requireRepo(repoId);
    // Validate name: blank / leading '-' / path separators / control chars.
    const name = profile.name;
    const badName =
      name.trim() === '' ||
      name.startsWith('-') ||
      name.includes('/') ||
      name.includes('\\') ||
      [...name].some((c) => {
        const code = c.charCodeAt(0);
        // C0 controls, plus DEL and the C1 range (0x7f–0x9f) to match Rust's
        // char::is_control.
        return code < 0x20 || (code >= 0x7f && code <= 0x9f);
      });

    if (badName) {
      const err: AppError = { kind: 'invalidName', message: `invalid profile name: '${name}'` };
      throw err;
    }
    // Every target must be a single-file descriptor id.
    for (const t of profile.targets) {
      if (!(t.assetId in SINGLE_FILE_PATHS)) {
        const err: AppError = {
          kind: 'invalidName',
          message: `invalid profile target asset: '${t.assetId}'`,
        };
        throw err;
      }
    }
    const idx = state.profiles.profiles.findIndex((p) => p.name === name);
    if (idx >= 0) {
      state.profiles.profiles[idx] = structuredClone(profile);
    } else {
      state.profiles.profiles.push(structuredClone(profile));
    }
    return structuredClone(state.profiles);
  },

  async deleteProfile(repoId: string, name: string): Promise<ProfileStore> {
    await delay(100);
    const state = requireRepo(repoId);
    state.profiles.profiles = state.profiles.profiles.filter((p) => p.name !== name);
    if (state.profiles.activeProfile === name) {
      state.profiles.activeProfile = null;
    }
    return structuredClone(state.profiles);
  },

  async previewProfile(repoId: string, name: string): Promise<ProfilePreviewEntry[]> {
    await delay(120);
    const state = requireRepo(repoId);
    const profile = state.profiles.profiles.find((p) => p.name === name);
    if (profile === undefined) {
      const err: AppError = { kind: 'other', message: `profile '${name}' not found` };
      throw err;
    }
    return profile.targets.map((t) => {
      const path = SINGLE_FILE_PATHS[t.assetId] ?? t.assetId;
      const current = state.assetContent[path] ?? null;
      return { assetId: t.assetId, path, current, proposed: t.content, changed: current !== t.content };
    });
  },

  async activateProfile(repoId: string, name: string): Promise<ProfileActivation> {
    await delay(160);
    const state = requireRepo(repoId);
    const profile = state.profiles.profiles.find((p) => p.name === name);
    if (profile === undefined) {
      const err: AppError = { kind: 'other', message: `profile '${name}' not found` };
      throw err;
    }
    const results: TargetWriteResult[] = profile.targets.map((t) => {
      const path = SINGLE_FILE_PATHS[t.assetId] ?? t.assetId;
      const current = state.assetContent[path];
      let action: TargetWriteAction;
      if (current === undefined) {
        action = 'created';
      } else if (current === t.content) {
        action = 'unchanged';
      } else {
        action = 'written';
      }
      if (action !== 'unchanged') {
        applyMockWrite(state, t.assetId, path, t.content);
      }
      return { assetId: t.assetId, path, action };
    });
    // D5: record the activation under the TAB'S OWN worktree key; the legacy
    // `activeProfile` field mirrors only the "@main" entry (P31 D4).
    const key = tabWorktreeKey(state);
    state.profiles.worktreeActivations = {
      ...state.profiles.worktreeActivations,
      [key]: name,
    };
    if (key === '@main') {
      state.profiles.activeProfile = name;
    } else {
      // Keep the shared per-worktree file map in step so the matrix agrees.
      const files = worktreeFilesFor(key);
      for (const t of profile.targets) {
        files[SINGLE_FILE_PATHS[t.assetId] ?? t.assetId] = t.content;
      }
    }
    return { profile: name, results, store: structuredClone(state.profiles) };
  },

  // P31 §6: per-worktree AI contexts. The matrix derives "@main" counts from
  // the tab's stateful inventory (same math as listAiAssets) and linked-row
  // counts from the shared per-worktree file maps; activation mutates ONLY the
  // target worktree's map, so a re-list flips that row's chips alone.
  async listWorktreeContexts(repoId: string): Promise<WorktreeContextStatus[]> {
    await delay(150);
    const state = requireRepo(repoId);
    const rows = worktreesFor(state);
    const hasMatch = rows.some((w) => w.absPath === state.path);
    return rows.map((w) => {
      const key = w.isMain ? '@main' : w.name;
      const activatable = w.valid && !w.prunable && !w.locked;
      let blockedReason: string | null = null;
      if (!w.valid) {
        blockedReason = 'worktree is invalid (working directory missing or broken)';
      } else if (w.prunable) {
        blockedReason = 'worktree is stale (prunable)';
      } else if (w.locked) {
        blockedReason =
          w.lockReason === null ? 'worktree is locked' : `worktree is locked: ${w.lockReason}`;
      }
      let drifted = 0;
      let missing = 0;
      if (activatable) {
        if (w.isMain) {
          const entries = recomputeDrift(state.inventory).entries;
          drifted = entries.filter((e) => e.comparable && e.exists && !e.inSync).length;
          missing = entries.filter((e) => e.comparable && !e.exists).length;
        } else {
          ({ drifted, missing } = worktreeDriftCounts(worktreeFilesFor(key)));
        }
      }
      return {
        worktreeKey: key,
        name: w.name,
        absPath: w.absPath,
        branch: w.branch,
        isMain: w.isMain,
        isCurrent: hasMatch ? w.absPath === state.path : w.isMain,
        locked: w.locked,
        prunable: w.prunable,
        valid: w.valid,
        activeProfile: state.profiles.worktreeActivations?.[key] ?? null,
        driftedCount: drifted,
        missingCount: missing,
        activatable,
        blockedReason,
      };
    });
  },

  async previewWorktreeProfile(
    repoId: string,
    worktreeKey: string,
    name: string,
  ): Promise<ProfilePreviewEntry[]> {
    await delay(120);
    const state = requireRepo(repoId);
    const profile = state.profiles.profiles.find((p) => p.name === name);
    if (profile === undefined) {
      const err: AppError = { kind: 'other', message: `profile '${name}' not found` };
      throw err;
    }
    const row = requireEligibleWorktree(state, worktreeKey); // D6
    const files = row.isMain ? state.assetContent : worktreeFilesFor(worktreeKey);
    return profile.targets.map((t) => {
      const path = SINGLE_FILE_PATHS[t.assetId] ?? t.assetId;
      const current = files[path] ?? null;
      return {
        assetId: t.assetId,
        path,
        current,
        proposed: t.content,
        changed: current !== t.content,
      };
    });
  },

  async activateWorktreeProfile(
    repoId: string,
    worktreeKey: string,
    name: string,
  ): Promise<ProfileActivation> {
    await delay(160);
    const state = requireRepo(repoId);
    const profile = state.profiles.profiles.find((p) => p.name === name);
    if (profile === undefined) {
      const err: AppError = { kind: 'other', message: `profile '${name}' not found` };
      throw err;
    }
    const row = requireEligibleWorktree(state, worktreeKey); // D6
    const files = row.isMain ? state.assetContent : worktreeFilesFor(worktreeKey);
    const results: TargetWriteResult[] = profile.targets.map((t) => {
      const path = SINGLE_FILE_PATHS[t.assetId] ?? t.assetId;
      const current = files[path];
      let action: TargetWriteAction;
      if (current === undefined) {
        action = 'created';
      } else if (current === t.content) {
        action = 'unchanged';
      } else {
        action = 'written';
      }
      if (action !== 'unchanged') {
        if (row.isMain) {
          // Keep the tab's inventory in step so AiAssetsPanel chips flip too.
          applyMockWrite(state, t.assetId, path, t.content);
        } else {
          files[path] = t.content;
        }
      }
      return { assetId: t.assetId, path, action };
    });
    state.profiles.worktreeActivations = {
      ...state.profiles.worktreeActivations,
      [worktreeKey]: name,
    };
    if (worktreeKey === '@main') {
      state.profiles.activeProfile = name; // legacy mirror (D4)
    }
    return { profile: name, results, store: structuredClone(state.profiles) };
  },

  // P24e: translate one instruction file into another agent's flavor. Writes
  // NOTHING — returns canned proposed text after a delay. Gated on the mock's
  // AI-off convention (mirrors generateCommitMessage's AI_OFF handling): `?ai=off`
  // simulates disabled AI → aiUnavailable, else always succeed.
  async aiGenerateAsset(
    repoId: string,
    sourceAssetId: string,
    targetAgent: string,
    guidance?: string,
  ): Promise<AiGeneratedAsset> {
    await delay(500);
    requireRepo(repoId);
    if (AI_OFF) {
      const err: AppError = {
        kind: 'aiUnavailable',
        message: 'AI features are disabled — enable them in Settings',
      };
      throw err;
    }
    const guidanceLine =
      guidance !== undefined && guidance.trim() !== ''
        ? `\n\n> Guidance applied: ${guidance.trim()}`
        : '';
    return {
      targetAgent,
      content:
        `# ${targetAgent} instructions\n\n` +
        `…(translated from \`${sourceAssetId}\`)…\n\n` +
        `Preserve the original guidance; adapt tone and format for ${targetAgent}.` +
        guidanceLine +
        '\n',
    };
  },

  // P42 (INV-2): stateful update seam keyed on `?update=available|none|error`.
  async checkForUpdate(): Promise<UpdateCheckResult> {
    await delay(400);
    if (UPDATE_MODE === 'error') {
      mockUpdateReady = false;
      const err: AppError = {
        kind: 'networkError',
        message: 'mock: could not reach the update endpoint (?update=error)',
      };
      throw err;
    }
    if (UPDATE_MODE === 'available') {
      mockUpdateReady = true;
      return {
        available: true,
        currentVersion: MOCK_CURRENT_VERSION,
        version: MOCK_NEXT_VERSION,
        notes: '- Mock release notes\n- Harness fixture',
        date: '2026-08-04',
      };
    }
    mockUpdateReady = false;
    return {
      available: false,
      currentVersion: MOCK_CURRENT_VERSION,
      version: null,
      notes: null,
      date: null,
    };
  },

  async downloadAndInstallUpdate(onProgress: (p: UpdateProgress) => void): Promise<void> {
    if (!mockUpdateReady) {
      const err: AppError = {
        kind: 'noOperationInProgress',
        message: 'No update found — call checkForUpdate first',
      };
      throw err;
    }
    const contentLength = 5_000_000;
    onProgress({ phase: 'started', downloadedBytes: 0, contentLength });
    const ticks = 15;
    const chunk = Math.ceil(contentLength / ticks);
    let downloadedBytes = 0;
    for (let i = 0; i < ticks; i += 1) {
      await delay(120);
      downloadedBytes = Math.min(contentLength, downloadedBytes + chunk);
      onProgress({ phase: 'downloading', downloadedBytes, contentLength });
    }
    onProgress({ phase: 'finished', downloadedBytes: contentLength, contentLength });
  },

  async relaunchApp(): Promise<void> {
    // No reload — keeps harness state so the flow stays inspectable (D1/INV-2).
    console.info('[mock] relaunch');
  },
};
