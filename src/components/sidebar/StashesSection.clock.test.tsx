/** P121c test 3 — a shared-clock tick re-renders ONLY the `RelativeAge` leaf:
 *  the (unmemoised) Stashes section and its memoised rows stay put, yet the age
 *  badge still advances. StrictMode (`wrapper: StrictMode`), fake timers; row
 *  renders are read from the obs `render.tally` sink (`test/renderTallyKit`). */
import { StrictMode } from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { act, cleanup, render, screen } from '@testing-library/react';

import type { StashEntry } from '../../ipc';
import { StashesSection } from './StashesSection';
import type { StashesSectionProps } from './StashesSection';
import { __resetNowForTests } from '../../hooks/useNow';
import {
  drainRenderTally,
  resetRenderTally,
  startRenderTally,
  stopRenderTally,
  tallyRenders,
} from '../../test/renderTallyKit';

const T0 = 1_700_000_010_000; // an exact 30 s boundary
const T0_SEC = T0 / 1000;

const STASHES: StashEntry[] = [0, 1, 2].map((i) => ({
  index: i,
  oid: String(i).repeat(40),
  baseOid: 'f'.repeat(40),
  message: `WIP ${i}`,
  ts: T0_SEC - 60 - i * 3600, // "1m", "1h", "2h"
}));

const noop = () => {};
const PROPS: StashesSectionProps = {
  stashes: STASHES,
  collapsed: false,
  setCollapsed: noop,
  headUnborn: false,
  onCreateStash: noop,
  onContextMenu: noop,
};

beforeEach(() => {
  vi.useFakeTimers();
  vi.setSystemTime(T0);
  __resetNowForTests();
  startRenderTally();
});

afterEach(() => {
  cleanup();
  __resetNowForTests();
  vi.useRealTimers();
  stopRenderTally();
});

describe('StashesSection clock tick (P121c, StrictMode)', () => {
  it('advances the age badge without re-rendering the section or its rows', async () => {
    let sectionRenders = 0;
    function Host() {
      sectionRenders += 1; // the section is unmemoised: it renders iff this does
      return <StashesSection {...PROPS} />;
    }
    render(<Host />, { wrapper: StrictMode });
    expect(screen.getByText('1m')).toBeInTheDocument();
    expect(tallyRenders('StashRow')).toBe(0); // nothing flushed yet
    await drainRenderTally();
    expect(tallyRenders('StashRow')).toBeGreaterThan(0); // the probe is live
    resetRenderTally();
    const mounted = sectionRenders;

    await act(async () => {
      vi.advanceTimersByTime(60_000);
    });
    await drainRenderTally();

    expect(tallyRenders('StashRow')).toBe(0);
    expect(sectionRenders).toBe(mounted);
    expect(screen.getByText('2m')).toBeInTheDocument();
    expect(screen.queryByText('1m')).toBeNull();
  });
});
