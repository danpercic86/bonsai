import type { AppError, IpcApi, RepoInfo } from './types';

const MOCK_REPO_PATH = 'C:\\mock\\bonsai-fixture';
const MOCK_OID = '9fceb02d0ae598e95dc970b74767f19372d61af8';

function delay(ms = 150): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

export const mockIpc: IpcApi = {
  async openRepo(path: string): Promise<RepoInfo> {
    await delay(150);

    if (path.includes('error')) {
      const err: AppError = { kind: 'io', message: 'mock: path does not exist' };
      throw err;
    }
    if (path.includes('not-a-repo')) {
      return { path, isRepo: false, head: null };
    }
    if (path.includes('unborn')) {
      return {
        path,
        isRepo: true,
        head: { branchName: 'main', oid: '', detached: false, unborn: true },
      };
    }
    // Default fixture: the canonical mock repo (and any unknown path).
    return {
      path,
      isRepo: true,
      head: { branchName: 'main', oid: MOCK_OID, detached: false, unborn: false },
    };
  },

  async pickFolder(): Promise<string | null> {
    await delay(150);
    return MOCK_REPO_PATH;
  },
};
