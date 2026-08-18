/**
 * P68d §C — the per-path AI run store. THIS IS THE FIX FOR USER ITEM 5.
 *
 * The report was: "I clicked Resolve with AI, switched to another conflicted file,
 * and when it finished nothing was resolved; staying on the same file worked." That
 * was two independent single-slot bugs, and both are gone here:
 *
 *  (a) In-flight state was ONE scalar (`RepoWorkspace.aiResolvingPath`), and the
 *      conflicts section did `aiDisabled={aiResolvingPath !== null}` — so a run on
 *      one file disabled EVERY row's ✨AI button. This store is keyed per run, and
 *      the only thing that disables a row now is the concurrency cap.
 *
 *  (b) The result sink was the ONE global `diffSlot`, guarded by the SHARED
 *      `fileDiffReqId` that ordinary diff opening also bumps. The old sequence was
 *      `++fileDiffReqId` → `await ipc.aiResolveConflict(path)` → `if (id !==
 *      fileDiffReqId.current) return;`, so opening any other file during the run —
 *      *even after the CLI call had already succeeded* — silently discarded the
 *      computed proposal: no toast, no cache, no retry. Here the proposal lands in
 *      THIS store, which no diff open can touch; opening the review editor is a
 *      separate, explicitly-triggered `openAiProposal`. A file switch can lose the
 *      editor SLOT (re-openable from the row's `✓ review`) but never the proposal.
 *
 * Keys are `conflict:<path>` (and `bulk:<startedAt>` for P68f), deliberately shaped
 * so `analyze:<oid>` fits when the other six AI runners adopt the dock (D14).
 *
 * D5 — NO per-line re-render: `RepoWorkspace.tsx` is ~3050 lines, so a `setState`
 * per log line would repaint the whole workspace at CLI output speed. Log lines and
 * heartbeat metrics accumulate in refs and commit on ONE shared 50 ms timer;
 * status-changing events flush immediately.
 *
 * D4 — this store WRITES NOTHING itself. Staging goes through the single
 * `applyResolution` dep (= `handleResolveConflictText`).
 */
import { useCallback, useEffect, useMemo, useRef, useState } from 'react';

import { ipc } from '../../ipc';
import { AI_MAX_CONCURRENT_RUNS } from '../../settings/ranges';
import { errorMessage, isAppError } from '../../utils/errors';
import { AI_LOG_FLUSH_MS, appendCapped, type AiRunLogLine } from './aiRunLog';
import { decideEvent } from './aiRunEvent';
import {
  conflictKey,
  deriveRowStates,
  isTerminalStatus,
  newRun,
  pruneRuns,
  settleBatch,
  type AiRowState,
  type AiRunState,
} from './aiRunState';
import type { AiAutonomy, AiResolveBatch, AiRunEvent } from '../../ipc';

export { AI_LOG_FLUSH_MS, AI_LOG_MAX } from './aiRunLog';
export type { AiRunLogKind, AiRunLogLine } from './aiRunLog';
export { AI_MAX_CONCURRENT_RUNS } from '../../settings/ranges';
// The state shape and its pure transforms live next door (file-size discipline);
// re-exported here so consumers keep one import site.
export {
  AI_TERMINAL_RUNS_MAX,
  conflictKey,
  isTerminalStatus,
  settleBatch,
} from './aiRunState';
export type { AiRowState, AiRunFileState, AiRunState, AiRunStatus } from './aiRunState';

export interface AiRunsApi {
  runs: Record<string, AiRunState>;
  /** Newest first. */
  orderedRuns: AiRunState[];
  runForPath(path: string): AiRunState | null;
  /** Ready-made row affordance input, keyed by conflicted path. */
  rowStates: Record<string, AiRowState>;
  /** How many runs are live (running or awaiting input). */
  runningCount: number;
  /** True when the concurrency cap is reached (OQ1) — the ONLY thing that disables
   *  a conflict row's ✨AI button. */
  atCapacity: boolean;
  startConflictRun(path: string): void;
  startBulkRun(paths: string[]): void;
  cancelRun(key: string): void;
  replyRun(key: string, text: string): void;
  /** Re-open a ready proposal in the center-pane review editor. */
  reviewProposal(key: string, path: string): void;
  /** Remove a terminal run from the store (dock ✕). No-op while running. */
  dismissRun(key: string): void;
  /** `Date.now()` as of the last commit or the last one-second interval fire; the
   *  interval exists ONLY while a run is active (D5). Consumers derive elapsed as
   *  `tick - startedAt` so a re-render is the only thing that can change it. */
  tick: number;
}

