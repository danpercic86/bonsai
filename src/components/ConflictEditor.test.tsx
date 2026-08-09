/** T3.7 — ConflictEditor React wiring (CodeMirror is mounted, but these assert
 *  the React shell that RepoWorkspace depends on: mode toggle, Stage-resolved
 *  gating on unresolved markers, onResolve/onCancel wiring, inline save error).
 *  The doc-editing / region-widget behavior is covered separately against the
 *  pure conflictRegions helpers + conflictCmExtensions. */
import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import ConflictEditor from './ConflictEditor';
import type { ConflictFile } from '../ipc';

const CLEAN = 'line a\nline b\n'; // no markers → resolved
const MARKED = ['top', '<<<<<<< HEAD', 'ours', '=======', 'theirs', '>>>>>>> b', 'end'].join('\n');

function file(over: Partial<ConflictFile> = {}): ConflictFile {
  return {
    path: 'src/x.ts',
    kind: 'bothModified',
    binary: false,
    tooLarge: false,
    missing: false,
    text: CLEAN,
    ours: 'line a\nline b\n',
    theirs: 'line a\nline c\n',
    ...over,
  };
}

describe('ConflictEditor', () => {
  it('renders the header controls and mounts the editor host', () => {
    render(<ConflictEditor file={file()} onResolve={vi.fn(async () => {})} onCancel={vi.fn()} mutating={false} />);
    expect(screen.getByTestId('conflict-editor')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Unified' })).toHaveAttribute('aria-pressed', 'true');
    expect(screen.getByRole('button', { name: 'Side-by-side' })).toHaveAttribute(
      'aria-pressed',
      'false',
    );
  });

  it('Stage-resolved is enabled for a marker-free file and calls onResolve with the text', () => {
    const onResolve = vi.fn(async () => {});
    render(
      <ConflictEditor file={file({ text: CLEAN })} onResolve={onResolve} onCancel={vi.fn()} mutating={false} />,
    );
    const stage = screen.getByRole('button', { name: 'Stage resolved' });
    expect(stage).toBeEnabled();
    fireEvent.click(stage);
    expect(onResolve).toHaveBeenCalledTimes(1);
    expect(onResolve).toHaveBeenCalledWith('src/x.ts', CLEAN);
  });

  it('Stage-resolved is disabled while unresolved markers remain', () => {
    render(
      <ConflictEditor file={file({ text: MARKED })} onResolve={vi.fn(async () => {})} onCancel={vi.fn()} mutating={false} />,
    );
    expect(screen.getByRole('button', { name: 'Stage resolved' })).toBeDisabled();
  });

  it('Stage-resolved is disabled while mutating even when resolved', () => {
    render(
      <ConflictEditor file={file({ text: CLEAN })} onResolve={vi.fn(async () => {})} onCancel={vi.fn()} mutating />,
    );
    expect(screen.getByRole('button', { name: 'Stage resolved' })).toBeDisabled();
  });

  it('Cancel fires onCancel', () => {
    const onCancel = vi.fn();
    render(<ConflictEditor file={file()} onResolve={vi.fn(async () => {})} onCancel={onCancel} mutating={false} />);
    fireEvent.click(screen.getByRole('button', { name: 'Cancel' }));
    expect(onCancel).toHaveBeenCalledTimes(1);
  });

  it('shows an inline, dismissible error when onResolve rejects', async () => {
    const onResolve = vi.fn(async () => {
      throw new Error('index locked');
    });
    render(
      <ConflictEditor file={file({ text: CLEAN })} onResolve={onResolve} onCancel={vi.fn()} mutating={false} />,
    );
    fireEvent.click(screen.getByRole('button', { name: 'Stage resolved' }));
    await waitFor(() => expect(screen.getByRole('alert')).toHaveTextContent('index locked'));
    fireEvent.click(screen.getByRole('button', { name: 'Dismiss error' }));
    expect(screen.queryByRole('alert')).not.toBeInTheDocument();
  });

  it('the Side-by-side toggle switches the active mode', () => {
    render(<ConflictEditor file={file()} onResolve={vi.fn(async () => {})} onCancel={vi.fn()} mutating={false} />);
    fireEvent.click(screen.getByRole('button', { name: 'Side-by-side' }));
    expect(screen.getByRole('button', { name: 'Side-by-side' })).toHaveAttribute(
      'aria-pressed',
      'true',
    );
    // Split-mode column labels appear.
    expect(screen.getByText('Ours')).toBeInTheDocument();
  });
});
