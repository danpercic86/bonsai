/** P70 UI §6/§7: the notice bar's states, both variants, the a11y contract, and
 *  the rule that a failed re-check produces an in-banner readout and NO toast. */
import { afterEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen, waitFor, within } from '@testing-library/react';

import { GitMissingBanner } from './GitMissingBanner';
import type { GitAvailabilityState } from '../hooks/useGitAvailability';
import type { GitAvailability } from '../ipc';

const FOUND: GitAvailability = {
  found: true,
  path: '/usr/bin/git',
  version: '2.47.1',
  source: 'path',
  detail: 'Git 2.47.1 — /usr/bin/git (path)',
};
const MISSING: GitAvailability = {
  found: false,
  path: null,
  version: null,
  source: 'fallback',
  detail: 'Git is not available. Bonsai could not find a runnable `git` executable — …',
};
const UNRUNNABLE: GitAvailability = {
  found: false,
  path: 'C:\\Users\\dev\\AppData\\Local\\Programs\\Git\\cmd\\git.exe',
  version: null,
  source: 'override',
  detail: 'Git is not available. …',
};

function state(over: Partial<GitAvailabilityState> = {}): GitAvailabilityState {
  return {
    status: null,
    checking: false,
    latched: false,
    recheck: vi.fn().mockResolvedValue(null),
    noteGitNotFound: vi.fn(),
    ...over,
  };
}

function renderBanner(over: Partial<GitAvailabilityState> = {}, onGitAvailable = vi.fn()) {
  const git = state(over);
  const view = render(<GitMissingBanner git={git} onGitAvailable={onGitAvailable} />);
  return { git, onGitAvailable, ...view };
}

afterEach(() => vi.restoreAllMocks());

describe('GitMissingBanner — when it renders at all', () => {
  it('status === null renders ONLY the (empty) announcer: zero height, no layout shift', () => {
    const { container } = renderBanner();
    expect(container.querySelector('.git-banner')).toBeNull();
    const announce = container.querySelector('.git-banner-announce');
    expect(announce).not.toBeNull();
    expect(announce?.textContent).toBe('');
    // The live region must be MOUNTED before it has anything to say.
    expect(announce?.getAttribute('aria-live')).toBe('polite');
  });

  it('found === true renders nothing but the announcer', () => {
    const { container } = renderBanner({ status: FOUND });
    expect(container.querySelector('.git-banner')).toBeNull();
  });

  it('a stale `found: true` status keeps the bar hidden — the LATCH is not the source of truth', () => {
    // MUST-FIX (round 2): the fix for a mid-session breakage is a fresh probe
    // in `useGitAvailability` (the latch's rising edge), NOT `latched ||
    // !found` here — pinning the bar open off the latch would render Variant A
    // copy over a stale `path`-bearing status, i.e. the wrong diagnosis.
    const { container } = renderBanner({ status: FOUND, latched: true });
    expect(container.querySelector('.git-banner')).toBeNull();
  });

  it('the announcer element is never remounted when the bar appears', () => {
    const { container, rerender } = render(
      <GitMissingBanner git={state({ status: FOUND })} onGitAvailable={vi.fn()} />,
    );
    const before = container.querySelector('.git-banner-announce');
    rerender(<GitMissingBanner git={state({ status: MISSING })} onGitAvailable={vi.fn()} />);
    // Identity, not just presence: a remounted live region drops its pending
    // announcement on several screen readers.
    expect(container.querySelector('.git-banner-announce')).toBe(before);
    expect(container.querySelector('.git-banner')).not.toBeNull();
  });

  it('a latch fired while status is still null shows Variant A with NO technical block', () => {
    renderBanner({ status: null, latched: true });
    expect(screen.getByText('Git is not available')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: /Details/ }));
    expect(screen.queryByText('TECHNICAL DETAILS')).toBeNull();
  });
});

