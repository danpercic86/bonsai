// Split out of the former monolithic mock.ts (pure refactor; no behavior change).
import type { IpcApi } from './types';
import { repoHandlers } from './mock/handlers/repo';
import { statusHandlers } from './mock/handlers/status';
import { diffHandlers } from './mock/handlers/diff';
import { graphStreamHandlers } from './mock/handlers/graphStream';
import { branchHandlers } from './mock/handlers/branches';
import { remotesSyncHandlers } from './mock/handlers/remotesSync';
import { mergeHandlers } from './mock/handlers/merge';
import { activityHandlers } from './mock/handlers/activity';
import { aiHandlers } from './mock/handlers/ai';
import { aiStreamHandlers } from './mock/handlers/aiStream';
import { composeHandlers } from './mock/handlers/compose';
import { rebaseHandlers } from './mock/handlers/rebase';
import { bisectHistoryHandlers } from './mock/handlers/bisectHistory';
import { undoHandlers } from './mock/handlers/undo';
import { searchHandlers } from './mock/handlers/search';
import { signingHandlers } from './mock/handlers/signing';
import { historyHandlers } from './mock/handlers/history';
import { configHandlers } from './mock/handlers/config';
import { stashHandlers } from './mock/handlers/stash';
import { resetRevertHandlers } from './mock/handlers/resetRevert';
import { submoduleHandlers } from './mock/handlers/submodules';
import { worktreeHandlers } from './mock/handlers/worktrees';
import { repoMetaHandlers } from './mock/handlers/repoMeta';
import { tagSyncHandlers } from './mock/handlers/tagSync';
import { sessionHandlers } from './mock/handlers/session';
import { mcpHandlers } from './mock/handlers/mcp';
import { assetsHandlers } from './mock/handlers/assets';
import { updateHandlers } from './mock/handlers/update';
import { gitEnvHandlers } from './mock/handlers/gitEnv';
import { externalHandlers } from './mock/handlers/external';
import { forgeHandlers } from './mock/handlers/forge';

// Assembled from per-domain handler groups. Public surface unchanged: index.ts
// still imports { mockIpc } from './mock'.
export const mockIpc: IpcApi = {
  ...repoHandlers,
  ...statusHandlers,
  ...diffHandlers,
  ...graphStreamHandlers,
  ...branchHandlers,
  ...remotesSyncHandlers,
  ...mergeHandlers,
  ...activityHandlers,
  ...aiHandlers,
  ...aiStreamHandlers,
  ...composeHandlers,
  ...rebaseHandlers,
  ...bisectHistoryHandlers,
  ...undoHandlers,
  ...searchHandlers,
  ...signingHandlers,
  ...historyHandlers,
  ...configHandlers,
  ...stashHandlers,
  ...resetRevertHandlers,
  ...submoduleHandlers,
  ...worktreeHandlers,
  ...repoMetaHandlers,
  ...tagSyncHandlers,
  ...sessionHandlers,
  ...mcpHandlers,
  ...assetsHandlers,
  ...updateHandlers,
  ...gitEnvHandlers,
  ...externalHandlers,
  ...forgeHandlers,
};
