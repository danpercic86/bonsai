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
  CompareDiff,
  CompareEndpoint,
  ConflictEntry,
  ConflictFile,
  ConflictKind,
  ConflictResolution,
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
  ListView,
  MergeOutcome,
  OpenRepoResult,
  RebaseOutcome,
  RecentRepo,
  RefKind,
  RefLabel,
  PaneWidths,
  RemoteBranchInfo,
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
