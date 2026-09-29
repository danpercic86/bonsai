// P125-ui §10 AC1 — name/title per §3 state, the vulnerability-only chip, and
// the §1 toolbar order.
import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, fireEvent, render, screen } from '@testing-library/react';

import { buildDepView } from '../ipc/fixtures/depTracker';
import { mockIpc } from '../ipc/mock';
import { resetEffectiveIdentityForTests } from '../hooks/useEffectiveIdentity';
import { DepsToolbarButton } from './DepsToolbarButton';
import { HeaderToolbar } from './HeaderToolbar';
import type { HeaderToolbarProps } from './HeaderToolbar';
import type { ConfigView } from '../ipc';

const NOW = 1_800_000_000_000;

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
  resetEffectiveIdentityForTests();
});

describe('DepsToolbarButton', () => {
  it('off: bare name, off title, no chip', () => {
    render(<DepsToolbarButton view={buildDepView('off', 'r', NOW)} onOpen={vi.fn()} nowMs={NOW} />);
    const b = screen.getByRole('button', { name: 'Dependencies' });
    expect(b.getAttribute('title')).toBe('Dependencies — off');
    expect(b.querySelector('.deps-count-chip')).toBeNull();
  });

  it('populated: the chip shows the vulnerable count, aria-hidden, named in the label', () => {
    const view = buildDepView('populated', 'r', NOW);
    const n = view.report?.summary.vulnerable ?? 0;
    const onOpen = vi.fn();
    render(<DepsToolbarButton view={view} onOpen={onOpen} nowMs={NOW} />);
    const b = screen.getByRole('button', { name: `Dependencies, ${n} vulnerable` });
    const chip = b.querySelector('.deps-count-chip');
    expect(chip?.getAttribute('aria-hidden')).toBe('true');
    expect(chip?.textContent).toBe(`⊘${String.fromCharCode(0x2009)}${n}`);
    expect(b.getAttribute('aria-busy')).toBeNull();
    fireEvent.click(b);
    expect(onOpen).toHaveBeenCalledTimes(1);
  });

  it('uptodate: no chip (updates never reach the toolbar)', () => {
    render(<DepsToolbarButton view={buildDepView('uptodate', 'r', NOW)} onOpen={vi.fn()} nowMs={NOW} />);
    expect(document.querySelector('.deps-count-chip')).toBeNull();
  });

  it('re-check: aria-busy on the button, the label says checking again', () => {
    const base = buildDepView('stale', 'r', NOW);
    const view = { ...base, running: { checkId: 3, startedAtMs: NOW, done: 0, total: 0 } };
    render(<DepsToolbarButton view={view} onOpen={vi.fn()} nowMs={NOW} />);
    const b = screen.getByRole('button', { name: /^Dependencies, \d+ vulnerable, checking again$/ });
    expect(b.getAttribute('aria-busy')).toBe('true');
    expect(b.getAttribute('title')?.endsWith(' — checking again…')).toBe(true);
  });
});

describe('HeaderToolbar order (§1)', () => {
  function props(activeRepo: string | null): HeaderToolbarProps {
    return {
      theme: 'dark',
      onToggleTheme: vi.fn(),
      listView: 'tree',
      onToggleListView: vi.fn(),
      activeRepo,
      onOpenAiAssets: vi.fn(),
      onOpenHealth: vi.fn(),
      depsView: null,
      onOpenDeps: vi.fn(),
      onOpenSettings: vi.fn(),
      onOpenSettingsAt: vi.fn(),
      devEnabled: true,
      onMenuOpenChange: vi.fn(),
      profiles: [],
      onProfilesChange: vi.fn(),
    };
  }

  it('dependencies sits between health and dev, only with a repo open', () => {
    const empty: ConfigView = { targetLevel: 'local', curated: [], advanced: [] };
    vi.spyOn(mockIpc, 'getConfig').mockResolvedValue(empty);
    const { rerender } = render(<HeaderToolbar {...props(null)} />);
    expect(screen.queryByRole('button', { name: 'Dependencies' })).toBeNull();
    rerender(<HeaderToolbar {...props('/repo')} />);
    const classes = [...document.querySelectorAll('.header-toolbar > *')].map((el) => el.className);
    const at = (needle: string) => classes.findIndex((c) => c.includes(needle));
    expect(at('repo-health-toggle')).toBeLessThan(at('deps-toggle'));
    expect(at('deps-toggle')).toBeLessThan(at('dev-mode-pill'));
    expect(at('dev-mode-pill')).toBeLessThan(at('settings-toggle'));
  });
});
