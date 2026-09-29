// P49b — the per-tab "Open externally" context menu (App owns the target: the
// tab strip spans all tabs). Extracted verbatim from `App.tsx` by P125g, which
// sits at its size ratchet: the external-tool launchers are only used here, so
// the `useExternalTools` call moved with the menu. Never gated by any repo op;
// failures surface via the AppError→toast path.
import type { PushToast } from '../ToastContext';
import { useExternalTools } from '../hooks/useExternalTools';
import { ContextMenu } from './ContextMenu';
import { externalToolsItems } from './workspaceMenus';

export interface TabMenuTarget {
  path: string;
  x: number;
  y: number;
}

export function TabContextMenu({
  menu,
  onClose,
  pushToast,
}: {
  menu: TabMenuTarget | null;
  onClose(): void;
  pushToast: PushToast;
}) {
  const {
    handleOpenInTerminal: openInTerminal,
    handleRevealInFileManager: revealInFileManager,
    handleOpenInEditor: openInEditor,
  } = useExternalTools(pushToast);
  if (menu === null) return null;
  return (
    <ContextMenu
      x={menu.x}
      y={menu.y}
      items={externalToolsItems(menu.path, {
        onOpenInTerminal: openInTerminal,
        onRevealInFileManager: revealInFileManager,
        onOpenInEditor: openInEditor,
      })}
      onClose={onClose}
    />
  );
}
