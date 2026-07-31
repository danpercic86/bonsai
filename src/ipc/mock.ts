import { INITIAL_BRANCHES, MOCK_OID } from './fixtures/branches';
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
  AgentAssetInventory,
  AgentAssetKind,
  AiAnalysis,
  AiAnalysisMode,
  AiAssetInventory,
  AiAutonomy,
  AiAvailability,
  AiDiffTarget,
  AiGeneratedAsset,
  AiResolveProposal,
  AiSummary,
  AppError,
  ApplyStashOutcome,
  AssetContent,
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
  CommitMessageProposal,
  CommitResult,
  CompareDiff,
  ConflictEntry,
  ConflictFile,
  ConflictResolution,
  CreateBranchHereResult,
  CreateStashResult,
  FetchResult,
  FileDiff,
  FileHistoryEntry,
  AutoFetchSettings,
  GraphLayout,
  GraphPrefs,
  IpcApi,
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
  RemoteInfo,
  RepoChangedPayload,
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
} from './types';
import {
  AUTO_FETCH_INTERVAL_MAX,
  AUTO_FETCH_INTERVAL_MIN,
  AVATAR_RADIUS_MAX,
  AVATAR_RADIUS_MIN,
  DOT_RADIUS_MAX,
  DOT_RADIUS_MIN,
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

const MERGE_AUTH_TEXT = [
  'import { hash } from "./crypto";',
  '',
  'export interface Session {',
  '  user: string;',
  '  token: string;',
  '}',
  '',
  'export function login(user: string, password: string): Session {',
  '<<<<<<< HEAD',
  '  const token = hash(`${user}:${password}:v2`);',
  '  return { user, token };',
  '=======',
  '  const token = hash(password + user);',
  '  return { user: user.toLowerCase(), token };',
  '>>>>>>> feature/login',
  '}',
  '',
  'export function logout(session: Session): void {',
  '  void session;',
  '}',
  '',
].join('\n');

// P12 §1.4: the OURS / THEIRS blob sides for MERGE_AUTH_TEXT's single conflict
// region — the file with the region collapsed to its ours / theirs block
// (markers removed). Hand-written to match MERGE_AUTH_TEXT above.
const MERGE_AUTH_OURS = [
  'import { hash } from "./crypto";',
  '',
  'export interface Session {',
  '  user: string;',
  '  token: string;',
  '}',
  '',
  'export function login(user: string, password: string): Session {',
  '  const token = hash(`${user}:${password}:v2`);',
  '  return { user, token };',
  '}',
  '',
  'export function logout(session: Session): void {',
  '  void session;',
  '}',
  '',
].join('\n');

const MERGE_AUTH_THEIRS = [
  'import { hash } from "./crypto";',
  '',
  'export interface Session {',
  '  user: string;',
  '  token: string;',
  '}',
  '',
  'export function login(user: string, password: string): Session {',
  '  const token = hash(password + user);',
  '  return { user: user.toLowerCase(), token };',
  '}',
  '',
  'export function logout(session: Session): void {',
  '  void session;',
  '}',
  '',
].join('\n');

const MERGE_README_TEXT = [
  '# Bonsai fixture',
  '',
  'Our side kept this README while feature/login deleted it.',
  '',
].join('\n');

// P20 §8.4 demo trigger: a cherry-pick / revert of a commit whose oid ends in
// this suffix pauses with a conflict (mirrors the mergeBranch `name.includes
// ('conflict')` convention, keyed on oid). Any other oid commits cleanly.
const PICK_REVERT_CONFLICT_OID_SUFFIX = 'c0ffee';

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

// ---------------------------------------------------------------------------
// P3e-c: per-repo state. Every stateful flow that used to live in module-level
// singletons now lives inside a MockRepoState, one per open repoId. The map is
// the single source of per-repo truth — there is NO module-level per-repo
// singleton anymore. `openRepo` creates entries lazily; `closeRepo` deletes.
// ---------------------------------------------------------------------------

// P24 — AI-asset fixture (§7). CLAUDE.md / AGENTS.md / copilot exist; AGENTS.md
// is DRIFTED (its own normalized hash) while copilot is IN SYNC with the
// canonical `claude` (shared hash). One detected `.cursor/rules` dir with 2
// members and `.mcp.json` are `managed:false`. Hashes are opaque 40-hex
// placeholders — only their equality matters to the drift math.
const HASH_CLAUDE = 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa';
const HASH_AGENTS = 'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb';
const HASH_RAW = 'cccccccccccccccccccccccccccccccccccccccc';

/** Canned file bodies served by `readAiAsset` for known paths (§7). */
const MOCK_ASSET_CONTENT: Record<string, string> = {
  'CLAUDE.md': '# CLAUDE.md\n\nProject instructions for Claude Code.\n',
  'AGENTS.md': '# AGENTS.md\n\nProject instructions, OpenAI/Codex flavor (drifted).\n',
  '.github/copilot-instructions.md':
    '# CLAUDE.md\n\nProject instructions for Claude Code.\n',
  '.cursor/rules/style.mdc': '---\ndescription: style\n---\n\nUse tabs.\n',
  '.cursor/rules/testing.mdc': '---\ndescription: testing\n---\n\nWrite tests.\n',
  '.mcp.json': '{\n  "mcpServers": {}\n}\n',
};

function mockAssetFile(
  path: string,
  contentHash: string,
  normalizedHash: string,
): { path: string; size: number; contentHash: string; normalizedHash: string; modified: number } {
  const size = MOCK_ASSET_CONTENT[path]?.length ?? 0;
  return { path, size, contentHash, normalizedHash, modified: 1_753_900_000 };
}

/** Seed inventory (drift recomputed on every `listAiAssets`, so `drift` here is
 *  just the initial no-override view). */
const mockInventory: AiAssetInventory = {
  assets: [
    {
      id: 'claude',
      agent: 'Claude Code',
      label: 'CLAUDE.md',
      kind: 'singleFile',
      path: 'CLAUDE.md',
      managed: true,
      exists: true,
      files: [mockAssetFile('CLAUDE.md', HASH_RAW, HASH_CLAUDE)],
    },
    {
      id: 'agents',
      agent: 'Codex/Cursor/Gemini/Zed',
      label: 'AGENTS.md',
      kind: 'singleFile',
      path: 'AGENTS.md',
      managed: true,
      exists: true,
      files: [mockAssetFile('AGENTS.md', HASH_RAW, HASH_AGENTS)],
    },
    {
      id: 'copilot',
      agent: 'GitHub Copilot',
      label: 'copilot-instructions.md',
      kind: 'singleFile',
      path: '.github/copilot-instructions.md',
      managed: true,
      exists: true,
      // Same normalized hash as claude -> IN SYNC.
      files: [mockAssetFile('.github/copilot-instructions.md', HASH_RAW, HASH_CLAUDE)],
    },
    { id: 'gemini', agent: 'Gemini CLI', label: 'GEMINI.md', kind: 'singleFile', path: 'GEMINI.md', managed: true, exists: false, files: [] },
    { id: 'windsurf', agent: 'Windsurf (legacy)', label: '.windsurfrules', kind: 'singleFile', path: '.windsurfrules', managed: true, exists: false, files: [] },
    { id: 'cursorLegacy', agent: 'Cursor (legacy)', label: '.cursorrules', kind: 'singleFile', path: '.cursorrules', managed: true, exists: false, files: [] },
    {
      id: 'cursorRules',
      agent: 'Cursor',
      label: '.cursor/rules/',
      kind: 'rulesDir',
      path: '.cursor/rules',
      managed: true,
      exists: true,
      files: [
        mockAssetFile('.cursor/rules/style.mdc', HASH_RAW, HASH_RAW),
        mockAssetFile('.cursor/rules/testing.mdc', HASH_RAW, HASH_RAW),
      ],
    },
    { id: 'windsurfRules', agent: 'Windsurf', label: '.windsurf/rules/', kind: 'rulesDir', path: '.windsurf/rules', managed: true, exists: false, files: [] },
    { id: 'copilotInstr', agent: 'GitHub Copilot', label: '.github/instructions/', kind: 'rulesDir', path: '.github/instructions', managed: false, exists: false, files: [] },
    { id: 'copilotPrompts', agent: 'GitHub Copilot', label: '.github/prompts/', kind: 'rulesDir', path: '.github/prompts', managed: false, exists: false, files: [] },
    { id: 'claudeDir', agent: 'Claude Code', label: '.claude/ (skills/agents/commands)', kind: 'config', path: '.claude', managed: false, exists: false, files: [] },
    {
      id: 'mcp',
      agent: 'MCP clients',
      label: '.mcp.json',
      kind: 'config',
      path: '.mcp.json',
      managed: false,
      exists: true,
      files: [mockAssetFile('.mcp.json', HASH_RAW, HASH_RAW)],
    },
  ],
  drift: {
    canonicalId: 'claude',
    canonicalHash: HASH_CLAUDE,
    entries: [],
    inSync: false,
  },
};

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

/** Seed agent-asset inventory (§6): one valid asset per kind, one invalid agent
 *  (missing required `description`), and one skill flagged `complex` (multi-line
 *  frontmatter, read-only). Kinds/names are pre-sorted (skill<agent<command). */
const mockAgentAssets: AgentAsset[] = [
  {
    kind: 'skill',
    name: 'code-review',
    path: '.claude/skills/code-review/SKILL.md',
    exists: true,
    frontmatter: [
      { key: 'name', value: 'code-review' },
      { key: 'description', value: 'Reviews a diff for correctness and style' },
    ],
    body: '\n# Code review\n\nReview the staged changes.\n',
    validation: { valid: true, issues: [] },
  },
  {
    kind: 'skill',
    name: 'release-notes',
    path: '.claude/skills/release-notes/SKILL.md',
    exists: true,
    // Multi-line YAML the editor can't round-trip -> read-only Error.
    frontmatter: [{ key: 'name', value: 'release-notes' }],
    body: '\n# Release notes\n\nSummarize merged PRs.\n',
    validation: {
      valid: false,
      issues: [
        {
          severity: 'error',
          message:
            "frontmatter uses multi-line YAML this editor can't safely round-trip — edit the file directly",
        },
      ],
    },
  },
  {
    kind: 'agent',
    name: 'test-runner',
    path: '.claude/agents/test-runner.md',
    exists: true,
    frontmatter: [
      { key: 'name', value: 'test-runner' },
      { key: 'description', value: 'Runs the test suite and triages failures' },
      { key: 'tools', value: 'Bash, Read' },
      { key: 'model', value: 'inherit' },
    ],
    body: '\nYou run the project test suite and report failures.\n',
    validation: { valid: true, issues: [] },
  },
  {
    kind: 'agent',
    name: 'broken',
    path: '.claude/agents/broken.md',
    exists: true,
    // Missing required `description` -> invalid.
    frontmatter: [{ key: 'name', value: 'broken' }],
    body: '\nAn incomplete subagent.\n',
    validation: {
      valid: false,
      issues: [
        { severity: 'error', message: "agent requires frontmatter field 'description'" },
      ],
    },
  },
  {
    kind: 'command',
    name: 'changelog',
    path: '.claude/commands/changelog.md',
    exists: true,
    frontmatter: [
      { key: 'description', value: 'Draft a changelog entry' },
      { key: 'argument-hint', value: '<version>' },
    ],
    body: '\nDraft a changelog entry for $ARGUMENTS.\n',
    validation: { valid: true, issues: [] },
  },
];

/** Deterministic (kind order skill<agent<command, then name) sort, matching
 *  `scan_agent_assets`. */
const AGENT_KIND_ORD: Record<AgentAssetKind, number> = { skill: 0, agent: 1, command: 2 };
function sortAgentAssets(assets: AgentAsset[]): AgentAsset[] {
  return [...assets].sort(
    (a, b) => AGENT_KIND_ORD[a.kind] - AGENT_KIND_ORD[b.kind] || a.name.localeCompare(b.name),
  );
}

/** Name safety mirror of Rust `validate_asset_name` (§4.4); throws `invalidName`. */
function requireValidAssetName(name: string): void {
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
    [...name].some((c) => !/[A-Za-z0-9._-]/.test(c));
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

/** Rich profile body shared by `opus-rich`'s claude + agents targets, so both
 *  land on the same normalized hash — after activation AGENTS.md flips in-sync. */
const OPUS_RICH_BODY =
  '# Project instructions (Opus-rich)\n\n' +
  'Detailed, high-context guidance for a top-tier model. Explain rationale, ' +
  'cover edge cases, and prefer thorough answers.\n';

const CHEAP_TERSE_BODY =
  '# Project instructions (terse)\n\nBe brief. Do the task. No preamble.\n';

/** Seed profile store (§7): two profiles, no active profile yet. */
const mockProfiles: ProfileStore = {
  version: 1,
  profiles: [
    {
      name: 'opus-rich',
      description: 'Full-context instructions for a top-tier model.',
      model: 'opus',
      targets: [
        { assetId: 'claude', content: OPUS_RICH_BODY },
        { assetId: 'agents', content: OPUS_RICH_BODY },
      ],
    },
    {
      name: 'cheap-terse',
      description: 'Minimal instructions for a cheap/fast model.',
      model: 'haiku',
      targets: [{ assetId: 'claude', content: CHEAP_TERSE_BODY }],
    },
  ],
  activeProfile: null,
};

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
  /** `?fixture=noconfig` — commit() rejects with configMissing. */
  noConfig: boolean;
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
      index: 2,
      message: 'WIP on main: experiment with lane colors',
      oid: randomOid(),
      baseOid: fixtureOid(6), // `core work 2` — carries stash@{2}
      ts: now - 10800,
    },
  ];
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

