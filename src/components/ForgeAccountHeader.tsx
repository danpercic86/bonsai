// P80 (UI §1.6): the connected-account header pinned atop the PR panel, shown
// only when `ctx.viewer !== null`. A thin wrapper that composes
// ForgeAccountSwitcher (the trigger + source caption + switcher/kebab menus).
// Presentational: it REQUESTS actions; PrPanel owns state, the account cache,
// and every IPC write. No IPC, no confirm here.
import type { AccountSource, ForgeAccount, ForgeKind, ForgeViewer } from '../ipc';
import { ForgeAccountSwitcher } from './ForgeAccountSwitcher';

export interface ForgeAccountHeaderProps {
  /** Non-null: the parent gates on `ctx.viewer`. */
  viewer: ForgeViewer;
  host: string;
  kind: ForgeKind;
  accountSource: AccountSource;
  resolvedAccountId: string | null;
  /** null ⇒ not yet loaded (lazy on first switcher open); filtered to `host`. */
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

export function ForgeAccountHeader({
  viewer,
  host,
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
}: ForgeAccountHeaderProps) {
  return (
    <ForgeAccountSwitcher
      host={host}
      activeLogin={viewer.login}
      activeAvatarUrl={viewer.avatarUrl}
      kind={kind}
      accountSource={accountSource}
      resolvedAccountId={resolvedAccountId}
      accounts={accounts}
      accountsError={accountsError}
      busy={busy}
      onOpenMenu={onOpenMenu}
      onSelectAccount={onSelectAccount}
      onUseHostDefault={onUseHostDefault}
      onAddAnother={onAddAnother}
      onChangeToken={onChangeToken}
      onResetToDefault={onResetToDefault}
      onManageAccounts={onManageAccounts}
    />
  );
}
