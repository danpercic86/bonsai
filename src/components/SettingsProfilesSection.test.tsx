/**
 * P69d — Identity profiles, after the pill moved onto the effective-identity store.
 *
 * The defect being closed (UI D6): the "Active on this repo" pill used to match the
 * repo's LOCAL user.name/user.email only. Git resolves local-overrides-global, so in
 * the ordinary case — nothing set locally, an identity in ~/.gitconfig — the repo
 * commits happily under a profile's identity while the pill stayed dark. The first
 * two tests are that exact scenario.
 *
 * Also pinned: Apply sends the profile's CURRENT in-memory fields (never an id, which
 * would race an unsaved edit), the per-profile error, the "Applied" flash, and the
 * explicit store invalidation that makes the pill catch up without a repo-changed
 * event (setConfig does not emit one).
 */
import { useState } from 'react';
import { describe, expect, it, vi, beforeEach, afterEach } from 'vitest';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';

import { SettingsProfilesSection } from './SettingsProfilesSection';
import { mockIpc } from '../ipc/mock';
import { resetEffectiveIdentityForTests } from '../hooks/useEffectiveIdentity';
import type { ConfigLevelName, ConfigView, IdentityProfile } from '../ipc';

const WORK: IdentityProfile = {
  id: 'p-work',
  label: 'Work',
  userName: 'Ada Lovelace',
  userEmail: 'ada@work.dev',
  signingKey: null,
};
const PERSONAL: IdentityProfile = {
  id: 'p-personal',
  label: 'Personal',
  userName: 'Ada L',
  userEmail: 'me@personal.dev',
  signingKey: 'KEY1',
};

/** A `getConfig(repo, 'local')` reply whose identity is effective at `level` — with
 *  `targetValue` set only when that level IS local (which is the whole point). */
function identityView(name: string, email: string, level: ConfigLevelName): ConfigView {
  const local = level === 'local';
  return {
    targetLevel: 'local',
    curated: [
      {
        key: 'user.name',
        kind: 'text',
        enumValues: [],
        effectiveValue: name,
        effectiveLevel: level,
        targetValue: local ? name : null,
      },
      {
        key: 'user.email',
        kind: 'text',
        enumValues: [],
        effectiveValue: email,
        effectiveLevel: level,
        targetValue: local ? email : null,
      },
    ],
    advanced: [],
  };
}

const EMPTY_VIEW: ConfigView = { targetLevel: 'local', curated: [], advanced: [] };

/** A live harness: edits are applied back onto the props exactly as App does, so an
 *  edit-then-Apply flow behaves like the real panel (that flow is the reason Apply
 *  takes the profile object rather than its id). */
function renderLive(repoId: string, initial: IdentityProfile[]) {
  function Harness() {
    const [profiles, setProfiles] = useState(initial);
    return (
      <SettingsProfilesSection
        repoId={repoId}
        profiles={profiles}
        onProfilesChange={setProfiles}
      />
    );
  }
  return render(<Harness />);
}

function renderSection(repoId: string, profiles: IdentityProfile[] = [WORK, PERSONAL]) {
  const onProfilesChange = vi.fn();
  const utils = render(
    <SettingsProfilesSection
      repoId={repoId}
      profiles={profiles}
      onProfilesChange={onProfilesChange}
    />,
  );
  return { ...utils, onProfilesChange };
}

beforeEach(() => {
  resetEffectiveIdentityForTests();
});
afterEach(() => {
  vi.restoreAllMocks();
  resetEffectiveIdentityForTests();
});

