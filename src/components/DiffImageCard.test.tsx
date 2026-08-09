/** T3.5 — DiffImageCard: the commit/compare image card's own local fetch
 *  (request shape per source mode), loading/error/retry states, and the
 *  compare-mode switcher. IPC via vi.spyOn(mockIpc, …). */
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { DiffImageCard } from './DiffImageCard';
import { mockIpc } from '../ipc/mock';
import type { FileDiffHeader, ImageDiff } from '../ipc';

const PNG_B64 = 'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNkYPhfDwAChwGA60e6kgAAAABJRU5ErkJggg==';

const HEADER: FileDiffHeader = {
  path: 'logo.png',
  origPath: null,
  status: 'modified',
  additions: 0,
  deletions: 0,
  binary: true,
};

const DIFF: ImageDiff = {
  path: 'logo.png',
  old: { base64: PNG_B64, mime: 'image/png', byteLen: 67 },
  new: { base64: PNG_B64, mime: 'image/png', byteLen: 67 },
  oldTooLarge: false,
  newTooLarge: false,
};

beforeEach(() => vi.restoreAllMocks());

describe('DiffImageCard', () => {
  it('commit source fetches with a kind:"commit" request and renders side-by-side', async () => {
    const spy = vi.spyOn(mockIpc, 'getImageDiff').mockResolvedValue(DIFF);
    render(
      <DiffImageCard repoId="/mock/repo" source={{ mode: 'commit', oid: 'abc123', title: 't' }} header={HEADER} />,
    );
    expect(spy).toHaveBeenCalledWith('/mock/repo', {
      kind: 'commit',
      oid: 'abc123',
      path: 'logo.png',
      origPath: null,
    });
    expect(await screen.findByAltText('Old version')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Side by side' })).toHaveAttribute('aria-pressed', 'true');
  });

  it('compare source fetches with kind:"compare" (toOid); mode switcher flips views', async () => {
    const spy = vi.spyOn(mockIpc, 'getImageDiff').mockResolvedValue(DIFF);
    const { container } = render(
      <DiffImageCard repoId="/mock/repo" source={{ mode: 'compare', oid: 'def456', fromLabel: 'a', toLabel: 'b' }} header={HEADER} />,
    );
    expect(spy).toHaveBeenCalledWith('/mock/repo', {
      kind: 'compare',
      toOid: 'def456',
      path: 'logo.png',
      origPath: null,
    });
    await screen.findByAltText('Old version');
    fireEvent.click(screen.getByRole('button', { name: 'Onion' }));
    expect(container.querySelector('.img-diff-onion')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'Swipe' }));
    expect(container.querySelector('.img-swipe-stage')).toBeInTheDocument();
  });

  it('shows the skeleton while loading, then the error banner with a working Retry', async () => {
    const spy = vi
      .spyOn(mockIpc, 'getImageDiff')
      .mockRejectedValueOnce({ kind: 'other', message: 'blob read failed' })
      .mockResolvedValueOnce(DIFF);
    const { container } = render(
      <DiffImageCard repoId="/mock/repo" source={{ mode: 'commit', oid: 'abc', title: 't' }} header={HEADER} />,
    );
    expect(container.querySelector('.skeleton-group')).toBeInTheDocument();
    expect(await screen.findByText('blob read failed')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'Retry' }));
    expect(await screen.findByAltText('Old version')).toBeInTheDocument();
    expect(spy).toHaveBeenCalledTimes(2);
  });
});
