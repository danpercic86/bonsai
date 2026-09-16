/**
 * P91 §12 row 4 — React causality hooks (obs/react.ts + renderTally.ts).
 *
 * Covers acceptance (c) effect-no-change, (e) flicker tally, the aggregate
 * per-window collapse that underpins (d), and the reserved `useStateTransitionLog`.
 */
import { StrictMode, useState } from 'react';
import { act, render, cleanup } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { configureObs, resetObsConfigForTests } from './enabled';
import { attachSink, flushNow, resetBatcherForTests } from './batcher';
import { clearSessionSalt, setSessionSalt } from './redact';
import { useRenderCount, useTracedEffect, useStateTransitionLog } from './react';
import {
  RENDER_TALLY_WINDOW_MS,
  __resetRenderTally,
  flushRenderTally,
} from './renderTally';
import type { DevSettings } from '../ipc/types/settings';
import type { LogRecord } from './types';

const DEV_TRACE: DevSettings = {
  enabled: true,
  level: 'trace',
  captureIpc: true,
  captureReact: true,
  captureFrames: true,
  includeRawNames: false,
};

let sunk: LogRecord[] = [];

function attachCapture(): void {
  attachSink({
    async logAppend(records) {
      sunk.push(...records);
    },
    async logSessionInfo() {
      return { salt: '00112233445566778899aabbccddeeff' };
    },
  });
  setSessionSalt('00112233445566778899aabbccddeeff');
}

async function drain(): Promise<void> {
  flushRenderTally();
  await flushNow();
}

beforeEach(() => {
  sunk = [];
  resetObsConfigForTests();
  resetBatcherForTests();
  __resetRenderTally();
  clearSessionSalt();
  attachCapture();
  configureObs(DEV_TRACE);
});

afterEach(() => {
  cleanup();
  vi.useRealTimers();
  resetObsConfigForTests();
  resetBatcherForTests();
  __resetRenderTally();
  clearSessionSalt();
});

const byKind = (k: string) => sunk.filter((r) => r.kind === k);

describe('useTracedEffect — acceptance (c)', () => {
  function EffectHost({ dep }: { dep: number }) {
    useTracedEffect('Host', 'sub', () => undefined, [dep], ['dep']);
    return null;
  }

  it('a re-run with unchanged deps emits changedDeps: [] (effect-no-change)', async () => {
    // StrictMode double-invokes the mount effect on the SAME hook instance, which
    // is exactly the "ran again with no semantic change" (double-fire) shape.
    await act(async () => {
      render(
        <StrictMode>
          <EffectHost dep={1} />
        </StrictMode>,
      );
    });
    await drain();
    const effects = byKind('effect');
    // At least one run recorded no changed deps.
    const noChange = effects.filter((r) => (r.changedDeps as string[]).length === 0);
    expect(noChange.length).toBeGreaterThanOrEqual(1);
    // And a genuinely double-fired run (run >= 2) with [] deps exists — the signal.
    expect(effects.some((r) => (r.run as number) >= 2 && (r.changedDeps as string[]).length === 0)).toBe(
      true,
    );
  });

  it('records the changed dep name (never its value) on a real dep change', async () => {
    let set: (n: number) => void = () => undefined;
    function Ctl() {
      const [n, setN] = useState(1);
      set = setN;
      return <EffectHost dep={n} />;
    }
    await act(async () => {
      render(<Ctl />);
    });
    await act(async () => set(2));
    await drain();
    const changed = byKind('effect').find((r) => (r.changedDeps as string[]).length > 0);
    expect(changed?.changedDeps).toEqual(['dep']);
    // Privacy: the value 2 must appear nowhere in the effect record.
    expect(JSON.stringify(changed)).not.toContain('"2"');
  });
});

