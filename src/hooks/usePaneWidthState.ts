// §5.1 / P69b: the 3-pane split widths — live drag state plus the debounced
// persist. Extracted verbatim from App so the container only forwards
// `paneWidths` and the three resize handlers to RepoWorkspace.
import { useCallback, useRef, useState } from 'react';
import type { PaneWidths, UiSettingsPatch } from '../ipc';
import { clampLive, DEFAULT_PANE_WIDTHS } from '../appHelpers';

export interface UsePaneWidthState {
  paneWidths: PaneWidths;
  /** Launch hydration writes the stored widths through here so the
   *  authoritative ref and the render state move together. */
  applyPaneWidths: (next: PaneWidths) => void;
  handleSidebarResize: (delta: number) => void;
  handleRightPanelResize: (delta: number) => void;
  handlePaneResizeEnd: () => void;
}

export function usePaneWidthState(
  queueSettingsWrite: (patch: UiSettingsPatch) => void,
): UsePaneWidthState {
  const [paneWidths, setPaneWidths] = useState<PaneWidths>(DEFAULT_PANE_WIDTHS);
  const paneWidthsRef = useRef(paneWidths);

  // P69b: these three (plus `closeOnboarding`) each used to fire their own
  // `ipc.setUiSettings`, racing the hook's debounced merge — disjoint key sets
  // today, silent field loss the day they overlap. They now update App's state
  // for the live preview and hand the persist to the ONE coalescing window.
  const commitPaneWidths = useCallback(() => {
    queueSettingsWrite({ paneWidths: paneWidthsRef.current });
  }, [queueSettingsWrite]);

  // P69b: `paneWidthsRef` is authoritative AT CALL TIME, not from the next
  // render — PaneDivider's Arrow-key path calls onResize + onResizeEnd in one
  // handler, so `commitPaneWidths` would otherwise persist the pre-nudge width.
  const applyPaneWidths = useCallback((next: PaneWidths) => {
    paneWidthsRef.current = next;
    setPaneWidths(next);
  }, []);

  const handleSidebarResize = useCallback((delta: number) => {
    const w = paneWidthsRef.current;
    applyPaneWidths({ ...w, sidebar: clampLive(w.sidebar + delta, 'sidebar', w.rightPanel) });
  }, [applyPaneWidths]);

  const handleRightPanelResize = useCallback((delta: number) => {
    const w = paneWidthsRef.current;
    applyPaneWidths({ ...w, rightPanel: clampLive(w.rightPanel + delta, 'rightPanel', w.sidebar) });
  }, [applyPaneWidths]);

  const handlePaneResizeEnd = useCallback(() => {
    commitPaneWidths();
  }, [commitPaneWidths]);

  return {
    paneWidths,
    applyPaneWidths,
    handleSidebarResize,
    handleRightPanelResize,
    handlePaneResizeEnd,
  };
}
