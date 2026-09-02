/** P38 follow-up — the CALL SITE of the repo-went-unusable teardown.
 *
 *  `unusableRepoTeardown.test.tsx` proves the helper empties everything. It
 *  cannot prove the container still CALLS it: deleting the whole
 *  `tearDownUnusableRepo({...})` block from `runRefreshRound` left that suite
 *  green. This suite mounts the real app (mock IPC, one persisted tab), lets the
 *  workspace settle, opens the commit-search bar, then drives a `full`-scope
 *  refresh with `openRepo` answering "not a repo any more" — the real
 *  went-unusable path — and asserts:
 *    1. the container reached `tearDownUnusableRepo`,
 *    2. every close mirror was WIRED (non-null) at that moment, so no overlay
 *       would silently linger, and
 *    3. observably, the open search bar is gone (state-rendered, so it survives
 *       `clearGraph()` — only the teardown closes it).
 *
 *  The teardown module is spied through `importActual`, so the REAL
 *  implementation still runs: the spy observes, it does not stub. */
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';

import App from '../../App';
import { DEFAULT_UI_SETTINGS } from '../../ipc/mock/persistence';
import { mockIpc } from '../../ipc/mock';
import { tearDownUnusableRepo } from './unusableRepoTeardown';
import type { OpenRepoResult, SessionState, UiSettings } from '../../ipc';
import type { UnusableRepoTeardownDeps } from './unusableRepoTeardown';

vi.mock('./unusableRepoTeardown', async (importOriginal) => {
  const actual = await importOriginal<typeof import('./unusableRepoTeardown')>();
  return { ...actual, tearDownUnusableRepo: vi.fn(actual.tearDownUnusableRepo) };
});

const REPO = '/mock/repo';

function ui(over: Partial<UiSettings> = {}): UiSettings {
  return { ...structuredClone(DEFAULT_UI_SETTINGS), onboardingSeen: true, ...over };
}

const SESSION: SessionState = { openRepos: [REPO], activeRepo: REPO };

/** The `full`-scope usability verdict: a path that is no longer a repo. */
const GONE: OpenRepoResult = {
  repoId: REPO,
  info: { path: REPO, isRepo: false, bare: false, head: null },
};

beforeEach(() => {
  vi.restoreAllMocks();
  vi.mocked(tearDownUnusableRepo).mockClear();
  vi.spyOn(mockIpc, 'getUiSettings').mockResolvedValue(ui());
  vi.spyOn(mockIpc, 'getRecentRepos').mockResolvedValue([]);
  vi.spyOn(mockIpc, 'getSession').mockResolvedValue(SESSION);
  vi.spyOn(mockIpc, 'setUiSettings').mockResolvedValue(undefined as never);
  vi.spyOn(mockIpc, 'setSession').mockResolvedValue(undefined as never);
});

describe('runRefreshRound — the repo went unusable while open', () => {
  it('tears down through tearDownUnusableRepo with every close mirror wired', async () => {
    render(<App />);

    // The tab mounts and the graph settles (the search affordance is gated on a
    // loaded layout — it is also the proof the repo opened NORMALLY first).
    const fab = await screen.findByRole('button', { name: 'Search commits' }, { timeout: 10_000 });
    fireEvent.click(fab);
    expect(screen.getByRole('search')).toBeInTheDocument();

    // Now the repo disappears underneath us. The manual refresh is a `full`-scope
    // round, the only scope that re-openRepos for the usability check.
    vi.spyOn(mockIpc, 'openRepo').mockResolvedValue(GONE);
    const refresh = await screen.findByRole('button', { name: 'Refresh' });
    await waitFor(() => expect(refresh).toBeEnabled());
    fireEvent.click(refresh);

    await waitFor(() => expect(tearDownUnusableRepo).toHaveBeenCalled());

    // Every mirror wired at call time — an unwired one would have thrown inside
    // the helper (surfacing a `Refresh failed: …` toast) and leaked its overlay.
    const deps: UnusableRepoTeardownDeps = vi.mocked(tearDownUnusableRepo).mock.calls.at(-1)![0];
    expect(deps.historySearchCloseRef.current).toBeTypeOf('function');
    expect(deps.commitSearchCloseRef.current).toBeTypeOf('function');
    expect(deps.replayExitRef.current).toBeTypeOf('function');

    // …and the observable consequence: the open search bar is closed.
    await waitFor(() => expect(screen.queryByRole('search')).not.toBeInTheDocument());
    // The refresh did not fail on the way (an unwired mirror would toast this).
    expect(screen.queryByText(/Refresh failed/)).not.toBeInTheDocument();
    // Generous timeout: a whole-App mount under a loaded CI machine outruns
    // vitest's 5 s default (cf. Sidebar.churn.test.tsx).
  }, 20_000);
});
