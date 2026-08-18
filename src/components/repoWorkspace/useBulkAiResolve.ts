/**
 * P68f §6.4 — the "Resolve all with AI" entry points.
 *
 * The user's request was literal: *"When there's a conflict, I want to have the
 * option to resolve all of them with AI, not just individually."*
 *
 * This hook is deliberately THIN, because everything hard already exists:
 * `ai_resolve_conflict_stream` has always taken a `paths: Vec<String>` (§12-A1), so
 * bulk is ONE run for all conflicts (D11 — the common case is one logical change
 * spread over several files, e.g. the reported i18n JSON), with the per-file
 * delimiters, exact-path attribution, the byte-cap batch split and per-file `failed`
 * marking all living in Rust. `useAiRuns.startBulkRun(paths)` already drives it and
 * already funnels the run through the single store the dock reads.
 *
 * So what is left, and all this file does:
 *   1. compute the ELIGIBLE path list — `bothModified` / `bothAdded`, the exact same
 *      `aiShown` gate the per-row ✨AI button uses, so an ineligible kind can never
 *      be dragged into a bulk run (§9 P68f acceptance);
 *   2. decide whether the affordance is offered at all (≥2 eligible: a single
 *      eligible conflict already has its row button, and a one-path "bulk" run would
 *      just be that run under a different key);
 *   3. arm the CONFIRM gate — this spends real money and touches N files at once, so
 *      it follows the repo's confirm-dialog idiom rather than firing on click;
 *   4. expose ONE control object that both entry points render identically, whose
 *      `Cancel` maps to ONE `ai_cancel_run` because there is only ONE run (P68e §6:
 *      the dock's own Cancel and this button are the same operation and therefore
 *      cannot disagree).
 *
 * No new hook state beyond the pending confirmation: the run itself lives in
 * `useAiRuns` (one store, one dock, one cancel).
 */
import { useCallback, useMemo, useState } from 'react';

import { isTerminalStatus, type AiRunState, type AiRunsApi } from './useAiRuns';
import type { AiAutonomy, ConflictEntry } from '../../ipc';

/**
 * P13 §8.2 / P68d: the two text-mergeable kinds — THE one definition. The per-row ✨AI
 * button (`StatusConflictsSection.aiShown`) and this bulk button must offer AI for
 * exactly the same set of files: if they diverged, a row would offer AI for a file bulk
 * refuses, or worse the other way round.
 *
 * `null` (a kind-lookup miss while the conflicts list is momentarily stale) is NOT
 * resolvable — the row hides the button rather than guessing.
 */
export function isAiResolvableKind(kind: ConflictEntry['kind'] | null): boolean {
  return kind === 'bothModified' || kind === 'bothAdded';
}

/** Everything an entry point needs; rendered by `BulkAiResolveButton`. */
export interface BulkAiControl {
  /** Render the affordance at all. `active` is part of it on purpose: the eligible list
   *  can drop below 2 WHILE a run is live — the user resolves a file by hand with
   *  `ours`/`theirs`, or a watcher/focus rescan lands a fresh status — and a
   *  `paths.length >= 2` test alone would make `Cancel all` vanish mid-run. (Not the
   *  run's own staging: `settle` patches the run terminal before it stages, so `active`
   *  is already false by then.) */
  shown: boolean;
  /** The eligible conflicted paths, in list order — exactly what a run is started with. */
  paths: string[];
  /** What the label talks about: the LIVE run's file count while active (the eligible
   *  count shrinks under it), else the eligible count. */
  count: number;
  /** A bulk run is in flight → this button is Cancel, not Resolve. */
  active: boolean;
  /** Disabled by AI state alone (consent/availability, the concurrency cap, an
   *  already-requested cancel). The host section ORs in its own `busy` — the section
   *  knows whether a mutation is in flight, this hook does not need to. */
  disabled: boolean;
  label: string;
  title: string;
  ariaLabel: string;
  onClick(): void;
}

