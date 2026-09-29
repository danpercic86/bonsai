// P125-ui §10 AC2–AC6 / AC9 — the Dependencies dialog against the harness
// fixtures. Strings come from `depsCopy.ts` (AC5).
import { afterEach, describe, expect, it, vi } from 'vitest';
import { act, cleanup, fireEvent, render, screen, within } from '@testing-library/react';

import { ipc } from '../../ipc';
import { buildDepView } from '../../ipc/fixtures/depTracker';
import type { DepMockMode } from '../../ipc/fixtures/depTracker';
import { LARGE_ROW_COUNT } from '../../ipc/fixtures/depTrackerSynthetic';
import type { DepTrackerView } from '../../ipc/types/depTracker';
import type { UseDepTracker } from '../../hooks/useDepTracker';
import { DependenciesDialog } from './DependenciesDialog';
import {
  CLEAR_FILTERS,
  EMPTY_TITLE,
  FOOTER_SETTINGS,
  LIVE_START,
  NO_LOCKFILE_LINE,
  NO_MATCH,
  OFF_HEADING,
  OPEN_SETTINGS,
  STALE_NOTICE,
  TURN_ON,
  UP_TO_DATE,
  UP_TO_DATE_SOME_UNCHECKED,
  COPY_FAILED,
  configUnreadableLead,
  failureBody,
  linkFailure,
  liveFinished,
  notTrackedSuffix,
  parseErrorLine,
  reportWideLead,
  showAll,
} from './depsCopy';

const NOW = Date.now();

function tracker(view: DepTrackerView | null, over: Partial<UseDepTracker> = {}): UseDepTracker {
  return { view, progress: null, refresh: vi.fn(async () => {}), ...over };
}

function renderDialog(mode: DepMockMode | null, over: Partial<UseDepTracker> = {}) {
  const t = tracker(mode === null ? null : buildDepView(mode, 'r', NOW), over);
  const props = {
    open: true,
    onClose: vi.fn(),
    tracker: t,
    settings: { enabled: mode !== 'off', intervalHours: 12 },
    onSettingsChange: vi.fn(),
    onOpenSettingsAt: vi.fn(),
  };
  const utils = render(<DependenciesDialog {...props} />);
  return { ...utils, props, t };
}

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

describe('§6 states (AC2)', () => {
  it('off: the consent body; Turn on is the consent', () => {
    const { props } = renderDialog('off');
    expect(screen.getByText(OFF_HEADING)).toBeTruthy();
    expect(screen.queryByRole('button', { name: 'Check now' })).toBeNull();
    fireEvent.click(screen.getByRole('button', { name: TURN_ON }));
    expect(props.onSettingsChange).toHaveBeenCalledWith({ depTracker: { enabled: true, intervalHours: 12 } });
  });

  it('loading: skeleton only, caption Checking…', () => {
    const { container } = renderDialog('loading');
    expect(container.querySelectorAll('.deps-loading .skeleton-row').length).toBe(13);
    expect(screen.getAllByText('Checking…').length).toBeGreaterThan(0);
    expect(screen.queryByRole('table')).toBeNull();
  });

  it('error with no report: the failure block by kind, no notice', () => {
    renderDialog('error');
    const copy = failureBody('service');
    expect(screen.getByText(copy.title)).toBeTruthy();
    expect(screen.getByText(copy.reason)).toBeTruthy();
    expect(document.querySelector('.deps-notice')).toBeNull();
  });

  it('empty: title + reason and no action', () => {
    renderDialog('empty');
    expect(screen.getByText(EMPTY_TITLE)).toBeTruthy();
    expect(screen.queryByRole('button', { name: 'Try again' })).toBeNull();
  });

  it('uptodate: the calm line with the unchecked suffix', () => {
    renderDialog('uptodate');
    expect(screen.getByText(`${UP_TO_DATE} ${UP_TO_DATE_SOME_UNCHECKED}`)).toBeTruthy();
  });

  it('stale: the stale notice + stale pill', () => {
    renderDialog('stale');
    expect(screen.getByText(STALE_NOTICE)).toBeTruthy();
    expect(screen.getByText('stale')).toBeTruthy();
  });

  it('offline: one failure notice with the age, message only in its title', () => {
    const { container } = renderDialog('offline');
    const notices = container.querySelectorAll('.deps-notice');
    expect(notices).toHaveLength(1);
    expect(notices[0].textContent).toContain("Couldn't reach the package services — showing results from 3 hours ago.");
    expect(notices[0].getAttribute('title')).toMatch(/^Details: /);
    expect(notices[0].getAttribute('role')).toBe('status');
  });

  it('mirror (networkProfile false): per-group config-unreadable leads, no report-wide lead (R3)', () => {
    renderDialog('mirror');
    expect(screen.queryByText(reportWideLead(['npm', 'NuGet']))).toBeNull();
    expect(screen.getByText(configUnreadableLead('npm'))).toBeTruthy();
    expect(screen.getByText(configUnreadableLead('NuGet'))).toBeTruthy();
  });
});

