/** Spec-007: assembles the graph pane's replay bundle (fab gate + overlay
 *  props) from workspace state. Extracted here — spec-005 railProps.ts
 *  precedent — because RepoWorkspace and WorkspaceGraphPane both sit at their
 *  size caps; only the minimal seam lives in the pane.
 *
 *  The overlay receives a SNAPSHOT of the layout captured at entry (advisor/
 *  plan: a watcher refresh can swap the live graph identity mid-replay; the
 *  replay model must stay consistent with the rows it was built from). The
 *  frozen working canvas underneath keeps tracking live data — exit shows
 *  fresh state for free. */
import { useCallback, useMemo, useRef, useState } from 'react';
import type { GraphLayout } from '../../ipc';
import type { ReplayModeProps } from '../../graph/replay/ReplayMode';

export interface UseReplayControllerDeps {
  graph: GraphLayout | null;
  metrics: ReplayModeProps['metrics'];
  metricsVersion: number;
  display: ReplayModeProps['display'];
  graphStyle: ReplayModeProps['graphStyle'];
  graphSeason: ReplayModeProps['graphSeason'];
  themeVersion: number;
  reducedMotion: boolean;
  pushToast(kind: 'info', message: string): void;
}

export interface ReplayPaneProps {
  open: boolean;
  /** Mirrors `open` for window keydown listeners (useWorkspaceKeyboard). */
  openRef: { readonly current: boolean };
  /** False on an empty/unborn layout — fab disabled, palette row disabled. */
  canReplay: boolean;
  onOpen(): void;
  onExit(): void;
  /** Non-null while open — the ReplayMode overlay's props. */
  mode: ReplayModeProps | null;
}

export function useReplayController(deps: UseReplayControllerDeps): ReplayPaneProps {
  const { graph, pushToast } = deps;
  // The entry snapshot doubles as the open flag.
  const [snapshot, setSnapshot] = useState<GraphLayout | null>(null);
  const open = snapshot !== null;
  const openRef = useRef(open);
  openRef.current = open;
  const canReplay = graph !== null && graph.nodes.length > 0;

  const graphRef = useRef(graph);
  graphRef.current = graph;
  const onOpen = useCallback(() => {
    const g = graphRef.current;
    if (g === null || g.nodes.length === 0) {
      // UI contract §3.4 — raced empty-repo invocation (palette/fab are
      // normally disabled before this point).
      pushToast('info', 'Nothing to replay — this repository has no commits yet.');
      return;
    }
    setSnapshot(g);
  }, [pushToast]);

  const onExit = useCallback(() => {
    setSnapshot(null);
    // §3.3 exit focus: the replay fab, else the graph canvas host (palette
    // entry with the fab absent). Deferred one frame — the fab re-renders
    // enabled/interactive after the overlay unmounts.
    requestAnimationFrame(() => {
      const fab = document.querySelector<HTMLElement>('.graph-replay-fab');
      // :not() — the replay scroller carries .graph-scroll too; never target it.
      (fab ?? document.querySelector<HTMLElement>('.graph-scroll:not(.graph-replay-scroll)'))?.focus();
    });
  }, []);

  const { metrics, metricsVersion, display, graphStyle, graphSeason, themeVersion, reducedMotion } =
    deps;
  const mode = useMemo<ReplayModeProps | null>(
    () =>
      snapshot === null
        ? null
        : {
            layout: snapshot,
            metrics,
            metricsVersion,
            display,
            graphStyle,
            graphSeason,
            themeVersion,
            reducedMotion,
            onExit,
          },
    [snapshot, metrics, metricsVersion, display, graphStyle, graphSeason, themeVersion, reducedMotion, onExit],
  );

  return useMemo(
    () => ({ open, openRef, canReplay, onOpen, onExit, mode }),
    [open, canReplay, onOpen, onExit, mode],
  );
}
