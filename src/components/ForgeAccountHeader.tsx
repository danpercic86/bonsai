// P79 (UI §1): the connected-account header pinned atop the PR panel, shown only
// when `ctx.viewer !== null`. Presentational: it REQUESTS actions (change token /
// disconnect); PrPanel owns the ConfirmDialog and connectMode. No IPC here.
import { useRef, useState } from 'react';

import type { ForgeKind, ForgeViewer } from '../ipc';
import { ContextMenu, type ContextMenuItem } from './ContextMenu';
import { ForgeAvatar } from './ForgeAvatar';
import { ForgeProviderBadge } from './ForgeProviderBadge';

export interface ForgeAccountHeaderProps {
  /** Non-null: the parent gates on `ctx.viewer`. */
  viewer: ForgeViewer;
  host: string;
  kind: ForgeKind;
  onChangeToken(): void;
  /** The header only REQUESTS disconnect; the parent opens the confirm. */
  onDisconnect(): void;
}

export function ForgeAccountHeader({
  viewer,
  host,
  kind,
  onChangeToken,
  onDisconnect,
}: ForgeAccountHeaderProps) {
  const [menu, setMenu] = useState<{ x: number; y: number } | null>(null);
  const kebabRef = useRef<HTMLButtonElement>(null);

  const openMenu = () => {
    const r = kebabRef.current?.getBoundingClientRect();
    // Anchor the menu at the kebab's bottom-right; ContextMenu clamps into view.
    setMenu(r === undefined ? { x: 0, y: 0 } : { x: r.right, y: r.bottom });
  };

  const items: ContextMenuItem[] = [
    { label: 'Change token', onSelect: onChangeToken },
    { label: 'Disconnect', tone: 'danger', onSelect: onDisconnect },
  ];

  return (
    <div className="forge-account-header">
      <ForgeAvatar avatarUrl={viewer.avatarUrl} login={viewer.login} />
      <span className="forge-account-login" title={viewer.login}>
        {viewer.login}
      </span>
      <span className="forge-account-host" title={host}>
        {host}
      </span>
      <ForgeProviderBadge kind={kind} />
      <button
        ref={kebabRef}
        type="button"
        className="btn-icon forge-account-kebab"
        aria-label="Account actions"
        aria-haspopup="menu"
        aria-expanded={menu !== null}
        onClick={openMenu}
      >
        {'⋯'}
      </button>
      {menu !== null && (
        <ContextMenu x={menu.x} y={menu.y} items={items} onClose={() => setMenu(null)} />
      )}
    </div>
  );
}
