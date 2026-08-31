/** T3.5 — RepoHealthPanel: renders the four Section<T> envelopes independently
 *  from fixture health data, ≥-capped values, warn/ok chips, per-section error
 *  isolation, and Refresh wiring. IPC is stubbed via vi.spyOn(mockIpc, …). */
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, act, waitFor } from '@testing-library/react';
import { RepoHealthPanel } from './RepoHealthPanel';
import { mockIpc } from '../ipc/mock';
import type { RepoChangedPayload, RepoHealth, Section } from '../ipc';
import {
  __resetEchoSuppression,
  armEcho,
  clearEchoSuppression,
} from './repoWorkspace/echoSuppression';

function sec<T>(data: T, elapsedMs = 5): Section<T> {
  return { data, error: null, elapsedMs };
}

function health(over: Partial<RepoHealth> = {}): RepoHealth {
  return {
    generatedAt: Math.floor(Date.now() / 1000),
    stats: sec({
      commitCount: 20000,
      commitCountCapped: true,
      commitsLast30d: 42,
      authorsLast30d: 3,
      authorsTotal: 7,
      objectCount: 123456,
      objectScanCapped: false,
      largestBlobs: [{ oid: 'ab12cd34'.padEnd(40, '0'), size: 5 * 1024 * 1024 }],
      workdirFileCount: 321,
      workdirBytes: 10 * 1024 * 1024,
      workdirScanCapped: false,
      largestFiles: [{ path: 'assets/big.bin', size: 12 * 1024 * 1024 }],
      largeFileCount: 1,
      gitDirBytes: 2048,
      gitDirScanCapped: false,
    }),
    branches: sec({
      localCount: 4,
      remoteCount: 6,
      tagCount: 2,
      currentBranch: 'main',
      detached: false,
      unborn: false,
      ahead: 2,
      behind: 0,
      upstream: 'origin/main',
      stale: { base: 'main', mergedCount: 1, goneUpstreamCount: 0 },
      staleError: null,
    }),
    workingState: sec({
      staged: 1,
      unstaged: 2,
      untracked: 3,
      conflicted: 0,
      opState: { kind: 'merge', incoming: 'dev', message: 'Merge dev' },
      stashCount: 1,
      hasGitignore: true,
    }),
    structure: sec({
      submoduleCount: 0,
      submodulesUninitialized: 0,
      submodulesOutOfSync: 0,
      submodulesModified: 0,
      worktreeCount: 1,
      worktreesLocked: 0,
      worktreesPrunable: 0,
      worktreesInvalid: 0,
      assetDriftedCount: 2,
      assetsInSync: false,
    }),
    ...over,
  };
}

beforeEach(() => {
  vi.restoreAllMocks();
  __resetEchoSuppression();
  vi.spyOn(mockIpc, 'onRepoChanged').mockResolvedValue(() => {});
});

