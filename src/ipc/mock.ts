import { INITIAL_BRANCHES, MOCK_OID } from './fixtures/branches';
import { mockCommitDiff, mockCommitFileDiff, mockWorkdirDiff } from './fixtures/diffs';
import { buildMockGraph, buildMockGraphDetached } from './fixtures/graph';
import { generateLayout20k } from './fixtures/graph20k';
import type {
  AppError,
  BranchesSnapshot,
  CommitDiff,
  CommitResult,
  FetchResult,
  FileDiff,
  GraphLayout,
  IpcApi,
  PullResult,
  PushResult,
  RepoChangedPayload,
  RepoInfo,
  StatusEntry,
  StatusSnapshot,
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

export const mockIpc: IpcApi = {
  async openRepo(path: string): Promise<RepoInfo> {
    await delay(150);

    if (path !== openedPath) {
      mockStatus = structuredClone(INITIAL_STATUS);
      mockHeadOid = MOCK_OID;
      mockBranches = structuredClone(INITIAL_BRANCHES);
      mockHeadBranch = 'main';
      mockFetched = false;
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
      return {
        path,
        isRepo: true,
        bare: false,
        head: { branchName: 'main', oid: '', detached: false, unborn: true },
      };
    }
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
    // TODO(polish): prepend a synthetic graph row on mock commit (contract §5
    // decision: not worth the coupling for now — harness proof of commit is
    // the emptied staged list + changed header oid + cleared textarea).
    const summary = message.trim().split('\n', 1)[0] ?? '';
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
          : buildMockGraph();
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
    return fixture === 'detached' ? buildMockGraphDetached() : buildMockGraph();
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
};
