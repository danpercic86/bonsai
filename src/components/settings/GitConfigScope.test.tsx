/**
 * P69h — scope clarity (UI §1.1 / §1.2).
 *
 * Git config is the ONE per-repository category in an otherwise global dialog,
 * and before this increment nothing on the pane said so: the control was a
 * self-labelling `Local | Global` button pair using libgit2's word, and the
 * no-repo state was a bare sentence. The three things pinned here are the three
 * that carry the meaning — a named `Scope` radiogroup, a line naming the actual
 * FILE, and an empty state that offers the fix instead of describing the problem.
 */
import { describe, expect, it, vi, afterEach } from 'vitest';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';

import { SettingsPanel } from '../SettingsPanel';
import type { SettingsPanelProps } from '../SettingsPanel';
import { MAXIMAL, MINIMAL, FIXTURE_CONFIG_VIEW } from './coverageFixtures';
import { mockIpc } from '../../ipc/mock';
import { resetEffectiveIdentityForTests } from '../../hooks/useEffectiveIdentity';

function renderPanel(over: Partial<SettingsPanelProps> = {}) {
  const props: SettingsPanelProps = {
    open: true,
    onClose: vi.fn(),
    requestSeq: 0,
    initialCategory: 'git-config',
    onChange: vi.fn(),
    onToggleTheme: vi.fn(),
    onToggleListView: vi.fn(),
    onRequestEnableAi: vi.fn(),
    onSetMcpEnabled: vi.fn(),
    onRequestEnableMcp: vi.fn(),
    onSetMcpAllowWrite: vi.fn(),
    onRequestEnableMcpWrite: vi.fn(),
    onRegisterMcp: vi.fn(async () => {}),
    onShowOnboarding: vi.fn(),
    onOpenRepository: vi.fn(),
    onCheckUpdate: vi.fn(),
    onOpenUpdateDialog: vi.fn(),
    ...MINIMAL,
    ...over,
  };
  return { ...render(<SettingsPanel {...props} />), props };
}

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
  resetEffectiveIdentityForTests();
});

describe('Git config — the scope switch (UI §1.1)', () => {
  it('is a named radiogroup in the pane header, not two self-labelling buttons', async () => {
    vi.spyOn(mockIpc, 'getConfig').mockResolvedValue(FIXTURE_CONFIG_VIEW);
    renderPanel({ ...MAXIMAL });

    const group = await screen.findByRole('radiogroup', { name: 'Scope' });
    expect(group.closest('.settings-pane-header')).not.toBeNull();
    const options = screen.getAllByRole('radio').map((r) => r.getAttribute('value'));
    expect(options).toEqual(['local', 'global']);
    expect(screen.getByRole('radio', { name: 'This repository' })).toBeChecked();
  });

  it('names the FILE being edited, and the repo it belongs to', async () => {
    vi.spyOn(mockIpc, 'getConfig').mockResolvedValue(FIXTURE_CONFIG_VIEW);
    renderPanel({ ...MAXIMAL, repoPath: '/work/bonsai' });

    const line = await screen.findByText(/Editing/);
    expect(line).toHaveTextContent('Editing .git/config in bonsai');
    // The full path is a hover fact, not a truncated headline.
    expect(line).toHaveAttribute('title', '/work/bonsai');
  });

  it('switching to Global retargets the reads and re-words the scope line', async () => {
    const getConfig = vi.spyOn(mockIpc, 'getConfig').mockResolvedValue(FIXTURE_CONFIG_VIEW);
    renderPanel({ ...MAXIMAL });
    await screen.findByRole('radiogroup', { name: 'Scope' });

    fireEvent.click(screen.getByRole('radio', { name: 'Global' }));

    expect(getConfig).toHaveBeenCalledWith(MAXIMAL.repoPath, 'global');
    expect(await screen.findByText(/global Git config/)).toBeInTheDocument();
  });

  it('shows no scope switch when there is no repository to scope to', () => {
    renderPanel();
    expect(screen.queryByRole('radiogroup', { name: 'Scope' })).toBeNull();
  });
});

describe('Git config — no repository open (UI §1.2)', () => {
  it('replaces the bare sentence with a titled block that offers the fix', () => {
    const { props } = renderPanel();

    expect(screen.getByText('No repository open')).toBeInTheDocument();
    expect(
      screen.getByText('Git config is stored per repository. Open one to view and edit it.'),
    ).toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: 'Open repository…' }));
    expect(props.onOpenRepository).toHaveBeenCalledTimes(1);
  });

  it('leaves the rail item enabled — a dead tab explains nothing', () => {
    renderPanel();
    const tab = screen.getByRole('tab', { name: 'Git config, repository' });
    expect(tab).toHaveAttribute('aria-selected', 'true');
    expect(tab).not.toBeDisabled();
  });
});

describe('Git config — the configMissing deep link (§5.4)', () => {
  it('re-focuses user.name when a SECOND deep link arrives on the pane it is already on', async () => {
    vi.spyOn(mockIpc, 'getConfig').mockImplementation(() => Promise.resolve(FIXTURE_CONFIG_VIEW));
    const { rerender, props } = renderPanel({ ...MAXIMAL, configInitialFocus: 'identity' });

    await waitFor(() =>
      expect(document.activeElement).toBe(document.getElementById('cfg-user.name')),
    );
    // The user moves on; the `focusedOnce` guard must keep focus put…
    (document.getElementById('cfg-user.email') as HTMLInputElement).focus();

    // …until a NEW request says the identity is missing again.
    rerender(
      <SettingsPanel
        {...props}
        {...MAXIMAL}
        configInitialFocus="identity"
        initialCategory="git-config"
        requestSeq={props.requestSeq + 1}
      />,
    );
    await waitFor(() =>
      expect(document.activeElement).toBe(document.getElementById('cfg-user.name')),
    );
  });
});