export interface AiRunsDeps {
  repoId: string;
  pushToast: (level: 'info' | 'success' | 'error', msg: string) => void;
  aiConflictAutonomy: AiAutonomy;
  aiEligible: boolean;
  /** = `handleResolveConflictText` — the ONLY writer (D4). The third argument
   *  overrides its success toast so the AI path keeps its own P13 copy instead of
   *  double-toasting (`null` = stay silent, used for a bulk stage that summarises
   *  once); the fourth defers the `refreshAll` to the caller (P68f — one refresh for
   *  the whole batch, not one per file). */
  applyResolution: (
    path: string,
    text: string,
    successMessage?: string | null,
    deferRefresh?: boolean,
  ) => Promise<void>;
  /** P68f: refresh status/graph ONCE after a multi-file stage. `applyResolution` used
   *  to refresh per file, so an N-file bulk `autoResolve` did N full refreshes. */
  refreshAll: () => Promise<void>;
  /** = `useMergeActions.openAiProposal` — opens the center-pane review editor. */
  openAiProposal: (path: string, proposedText: string) => Promise<void>;
  /** Currently conflicted paths: a terminal run whose paths are all resolved is
   *  pruned, so the store does not accumulate stale entries across a long merge. */
  conflictPaths: string[];
  /**
   * P68e FOLD-IN 1 — identity of whatever the CENTER PANE currently shows (the diff
   * slot's key, or null).
   *
   * Fixing item-5 removed an accidental side benefit: the old reqId guard meant a
   * finished run could never steal the center pane, because a superseded open simply
   * returned. With the guard gone, `settle → openAiProposal` opens unconditionally,
   * so a user reading file B's diff can have it replaced 40 s later by file A's
   * proposal — repeatedly under a bulk run.
   *
   * USER DECISION: auto-open only if the user has NOT navigated away. This callback
   * is sampled when the run STARTS and again at settle; if the two differ, nothing is
   * stolen — the row's `✓ review` badge, the dock's `Review proposal` button and the
   * toast are the affordance, and the proposal stays in the store either way.
   *
   * Optional so the six other (non-conflict) runners can adopt the store later
   * without inventing a slot; absent ⇒ never suppress, i.e. the pre-fold-in
   * behaviour.
   */
  diffSlotKey?: () => string | null;
}

