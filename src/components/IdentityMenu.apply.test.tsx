/**
 * P69i — applying an identity from the header menu (UI §4.5): the confirmation
 * rule, re-entrancy, the in-flight label, and the success/failure toasts. The
 * trigger states live in `IdentityMenu.test.tsx`.
 */
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { fireEvent, screen, waitFor } from '@testing-library/react';

import { mockIpc } from '../ipc/mock';
import { resetEffectiveIdentityForTests } from '../hooks/useEffectiveIdentity';
import { EMPTY_VIEW, WORK, identityView, openMenu, renderMenu, trigger } from '../test/identityKit';
import type { ConfigView, IdentityProfile } from '../ipc';

beforeEach(() => {
  resetEffectiveIdentityForTests();
});
afterEach(() => {
  vi.restoreAllMocks();
  resetEffectiveIdentityForTests();
});

describe('IdentityMenu — applying (UI §4.5)', () => {
  it('a repo that INHERITS the global identity applies with no confirm', async () => {
    vi.spyOn(mockIpc, 'getConfig').mockResolvedValue(
      identityView('Mock Fixture User', 'fixture@bonsai.dev', 'global'),
    );
    const apply = vi.spyOn(mockIpc, 'applyIdentityProfile').mockResolvedValue(EMPTY_VIEW);
    const { pushToast } = renderMenu('/repo/inherit');
    await waitFor(() =>
      expect(trigger()).toHaveAttribute('aria-label', 'Commit identity: Mock Fixture User'),
    );
    await openMenu();

    fireEvent.click(screen.getByRole('menuitemradio', { name: /Work/ }));

    expect(screen.queryByRole('dialog')).toBeNull();
    await waitFor(() =>
      // The profile's CURRENT in-memory fields, never its id.
      expect(apply).toHaveBeenCalledWith('/repo/inherit', 'Ada Lovelace', 'work@bonsai.dev', null),
    );
    await waitFor(() =>
      expect(pushToast).toHaveBeenCalledWith(
        'success',
        'Now committing as Work in this repository.',
        'identity:/repo/inherit',
      ),
    );
  });

  it('a DIFFERING local identity confirms first, naming both sides', async () => {
    vi.spyOn(mockIpc, 'getConfig').mockResolvedValue(
      identityView('Sam Carter', 'sam@old.dev', 'local'),
    );
    const apply = vi.spyOn(mockIpc, 'applyIdentityProfile').mockResolvedValue(EMPTY_VIEW);
    renderMenu('/repo/differs');
    await waitFor(() =>
      expect(trigger()).toHaveAttribute('aria-label', 'Commit identity: Sam Carter'),
    );
    await openMenu();

    fireEvent.click(screen.getByRole('menuitemradio', { name: /Work/ }));

    expect(apply).not.toHaveBeenCalled();
    expect(await screen.findByText('Change this repository’s identity?')).toBeInTheDocument();
    expect(
      screen.getByText(
        'This repository commits as Sam Carter <sam@old.dev>, set in its own Git config. Using Work replaces that with Ada Lovelace <work@bonsai.dev>.',
      ),
    ).toBeInTheDocument();
    expect(
      screen.getByText(
        'Commits you have already made are not changed. You can switch back at any time.',
      ),
    ).toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: 'Change identity' }));
    await waitFor(() =>
      expect(apply).toHaveBeenCalledWith('/repo/differs', 'Ada Lovelace', 'work@bonsai.dev', null),
    );
  });

  it('cancelling the confirm writes nothing', async () => {
    vi.spyOn(mockIpc, 'getConfig').mockResolvedValue(
      identityView('Sam Carter', 'sam@old.dev', 'local'),
    );
    const apply = vi.spyOn(mockIpc, 'applyIdentityProfile').mockResolvedValue(EMPTY_VIEW);
    renderMenu('/repo/cancel');
    await waitFor(() =>
      expect(trigger()).toHaveAttribute('aria-label', 'Commit identity: Sam Carter'),
    );
    await openMenu();
    fireEvent.click(screen.getByRole('menuitemradio', { name: /Personal/ }));

    fireEvent.click(await screen.findByRole('button', { name: 'Cancel' }));
    await waitFor(() => expect(screen.queryByText('Change this repository’s identity?')).toBeNull());
    expect(apply).not.toHaveBeenCalled();
  });

  it('re-selecting the identity the repo already has locally is a silent no-op', async () => {
    vi.spyOn(mockIpc, 'getConfig').mockResolvedValue(
      identityView('Ada Lovelace', 'work@bonsai.dev', 'local'),
    );
    const apply = vi.spyOn(mockIpc, 'applyIdentityProfile').mockResolvedValue(EMPTY_VIEW);
    renderMenu('/repo/noop');
    await waitFor(() => expect(trigger()).toHaveAttribute('aria-label', 'Commit identity: Work'));
    await openMenu();

    fireEvent.click(screen.getByRole('menuitemradio', { name: /Work/ }));
    await waitFor(() => expect(screen.queryByRole('menu')).toBeNull());
    expect(apply).not.toHaveBeenCalled();
    expect(screen.queryByRole('dialog')).toBeNull();
  });

  it('shows Applying… in the open menu, then closes and toasts', async () => {
    vi.spyOn(mockIpc, 'getConfig').mockResolvedValue(
      identityView('Mock Fixture User', 'fixture@bonsai.dev', 'global'),
    );
    let settle: (v: ConfigView) => void = () => {};
    vi.spyOn(mockIpc, 'applyIdentityProfile').mockReturnValue(
      new Promise<ConfigView>((r) => {
        settle = r;
      }),
    );
    const { pushToast } = renderMenu('/repo/inflight');
    await waitFor(() =>
      expect(trigger()).toHaveAttribute('aria-label', 'Commit identity: Mock Fixture User'),
    );
    await openMenu();
    fireEvent.click(screen.getByRole('menuitemradio', { name: /Work/ }));

    // UI §4.5: the menu deliberately STAYS open, busy, until the write settles.
    const menu = await screen.findByRole('menu');
    expect(menu).toHaveAttribute('aria-busy', 'true');
    expect(screen.getByRole('menuitemradio', { name: /Applying…/ })).toBeInTheDocument();
    expect(screen.getByRole('menuitemradio', { name: /Personal/ })).toBeDisabled();

    settle(EMPTY_VIEW);
    await waitFor(() => expect(screen.queryByRole('menu')).toBeNull());
    expect(pushToast).toHaveBeenCalledWith(
      'success',
      'Now committing as Work in this repository.',
      'identity:/repo/inflight',
    );
  });

  it('a second click while a write is in flight does not fire a second write', async () => {
    vi.spyOn(mockIpc, 'getConfig').mockResolvedValue(
      identityView('Mock Fixture User', 'fixture@bonsai.dev', 'global'),
    );
    let settle: (v: ConfigView) => void = () => {};
    const apply = vi.spyOn(mockIpc, 'applyIdentityProfile').mockReturnValue(
      new Promise<ConfigView>((r) => {
        settle = r;
      }),
    );
    renderMenu('/repo/reentrant');
    await waitFor(() =>
      expect(trigger()).toHaveAttribute('aria-label', 'Commit identity: Mock Fixture User'),
    );
    await openMenu();

    const row = screen.getByRole('menuitemradio', { name: /Work/ });
    fireEvent.click(row);
    // The in-flight row stays ENABLED (the menu is held open on purpose), so the
    // guard has to be in `select`, not in the disabled attribute.
    fireEvent.click(screen.getByRole('menuitemradio', { name: /Applying…/ }));
    expect(apply).toHaveBeenCalledTimes(1);

    settle(EMPTY_VIEW);
    await waitFor(() => expect(screen.queryByRole('menu')).toBeNull());
    expect(apply).toHaveBeenCalledTimes(1);
  });

  it('a ticked row inherited from GLOBAL is still a no-op, not a fresh write', async () => {
    // Orchestrator ruling (P69i review): `checked` is computed from the
    // EFFECTIVE identity, so a repo inheriting a matching global identity shows
    // ✓ — and clicking the ticked thing must never write.
    vi.spyOn(mockIpc, 'getConfig').mockResolvedValue(
      identityView('Ada Lovelace', 'work@bonsai.dev', 'global'),
    );
    const apply = vi.spyOn(mockIpc, 'applyIdentityProfile').mockResolvedValue(EMPTY_VIEW);
    renderMenu('/repo/globalmatch');
    await waitFor(() => expect(trigger()).toHaveAttribute('aria-label', 'Commit identity: Work'));
    await openMenu();
    expect(screen.getByRole('menuitemradio', { name: /Work/ })).toHaveAttribute(
      'aria-checked',
      'true',
    );

    fireEvent.click(screen.getByRole('menuitemradio', { name: /Work/ }));
    await waitFor(() => expect(screen.queryByRole('menu')).toBeNull());
    expect(apply).not.toHaveBeenCalled();
  });

  it('a LOCAL user.email under a GLOBAL user.name still confirms before overwriting', async () => {
    // `source` is user.name's level only; reading it alone would clobber a local
    // email with no warning at all.
    vi.spyOn(mockIpc, 'getConfig').mockResolvedValue({
      targetLevel: 'local',
      curated: [
        {
          key: 'user.name',
          kind: 'text',
          enumValues: [],
          effectiveValue: 'Sam Carter',
          effectiveLevel: 'global',
          targetValue: null,
        },
        {
          key: 'user.email',
          kind: 'text',
          enumValues: [],
          effectiveValue: 'sam@this-repo.dev',
          effectiveLevel: 'local',
          targetValue: 'sam@this-repo.dev',
        },
      ],
      advanced: [],
    });
    const apply = vi.spyOn(mockIpc, 'applyIdentityProfile').mockResolvedValue(EMPTY_VIEW);
    renderMenu('/repo/splitlevel');
    await waitFor(() =>
      expect(trigger()).toHaveAttribute('aria-label', 'Commit identity: Sam Carter'),
    );
    await openMenu();
    fireEvent.click(screen.getByRole('menuitemradio', { name: /Work/ }));

    expect(await screen.findByText('Change this repository’s identity?')).toBeInTheDocument();
    expect(apply).not.toHaveBeenCalled();
  });

  it('two profiles with the same identity tick only the first (menuitemradio)', async () => {
    vi.spyOn(mockIpc, 'getConfig').mockResolvedValue(
      identityView('Ada Lovelace', 'work@bonsai.dev', 'local'),
    );
    const TWIN: IdentityProfile = { ...WORK, id: 'p-twin', label: 'Work copy' };
    renderMenu('/repo/twins', [WORK, TWIN]);
    await waitFor(() => expect(trigger()).toHaveAttribute('aria-label', 'Commit identity: Work'));
    await openMenu();

    const checked = screen
      .getAllByRole('menuitemradio')
      .filter((r) => r.getAttribute('aria-checked') === 'true');
    expect(checked).toHaveLength(1);
    expect(checked[0].textContent).toContain('Work');
    expect(checked[0].textContent).not.toContain('Work copy');
  });

  it('a failed write closes the menu and surfaces the backend message verbatim', async () => {
    vi.spyOn(mockIpc, 'getConfig').mockResolvedValue(
      identityView('Mock Fixture User', 'fixture@bonsai.dev', 'global'),
    );
    vi.spyOn(mockIpc, 'applyIdentityProfile').mockRejectedValue(new Error('permission denied'));
    const { pushToast } = renderMenu('/repo/failed');
    await waitFor(() =>
      expect(trigger()).toHaveAttribute('aria-label', 'Commit identity: Mock Fixture User'),
    );
    await openMenu();
    fireEvent.click(screen.getByRole('menuitemradio', { name: /Work/ }));

    await waitFor(() =>
      expect(pushToast).toHaveBeenCalledWith(
        'error',
        'Couldn’t switch identity. permission denied',
        'identity:/repo/failed',
      ),
    );
    expect(screen.queryByRole('menu')).toBeNull();
  });
});
