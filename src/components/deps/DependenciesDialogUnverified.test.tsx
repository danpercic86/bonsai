// P125-ui §9 (P125h modes) / §10 AC4 — `?deps=unverified` and
// `?deps=netprofile` as RENDERED by the Dependencies dialog. Strings come from
// `depsCopy.ts` (AC5).
import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, fireEvent, render, screen } from '@testing-library/react';

import { buildDepView } from '../../ipc/fixtures/depTracker';
import type { DepMockMode } from '../../ipc/fixtures/depTracker';
import { DependenciesDialog } from './DependenciesDialog';
import {
  AVAIL_NOT_CHECKED,
  INCOMPLETE_MALFORMED,
  INCOMPLETE_TOO_LARGE,
  NOT_VERIFIED,
  NOT_VERIFIED_MALFORMED,
  NOT_VERIFIED_NO_VERSION,
  NOT_VERIFIED_NUGET,
  NOT_VERIFIED_TOO_LARGE,
  NO_SUMMARY,
  PRIVATE_TITLE_LOCAL_FEED,
  UNKNOWN_TITLE,
  combinedVulnTitle,
  configUnreadableLead,
  reportWideLead,
} from './depsCopy';

const NOW = Date.now();

function renderMode(mode: DepMockMode) {
  const view = buildDepView(mode, 'r', NOW);
  const utils = render(
    <DependenciesDialog
      open
      onClose={vi.fn()}
      tracker={{ view, progress: null, refresh: vi.fn(async () => {}) }}
      settings={{ enabled: true, intervalHours: 12 }}
      onSettingsChange={vi.fn()}
      onOpenSettingsAt={vi.fn()}
    />,
  );
  return { ...utils, view };
}

/** The package `<tr>` whose name cell carries `name`. */
function trFor(container: HTMLElement, name: string): HTMLTableRowElement {
  const tr = [...container.querySelectorAll<HTMLElement>('.deps-name')]
    .find((el) => el.getAttribute('title') === name)
    ?.closest('tr');
  if (tr == null) throw new Error(`no row ${name}`);
  return tr;
}

/** The Vulnerability cell (5th of 6; spans shift only on local/private rows). */
function vulnCell(tr: HTMLTableRowElement): HTMLElement {
  const cells = [...tr.cells];
  return cells[cells.length - 2];
}

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

