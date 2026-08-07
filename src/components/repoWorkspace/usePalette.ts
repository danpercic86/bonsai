import { useCallback, useEffect, useRef, useState } from 'react';

export interface UsePalette {
  open: boolean;
  /** For the workspace Esc-layering (read without a re-subscribe). */
  openRef: { current: boolean };
  close(): void;
  toggle(): void;
}

/** P50c: command-palette open/close state. The Ctrl/Cmd-K accelerator itself
 *  lives in useWorkspaceKeyboard (right next to Ctrl/Cmd-F) so every per-repo
 *  shortcut shares one gate + globalModalOpen suppression; this hook just owns
 *  the flag + an openRef for the Esc-layering, and force-closes when the tab is
 *  deactivated so the palette can never linger on a background (display:none)
 *  workspace after a Ctrl+Tab. */
export function usePalette(deps: { active: boolean }): UsePalette {
  const { active } = deps;
  const [open, setOpen] = useState(false);
  const openRef = useRef(false);
  openRef.current = open;

  const close = useCallback(() => setOpen(false), []);
  const toggle = useCallback(() => setOpen((o) => !o), []);

  useEffect(() => {
    if (!active) setOpen(false);
  }, [active]);

  return { open, openRef, close, toggle };
}
