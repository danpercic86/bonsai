/** T3.7 — AgentAssetEditor: create/edit form for one agent asset. IPC stubbed
 *  via vi.spyOn(mockIpc, …). Covers create-template render, name↔frontmatter
 *  mirroring, save payload + onSaved/onClose, edit-mode load, complex read-only
 *  gating, delete flow, inline invalidName error, and Esc. */
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { AgentAssetEditor } from './AgentAssetEditor';
import { ToastContext } from '../ToastContext';
import { mockIpc } from '../ipc/mock';
import type { AgentAsset, AgentAssetInput, AgentAssetInventory } from '../ipc';

function asset(over: Partial<AgentAsset> = {}): AgentAsset {
  return {
    kind: 'skill',
    name: 'my-skill',
    path: '.claude/skills/my-skill/SKILL.md',
    exists: true,
    frontmatter: [
      { key: 'name', value: 'my-skill' },
      { key: 'description', value: 'does a thing' },
    ],
    body: 'Instructions here.\n',
    complex: false,
    validation: { valid: true, issues: [] },
    ...over,
  };
}

function inv(assets: AgentAsset[]): AgentAssetInventory {
  return { assets };
}

function renderEditor(
  props: Partial<React.ComponentProps<typeof AgentAssetEditor>> = {},
  pushToast = vi.fn(),
) {
  const full = {
    repoId: '/mock/repo',
    kind: 'skill' as const,
    name: null,
    onSaved: vi.fn(),
    onClose: vi.fn(),
    ...props,
  };
  render(
    <ToastContext.Provider value={pushToast}>
      <AgentAssetEditor {...full} />
    </ToastContext.Provider>,
  );
  return { ...full, pushToast };
}

beforeEach(() => {
  vi.restoreAllMocks();
});

describe('AgentAssetEditor — create mode', () => {
  it('renders the New-skill template with known-key fields and no Delete button', () => {
    renderEditor();
    expect(screen.getByRole('dialog', { name: 'New skill' })).toBeInTheDocument();
    // skill known keys include a Description field.
    expect(screen.getByText(/Description/)).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'Delete' })).not.toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Save' })).toBeEnabled();
  });

  it('mirrors the identity name into the frontmatter name field', () => {
    renderEditor();
    const nameInput = screen.getByPlaceholderText('my-skill') as HTMLInputElement;
    fireEvent.change(nameInput, { target: { value: 'brew-tea' } });
    // The frontmatter "Name (optional)" field mirrors the identity while pristine.
    const fmName = screen
      .getAllByDisplayValue('brew-tea')
      .find((el) => el !== nameInput);
    expect(fmName).toBeDefined();
  });

  it('Save sends the built input, toasts success, and calls onSaved + onClose', async () => {
    const saveSpy = vi
      .spyOn(mockIpc, 'saveAgentAsset')
      .mockResolvedValue(inv([asset({ name: 'brew-tea' })]));
    const pushToast = vi.fn();
    const { onSaved, onClose } = renderEditor({}, pushToast);
    fireEvent.change(screen.getByPlaceholderText('my-skill'), { target: { value: 'brew-tea' } });
    fireEvent.click(screen.getByRole('button', { name: 'Save' }));
    await waitFor(() => expect(onClose).toHaveBeenCalledTimes(1));
    expect(saveSpy).toHaveBeenCalledTimes(1);
    const [, input] = saveSpy.mock.calls[0] as [string, AgentAssetInput];
    expect(input.name).toBe('brew-tea');
    expect(input.kind).toBe('skill');
    expect(onSaved).toHaveBeenCalledTimes(1);
    expect(pushToast).toHaveBeenCalledWith('success', expect.stringContaining('brew-tea'));
  });

  it('an invalidName rejection surfaces inline (no close)', async () => {
    vi.spyOn(mockIpc, 'saveAgentAsset').mockRejectedValue({
      kind: 'invalidName',
      message: 'name must be kebab-case',
    });
    const { onClose } = renderEditor();
    fireEvent.change(screen.getByPlaceholderText('my-skill'), { target: { value: 'Bad Name' } });
    fireEvent.click(screen.getByRole('button', { name: 'Save' }));
    await waitFor(() =>
      expect(screen.getByText('name must be kebab-case')).toBeInTheDocument(),
    );
    expect(onClose).not.toHaveBeenCalled();
  });
});

describe('AgentAssetEditor — edit mode', () => {
  it('loads the asset, disables the name, and shows a Delete button', async () => {
    vi.spyOn(mockIpc, 'readAgentAsset').mockResolvedValue(asset());
    renderEditor({ name: 'my-skill' });
    expect(await screen.findByRole('dialog', { name: 'Edit skill “my-skill”' })).toBeInTheDocument();
    const nameInput = screen.getByPlaceholderText('my-skill') as HTMLInputElement;
    expect(nameInput).toBeDisabled();
    expect(screen.getByRole('button', { name: 'Delete' })).toBeInTheDocument();
  });

  it('a complex asset opens read-only with Save disabled and a banner', async () => {
    vi.spyOn(mockIpc, 'readAgentAsset').mockResolvedValue(
      asset({ complex: true, validation: { valid: false, issues: [] } }),
    );
    renderEditor({ name: 'my-skill' });
    await screen.findByRole('dialog', { name: 'Edit skill “my-skill”' });
    expect(screen.getByText(/complex YAML frontmatter/)).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Save' })).toBeDisabled();
  });

  it('lists validation issues for a merely-invalid (still editable) asset', async () => {
    vi.spyOn(mockIpc, 'readAgentAsset').mockResolvedValue(
      asset({
        validation: { valid: false, issues: [{ severity: 'warning', message: 'missing hint' }] },
      }),
    );
    renderEditor({ name: 'my-skill' });
    expect(await screen.findByText('missing hint')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Save' })).toBeEnabled();
  });

  it('Delete → confirm calls deleteAgentAsset, onSaved and onClose', async () => {
    vi.spyOn(mockIpc, 'readAgentAsset').mockResolvedValue(asset());
    const delSpy = vi.spyOn(mockIpc, 'deleteAgentAsset').mockResolvedValue(inv([]));
    const { onSaved, onClose } = renderEditor({ name: 'my-skill' });
    await screen.findByRole('button', { name: 'Delete' });
    fireEvent.click(screen.getByRole('button', { name: 'Delete' }));
    // ConfirmDialog opens; its confirm button is also labelled "Delete".
    const confirm = screen.getAllByRole('button', { name: 'Delete' }).pop()!;
    fireEvent.click(confirm);
    await waitFor(() => expect(onClose).toHaveBeenCalledTimes(1));
    expect(delSpy).toHaveBeenCalledWith('/mock/repo', 'skill', 'my-skill');
    expect(onSaved).toHaveBeenCalledTimes(1);
  });

  it('a load failure shows the error banner', async () => {
    vi.spyOn(mockIpc, 'readAgentAsset').mockRejectedValue({ kind: 'other', message: 'no such asset' });
    renderEditor({ name: 'ghost' });
    expect(await screen.findByText('no such asset')).toBeInTheDocument();
  });
});

describe('AgentAssetEditor — Esc', () => {
  it('Escape closes the editor', () => {
    const { onClose } = renderEditor();
    fireEvent.keyDown(window, { key: 'Escape' });
    expect(onClose).toHaveBeenCalledTimes(1);
  });
});
