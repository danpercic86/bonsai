import type { ComponentProps } from 'react';
import type { ForgeAccount, ForgeRepoContext } from '../../ipc';
import { ForgeAccountHeader } from '../ForgeAccountHeader';

// The PR panel's account header, extracted from PrPanel (file-size discipline —
// PrPanel is a container and its render body must stay thin). Owns exactly one
// decision: the header renders only once a viewer is resolved for the current
// forge context. Everything else is pass-through.

type HeaderProps = ComponentProps<typeof ForgeAccountHeader>;

export interface PrPanelAccountHeaderProps
  extends Pick<
    HeaderProps,
    | 'accountsError'
    | 'busy'
    | 'onOpenMenu'
    | 'onSelectAccount'
    | 'onUseHostDefault'
    | 'onAddAnother'
    | 'onChangeToken'
    | 'onResetToDefault'
    | 'onManageAccounts'
  > {
  /** null (or a viewer-less context) ⇒ nothing renders. */
  ctx: ForgeRepoContext | null;
  /** All known accounts; filtered to the context's host here. */
  accounts: ForgeAccount[] | null;
}

export function PrPanelAccountHeader({ ctx, accounts, ...rest }: PrPanelAccountHeaderProps) {
  if (ctx?.viewer == null) return null;
  return (
    <ForgeAccountHeader
      viewer={ctx.viewer}
      host={ctx.host}
      kind={ctx.provider}
      accountSource={ctx.accountSource}
      resolvedAccountId={ctx.resolvedAccountId}
      accounts={accounts === null ? null : accounts.filter((a) => a.host === ctx.host)}
      {...rest}
    />
  );
}
