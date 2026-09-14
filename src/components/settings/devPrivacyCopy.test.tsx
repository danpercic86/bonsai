// P91-privacy-copy-ui §5 / AC4 — the consent copy is load-bearing, so it is pinned by
// test rather than by review. Three surfaces state a "never" list; all three must carry
// the SAME five clauses in the SAME order (§5's canonical wording and provenance). Two
// divergent lists in one flow is the exact condition that produced the original defect
// (§10.5), so an edit to one surface alone must fail here.

import { cleanup, render } from '@testing-library/react';
import type { ReactElement } from 'react';
import { describe, expect, it } from 'vitest';
import type { LogSessionInfo } from '../../ipc';
import type { RedactionMode } from '../../ipc/types/settings';
import { ExportConfirmDialog, RawNamesConfirmDialog } from './DevConfirmDialogs';
import { SettingsDevPrivacySection } from './SettingsDevPrivacySection';

const noop = () => {};

function sessionInfo(redaction: RedactionMode): LogSessionInfo {
  return {
    sessionId: 'sess-1',
    dir: '/home/dev/.local/share/bonsai/logs',
    files: ['bonsai-20260903-000000.jsonl'],
    bytes: 1024,
    records: 42,
    anomalies: 0,
    dropped: 0,
    redaction,
    salt: '0123456789abcdef0123456789abcdef',
    totalFiles: 3,
    totalBytes: 1024,
    droppedParts: 0,
    writeFailed: false,
    exportFiles: 0,
    exportBytes: 0,
  };
}

/** Prose as a screen reader gets it: JSX line wrapping collapsed away. */
function proseOf(root: HTMLElement): string {
  return (root.textContent ?? '').replace(/\s+/g, ' ').trim();
}

/**
 * Dialogs portal into `document.body`, so a second `render` in one test would also see
 * the first one's nodes. Each helper tears down before returning the text it measured.
 */
function readSurface(node: ReactElement, fromBody: boolean): string {
  const view = render(node);
  const prose = proseOf(fromBody ? (view.baseElement as HTMLElement) : view.container);
  view.unmount();
  cleanup();
  return prose;
}

function panelProse(rawNames: boolean): string {
  return readSurface(<SettingsDevPrivacySection rawNames={rawNames} />, false);
}

function rawNamesDialogProse(): string {
  return readSurface(<RawNamesConfirmDialog open onConfirm={noop} onCancel={noop} />, true);
}

function exportDialogProse(redaction: RedactionMode): string {
  return readSurface(
    <ExportConfirmDialog
      open
      info={sessionInfo(redaction)}
      busy={false}
      onConfirm={noop}
      onCancel={noop}
    />,
    true,
  );
}

type Surface = 'panel' | 'rawNamesDialog' | 'exportRaw' | 'exportStrict';

function proseFor(surface: Surface): string {
  switch (surface) {
    case 'panel':
      return panelProse(false);
    case 'rawNamesDialog':
      return rawNamesDialogProse();
    case 'exportRaw':
      return exportDialogProse('raw');
    case 'exportStrict':
      return exportDialogProse('strict');
  }
}

/**
 * §5's five clauses, in order, in each surface's own contracted wording. The wording
 * differs per surface by design; the coverage and the ORDER may not.
 */
const NEVER_LIST: ReadonlyArray<readonly [Surface, readonly string[]]> = [
  [
    'panel',
    [
      'your commit messages',
      'your search text or anything else you type',
      'the contents of your files or diffs',
      'the name and email address you commit under',
      'any password, access token or key',
    ],
  ],
  [
    'rawNamesDialog',
    [
      'Commit messages',
      'search text and other text you write',
      'the contents of your files',
      'the name and email address you commit under',
      'any password, access token or key',
    ],
  ],
  [
    'exportRaw',
    [
      'Commit messages',
      'search text and anything else you typed',
      'file contents',
      'the name and email address you commit under',
      'any password or access token',
    ],
  ],
  [
    'exportStrict',
    [
      'Commit messages',
      'search text and anything else you typed',
      'file contents',
      'the name and email address you commit under',
      'any password or access token',
    ],
  ],
];