describe('?deps=unverified (P125-ui §9)', () => {
  it('each reason shows `not verified` with its own title; the no-advisory rows have no chevron', () => {
    const { container } = renderMode('unverified');
    const pill = (name: string) => vulnCell(trFor(container, name)).querySelector('.deps-pill');
    // noVersion: the declared range in Installed.
    const lp = trFor(container, 'left-pad');
    expect(lp.cells[1].textContent).toBe('^1.3.0');
    expect(pill('left-pad')?.getAttribute('title')).toBe(NOT_VERIFIED_NO_VERSION);
    // nameCasing: an installed version.
    expect(trFor(container, 'Newtonsoft.Json').cells[1].textContent).toBe('13.0.3');
    expect(pill('Newtonsoft.Json')?.getAttribute('title')).toBe(NOT_VERIFIED_NUGET);
    // All-malformed direct row, vulns [].
    expect(pill('node-forge')?.getAttribute('title')).toBe(NOT_VERIFIED_MALFORMED);
    for (const n of ['left-pad', 'Newtonsoft.Json', 'node-forge', 'minimist']) {
      const tr = trFor(container, n);
      expect(tr.querySelector('.deps-row-toggle'), n).toBeNull();
      expect(vulnCell(tr).textContent, n).toBe(NOT_VERIFIED);
    }
    // Said once to screen readers — by the pill, never an extra sr-only suffix.
    const srText = [...container.querySelectorAll('tr.deps-row .sr-only')].map((el) => el.textContent ?? '');
    expect(srText.some((t) => t.includes(NOT_VERIFIED))).toBe(false);
  });

  it('the R2 transitive row: transitive tag, empty Latest, not verified, not counted as vulnerable', () => {
    const { container, view } = renderMode('unverified');
    const tr = trFor(container, 'minimist');
    expect([...tr.querySelectorAll('.deps-tag')].map((t) => t.textContent)).toEqual(['transitive']);
    expect(tr.cells[2].textContent).toBe('');
    expect(vulnCell(tr).querySelector('.deps-pill')?.getAttribute('title')).toBe(NOT_VERIFIED_MALFORMED);
    const vulnerable = view.report?.summary.vulnerable ?? -1;
    expect(screen.getByRole('button', { name: new RegExp(`^Vulnerable\\s*${vulnerable}`) })).toBeTruthy();
    // Hidden under a filter toggle (no update, no vulns) — intended (§4.4).
    fireEvent.click(screen.getByRole('button', { name: new RegExp(`^Vulnerable\\s*${vulnerable}`) }));
    expect(container.querySelector('.deps-name[title="minimist"]')).toBeNull();
  });

  it('partially malformed: the combined cell, then the malformed note when expanded', () => {
    const { container } = renderMode('unverified');
    const cell = vulnCell(trFor(container, 'ws'));
    expect(cell.querySelector('.deps-vuln')?.getAttribute('title')).toBe(
      combinedVulnTitle('GHSA-3h5v-q93c-6h6q', NOT_VERIFIED_MALFORMED),
    );
    expect([...cell.querySelectorAll('.deps-pill')].map((p) => p.textContent)).toEqual([
      '⊘high',
      NOT_VERIFIED,
    ]);
    // In the combined case the id has no own title: the cell's title covers it.
    expect(cell.querySelector('.deps-vuln-id')?.hasAttribute('title')).toBe(false);
    fireEvent.click(screen.getByRole('button', { name: /^ws, high vulnerability GHSA-3h5v-q93c-6h6q$/ }));
    const note = container.querySelector('.deps-advisory-note');
    expect(note?.textContent).toBe(`${NOT_VERIFIED}${INCOMPLETE_MALFORMED}`);
    expect(container.querySelector('.deps-advisory:last-child')).toBe(note);
  });

  it('responseTooLarge: the placeholder advisory reads unrated; the note follows `No summary provided.`', () => {
    const { container } = renderMode('unverified');
    const cell = vulnCell(trFor(container, 'lodash'));
    expect(cell.querySelector('.deps-vuln')?.getAttribute('title')).toBe(
      combinedVulnTitle('GHSA-35jh-r3h4-6jhm', NOT_VERIFIED_TOO_LARGE),
    );
    expect([...cell.querySelectorAll('.deps-pill')].map((p) => p.textContent)).toEqual(['unrated', NOT_VERIFIED]);
    fireEvent.click(screen.getByRole('button', { name: /^lodash, unrated vulnerability / }));
    expect(screen.getByText(NO_SUMMARY)).toBeTruthy();
    expect(screen.getByText(INCOMPLETE_TOO_LARGE)).toBeTruthy();
  });

  it('unknown/tooLarge keeps its own Vulnerability cell and registry button; localFeed has neither', () => {
    const { container } = renderMode('unverified');
    const unk = trFor(container, 'aws-sdk');
    expect(unk.textContent).toContain('too large to look up');
    expect(unk.querySelector('.deps-avail')?.getAttribute('title')).toBe(UNKNOWN_TITLE.tooLarge);
    expect(screen.getByRole('button', { name: 'Open aws-sdk on npmjs.com' })).toBeTruthy();
    const feed = trFor(container, 'Corp.Internal.Utils');
    expect(feed.textContent).toContain('private — not checked');
    expect(feed.textContent).toContain('folder feed');
    expect(feed.querySelector('.deps-avail')?.getAttribute('title')).toBe(PRIVATE_TITLE_LOCAL_FEED);
    expect(feed.querySelector('.deps-row-action')).toBeNull();
  });
});

describe('?deps=netprofile (P125-ui §9, R3)', () => {
  it('exactly one report-wide lead, no per-group leads, every row `not checked` without a title', () => {
    const { container } = renderMode('netprofile');
    const leads = container.querySelectorAll('.deps-lead');
    expect(leads).toHaveLength(1);
    expect(leads[0].textContent).toBe(reportWideLead(['npm', 'PyPI', 'NuGet']));
    for (const eco of ['npm', 'PyPI', 'NuGet']) expect(screen.queryByText(configUnreadableLead(eco))).toBeNull();
    const avails = [...container.querySelectorAll('.deps-avail')];
    expect(avails.length).toBe(7);
    for (const a of avails) {
      expect(a.textContent).toBe(AVAIL_NOT_CHECKED);
      expect(a.getAttribute('title')).toBeNull();
    }
    expect(container.querySelectorAll('.deps-row-action')).toHaveLength(0);
    expect(screen.getByRole('button', { name: /^Vulnerable\s*0/ }).hasAttribute('disabled')).toBe(true);
  });
});
