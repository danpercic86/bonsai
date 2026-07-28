import type { IpcApi } from './types';

// Dynamic imports so a plain browser never loads @tauri-apps/* in mock mode.
export const ipc: IpcApi =
  import.meta.env.VITE_MOCK_IPC === '1'
    ? (await import('./mock')).mockIpc
    : (await import('./tauri')).tauriIpc;

export type {
  AppError,
  CommitResult,
  FileStatus,
  GraphEdge,
  GraphLayout,
  GraphNode,
  HeadInfo,
  IpcApi,
  RefKind,
  RefLabel,
  RepoChangedPayload,
  RepoInfo,
  StatusEntry,
  StatusSnapshot,
  Unsubscribe,
} from './types';
