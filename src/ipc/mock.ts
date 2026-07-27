import { buildMockGraph, buildMockGraphDetached } from './fixtures/graph';
import { generateLayout20k } from './fixtures/graph20k';
import type {
  AppError,
  GraphLayout,
  IpcApi,
  RepoChangedPayload,
  RepoInfo,
  StatusSnapshot,
  Unsubscribe,
} from './types';

const MOCK_REPO_PATH = 'C:\\mock\\bonsai-fixture';
const MOCK_OID = '9fceb02d0ae598e95dc970b74767f19372d61af8';

function delay(ms = 150): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

/** Exercises every status render path, incl. a file both staged AND modified. */
const MOCK_STATUS: StatusSnapshot = {
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

export const mockIpc: IpcApi = {
  async openRepo(path: string): Promise<RepoInfo> {
    await delay(150);

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
      head: { branchName: 'main', oid: MOCK_OID, detached: false, unborn: false },
    };
  },

  async pickFolder(): Promise<string | null> {
    await delay(150);
    return MOCK_REPO_PATH;
  },

  async getStatus(): Promise<StatusSnapshot> {
    await delay(150);
    // Fresh copy so callers can't mutate the fixture between fetches.
    return structuredClone(MOCK_STATUS);
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