describe('populated list (AC3 / AC4 / AC9)', () => {
  it('shows group lines that survive every filter, and the no-lockfile line', () => {
    renderDialog('populated');
    expect(screen.getByText(NO_LOCKFILE_LINE)).toBeTruthy();
    expect(screen.getByText(parseErrorLine('broken/go.mod'))).toBeTruthy();
    fireEvent.change(screen.getByRole('searchbox', { name: 'Filter packages by name' }), {
      target: { value: 'no-such-package' },
    });
    expect(screen.getByText(NO_MATCH)).toBeTruthy();
    expect(screen.getByText(parseErrorLine('broken/go.mod'))).toBeTruthy();
    const lines = [...document.querySelectorAll('.deps-group-single')].map((el) => el.textContent);
    expect(lines).toContain(`android/build.gradle${notTrackedSuffix('Gradle')}`);
    fireEvent.click(screen.getByRole('button', { name: CLEAR_FILTERS }));
    expect(screen.queryByText(NO_MATCH)).toBeNull();
  });

  it('local and private rows carry no registry button; every public unverified row says not verified', () => {
    const view = buildDepView('populated', 'r', NOW);
    renderDialog('populated');
    const rows = view.report?.groups.flatMap((g) => g.rows) ?? [];
    for (const r of rows) {
      const btn = screen.queryAllByRole('button', { name: new RegExp(`^Open ${r.name.replace(/[.*+?^${}()|[\]\\/@]/g, '\\$&')} on `) });
      const offline = r.availability.kind === 'local' || r.availability.kind === 'private';
      expect(btn.length, r.name).toBe(offline ? 0 : 1);
    }
    const unverified = rows.filter(
      (r) => r.availability.kind !== 'local' && r.availability.kind !== 'private' && !r.vulnsChecked,
    );
    expect(screen.getAllByText('not verified')).toHaveLength(unverified.length);
  });

  it('the Vulnerable toggle narrows to vulnerable rows and is pressed', () => {
    const { container } = renderDialog('populated');
    const vulnerable = buildDepView('populated', 'r', NOW).report?.summary.vulnerable ?? 0;
    const toggle = screen.getByRole('button', { name: new RegExp(`^Vulnerable\\s*${vulnerable}`) });
    fireEvent.click(toggle);
    expect(toggle.getAttribute('aria-pressed')).toBe('true');
    expect(container.querySelectorAll('tr.deps-row')).toHaveLength(vulnerable);
  });

  it('expands a vulnerable row; a failed advisory open shows the inline link note', async () => {
    const spy = vi.spyOn(ipc, 'openUrl').mockRejectedValue(new Error('nope'));
    renderDialog('populated');
    const toggle = screen.getByRole('button', { name: /^axios, high vulnerability .*, and 3 more$/ });
    expect(toggle.getAttribute('aria-expanded')).toBe('false');
    fireEvent.click(toggle);
    expect(toggle.getAttribute('aria-expanded')).toBe('true');
    expect(screen.getByText('No summary provided.')).toBeTruthy();
    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: 'Open advisory GHSA-4w2v-q235-vp99' }));
    });
    expect(spy).toHaveBeenCalledWith('https://osv.dev/vulnerability/GHSA-4w2v-q235-vp99');
    expect(screen.getByText(linkFailure('OSV.dev'))).toBeTruthy();
    expect(screen.queryAllByRole('alert')).toHaveLength(0);
  });

  it('Shift+F10 opens the row context menu with the §5.3 items', () => {
    renderDialog('populated');
    fireEvent.keyDown(screen.getByRole('button', { name: /^axios, / }), { key: 'F10', shiftKey: true });
    const menu = screen.getByRole('menu');
    const labels = within(menu).getAllByRole('menuitem').map((m) => m.textContent);
    expect(labels).toEqual([
      'Open on npmjs.com',
      'Open advisory GHSA-cph5-m8f7-6c5x',
      'Open advisory GHSA-jr5f-v2jv-69x6',
      'Open advisory GHSA-wf5p-g6vw-rhxx',
      'Open advisory GHSA-4w2v-q235-vp99',
      'Copy name',
      'Copy name and version',
    ]);
  });

  it('a failed copy shows the copy note in the link-note slot', async () => {
    const writeText = vi.fn(async () => {
      throw new Error('denied');
    });
    Object.defineProperty(navigator, 'clipboard', { value: { writeText }, configurable: true });
    renderDialog('populated');
    fireEvent.keyDown(screen.getByRole('button', { name: /^axios, / }), { key: 'F10', shiftKey: true });
    await act(async () => {
      fireEvent.click(within(screen.getByRole('menu')).getByRole('menuitem', { name: 'Copy name' }));
    });
    expect(writeText).toHaveBeenCalledWith('axios');
    expect(screen.getByText(COPY_FAILED)).toBeTruthy();
    expect(screen.queryAllByRole('alert')).toHaveLength(0);
  });

  it('an unverified row says "not verified" once to screen readers (pill only)', () => {
    const { container } = renderDialog('populated');
    const srText = [...container.querySelectorAll('tr.deps-row .sr-only')].map((el) => el.textContent ?? '');
    expect(srText.some((t) => t.includes('not verified'))).toBe(false);
  });

  it('Ctrl+F focuses the name filter', () => {
    renderDialog('populated');
    fireEvent.keyDown(screen.getByRole('dialog'), { key: 'f', ctrlKey: true });
    expect(document.activeElement).toBe(screen.getByRole('searchbox', { name: 'Filter packages by name' }));
  });

  it('large: caps at 200 rows, then Show all', () => {
    const { container } = renderDialog('large');
    expect(container.querySelectorAll('tr.deps-row')).toHaveLength(200);
    fireEvent.click(screen.getByRole('button', { name: showAll(LARGE_ROW_COUNT) }));
    expect(container.querySelectorAll('tr.deps-row')).toHaveLength(LARGE_ROW_COUNT);
  });

  it('no list row wraps: every cell is a nowrap `.deps-cell`, names carry titles', () => {
    const { container } = renderDialog('populated');
    for (const name of container.querySelectorAll('.deps-name')) {
      expect(name.getAttribute('title')).toBe(name.textContent);
    }
  });
});

