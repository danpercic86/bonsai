/** P80 §1 — the PR-panel account switcher. Presentational: it renders the
 *  trigger (switcher when ≥2 accounts, static otherwise), the source caption,
 *  and the switcher/kebab menus; it REQUESTS actions via callbacks and owns no
 *  IPC. */
import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';

import { ForgeAccountSwitcher, type ForgeAccountSwitcherProps } from './ForgeAccountSwitcher';
import type { ForgeAccount } from '../ipc';

const OCTO: ForgeAccount = {
  accountId: 'gitHub:github.com:octocat',
  host: 'github.com',
  kind: 'gitHub',
  login: 'octocat',
  avatarUrl: null,
  connected: true,
  isHostDefault: true,
};
const ALT: ForgeAccount = {
  accountId: 'gitHub:github.com:danpercic86',
  host: 'github.com',
  kind: 'gitHub',
  login: 'danpercic86',
  avatarUrl: null,
  connected: true,
  isHostDefault: false,
};

function renderSwitcher(overrides: Partial<ForgeAccountSwitcherProps> = {}) {
  const props: ForgeAccountSwitcherProps = {
    host: 'github.com',
    activeLogin: 'octocat',
    activeAvatarUrl: null,
    kind: 'gitHub',
    accountSource: 'hostDefault',
    resolvedAccountId: OCTO.accountId,
    accounts: [OCTO, ALT],
    accountsError: null,
    busy: false,
    onOpenMenu: vi.fn(),
    onSelectAccount: vi.fn(),
    onUseHostDefault: vi.fn(),
    onAddAnother: vi.fn(),
    onChangeToken: vi.fn(),
    onResetToDefault: vi.fn(),
    onManageAccounts: vi.fn(),
    ...overrides,
  };
  render(<ForgeAccountSwitcher {...props} />);
  return props;
}

