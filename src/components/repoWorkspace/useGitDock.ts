/**
 * P87b — everything the git activity dock (View D) + the toolbar busy participle
 * (View C) need from the container, assembled here (mirrors `useAiDock`).
 *
 * Geometry is SESSION-ONLY (P87-ui §3.1 / Q5, orchestrator-recommended): the
 * collapsed flag + height live in this hook's state, NOT in `UiSettings`, so the
 * settings contract is untouched. The dock NEVER auto-expands (§3.1).
 */
import { useCallback, useMemo, useRef, useState } from 'react';

import { useGitActivity } from './useGitActivity';
import { gitPaletteEntries, type PaletteAction } from '../paletteActions';
import { GIT_DOCK_HEIGHT_DEFAULT } from '../gitActivityFormat';
import type { GitActivityDockHandle, GitActivityDockProps } from '../GitActivityDock';
import type { GitActivityCategory, PanelDensity } from '../../ipc';
import type { GitActivityRun } from './useGitActivity';

/** The View C toolbar props bundle (spread onto `<WorkspaceToolbar>`). */
export interface GitToolbarProps {
  gitCategory: GitActivityCategory | null;
}

export interface GitDockApi {
  /** Ready to spread onto `<GitActivityDock {...panelProps} />`, ref included. */
  panelProps: GitActivityDockProps & { ref: React.Ref<GitActivityDockHandle> };
  /** The `Git activity` palette row (gated on there being a run). */
  paletteEntries: PaletteAction[];
  /** Palette / `Ctrl-Shift-L`: expand + focus. */
  focusDock: () => void;
  /** `Ctrl/Cmd+Shift+L`: toggle collapsed (expand+focus, or collapse). */
  toggleDock: () => void;
  /** The newest running run (View C readouts). */
  activeRun: GitActivityRun | null;
  /** View C toolbar props (busy-participle category). Spread onto
   *  `<WorkspaceToolbar>`. Populated only while a REMOTE op runs. */
  toolbarProps: GitToolbarProps;
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

export function useGitDock(deps: { density: PanelDensity; repoId: string }): GitDockApi {
  const { density, repoId } = deps;
  // The store lives HERE (one per-repo subscription per mount, P123), so the
  // container keeps a single hook call instead of two (file-size discipline).
  const { runs, activeRun, tick, clear, hasTerminalRuns } = useGitActivity(repoId);

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

  // The toolbar only needs the active remote op's category (busy participle);
  // phase/transfer detail is shown in the dock alone.
  const remoteRun = activeRun !== null && isRemoteCategory(activeRun.category) ? activeRun : null;
  const toolbarProps: GitToolbarProps = {
    gitCategory: remoteRun !== null ? remoteRun.category : null,
  };

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
    runCount: runs.length,
  };
}
