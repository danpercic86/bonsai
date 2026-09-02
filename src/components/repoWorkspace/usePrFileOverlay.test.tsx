/** P93 §6.1 — the PRODUCER side of the focus-restore "dismissal token".
 *
 *  `PrChangesSection` is well covered on the consumer guards; what is untested is
 *  the thing rev 2 was actually about: WHICH paths arm `prRestoreFocusTo` and
 *  which must leave it alone. The bump table in §6.1 says exactly one situation
 *  arms it — a user dismissal of the center overlay while a `pr:` slot is open
 *  (the overlay `×`, the Esc layer and the error-banner dismiss all funnel
 *  through `handleDismissDiffOverlay`; wiring verified in `RepoWorkspace.tsx`
 *  2145 / 2456 and `DiffOverlay.tsx` 428/441/448). Everything else — C1 tab
 *  leave, C2 PR switch / Back, C3 head advance (all three via
 *  `handleClosePrFileDiff`), C5 slot replacement (which never goes through a
 *  dismissal at all) and the re-click collapse — must not.
 *
 *  The fakes below are a miniature slot machine that mirrors the RepoWorkspace
 *  wiring: `fetchDiffSlot` installs the key, and `collapseDiffSlot` clears BOTH
 *  the slot and `prOverlayCtx`. That last part is load-bearing — it is what makes
 *  the "captured before collapse" assertion able to fail.
 */
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { act, renderHook } from '@testing-library/react';

import { ipc } from '../../ipc';
import type { FileDiff, FileDiffHeader } from '../../ipc';
import type { PrFileDiffOpen } from '../prPanel/PrChangesSection';
import type { DiffSlot } from '../StatusPanel';
import { prSlotKey } from './prSlotKey';
import type { PrOverlayCtx } from './types';
import { usePrFileOverlay, type PrFileOverlayDeps } from './usePrFileOverlay';

const BASE = 'a'.repeat(40);
const HEAD = 'b'.repeat(40);

function header(over: Partial<FileDiffHeader> = {}): FileDiffHeader {
  return {
    path: 'src/app/main.rs',
    origPath: null,
    status: 'modified',
    additions: 3,
    deletions: 1,
    binary: false,
    ...over,
  };
}

function openCtx(over: Partial<FileDiffHeader> = {}, prNumber = 142): PrFileDiffOpen {
  return { prNumber, baseOid: BASE, headOid: HEAD, header: header(over) };
}

interface Harness {
  deps: PrFileOverlayDeps;
  /** Reads whatever `setPrOverlayCtx` last wrote (state + ref stay in sync). */
  ctx: () => PrOverlayCtx | null;
  collapses: () => number;
}

function makeHarness(viewMode: 'diff' | 'file' | 'split' = 'diff', intraline = false): Harness {
  const diffSlotRef: { current: DiffSlot | null } = { current: null };
  const prOverlayCtxRef: { current: PrOverlayCtx | null } = { current: null };
  let collapseCalls = 0;

  const deps: PrFileOverlayDeps = {
    repoId: 'r1',
    diffSlotRef,
    diffViewModeRef: { current: viewMode },
    intralineRef: { current: intraline },
    prOverlayCtxRef,
    // RepoWorkspace keeps a ref mirror of this state; mirror it here too.
    setPrOverlayCtx: (next) => {
      prOverlayCtxRef.current =
        typeof next === 'function'
          ? (next as (p: PrOverlayCtx | null) => PrOverlayCtx | null)(prOverlayCtxRef.current)
          : next;
    },
    fetchDiffSlot: async (key, fetcher) => {
      diffSlotRef.current = { key, state: 'loading' } as unknown as DiffSlot;
      await fetcher();
    },
    // The real one clears the slot AND `prOverlayCtx` (§3/§6).
    collapseDiffSlot: () => {
      collapseCalls += 1;
      diffSlotRef.current = null;
      prOverlayCtxRef.current = null;
    },
  };

  return { deps, ctx: () => prOverlayCtxRef.current, collapses: () => collapseCalls };
}

