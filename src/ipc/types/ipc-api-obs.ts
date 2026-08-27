import type { LogRecord, LogSessionInfo } from './obs';

/**
 * P91 §6 — observability (Dev-mode structured logs), split out of `IpcApi`
 * (which extends this) the same way `IpcApiForge` is: `ipc-api.ts` is already
 * over the file-size ratchet's limit and may only shrink.
 *
 * EVERY method here is on the §2.3 instrumentation EXCLUSION list — the IPC
 * proxy (increment 2) must not wrap them, or writing a log record would itself
 * emit log records and the sink would amplify without bound.
 */
export interface IpcApiObs {
  /** Append a batch of frontend log records to the session file. A no-op when
   *  Dev mode is off. NEVER blocks: a full sink queue drops the record and the
   *  loss is reported by a `drop` record rather than by backpressure. */
  logAppend(records: LogRecord[]): Promise<void>;
  /** Current session summary + totals across ALL log files on disk (the delete
   *  confirm copy needs the totals), plus the session salt the frontend
   *  redactor mirror needs (§7.2). */
  logSessionInfo(): Promise<LogSessionInfo>;
  /** Open the logs folder in the OS file manager. Creates it if absent. */
  logRevealDir(): Promise<void>;
  /** Zip the current session's parts — or the newest session on disk when Dev
   *  mode is off, which is what the "turn Dev mode off, then export" workflow
   *  needs — and resolve with the archive path. */
  logExportSession(dest?: string | null): Promise<string>;
}
