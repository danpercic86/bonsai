/** T3.5 — ProfileActivateDialog: the activation safety gate. Preview loads on
 *  open (writes nothing), Activate stays disabled until the preview arrives,
 *  confirm is the ONLY write path, and worktree-path failures stay in-dialog
 *  and force a re-preview. IPC via vi.spyOn(mockIpc, …). */
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { ProfileActivateDialog } from './ProfileActivateDialog';
import { mockIpc } from '../ipc/mock';
import type { ProfileActivation, ProfilePreviewEntry } from '../ipc';

const PREVIEW: ProfilePreviewEntry[] = [
  { assetId: 'claude', path: 'CLAUDE.md', current: 'old text', proposed: 'new text', changed: true },
  { assetId: 'agents', path: 'AGENTS.md', current: null, proposed: 'fresh', changed: true },
  { assetId: 'gemini', path: 'GEMINI.md', current: 'same', proposed: 'same', changed: false },
];

const ACTIVATION: ProfileActivation = {
  profile: 'lean',
  results: [
    { assetId: 'claude', path: 'CLAUDE.md', action: 'written' },
    { assetId: 'agents', path: 'AGENTS.md', action: 'created' },
    { assetId: 'gemini', path: 'GEMINI.md', action: 'unchanged' },
  ],
  store: { version: 1, profiles: [], activeProfile: 'lean' },
};

function renderDialog(over: Partial<Parameters<typeof ProfileActivateDialog>[0]> = {}) {
  const props = {
    open: true,
    repoId: '/mock/repo',
    name: 'lean',
    onClose: vi.fn(),
    onActivated: vi.fn(),
    ...over,
  };
  return { ...render(<ProfileActivateDialog {...props} />), props };
}

beforeEach(() => vi.restoreAllMocks());

describe('ProfileActivateDialog', () => {
  it('closed or nameless renders nothing and never previews', () => {
    const spy = vi.spyOn(mockIpc, 'previewProfile');
    const { container } = renderDialog({ open: false });
    expect(container).toBeEmptyDOMElement();
    renderDialog({ name: null });
    expect(spy).not.toHaveBeenCalled();
  });

  it('Activate is disabled until the preview loads; entries get status chips', async () => {
    let settle!: (entries: ProfilePreviewEntry[]) => void;
    vi.spyOn(mockIpc, 'previewProfile').mockReturnValue(
      new Promise((resolve) => { settle = resolve; }),
    );
    renderDialog();
    const confirm = screen.getByRole('button', { name: 'Activate & write files' });
    expect(confirm).toBeDisabled();
    expect(screen.getByText('Loading preview…')).toBeInTheDocument();
    settle(PREVIEW);
    await vi.waitFor(() => expect(confirm).toBeEnabled());
    expect(screen.getByText('changed')).toBeInTheDocument();
    expect(screen.getByText('new file')).toBeInTheDocument();
    expect(screen.getByText('unchanged')).toBeInTheDocument();
    // Current-vs-proposed panes render the absent-file placeholder.
    expect(screen.getByText('No file — will be created')).toBeInTheDocument();
    expect(screen.getByText('old text')).toBeInTheDocument();
    expect(screen.getByText('new text')).toBeInTheDocument();
  });

  it('a profile with no targets says so', async () => {
    vi.spyOn(mockIpc, 'previewProfile').mockResolvedValue([]);
    renderDialog();
    expect(await screen.findByText('This profile has no targets.')).toBeInTheDocument();
  });

  it('confirm activates, then fires onActivated + onClose; cancel writes nothing', async () => {
    vi.spyOn(mockIpc, 'previewProfile').mockResolvedValue(PREVIEW);
    const activate = vi.spyOn(mockIpc, 'activateProfile').mockResolvedValue(ACTIVATION);
    const { props } = renderDialog();
    await vi.waitFor(() =>
      expect(screen.getByRole('button', { name: 'Activate & write files' })).toBeEnabled(),
    );
    fireEvent.click(screen.getByRole('button', { name: 'Cancel' }));
    expect(activate).not.toHaveBeenCalled();
    expect(props.onClose).toHaveBeenCalledTimes(1);
    fireEvent.click(screen.getByRole('button', { name: 'Activate & write files' }));
    await vi.waitFor(() => expect(props.onActivated).toHaveBeenCalledWith(ACTIVATION));
    expect(activate).toHaveBeenCalledWith('/mock/repo', 'lean');
    expect(props.onClose).toHaveBeenCalledTimes(2);
  });

  it('preview failure shows the error banner and keeps Activate disabled', async () => {
    vi.spyOn(mockIpc, 'previewProfile').mockRejectedValue({
      kind: 'other',
      message: 'profile not found',
    });
    renderDialog();
    expect(await screen.findByText('profile not found')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Activate & write files' })).toBeDisabled();
  });

  it('worktree path: preview/activate route to the worktree commands; failures stay in-dialog and force a re-preview', async () => {
    const preview = vi.spyOn(mockIpc, 'previewWorktreeProfile').mockResolvedValue(PREVIEW);
    const activate = vi.spyOn(mockIpc, 'activateWorktreeProfile').mockRejectedValue({
      kind: 'conflict',
      message: 'target file is dirty',
    });
    const { props } = renderDialog({ worktreeName: 'hotfix' });
    expect(
      screen.getByRole('dialog', { name: 'Activate lean in hotfix' }),
    ).toBeInTheDocument();
    await vi.waitFor(() => expect(preview).toHaveBeenCalledWith('/mock/repo', 'hotfix', 'lean'));
    const confirm = screen.getByRole('button', { name: 'Activate & write files' });
    await vi.waitFor(() => expect(confirm).toBeEnabled());
    fireEvent.click(confirm);
    expect(await screen.findByText('target file is dirty')).toBeInTheDocument();
    expect(activate).toHaveBeenCalledWith('/mock/repo', 'hotfix', 'lean');
    expect(props.onClose).not.toHaveBeenCalled(); // stays open
    // The re-preview is fired by the nonce-driven passive effect, which flushes
    // independently of the error banner that `findByText` awaits — assert the
    // eventual call count via waitFor, never synchronously (else it flakes under
    // parallel load when the banner's MutationObserver wins the scheduling race).
    await vi.waitFor(() => expect(preview).toHaveBeenCalledTimes(2)); // nonce-driven re-preview
  });

  it('Esc closes unless busy', () => {
    vi.spyOn(mockIpc, 'previewProfile').mockResolvedValue(PREVIEW);
    const { props } = renderDialog();
    fireEvent.keyDown(window, { key: 'Escape' });
    expect(props.onClose).toHaveBeenCalledTimes(1);
  });
});