/** Builds a fresh MockRepoState for a usable repo (default / detached / unborn). */
function createRepoState(path: string): MockRepoState {
  const graphFixture = repoGraphFixture(path);
  const kind = repoKind(path, graphFixture);
  const state: MockRepoState = {
    path,
    kind,
    graphFixture,
    noConfig: query('fixture') === 'noconfig',
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
    inventory: structuredClone(mockInventory),
    assetContent: structuredClone(MOCK_ASSET_CONTENT),
    profiles: structuredClone(mockProfiles),
    agentAssets: structuredClone(mockAgentAssets),
  };
  seedOpState(state, repoOp(path));
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
  graph: { dotRadius: 4, avatarRadius: 10, rowHeight: 32, laneWidth: 16 },
  // AI assistance (P13): enabled by default, but consent gates the feature.
  aiEnabled: true,
  aiConflictAutonomy: 'proposeReview',
  aiConsented: false,
  // Embedded MCP server (P16): consent gates the enable toggle.
  mcpConsented: false,
  // MCP write consent (P16c): a separate, stronger gate for the write toggle.
  mcpWriteConsented: false,
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
    return {
      theme,
      paneWidths,
      listView,
      autoFetch,
      graph,
      aiEnabled,
      aiConflictAutonomy,
      aiConsented,
      mcpConsented,
      mcpWriteConsented,
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
      claudeAddCommand: null,
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
    claudeAddCommand: `claude mcp add bonsai --transport http --header "Authorization: Bearer ${MOCK_MCP_TOKEN}" ${url}`,
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

  async commit(repoId: string, message: string): Promise<CommitResult> {
    await delay(150);
    const state = requireRepo(repoId);
    if (message.trim() === '') {
      const err: AppError = { kind: 'emptyMessage', message: 'commit message is empty' };
      throw err;
    }
    // Signature resolution happens before the nothing-to-commit check in the
    // backend (contract §2.4 steps 4→6) — mirror that precedence here.
    if (state.noConfig) {
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

  async checkoutBranch(repoId: string, name: string): Promise<void> {
    await delay(150);
    const state = requireRepo(repoId);
    const branch = state.branches.local.find((b) => b.name === name);
    if (branch === undefined) {
      const err: AppError = { kind: 'branchNotFound', message: `branch '${name}' not found` };
      throw err;
    }
    // Designated dirty-checkout branch (contract §5).
    if (name === 'fix/watcher-debounce') {
      const err: AppError = {
        kind: 'checkoutConflict',
        message:
          "cannot switch to 'fix/watcher-debounce': local changes would be overwritten. " +
          'Commit or discard them first.',
      };
      throw err;
    }
    for (const b of state.branches.local) b.isHead = false;
    branch.isHead = true;
    state.headBranch = name;
    state.branches.head = { branchName: name, oid: state.headOid, detached: false, unborn: false };
    // TODO(polish): move the HEAD/branch pills in the mock graph fixture too
    // (contract §5 decision: fixtures stay decoupled from branch state —
    // harness proof is the sidebar dot + header branch name).
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
    _includeUntracked: boolean,
  ): Promise<CreateStashResult> {
    await delay(150);
    const state = requireRepo(repoId);
    const s = state.status;
    // Nothing to stash when the worktree is clean (no tracked/untracked changes).
    if (s.staged.length === 0 && s.unstaged.length === 0 && s.untracked.length === 0) {
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
    // The worktree comes back clean (INCLUDE_UNTRACKED semantics).
    s.staged = [];
    s.unstaged = [];
    s.untracked = [];
    return { created: true };
  },

  async applyStash(repoId: string, index: number): Promise<ApplyStashOutcome> {
    await delay(150);
    const state = requireRepo(repoId);
    const entry = state.stashes.find((e) => e.index === index);
    // Demo conflict trigger — mirrors the P8 mergeBranch "conflict" convention.
    if (entry !== undefined && entry.message.includes('conflict')) {
      return { kind: 'conflicts', paths: ['src/app.ts'] };
    }
    // Apply leaves the stack unchanged.
    return { kind: 'applied' };
  },

  async popStash(repoId: string, index: number): Promise<ApplyStashOutcome> {
    await delay(150);
    const state = requireRepo(repoId);
    const entry = state.stashes.find((e) => e.index === index);
    // Conflict trigger: the entry is RETAINED (libgit2 only drops on clean pop).
    if (entry !== undefined && entry.message.includes('conflict')) {
      return { kind: 'conflicts', paths: ['src/app.ts'] };
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
    if (state.noConfig) {
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

  // P20 §5/§8.4: cherry-pick. An oid ending in the demo suffix pauses with a
  // conflict (op-state → cherryPick); any other oid commits a new top node.
  async cherrypickCommit(repoId: string, oid: string): Promise<CherrypickOutcome> {
    await delay(150);
    const state = requireRepo(repoId);
    if (state.opState.kind !== 'none') {
      const err: AppError = {
        kind: 'operationInProgress',
        message: 'an operation is already in progress — finish or abort it first',
      };
      throw err;
    }
    if (oid.endsWith(PICK_REVERT_CONFLICT_OID_SUFFIX)) {
      seedPickRevertConflict(state, 'cherryPick');
      return { kind: 'conflicts', paths: ['src/app.ts'] };
    }
    state.headOid = randomOid();
    state.commits.unshift({ oid: state.headOid, summary: `Cherry-pick ${oid.slice(0, 7)}` });
    return { kind: 'committed', oid: state.headOid };
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
    return { kind: 'committed', oid: state.headOid };
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

  // P20 §6/§8.4: revert. Same demo-trigger + op-state plumbing as cherry-pick.
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
    if (oid.endsWith(PICK_REVERT_CONFLICT_OID_SUFFIX)) {
      seedPickRevertConflict(state, 'revert');
      return { kind: 'conflicts', paths: ['src/app.ts'] };
    }
    state.headOid = randomOid();
    state.commits.unshift({ oid: state.headOid, summary: `Revert "${oid.slice(0, 7)}"` });
    return { kind: 'committed', oid: state.headOid };
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
    return { kind: 'committed', oid: state.headOid };
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

  // The mock never emits repo-changed (no backend watcher in the browser
  // harness); resolves to a no-op unsubscribe.
  async onRepoChanged(_cb: (p: RepoChangedPayload) => void): Promise<Unsubscribe> {
    return () => {};
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
      graph: patch.graph !== undefined ? clampGraphPrefs(patch.graph) : current.graph,
      aiEnabled: patch.aiEnabled ?? current.aiEnabled,
      aiConflictAutonomy: patch.aiConflictAutonomy ?? current.aiConflictAutonomy,
      aiConsented: patch.aiConsented ?? current.aiConsented,
      mcpConsented: patch.mcpConsented ?? current.mcpConsented,
      mcpWriteConsented: patch.mcpWriteConsented ?? current.mcpWriteConsented,
    };
    writeUiSettings(next);
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
      validation: {
        valid: false,
        issues: [{ severity: 'error', message: 'file does not exist' }],
      },
    };
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
    state.profiles.activeProfile = name;
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
};
