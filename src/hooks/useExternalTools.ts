// P49b (extracted for size, spec-003 ratchet): launch external tools at a
// filesystem path (repo / worktree / submodule). Never gated by
// mutating/opActive — launches touch no git state. Failures surface via the
// shared AppError→toast path; success is silent (the opened window is its own
// feedback). Shared by App (the tab strip's context menu) and RepoWorkspace;
// no behavior change.
import { useCallback } from 'react';

import { ipc } from '../ipc';
import type { PushToast } from '../ToastContext';
import { errorMessage } from '../utils/errors';

export interface ExternalTools {
  handleOpenInTerminal(path: string): void;
  handleRevealInFileManager(path: string): void;
  handleOpenInEditor(path: string): void;
}

export function useExternalTools(pushToast: PushToast): ExternalTools {
  const handleOpenInTerminal = useCallback(
    (path: string) => {
      void ipc.openInTerminal(path).catch((e) => pushToast('error', errorMessage(e)));
    },
    [pushToast],
  );
  const handleRevealInFileManager = useCallback(
    (path: string) => {
      void ipc.revealInFileManager(path).catch((e) => pushToast('error', errorMessage(e)));
    },
    [pushToast],
  );
  const handleOpenInEditor = useCallback(
    (path: string) => {
      void ipc.openInEditor(path).catch((e) => pushToast('error', errorMessage(e)));
    },
    [pushToast],
  );
  return { handleOpenInTerminal, handleRevealInFileManager, handleOpenInEditor };
}
