/**
 * P69i §5.2 — the extracted `.header-toolbar`.
 *
 * Two things are worth pinning: the repo-scoped gating (three controls appear
 * only with a repo open, and the identity trigger is one of them — `getConfig`
 * needs a repo id, so with none Bonsai cannot read even the global identity),
 * and that the identity menu's open state reaches App, which is what keeps
 * global shortcuts suppressed while it is open.
 */
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';

import { HeaderToolbar } from './HeaderToolbar';
import { mockIpc } from '../ipc/mock';
import { resetEffectiveIdentityForTests } from '../hooks/useEffectiveIdentity';
import type { ConfigView, IdentityProfile } from '../ipc';

const EMPTY_VIEW: ConfigView = { targetLevel: 'local', curated: [], advanced: [] };

const PROFILES: IdentityProfile[] = [
  { id: 'p-1', label: 'Work', userName: 'Ada Lovelace', userEmail: 'work@bonsai.dev', signingKey: null },
];

function renderToolbar(activeRepo: string | null) {
  const props = {
    theme: 'dark' as const,
    onToggleTheme: vi.fn(),
    listView: 'tree' as const,
    onToggleListView: vi.fn(),
    activeRepo,
    onOpenAiAssets: vi.fn(),
    onOpenHealth: vi.fn(),
    onOpenSettings: vi.fn(),
    onOpenSettingsAt: vi.fn(),
    onMenuOpenChange: vi.fn(),
    profiles: PROFILES,
    onProfilesChange: vi.fn(),
  };
  render(<HeaderToolbar {...props} />);
  return props;
}

beforeEach(() => {
  resetEffectiveIdentityForTests();
  vi.spyOn(mockIpc, 'getConfig').mockResolvedValue(EMPTY_VIEW);
});
afterEach(() => {
  vi.restoreAllMocks();
  resetEffectiveIdentityForTests();
});

describe('HeaderToolbar', () => {
  it('with a repo open, renders all six controls and wires each one', async () => {
    const props = renderToolbar('/repo/open');

    fireEvent.click(screen.getByRole('button', { name: 'Switch to light theme' }));
    expect(props.onToggleTheme).toHaveBeenCalledTimes(1);
    fireEvent.click(screen.getByRole('button', { name: 'Switch to flat lists' }));
    expect(props.onToggleListView).toHaveBeenCalledTimes(1);
    fireEvent.click(screen.getByRole('button', { name: 'AI Assets' }));
    expect(props.onOpenAiAssets).toHaveBeenCalledTimes(1);
    fireEvent.click(screen.getByRole('button', { name: 'Health' }));
    expect(props.onOpenHealth).toHaveBeenCalledTimes(1);
    fireEvent.click(screen.getByRole('button', { name: 'Settings' }));
    expect(props.onOpenSettings).toHaveBeenCalledTimes(1);

    await waitFor(() =>
      expect(screen.getByRole('button', { name: 'Commit identity not set' })).toBeInTheDocument(),
    );
  });

  it('with no repo, the three repo-scoped controls (identity included) are absent', () => {
    renderToolbar(null);

    expect(screen.getByRole('button', { name: 'Settings' })).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'AI Assets' })).toBeNull();
    expect(screen.queryByRole('button', { name: 'Health' })).toBeNull();
    expect(document.querySelector('.identity-trigger')).toBeNull();
    // UI §4.1: with no repo there is nothing to read, so no call is made either.
    expect(mockIpc.getConfig).not.toHaveBeenCalled();
  });

  it('the identity trigger is last — the conventional account slot', async () => {
    renderToolbar('/repo/order');
    await waitFor(() => expect(document.querySelector('.identity-trigger')).not.toBeNull());
    const toolbar = document.querySelector('.header-toolbar');
    expect(toolbar?.lastElementChild).toHaveClass('identity-trigger');
  });

  it('lifts the identity menu open state up to App', async () => {
    const props = renderToolbar('/repo/lift');
    await waitFor(() => expect(document.querySelector('.identity-trigger')).not.toBeNull());
    props.onMenuOpenChange.mockClear();

    fireEvent.click(document.querySelector('.identity-trigger') as HTMLElement);
    await screen.findByRole('menu');
    expect(props.onMenuOpenChange).toHaveBeenLastCalledWith(true);
  });
});
