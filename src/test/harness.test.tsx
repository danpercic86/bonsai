/**
 * T1 acceptance artifact — proves the jsdom (`dom`) vitest project works:
 * RTL renders a real presentational component, jest-dom matchers are wired
 * via src/test/setup.ts, and the canvas 2D stub answers getContext('2d').
 *
 * Mock-IPC note (contract §1.5): the `dom` project sets VITE_MOCK_IPC=1, so
 * `import { ipc } from '../ipc'` resolves to the mock layer at module load —
 * identical to the browser harness. Per-test overrides: spy on the concrete
 * object (`import { mockIpc } from '../ipc/mock'; vi.spyOn(mockIpc, ...)`).
 * Fixture state resets via localStorage.clear() in setup.ts; there is no
 * global mock reset export — targeted seed helpers live in
 * src/ipc/mock/opStateSeed.ts (seedOpState, seedPickRevertConflict).
 */
import { describe, it, expect } from 'vitest';
import { render, screen } from '@testing-library/react';
import { EmptyState } from '../components/EmptyState';

const noop = () => {};

describe('T1 jsdom harness', () => {
  it('renders a component with RTL and jest-dom matchers', () => {
    render(
      <EmptyState
        loading={false}
        error={null}
        recents={[]}
        onOpenRepository={noop}
        onCloneOpen={noop}
        onInitRepository={noop}
        onOpenRecent={noop}
      />,
    );
    expect(screen.getByRole('heading', { name: 'Bonsai' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Open repository' })).toBeEnabled();
  });

  it('provides a tolerant canvas 2D context stub', () => {
    const canvas = document.createElement('canvas');
    const ctx = canvas.getContext('2d');
    expect(ctx).toBeTruthy();
    expect(ctx!.measureText('bonsai').width).toBeGreaterThan(0);
    // Anything not explicitly stubbed is a no-op function, not a crash.
    expect(() => (ctx as CanvasRenderingContext2D).fillRect(0, 0, 1, 1)).not.toThrow();
  });
});