describe('useRenderCount aggregate — acceptance (e) + (d) collapse', () => {
  function Row() {
    useRenderCount('Row', undefined, 'aggregate');
    return null;
  }
  function Section({ tick }: { tick: number }) {
    useRenderCount('Section', { tick }, 'aggregate');
    return null;
  }

  it('a flicker (many re-renders, few instances) yields renders > 3 × instances', async () => {
    let bump: () => void = () => undefined;
    function Host() {
      const [, setN] = useState(0);
      bump = () => setN((n) => n + 1);
      useRenderCount('Host', undefined, 'aggregate');
      return null;
    }
    await act(async () => {
      render(<Host />);
    });
    // 5 re-renders of a single instance = a flicker.
    for (let i = 0; i < 5; i += 1) await act(async () => bump());
    await drain();
    const tally = byKind('render.tally').find((r) => r.component === 'Host');
    expect(tally).toBeTruthy();
    expect(tally?.instances).toBe(1);
    expect(tally?.renders as number).toBeGreaterThan(3 * (tally?.instances as number));
  });

  it('N row instances collapse to ONE tally per component per window', async () => {
    function List({ tick }: { tick: number }) {
      return (
        <>
          <Section tick={tick} />
          {Array.from({ length: 500 }, (_, i) => (
            <Row key={i} />
          ))}
        </>
      );
    }
    let bump: () => void = () => undefined;
    function Host() {
      const [tick, setTick] = useState(0);
      bump = () => setTick((t) => t + 1);
      return <List tick={tick} />;
    }
    await act(async () => {
      render(<Host />);
    });
    sunk = [];
    __resetRenderTally();
    // One "ref change" re-renders the section + all 500 rows.
    await act(async () => bump());
    await drain();
    // Exactly one tally record per component, never 500 Row records.
    expect(byKind('render.tally').filter((r) => r.component === 'Row')).toHaveLength(1);
    expect(byKind('render.tally').filter((r) => r.component === 'Section')).toHaveLength(1);
    const rowTally = byKind('render.tally').find((r) => r.component === 'Row');
    expect(rowTally?.instances).toBe(500);
    expect(rowTally?.renders).toBe(500);
  });

  it('a zero-render window emits nothing', async () => {
    // No renders → tallyRender never called → no timer → flush emits nothing.
    flushRenderTally();
    await flushNow();
    expect(byKind('render.tally')).toHaveLength(0);
  });
});

describe('render.tally window scheduling (fake timers)', () => {
  it('flushes one record per component after the 500 ms window', async () => {
    vi.useFakeTimers();
    function Row() {
      useRenderCount('W', undefined, 'aggregate');
      return null;
    }
    render(<Row />);
    expect(sunk.filter((r) => r.kind === 'render.tally')).toHaveLength(0);
    await act(async () => {
      vi.advanceTimersByTime(RENDER_TALLY_WINDOW_MS + 1);
    });
    await flushNow();
    expect(sunk.filter((r) => r.kind === 'render.tally' && r.component === 'W')).toHaveLength(1);
  });
});

