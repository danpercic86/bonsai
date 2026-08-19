/** P70 UI §7 — the Re-check button wired to the REAL hook.
 *
 *  Why this file exists (harness finding H1). `useGitAvailability.test.tsx`
 *  asserted the 400 ms floor by timing the promise `recheck()` returns — a
 *  variable the button never consumes. That passes even if the pending state
 *  never reaches the DOM, so it could not answer "does the user actually see
 *  that Re-check ran?". These tests assert the rendered LABEL of the control the
 *  banner offers, through the same hook → component wiring the app uses.
 *
 *  Deliberately in its own file: `GitMissingBanner.test.tsx` drives the
 *  component with a hand-built state object (the right tool for copy/variant
 *  coverage), and mixing an integration harness into it would make every case
 *  there pay for real timers.
 */
import { describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { StrictMode } from 'react';

import { GitMissingBanner } from '../GitMissingBanner';
import { useGitAvailability, MIN_CHECKING_MS } from '../../hooks/useGitAvailability';
import { mockIpc } from '../../ipc/mock';
import type { GitAvailability } from '../../ipc';

const MISSING: GitAvailability = {
  found: false,
  path: null,
  version: null,
  source: 'fallback',
  detail: 'Git is not available. …',
};

function Harness() {
  const git = useGitAvailability();
  return <GitMissingBanner git={git} onGitAvailable={() => {}} />;
}

/** StrictMode on purpose: it is what the app mounts under (`main.tsx`), and it
 *  double-invokes the mount probe — exactly the traffic the run-id guard has to
 *  survive before a click is even possible. */
function renderHarness() {
  return render(
    <StrictMode>
      <Harness />
    </StrictMode>,
  );
}

describe('Re-check — the pending state the user actually sees', () => {
  it('shows "Checking…" for the whole minimum window, not just an invisible tick', async () => {
    vi.spyOn(mockIpc, 'checkGitAvailability').mockResolvedValue(MISSING);
    renderHarness();
    await waitFor(() => expect(screen.getByText('Git is not available')).toBeInTheDocument());
    const btn = screen.getByRole('button', { name: 'Re-check' });

    fireEvent.click(btn);
    // Synchronously after the click — no awaiting, no fake timers: the label
    // must have flipped in the same commit as the press.
    expect(btn.textContent).toBe('Checking…');
    expect(btn).toBeDisabled();

    // …and still be showing it well after the IPC (which resolves immediately)
    // has answered. Without the floor this reads "Re-check" again by now, which
    // is the silent no-op button H1 reported.
    await new Promise<void>((resolve) => {
      setTimeout(resolve, MIN_CHECKING_MS * 0.6);
    });
    expect(btn.textContent).toBe('Checking…');

    await waitFor(() => expect(btn.textContent).toBe('Re-check'), { timeout: 2000 });
    expect(btn).not.toBeDisabled();
  });

  it('a slow probe holds the pending state until it answers (the floor is a floor, not a cap)', async () => {
    // A flag rather than `mockResolvedValueOnce`: StrictMode's double mount
    // fires the startup probe TWICE, so a per-call queue would be off by one.
    let hang = false;
    // Boxed so TypeScript keeps the call signature (a bare `let` assigned only
    // inside the callback narrows to `never` at the call site).
    const pending: { settle: ((v: GitAvailability) => void) | null } = { settle: null };
    vi.spyOn(mockIpc, 'checkGitAvailability').mockImplementation(() =>
      hang
        ? new Promise<GitAvailability>((resolve) => {
            pending.settle = resolve;
          })
        : Promise.resolve(MISSING),
    );
    renderHarness();
    await waitFor(() => expect(screen.getByText('Git is not available')).toBeInTheDocument());
    const btn = screen.getByRole('button', { name: 'Re-check' });

    hang = true;
    fireEvent.click(btn);
    await new Promise<void>((resolve) => {
      setTimeout(resolve, MIN_CHECKING_MS * 1.5);
    });
    expect(btn.textContent).toBe('Checking…');

    pending.settle?.(MISSING);
    await waitFor(() => expect(btn.textContent).toBe('Re-check'));
  });
});
