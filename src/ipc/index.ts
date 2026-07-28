import type { IpcApi } from './types';

// Dynamic imports so a plain browser never loads @tauri-apps/* in mock mode.
export const ipc: IpcApi =
  import.meta.env.VITE_MOCK_IPC === '1'
    ? (await import('./mock')).mockIpc
    : (await import('./tauri')).tauriIpc;

export type {
  AppError,
  BranchesSnapshot,
  BranchInfo,
  CommitDetails,
  CommitDiff,
  CommitResult,
  DiffLine,
  FileDiff,
  FileDiffHeader,
  FileStatus,
  GraphEdge,
  GraphLayout,
  GraphNode,
  HeadInfo,
  Hunk,
  IpcApi,
  LineKind,
  RefKind,
  RefLabel,
  RemoteBranchInfo,
  RepoChangedPayload,
  RepoInfo,
  StatusEntry,
  StatusSnapshot,
  Unsubscribe,
} from './types';
