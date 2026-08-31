/**
 * P69i — the header identity control: the four trigger states and the menu's
 * structure per state (UI §4.2/§4.3). Applying — the half that writes to
 * `.git/config` — lives in `IdentityMenu.apply.test.tsx`; fixtures are shared
 * through `src/test/identityKit.tsx`.
 */
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { fireEvent, screen, waitFor } from '@testing-library/react';

import { identityInitials } from './identityCopy';
import { mockIpc } from '../ipc/mock';
import { resetEffectiveIdentityForTests } from '../hooks/useEffectiveIdentity';
import {
  EMPTY_VIEW,
  identityView,
  openMenu,
  renderMenu,
  trigger,
} from '../test/identityKit';
import type { ConfigView, IdentityProfile } from '../ipc';

beforeEach(() => {
  resetEffectiveIdentityForTests();
});
afterEach(() => {
  vi.restoreAllMocks();
  resetEffectiveIdentityForTests();
});

describe('identityInitials', () => {
  it('takes the first letters of the first two words, uppercased', () => {
    expect(identityInitials('Ada Lovelace')).toBe('AL');
    expect(identityInitials('ada')).toBe('A');
    expect(identityInitials('  mary  jane  watson ')).toBe('MJ');
    expect(identityInitials('')).toBe('');
    expect(identityInitials(null)).toBe('');
  });
});

describe('IdentityMenu — the four trigger states (UI §4.2)', () => {
  it('state 1: a LOCAL identity matching a saved profile names the profile', async () => {
    vi.spyOn(mockIpc, 'getConfig').mockResolvedValue(
      identityView('Ada Lovelace', 'work@bonsai.dev', 'local'),
    );
    renderMenu('/repo/match');

    await waitFor(() =>
      expect(trigger()).toHaveAttribute('aria-label', 'Commit identity: Work'),
    );
    expect(trigger().title).toBe(
      'Work\nAda Lovelace <work@bonsai.dev>\nFrom this repository’s config',
    );
    expect(document.querySelector('.identity-avatar')?.textContent).toBe('AL');
  });

  it('state 2: a GLOBAL identity with no matching profile names the person', async () => {
    // The honest default harness state: global `Mock Fixture User`, seeded
    // profiles that deliberately do not match it.
    vi.spyOn(mockIpc, 'getConfig').mockResolvedValue(
      identityView('Mock Fixture User', 'fixture@bonsai.dev', 'global'),
    );
    renderMenu('/repo/nomatch');

    await waitFor(() =>
      expect(trigger()).toHaveAttribute('aria-label', 'Commit identity: Mock Fixture User'),
    );
    expect(trigger().title).toBe(
      'Mock Fixture User <fixture@bonsai.dev>\nFrom your global Git config',
    );
    expect(document.querySelector('.identity-avatar')?.textContent).toBe('MF');
  });

  it('state 3: no identity anywhere shows the ? glyph and the warning wording', async () => {
    vi.spyOn(mockIpc, 'getConfig').mockResolvedValue(EMPTY_VIEW);
    renderMenu('/repo/unset');

    await waitFor(() => expect(trigger()).toHaveAttribute('aria-label', 'Commit identity not set'));
    expect(trigger().title).toBe(
      'No name and email are set. Commits will fail until you set one.',
    );
    const avatar = document.querySelector('.identity-avatar');
    expect(avatar?.textContent).toBe('?');
    expect(avatar).toHaveAttribute('data-identity-state', 'unset');
  });

  it('an unreadable config renders as state 3 but says why', async () => {
    vi.spyOn(mockIpc, 'getConfig').mockRejectedValue(new Error('boom'));
    renderMenu('/repo/broken');

    await waitFor(() =>
      expect(trigger().title).toBe('Bonsai couldn’t read this repository’s Git config.'),
    );
    expect(document.querySelector('.identity-avatar')).toHaveAttribute(
      'data-identity-state',
      'unreadable',
    );

    await openMenu();
    expect(screen.getByText('Bonsai couldn’t read this repository’s Git config.')).toBeInTheDocument();
    expect(screen.getByRole('menuitem', { name: 'Set an identity…' })).toBeInTheDocument();
  });

  it('loading shows the middle dot and announces itself as busy', async () => {
    let resolve: (v: ConfigView) => void = () => {};
    vi.spyOn(mockIpc, 'getConfig').mockReturnValue(
      new Promise<ConfigView>((r) => {
        resolve = r;
      }),
    );
    renderMenu('/repo/slow');

    expect(trigger()).toHaveAttribute('aria-label', 'Reading commit identity…');
    expect(trigger()).toHaveAttribute('aria-busy', 'true');
    expect(document.querySelector('.identity-avatar')?.textContent).toBe('·');
    resolve(EMPTY_VIEW);
    await waitFor(() => expect(trigger()).toHaveAttribute('aria-label', 'Commit identity not set'));
  });
});

