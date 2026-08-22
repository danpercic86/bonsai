/**
 * P87b — everything the git activity dock (View D) + the toolbar/commit-box phase
 * readout (View C) need from the container, assembled here (mirrors `useAiDock`).
 *
 * Geometry is SESSION-ONLY (P87-ui §3.1 / Q5, orchestrator-recommended): the
 * collapsed flag + height live in this hook's state, NOT in `UiSettings`, so the
 * settings contract is untouched. The dock NEVER auto-expands (§3.1).
 */
import { useCallback, useMemo, useRef, useState } from 'react';

import { useGitActivity } from './useGitActivity';
import { gitPaletteEntries, type PaletteAction } from '../paletteActions';
import {
  GIT_DOCK_HEIGHT_DEFAULT,
  objectsReadout,
  phaseLabel,
  progressFraction,
} from '../gitActivityFormat';
import type { GitActivityDockHandle, GitActivityDockProps } from '../GitActivityDock';
import type { GitActivityCategory, PanelDensity } from '../../ipc';
import type { GitActivityRun } from './useGitActivity';

/** The View C toolbar props bundle (spread onto `<WorkspaceToolbar>`). */
export interface GitToolbarProps {
  gitCategory: GitActivityCategory | null;
  gitPhase: string | null;
  gitProgress: number | null;
  onShowGitActivity: () => void;
}

export interface GitDockApi {
  /** Ready to spread onto `<GitActivityDock {...panelProps} />`, ref included. */
  panelProps: GitActivityDockProps & { ref: React.Ref<GitActivityDockHandle> };
  /** The `Git activity` palette row (gated on there being a run). */
  paletteEntries: PaletteAction[];
  /** Palette / `Ctrl-Shift-L` / the clickable toolbar readout: expand + focus. */
  focusDock: () => void;
  /** `Ctrl/Cmd+Shift+L`: toggle collapsed (expand+focus, or collapse). */
  toggleDock: () => void;
  /** The active run for View C (drives the determinate progress bar). */
  activeRun: GitActivityRun | null;
  /** View C toolbar props (participle category + phase/transfer readout +
   *  determinate fraction + reveal). Spread onto `<WorkspaceToolbar>`. Populated
   *  only while a REMOTE op runs (commit-family runs feed `commitPhase`). */
  toolbarProps: GitToolbarProps;
  /** The commit-box phase readout (active commit/amend/mergeCommit run only). */
  commitPhase: string | null;
  runCount: number;
}

type GitRemoteCategory = 'push' | 'forcePush' | 'fetch' | 'pull';

function isRemoteCategory(category: GitActivityRun['category']): category is GitRemoteCategory {
  return (
    category === 'push' ||
    category === 'forcePush' ||
    category === 'fetch' ||
    category === 'pull'
  );
}

export function useGitDock(deps: { density: PanelDensity }): GitDockApi {
  const { density } = deps;
  // The store lives HERE (one subscription for the session), so the container
  // keeps a single hook call instead of two (file-size discipline).
  const { runs, activeRun, tick, clear, hasTerminalRuns } = useGitActivity();

  const panelRef = useRef<GitActivityDockHandle | null>(null);
  // Session-only geometry (Q5). Starts collapsed; never auto-expands.
  const [collapsed, setCollapsed] = useState(true);
  const [height, setHeight] = useState(GIT_DOCK_HEIGHT_DEFAULT);

  const focusDock = useCallback(() => {
    setCollapsed(false);
    panelRef.current?.focusLog();
  }, []);

  const toggleDock = useCallback(() => {
    setCollapsed((wasCollapsed) => {
      if (wasCollapsed) panelRef.current?.focusLog();
      return !wasCollapsed;
    });
  }, []);

  const paletteEntries = useMemo(
    () => gitPaletteEntries({ hasGitRuns: runs.length > 0, onGitActivity: focusDock }),
    [runs.length, focusDock],
  );

  // View C readouts. The toolbar readout is shown only for a remote op (its
  // transfer count while a fetch/pull streams progress, else the phase label);
  // commit-family runs feed the commit box instead (§2.2).
  const remoteRun = activeRun !== null && isRemoteCategory(activeRun.category) ? activeRun : null;
  const toolbarProps: GitToolbarProps = {
    gitCategory: remoteRun !== null ? remoteRun.category : null,
    gitPhase:
      remoteRun !== null
        ? (objectsReadout(remoteRun) ?? phaseLabel(remoteRun.category, remoteRun.phase))
        : null,
    gitProgress: remoteRun !== null ? progressFraction(remoteRun) : null,
    onShowGitActivity: focusDock,
  };

  const commitRun =
    activeRun !== null &&
    (activeRun.category === 'commit' ||
      activeRun.category === 'amend' ||
      activeRun.category === 'mergeCommit')
      ? activeRun
      : null;
  const commitPhase =
    commitRun !== null ? phaseLabel(commitRun.category, commitRun.phase) : null;

  return {
    panelProps: {
      ref: panelRef,
      runs,
      activeRun,
      tick,
      collapsed,
      onToggleCollapsed: setCollapsed,
      height,
      onResizeHeight: setHeight,
      onClear: clear,
      hasTerminalRuns,
      density,
    },
    paletteEntries,
    focusDock,
    toggleDock,
    activeRun,
    toolbarProps,
    commitPhase,
    runCount: runs.length,
  };
}
