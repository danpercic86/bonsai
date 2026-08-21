// ---- P30: background-job scheduler (mirrors scheduler.rs / commands.rs) ----

/** The two Rust-side background jobs (P30 §5). */
export type JobKind = 'autoFetch' | 'healthRefresh';

/** Outcome of one job run. `skipped` = overlap guard; `suppressed` = an
 *  operation (merge/rebase/…) was in progress. */
export type JobOutcome = 'success' | 'failed' | 'suppressed' | 'skipped';

/** One background job's status for the UI readout (P30 §3). */
export interface JobStatus {
  job: JobKind;
  enabled: boolean;
  lastRunMs: number | null;
  lastOutcome: JobOutcome | null;
  lastError: string | null;
  consecutiveFailures: number;
  inBackoff: boolean;
  /** Estimate; null when disabled (or never seen by the loop yet). */
  nextRunMs: number | null;
}

/** Payload of the `job-status-changed` event (P30 §4). */
export interface JobStatusChangedPayload {
  repoId: string;
  job: JobKind;
  outcome: JobOutcome;
  /** autoFetch success only. */
  updatedRefs?: number;
  /** failed only. */
  error?: string;
  consecutiveFailures: number;
  inBackoff: boolean;
  /** true exactly on the 2→3 failure transition — toast ONLY then (D6). */
  enteredBackoff: boolean;
  tsMs: number;
  nextRunMs: number | null;
}