describe('usePrFileOverlay', () => {
  beforeEach(() => {
    vi.restoreAllMocks();
    vi.spyOn(ipc, 'forgePrFileDiff').mockResolvedValue({
      header: header(),
      hunks: [],
    } as unknown as FileDiff);
  });

  // ── opening ───────────────────────────────────────────────────────────────

  it('opens a pr: slot with the §3 key and the full forgePrFileDiff arg list', async () => {
    const h = makeHarness('file', true);
    const { result } = renderHook(() => usePrFileOverlay(h.deps));

    await act(async () => {
      result.current.handleOpenPrFileDiff(openCtx({ path: 'src/b.ts', origPath: 'src/a.ts' }));
    });

    expect(h.deps.diffSlotRef.current?.key).toBe(prSlotKey(BASE, HEAD, 'src/b.ts'));
    expect(ipc.forgePrFileDiff).toHaveBeenCalledWith(
      'r1',
      BASE,
      HEAD,
      'src/b.ts',
      'src/a.ts',
      true, // fullContext — diffViewModeRef === 'file'
      true, // intraline
    );
    expect(h.ctx()).toEqual({
      prNumber: 142,
      baseOid: BASE,
      headOid: HEAD,
      path: 'src/b.ts',
      origPath: 'src/a.ts',
      status: 'modified',
    });
    // Opening never arms the token.
    expect(result.current.prRestoreFocusTo).toBeNull();
  });

  it('passes fullContext=false outside File mode', async () => {
    const h = makeHarness('split', false);
    const { result } = renderHook(() => usePrFileOverlay(h.deps));
    await act(async () => result.current.handleOpenPrFileDiff(openCtx()));
    expect(ipc.forgePrFileDiff).toHaveBeenCalledWith(
      'r1',
      BASE,
      HEAD,
      'src/app/main.rs',
      null,
      false,
      false,
    );
  });

  // ── the ONE path that arms the token ──────────────────────────────────────

  it('arms the token on dismissal (× / Esc / error-banner — one handler)', async () => {
    const h = makeHarness();
    const { result } = renderHook(() => usePrFileOverlay(h.deps));
    await act(async () => result.current.handleOpenPrFileDiff(openCtx()));

    act(() => result.current.handleDismissDiffOverlay());

    expect(h.collapses()).toBe(1);
    expect(result.current.prRestoreFocusTo).toEqual({ path: 'src/app/main.rs', token: 1 });
  });

  it('captures the path from prOverlayCtx BEFORE collapse clears it', async () => {
    // The fake collapse nulls prOverlayCtxRef, so an implementation that read the
    // path after collapsing would arm nothing (or the wrong path) here.
    const h = makeHarness();
    const { result } = renderHook(() => usePrFileOverlay(h.deps));
    await act(async () => result.current.handleOpenPrFileDiff(openCtx({ path: 'deep/x:y.md' })));

    act(() => result.current.handleDismissDiffOverlay());

    expect(h.ctx()).toBeNull();
    expect(result.current.prRestoreFocusTo?.path).toBe('deep/x:y.md');
  });

  it('is monotonic and one-shot: re-open clears, next dismissal bumps to 2', async () => {
    const h = makeHarness();
    const { result } = renderHook(() => usePrFileOverlay(h.deps));

    await act(async () => result.current.handleOpenPrFileDiff(openCtx()));
    act(() => result.current.handleDismissDiffOverlay());
    expect(result.current.prRestoreFocusTo).toEqual({ path: 'src/app/main.rs', token: 1 });

    // A newly opened pr: slot invalidates the pending restore (§6.1 last line).
    await act(async () => result.current.handleOpenPrFileDiff(openCtx()));
    expect(result.current.prRestoreFocusTo).toBeNull();

    act(() => result.current.handleDismissDiffOverlay());
    expect(result.current.prRestoreFocusTo).toEqual({ path: 'src/app/main.rs', token: 2 });
  });

  it('bumps per dismissal even for the same row (token never repeats)', async () => {
    const h = makeHarness();
    const { result } = renderHook(() => usePrFileOverlay(h.deps));
    const tokens: number[] = [];
    for (let i = 0; i < 3; i += 1) {
      await act(async () => result.current.handleOpenPrFileDiff(openCtx()));
      act(() => result.current.handleDismissDiffOverlay());
      tokens.push(result.current.prRestoreFocusTo?.token ?? -1);
    }
    expect(tokens).toEqual([1, 2, 3]);
  });

  // ── paths that must NOT arm the token ─────────────────────────────────────

  it('C1/C2/C3 via handleClosePrFileDiff collapse the pr: slot without arming', async () => {
    const h = makeHarness();
    const { result } = renderHook(() => usePrFileOverlay(h.deps));
    await act(async () => result.current.handleOpenPrFileDiff(openCtx()));

    act(() => result.current.handleClosePrFileDiff());

    expect(h.collapses()).toBe(1);
    expect(h.deps.diffSlotRef.current).toBeNull();
    expect(result.current.prRestoreFocusTo).toBeNull();
  });

  it('handleClosePrFileDiff leaves a non-pr: slot alone (prefix check)', () => {
    const h = makeHarness();
    h.deps.diffSlotRef.current = { key: 'unstaged:src/a.ts' } as unknown as DiffSlot;
    const { result } = renderHook(() => usePrFileOverlay(h.deps));

    act(() => result.current.handleClosePrFileDiff());

    expect(h.collapses()).toBe(0);
    expect(h.deps.diffSlotRef.current?.key).toBe('unstaged:src/a.ts');
    expect(result.current.prRestoreFocusTo).toBeNull();
  });

  it('C5: a slot replacement never arms, and dismissing the replacement does not either', async () => {
    const h = makeHarness();
    const { result } = renderHook(() => usePrFileOverlay(h.deps));
    await act(async () => result.current.handleOpenPrFileDiff(openCtx()));

    // A graph commit click goes straight through fetchDiffSlot — no dismissal.
    await act(async () => {
      await h.deps.fetchDiffSlot('commit:deadbeef:src/app/main.rs', async () => ({}) as FileDiff);
    });
    expect(result.current.prRestoreFocusTo).toBeNull();

    // …and closing THAT overlay must not restore focus into the PR list either.
    act(() => result.current.handleDismissDiffOverlay());
    expect(h.collapses()).toBe(1);
    expect(result.current.prRestoreFocusTo).toBeNull();
  });

  it('re-clicking the active row collapses without arming or refetching', async () => {
    const h = makeHarness();
    const { result } = renderHook(() => usePrFileOverlay(h.deps));
    await act(async () => result.current.handleOpenPrFileDiff(openCtx()));
    expect(ipc.forgePrFileDiff).toHaveBeenCalledTimes(1);

    await act(async () => result.current.handleOpenPrFileDiff(openCtx()));

    expect(h.collapses()).toBe(1);
    expect(h.deps.diffSlotRef.current).toBeNull();
    expect(ipc.forgePrFileDiff).toHaveBeenCalledTimes(1);
    expect(result.current.prRestoreFocusTo).toBeNull();
  });

  it('handleDismissDiffOverlay is a plain collapse when the slot key is not pr:', () => {
    const h = makeHarness();
    h.deps.diffSlotRef.current = { key: 'staged:src/a.ts' } as unknown as DiffSlot;
    const { result } = renderHook(() => usePrFileOverlay(h.deps));

    act(() => result.current.handleDismissDiffOverlay());

    expect(h.collapses()).toBe(1);
    expect(result.current.prRestoreFocusTo).toBeNull();
  });

  it('handleDismissDiffOverlay with no slot open collapses and does not arm', () => {
    const h = makeHarness();
    const { result } = renderHook(() => usePrFileOverlay(h.deps));

    act(() => result.current.handleDismissDiffOverlay());

    expect(h.collapses()).toBe(1);
    expect(result.current.prRestoreFocusTo).toBeNull();
  });

  // ── stability (the close handler is a cleanup-effect dependency) ──────────

  it('all three handlers are referentially stable across re-renders', () => {
    const h = makeHarness();
    const { result, rerender } = renderHook(() => usePrFileOverlay(h.deps));
    const first = { ...result.current };
    rerender();
    expect(result.current.handleOpenPrFileDiff).toBe(first.handleOpenPrFileDiff);
    expect(result.current.handleClosePrFileDiff).toBe(first.handleClosePrFileDiff);
    expect(result.current.handleDismissDiffOverlay).toBe(first.handleDismissDiffOverlay);
  });
});
