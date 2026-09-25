/** P121e test 1 — a tag-sync RE-check renders the memoised Tags section body
 *  once (when the new report lands), not twice (badges off, badges on). The
 *  in-flight flag reaches only the rollup leaf via `TagSyncCheckingContext`.
 *  Non-StrictMode; real `useTagSync` over a deferred mock `listTagSync`. */
import { useCallback, useEffect } from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { act, cleanup, fireEvent, render, screen } from '@testing-library/react';

import { mockIpc } from '../../ipc/mock';
import type { RemoteInfo, TagSyncReport } from '../../ipc';
import { useTagSync } from '../repoWorkspace/useTagSync';
import type { UseTagSync } from '../repoWorkspace/useTagSync';
import { TagsSection } from './TagsSection';
import { TagSyncCheckingContext } from './tagSyncCheckingContext';
import {
  drainRenderTally,
  resetRenderTally,
  startRenderTally,
  stopRenderTally,
  tallyRenders,
} from '../../test/renderTallyKit';

const ORIGIN: RemoteInfo[] = [{ name: 'origin', url: 'https://example.invalid/o.git' }];
const TAGS = ['v1.0', 'v2.0'];
const A = 'a'.repeat(40);
const B = 'b'.repeat(40);

function report(staleTag: string): TagSyncReport {
  return {
    remote: 'origin',
    entries: TAGS.map((name) => ({
      name,
      status: name === staleTag ? 'stale' : 'in-sync',
      localOid: A,
      remoteOid: name === staleTag ? B : A,
      annotated: false,
    })),
  };
}

function deferred<T>() {
  let resolve!: (v: T) => void;
  const promise = new Promise<T>((res) => {
    resolve = res;
  });
  return { promise, resolve };
}

const noop = () => {};
let hook!: UseTagSync;

function Harness() {
  const ts = useTagSync('repo-1', ORIGIN);
  useEffect(() => {
    hook = ts;
  });
  const { refetch } = ts;
  // Stable, like the container's `onTagsExpand` — the section is memoised.
  // NON-forced: a forced call from `idle` is a no-op by design.
  const onExpand = useCallback(() => void refetch(), [refetch]);
  return (
    <TagSyncCheckingContext.Provider value={ts.checking}>
      <TagsSection
        tags={TAGS}
        treeMode={false}
        onTagContextMenu={noop}
        tagSyncReport={ts.report}
        tagSyncState={ts.state}
        tagSyncRemote={ts.remote}
        tagSyncCheckedAt={ts.checkedAt}
        onExpand={onExpand}
      />
    </TagSyncCheckingContext.Provider>
  );
}

/** The row `<li>` for `tag` (its name span carries `title={name}`). */
function row(tag: string): HTMLElement {
  const li = screen.getByTitle(tag).closest('li');
  if (li === null) throw new Error(`no row for ${tag}`);
  return li;
}

beforeEach(() => startRenderTally());
afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
  stopRenderTally();
});

describe('TagsSection re-check (P121e, non-StrictMode)', () => {
  it('renders the body once per re-check and keeps the previous badges while pending', async () => {
    const first = deferred<TagSyncReport>();
    const second = deferred<TagSyncReport>();
    const spy = vi
      .spyOn(mockIpc, 'listTagSync')
      .mockReturnValueOnce(first.promise)
      .mockReturnValueOnce(second.promise);
    render(<Harness />);

    // Expanding fires the first (non-forced) check.
    fireEvent.click(screen.getByRole('treeitem', { name: /Tags/ }));
    expect(spy).toHaveBeenCalledTimes(1);
    expect(screen.getByText('checking…')).toBeInTheDocument();
    await act(async () => {
      first.resolve(report('v1.0'));
      await first.promise;
    });
    expect(screen.queryByText('checking…')).toBeNull();
    expect(row('v1.0').textContent).toContain('out of sync');

    await drainRenderTally();
    resetRenderTally();

    // Re-check (forced, like a focus rescan / auto-fetch).
    let pending!: Promise<void>;
    act(() => {
      pending = hook.refetch({ force: true });
    });
    expect(spy).toHaveBeenCalledTimes(2);
    // While pending (P123): the rollup KEEPS its verdict pill + the re-check
    // cue — no `checking…` swap — and the previous row badges REMAIN.
    expect(screen.queryByText('checking…')).toBeNull();
    const rechecking = screen.getByRole('img', {
      name: '1 tag out of sync on origin, checking again',
    });
    expect(rechecking).toHaveAttribute('aria-busy', 'true');
    expect(rechecking).toHaveAttribute('title', '1 tag differs from origin — checking again…');
    expect(row('v1.0').textContent).toContain('out of sync');

    await act(async () => {
      second.resolve(report('v2.0'));
      await pending;
    });
    expect(screen.queryByText('checking…')).toBeNull();
    const settled = screen.getByRole('img', { name: '1 tag out of sync on origin' });
    expect(settled).not.toHaveAttribute('aria-busy');
    expect(settled).toHaveAttribute('title', '1 tag differs from origin. Expand to resolve.');
    expect(row('v1.0').textContent).not.toContain('out of sync');
    expect(row('v2.0').textContent).toContain('out of sync');

    await drainRenderTally();
    expect(tallyRenders('TagsSection')).toBe(1);
  });

  // Obs OFF for this case: with it on, the ipc proxy hands back the wrapper it
  // memoised around the FIRST test's `listTagSync` spy — `resetObsConfigForTests`
  // clears the listener that would drop that memo — so a second spy is unseen.
  // This case needs no render tally.
  it('a re-check of a clean (0-divergence) verdict renders no pill at any point', async () => {
    stopRenderTally();
    const first = deferred<TagSyncReport>();
    const second = deferred<TagSyncReport>();
    const spy = vi
      .spyOn(mockIpc, 'listTagSync')
      .mockReturnValueOnce(first.promise)
      .mockReturnValueOnce(second.promise);
    render(<Harness />);
    fireEvent.click(screen.getByRole('treeitem', { name: /Tags/ }));
    expect(spy).toHaveBeenCalledTimes(1);
    await act(async () => {
      first.resolve(report('none'));
      await first.promise;
    });
    const noPill = () => {
      expect(screen.queryByText('checking…')).toBeNull();
      expect(screen.queryByRole('img', { name: /out of sync/ })).toBeNull();
    };
    noPill();
    let pending!: Promise<void>;
    act(() => {
      pending = hook.refetch({ force: true });
    });
    noPill();
    await act(async () => {
      second.resolve(report('none'));
      await pending;
    });
    noPill();
  });
});
