import { buildMockGraph, buildMockGraphDetached } from './fixtures/graph';
import { generateLayout20k } from './fixtures/graph20k';
import type {
  AppError,
  CommitResult,
  GraphLayout,
  IpcApi,
  RepoChangedPayload,
  RepoInfo,
  StatusEntry,
  StatusSnapshot,
  Unsubscribe,
} from './types';

const MOCK_REPO_PATH = 'C:\\mock\\bonsai-fixture';
const MOCK_OID = '9fceb02d0ae598e95dc970b74767f19372d61af8';

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
    // Default fixture: the canonical mock repo (and any unknown path).
    return {
      path,
      isRepo: true,
      bare: false,
      head: { branchName: 'main', oid: mockHeadOid, detached: false, unborn: false },
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
    // TODO(polish): prepend a synthetic graph row on mock commit (contract §5
    // decision: not worth the coupling for now — harness proof of commit is
    // the emptied staged list + changed header oid + cleared textarea).
    const summary = message.trim().split('\n', 1)[0] ?? '';
    return { oid: mockHeadOid, summary, branch: 'main' };
  },

  async getGraph(): Promise<GraphLayout> {
    await delay(150);
    // Built fresh per call (timestamps relative to now; callers own the copy).
    // `?fixture=` selects a variant (contract §5.4 mechanism).
    const fixture = new URLSearchParams(window.location.search).get('fixture');
    if (fixture === '20k') return generateLayout20k();
    return fixture === 'detached' ? buildMockGraphDetached() : buildMockGraph();
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
