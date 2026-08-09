/** T3.5 — ProfileManager: list rendering (active chip), create/edit form,
 *  save/delete round-trips (onStoreChange), inline invalidName error, and the
 *  Activate entry into the safety-gated dialog. IPC via vi.spyOn(mockIpc, …). */
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { ProfileManager } from './ProfileManager';
import { mockIpc } from '../ipc/mock';
import type { AiAssetInventory, ContextProfile, ProfileStore } from '../ipc';

function profile(name: string, over: Partial<ContextProfile> = {}): ContextProfile {
  return { name, description: null, model: null, targets: [], ...over };
}

function store(over: Partial<ProfileStore> = {}): ProfileStore {
  return {
    version: 1,
    profiles: [profile('opus-rich', { model: 'opus', description: 'Rich context' })],
    activeProfile: 'opus-rich',
    ...over,
  };
}

const INVENTORY: AiAssetInventory = {
  assets: [
    {
      id: 'claude',
      agent: 'claude',
      label: 'CLAUDE.md',
      kind: 'singleFile',
      path: 'CLAUDE.md',
      managed: true,
      exists: true,
      files: [],
    },
  ],
  drift: { canonicalId: 'claude', canonicalHash: null, entries: [], inSync: true },
};

function renderManager(over: Partial<Parameters<typeof ProfileManager>[0]> = {}) {
  const props = {
    repoId: '/mock/repo',
    store: store(),
    inventory: INVENTORY,
    aiEnabled: false,
    onStoreChange: vi.fn(),
    onActivated: vi.fn(),
    ...over,
  };
  return { ...render(<ProfileManager {...props} />), props };
}

beforeEach(() => vi.restoreAllMocks());

describe('ProfileManager', () => {
  it('lists profiles with active chip, model chip, and target count', () => {
    renderManager();
    expect(screen.getByText('opus-rich')).toBeInTheDocument();
    expect(screen.getByText('active')).toBeInTheDocument();
    expect(screen.getByText('opus')).toBeInTheDocument();
    expect(screen.getByText('Rich context')).toBeInTheDocument();
    expect(screen.getByText('0 targets')).toBeInTheDocument();
  });

  it('empty store shows the create hint', () => {
    renderManager({ store: store({ profiles: [], activeProfile: null }) });
    expect(screen.getByText(/No profiles yet/)).toBeInTheDocument();
  });

  it('save trims optionals to null, strips target uids, and reports the new store', async () => {
    const updated = store({ profiles: [profile('lean')] });
    const spy = vi.spyOn(mockIpc, 'saveProfile').mockResolvedValue(updated);
    const { props } = renderManager();
    fireEvent.click(screen.getByRole('button', { name: 'New profile' }));
    fireEvent.change(screen.getByPlaceholderText('opus-rich'), { target: { value: '  lean  ' } });
    fireEvent.click(screen.getByRole('button', { name: 'Add target' }));
    fireEvent.change(screen.getByPlaceholderText('Instruction file content…'), {
      target: { value: 'be terse' },
    });
    fireEvent.click(screen.getByRole('button', { name: 'Save profile' }));
    await vi.waitFor(() => expect(props.onStoreChange).toHaveBeenCalledWith(updated));
    expect(spy).toHaveBeenCalledWith('/mock/repo', {
      name: 'lean',
      description: null,
      model: null,
      targets: [{ assetId: 'claude', content: 'be terse' }],
    });
    // Form closes on success.
    expect(screen.queryByRole('button', { name: 'Save profile' })).not.toBeInTheDocument();
  });

  it('invalidName rejection surfaces inline and keeps the form open', async () => {
    vi.spyOn(mockIpc, 'saveProfile').mockRejectedValue({
      kind: 'invalidName',
      message: 'name must not contain /',
    });
    renderManager();
    fireEvent.click(screen.getByRole('button', { name: 'New profile' }));
    fireEvent.change(screen.getByPlaceholderText('opus-rich'), { target: { value: 'a/b' } });
    fireEvent.click(screen.getByRole('button', { name: 'Save profile' }));
    expect(await screen.findByText('name must not contain /')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Save profile' })).toBeInTheDocument();
  });

  it('Edit seeds the form from the profile', () => {
    renderManager({
      store: store({
        profiles: [
          profile('opus-rich', {
            description: 'Rich context',
            model: 'opus',
            targets: [{ assetId: 'claude', content: 'existing' }],
          }),
        ],
      }),
    });
    fireEvent.click(screen.getByRole('button', { name: 'Edit' }));
    expect(screen.getByText('Edit “opus-rich”')).toBeInTheDocument();
    expect(screen.getByDisplayValue('opus-rich')).toBeInTheDocument();
    expect(screen.getByDisplayValue('existing')).toBeInTheDocument();
  });

  it('Delete arms the ConfirmDialog; confirming calls deleteProfile + onStoreChange', async () => {
    const updated = store({ profiles: [], activeProfile: null });
    const spy = vi.spyOn(mockIpc, 'deleteProfile').mockResolvedValue(updated);
    const { props } = renderManager();
    fireEvent.click(screen.getByRole('button', { name: 'Delete' }));
    expect(screen.getByText('Delete profile?')).toBeInTheDocument();
    expect(spy).not.toHaveBeenCalled(); // nothing until confirm
    // Two Delete buttons exist now (row + dialog); the dialog confirm is last.
    const confirms = screen.getAllByRole('button', { name: 'Delete' });
    fireEvent.click(confirms[confirms.length - 1]);
    await vi.waitFor(() => expect(props.onStoreChange).toHaveBeenCalledWith(updated));
    expect(spy).toHaveBeenCalledWith('/mock/repo', 'opus-rich');
  });

  it('Cancel in the delete dialog writes nothing', () => {
    const spy = vi.spyOn(mockIpc, 'deleteProfile');
    renderManager();
    fireEvent.click(screen.getByRole('button', { name: 'Delete' }));
    fireEvent.click(screen.getByRole('button', { name: 'Cancel' }));
    expect(spy).not.toHaveBeenCalled();
    expect(screen.queryByText('Delete profile?')).not.toBeInTheDocument();
  });

  it('Activate opens the preview dialog (no write until its confirm)', async () => {
    const preview = vi.spyOn(mockIpc, 'previewProfile').mockResolvedValue([]);
    const activate = vi.spyOn(mockIpc, 'activateProfile');
    renderManager();
    fireEvent.click(screen.getByRole('button', { name: 'Activate' }));
    expect(await screen.findByRole('dialog', { name: 'Activate opus-rich' })).toBeInTheDocument();
    expect(preview).toHaveBeenCalledWith('/mock/repo', 'opus-rich');
    expect(activate).not.toHaveBeenCalled();
  });

  it('Translate button is gated on aiEnabled', () => {
    renderManager({ aiEnabled: false });
    fireEvent.click(screen.getByRole('button', { name: 'New profile' }));
    fireEvent.click(screen.getByRole('button', { name: 'Add target' }));
    const translate = screen.getByRole('button', { name: /Translate for claude/ });
    expect(translate).toBeDisabled();
    expect(translate).toHaveAttribute('title', 'Enable AI features in Settings to use this');
  });
});
