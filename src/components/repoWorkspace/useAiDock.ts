/**
 * P68e — everything the AI activity dock needs from the container, assembled here.
 *
 * Extracted rather than inlined: `RepoWorkspace.tsx` is already ~3070 lines, and the
 * dock needs a store→props mapper, a selected-run cursor, the once-per-run
 * auto-expand guard, the imperative panel ref, the palette rows and a reveal entry
 * point for the conflict rows. Putting that in the container would be exactly the
 * god-file growth the house style forbids, so `RepoWorkspace` keeps ONE call, one
 * spread and three callbacks.
 *
 * U6 lives half here and half in `AiActivityPanel`: this hook decides when to
 * EXPAND (once per run key), the panel decides whether to FOCUS (only when the user
 * is demonstrably idle). Splitting it that way is deliberate — expanding a panel is
 * harmless, moving the caret out of a half-typed commit message is not.
 */
import { useCallback, useEffect, useMemo, useRef, useState } from 'react';

import { aiPaletteEntries, type PaletteAction } from '../paletteActions';
import type { AiActivityPanelProps, AiActivityRun } from '../aiDockFormat';
import type { AiActivityPanelHandle } from '../AiActivityPanel';
import type { PanelDensity, UiSettingsPatch } from '../../ipc';
import type { AiRunState, AiRunsApi } from './useAiRuns';

/** Store entry → dock prop. `tick` makes elapsed a pure function of state. */
function toActivityRun(run: AiRunState, tick: number): AiActivityRun {
  return {
    key: run.key,
    label: run.label,
    status: run.status,
    elapsedMs: Math.max(0, (run.endedAt ?? tick) - run.startedAt),
    costUsd: run.costUsd,
    question: run.question,
    error: run.error,
    partialText: run.partialText,
    log: run.log,
    logDropped: run.logDropped,
    files: run.files.map((f) => ({
      path: f.path,
      status: f.status,
      error: f.error,
      hasProposal: f.proposal !== null,
    })),
    paths: run.paths,
    cancelRequested: run.cancelRequested,
    turn: run.turn,
    thinkingTokens: run.thinkingTokens,
    openedInPane: run.openedInPane,
  };
}

export interface AiDockApi {
  /** Ready to spread onto `<AiActivityPanel {...panelProps} />`, ref included. */
  panelProps: AiActivityPanelProps & { ref: React.Ref<AiActivityPanelHandle> };
  /** §E rows: the two P55c/P56b leads plus the P68e dock entries. */
  paletteEntries: { lead: PaletteAction[]; trail: PaletteAction[] };
  /** The conflict row's live-run affordance: expand, select, focus the reply box. */
  revealForPath: (path: string) => void;
  /** The conflict row's `✓ review` affordance: re-open the stored proposal for this
   *  path from whichever run produced it (single OR bulk — P68f). Never re-runs the
   *  CLI. Lives here, beside `revealForPath` and `retryFile`, so the container is a
   *  composition site rather than a place that knows about run keys. */
  reviewForPath: (path: string) => void;
  /** `Ctrl/Cmd+Shift+A`: expand; focus the reply box if anything is blocked, else
   *  the log. Bound BEFORE the typing guard, so it works from the commit box. */
  focusDock: () => void;
  /** True while some run awaits an answer (drives the palette's Answer row). */
  awaitingInput: boolean;
  runCount: number;
}

