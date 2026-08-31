// P80 (UI §1): the PR-panel account header — a switcher trigger (when the host
// has ≥2 accounts) or the static P79 row (0–1 accounts), the muted source
// caption, and the two menus (account switcher + kebab token/override actions).
// Presentational: it REQUESTS actions via callbacks; PrPanel owns state, the
// account cache, and every IPC write. No IPC, no confirm here.
import { useRef, useState } from 'react';

import type { AccountSource, ForgeAccount, ForgeKind } from '../ipc';
import { MoreIcon } from './appIcons';
import { ContextMenu, type ContextMenuItem } from './ContextMenu';
import { accountSourceCaption, accountSourceTooltip } from './forgeAccountSource';
import { ForgeAvatar } from './ForgeAvatar';
import { ForgeProviderBadge } from './ForgeProviderBadge';

export interface ForgeAccountSwitcherProps {
  host: string;
  activeLogin: string | null;
  activeAvatarUrl: string | null;
  kind: ForgeKind;
  accountSource: AccountSource;
  resolvedAccountId: string | null;
  /** null ⇒ not yet loaded; already filtered to `host` by the parent. */
  accounts: ForgeAccount[] | null;
  accountsError: string | null;
  busy: boolean;
  onOpenMenu(): void;
  onSelectAccount(accountId: string): void;
  onUseHostDefault(): void;
  onAddAnother(): void;
  onChangeToken(): void;
  onResetToDefault(): void;
  onManageAccounts(): void;
}

type OpenMenu = 'switcher' | 'kebab';

export function ForgeAccountSwitcher({
  host,
  activeLogin,
  activeAvatarUrl,
  kind,
  accountSource,
  resolvedAccountId,
  accounts,
  accountsError,
  busy,
  onOpenMenu,
  onSelectAccount,
  onUseHostDefault,
  onAddAnother,
  onChangeToken,
  onResetToDefault,
  onManageAccounts,
}: ForgeAccountSwitcherProps) {
  const [menu, setMenu] = useState<{ which: OpenMenu; x: number; y: number } | null>(null);
  const triggerRef = useRef<HTMLButtonElement>(null);
  const kebabRef = useRef<HTMLButtonElement>(null);

  // The header trigger is a switcher only once we know the host has ≥2 accounts.
  // Until the (lazy) list loads we can't be sure, so infer from `accountSource`:
  // `single`/`none` are never switchable; anything else means multi is possible.
  const maybeMulti = accountSource !== 'single' && accountSource !== 'none';
  // Once the (lazy) list has loaded, a host with <2 accounts is never switchable
  // — fall back to the static row (covers the single-account owner-match case).
  const isSwitcher = maybeMulti && (accounts === null || accounts.length >= 2);

  const caption = accountSourceCaption(accountSource);
  const tooltip = accountSourceTooltip(accountSource);
  const login = activeLogin ?? host;

  const openSwitcher = () => {
    const r = triggerRef.current?.getBoundingClientRect();
    onOpenMenu(); // triggers the lazy fetch in the parent
    setMenu(r === undefined ? { which: 'switcher', x: 0, y: 0 } : { which: 'switcher', x: r.right, y: r.bottom + 2 });
  };

  const openKebab = () => {
    const r = kebabRef.current?.getBoundingClientRect();
    setMenu(r === undefined ? { which: 'kebab', x: 0, y: 0 } : { which: 'kebab', x: r.right, y: r.bottom + 2 });
  };

  const closeMenu = () => setMenu(null);

  // ----- switcher menu items (accounts + host-default + add) -----
  function switcherItems(): ContextMenuItem[] {
    if (accountsError !== null) return [];
    if (accounts === null) return [];
    const rows: ContextMenuItem[] = accounts.map((a) => ({
      label: a.login ?? a.host,
      checked: a.accountId === resolvedAccountId,
      detail: a.isHostDefault ? 'Host default' : undefined,
      onSelect: () => onSelectAccount(a.accountId),
    }));
    return [
      ...rows,
      {
        label: 'Use host default',
        disabled: accountSource !== 'override',
        onSelect: onUseHostDefault,
      },
      { label: 'Add another account…', onSelect: onAddAnother },
    ];
  }

  const noDefault =
    accounts !== null && accounts.length >= 2 && !accounts.some((a) => a.isHostDefault);

  const switcherHeader =
    accountsError !== null ? (
      <span className="context-menu-note">{`Couldn't load accounts. ${accountsError}`}</span>
    ) : accounts === null ? (
      <span className="context-menu-note">Loading accounts…</span>
    ) : (
      <>
        <span className="context-menu-note">{`Accounts on ${host}`}</span>
        {noDefault && (
          <span className="context-menu-subnote">
            No default account set — pick one below or in Settings.
          </span>
        )}
      </>
    );

  // ----- kebab menu items (token / override commands) -----
  const kebabItems: ContextMenuItem[] = [
    { label: 'Change token', onSelect: onChangeToken },
    ...(accountSource === 'override'
      ? [{ label: 'Reset to host default', onSelect: onResetToDefault }]
      : []),
    { label: 'Manage accounts…', onSelect: onManageAccounts },
  ];

  return (
    <div className="forge-account-header">
      <ForgeAvatar avatarUrl={activeAvatarUrl} login={activeLogin} />
      {isSwitcher ? (
        <button
          ref={triggerRef}
          type="button"
          className="forge-account-trigger"
          aria-haspopup="menu"
          aria-expanded={menu?.which === 'switcher'}
          aria-disabled={busy}
          aria-label={`Switch account (currently ${login})`}
          onClick={() => {
            if (busy) return;
            openSwitcher();
          }}
        >
          <span className="forge-account-login" title={activeLogin ?? host}>
            {login}
          </span>
          <span className="forge-account-host" title={host}>
            {host}
          </span>
          <span className="forge-account-caret" aria-hidden="true">
            ▾
          </span>
        </button>
      ) : (
        <>
          <span className="forge-account-login" title={activeLogin ?? host}>
            {login}
          </span>
          <span className="forge-account-host" title={host}>
            {host}
          </span>
        </>
      )}
      {caption !== null && (
        <span className="forge-account-source" title={tooltip ?? undefined}>
          {caption}
        </span>
      )}
      <span className="forge-account-spacer" />
      <ForgeProviderBadge kind={kind} />
      <button
        ref={kebabRef}
        type="button"
        className="btn-icon forge-account-kebab"
        aria-label="Account actions"
        aria-haspopup="menu"
        aria-expanded={menu?.which === 'kebab'}
        onClick={openKebab}
      >
        <MoreIcon />
      </button>
      {menu?.which === 'switcher' && (
        <ContextMenu
          x={menu.x}
          y={menu.y}
          items={switcherItems()}
          header={switcherHeader}
          busy={busy}
          onClose={closeMenu}
        />
      )}
      {menu?.which === 'kebab' && (
        <ContextMenu x={menu.x} y={menu.y} items={kebabItems} onClose={closeMenu} />
      )}
    </div>
  );
}
