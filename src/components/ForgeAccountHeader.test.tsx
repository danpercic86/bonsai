/** P79 §1 — the connected-account header pinned atop the PR panel. Presentational:
 *  it renders the login/host/badge and REQUESTS actions via callbacks; it owns no
 *  IPC and no confirm (PrPanel owns those — covered in PrPanel.reauth.test.tsx). */
import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';

import { ForgeAccountHeader } from './ForgeAccountHeader';
import type { ForgeViewer } from '../ipc';

const VIEWER: ForgeViewer = {
  login: 'octocat',
  avatarUrl: 'https://avatars.githubusercontent.com/u/583231?v=4',
};

function renderHeader() {
  const onChangeToken = vi.fn();
  const onDisconnect = vi.fn();
  render(
    <ForgeAccountHeader
      viewer={VIEWER}
      host="github.com"
      kind="gitHub"
      onChangeToken={onChangeToken}
      onDisconnect={onDisconnect}
    />,
  );
  return { onChangeToken, onDisconnect };
}

describe('ForgeAccountHeader', () => {
  it('renders the login and host', () => {
    renderHeader();
    expect(screen.getByText('octocat')).toBeInTheDocument();
    expect(screen.getByText('github.com')).toBeInTheDocument();
    // provider badge is present as an accessible label
    expect(screen.getByLabelText('GitHub')).toBeInTheDocument();
  });

  it('opens the kebab menu with Change token + Disconnect', () => {
    renderHeader();
    const kebab = screen.getByRole('button', { name: 'Account actions' });
    expect(kebab).toHaveAttribute('aria-expanded', 'false');
    fireEvent.click(kebab);
    expect(kebab).toHaveAttribute('aria-expanded', 'true');
    expect(screen.getByRole('menuitem', { name: 'Change token' })).toBeInTheDocument();
    expect(screen.getByRole('menuitem', { name: 'Disconnect' })).toBeInTheDocument();
  });

  it('requests change-token when the menu item is chosen', () => {
    const { onChangeToken, onDisconnect } = renderHeader();
    fireEvent.click(screen.getByRole('button', { name: 'Account actions' }));
    fireEvent.click(screen.getByRole('menuitem', { name: 'Change token' }));
    expect(onChangeToken).toHaveBeenCalledTimes(1);
    expect(onDisconnect).not.toHaveBeenCalled();
  });

  it('requests disconnect when the menu item is chosen (header opens no confirm itself)', () => {
    const { onChangeToken, onDisconnect } = renderHeader();
    fireEvent.click(screen.getByRole('button', { name: 'Account actions' }));
    fireEvent.click(screen.getByRole('menuitem', { name: 'Disconnect' }));
    expect(onDisconnect).toHaveBeenCalledTimes(1);
    expect(onChangeToken).not.toHaveBeenCalled();
    // No ConfirmDialog lives inside the header.
    expect(screen.queryByRole('dialog')).toBeNull();
  });
});