describe('RepoHealthPanel', () => {
  it('renders nothing while closed (and never fetches)', () => {
    const spy = vi.spyOn(mockIpc, 'getRepoHealth').mockResolvedValue(health());
    const { container } = render(
      <RepoHealthPanel open={false} onClose={vi.fn()} repoId="/mock/repo" />,
    );
    expect(container).toBeEmptyDOMElement();
    expect(spy).not.toHaveBeenCalled();
  });

  it('fetches on open and renders all four sections with elapsed times', async () => {
    vi.spyOn(mockIpc, 'getRepoHealth').mockResolvedValue(health());
    render(<RepoHealthPanel open onClose={vi.fn()} repoId="/mock/repo" />);
    expect(await screen.findByText('Commits (HEAD)')).toBeInTheDocument();
    // Capped commit count renders the ≥ floor + chip.
    expect(screen.getByText('≥ 20000')).toBeInTheDocument();
    expect(screen.getAllByText('capped').length).toBeGreaterThan(0);
    // Branches: current branch + ahead chip ('main' also appears as stale base).
    expect(screen.getAllByText('main').length).toBeGreaterThanOrEqual(1);
    expect(screen.getByText('↑2 ahead')).toBeInTheDocument();
    // Working state: op label + warn/ok chips.
    expect(screen.getByText('merge in progress (dev)')).toBeInTheDocument();
    expect(screen.getByText('0 conflicted')).toHaveClass('asset-chip-sync');
    expect(screen.getByText('present')).toBeInTheDocument();
    // Structure: drifted AI assets warn badge.
    expect(screen.getByText('2 files drifted')).toBeInTheDocument();
    // Stats extras: largest files/blobs lists.
    expect(screen.getByText('assets/big.bin')).toBeInTheDocument();
    expect(screen.getByText('blob ab12cd3')).toBeInTheDocument();
    expect(screen.getByText('1 large')).toHaveClass('asset-chip-drifted');
    expect(screen.getAllByText('5 ms')).toHaveLength(4);
  });

  it('a section error renders inline while sibling sections still render', async () => {
    vi.spyOn(mockIpc, 'getRepoHealth').mockResolvedValue(
      health({ stats: { data: null, error: 'stats scan failed', elapsedMs: 9 } }),
    );
    render(<RepoHealthPanel open onClose={vi.fn()} repoId="/mock/repo" />);
    expect(await screen.findByText('stats scan failed')).toBeInTheDocument();
    expect(screen.getByText('Local branches')).toBeInTheDocument(); // sibling data intact
  });

  it('a whole-fetch failure shows the top-level error banner', async () => {
    vi.spyOn(mockIpc, 'getRepoHealth').mockRejectedValue({
      kind: 'other',
      message: 'repo gone',
    });
    render(<RepoHealthPanel open onClose={vi.fn()} repoId="/mock/repo" />);
    expect(await screen.findByText('repo gone')).toBeInTheDocument();
  });

  it('Refresh refetches; the button disables while loading', async () => {
    const spy = vi.spyOn(mockIpc, 'getRepoHealth').mockResolvedValue(health());
    render(<RepoHealthPanel open onClose={vi.fn()} repoId="/mock/repo" />);
    await screen.findByText('Commits (HEAD)');
    expect(spy).toHaveBeenCalledTimes(1);
    let settle!: (h: RepoHealth) => void;
    spy.mockReturnValue(new Promise((resolve) => { settle = resolve; }));
    fireEvent.click(screen.getByRole('button', { name: 'Refresh' }));
    expect(spy).toHaveBeenCalledTimes(2);
    expect(screen.getByRole('button', { name: 'Refreshing…' })).toBeDisabled();
    settle(health());
    expect(await screen.findByRole('button', { name: 'Refresh' })).toBeEnabled();
  });

  it('P81 (AC8): drops the self-echo repo-changed within the window, refetches after it', async () => {
    const spy = vi.spyOn(mockIpc, 'getRepoHealth').mockResolvedValue(health());
    let handler: ((p: RepoChangedPayload) => void) | null = null;
    vi.spyOn(mockIpc, 'onRepoChanged').mockImplementation((cb) => {
      handler = cb;
      return Promise.resolve(() => {});
    });
    render(<RepoHealthPanel open onClose={vi.fn()} repoId="/mock/repo" />);
    await screen.findByText('Commits (HEAD)');
    expect(spy).toHaveBeenCalledTimes(1);
    expect(handler).toBeTruthy();

    // Armed window active → the self-caused echo is a no-op.
    armEcho('/mock/repo');
    await act(async () => {
      handler?.({ repoId: '/mock/repo', reason: 'test' });
    });
    expect(spy).toHaveBeenCalledTimes(1);

    // A different repo's echo is never our concern regardless.
    await act(async () => {
      handler?.({ repoId: '/other', reason: 'test' });
    });
    expect(spy).toHaveBeenCalledTimes(1);

    // Span closed (P85 A2: the round settled + tail elapsed) → a genuine
    // external change refetches. clearEchoSuppression models "no longer suppressed".
    clearEchoSuppression('/mock/repo');
    await act(async () => {
      handler?.({ repoId: '/mock/repo', reason: 'test' });
    });
    await waitFor(() => expect(spy).toHaveBeenCalledTimes(2));
  });

  it('close button and backdrop call onClose', async () => {
    vi.spyOn(mockIpc, 'getRepoHealth').mockResolvedValue(health());
    const onClose = vi.fn();
    const { container } = render(
      <RepoHealthPanel open onClose={onClose} repoId="/mock/repo" />,
    );
    await screen.findByText('Commits (HEAD)');
    fireEvent.click(screen.getByRole('button', { name: 'Close' }));
    expect(onClose).toHaveBeenCalledTimes(1);
    fireEvent.mouseDown(container.querySelector('.dialog-overlay')!);
    expect(onClose).toHaveBeenCalledTimes(2);
  });
});
