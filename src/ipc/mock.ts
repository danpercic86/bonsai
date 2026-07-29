import { INITIAL_BRANCHES, MOCK_OID } from './fixtures/branches';
import { mockCommitDiff, mockCommitFileDiff, mockWorkdirDiff } from './fixtures/diffs';
import { buildMockGraph, buildMockGraphDetached, prependCommits } from './fixtures/graph';
import type { MockCommit } from './fixtures/graph';
import { generateLayout20k } from './fixtures/graph20k';
import type {
  AppError,
  BranchesSnapshot,
  CommitDiff,
  CommitResult,
  ConflictEntry,
  ConflictFile,
  ConflictResolution,
  FetchResult,
  FileDiff,
  GraphLayout,
  IpcApi,
  ListView,
  MergeOutcome,
  PullResult,
  PushResult,
  PaneWidths,
  RebaseOutcome,
  RecentRepo,
  RepoChangedPayload,
  RepoInfo,
  RepoOpState,
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

// Stateful mock (M3 contract §5): stage/unstage/commit mutate this snapshot so
// the browser harness round-trips visibly. Reset only when a DIFFERENT path is
// opened (the post-commit openRepo call must not resurrect staged files).
let mockStatus: StatusSnapshot = structuredClone(INITIAL_STATUS);
let mockHeadOid: string = MOCK_OID;
let openedPath: string | null = null;

// Stateful branch mock (M5 contract §5): create/checkout/delete mutate this
// snapshot; mockHeadBranch drives the header after a checkout.
let mockBranches: BranchesSnapshot = structuredClone(INITIAL_BRANCHES);
let mockHeadBranch = 'main';

// Stateful remote mock (M6 contract §5): the first fetch "discovers" one new
// commit on origin/main (main goes behind:1) so a subsequent pull fast-forwards.
let mockFetched = false;

// Synthetic commit rows (P1 contract §3.5): commit() prepends lane-0 rows to
// the DEFAULT graph fixture so the harness shows the new commit at the top.
let mockCommits: MockCommit[] = [];

// Stateful op-state mock (P3c contract §7.2): `?op=merge` seeds a paused
// conflicted merge; resolve/commit/abort mutate this state so the harness
// walks the full merge story. Composable with `?fixture=`.
let opState: RepoOpState = { kind: 'none' };
let conflicts: ConflictEntry[] = [];
let conflictTexts = new Map<string, ConflictFile>();

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

/** Seeds (or clears) the `?op=merge` / `?op=rebase` paused-op state + status rows. */
function seedOpState(): void {
  conflictTexts = new Map();
  const op = new URLSearchParams(window.location.search).get('op');
  if (op === 'rebase') {
    // Pre-seeded paused conflicted rebase at step 2/3 — the "resolve → continue
    // finishes" demo. `rebaseBranch` is the separate clean-rebase demo path.
    opState = {
      kind: 'rebase',
      headName: 'feature/topic',
      onto: '00'.repeat(20), // fixture full oid of the onto tip (base row 0's oid)
      currentStep: 2,
      totalSteps: 3,
    };
    conflicts = [
      { path: 'src/auth.ts', kind: 'bothModified', hasBase: true, hasOurs: true, hasTheirs: true },
    ];
    conflictTexts.set('src/auth.ts', {
      path: 'src/auth.ts',
      kind: 'bothModified',
      binary: false,
      tooLarge: false,
      missing: false,
      text: MERGE_AUTH_TEXT, // reuse the marker fixture
    });
    mockStatus.conflicted = conflicts.map((c) => ({
      path: c.path,
      origPath: null,
      status: 'conflicted',
    }));
    return;
  }
  if (op !== 'merge') {
    opState = { kind: 'none' };
    conflicts = [];
    return;
  }
  opState = {
    kind: 'merge',
    incoming: 'feature/login',
    message: "Merge branch 'feature/login'\n\nConflicts:\n\tsrc/auth.ts\n\tREADME.md",
  };
  // Path-ascending, like the backend's list_conflicts.
  conflicts = [
    { path: 'README.md', kind: 'deletedByThem', hasBase: true, hasOurs: true, hasTheirs: false },
    { path: 'src/auth.ts', kind: 'bothModified', hasBase: true, hasOurs: true, hasTheirs: true },
  ];
  conflictTexts.set('src/auth.ts', {
    path: 'src/auth.ts',
    kind: 'bothModified',
    binary: false,
    tooLarge: false,
    missing: false,
    text: MERGE_AUTH_TEXT,
  });
  // deletedByThem: the worktree keeps OUR version (no markers).
  conflictTexts.set('README.md', {
    path: 'README.md',
    kind: 'deletedByThem',
    binary: false,
    tooLarge: false,
    missing: false,
    text: MERGE_README_TEXT,
  });
  mockStatus.conflicted = conflicts.map((c) => ({
    path: c.path,
    origPath: null,
    status: 'conflicted',
  }));
  // README.md is conflicted, not plain-modified, while the merge is paused.
  mockStatus.unstaged = mockStatus.unstaged.filter((e) => e.path !== 'README.md');
}
seedOpState();

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

/** `?remote=` failure trigger — separate from `?fixture=` so they compose. */
function remoteTrigger(): string | null {
  return new URLSearchParams(window.location.search).get('remote');
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
function finishRebase(steps: number): RebaseOutcome {
  opState = { kind: 'none' };
  mockStatus.conflicted = [];
  mockHeadOid = randomOid();
  // mockCommits[0] is the topmost row = the new HEAD tip (prependCommits maps
  // index 0 to headIndex 0), so the tip carries mockHeadOid.
  const replayed: MockCommit[] = Array.from({ length: steps }, (_, i) => ({
    oid: i === 0 ? mockHeadOid : randomOid(),
    summary: `pick: replayed ${steps - i}`,
  }));
  mockCommits.unshift(...replayed);
  return { kind: 'rebased', branch: mockHeadBranch, head: mockHeadOid, steps };
}

export const mockIpc: IpcApi = {
  async openRepo(path: string): Promise<RepoInfo> {
    await delay(150);

    if (path !== openedPath) {
      mockStatus = structuredClone(INITIAL_STATUS);
      mockHeadOid = MOCK_OID;
      mockBranches = structuredClone(INITIAL_BRANCHES);
      mockHeadBranch = 'main';
      mockFetched = false;
      mockCommits = [];
      seedOpState();
      openedPath = path;
    }

    if (path.includes('error')) {
      const err: AppError = { kind: 'io', message: 'mock: path does not exist' };
      throw err;
    }
    if (path.includes('not-a-repo')) {
      return { path, isRepo: false, bare: false, head: null };
    }
    if (path.includes('bare')) {
      return {
        path,
        isRepo: true,
        bare: true,
        head: { branchName: 'main', oid: '', detached: false, unborn: true },
      };
    }
    if (path.includes('unborn')) {
      recordRecent(path); // usable open: isRepo && !bare (unborn included)
      return {
        path,
        isRepo: true,
        bare: false,
        head: { branchName: 'main', oid: '', detached: false, unborn: true },
      };
    }
    // Every successful usable open (isRepo && !bare) lands in the recents
    // list, like the backend open_repo hook (P1 contract §3.2/§3.4).
    recordRecent(path);
    // `?fixture=detached` mirrors the detached listBranches/graph fixtures in
    // the header HEAD too (M6 §6.8.9: Pull/Push disabled, Fetch enabled).
    if (new URLSearchParams(window.location.search).get('fixture') === 'detached') {
      return {
        path,
        isRepo: true,
        bare: false,
        head: { branchName: null, oid: mockHeadOid, detached: true, unborn: false },
      };
    }
    // Default fixture: the canonical mock repo (and any unknown path). The
    // branch name follows mock checkouts so the App's post-checkout openRepo
    // visibly updates the header (M5 contract §5).
    return {
      path,
      isRepo: true,
      bare: false,
      head: { branchName: mockHeadBranch, oid: mockHeadOid, detached: false, unborn: false },
    };
  },

  async pickFolder(): Promise<string | null> {
    await delay(150);
    return MOCK_REPO_PATH;
  },

  async getStatus(): Promise<StatusSnapshot> {
    await delay(150);
    // Fresh copy so callers can't mutate the fixture between fetches.
    return structuredClone(mockStatus);
  },

  async stage(paths: string[]): Promise<void> {
    await delay(150);
    for (const entry of takeMatching(mockStatus.unstaged, paths)) {
      upsert(mockStatus.staged, entry);
    }
    for (const entry of takeMatching(mockStatus.untracked, paths)) {
      upsert(mockStatus.staged, { ...entry, status: 'added' });
    }
    sortByPath(mockStatus.staged);
  },

  async unstage(paths: string[]): Promise<void> {
    await delay(150);
    for (const entry of takeMatching(mockStatus.staged, paths)) {
      if (entry.status === 'added') {
        upsert(mockStatus.untracked, { ...entry, status: 'untracked' });
      } else {
        upsert(mockStatus.unstaged, entry); // status + origPath preserved
      }
    }
    sortByPath(mockStatus.unstaged);
    sortByPath(mockStatus.untracked);
  },

  async commit(message: string): Promise<CommitResult> {
    await delay(150);
    if (message.trim() === '') {
      const err: AppError = { kind: 'emptyMessage', message: 'commit message is empty' };
      throw err;
    }
    // Signature resolution happens before the nothing-to-commit check in the
    // backend (contract §2.4 steps 4→6) — mirror that precedence here.
    if (new URLSearchParams(window.location.search).get('fixture') === 'noconfig') {
      const err: AppError = {
        kind: 'configMissing',
        message:
          'git identity not configured: user.name and user.email are not set. ' +
          'Run: git config --global user.name "Your Name" and ' +
          'git config --global user.email "you@example.com"',
      };
      throw err;
    }
    if (mockStatus.staged.length === 0) {
      const err: AppError = {
        kind: 'nothingToCommit',
        message: 'nothing to commit (index matches HEAD)',
      };
      throw err;
    }
    mockStatus.staged = [];
    mockHeadOid = randomOid();
    // M6 contract §5: bump the current branch's ahead count so the harness
    // gets the natural commit → push story (main: 0/0 → ↑1 → push clears).
    const headBranch = mockBranches.local.find((b) => b.name === mockHeadBranch);
    if (headBranch !== undefined && headBranch.upstream !== null) {
      headBranch.ahead = (headBranch.ahead ?? 0) + 1;
    }
    const summary = message.trim().split('\n', 1)[0] ?? '';
    // P1 contract §3.5: the DEFAULT graph fixture gains a synthetic lane-0 row
    // per mock commit (newest first) so the harness shows the commit on top.
    mockCommits.unshift({ oid: mockHeadOid, summary });
    return { oid: mockHeadOid, summary, branch: mockHeadBranch };
  },

  async getWorkdirFileDiff(
    path: string,
    origPath: string | null,
    staged: boolean,
  ): Promise<FileDiff> {
    await delay(150);
    return structuredClone(mockWorkdirDiff(path, origPath, staged));
  },

  async getCommitDiff(oid: string): Promise<CommitDiff> {
    await delay(150);
    // Route by row index of the ACTIVE fixture layout (contract §5: robust
    // against oid spelling; 20k rows fall through to the generic diff).
    const fixture = new URLSearchParams(window.location.search).get('fixture');
    const layout =
      fixture === '20k'
        ? generateLayout20k()
        : fixture === 'detached'
          ? buildMockGraphDetached()
          : prependCommits(buildMockGraph(), mockCommits);
    const index = layout.nodes.findIndex((n) => n.id === oid);
    if (index === -1) {
      const err: AppError = { kind: 'git', message: 'mock: unknown commit' };
      throw err;
    }
    return structuredClone(mockCommitDiff(index, oid));
  },

  async getCommitFileDiff(oid: string, path: string, origPath: string | null): Promise<FileDiff> {
    await delay(150);
    return structuredClone(mockCommitFileDiff(oid, path, origPath));
  },

  async getGraph(): Promise<GraphLayout> {
    await delay(150);
    // Built fresh per call (timestamps relative to now; callers own the copy).
    // `?fixture=` selects a variant (contract §5.4 mechanism).
    const fixture = new URLSearchParams(window.location.search).get('fixture');
    if (fixture === '20k') return generateLayout20k();
    if (fixture === 'detached') return buildMockGraphDetached();
    // Default fixture: synthetic mock-commit rows prepended (P1 §3.5); the
    // 20k and detached fixtures stay as-is.
    return prependCommits(buildMockGraph(), mockCommits);
  },

  async listBranches(): Promise<BranchesSnapshot> {
    await delay(150);
    const snapshot = structuredClone(mockBranches);
    if (new URLSearchParams(window.location.search).get('fixture') === 'detached') {
      snapshot.head = { branchName: null, oid: mockHeadOid, detached: true, unborn: false };
      for (const branch of snapshot.local) branch.isHead = false;
    } else {
      snapshot.head = {
        branchName: mockHeadBranch,
        oid: mockHeadOid,
        detached: false,
        unborn: false,
      };
    }
    return snapshot;
  },

  async createBranch(name: string): Promise<void> {
    await delay(150);
    if (isInvalidBranchName(name)) {
      const err: AppError = { kind: 'invalidName', message: `invalid branch name: '${name}'` };
      throw err;
    }
    const trimmed = name.trim();
    if (mockBranches.local.some((b) => b.name === trimmed)) {
      const err: AppError = {
        kind: 'branchExists',
        message: `branch '${trimmed}' already exists`,
      };
      throw err;
    }
    mockBranches.local.push({
      name: trimmed,
      isHead: false,
      upstream: null,
      ahead: null,
      behind: null,
    });
    mockBranches.local.sort((a, b) => a.name.toLowerCase().localeCompare(b.name.toLowerCase()));
  },

  async checkoutBranch(name: string): Promise<void> {
    await delay(150);
    const branch = mockBranches.local.find((b) => b.name === name);
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
    for (const b of mockBranches.local) b.isHead = false;
    branch.isHead = true;
    mockHeadBranch = name;
    mockBranches.head = { branchName: name, oid: mockHeadOid, detached: false, unborn: false };
    // TODO(polish): move the HEAD/branch pills in the mock graph fixture too
    // (contract §5 decision: fixtures stay decoupled from branch state —
    // harness proof is the sidebar dot + header branch name).
  },

  async deleteBranch(name: string): Promise<void> {
    await delay(150);
    const branch = mockBranches.local.find((b) => b.name === name);
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
    mockBranches.local = mockBranches.local.filter((b) => b.name !== name);
  },

  // Stateful remote mock (M6 contract §5). Failure triggers via `?remote=`
  // (authfail | network | rejected | conflict), composable with `?fixture=`.
  async fetch(): Promise<FetchResult> {
    await delay(400);
    const trigger = remoteTrigger();
    if (trigger === 'authfail') throwAuthFailed();
    if (trigger === 'network') throwNetworkError();
    if (!mockFetched) {
      mockFetched = true;
      // The fetch "discovers" one new upstream commit on main.
      const main = mockBranches.local.find((b) => b.name === 'main');
      if (main !== undefined && main.upstream !== null) {
        main.behind = 1;
      }
      return { remotes: [{ remote: 'origin', receivedObjects: 12, updatedRefs: 1 }] };
    }
    return { remotes: [{ remote: 'origin', receivedObjects: 0, updatedRefs: 0 }] };
  },

  async pull(): Promise<PullResult> {
    await delay(400);
    const trigger = remoteTrigger();
    if (trigger === 'authfail') throwAuthFailed();
    if (trigger === 'network') throwNetworkError();
    if (trigger === 'conflict') {
      const err: AppError = {
        kind: 'checkoutConflict',
        message:
          'cannot pull: local changes would be overwritten by the update. ' +
          'Commit or discard them first.',
      };
      throw err;
    }
    const branch = mockBranches.local.find((b) => b.name === mockHeadBranch);
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
      const from = mockHeadOid;
      mockHeadOid = randomOid();
      branch.behind = 0;
      return { kind: 'fastForwarded', branch: branch.name, from, to: mockHeadOid };
    }
    return { kind: 'upToDate' };
  },

  async push(): Promise<PushResult> {
    await delay(400);
    const trigger = remoteTrigger();
    if (trigger === 'authfail') throwAuthFailed();
    if (trigger === 'network') throwNetworkError();
    if (trigger === 'rejected') {
      const err: AppError = {
        kind: 'pushRejected',
        message:
          'push rejected: the remote contains commits you do not have. ' +
          'Fetch/pull first — Bonsai v1 never force-pushes.',
      };
      throw err;
    }
    const branch = mockBranches.local.find((b) => b.name === mockHeadBranch);
    if (branch === undefined) {
      return { kind: 'upToDate', remote: 'origin', branch: mockHeadBranch };
    }
    if (branch.upstream === null) {
      // First push of a new branch: push to origin/<name> AND set upstream.
      branch.upstream = `origin/${branch.name}`;
      branch.ahead = 0;
      branch.behind = 0;
      if (!mockBranches.remote.some((r) => r.name === branch.upstream)) {
        mockBranches.remote.push({ name: `origin/${branch.name}` });
        mockBranches.remote.sort((a, b) =>
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

  // Stateful op-state mock (P3c contract §7.2). `?op=merge` starts the
  // harness pre-seeded in a paused conflicted merge; mergeBranch is the
  // clean-merge demo path.
  async getOpState(): Promise<RepoOpState> {
    await delay(150);
    return structuredClone(opState);
  },

  async mergeBranch(name: string): Promise<MergeOutcome> {
    await delay(150);
    if (opState.kind !== 'none') {
      const err: AppError = {
        kind: 'operationInProgress',
        message: 'an operation is already in progress — commit or abort it first',
      };
      throw err;
    }
    // Clean-merge demo: auto-committed 2-parent node on top of the graph.
    mockHeadOid = randomOid();
    mockCommits.unshift({
      oid: mockHeadOid,
      summary: `Merge branch '${name}'`,
      mergeParentBase: 1, // the 'feat' fixture tip
    });
    const headBranch = mockBranches.local.find((b) => b.name === mockHeadBranch);
    if (headBranch !== undefined && headBranch.upstream !== null) {
      headBranch.ahead = (headBranch.ahead ?? 0) + 1;
    }
    return { kind: 'merged', oid: mockHeadOid };
  },

  async commitMerge(message: string): Promise<CommitResult> {
    await delay(150);
    if (opState.kind !== 'merge') {
      const err: AppError = { kind: 'noOperationInProgress', message: 'no merge in progress' };
      throw err;
    }
    if (conflicts.length > 0) {
      const err: AppError = {
        kind: 'unresolvedConflicts',
        message: `cannot commit: ${conflicts.length} unresolved conflict(s) remain`,
      };
      throw err;
    }
    if (message.trim() === '') {
      const err: AppError = { kind: 'emptyMessage', message: 'commit message is empty' };
      throw err;
    }
    opState = { kind: 'none' };
    mockStatus.conflicted = [];
    mockHeadOid = randomOid();
    const summary = message.trim().split('\n', 1)[0] ?? '';
    // Faithful twin: a visible 2-parent merge node on top of the graph
    // (second parent = the 'feat' fixture tip, base row 1).
    mockCommits.unshift({ oid: mockHeadOid, summary, mergeParentBase: 1 });
    const headBranch = mockBranches.local.find((b) => b.name === mockHeadBranch);
    if (headBranch !== undefined && headBranch.upstream !== null) {
      headBranch.ahead = (headBranch.ahead ?? 0) + 1;
    }
    return { oid: mockHeadOid, summary, branch: mockHeadBranch };
  },

  async abortMerge(): Promise<void> {
    await delay(150);
    if (opState.kind !== 'merge') {
      const err: AppError = { kind: 'noOperationInProgress', message: 'no merge in progress' };
      throw err;
    }
    // Restore the pre-merge state.
    opState = { kind: 'none' };
    conflicts = [];
    conflictTexts = new Map();
    mockStatus.conflicted = [];
  },

  async listConflicts(): Promise<ConflictEntry[]> {
    await delay(150);
    return structuredClone(conflicts);
  },

  async getConflict(path: string): Promise<ConflictFile> {
    await delay(150);
    const file = conflictTexts.get(path);
    if (file === undefined) {
      const err: AppError = { kind: 'git', message: `path '${path}' has no conflict` };
      throw err;
    }
    return structuredClone(file);
  },

  async resolveConflict(path: string, resolution: ConflictResolution): Promise<void> {
    await delay(150);
    const entry = conflicts.find((c) => c.path === path);
    if (entry === undefined) {
      const err: AppError = { kind: 'git', message: `path '${path}' has no conflict` };
      throw err;
    }
    conflicts = conflicts.filter((c) => c.path !== path);
    conflictTexts.delete(path);
    mockStatus.conflicted = mockStatus.conflicted.filter((e) => e.path !== path);
    // Taking THEIRS on a deletedByThem conflict accepts their deletion: the
    // file shows up as a staged deletion in the mock lists (contract §7.2).
    if (resolution === 'theirs' && entry.kind === 'deletedByThem') {
      upsert(mockStatus.staged, { path, origPath: null, status: 'deleted' });
      sortByPath(mockStatus.staged);
    }
  },

  // Stateful rebase mock (P3d contract §7.2). `?op=rebase` starts the harness
  // pre-seeded in a paused conflicted rebase (step 2/3); rebaseBranch is the
  // clean-rebase demo path. Shares opState/conflicts/conflictTexts with merge.
  async rebaseBranch(_onto: string): Promise<RebaseOutcome> {
    await delay(150);
    if (opState.kind !== 'none') {
      const err: AppError = {
        kind: 'operationInProgress',
        message: 'an operation is already in progress — commit or abort it first',
      };
      throw err;
    }
    // Clean-rebase demo: replay 3 plain commits atop the graph so they appear.
    // mockCommits[0] is the topmost row = the new HEAD tip, so it carries the oid.
    mockHeadOid = randomOid();
    mockCommits.unshift(
      { oid: mockHeadOid, summary: 'pick: replayed 3' },
      { oid: randomOid(), summary: 'pick: replayed 2' },
      { oid: randomOid(), summary: 'pick: replayed 1' },
    );
    const headBranch = mockBranches.local.find((b) => b.name === mockHeadBranch);
    if (headBranch !== undefined && headBranch.upstream !== null) {
      headBranch.ahead = (headBranch.ahead ?? 0) + 3;
    }
    return { kind: 'rebased', branch: mockHeadBranch, head: mockHeadOid, steps: 3 };
  },

  async rebaseContinue(): Promise<RebaseOutcome> {
    await delay(150);
    if (opState.kind !== 'rebase') {
      const err: AppError = { kind: 'noOperationInProgress', message: 'no rebase in progress' };
      throw err;
    }
    if (conflicts.length > 0) {
      const err: AppError = {
        kind: 'unresolvedConflicts',
        message: `cannot continue: ${conflicts.length} unresolved conflict(s) remain`,
      };
      throw err;
    }
    // Advance the current step (so a mid-call getOpState would reflect it), then
    // finish: the seeded demo has no further conflict, so a single continue
    // completes the remaining steps (2/3 → done).
    const totalSteps = opState.totalSteps;
    opState = { ...opState, currentStep: opState.currentStep + 1 };
    return finishRebase(totalSteps);
  },

  async rebaseSkip(): Promise<RebaseOutcome> {
    await delay(150);
    if (opState.kind !== 'rebase') {
      const err: AppError = { kind: 'noOperationInProgress', message: 'no rebase in progress' };
      throw err;
    }
    // Skip is allowed WITH conflicts — dropping the offending commit resolves it.
    const totalSteps = opState.totalSteps;
    conflicts = [];
    conflictTexts = new Map();
    mockStatus.conflicted = [];
    opState = { ...opState, currentStep: opState.currentStep + 1 };
    return finishRebase(totalSteps);
  },

  async rebaseAbort(): Promise<void> {
    await delay(150);
    if (opState.kind !== 'rebase') {
      const err: AppError = { kind: 'noOperationInProgress', message: 'no rebase in progress' };
      throw err;
    }
    // Abort rewinds: restore the pre-rebase state, prepend NOTHING.
    opState = { kind: 'none' };
    conflicts = [];
    conflictTexts = new Map();
    mockStatus.conflicted = [];
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
      paneWidths: patch.paneWidths !== undefined ? clampPaneWidths(patch.paneWidths) : current.paneWidths,
      listView: patch.listView ?? current.listView,
    };
    writeUiSettings(next);
    return next;
  },
};
