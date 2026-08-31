/** CloneDialog — clone-a-repo modal (P21 §6.1). Two safety-relevant properties
 *  are pinned here: (1) the URL→name→path derivation helpers, and (2) the
 *  backdrop-dismiss discipline — a stray backdrop click must NOT abandon a clone
 *  that is in flight (busy), while Esc / the Cancel button stay live as the
 *  deliberate stop path. */
import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { CloneDialog, deriveRepoName, joinRepoPath } from './CloneDialog';
import type { CloneProgress } from '../ipc';

function renderClone(over: Partial<Parameters<typeof CloneDialog>[0]> = {}) {
  const onPickDest = vi.fn();
  const onSubmit = vi.fn();
  const onCancel = vi.fn();
  const utils = render(
    <CloneDialog
      open
      busy={false}
      progress={null}
      error={null}
      dest={null}
      onPickDest={onPickDest}
      onSubmit={onSubmit}
      onCancel={onCancel}
      {...over}
    />,
  );
  return { ...utils, onPickDest, onSubmit, onCancel };
}

describe('deriveRepoName', () => {
  it('strips a trailing .git from an https URL', () => {
    expect(deriveRepoName('https://github.com/foo/bar.git')).toBe('bar');
  });
  it('parses an scp-style ssh URL', () => {
    expect(deriveRepoName('git@host:foo/bar.git')).toBe('bar');
  });
  it('ignores a trailing slash', () => {
    expect(deriveRepoName('https://github.com/foo/bar/')).toBe('bar');
    expect(deriveRepoName('https://github.com/foo/bar.git/')).toBe('bar');
  });
  it('keeps a name that has no .git suffix', () => {
    expect(deriveRepoName('https://github.com/foo/bar')).toBe('bar');
  });
  it("falls back to 'repository' for an all-dots or empty name", () => {
    expect(deriveRepoName('...')).toBe('repository');
    expect(deriveRepoName('.')).toBe('repository');
    expect(deriveRepoName('')).toBe('repository');
  });
});

describe('joinRepoPath', () => {
  it('joins a POSIX parent with a forward slash', () => {
    expect(joinRepoPath('/home/me', 'bar')).toBe('/home/me/bar');
  });
  it('joins a Windows parent with a backslash', () => {
    expect(joinRepoPath('C:\\Users\\me', 'bar')).toBe('C:\\Users\\me\\bar');
  });
  it('strips a trailing separator off the parent (both styles)', () => {
    expect(joinRepoPath('/home/me/', 'bar')).toBe('/home/me/bar');
    expect(joinRepoPath('C:\\Users\\me\\', 'bar')).toBe('C:\\Users\\me\\bar');
  });
});

