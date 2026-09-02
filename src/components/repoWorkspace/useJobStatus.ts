// P30 D11 / P30 §6: the background-job status readout for one open repo — the
// `get_job_status` mount snapshot, the live `job-status-changed` subscription
// (upsert + the backoff/fetched toasts) and the 30 s relative-time ticker.
// Extracted verbatim from RepoWorkspace so the container only wires it; this
// block renders status + toasts only and must NOT double-refresh (auto-fetch
// itself runs in the Rust scheduler).
import { useEffect, useState } from 'react';
import { ipc } from '../../ipc';
import type { JobStatus, Unsubscribe } from '../../ipc';
import type { PushToast } from '../../ToastContext';

export interface UseJobStatus {
  jobStatus: JobStatus[];
  jobNow: number;
}

export function useJobStatus(repoId: string, pushToast: PushToast): UseJobStatus {
  // P30 D11: background-job status readout (fed by get_job_status on mount +
  // live job-status-changed events); jobNow re-renders the relative label.
  const [jobStatus, setJobStatus] = useState<JobStatus[]>([]);
  const [jobNow, setJobNow] = useState(() => Date.now());

  // P30 §6: the P11e frontend auto-fetch timer is GONE — auto-fetch now runs
  // in the Rust scheduler for ALL open repos (scheduler.rs); data refresh
  // arrives via the emitted `repo-changed`. This block only renders status
  // (D11 readout) + toasts — it must NOT double-refresh.
  useEffect(() => {
    let cancelled = false;
    const unsubs: Unsubscribe[] = [];
    // Initial snapshot on mount (D11).
    void ipc
      .getJobStatus(repoId)
      .then((list) => {
        if (!cancelled) setJobStatus(list);
      })
      .catch(() => {
        // Non-fatal: the readout simply stays hidden until the first event.
      });
    const subscribe = async () => {
      const off = await ipc.onJobStatusChanged((p) => {
        if (p.repoId !== repoId) return;
        setJobStatus((prev) => {
          // Upsert: the mount snapshot may predate the user enabling the job
          // (or may have failed) — a run event implies the job is enabled.
          const updated = {
            job: p.job,
            enabled: true,
            lastRunMs: p.tsMs,
            lastOutcome: p.outcome,
            lastError: p.error ?? null,
            consecutiveFailures: p.consecutiveFailures,
            inBackoff: p.inBackoff,
            nextRunMs: p.nextRunMs,
          };
          return prev.some((s) => s.job === p.job)
            ? prev.map((s) => (s.job === p.job ? { ...s, ...updated } : s))
            : [...prev, updated];
        });
        // SINGLE toast on the 2→3 failure transition (D6) — individual
        // background failures stay silent (D9).
        if (p.enteredBackoff) {
          pushToast('warning', 'Auto-fetch failing — backing off');
        }
        // §6.2: the quiet "Fetched N refs" success toast (data refresh itself
        // arrives via the scheduler's repo-changed emit).
        if (
          p.job === 'autoFetch' &&
          p.outcome === 'success' &&
          p.updatedRefs !== undefined &&
          p.updatedRefs > 0
        ) {
          pushToast('info', `Fetched ${p.updatedRefs} ref${p.updatedRefs === 1 ? '' : 's'}`);
        }
      });
      if (cancelled) {
        off();
        return;
      }
      unsubs.push(off);
    };
    // Subscription loss = degraded job-status readout only — log, don't crash.
    void subscribe().catch((e: unknown) => {
      console.error('job-status subscription failed', e);
    });
    return () => {
      cancelled = true;
      for (const unsub of unsubs) unsub();
    };
  }, [repoId, pushToast]);

  // Keep the relative-time readout fresh (30 s granularity is plenty).
  useEffect(() => {
    const id = window.setInterval(() => setJobNow(Date.now()), 30_000);
    return () => window.clearInterval(id);
  }, []);

  return { jobStatus, jobNow };
}
