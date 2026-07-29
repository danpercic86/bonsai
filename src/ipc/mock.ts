import { INITIAL_BRANCHES, MOCK_OID } from './fixtures/branches';
import {
  mockCommitDiff,
  mockCommitFileDiff,
  mockCompareDiff,
  mockWorkdirDiff,
} from './fixtures/diffs';
import { buildMockGraph, buildMockGraphDetached, prependCommits } from './fixtures/graph';
import type { MockCommit } from './fixtures/graph';
import { generateLayout20k } from './fixtures/graph20k';
import type {
  AppError,
  BranchesSnapshot,
  CommitDiff,
  CommitResult,
  CompareDiff,
  ConflictEntry,
  ConflictFile,
  ConflictResolution,
  FetchResult,
  FileDiff,
  GraphLayout,
  IpcApi,
  ListView,
  MergeOutcome,
  OpenRepoResult,
  PullResult,
  PushResult,
  PaneWidths,
  RebaseOutcome,
  RecentRepo,
  RepoChangedPayload,
  RepoInfo,
  RepoOpState,
  SessionState,
  StatusEntry,
  StatusSnapshot,
  Theme,
  UiSettings,
  UiSettingsPatch,
  Unsubscribe,
} from './types';

const MOCK_REPO_PATH = 'C:\\mock\\bonsai-fixture';

function delay(ms = 150): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

/** Exercises every status render path, incl. a file both staged AND modified. */
const INITIAL_STATUS: StatusSnapshot = {
  staged: [
    { path: 'src/app.rs', origPath: null, status: 'added' },
    { path: 'src/main.rs', origPath: null, status: 'modified' },
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

const MERGE_README_TEXT = [
  '# Bonsai fixture',
  '',
  'Our side kept this README while feature/login deleted it.',
  '',
].join('\n');

// ---------------------------------------------------------------------------
// P3e-c: per-repo state. Every stateful flow that used to live in module-level
// singletons now lives inside a MockRepoState, one per open repoId. The map is
// the single source of per-repo truth — there is NO module-level per-repo
// singleton anymore. `openRepo` creates entries lazily; `closeRepo` deletes.
// ---------------------------------------------------------------------------

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
}

const repos = new Map<string /* repoId */, MockRepoState>();

function query(name: string): string | null {
  return new URLSearchParams(window.location.search).get(name);
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
  });
  // deletedByThem: the worktree keeps OUR version (no markers).
  state.conflictTexts.set('README.md', {
    path: 'README.md',
    kind: 'deletedByThem',
    binary: false,
    tooLarge: false,
    missing: false,
    text: MERGE_README_TEXT,
  });
  state.status.conflicted = state.conflicts.map((c) => ({
    path: c.path,
    origPath: null,
    status: 'conflicted',
  }));
  // README.md is conflicted, not plain-modified, while the merge is paused.
  state.status.unstaged = state.status.unstaged.filter((e) => e.path !== 'README.md');
}

/** Builds a fresh MockRepoState for a usable repo (default / detached / unborn). */
function createRepoState(path: string): MockRepoState {
  const graphFixture = repoGraphFixture(path);
  const state: MockRepoState = {
    path,
    kind: repoKind(path, graphFixture),
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
};

function clampPaneWidths(w: PaneWidths): PaneWidths {
  return {
    sidebar: Math.min(SIDEBAR_MAX, Math.max(SIDEBAR_MIN, w.sidebar)),
    rightPanel: Math.min(RIGHT_PANEL_MAX, Math.max(RIGHT_PANEL_MIN, w.rightPanel)),
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
    return { theme, paneWidths, listView };
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

function matchesAny(entry: StatusEntry, paths: string[]): boolean {
  return paths.includes(entry.path) || (entry.origPath !== null && paths.includes(entry.origPath));
}

function sortByPath(entries: StatusEntry[]): void {
  entries.sort((a, b) => (a.path < b.path ? -1 : a.path > b.path ? 1 : 0));
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
    return structuredClone(state.status);
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
  ): Promise<FileDiff> {
    await delay(150);
    requireRepo(repoId);
    return structuredClone(mockWorkdirDiff(path, origPath, staged));
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
  ): Promise<FileDiff> {
    await delay(150);
    requireRepo(repoId);
    return structuredClone(mockCommitFileDiff(oid, path, origPath));
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
  ): Promise<FileDiff> {
    await delay(150);
    requireRepo(repoId);
    // A compare file diff has the same FileDiff shape — reuse the commit builder.
    return structuredClone(mockCommitFileDiff(oid, path, origPath));
  },

  async getGraph(repoId: string): Promise<GraphLayout> {
    await delay(150);
    const state = requireRepo(repoId);
    // Built fresh per call (timestamps relative to now; callers own the copy).
    if (state.graphFixture === '20k') return generateLayout20k();
    if (state.graphFixture === 'detached') return buildMockGraphDetached();
    // Default fixture: synthetic mock-commit rows prepended (P1 §3.5).
    return prependCommits(buildMockGraph(), state.commits);
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
    return { kind: 'merged', oid: state.headOid };
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
    // Abort rewinds: restore the pre-rebase state, prepend NOTHING.
    state.opState = { kind: 'none' };
    state.conflicts = [];
    state.conflictTexts = new Map();
    state.status.conflicted = [];
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
};