describe('GitMissingBanner — variants', () => {
  it('Variant A (path === null): not-found copy, no Tried: row', () => {
    const { container } = renderBanner({ status: MISSING });
    // Scoped to the visible text column: the live region legitimately repeats
    // this copy (UI §5.7), so an unscoped text query is ambiguous by design.
    const text = within(container.querySelector('.git-banner-text') as HTMLElement);
    expect(text.getByText('Git is not available')).toBeInTheDocument();
    expect(
      text.getByText(/Your saved credentials are fine — Bonsai never got as far/),
    ).toBeInTheDocument();
    expect(container.querySelector('.git-banner-path')).toBeNull();
    // Severity is a region, NOT an assertive alert (which would re-announce the
    // whole bar on every retry).
    const region = screen.getByRole('region');
    expect(region.getAttribute('aria-labelledby')).toBe('git-banner-title');
  });

  it('Variant B (path !== null): "couldn\'t be started" + the ellipsised path with a title', () => {
    const { container } = renderBanner({ status: UNRUNNABLE });
    const text = within(container.querySelector('.git-banner-text') as HTMLElement);
    expect(text.getByText("Git couldn't be started")).toBeInTheDocument();
    const path = container.querySelector('.git-banner-path');
    expect(path?.textContent).toContain(UNRUNNABLE.path);
    expect(path?.getAttribute('title')).toBe(UNRUNNABLE.path);
    // `override` source ⇒ the BONSAI_GIT_BIN remedy is the HEADLINE, so it is
    // not repeated in the disclosure list.
    expect(text.getByText(/BONSAI_GIT_BIN points at a program/)).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: /Details/ }));
    expect(screen.queryByText(/Set BONSAI_GIT_BIN to the full path/)).toBeNull();
  });
});

describe('GitMissingBanner — interaction', () => {
  it('checking swaps the button label and disables it, leaving the bar rendered', () => {
    renderBanner({ status: MISSING, checking: true });
    const btn = screen.getByRole('button', { name: 'Checking…' });
    expect(btn).toBeDisabled();
    expect(btn.getAttribute('aria-busy')).toBe('true');
    // The banner itself must NOT be hidden or dimmed while checking.
    expect(screen.getByText('Git is not available')).toBeInTheDocument();
  });

  it('a failed re-check adds the checked-at line, announces politely, and toasts NOTHING', async () => {
    const recheck = vi.fn().mockResolvedValue(MISSING);
    const { container, onGitAvailable } = renderBanner({ status: MISSING, recheck });

    fireEvent.click(screen.getByRole('button', { name: 'Re-check' }));
    await waitFor(() =>
      expect(container.querySelector('.git-banner-checked')?.textContent).toMatch(
        /^Still not found — checked \d\d:\d\d\.$/,
      ),
    );
    expect(onGitAvailable).not.toHaveBeenCalled();
    expect(container.querySelector('.git-banner-announce')?.textContent).toBe(
      'Git is still not available.',
    );
  });

  it('a successful re-check fires exactly one success toast naming the version', async () => {
    const recheck = vi.fn().mockResolvedValue(FOUND);
    const { onGitAvailable } = renderBanner({ status: MISSING, recheck });

    fireEvent.click(screen.getByRole('button', { name: 'Re-check' }));
    await waitFor(() => expect(onGitAvailable).toHaveBeenCalledTimes(1));
    expect(onGitAvailable).toHaveBeenCalledWith(
      'Git is available again — Bonsai found Git 2.47.1.',
    );
  });

  it('Details is a real disclosure and shows the SSH-honest capability rows', () => {
    renderBanner({ status: MISSING });
    const toggle = screen.getByRole('button', { name: /Details/ });
    expect(toggle.getAttribute('aria-expanded')).toBe('false');
    expect(toggle.getAttribute('aria-controls')).toBe('git-banner-details');

    fireEvent.click(toggle);
    expect(toggle.getAttribute('aria-expanded')).toBe('true');
    // The load-bearing sentence: SSH remotes keep working (ratified decision 5).
    expect(screen.getByText(/Remotes you connect to over SSH also keep working/)).toBeInTheDocument();
    expect(screen.getByText(/signing in to HTTPS remotes/)).toBeInTheDocument();
    // …and the BANNER'S OWN prose never says "authentication", "credential
    // helper" or "cached credentials" (UI §5.6). Scoped to the collapsed text
    // column on purpose: §5.4's "Doesn't work" row names the credential helper
    // in plain language deliberately, and §5.5's technical block quotes the Rust
    // payload verbatim so it can be pasted into a bug report.
  });

  it('the collapsed prose avoids jargon and never blames authentication', () => {
    const { container } = renderBanner({ status: MISSING });
    const prose = container.querySelector('.git-banner-text')?.textContent ?? '';
    expect(prose).not.toMatch(/authentication/i);
    expect(prose).not.toMatch(/credential helper/i);
    expect(prose).not.toMatch(/cached credentials/i);
    expect(prose.length).toBeGreaterThan(0);
  });

  it('there is no dismiss control at all — not even a disabled one', () => {
    renderBanner({ status: MISSING });
    const labels = screen.getAllByRole('button').map((b) => b.textContent ?? '');
    expect(labels.some((l) => /dismiss|close|✕|×/i.test(l))).toBe(false);
  });
});

