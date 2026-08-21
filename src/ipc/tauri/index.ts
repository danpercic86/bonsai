import type { IpcApi } from '../types';
import { repoCommands } from './repo';
import { workdirCommands } from './workdir';
import { branchesCommands } from './branches';
import { remotesCommands } from './remotes';
import { tagsCommands } from './tags';
import { mergeCommands } from './merge';
import { rebaseCommands } from './rebase';
import { commitOpsCommands } from './commit-ops';
import { stashCommands } from './stash';
import { submoduleCommands } from './submodule';
import { worktreeCommands } from './worktree';
import { historyCommands } from './history';
import { signingCommands } from './signing';
import { configCommands } from './config';
import { aiCommands } from './ai';
import { assetsCommands } from './assets';
import { mcpCommands } from './mcp';
import { appCommands } from './app';
import { updateCommands } from './update';
import { forgeCommands } from './forge';

// Real-Tauri IPC surface. Split by concern (P-refactor); the render/app code
// imports `tauriIpc` from `../ipc/tauri` unchanged — this barrel composes the
// per-domain command groups into the single IpcApi object.
export const tauriIpc: IpcApi = {
  ...repoCommands,
  ...workdirCommands,
  ...branchesCommands,
  ...remotesCommands,
  ...tagsCommands,
  ...mergeCommands,
  ...rebaseCommands,
  ...commitOpsCommands,
  ...stashCommands,
  ...submoduleCommands,
  ...worktreeCommands,
  ...historyCommands,
  ...signingCommands,
  ...configCommands,
  ...aiCommands,
  ...assetsCommands,
  ...mcpCommands,
  ...appCommands,
  ...updateCommands,
  ...forgeCommands,
};