describe('IdentityMenu — the menu (UI §4.3)', () => {
  it('names the effective identity and its source, and checks the matching row', async () => {
    vi.spyOn(mockIpc, 'getConfig').mockResolvedValue(
      identityView('Ada Lovelace', 'work@bonsai.dev', 'local'),
    );
    renderMenu('/repo/menu');
    await waitFor(() => expect(trigger()).toHaveAttribute('aria-label', 'Commit identity: Work'));
    await openMenu();

    expect(screen.getByText('Committing as')).toBeInTheDocument();
    expect(screen.getByText('From this repository’s config')).toBeInTheDocument();
    const rows = screen.getAllByRole('menuitemradio');
    expect(rows).toHaveLength(2);
    expect(rows[0]).toHaveAttribute('aria-checked', 'true');
    expect(rows[1]).toHaveAttribute('aria-checked', 'false');
    // Each profile row carries its identity on a second line.
    expect(rows[0].textContent).toContain('Ada Lovelace · work@bonsai.dev');
    // Item 2/3 must NOT appear when a profile matches; the tail row always does.
    expect(screen.queryByRole('menuitem', { name: /Save “/ })).toBeNull();
    expect(screen.getByRole('menuitem', { name: 'Manage identities…' })).toBeInTheDocument();
  });

  it('state 2 offers Save-as, state 3 offers Set, an empty list offers neither twice', async () => {
    vi.spyOn(mockIpc, 'getConfig').mockResolvedValue(
      identityView('Mock Fixture User', 'fixture@bonsai.dev', 'global'),
    );
    const { onOpenSettingsAt, onProfilesChange } = renderMenu('/repo/s2');
    await waitFor(() =>
      expect(trigger()).toHaveAttribute('aria-label', 'Commit identity: Mock Fixture User'),
    );
    await openMenu();

    const saveAs = screen.getByRole('menuitem', {
      name: 'Save “Mock Fixture User” as an identity…',
    });
    fireEvent.click(saveAs);

    // The row SAVES: it appends the effective identity as a real profile and
    // then lands on that card. Merely navigating to Identities would make the
    // user retype the name and email already on screen.
    expect(onProfilesChange).toHaveBeenCalledTimes(1);
    const next = onProfilesChange.mock.calls[0][0] as IdentityProfile[];
    expect(next).toHaveLength(3);
    expect(next[2]).toMatchObject({
      label: '',
      userName: 'Mock Fixture User',
      userEmail: 'fixture@bonsai.dev',
      signingKey: null,
    });
    expect(next[2].id).toEqual(expect.any(String));
    expect(onOpenSettingsAt).toHaveBeenCalledWith('identities', null, next[2].id);
  });

  it('an empty profile list still renders an actionable menu', async () => {
    vi.spyOn(mockIpc, 'getConfig').mockResolvedValue(EMPTY_VIEW);
    const { onOpenSettingsAt } = renderMenu('/repo/empty', []);
    await waitFor(() => expect(trigger()).toHaveAttribute('aria-label', 'Commit identity not set'));
    await openMenu();

    expect(screen.getByText('No commit identity set')).toBeInTheDocument();
    expect(screen.getByText('Commits will fail until you set a name and email.')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('menuitem', { name: 'Set an identity…' }));
    // The exact `configMissing` deep link (UI §4.7).
    expect(onOpenSettingsAt).toHaveBeenCalledWith('git-config', 'identity');
  });

  it('lifts its open state so App can suppress global shortcuts', async () => {
    vi.spyOn(mockIpc, 'getConfig').mockResolvedValue(EMPTY_VIEW);
    const { onMenuOpenChange } = renderMenu('/repo/lift');
    await waitFor(() => expect(trigger()).toHaveAttribute('aria-label', 'Commit identity not set'));

    onMenuOpenChange.mockClear();
    await openMenu();
    expect(onMenuOpenChange).toHaveBeenLastCalledWith(true);
    fireEvent.keyDown(window, { key: 'Escape' });
    await waitFor(() => expect(onMenuOpenChange).toHaveBeenLastCalledWith(false));
  });
});