describe('GitMissingBanner — the announcement tracks the variant (UI §5.7)', () => {
  // The regression guard §5.7 mandates: compare the ANNOUNCED text against the
  // text actually in the DOM, for both variants. Asserting two literals against
  // each other would not have caught the defect this closes (Variant B
  // announced the Variant A diagnosis, with no remedy at all).
  it.each([
    ['Variant A', MISSING],
    ['Variant B', UNRUNNABLE],
  ])('%s: the announcement starts with the rendered title and ends with the rendered remedy', (
    _label,
    status,
  ) => {
    const { container } = renderBanner({ status });
    const announced = container.querySelector('.git-banner-announce')?.textContent ?? '';
    const title = container.querySelector('.git-banner-title')?.textContent ?? '';
    const remedy = container.querySelector('.git-banner-remedy')?.textContent ?? '';

    expect(title).not.toBe('');
    expect(remedy).not.toBe('');
    expect(announced.startsWith(title)).toBe(true);
    expect(announced.endsWith(remedy)).toBe(true);
    // …and it carries the remedy, which is the half the old constant dropped.
    expect(announced).toContain(remedy);
  });

  it('Variant B never reads the resolved path aloud (a 250-char path is hostile)', () => {
    const { container } = renderBanner({ status: UNRUNNABLE });
    const announced = container.querySelector('.git-banner-announce')?.textContent ?? '';
    expect(container.querySelector('.git-banner-path')?.textContent).toContain(UNRUNNABLE.path);
    expect(announced).not.toContain(UNRUNNABLE.path);
    expect(announced).not.toContain('Tried:');
  });

  it('re-announces when a latch-only Variant A is replaced by a Variant B status', () => {
    const { container, rerender } = render(
      <GitMissingBanner git={state({ status: null, latched: true })} onGitAvailable={vi.fn()} />,
    );
    expect(container.querySelector('.git-banner-announce')?.textContent).toContain(
      'Git is not available.',
    );
    rerender(
      <GitMissingBanner
        git={state({ status: UNRUNNABLE, latched: true })}
        onGitAvailable={vi.fn()}
      />,
    );
    expect(container.querySelector('.git-banner-announce')?.textContent).toContain(
      "Git couldn't be started.",
    );
  });

  it('a successful re-check announces the recovery WITH the version', async () => {
    const recheck = vi.fn().mockResolvedValue(FOUND);
    const { container } = renderBanner({ status: MISSING, recheck });
    fireEvent.click(screen.getByRole('button', { name: 'Re-check' }));
    await waitFor(() =>
      expect(container.querySelector('.git-banner-announce')?.textContent).toBe(
        'Git is available. Bonsai found Git 2.47.1.',
      ),
    );
  });
});