export function useAiDock(deps: {
  aiRuns: AiRunsApi;
  /** Persisted `aiDockHeight` / `aiDockCollapsed` and the debounced patch sink. */
  height: number;
  collapsed: boolean;
  onChange: (patch: UiSettingsPatch) => void;
  density: PanelDensity;
  /** `UiSettings.aiStreamLog` — the dock says so rather than looking broken. */
  streamLogEnabled: boolean;
  aiEligible: boolean;
  /** MUST be referentially stable (a `useCallback`, not an inline arrow): both land in
   *  the `paletteEntries` memo below, and the palette's `actions` array is rebuilt from
   *  it — a fresh identity per render makes `CommandPalette` reset its highlight. */
  onAskBonsai: () => void;
  onChangelog: () => void;
}): AiDockApi {
  const { aiRuns, height, collapsed, onChange, density, streamLogEnabled } = deps;
  const panelRef = useRef<AiActivityPanelHandle | null>(null);
  const [activeKey, setActiveKey] = useState<string | null>(null);
  const autoExpandedForRef = useRef(new Set<string>());

  const runs = useMemo(
    () => aiRuns.orderedRuns.map((run) => toActivityRun(run, aiRuns.tick)),
    [aiRuns.orderedRuns, aiRuns.tick],
  );

  // `orderedRuns` gets a new identity on EVERY store commit — i.e. about once a second
  // while any run is live. `focusDock` therefore reads the list through a ref instead
  // of closing over it, so its own identity is stable and the palette's `actions` array
  // (which has this in its dependency chain) does not churn while an AI run streams.
  const runsRef = useRef(aiRuns.orderedRuns);
  runsRef.current = aiRuns.orderedRuns;

  const setCollapsed = useCallback(
    (next: boolean) => onChange({ aiDockCollapsed: next }),
    [onChange],
  );

  // §4.4-1: a run that starts asking auto-EXPANDS the dock once, and selects itself.
  // The guard is per run key, so a later manual collapse is respected.
  useEffect(() => {
    const asking = runs.find((r) => r.status === 'awaitingInput');
    if (asking === undefined || autoExpandedForRef.current.has(asking.key)) return;
    autoExpandedForRef.current.add(asking.key);
    setActiveKey(asking.key);
    setCollapsed(false);
  }, [runs, setCollapsed]);

  // Keep the cursor on a run that still exists (prune/dismiss can remove it).
  useEffect(() => {
    if (activeKey === null) return;
    if (!runs.some((r) => r.key === activeKey)) setActiveKey(null);
  }, [activeKey, runs]);

  const focusDock = useCallback(() => {
    const list = runsRef.current;
    if (list.length === 0) return;
    setCollapsed(false);
    const asking = list.find((r) => r.status === 'awaitingInput');
    if (asking !== undefined) setActiveKey(asking.key);
    // The body has to be mounted before anything in it can take focus.
    window.setTimeout(() => {
      if (asking !== undefined) panelRef.current?.focusReply();
      else panelRef.current?.focusLog();
    }, 0);
  }, [setCollapsed]);

  const revealForPath = useCallback(
    (path: string) => {
      const run = aiRuns.runForPath(path);
      if (run === null) return;
      setActiveKey(run.key);
      setCollapsed(false);
      if (run.status !== 'awaitingInput') return;
      window.setTimeout(() => panelRef.current?.focusReply(), 0);
    },
    [aiRuns, setCollapsed],
  );

  const reviewForPath = useCallback(
    (path: string) => {
      const run = aiRuns.runForPath(path);
      if (run !== null) aiRuns.reviewProposal(run.key, path);
    },
    [aiRuns],
  );

  // §5 Retry: a fresh single run for one failed file of a bulk run. The run `key` is
  // accepted and ignored so the dock's prop stays keyed by run, which is what lets
  // the other six AI runners adopt it later (D14).
  const retryFile = useCallback(
    (_key: string, path: string) => aiRuns.startConflictRun(path),
    [aiRuns],
  );

  const awaitingInput = runs.some((r) => r.status === 'awaitingInput');

  const paletteEntries = useMemo(
    () =>
      aiPaletteEntries({
        aiEligible: deps.aiEligible,
        onAskBonsai: deps.onAskBonsai,
        onChangelog: deps.onChangelog,
        hasAiRuns: runs.length > 0,
        aiAwaitingInput: awaitingInput,
        onAiActivity: focusDock,
      }),
    [deps.aiEligible, deps.onAskBonsai, deps.onChangelog, runs.length, awaitingInput, focusDock],
  );

  return {
    panelProps: {
      ref: panelRef,
      runs,
      activeKey: activeKey ?? runs[0]?.key ?? null,
      onSelectRun: setActiveKey,
      collapsed,
      onToggleCollapsed: setCollapsed,
      height,
      onResizeHeight: (next) => onChange({ aiDockHeight: next }),
      onCancel: aiRuns.cancelRun,
      onReply: aiRuns.replyRun,
      onDismiss: aiRuns.dismissRun,
      onReviewFile: aiRuns.reviewProposal,
      onRetryFile: retryFile,
      density,
      streamLogEnabled,
      atCapacity: aiRuns.atCapacity,
    },
    paletteEntries,
    revealForPath,
    reviewForPath,
    focusDock,
    awaitingInput,
    runCount: runs.length,
  };
}