describe('useStateTransitionLog — reserved API', () => {
  /** §7.1 — a branch name is repo content: `strict` must emit an ordinal, and the
   *  ordinal must be STABLE per value so a flip-flop is still visible. */
  it('redacts a branch-name-shaped value instead of logging it', async () => {
    function Store({ v }: { v: string }) {
      useStateTransitionLog('repo', { branch: v });
      return null;
    }
    const { rerender } = render(<Store v="main" />);
    await act(async () => rerender(<Store v="feature/secret-customer" />));
    await act(async () => rerender(<Store v="main" />));
    await drain();
    const transitions = byKind('state').filter((r) => r.field === 'branch');
    expect(transitions).toHaveLength(2);
    const values = transitions.flatMap((r) => [r.from, r.to]);
    for (const v of values) {
      expect(v).toMatch(/^ui:(other|path)#\d+/);
    }
    expect(JSON.stringify(transitions)).not.toContain('secret-customer');
    expect(JSON.stringify(transitions)).not.toContain('main');
    // Same value ⇒ same ordinal, so `main → x → main` stays mechanically visible.
    expect(transitions[0].from).toBe(transitions[1].to);
  });

  it('redacts a path-shaped value as a path ordinal, keeping the extension', async () => {
    function Store({ v }: { v: string }) {
      useStateTransitionLog('diff', { file: v });
      return null;
    }
    const { rerender } = render(<Store v="src/app.ts" />);
    await act(async () => rerender(<Store v="C:/Users/dana/private/notes.md" />));
    await drain();
    const st = byKind('state').find((r) => r.field === 'file');
    expect(st?.from).toMatch(/^ui:path#\d+\.ts$/);
    expect(st?.to).toMatch(/^ui:path#\d+\.md$/);
    expect(JSON.stringify(st)).not.toContain('dana');
    expect(JSON.stringify(st)).not.toContain('notes');
  });

  it('keeps structural values and logs raw names only in raw mode', async () => {
    configureObs({ ...DEV_TRACE, includeRawNames: true });
    function Store({ v, n }: { v: string; n: number }) {
      useStateTransitionLog('repo', { branch: v, count: n });
      return null;
    }
    const { rerender } = render(<Store v="main" n={1} />);
    await act(async () => rerender(<Store v="feature" n={2} />));
    await drain();
    const branch = byKind('state').find((r) => r.field === 'branch');
    expect(branch?.from).toBe('main');
    expect(branch?.to).toBe('feature');
    const count = byKind('state').find((r) => r.field === 'count');
    expect(count?.from).toBe('1');
    expect(count?.to).toBe('2');
  });

  /** RAW widens IDENTIFIER fidelity, never CONTENT fidelity: a free-text- or
   *  credential-named field is an ordinal in BOTH modes, so wiring this hook to
   *  a commit-message draft or the search input cannot write prose to disk.
   *  `state` records are outside the writer's `raw_args` backstop. */
  it('still redacts free-text and credential FIELDS in raw mode', async () => {
    configureObs({ ...DEV_TRACE, includeRawNames: true });
    function Store({ v }: { v: string }) {
      useStateTransitionLog('composer', { message: v, searchText: v, token: v, branch: v });
      return null;
    }
    const { rerender } = render(<Store v="alpha" />);
    await act(async () => rerender(<Store v="NEGTEST_DRAFT_ZQX" />));
    await drain();
    const states = byKind('state');
    for (const field of ['message', 'searchText', 'token']) {
      const st = states.find((r) => r.field === field);
      expect(st?.to, field).toMatch(/^ui:(other|path)#\d+/);
    }
    // The identifier-shaped field is unaffected: raw still widens those.
    expect(states.find((r) => r.field === 'branch')?.to).toBe('NEGTEST_DRAFT_ZQX');
    // ...and the three content fields carry no occurrence of the draft at all.
    const content = states.filter((r) => r.field !== 'branch');
    expect(JSON.stringify(content)).not.toContain('NEGTEST_DRAFT_ZQX');
  });
});

/**
 * The wire contract for `changedProps` (P91 §9.2): three states, all distinct.
 * Absent ⇒ the call site tracks no props. `[]` ⇒ tracked, nothing changed.
 * Names ⇒ tracked, these changed. Coercing an unsupplied props bag to `{}`
 * collapses the first two, and a tally that can only ever say `[]` is a
 * diagnostic that says nothing.
 */
describe('render.tally changedProps — absence means "not tracked"', () => {
  function Untracked() {
    useRenderCount('Untracked', undefined, 'aggregate');
    return null;
  }
  function Tracked({ tick }: { tick: number }) {
    useRenderCount('Tracked', { tick }, 'aggregate');
    return null;
  }

  it('a site that supplies no props emits NO changedProps key at all', async () => {
    await act(async () => {
      render(<Untracked />);
    });
    await drain();
    const tally = byKind('render.tally').find((r) => r.component === 'Untracked');
    expect(tally).toBeTruthy();
    // `not.toHaveProperty`, not `toBeUndefined` — the latter also passes for a
    // present-but-undefined key, which serializes to nothing but reads as tracked.
    expect(tally).not.toHaveProperty('changedProps');
  });

  it('a props-supplied site that saw no change emits changedProps: []', async () => {
    await act(async () => {
      render(<Tracked tick={0} />);
    });
    await drain();
    const tally = byKind('render.tally').find((r) => r.component === 'Tracked');
    expect(tally).toHaveProperty('changedProps');
    expect(tally?.changedProps).toEqual([]);
  });

  it('a props-supplied site whose prop changed emits the NAME', async () => {
    let bump: () => void = () => undefined;
    function Host() {
      const [tick, setTick] = useState(0);
      bump = () => setTick((t) => t + 1);
      return <Tracked tick={tick} />;
    }
    await act(async () => {
      render(<Host />);
    });
    sunk = [];
    __resetRenderTally();
    await act(async () => bump());
    await drain();
    const tally = byKind('render.tally').find((r) => r.component === 'Tracked');
    expect(tally?.changedProps).toEqual(['tick']);
  });
});