describe('SettingsProfilesSection — the Active pill is effective-based', () => {
  it('lights up for a GLOBAL identity with an empty local block (UI D6)', async () => {
    vi.spyOn(mockIpc, 'getConfig').mockResolvedValue(
      identityView('Ada Lovelace', 'ada@work.dev', 'global'),
    );
    renderSection('/repo/d6');

    expect(await screen.findByText('Active on this repo')).toBeInTheDocument();
    // Exactly one profile is ever active.
    expect(screen.getAllByText('Active on this repo')).toHaveLength(1);
    expect(screen.getByText('Work').closest('.settings-profile')).toContainElement(
      screen.getByText('Active on this repo'),
    );
  });

  it('a LOCAL identity still matches (local wins)', async () => {
    vi.spyOn(mockIpc, 'getConfig').mockResolvedValue(
      identityView('Ada L', 'me@personal.dev', 'local'),
    );
    renderSection('/repo/local');

    await waitFor(() =>
      expect(screen.getByText('Personal').closest('.settings-profile')).toContainElement(
        screen.getByText('Active on this repo'),
      ),
    );
  });

  it('an identity that matches no profile lights nothing (the default harness state)', async () => {
    // The mock fixtures seed global `Mock Fixture User` / `fixture@bonsai.dev`, which
    // is deliberately NOT one of the seeded profiles.
    const spy = vi
      .spyOn(mockIpc, 'getConfig')
      .mockResolvedValue(identityView('Mock Fixture User', 'fixture@bonsai.dev', 'global'));
    renderSection('/repo/nomatch');

    await waitFor(() => expect(spy).toHaveBeenCalled());
    expect(screen.queryByText('Active on this repo')).toBeNull();
  });

  it('no identity anywhere, and an unreadable config, light nothing', async () => {
    vi.spyOn(mockIpc, 'getConfig').mockResolvedValue(EMPTY_VIEW);
    const first = renderSection('/repo/unset');
    await waitFor(() => expect(screen.getByText('Work')).toBeInTheDocument());
    expect(screen.queryByText('Active on this repo')).toBeNull();
    first.unmount();

    vi.spyOn(mockIpc, 'getConfig').mockRejectedValue(new Error('config unreadable'));
    renderSection('/repo/broken');
    await waitFor(() => expect(screen.getByText('Work')).toBeInTheDocument());
    expect(screen.queryByText('Active on this repo')).toBeNull();
  });

  it('no repo open: no config read, Apply disabled, and the reason is shown', async () => {
    const spy = vi.spyOn(mockIpc, 'getConfig').mockResolvedValue(EMPTY_VIEW);
    render(
      <SettingsProfilesSection repoId={null} profiles={[WORK]} onProfilesChange={vi.fn()} />,
    );

    await Promise.resolve();
    expect(spy).not.toHaveBeenCalled();
    expect(screen.getByRole('button', { name: 'Apply to current repo' })).toBeDisabled();
    expect(screen.getByText('Open a repository to apply a profile.')).toBeInTheDocument();
  });
});

describe('SettingsProfilesSection — Apply and CRUD', () => {
  it('applies the CURRENT in-memory fields, flashes Applied, and refreshes the pill', async () => {
    const getConfig = vi
      .spyOn(mockIpc, 'getConfig')
      .mockResolvedValue(identityView('Nobody', 'nobody@x.dev', 'global'));
    const apply = vi.spyOn(mockIpc, 'applyIdentityProfile').mockResolvedValue(EMPTY_VIEW);
    renderLive('/repo/apply', [WORK]);
    await waitFor(() => expect(getConfig).toHaveBeenCalledTimes(1));
    expect(screen.queryByText('Active on this repo')).toBeNull();

    // An unsaved edit must be what gets applied — this is why the handler takes the
    // profile object, not its id.
    fireEvent.change(screen.getByLabelText('user.email'), { target: { value: 'ada@edited.dev' } });
    // What the repo will read back after the write: the EDITED identity.
    getConfig.mockResolvedValue(identityView('Ada Lovelace', 'ada@edited.dev', 'local'));
    fireEvent.click(screen.getByRole('button', { name: 'Apply to current repo' }));

    await waitFor(() =>
      expect(apply).toHaveBeenCalledWith('/repo/apply', 'Ada Lovelace', 'ada@edited.dev', null),
    );
    expect(await screen.findByText('Applied')).toBeInTheDocument();
    // Invalidation refetched, so the pill catches up with no repo-changed event.
    await waitFor(() => expect(getConfig).toHaveBeenCalledTimes(2));
    expect(await screen.findByText('Active on this repo')).toBeInTheDocument();
  });

  it('a failed Apply shows the error on that profile and no Applied flash', async () => {
    vi.spyOn(mockIpc, 'getConfig').mockResolvedValue(EMPTY_VIEW);
    vi.spyOn(mockIpc, 'applyIdentityProfile').mockRejectedValue(new Error('identity write failed'));
    renderSection('/repo/fail', [WORK]);

    fireEvent.click(await screen.findByRole('button', { name: 'Apply to current repo' }));

    expect(await screen.findByText('identity write failed')).toBeInTheDocument();
    expect(screen.queryByText('Applied')).toBeNull();
  });

  it('Add and Delete replace the whole list', async () => {
    vi.spyOn(mockIpc, 'getConfig').mockResolvedValue(EMPTY_VIEW);
    const { onProfilesChange } = renderSection('/repo/crud', [WORK]);

    fireEvent.click(screen.getByRole('button', { name: 'Add profile' }));
    expect(onProfilesChange.mock.calls[0][0]).toHaveLength(2);
    expect(onProfilesChange.mock.calls[0][0][1]).toMatchObject({ label: '', userName: '' });

    fireEvent.click(screen.getByRole('button', { name: 'Delete' }));
    expect(onProfilesChange).toHaveBeenLastCalledWith([]);
  });

  it('field edits patch only that profile', async () => {
    vi.spyOn(mockIpc, 'getConfig').mockResolvedValue(EMPTY_VIEW);
    const { onProfilesChange } = renderSection('/repo/edit');

    fireEvent.change(screen.getByLabelText('Label', { selector: '#profile-label-p-work' }), {
      target: { value: 'Job' },
    });
    expect(onProfilesChange).toHaveBeenLastCalledWith([{ ...WORK, label: 'Job' }, PERSONAL]);
  });
});
