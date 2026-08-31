/**
 * P87b §7/§8 — the git-activity session store (View C + View D), the git twin of
 * `useAiRuns`.
 *
 * Subscribes ONCE to `ipc.gitActivitySubscribe` on mount; every git op that runs
 * hooks or does network I/O streams `GitActivityEvent`s onto that one channel and
 * lands here. Same D5 discipline as `useAiRuns`: an authoritative `runsRef`, a
 * render mirror committed on a 50 ms flush, log lines/progress buffered per id, a
 * status/phase change flushed immediately, and any event whose `seq <= last-seen`
 * for its id dropped (HMR/reload can redeliver).
 *
 * Retention (§8): per-run 500-line ring (`linesDropped` counts the overflow); a
 * 200-run session ring that evicts the OLDEST TERMINAL run and NEVER a running
 * one; session-scoped only (nothing persists across app restart).
 */
import { useCallback, useEffect, useMemo, useRef, useState } from 'react';

import { ipc } from '../../ipc';
import {
  GIT_ACTIVITY_FLUSH_MS,
  appendCappedLines,
  type GitActivityLine,
} from './gitActivityLog';
import {
  isTerminalGitStatus,
  newGitRun,
  pruneGitRuns,
  type GitActivityRun,
  type GitHookRecord,
} from './gitActivityState';
import type { GitActivityEvent } from '../../ipc';

export { GIT_ACTIVITY_FLUSH_MS, GIT_ACTIVITY_LINES_MAX } from './gitActivityLog';
export { GIT_ACTIVITY_RUNS_MAX, isTerminalGitStatus } from './gitActivityState';
export type { GitActivityLine } from './gitActivityLog';
export type { GitActivityRun, GitHookRecord, GitRunStatus } from './gitActivityState';

export interface GitActivityApi {
  /** Newest-first (View D log). */
  runs: GitActivityRun[];
  /** Newest `status==='running'` run (View C toolbar / commit-box readout). */
  activeRun: GitActivityRun | null;
  /** Clears terminal runs from the log; never touches a running run (§4.6). */
  clear(): void;
  /** True while ≥1 terminal run exists — gates the Clear button. */
  hasTerminalRuns: boolean;
  /** `Date.now()` as of the last commit or the 1 s interval tick; the interval
   *  runs ONLY while something is running. Elapsed = `tick - startedAt`. */
  tick: number;
}