/** The confirm gate's state; spread onto `BulkAiConfirmDialog`. */
export interface BulkAiConfirmState {
  open: boolean;
  /** Snapshotted at request time so the dialog cannot restate itself under the user. */
  paths: string[];
  /** Drives the one sentence that differs: `autoResolve` stages marker-free results. */
  autonomy: AiAutonomy;
  onConfirm(): void;
  onCancel(): void;
}

export interface BulkAiResolveApi {
  control: BulkAiControl;
  confirm: BulkAiConfirmState;
}

/** The live bulk run, or null. Identified by "covers more than one path and is not
 *  terminal" rather than by a `bulk:` key prefix, so it keeps working if the key
 *  scheme grows a third shape (D14). */
function findLiveBulkRun(runs: Record<string, AiRunState>): AiRunState | null {
  const live = Object.values(runs)
    .filter((r) => r.paths.length > 1 && !isTerminalStatus(r.status))
    .sort((a, b) => b.startedAt - a.startedAt);
  return live[0] ?? null;
}

export function useBulkAiResolve(deps: {
  conflicts: ConflictEntry[];
  /** P13 §8.2: AI enabled + consented + CLI installed. */
  aiEligible: boolean;
  aiConflictAutonomy: AiAutonomy;
  aiRuns: AiRunsApi;
}): BulkAiResolveApi {
  const { conflicts, aiEligible, aiConflictAutonomy, aiRuns } = deps;
  const [pending, setPending] = useState<string[] | null>(null);

  const paths = useMemo(
    () => conflicts.filter((c) => isAiResolvableKind(c.kind)).map((c) => c.path),
    [conflicts],
  );
  const live = useMemo(() => findLiveBulkRun(aiRuns.runs), [aiRuns.runs]);

  const onClick = useCallback(() => {
    // ONE run covers N files, so ONE cancel stops all of it (D7/P68e §6). No confirm
    // for cancel: it destroys nothing on disk (D4) and a modal in front of a run the
    // user is trying to stop would be hostile.
    if (live !== null) {
      aiRuns.cancelRun(live.key);
      return;
    }
    setPending(paths);
  }, [aiRuns, live, paths]);

  const onConfirm = useCallback(() => {
    const requested = pending;
    setPending(null);
    if (requested === null || requested.length < 2) return;
    // ONE `aiResolveConflictStream` call with ALL the paths — the locked "one run"
    // decision, so the model can reason across the files.
    aiRuns.startBulkRun(requested);
  }, [aiRuns, pending]);

  const onCancel = useCallback(() => setPending(null), []);

  const control: BulkAiControl = useMemo(() => {
    const active = live !== null;
    const count = active ? live.paths.length : paths.length;
    const stopping = live?.cancelRequested === true;
    return {
      shown: paths.length >= 2 || active,
      paths,
      count,
      active,
      disabled: active ? stopping : !aiEligible || aiRuns.atCapacity,
      label: active
        ? stopping
          ? 'Stopping…'
          : 'Cancel all'
        : '✨ Resolve all with AI',
      // P68g §2.4: these said "the one AI run" / "in ONE AI run", which the confirm
      // dialog now correctly denies — Rust packs the payload into batches by byte size,
      // so a click can spend more than one metered run. Claiming a count the code does
      // not guarantee is the same defect class as telling the user a result is somewhere
      // it is not, so the copy states the scope (all N files) and not the run count.
      title: active
        ? stopping
          ? 'Stopping the AI run…'
          : `Stop the AI work covering all ${count} files`
        : !aiEligible
          ? 'Enable AI features in Settings to use this'
          : aiRuns.atCapacity
            ? 'Too many AI runs in progress — cancel one and try again'
            : `Resolve all ${count} conflicted files together with AI`,
      ariaLabel: active
        ? `Cancel the AI run for all ${count} files`
        : `Resolve all ${count} conflicts with AI`,
      onClick,
    };
  }, [aiEligible, aiRuns.atCapacity, live, onClick, paths]);

  return {
    control,
    confirm: {
      open: pending !== null,
      paths: pending ?? [],
      autonomy: aiConflictAutonomy,
      onConfirm,
      onCancel,
    },
  };
}
