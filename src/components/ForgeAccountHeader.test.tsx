/** P80 §1.6 — ForgeAccountHeader is a thin wrapper composing ForgeAccountSwitcher
 *  from the viewer + resolved-account props. Detailed switcher behaviour lives in
 *  ForgeAccountSwitcher.test.tsx; here we assert the wiring (viewer → switcher). */
import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';

import { ForgeAccountHeader, type ForgeAccountHeaderProps } from './ForgeAccountHeader';
import type { ForgeAccount, ForgeViewer } from '../ipc';

const VIEWER: ForgeViewer = {
  login: 'octocat',
  avatarUrl: 'https://avatars.githubusercontent.com/u/583231?v=4',
};
const ACCT: ForgeAccount = {
  accountId: 'gitHub:github.com:octocat',
  host: 'github.com',
  kind: 'gitHub',
  login: 'octocat',
  avatarUrl: null,
  connected: true,
  isHostDefault: true,
};

function renderHeader(overrides: Partial<ForgeAccountHeaderProps> = {}) {
  const props: ForgeAccountHeaderProps = {
    viewer: VIEWER,
    host: 'github.com',
    kind: 'gitHub',
    accountSource: 'single',
    resolvedAccountId: ACCT.accountId,
    accounts: [ACCT],
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
  render(<ForgeAccountHeader {...props} />);
  return props;
}

describe('ForgeAccountHeader — P80', () => {
  it('renders the login, host and provider badge from the viewer', () => {
    renderHeader();
    expect(screen.getByText('octocat')).toBeInTheDocument();
    expect(screen.getByText('github.com')).toBeInTheDocument();
    expect(screen.getByLabelText('GitHub')).toBeInTheDocument();
  });

  it('single account ⇒ static header (no switcher); the kebab requests change-token', () => {
    const props = renderHeader({ accountSource: 'single' });
    expect(screen.queryByRole('button', { name: /Switch account/ })).toBeNull();
    fireEvent.click(screen.getByRole('button', { name: 'Account actions' }));
    fireEvent.click(screen.getByRole('menuitem', { name: 'Change token' }));
    expect(props.onChangeToken).toHaveBeenCalledTimes(1);
  });

  it('multi account ⇒ switcher trigger present', () => {
    renderHeader({
      accountSource: 'ownerMatch',
      accounts: [ACCT, { ...ACCT, accountId: 'gitHub:github.com:alt', login: 'alt', isHostDefault: false }],
    });
    expect(screen.getByRole('button', { name: /Switch account/ })).toBeInTheDocument();
  });
});