describe('dev-mode privacy consent copy', () => {
  it('states all five §5 "never" clauses, in order, on every surface', () => {
    for (const [surface, clauses] of NEVER_LIST) {
      const prose = proseFor(surface);
      let cursor = -1;
      for (const clause of clauses) {
        const at = prose.indexOf(clause, cursor + 1);
        expect(at, `${surface}: missing or out-of-order clause "${clause}"`).toBeGreaterThan(cursor);
        cursor = at;
      }
    }
  });

  it('never says "arguments" — §1 replaced it with the identifier list', () => {
    for (const [surface] of NEVER_LIST) {
      expect(proseFor(surface).toLowerCase(), surface).not.toContain('argument');
    }
  });

  it('discloses remotes, full commit IDs and the account name in raw mode', () => {
    const dialog = rawNamesDialogProse();
    expect(dialog).toContain('your remote addresses');
    expect(dialog).toContain('full commit IDs');
    expect(dialog).toContain('A folder path may include your computer account name.');
    expect(dialog).toContain('It never adds anything you typed.');

    const panel = panelProse(false);
    expect(panel).toContain('your remote addresses (with any username or password removed)');
    expect(panel).toContain('commit IDs are shortened');
  });

  it('keeps the placeholder ordinals in a mono span', () => {
    const panel = render(<SettingsDevPrivacySection rawNames={false} />);
    // ¶7 (§F6) adds exactly ONE more `.mono` span, and it is the folder name.
    expect(Array.from(panel.container.querySelectorAll('.mono'), (el) => el.textContent)).toEqual([
      'ref#3',
      'path#7',
      'remote#1',
      'repo#1',
      'metrics',
    ]);
    panel.unmount();
    cleanup();

    const dialog = render(<RawNamesConfirmDialog open onConfirm={noop} onCancel={noop} />);
    expect(
      Array.from(dialog.baseElement.querySelectorAll('.mono'), (el) => el.textContent),
    ).toEqual(['ref#3', 'path#7']);
  });

  it('makes the one enforcement claim, and does not escalate it', () => {
    const panel = panelProse(false);
    expect(panel).toContain(
      'Bonsai drops these where the file is written, so the rule holds in both modes.',
    );
    // §5: the claim is deliberately bounded — never "impossible"/"guaranteed".
    expect(panel).not.toMatch(/impossible|guaranteed|never can be/i);
  });

  // §F6 (user ruling 2026-09-11) REVERSED the premise of the test that stood
  // here: the panel used to be asserted NOT to mention the usage count, because
  // §6 was a pending decision. It is now ruled and the disclosure is mandatory,
  // so the assertion is inverted rather than deleted — that inversion is the
  // clearest signal in the suite that the scope changed deliberately.
  it('discloses the usage count, in two sentences (§6.2, signed verbatim)', () => {
    const panel = panelProse(false);
    expect(panel).toContain('Bonsai also keeps a small usage count.');
    expect(panel).toContain('whether or not Dev mode is on');
    // The window, and the clause that is true only because `lifetime` survives it.
    expect(panel).toContain('The day-by-day detail is kept for 90 days, and a running total');
    // The FOLDER, never the file: deleting `usage.json` alone is undone by `.bak`.
    expect(panel).toContain('folder beside your log files');
    expect(panel).not.toContain('usage.json');
    // The quoted label must name the control that exists (§6.2's byte-identical
    // pair with the row label in `SettingsDevLogsSection.tsx`).
    expect(panel).toContain('“Delete logs and usage counts” below clears all of it.');
  });

  // The trap §6.1.1 exists to stop: durations are Dev-mode ONLY
  // (`metrics.rs:148-152`). A single sentence claiming always-on counts AND
  // always-on timings would be an overclaim in the app's only privacy surface.
  it('never claims the durations are always-on', () => {
    const panel = panelProse(false);
    const always = panel.slice(
      panel.indexOf('From the first time you open it'),
      panel.indexOf('With Dev mode on it also records'),
    );
    expect(always).not.toMatch(/how long/);
    expect(panel).toContain('With Dev mode on it also records how long its own actions took.');
  });

  it('is poll-independent: only the "Right now:" note varies (§8.4 stickiness)', () => {
    const strict = panelProse(false);
    expect(panelProse(false)).toBe(strict);
    expect(strict.replace('Right now: names are replaced.', '')).toBe(
      panelProse(true).replace('⚠Right now: raw names are included.', ''),
    );
  });

  it('adds no aria-live to the privacy block — §8 leaves announcing to the page region', () => {
    const { container } = render(<SettingsDevPrivacySection rawNames />);
    expect(container.querySelector('[aria-live]')).toBeNull();
    expect(container.querySelector('[role="group"]')).not.toBeNull();
  });

  it('keeps the warning bar on the raw export branch only', () => {
    const raw = render(
      <ExportConfirmDialog
        open
        info={sessionInfo('raw')}
        busy={false}
        onConfirm={noop}
        onCancel={noop}
      />,
    );
    expect(raw.baseElement.querySelector('.dev-warning-bar')).not.toBeNull();
    raw.unmount();
    cleanup();

    const strict = render(
      <ExportConfirmDialog
        open
        info={sessionInfo('strict')}
        busy={false}
        onConfirm={noop}
        onCancel={noop}
      />,
    );
    expect(strict.baseElement.querySelector('.dev-warning-bar')).toBeNull();
  });
});