export function useAiRuns(deps: AiRunsDeps): AiRunsApi {
  // Only `conflictPaths` is read during render (the prune effect). Everything else
  // is reached through `depsRef` because the run driver outlives the render that
  // started it — reading a captured `pushToast`/`aiConflictAutonomy` there is
  // exactly how a stale-closure bug gets in.
  const { conflictPaths } = deps;

  // The AUTHORITATIVE store is the ref: events arrive from an IPC callback that
  // closes over nothing else, and buffered flushes must never read stale state.
  // `view` is the render mirror, replaced only when we deliberately commit.
  const runsRef = useRef<Record<string, AiRunState>>({});
  const [view, setView] = useState<Record<string, AiRunState>>({});
  /** The clock the row/dock affordances read, as a TIMESTAMP rather than a counter:
   *  it makes `elapsedSecs` a pure function of state (no `Date.now()` inside a memo,
   *  so React and eslint both see the real dependency) and it is refreshed from
   *  exactly two places — the once-a-second interval that runs ONLY while something is
   *  active (D5), and every commit, so a status change updates elapsed immediately. */
  const [tick, setTick] = useState(() => Date.now());

  const logBuf = useRef(new Map<string, AiRunLogLine[]>());
  /** Buffered non-log scalars (heartbeat tokens, turn) — same 50 ms commit. */
  const metaBuf = useRef(new Map<string, { thinkingTokens?: number; turn?: number }>());
  const flushTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const mounted = useRef(true);
  /** FOLD-IN 1: which diff slot the user was looking at when each run started. */
  const slotAtStart = useRef(new Map<string, string | null>());

  // Latest deps for the async run driver, which outlives any single render.
  const depsRef = useRef(deps);
  depsRef.current = deps;

  const commit = useCallback(() => {
    if (!mounted.current) return;
    setView({ ...runsRef.current });
    setTick(Date.now());
  }, []);

  /**
   * Drain the buffers into the ref store, then take ONE state commit (D5).
   *
   * It commits UNCONDITIONALLY: callers reach it either from the 50 ms timer (which
   * is only armed after something was buffered) or right after a `patch` for a
   * status-changing event, and in the latter case the buffers are usually empty while
   * the render mirror is exactly what has to move.
   */
  const flush = useCallback(() => {
    if (flushTimer.current !== null) {
      clearTimeout(flushTimer.current);
      flushTimer.current = null;
    }
    for (const [key, lines] of logBuf.current) {
      const entry = runsRef.current[key];
      if (entry === undefined) continue;
      const { log, dropped } = appendCapped(entry.log, lines);
      runsRef.current[key] = { ...entry, log, logDropped: entry.logDropped + dropped };
    }
    logBuf.current.clear();
    for (const [key, meta] of metaBuf.current) {
      const entry = runsRef.current[key];
      if (entry === undefined) continue;
      runsRef.current[key] = {
        ...entry,
        thinkingTokens: meta.thinkingTokens ?? entry.thinkingTokens,
        turn: meta.turn ?? entry.turn,
      };
    }
    metaBuf.current.clear();
    commit();
  }, [commit]);

  const scheduleFlush = useCallback(() => {
    if (flushTimer.current !== null) return;
    flushTimer.current = setTimeout(flush, AI_LOG_FLUSH_MS);
  }, [flush]);

  /** Immutable per-entry patch. Does not commit — the caller decides when. */
  const patch = useCallback((key: string, next: Partial<AiRunState>) => {
    const entry = runsRef.current[key];
    if (entry === undefined) return;
    runsRef.current[key] = { ...entry, ...next };
  }, []);

  // P68e BUGFIX. This used to be cleanup-only, relying on `useRef(true)` for the
  // mounted state — which is wrong under React 19 StrictMode, where the dev-mode
  // mount → cleanup → mount cycle runs on the SAME component instance and therefore
  // the same ref. The cleanup latched `mounted.current = false` and nothing ever set
  // it back, so `commit()` returned early FOREVER: no row status, no dock, no visible
  // run at all in `pnpm dev` / `pnpm tauri dev`. Re-arming it on mount is the fix.
  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
      if (flushTimer.current !== null) clearTimeout(flushTimer.current);
    };
  }, []);

  // ---------------------------------------------------------------- events

  /** Apply the pure decision from `aiRunEvent.ts`. No interpretation happens here. */
  const onEvent = useCallback(
    (key: string, ev: AiRunEvent) => {
      const entry = runsRef.current[key];
      if (entry === undefined) return;
      const d = decideEvent(entry, ev, Date.now());
      if (Object.keys(d.patch).length > 0) patch(key, d.patch);
      if (d.logLine !== null) {
        const lines = logBuf.current.get(key) ?? [];
        lines.push(d.logLine);
        logBuf.current.set(key, lines);
      }
      if (d.thinkingTokens !== null) {
        const meta = metaBuf.current.get(key) ?? {};
        meta.thinkingTokens = d.thinkingTokens;
        metaBuf.current.set(key, meta);
      }
      if (d.fireQueuedCancel !== null) {
        void ipc.aiCancelRun(d.fireQueuedCancel).catch(() => undefined);
      }
      if (d.flushNow) flush();
      else if (d.logLine !== null || d.thinkingTokens !== null) scheduleFlush();
    },
    [flush, patch, scheduleFlush],
  );

  // ---------------------------------------------------------------- settle

  /**
   * Turn the resolved batch into per-file state and route it by autonomy.
   *
   * THE SAFETY GATE (blocking requirement from the P68b review): a `proposedText`
   * here is a REVIEWABLE proposal, not a verified-clean merge — the single-path
   * stream returns the model's body verbatim (P13 parity), and bulk's markerful
   * check lives on the other side of the IPC boundary. So `hasUnresolvedMarkers` is
   * applied HERE, before anything can be staged, exactly as the deleted
   * `handleAiResolveConflict` did. Nothing markerful is ever presented as clean.
   */
  const settle = useCallback(
    async (key: string, batch: AiResolveBatch) => {
      const entry = runsRef.current[key];
      if (entry === undefined) return;
      const d = depsRef.current;
      const autonomy = d.aiConflictAutonomy;
      // `settleBatch` owns the arithmetic AND the markerful safety gate (see
      // `aiRunState.ts`): a body that still carries conflict markers is demoted to
      // `failed` for its row and can never reach `applyResolution`.
      const out = settleBatch(entry.paths, batch, autonomy);

      patch(key, {
        files: out.files,
        proposal: out.proposal,
        costUsd: batch.costUsd ?? entry.costUsd,
        status: out.status,
        error: out.error,
        endedAt: Date.now(),
      });
      flush();

      for (const f of out.markerful) {
        if (f.error !== null) d.pushToast('error', f.error);
      }

      if (autonomy === 'autoResolve' && out.stageable.length > 0) {
        // P68f: stage every marker-free file, then refresh ONCE. Anything markerful was
        // already demoted to `failed` by `settleBatch` above, so it cannot get here —
        // that is the safety gate, and it runs BEFORE `stageable` is computed.
        const many = out.stageable.length > 1;
        let staged = 0;
        for (const f of out.stageable) {
          try {
            await d.applyResolution(
              f.path,
              f.proposal ?? '',
              // Bulk: stay silent per file and summarise once, instead of N toasts.
              many ? null : `Resolved ${f.path} with AI — review the staged result`,
              true,
            );
            staged += 1;
          } catch {
            // applyResolution already toasted; keep going for the other files.
          }
        }
        await d.refreshAll();
        if (many && staged > 0) {
          d.pushToast(
            'success',
            `Resolved ${staged} file${staged === 1 ? '' : 's'} with AI — review the staged results`,
          );
        }
      }

      // ONE center pane opens: the marker fallback under autoResolve, otherwise the
      // first ready proposal. A bulk run with several ready files opens nothing and
      // points at the activity dock instead.
      const toOpen = autonomy === 'autoResolve' ? out.markerful[0] : out.stageable[0];
      const text = toOpen?.proposal ?? null;
      if (toOpen === undefined || text === null) return;
      if (autonomy === 'proposeReview') {
        // FOLD-IN 1: the pane is only taken when the user is still looking at what
        // they were looking at when the run started.
        const stayed = slotAtStart.current.get(key) === (d.diffSlotKey?.() ?? null);
        if (out.stageable.length > 1) {
          d.pushToast(
            'success',
            `AI proposals ready for ${out.stageable.length} files — review them from the AI activity dock`,
          );
          return;
        }
        d.pushToast(
          'success',
          stayed
            ? `AI proposal ready for ${toOpen.path} — opened for review`
            : `AI proposal ready for ${toOpen.path} — review it from the AI activity dock`,
        );
        if (!stayed) return;
      }
      // The markerful fallback under `autoResolve` opens UNCONDITIONALLY: its row
      // shows `⚠` (retry), so this open is the only path to that body, and the whole
      // point is that the user must see what the model actually produced. Under BULK
      // (P68f) several files can be markerful at once; only `markerful[0]` is opened,
      // so N finishing files still take the centre pane AT MOST ONCE — the rest are
      // reachable from their queue rows and each already got its own error toast.
      //
      // P68e M1: record that the pane really was taken, BEFORE the await, so the dock
      // renders `Proposal is open in the center pane.` only in this branch — the
      // suppressed branch gets the sentence that points at the dock instead.
      patch(key, { openedInPane: true });
      flush();
      await d.openAiProposal(toOpen.path, text);
    },
    [flush, patch],
  );

  // ---------------------------------------------------------------- drive

  const drive = useCallback(
    async (key: string, paths: string[]) => {
      const d = depsRef.current;
      try {
        const batch = await ipc.aiResolveConflictStream(d.repoId, paths, (ev) => onEvent(key, ev));
        await settle(key, batch);
      } catch (e) {
        // ONE catch path (D7): cancel arrives as an `aiCancelled` rejection.
        const cancelled = isAppError(e) && e.kind === 'aiCancelled';
        patch(key, {
          status: cancelled ? 'cancelled' : 'failed',
          error: cancelled ? null : errorMessage(e),
          endedAt: Date.now(),
        });
        flush();
        if (!cancelled) d.pushToast('error', errorMessage(e));
      }
    },
    [flush, onEvent, patch, settle],
  );

  const start = useCallback(
    (key: string, label: string, paths: string[]) => {
      const d = depsRef.current;
      if (!d.aiEligible) {
        d.pushToast('error', 'AI features are off. Turn them on in Settings → AI.');
        return;
      }
      const existing = runsRef.current[key];
      if (existing !== undefined && !isTerminalStatus(existing.status)) return;
      const live = Object.values(runsRef.current).filter(
        (r) => !isTerminalStatus(r.status),
      ).length;
      if (live >= AI_MAX_CONCURRENT_RUNS) {
        d.pushToast(
          'error',
          `Too many AI runs in progress (${live} of ${AI_MAX_CONCURRENT_RUNS} allowed) — cancel one and try again.`,
        );
        return;
      }
      // A retry replaces the previous terminal entry for the same key, buffers and
      // all, so a stale log cannot bleed into the new run.
      logBuf.current.delete(key);
      metaBuf.current.delete(key);
      slotAtStart.current.set(key, d.diffSlotKey?.() ?? null);
      runsRef.current[key] = newRun(key, label, paths, Date.now());
      commit();
      void drive(key, paths);
    },
    [commit, drive],
  );

  const startConflictRun = useCallback(
    (path: string) => start(conflictKey(path), path, [path]),
    [start],
  );

  const startBulkRun = useCallback(
    (paths: string[]) => {
      if (paths.length === 0) return;
      if (paths.length === 1) {
        startConflictRun(paths[0] ?? '');
        return;
      }
      start(`bulk:${Date.now()}`, `${paths.length} conflicts`, paths);
    },
    [start, startConflictRun],
  );

  const cancelRun = useCallback(
    (key: string) => {
      const entry = runsRef.current[key];
      if (entry === undefined || isTerminalStatus(entry.status)) return;
      // Immediate feedback even before the id exists (D8 / P68e §12-A2).
      patch(key, { cancelRequested: true });
      commit();
      if (entry.runId !== null) void ipc.aiCancelRun(entry.runId).catch(() => undefined);
    },
    [commit, patch],
  );

  const replyRun = useCallback(
    (key: string, text: string) => {
      const entry = runsRef.current[key];
      if (entry === undefined || entry.runId === null || entry.status !== 'awaitingInput') return;
      patch(key, { status: 'running', question: null });
      commit();
      void ipc.aiReplyRun(entry.runId, text).catch((e: unknown) => {
        depsRef.current.pushToast('error', errorMessage(e));
      });
    },
    [commit, patch],
  );

  const reviewProposal = useCallback(
    (key: string, path: string) => {
      const entry = runsRef.current[key];
      if (entry === undefined) return;
      const file = entry.files.find((f) => f.path === path);
      const text = file?.proposal ?? (entry.paths.length === 1 ? entry.proposal : null);
      if (text === null || text === undefined) return;
      // M1: the user opened it themselves, so the pane IS showing it — the dock's hint
      // flips to the "open in the center pane" sentence from here on.
      patch(key, { openedInPane: true });
      commit();
      void depsRef.current.openAiProposal(path, text);
    },
    [commit, patch],
  );

  const dismissRun = useCallback(
    (key: string) => {
      const entry = runsRef.current[key];
      if (entry === undefined || !isTerminalStatus(entry.status)) return;
      const { [key]: _gone, ...rest } = runsRef.current;
      runsRef.current = rest;
      logBuf.current.delete(key);
      metaBuf.current.delete(key);
      slotAtStart.current.delete(key);
      commit();
    },
    [commit],
  );

  // ------------------------------------------------------- elapsed + prune

  const runningCount = useMemo(
    () => Object.values(view).filter((r) => !isTerminalStatus(r.status)).length,
    [view],
  );

  // D5: exactly ONE interval, and only while something is actually running.
  useEffect(() => {
    if (runningCount === 0) return undefined;
    const id = setInterval(() => setTick(Date.now()), 1000);
    return () => clearInterval(id);
  }, [runningCount]);

  // P68e §12-A4: keep the store from filling with stale entries over a long merge.
  useEffect(() => {
    const pruned = pruneRuns(runsRef.current, conflictPaths);
    if (pruned === null) return;
    for (const key of pruned.dropped) {
      logBuf.current.delete(key);
      metaBuf.current.delete(key);
      slotAtStart.current.delete(key);
    }
    runsRef.current = pruned.kept;
    commit();
  }, [commit, conflictPaths]);

  // ---------------------------------------------------------------- derived

  const orderedRuns = useMemo(
    () => Object.values(view).sort((a, b) => b.startedAt - a.startedAt),
    [view],
  );

  const runForPath = useCallback(
    (path: string): AiRunState | null => {
      const direct = view[conflictKey(path)];
      if (direct !== undefined) return direct;
      // A bulk run covers several paths under one key (P68f).
      const covering = Object.values(view)
        .filter((r) => r.paths.includes(path))
        .sort((a, b) => b.startedAt - a.startedAt);
      return covering[0] ?? null;
    },
    [view],
  );

  const rowStates = useMemo(() => deriveRowStates(view, tick), [view, tick]);

  return {
    runs: view,
    orderedRuns,
    runForPath,
    rowStates,
    runningCount,
    atCapacity: runningCount >= AI_MAX_CONCURRENT_RUNS,
    startConflictRun,
    startBulkRun,
    cancelRun,
    replyRun,
    reviewProposal,
    dismissRun,
    tick,
  };
}
