/**
 * P68f — the confirm gate in front of "Resolve all with AI".
 *
 * What the copy must state, because the whole point of the gate is informed consent:
 * the file COUNT, that it is ONE AI run, that it costs Claude quota, and — the branch
 * that actually differs — whether marker-free results will be staged automatically.
 */
import { describe, expect, it, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';

import { BulkAiConfirmDialog } from './BulkAiConfirmDialog';
import type { BulkAiConfirmState } from '../repoWorkspace/useBulkAiResolve';

const PATHS = ['src/auth.ts', 'src/locales/de.json'];

function renderDialog(over: Partial<BulkAiConfirmState> = {}) {
  const props: BulkAiConfirmState = {
    open: true,
    paths: PATHS,
    autonomy: 'proposeReview',
    onConfirm: vi.fn(),
    onCancel: vi.fn(),
    ...over,
  };
  render(<BulkAiConfirmDialog {...props} />);
  return props;
}

describe('BulkAiConfirmDialog', () => {
  it('renders nothing while closed', () => {
    renderDialog({ open: false });
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
  });

  it('states the count, the ONE run, the spend, and lists the files', () => {
    renderDialog();
    const dialog = screen.getByRole('dialog', { name: 'Resolve all conflicts with AI' });
    expect(dialog).toHaveTextContent('Send 2 conflicted files to Claude in one AI run');
    expect(dialog).toHaveTextContent('uses your Claude quota');
    for (const path of PATHS) expect(dialog).toHaveTextContent(path);
  });

  it('proposeReview promises nothing is written; autoResolve says what IS staged', () => {
    const { unmount } = render(
      <BulkAiConfirmDialog
        open
        paths={PATHS}
        autonomy="proposeReview"
        onConfirm={vi.fn()}
        onCancel={vi.fn()}
      />,
    );
    expect(screen.getByRole('dialog')).toHaveTextContent(
      'Nothing is written to your files: each result is a proposal you review before it is staged.',
    );
    unmount();

    renderDialog({ autonomy: 'autoResolve' });
    const dialog = screen.getByRole('dialog');
    expect(dialog).toHaveTextContent('Marker-free results are staged automatically');
    // THE SAFETY GATE, stated to the user before they agree: markerful ⇒ review.
    expect(dialog).toHaveTextContent(
      'Anything that still contains conflict markers is opened for review instead — never staged.',
    );
  });

  it('lists at most ten paths and counts the rest', () => {
    const many = Array.from({ length: 14 }, (_, i) => `src/locales/l${i}.json`);
    renderDialog({ paths: many });
    expect(document.querySelectorAll('.confirm-name-list li.mono')).toHaveLength(10);
    expect(screen.getByRole('dialog')).toHaveTextContent('+4 more');
  });

  it('Confirm and Cancel call through', () => {
    const props = renderDialog();
    fireEvent.click(screen.getByRole('button', { name: 'Resolve all with AI' }));
    expect(props.onConfirm).toHaveBeenCalledTimes(1);
    fireEvent.click(screen.getByRole('button', { name: 'Cancel' }));
    expect(props.onCancel).toHaveBeenCalledTimes(1);
  });
});