describe('ForgeAccountSwitcher — P80', () => {
  it('single-account host renders a static header (no switcher trigger, no caption)', () => {
    renderSwitcher({ accountSource: 'single', accounts: [OCTO] });
    expect(screen.queryByRole('button', { name: /Switch account/ })).toBeNull();
    expect(screen.getByText('octocat')).toBeInTheDocument();
    // No source caption for `single`.
    expect(screen.queryByText('Host default')).toBeNull();
  });

  it('multi-account host renders a switcher trigger with the source caption', () => {
    renderSwitcher({ accountSource: 'ownerMatch' });
    expect(
      screen.getByRole('button', { name: 'Switch account (currently octocat)' }),
    ).toBeInTheDocument();
    expect(screen.getByText('Matched by owner')).toBeInTheDocument();
  });

  it('opening the switcher fires onOpenMenu and lists every account + the two actions', () => {
    const props = renderSwitcher({ accountSource: 'hostDefault' });
    fireEvent.click(screen.getByRole('button', { name: /Switch account/ }));
    expect(props.onOpenMenu).toHaveBeenCalledTimes(1);
    // One radio row per account, the active one checked.
    const octo = screen.getByRole('menuitemradio', { name: /octocat/ });
    expect(octo).toHaveAttribute('aria-checked', 'true');
    expect(screen.getByRole('menuitemradio', { name: /danpercic86/ })).toHaveAttribute(
      'aria-checked',
      'false',
    );
    expect(screen.getByRole('menuitem', { name: 'Use host default' })).toBeInTheDocument();
    expect(screen.getByRole('menuitem', { name: 'Add another account…' })).toBeInTheDocument();
  });

  it('selecting an account requests a pin; "Use host default" is disabled unless overridden', () => {
    const props = renderSwitcher({ accountSource: 'hostDefault' });
    fireEvent.click(screen.getByRole('button', { name: /Switch account/ }));
    // Not an override ⇒ Use host default disabled.
    expect(screen.getByRole('menuitem', { name: 'Use host default' })).toBeDisabled();
    fireEvent.click(screen.getByRole('menuitemradio', { name: /danpercic86/ }));
    expect(props.onSelectAccount).toHaveBeenCalledWith(ALT.accountId);
  });

  it('override state enables "Use host default" and the kebab "Reset to host default"', () => {
    const props = renderSwitcher({ accountSource: 'override', resolvedAccountId: ALT.accountId });
    // caption reflects the override.
    expect(screen.getByText('Pinned to this repo')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: /Switch account/ }));
    const useDefault = screen.getByRole('menuitem', { name: 'Use host default' });
    expect(useDefault).not.toBeDisabled();
    fireEvent.click(useDefault);
    expect(props.onUseHostDefault).toHaveBeenCalledTimes(1);
  });

  it('kebab shows Reset to host default only under an override; Change token + Manage always', () => {
    // no override → no Reset.
    const { rerender } = render(
      <ForgeAccountSwitcher
        host="github.com"
        activeLogin="octocat"
        activeAvatarUrl={null}
        kind="gitHub"
        accountSource="hostDefault"
        resolvedAccountId={OCTO.accountId}
        accounts={[OCTO, ALT]}
        accountsError={null}
        busy={false}
        onOpenMenu={vi.fn()}
        onSelectAccount={vi.fn()}
        onUseHostDefault={vi.fn()}
        onAddAnother={vi.fn()}
        onChangeToken={vi.fn()}
        onResetToDefault={vi.fn()}
        onManageAccounts={vi.fn()}
      />,
    );
    fireEvent.click(screen.getByRole('button', { name: 'Account actions' }));
    expect(screen.getByRole('menuitem', { name: 'Change token' })).toBeInTheDocument();
    expect(screen.getByRole('menuitem', { name: 'Manage accounts…' })).toBeInTheDocument();
    expect(screen.queryByRole('menuitem', { name: 'Reset to host default' })).toBeNull();
    rerender(
      <ForgeAccountSwitcher
        host="github.com"
        activeLogin="danpercic86"
        activeAvatarUrl={null}
        kind="gitHub"
        accountSource="override"
        resolvedAccountId={ALT.accountId}
        accounts={[OCTO, ALT]}
        accountsError={null}
        busy={false}
        onOpenMenu={vi.fn()}
        onSelectAccount={vi.fn()}
        onUseHostDefault={vi.fn()}
        onAddAnother={vi.fn()}
        onChangeToken={vi.fn()}
        onResetToDefault={vi.fn()}
        onManageAccounts={vi.fn()}
      />,
    );
    fireEvent.click(screen.getByRole('button', { name: 'Account actions' }));
    expect(screen.getByRole('menuitem', { name: 'Reset to host default' })).toBeInTheDocument();
  });

  it('shows the no-default nudge in the switcher menu header when ≥2 accounts and none default', () => {
    renderSwitcher({
      accountSource: 'hostDefault',
      accounts: [
        { ...OCTO, isHostDefault: false },
        { ...ALT, isHostDefault: false },
      ],
    });
    fireEvent.click(screen.getByRole('button', { name: /Switch account/ }));
    expect(
      screen.getByText('No default account set — pick one below or in Settings.'),
    ).toBeInTheDocument();
  });

  it('renders a loading header while accounts are null and an error line on failure', () => {
    const { rerender } = render(
      <ForgeAccountSwitcher
        host="github.com"
        activeLogin="octocat"
        activeAvatarUrl={null}
        kind="gitHub"
        accountSource="hostDefault"
        resolvedAccountId={OCTO.accountId}
        accounts={null}
        accountsError={null}
        busy={false}
        onOpenMenu={vi.fn()}
        onSelectAccount={vi.fn()}
        onUseHostDefault={vi.fn()}
        onAddAnother={vi.fn()}
        onChangeToken={vi.fn()}
        onResetToDefault={vi.fn()}
        onManageAccounts={vi.fn()}
      />,
    );
    fireEvent.click(screen.getByRole('button', { name: /Switch account/ }));
    expect(screen.getByText('Loading accounts…')).toBeInTheDocument();
    rerender(
      <ForgeAccountSwitcher
        host="github.com"
        activeLogin="octocat"
        activeAvatarUrl={null}
        kind="gitHub"
        accountSource="hostDefault"
        resolvedAccountId={OCTO.accountId}
        accounts={null}
        accountsError="network down"
        busy={false}
        onOpenMenu={vi.fn()}
        onSelectAccount={vi.fn()}
        onUseHostDefault={vi.fn()}
        onAddAnother={vi.fn()}
        onChangeToken={vi.fn()}
        onResetToDefault={vi.fn()}
        onManageAccounts={vi.fn()}
      />,
    );
    fireEvent.click(screen.getByRole('button', { name: /Switch account/ }));
    expect(screen.getByText("Couldn't load accounts. network down")).toBeInTheDocument();
  });

  it('add-another requests the connect add flow', () => {
    const props = renderSwitcher();
    fireEvent.click(screen.getByRole('button', { name: /Switch account/ }));
    fireEvent.click(screen.getByRole('menuitem', { name: 'Add another account…' }));
    expect(props.onAddAnother).toHaveBeenCalledTimes(1);
  });
});
