/** T3.7 — App shell smoke suite. High-value shell behaviors only (not every
 *  useState): boots without crashing under mock IPC, renders the empty state,
 *  applies the theme to <html>, first-run onboarding overlay + its persistence,
 *  theme toggle, boot resilience (a failed session reopen warns + still renders),
 *  and mounting a workspace tab from a persisted session. IPC is stubbed via
 *  vi.spyOn(mockIpc, …) so boot is deterministic. */
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, fireEvent, waitFor, within } from '@testing-library/react';
import App from './App';
import { mockIpc } from './ipc/mock';
import { DEFAULT_UI_SETTINGS } from './ipc/mock/persistence';
import type { SessionState, UiSettings } from './ipc';

function ui(over: Partial<UiSettings> = {}): UiSettings {
  return { ...structuredClone(DEFAULT_UI_SETTINGS), ...over };
}

const EMPTY_SESSION: SessionState = { openRepos: [], activeRepo: null };

/** Stub the boot-time reads. `settings`/`session` default to a clean, no-repo,
 *  onboarding-already-seen launch. Returns the spies the caller may assert on. */
function stubBoot(opts: { settings?: UiSettings; session?: SessionState } = {}) {
  vi.spyOn(mockIpc, 'getUiSettings').mockResolvedValue(opts.settings ?? ui({ onboardingSeen: true }));
  vi.spyOn(mockIpc, 'getRecentRepos').mockResolvedValue([]);
  vi.spyOn(mockIpc, 'getSession').mockResolvedValue(opts.session ?? EMPTY_SESSION);
  const setUi = vi.spyOn(mockIpc, 'setUiSettings').mockResolvedValue(undefined as never);
  vi.spyOn(mockIpc, 'setSession').mockResolvedValue(undefined as never);
  return { setUi };
}

// Collect genuinely-uncaught errors (a render throw surfaces here in jsdom).
let uncaught: unknown[] = [];
const onError = (e: ErrorEvent) => uncaught.push(e.error ?? e.message);

beforeEach(() => {
  vi.restoreAllMocks();
  uncaught = [];
  window.addEventListener('error', onError);
});
afterEach(() => {
  window.removeEventListener('error', onError);
  expect(uncaught).toEqual([]);
});

describe('App shell', () => {
  it('boots to the empty state and applies the persisted theme to <html>', async () => {
    stubBoot();
    render(<App />);
    expect(await screen.findByText('A tidy Git client')).toBeInTheDocument();
    expect(document.documentElement.getAttribute('data-theme')).toBe('dark');
    // The always-present header toolbar (settings) renders alongside the empty state.
    expect(screen.getByRole('button', { name: 'Settings' })).toBeInTheDocument();
    // No workspace is mounted with zero tabs.
    expect(document.querySelector('.workspace-host')).toBeNull();
  });

  it('shows the first-run onboarding overlay when onboardingSeen is false', async () => {
    stubBoot({ settings: ui({ onboardingSeen: false }) });
    render(<App />);
    expect(
      await screen.findByRole('dialog', { name: 'Welcome to Bonsai' }),
    ).toBeInTheDocument();
  });

  it('dismissing onboarding persists onboardingSeen and removes the overlay', async () => {
    const { setUi } = stubBoot({ settings: ui({ onboardingSeen: false }) });
    render(<App />);
    const dialog = await screen.findByRole('dialog', { name: 'Welcome to Bonsai' });
    fireEvent.click(within(dialog).getByRole('button', { name: 'Close' }));
    await waitFor(() =>
      expect(screen.queryByRole('dialog', { name: 'Welcome to Bonsai' })).not.toBeInTheDocument(),
    );
    // P69b: `onboardingSeen` no longer writes inline — it rides the shared 300 ms
    // coalescing window in useUiSettings, so the write lands just after dismissal.
    await waitFor(() =>
      expect(setUi).toHaveBeenCalledWith(expect.objectContaining({ onboardingSeen: true })),
    );
  });

  it('the theme toggle flips the document theme', async () => {
    stubBoot();
    render(<App />);
    await screen.findByText('A tidy Git client');
    expect(document.documentElement.getAttribute('data-theme')).toBe('dark');
    fireEvent.click(screen.getByRole('button', { name: 'Switch to light theme' }));
    expect(document.documentElement.getAttribute('data-theme')).toBe('light');
  });

  it('a failed session reopen surfaces a warning toast and still renders the shell', async () => {
    // The mock openRepo rejects for any path containing "error".
    stubBoot({ session: { openRepos: ['/mock/error-me'], activeRepo: '/mock/error-me' } });
    render(<App />);
    // The boot warning toast appears…
    expect(await screen.findByText(/Could not reopen/)).toBeInTheDocument();
    // …and, with no tab opened, the empty state is shown (shell survived).
    expect(screen.getByText('A tidy Git client')).toBeInTheDocument();
  });

  it('mounts a workspace tab from a persisted session (no empty state)', async () => {
    stubBoot({ session: { openRepos: ['/mock/repo'], activeRepo: '/mock/repo' } });
    render(<App />);
    // A workspace host mounts; activeRepo != null lights the repo-only toolbar buttons.
    await waitFor(() => expect(document.querySelector('.workspace-host')).not.toBeNull());
    expect(screen.getByRole('button', { name: 'AI Assets' })).toBeInTheDocument();
    expect(screen.queryByText('A tidy Git client')).not.toBeInTheDocument();
  });

  // P69b MUST-FIX 1: PaneDivider's Arrow-key path calls onResize AND onResizeEnd
  // in ONE keydown handler, before React re-renders. The persisted width must
  // therefore come from a ref updated eagerly by the resize handler — reading a
  // ref that only refreshes during render persists the PRE-nudge width, so the
  // pane silently sits one 8px nudge behind after a reload.
  it('an Arrow-key pane nudge persists the POST-nudge width', async () => {
    // jsdom's default 1024px window collapses clampLive's dynamic max onto
    // SIDEBAR_MIN, which would mask the off-by-one-nudge bug.
    const realWidth = window.innerWidth;
    Object.defineProperty(window, 'innerWidth', { value: 1600, configurable: true });
    try {
      // PERSISTED widths must differ from DEFAULT_PANE_WIDTHS (App.tsx:64), or the
      // test passes vacuously: with sidebar = 240 either way, a bare
      // `setPaneWidths(s.paneWidths)` at boot (i.e. a ref never seeded from disk)
      // still nudges to 248 and hides the very path the deleted render-time ref
      // assignment used to cover.
      const { setUi } = stubBoot({
        settings: ui({ onboardingSeen: true, paneWidths: { sidebar: 300, rightPanel: 380 } }),
        session: { openRepos: ['/mock/repo'], activeRepo: '/mock/repo' },
      });
      render(<App />);
      await waitFor(() => expect(document.querySelector('.workspace-host')).not.toBeNull());
      const divider = document.querySelector('.pane-divider-sidebar');
      expect(divider).not.toBeNull();

      fireEvent.keyDown(divider as Element, { key: 'ArrowRight' });

      // 300 (persisted) + 8 (KEYBOARD_NUDGE_PX) — neither 300 (stale ref) nor 248
      // (ref stuck on the 240 default because boot never seeded it).
      await waitFor(() =>
        expect(setUi).toHaveBeenCalledWith(
          expect.objectContaining({ paneWidths: { sidebar: 308, rightPanel: 380 } }),
        ),
      );
    } finally {
      Object.defineProperty(window, 'innerWidth', { value: realWidth, configurable: true });
    }
  });
});