export function useGitActivity(): GitActivityApi {
  // The AUTHORITATIVE store: a Map in insertion (chronological) order. Events
  // arrive from an IPC callback that closes over nothing, so buffered flushes
  // must never read stale React state — they read/write the ref.
  const runsRef = useRef<Map<string, GitActivityRun>>(new Map());
  const [view, setView] = useState<GitActivityRun[]>([]);
  const [tick, setTick] = useState(() => Date.now());

  // Per-id line buffer + the latest progress value awaiting the next flush.
  const lineBuf = useRef(new Map<string, GitActivityLine[]>());
  const flushTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const mounted = useRef(true);

  /** Newest-first snapshot of the ref store. */
  const snapshot = useCallback((): GitActivityRun[] => {
    return [...runsRef.current.values()].reverse();
  }, []);

  const commit = useCallback(() => {
    if (!mounted.current) return;
    setView(snapshot());
    setTick(Date.now());
  }, [snapshot]);

  /** Drain the line buffers into the ref store, then take ONE state commit. */
  const flush = useCallback(() => {
    if (flushTimer.current !== null) {
      clearTimeout(flushTimer.current);
      flushTimer.current = null;
    }
    for (const [id, lines] of lineBuf.current) {
      const entry = runsRef.current.get(id);
      if (entry === undefined) continue;
      const { log, dropped } = appendCappedLines(entry.lines, lines);
      runsRef.current.set(id, {
        ...entry,
        lines: log,
        linesDropped: entry.linesDropped + dropped,
      });
    }
    lineBuf.current.clear();
    commit();
  }, [commit]);

  const scheduleFlush = useCallback(() => {
    if (flushTimer.current !== null) return;
    flushTimer.current = setTimeout(flush, GIT_ACTIVITY_FLUSH_MS);
  }, [flush]);

  /** Immutable per-entry patch on the ref store. Does not commit. */
  const patch = useCallback((id: string, next: Partial<GitActivityRun>) => {
    const entry = runsRef.current.get(id);
    if (entry === undefined) return;
    runsRef.current.set(id, { ...entry, ...next });
  }, []);

  const onEvent = useCallback(
    (ev: GitActivityEvent) => {
      const now = Date.now();
      const existing = runsRef.current.get(ev.id);

      if (ev.kind === 'started') {
        // A redelivered `started` (reload) must not reset an existing run.
        if (existing !== undefined) return;
        const phase = ev.phase ?? { kind: 'preparing' };
        const category = ev.category ?? 'commit';
        runsRef.current.set(ev.id, newGitRun(ev.id, category, phase, ev.seq, now));
        // Enforce the 200-run cap; running runs are never evicted (§8).
        const pruned = pruneGitRuns([...runsRef.current.keys()], runsRef.current);
        if (pruned !== null) {
          const kept = new Map<string, GitActivityRun>();
          for (const id of pruned.kept) {
            const r = runsRef.current.get(id);
            if (r !== undefined) kept.set(id, r);
          }
          runsRef.current = kept;
          for (const id of pruned.dropped) lineBuf.current.delete(id);
        }
        commit();
        return;
      }

      // Every non-started event must reference a known, not-yet-superseded run.
      if (existing === undefined || ev.seq <= existing.seq) return;

      switch (ev.kind) {
        case 'phase': {
          if (ev.phase !== undefined) patch(ev.id, { phase: ev.phase, seq: ev.seq });
          flush();
          return;
        }
        case 'stdoutLine':
        case 'stderrLine': {
          patch(ev.id, { seq: ev.seq });
          if (ev.line !== undefined) {
            const stream = ev.kind === 'stderrLine' ? 'stderr' : 'stdout';
            const buf = lineBuf.current.get(ev.id) ?? [];
            buf.push({ seq: ev.seq, stream, text: ev.line });
            lineBuf.current.set(ev.id, buf);
          }
          scheduleFlush();
          return;
        }
        case 'hookDone': {
          const record: GitHookRecord = {
            hook: ev.hook ?? '',
            code: ev.code ?? null,
            success: ev.success ?? false,
            at: now,
          };
          patch(ev.id, { hooks: [...existing.hooks, record], seq: ev.seq });
          flush();
          return;
        }
        case 'progress': {
          // No line, no cap (§14.9): latest value wins on the next 50 ms flush.
          if (ev.progress !== undefined) patch(ev.id, { progress: ev.progress, seq: ev.seq });
          scheduleFlush();
          return;
        }
        case 'finished': {
          patch(ev.id, {
            status: ev.success === false ? 'failed' : 'success',
            code: ev.code ?? null,
            endedAt: now,
            seq: ev.seq,
          });
          flush();
          return;
        }
        default:
          return;
      }
    },
    [commit, flush, patch, scheduleFlush],
  );

  // Subscribe ONCE on mount. `gitActivitySubscribe` resolves immediately; the
  // callback stays live for the session. Re-armed on mount for StrictMode.
  useEffect(() => {
    mounted.current = true;
    void ipc.gitActivitySubscribe(onEvent).catch(() => undefined);
    return () => {
      mounted.current = false;
      if (flushTimer.current !== null) clearTimeout(flushTimer.current);
    };
  }, [onEvent]);

  const clear = useCallback(() => {
    let changed = false;
    const kept = new Map<string, GitActivityRun>();
    for (const [id, run] of runsRef.current) {
      if (isTerminalGitStatus(run.status)) {
        changed = true;
        lineBuf.current.delete(id);
      } else {
        kept.set(id, run);
      }
    }
    if (!changed) return;
    runsRef.current = kept;
    commit();
  }, [commit]);

  const runningCount = useMemo(
    () => view.filter((r) => !isTerminalGitStatus(r.status)).length,
    [view],
  );

  // D5: exactly ONE interval, only while something is running.
  useEffect(() => {
    if (runningCount === 0) return undefined;
    const id = setInterval(() => setTick(Date.now()), 1000);
    return () => clearInterval(id);
  }, [runningCount]);

  const activeRun = useMemo(
    () => view.find((r) => r.status === 'running') ?? null,
    [view],
  );

  const hasTerminalRuns = useMemo(
    () => view.some((r) => isTerminalGitStatus(r.status)),
    [view],
  );

  return { runs: view, activeRun, clear, hasTerminalRuns, tick };
}
