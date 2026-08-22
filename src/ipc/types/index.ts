// Barrel for the IPC type surface. Split by feature domain (P-refactor); the
// import path `../ipc/types` / `./types` resolves here and re-exports every
// domain module, so the exported name set is unchanged.
export * from './ai';
export * from './ai-assets';
export * from './branches';
export * from './commit';
export * from './common';
export * from './config';
export * from './conflict';
export * from './diff';
export * from './forge';
export * from './graph';
export * from './health';
export * from './history';
export * from './hooks';
export * from './ipc-api';
export * from './jobs';
export * from './mcp';
export * from './remotes';
export * from './safe-op';
export * from './search';
export * from './settings';
export * from './signing';
export * from './stash';
export * from './status';
export * from './submodule';
export * from './update';
export * from './worktree';
