import { invoke } from './invoke';
import type { LogRecord, LogSessionInfo, LogsDeleteResult, MetricsSnapshot } from '../types';

/**
 * P91 §6 — the real-Tauri observability commands.
 *
 * All four are on the §2.3 instrumentation EXCLUSION list: the IPC proxy
 * (increment 2) must never wrap them, or every log write would log itself.
 */
export const obsCommands = {
  logAppend(records: LogRecord[]): Promise<void> {
    return invoke<void>('log_append', { records });
  },

  logSessionInfo(): Promise<LogSessionInfo> {
    return invoke<LogSessionInfo>('log_session_info');
  },

  logRevealDir(): Promise<void> {
    return invoke<void>('log_reveal_dir');
  },

  logExportSession(): Promise<string> {
    // No `dest`: the backend always writes into its own `exports/` directory
    // (audit F4 — a webview-supplied path is not a user-chosen path).
    return invoke<string>('log_export_session');
  },

  logsDeleteAll(): Promise<LogsDeleteResult> {
    return invoke<LogsDeleteResult>('logs_delete_all');
  },

  metricsSnapshot(): Promise<MetricsSnapshot> {
    return invoke<MetricsSnapshot>('metrics_snapshot');
  },

  metricsReset(): Promise<void> {
    return invoke<void>('metrics_reset');
  },
};
