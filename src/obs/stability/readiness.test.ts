import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { onPhaseChange, phaseAt, resetPhaseForTests } from './phase';
import { beginRepoBoot, noteLanded, resetReadinessForTests, tapReadiness } from './readiness';
import type { GraphChunk, IpcApi, StatusSnapshot } from '../../ipc/types';

let now = 0;
let phases: string[] = [];

beforeEach(() => {
  resetPhaseForTests();
  resetReadinessForTests();
  vi.useFakeTimers({ toFake: ['setTimeout', 'clearTimeout'] });
  now = 100;
  vi.spyOn(performance, 'now').mockImplementation(() => now);
  phases = [];
  onPhaseChange((p) => phases.push(p));
});
afterEach(() => {
  vi.useRealTimers();
  vi.restoreAllMocks();
});

/** node env: no rAF, so readiness falls back to a 16 ms timer. */
function flushFrame(): void {
  vi.advanceTimersByTime(20);
}

describe('readiness -> markInteractive (section 3.3)', () => {
  it('fires one frame after BOTH graph meta and status landed for the current repo', () => {
    beginRepoBoot('a');
    noteLanded('a', 'graph');
    flushFrame();
    expect(phases).toEqual([]);
    noteLanded('b', 'status'); // another tab: ignored
    flushFrame();
    expect(phases).toEqual([]);
    noteLanded('a', 'status');
    expect(phases).toEqual([]); // deferred past the commit/paint
    flushFrame();
    expect(phases).toEqual(['interactive']);
  });

  it('a switch re-arms boot and cancels a pending mark for the old repo', () => {
    beginRepoBoot('a');
    noteLanded('a', 'graph');
    noteLanded('a', 'status');
    now = 200;
    beginRepoBoot('b'); // before the deferred mark ran
    flushFrame();
    expect(phases).toEqual([]);
    noteLanded('b', 'status');
    noteLanded('b', 'graph');
    flushFrame();
    expect(phases).toEqual(['interactive']);
    now = 500;
    beginRepoBoot('a');
    expect(phases).toEqual(['interactive', 'boot']);
    expect(phaseAt(600)).toBe('boot');
    beginRepoBoot('a'); // same id (StrictMode re-run): no-op
    expect(phases).toEqual(['interactive', 'boot']);
  });

  it('null (no repo) does not re-arm boot', () => {
    beginRepoBoot(null);
    expect(phases).toEqual([]);
  });
});

describe('tapReadiness', () => {
  const meta = { kind: 'meta' } as GraphChunk;
  const snapshot = {} as StatusSnapshot;

  function fakeApi(opts: { statusFails?: boolean; metaChunks?: boolean }): IpcApi {
    const api = {
      getStatus: (): Promise<StatusSnapshot> =>
        opts.statusFails === true ? Promise.reject(new Error('io')) : Promise.resolve(snapshot),
      streamGraph: (_r: string, _f: unknown, onChunk: (c: GraphChunk) => void): Promise<void> => {
        if (opts.metaChunks === true) onChunk(meta);
        return new Promise<void>(() => {}); // never settles: meta alone must suffice
      },
      other: () => 'x',
    };
    return api as unknown as IpcApi;
  }

  it('first meta chunk + a status settle (even an error) mark interactive; chunks still reach the caller', async () => {
    const api = tapReadiness(fakeApi({ statusFails: true, metaChunks: true }));
    beginRepoBoot('r');
    const seen: GraphChunk[] = [];
    void api.streamGraph('r', null, (c) => seen.push(c));
    expect(seen).toEqual([meta]);
    await expect(api.getStatus('r')).rejects.toThrow('io');
    flushFrame();
    expect(phases).toEqual(['interactive']);
  });

  it('a graph stream that settles without meta still counts (empty result)', async () => {
    const raw = fakeApi({});
    const api = tapReadiness({ ...raw, streamGraph: () => Promise.resolve() } as IpcApi);
    beginRepoBoot('r');
    await api.streamGraph('r', null, () => {});
    await api.getStatus('r');
    flushFrame();
    expect(phases).toEqual(['interactive']);
  });

  it('passes other methods through with their identity, read live (spies still apply)', async () => {
    const raw = fakeApi({});
    const tapped = tapReadiness(raw);
    const rawRec = raw as unknown as Record<string, unknown>;
    expect((tapped as unknown as Record<string, unknown>).other).toBe(rawRec.other);
    const spy = vi.spyOn(raw, 'getStatus').mockResolvedValue({ spied: true } as unknown as StatusSnapshot);
    await expect(tapped.getStatus('x')).resolves.toEqual({ spied: true });
    expect(spy).toHaveBeenCalledWith('x');
  });
});
