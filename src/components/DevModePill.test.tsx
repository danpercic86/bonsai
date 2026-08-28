/**
 * P91 §8.4 — the Dev-mode header pill swaps to its danger variant ("Not logging",
 * triangle glyph) when the sink reports `writeFailed`, and shows the healthy "Dev
 * mode" label otherwise. The pill must never claim to record when it is not.
 */
import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, render, screen, waitFor } from '@testing-library/react';

import { DevModePill } from './DevModePill';
import { DevModeIndicator } from './DevModeIndicator';
import { mockIpc } from '../ipc/mock';
import type { LogSessionInfo } from '../ipc';

afterEach(cleanup);

function sessionInfo(over: Partial<LogSessionInfo> = {}): LogSessionInfo {
  return {
    sessionId: 's1',
    dir: '/logs',
    files: ['bonsai-a.jsonl'],
    bytes: 10,
    records: 1,
    anomalies: 0,
    dropped: 0,
    redaction: 'strict',
    salt: '',
    totalFiles: 1,
    totalBytes: 10,
    droppedParts: 0,
    writeFailed: false,
    exportFiles: 0,
    exportBytes: 0,
    ...over,
  };
}

describe('DevModeIndicator', () => {
  it('shows the healthy "Dev mode" label by default', () => {
    render(<DevModeIndicator onOpen={() => {}} />);
    expect(screen.getByText('Dev mode')).toBeInTheDocument();
    expect(screen.queryByText('Not logging')).toBeNull();
  });

  it('swaps to the danger "Not logging" variant when writeFailed', () => {
    const { container } = render(<DevModeIndicator onOpen={() => {}} writeFailed />);
    expect(screen.getByText('Not logging')).toBeInTheDocument();
    expect(screen.queryByText('Dev mode')).toBeNull();
    expect(container.querySelector('.dev-mode-pill.is-write-failed')).not.toBeNull();
    expect(screen.getByRole('button').getAttribute('aria-label')).toMatch(
      /stopped writing the log/,
    );
  });
});

describe('DevModePill', () => {
  it('lights the danger variant once logSessionInfo reports writeFailed', async () => {
    vi.spyOn(mockIpc, 'logSessionInfo').mockResolvedValue(sessionInfo({ writeFailed: true }));
    render(<DevModePill onOpen={() => {}} />);
    await waitFor(() => expect(screen.getByText('Not logging')).toBeInTheDocument());
  });

  it('stays healthy when logSessionInfo reports no failure', async () => {
    const spy = vi
      .spyOn(mockIpc, 'logSessionInfo')
      .mockResolvedValue(sessionInfo({ writeFailed: false }));
    render(<DevModePill onOpen={() => {}} />);
    await waitFor(() => expect(spy).toHaveBeenCalled());
    expect(screen.getByText('Dev mode')).toBeInTheDocument();
    expect(screen.queryByText('Not logging')).toBeNull();
  });
});
