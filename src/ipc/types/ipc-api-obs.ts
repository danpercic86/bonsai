import type { LogRecord, LogSessionInfo, LogsDeleteResult, MetricsSnapshot } from './obs';

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
   *  needs — and resolve with the archive path. The destination is ALWAYS the
   *  app-managed `exports/` directory: it takes NO path argument, because a path
   *  crossing this boundary comes from the webview rather than from the user
   *  (there is no save dialog in P91) and would escape the `logsDeleteAll`
   *  scope. A future "Save as…" must get its path from a backend-invoked Tauri
   *  dialog, not from an argument here. */
  logExportSession(): Promise<string>;
  /** §6.1/§6.2 — delete every in-scope log/export artifact (roll-then-purge when
   *  Dev mode is on, so logging continues into a fresh file). Reports honest
   *  counts including partial failures. The UI MUST confirm before invoking. On
   *  the §2.3 instrumentation exclusion list. */
  logsDeleteAll(): Promise<LogsDeleteResult>;
  /** §8 — read the durable local metrics aggregates (daily buckets + lifetime).
   *  The returned histograms carry DERIVED `p50Ms`/`p95Ms`, computed at snapshot
   *  time and never persisted. On the §2.3 instrumentation exclusion list. */
  metricsSnapshot(): Promise<MetricsSnapshot>;
  /** §8 — clear every local metric aggregate. HEADLESS: no settings-catalog row
   *  surfaces it in P91. On the §2.3 exclusion list. */
  metricsReset(): Promise<void>;
}
