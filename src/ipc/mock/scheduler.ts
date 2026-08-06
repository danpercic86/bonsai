// Split out of the former monolithic mock.ts (pure refactor; no behavior change).
import { jobStatusListeners, repoChangedListeners } from './events';
import { readUiSettings } from './persistence';
import { repos } from './repoState';
import type { JobKind, JobStatus, JobStatusChangedPayload, RepoChangedPayload, UiSettings } from '../types';

const MOCK_JOB_FAIL_KEY = 'bonsaiMockJobFail';
const BACKOFF_THRESHOLD = 3; // mirrors scheduler.rs
const BACKOFF_MAX_FACTOR = 8;
export const jobStatuses = new Map<string, JobStatus[]>();
/** Lazy per-repo seed (contract §7): autoFetch success 2 min ago,
 *  healthRefresh disabled/never run. */
export function seedJobStatuses(repoId: string): JobStatus[] {
  let list = jobStatuses.get(repoId);
  if (list === undefined) {
    list = [
      {
        job: 'autoFetch',
        enabled: false,
        lastRunMs: Date.now() - 2 * 60_000,
        lastOutcome: 'success',
        lastError: null,
        consecutiveFailures: 0,
        inBackoff: false,
        nextRunMs: null,
      },
      {
        job: 'healthRefresh',
        enabled: false,
        lastRunMs: null,
        lastOutcome: null,
        lastError: null,
        consecutiveFailures: 0,
        inBackoff: false,
        nextRunMs: null,
      },
    ];
    jobStatuses.set(repoId, list);
  }
  return list;
}

/** Mirrors scheduler.rs `effective_interval_ms` (base for failures 0–2,
 *  base*2^(f-2) for ≥3, capped at 8×). */
export function mockEffectiveIntervalMs(baseMs: number, failures: number): number {
  if (failures < BACKOFF_THRESHOLD) return baseMs;
  const factor = Math.min(BACKOFF_MAX_FACTOR, 2 ** (failures - (BACKOFF_THRESHOLD - 1)));
  return baseMs * factor;
}

export function mockJobFailEnabled(): boolean {
  try {
    return window.localStorage.getItem(MOCK_JOB_FAIL_KEY) === '1';
  } catch {
    return false;
  }
}

/** Completes one synthetic job run for `repoId`: updates the stateful status,
 *  dispatches `job-status-changed`, then `repo-changed` on refreshing
 *  successes — the same ordering/shape as the Rust scheduler. */
export function completeMockJobRun(repoId: string, job: JobKind): void {
  const state = repos.get(repoId);
  if (state === undefined) return;
  const settings = readUiSettings();
  const cfg = job === 'autoFetch' ? settings.autoFetch : settings.healthRefresh;
  const entry = seedJobStatuses(repoId).find((s) => s.job === job);
  if (entry === undefined) return;

  // Failure shim: only autoFetch on the fixture repo escalates.
  const failed =
    job === 'autoFetch' && mockJobFailEnabled() && state.path.includes('bonsai-fixture');
  const now = Date.now();
  const failures = failed ? entry.consecutiveFailures + 1 : 0;
  const inBackoff = failures >= BACKOFF_THRESHOLD;
  const enteredBackoff = failed && failures === BACKOFF_THRESHOLD;
  // Test-speed shim: intervalMinutes as SECONDS.
  const nextRunMs = cfg.enabled
    ? now + mockEffectiveIntervalMs(cfg.intervalMinutes * 1000, failures)
    : null;
  const updatedRefs = !failed && job === 'autoFetch' ? 2 : undefined;
  const error = failed ? 'mock: could not connect to origin (bonsaiMockJobFail=1)' : undefined;

  entry.enabled = cfg.enabled;
  entry.lastRunMs = now;
  entry.lastOutcome = failed ? 'failed' : 'success';
  entry.lastError = error ?? null;
  entry.consecutiveFailures = failures;
  entry.inBackoff = inBackoff;
  entry.nextRunMs = nextRunMs;

  const payload: JobStatusChangedPayload = {
    repoId,
    job,
    outcome: failed ? 'failed' : 'success',
    updatedRefs,
    error,
    consecutiveFailures: failures,
    inBackoff,
    enteredBackoff,
    tsMs: now,
    nextRunMs,
  };
  for (const cb of jobStatusListeners) cb(payload);
  // Rust emits repo-changed on autoFetch success with updatedRefs > 0 and on
  // every healthRefresh success.
  if (!failed && (job === 'healthRefresh' || (updatedRefs ?? 0) > 0)) {
    const rc: RepoChangedPayload = { repoId, reason: 'fs' };
    for (const cb of repoChangedListeners) cb(rc);
  }
}

const jobTimers: { autoFetch: number | null; healthRefresh: number | null } = {
  autoFetch: null,
  healthRefresh: null,
};

/** (Re)arms the synthetic tick timers from the given settings — called at
 *  module init and after every setUiSettings round-trip. */
export function applyMockJobTimers(s: UiSettings): void {
  for (const job of ['autoFetch', 'healthRefresh'] as const) {
    const timer = jobTimers[job];
    if (timer !== null) {
      window.clearInterval(timer);
      jobTimers[job] = null;
    }
    const cfg = job === 'autoFetch' ? s.autoFetch : s.healthRefresh;
    if (cfg.enabled) {
      jobTimers[job] = window.setInterval(() => {
        for (const repoId of repos.keys()) completeMockJobRun(repoId, job);
      }, cfg.intervalMinutes * 1000); // minutes-as-seconds shim
    }
  }
}

// Arm timers from persisted settings so a reload keeps ticking.
applyMockJobTimers(readUiSettings());