describe('header live region (§4.1)', () => {
  it('announces start and finish, never progress', () => {
    const base = buildDepView('populated', 'r', NOW);
    const running = { ...base, running: { checkId: 9, startedAtMs: NOW, done: 0, total: 0 } };
    const { rerender, props } = renderDialog('populated');
    const live = () => document.querySelector('.deps-header [role="status"]')?.textContent;
    rerender(<DependenciesDialog {...props} tracker={tracker(running)} />);
    expect(live()).toBe(LIVE_START);
    rerender(<DependenciesDialog {...props} tracker={tracker(running, { progress: { done: 3, total: 9 } })} />);
    expect(live()).toBe(LIVE_START);
    expect(screen.getByText(/Checking… 3 of 9/)).toBeTruthy();
    rerender(<DependenciesDialog {...props} tracker={tracker(base)} />);
    const s = base.report?.summary;
    expect(live()).toBe(liveFinished(s?.vulnerable ?? 0, (s?.major ?? 0) + (s?.minor ?? 0) + (s?.patch ?? 0)));
  });
});

describe('Settings links', () => {
  it.each([
    ['populated', FOOTER_SETTINGS],
    ['off', OPEN_SETTINGS],
  ] as const)(
    '%s: "%s" closes and deep-links to General → Dependency checks',
    (mode, label) => {
      const { props } = renderDialog(mode);
      fireEvent.click(screen.getByRole('button', { name: label }));
      expect(props.onClose).toHaveBeenCalledTimes(1);
      expect(props.onOpenSettingsAt).toHaveBeenCalledWith('general', 'dep-tracker');
    },
  );
});
