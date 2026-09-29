// P125 spec AC4 / AC6 / AC7 as RENDERED by the Dependencies dialog over the
// `?deps=populated` fixture: the "checked <age>" caption, advisory id +
// severity and the transitive tag on vulnerable rows, and the local /
// private pills. (The pure helpers are unit-tested in depsView.test.ts.)
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { cleanup, render, screen } from '@testing-library/react';

import { buildDepView } from '../../ipc/fixtures/depTracker';
import type { DepRow, DepTrackerView } from '../../ipc/types/depTracker';
import { formatAge } from '../../lib/formatAge';
import { DependenciesDialog } from './DependenciesDialog';
import { SEVERITY_LABEL, TAG_TRANSITIVE, checkedCaption, rowToggleName } from './depsCopy';
import { availabilityWords } from './depsView';

const NOW = Date.UTC(2026, 8, 29, 12, 0, 0);

function renderView(view: DepTrackerView) {
  return render(
    <DependenciesDialog
      open
      onClose={vi.fn()}
      tracker={{ view, progress: null, refresh: vi.fn(async () => {}) }}
      settings={{ enabled: true, intervalHours: 12 }}
      onSettingsChange={vi.fn()}
      onOpenSettingsAt={vi.fn()}
    />,
  );
}

function rowsOf(view: DepTrackerView): DepRow[] {
  return view.report?.groups.flatMap((g) => g.rows) ?? [];
}

/** Every rendered `<tr>` whose name cell carries `name`. */
function trsFor(container: HTMLElement, name: string): HTMLTableRowElement[] {
  return [...container.querySelectorAll<HTMLElement>('.deps-name')]
    .filter((el) => el.getAttribute('title') === name)
    .map((el) => el.closest('tr'))
    .filter((tr): tr is HTMLTableRowElement => tr !== null);
}

beforeEach(() => {
  vi.useFakeTimers({ toFake: ['Date'] });
  vi.setSystemTime(NOW);
});

afterEach(() => {
  cleanup();
  vi.useRealTimers();
});

describe('AC7 — the cached report is labelled with its age', () => {
  it('populated: the header caption reads "checked <age>"', () => {
    const view = buildDepView('populated', 'r', NOW);
    const at = view.report?.checkedAtMs;
    expect(at).toBeTypeOf('number');
    const { container } = renderView(view);
    const caption = container.querySelector('.deps-caption');
    expect(caption?.textContent).toBe(checkedCaption(formatAge(at as number, NOW)));
  });

  it('offline (a 3 h-old report) is still labelled with its own age', () => {
    const view = buildDepView('offline', 'r', NOW);
    const { container } = renderView(view);
    expect(container.querySelector('.deps-caption')?.textContent).toBe(checkedCaption('3 hours ago'));
  });
});

describe('AC4 — vulnerable rows show advisory id + severity; transitive rows are labelled', () => {
  it('every vulnerable row names its first advisory and severity', () => {
    const view = buildDepView('populated', 'r', NOW);
    renderView(view);
    const vulnerable = rowsOf(view).filter((r) => r.vulns.length > 0);
    expect(vulnerable.some((r) => r.relation === 'direct')).toBe(true);
    expect(vulnerable.some((r) => r.relation === 'transitive')).toBe(true);
    for (const r of vulnerable) {
      const label = rowToggleName(r.name, SEVERITY_LABEL[r.vulns[0].severity], r.vulns[0].id, r.vulns.length - 1);
      expect(screen.getAllByRole('button', { name: label }).length, r.name).toBeGreaterThan(0);
    }
  });

  it('every transitive row renders the transitive tag; direct normal rows do not', () => {
    const view = buildDepView('populated', 'r', NOW);
    const { container } = renderView(view);
    const tagged = (tr: HTMLElement) =>
      [...tr.querySelectorAll('.deps-tag')].some((t) => t.textContent === TAG_TRANSITIVE);
    const rows = rowsOf(view);
    const transitive = rows.filter((r) => r.relation === 'transitive');
    expect(transitive.length).toBeGreaterThan(0);
    for (const r of transitive) {
      expect(trsFor(container, r.name).some(tagged), r.name).toBe(true);
    }
    for (const r of rows.filter((x) => x.relation === 'direct' && x.scope === 'normal')) {
      if (rows.some((x) => x.name === r.name && x.relation === 'transitive')) continue;
      expect(trsFor(container, r.name).some(tagged), r.name).toBe(false);
    }
  });
});

describe('AC6 — local and private rows read as such', () => {
  it('every local / private row shows its availability pill and no version columns', () => {
    const view = buildDepView('populated', 'r', NOW);
    const { container } = renderView(view);
    const offline = rowsOf(view).filter((r) => r.availability.kind === 'local' || r.availability.kind === 'private');
    expect(offline.some((r) => r.availability.kind === 'local')).toBe(true);
    expect(offline.some((r) => r.availability.kind === 'private')).toBe(true);
    for (const r of offline) {
      const pill = availabilityWords(r, false)?.pill;
      expect(pill, r.name).toBeTruthy();
      const trs = trsFor(container, r.name);
      expect(trs.length, r.name).toBeGreaterThan(0);
      expect(
        trs.some((tr) => tr.textContent?.includes(pill as string)),
        `${r.name} shows "${pill}"`,
      ).toBe(true);
      expect(trs.some((tr) => tr.querySelector('.deps-latest--newer, .deps-latest--same') !== null)).toBe(false);
    }
  });
});