describe('CloneDialog', () => {
  it('renders nothing when closed', () => {
    const { container } = renderClone({ open: false });
    expect(container).toBeEmptyDOMElement();
  });

  it('renders the dialog shell when open', () => {
    renderClone();
    expect(screen.getByRole('dialog', { name: 'Clone repository' })).toBeInTheDocument();
  });

  const clone = () => screen.getByRole('button', { name: 'Clone' });

  it('Clone is disabled with neither a url nor a dest', () => {
    renderClone();
    expect(clone()).toBeDisabled();
  });

  it('Clone stays disabled with a url but no dest', () => {
    renderClone();
    fireEvent.change(screen.getByRole('textbox'), { target: { value: 'https://h/foo/bar.git' } });
    expect(clone()).toBeDisabled();
  });

  it('Clone stays disabled with a dest but a blank url', () => {
    renderClone({ dest: '/home/me' });
    expect(screen.getByRole('textbox')).toHaveValue('');
    expect(clone()).toBeDisabled();
  });

  it('Clone enables once BOTH a url and a dest are present', () => {
    renderClone({ dest: '/home/me' });
    fireEvent.change(screen.getByRole('textbox'), { target: { value: 'https://h/foo/bar.git' } });
    expect(clone()).toBeEnabled();
  });

  it('submits the trimmed url when Clone is clicked', () => {
    const { onSubmit } = renderClone({ dest: '/home/me' });
    fireEvent.change(screen.getByRole('textbox'), { target: { value: '  https://h/foo/bar.git  ' } });
    fireEvent.click(screen.getByRole('button', { name: 'Clone' }));
    expect(onSubmit).toHaveBeenCalledTimes(1);
    expect(onSubmit).toHaveBeenCalledWith('https://h/foo/bar.git');
  });

  it('shows the derived final path once url + dest are set (and not before)', () => {
    const { rerender } = render(
      <CloneDialog
        open
        busy={false}
        progress={null}
        error={null}
        dest={null}
        onPickDest={vi.fn()}
        onSubmit={vi.fn()}
        onCancel={vi.fn()}
      />,
    );
    // No dest yet → no note.
    fireEvent.change(screen.getByRole('textbox'), { target: { value: 'https://h/foo/bar.git' } });
    expect(screen.queryByText(/Will clone into/)).not.toBeInTheDocument();

    rerender(
      <CloneDialog
        open
        busy={false}
        progress={null}
        error={null}
        dest="/home/me"
        onPickDest={vi.fn()}
        onSubmit={vi.fn()}
        onCancel={vi.fn()}
      />,
    );
    fireEvent.change(screen.getByRole('textbox'), { target: { value: 'https://h/foo/bar.git' } });
    expect(screen.getByText(/Will clone into/)).toBeInTheDocument();
    const finalPath = screen.getByText('/home/me/bar');
    expect(finalPath.tagName).toBe('STRONG');
  });

  it('renders a determinate progress bar while busy (receiving phase)', () => {
    const progress: CloneProgress = {
      receivedObjects: 50,
      totalObjects: 100,
      indexedDeltas: 0,
      totalDeltas: 0,
      receivedBytes: 2048,
    };
    const { container } = renderClone({ busy: true, progress });
    expect(screen.getByText('Receiving objects…')).toBeInTheDocument();
    expect(screen.getByText('50%')).toBeInTheDocument();
    expect(screen.getByText('2.0 KiB received')).toBeInTheDocument();
    expect(container.querySelector('progress.clone-progress-bar')).toBeInTheDocument();
    // While busy the primary button reads "Cloning…" and is disabled.
    expect(screen.getByRole('button', { name: 'Cloning…' })).toBeDisabled();
  });

  it('switches the phase label once deltas are resolving', () => {
    const progress: CloneProgress = {
      receivedObjects: 100,
      totalObjects: 100,
      indexedDeltas: 30,
      totalDeltas: 60,
      receivedBytes: 0,
    };
    renderClone({ busy: true, progress });
    expect(screen.getByText('Resolving deltas…')).toBeInTheDocument();
    expect(screen.getByText('50%')).toBeInTheDocument();
  });

  it('renders an inline error', () => {
    renderClone({ error: 'Authentication failed' });
    const err = screen.getByText('Authentication failed');
    expect(err).toHaveClass('dialog-error');
  });

  // ---- backdrop-dismiss discipline (the pinned fix) ----------------------

  it('backdrop mousedown cancels when NOT busy (target === currentTarget)', () => {
    const { container, onCancel } = renderClone();
    fireEvent.mouseDown(container.querySelector('.dialog-overlay')!);
    expect(onCancel).toHaveBeenCalledTimes(1);
  });

  it('backdrop mousedown does NOT cancel while busy (accidental click must not abandon a running clone)', () => {
    const { container, onCancel } = renderClone({ busy: true });
    fireEvent.mouseDown(container.querySelector('.dialog-overlay')!);
    expect(onCancel).not.toHaveBeenCalled();
  });

  it('a mousedown that starts inside the card does NOT cancel (drag-out guard)', () => {
    const { onCancel } = renderClone();
    // Firing on an element inside the card → e.target !== e.currentTarget (overlay).
    fireEvent.mouseDown(screen.getByText('Clone repository'));
    expect(onCancel).not.toHaveBeenCalled();
  });

  it('Esc cancels even while busy', () => {
    const { onCancel } = renderClone({ busy: true });
    fireEvent.keyDown(window, { key: 'Escape' });
    expect(onCancel).toHaveBeenCalledTimes(1);
  });

  it('the Cancel button cancels even while busy', () => {
    const { onCancel } = renderClone({ busy: true });
    fireEvent.click(screen.getByRole('button', { name: 'Cancel' }));
    expect(onCancel).toHaveBeenCalledTimes(1);
  });
});
