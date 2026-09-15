/**
 * P112 §6 / §7 / §16.4 / §16.4a — the picker's FLOWS: Browse, Rescan, and the
 * four outcomes that reach the page through `useOutcomeNotes`.
 *
 * Two assertions here are about absent things, and both are the defect:
 *   * **UA18** samples the input on every commit between the pick resolving and
 *     the settled state. A final-value assertion cannot see this — the failure
 *     is a TRANSIENT blank, which is what committing the re-read settings before
 *     the refetched scan produces.
 *   * **UA11** checks that no outcome raises a toast. A toast from Settings is
 *     not merely dim: `elementFromPoint` at its own centre returns the overlay,
 *     so its ✕ cannot be clicked at any width.
 */
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { act, cleanup, fireEvent, render, screen } from '@testing-library/react';
import { useLayoutEffect, useState } from 'react';

import { SettingsPanel, type SettingsPanelProps } from '../SettingsPanel';
import { MINIMAL } from './coverageFixtures';
import { resetExternalToolMockForTests } from '../../ipc/mock/handlers/tools';
import { resetExternalToolScanCacheForTests } from './useExternalToolScan';
import type { ToolSelection } from '../../hooks/useUiSettings';

function seamUrl(seam: string): void {
  window.history.replaceState({}, '', seam === '' ? '/' : `/?tools=${seam}`);
}

async function settle(ms = 900): Promise<void> {
  await act(async () => {
    await vi.advanceTimersByTimeAsync(ms);
  });
}

const note = (id: string): HTMLElement | null => document.getElementById(id);
const announcer = (): HTMLElement => {
  const el = screen.getByRole('tabpanel').querySelector<HTMLElement>('.sr-only[role="status"]');
  if (el === null) throw new Error('no announcer');
  return el;
};
const input = (id: string): HTMLInputElement => {
  const el = document.getElementById(id);
  if (!(el instanceof HTMLInputElement)) throw new Error(`no input #${id}`);
  return el;
};
const editorInput = (): HTMLInputElement => {
  const el = document.getElementById('settings-editor-tool');
  if (!(el instanceof HTMLInputElement)) throw new Error('no editor picker');
  return el;
};
const browseEditor = (): HTMLElement =>
  screen.getByRole('button', { name: 'Browse for an editor program' });
const rescan = (): HTMLElement => screen.getByRole('button', { name: 'Rescan' });

/**
 * App's wiring, minimally: the two selections are STATE here, adopted from
 * `onAdoptToolSelection` exactly as `useUiSettings.adoptToolSelection` does —
 * a PARTIAL merge, because the real setter writes only the field the caller
 * names (§16.16-5). A test that passed them as constants could not observe the
 * Browse success path at all.
 *
 * The layout effect samples the input after every commit THIS component makes —
 * which is every commit that changes the selection, i.e. precisely the commits
 * UA18 is about.
 */
function Harness({
  samples,
  initial,
  over,
}: {
  samples: string[];
  initial: { terminalTool: string; editorTool: string };
  over?: Partial<SettingsPanelProps>;
}) {
  const [tools, setTools] = useState(initial);
  useLayoutEffect(() => {
    const el = document.getElementById('settings-editor-tool');
    if (el instanceof HTMLInputElement) samples.push(el.value);
  });
  const props: SettingsPanelProps = {
    open: true,
    initialCategory: 'general',
    onClose: vi.fn(),
    requestSeq: 0,
    onChange: vi.fn(),
    onToggleTheme: vi.fn(),
    onToggleListView: vi.fn(),
    onRequestEnableAi: vi.fn(),
    onSetMcpEnabled: vi.fn(),
    onRequestEnableMcp: vi.fn(),
    onSetMcpAllowWrite: vi.fn(),
    onRequestEnableMcpWrite: vi.fn(),
    onRegisterMcp: vi.fn(async () => {}),
    onShowOnboarding: vi.fn(),
    onOpenRepository: vi.fn(),
    onCheckUpdate: vi.fn(),
    onOpenUpdateDialog: vi.fn(),
    ...MINIMAL,
    ...tools,
    onAdoptToolSelection: (selection: ToolSelection) => {
      setTools((prev) => ({ ...prev, ...selection }));
    },
    ...over,
  };
  return <SettingsPanel {...props} />;
}

function renderHarness(
  initial = { terminalTool: '', editorTool: '' },
  over?: Partial<SettingsPanelProps>,
): { samples: string[] } {
  const samples: string[] = [];
  render(<Harness samples={samples} initial={initial} over={over} />);
  return { samples };
}

beforeEach(() => {
  vi.useFakeTimers();
  resetExternalToolScanCacheForTests();
  resetExternalToolMockForTests();
  window.localStorage.clear();
  seamUrl('');
});

afterEach(() => {
  cleanup();
  vi.useRealTimers();
  vi.restoreAllMocks();
  seamUrl('');
  window.localStorage.clear();
});

describe('P112 §16.4a — Browse confirmed (sites B and G)', () => {
  it('UA18: the input never blanks between the pick and the settled state', async () => {
    seamUrl('custom');
    const { samples } = renderHarness({ terminalTool: '', editorTool: 'vscode' });
    await settle();
    expect(editorInput()).toHaveValue('Visual Studio Code');
    const before = samples.length;

    fireEvent.click(browseEditor());
    await settle();

    // The blank window this guards: the moment `editorTool` becomes `'custom'`
    // the held scan still predates the pick and has no `custom` row, so
    // `options.find(o => o.value === 'custom')` misses and the input renders ''.
    const after = samples.slice(before);
    expect(after.length, 'the selection must actually have changed').toBeGreaterThan(0);
    expect(after).not.toContain('');
    expect(editorInput()).toHaveValue('editor-cli-launcher');
    expect(note('general-editor-tool-note')).toHaveTextContent('Runs C:\\Users\\a.very.long');
  });

  it('site B: announces the new tool, with NO second visible line', async () => {
    seamUrl('custom');
    renderHarness({ terminalTool: '', editorTool: 'vscode' });
    await settle();

    fireEvent.click(browseEditor());
    await settle();

    // Announce-only (§16.4 R2): the visible channel is already complete — the
    // label changed and the state note reads `Runs …`.
    expect(announcer()).toHaveTextContent('Editor set to editor-cli-launcher.');
    expect(document.querySelector('[data-outcome-note="general.editor-tool"]')).toHaveTextContent(
      '',
    );
    // It names the LABEL, never the path (not actionable by voice).
    expect(announcer().textContent).not.toContain('C:\\');
  });

  it('site G: a pick whose follow-up read fails says the choice WAS saved', async () => {
    seamUrl('browseadopterr');
    renderHarness({ terminalTool: '', editorTool: 'vscode' });
    await settle();

    fireEvent.click(browseEditor());
    await settle();

    const outcome = document.querySelector('[data-outcome-note="general.editor-tool"]');
    expect(outcome).toHaveTextContent(
      'Your choice was saved, but Bonsai couldn’t refresh this list. Press Rescan.',
    );
    expect(announcer()).toHaveTextContent('Your choice was saved');
    // The state note is the SECOND tenant's neighbour, never replaced by it —
    // and it still describes the OLD selection, which is what makes
    // `BROWSE_STALE` necessary: the new choice is on disk and not on screen.
    expect(note('general-editor-tool-note')).toHaveTextContent('Runs C:\\Users\\dev');
    expect(editorInput()).toHaveValue('Visual Studio Code');
    expect(document.querySelectorAll('.toast')).toHaveLength(0);
  });

  it('site G: the Rescan the note names really recovers the selection', async () => {
    seamUrl('browseadopterr');
    renderHarness({ terminalTool: '', editorTool: 'vscode' });
    await settle();
    fireEvent.click(browseEditor());
    await settle();
    expect(editorInput()).toHaveValue('Visual Studio Code');

    // `BROWSE_STALE` tells the user to press Rescan. A Rescan refetches the
    // LIST — which by then contains the `custom` row — so without re-reading
    // settings too the picker would still show the old tool and the copy would
    // name an action with no visible effect. (§16.4a specced the string and the
    // report, not the recovery.)
    fireEvent.click(rescan());
    await settle();

    expect(editorInput()).toHaveValue('editor-cli-launcher');
    expect(note('general-editor-tool-note')).toHaveTextContent('Runs C:\\Users\\a.very.long');
    // …and the recovery RETRACTS ITS OWN MESSAGE. An outcome note clears only
    // through `begin(key)` or unmount — there is no auto-dismiss — so an adopt
    // that does not clear the slot it wrote leaves the row showing the recovered
    // label, the `Runs …` path, AND a standing error still saying "Press
    // Rescan." That is the third layer of one defect: the string named a
    // recovery (fixed), the recovery was implemented (correct), and the
    // recovery has to retract the sentence that sent the user here.
    expect(document.querySelector('[data-outcome-note="general.editor-tool"]')).toHaveTextContent(
      '',
    );
    // …and retracting it did not eat the Rescan's own announcement: `begin`
    // blanks the section's ONE announcer (§8.2), so the clear and the counts are
    // issued in the same synchronous block, in that order.
    expect(announcer()).toHaveTextContent('3 terminals and 4 editors found.');
  });
});

describe('P112 §6 — Browse refused and cancelled (sites A and 9)', () => {
  it('site A: the refusal lands inline, beside the state note, with no toast', async () => {
    seamUrl('browseerr');
    renderHarness({ terminalTool: '', editorTool: 'sublime' });
    await settle();

    fireEvent.click(browseEditor());
    // In flight: `aria-disabled`, never `disabled`, and the label does not change.
    expect(browseEditor()).toHaveAttribute('aria-disabled', 'true');
    expect(browseEditor()).toHaveAttribute('aria-busy', 'true');
    expect(browseEditor()).toHaveTextContent('Browse…');
    expect(browseEditor()).not.toBeDisabled();

    await settle();
    expect(document.querySelector('[data-outcome-note="general.editor-tool"]')).toHaveTextContent(
      'That file isn’t a program Bonsai can launch. Try again, or pick a detected tool.',
    );
    // Both carriers, from one call: inline-only would be visible-only.
    expect(announcer()).toHaveTextContent('That file isn’t a program Bonsai can launch.');
    expect(document.querySelectorAll('.toast')).toHaveLength(0);
    // The state note still stands — a DIFFERENT file was rejected.
    expect(note('general-editor-tool-note')).toHaveTextContent('Runs C:\\Program Files');
    // …and the selection is untouched.
    expect(editorInput()).toHaveValue('Sublime Text');
  });

  it('state 9: a cancel is not an error — nothing is said and nothing changes', async () => {
    seamUrl('browsecancel');
    renderHarness({ terminalTool: '', editorTool: 'sublime' });
    await settle();

    fireEvent.click(browseEditor());
    await settle();

    expect(document.querySelector('[data-outcome-note="general.editor-tool"]')).toHaveTextContent(
      '',
    );
    expect(announcer()).toHaveTextContent('');
    expect(editorInput()).toHaveValue('Sublime Text');
    expect(browseEditor()).toHaveAttribute('aria-disabled', 'false');
  });
});

describe('P112 §7 / §16.6 — Rescan (sites D, E, F)', () => {
  it('site F: the FIRST scan announces nothing (it is not a user action)', async () => {
    renderHarness();
    await settle();
    expect(note('general-rescan-tools-note')).toHaveTextContent(
      '3 terminals and 3 editors found.',
    );
    expect(announcer()).toHaveTextContent('');
  });

  it('site E: a landed rescan announces the counts and writes no outcome note', async () => {
    renderHarness();
    await settle();
    fireEvent.click(rescan());
    expect(rescan()).toHaveAttribute('aria-disabled', 'true');
    expect(rescan()).toHaveAttribute('aria-busy', 'true');
    // Label unchanged — never `Rescanning…`.
    expect(rescan()).toHaveTextContent('Rescan');
    expect(note('general-rescan-tools-note')).toHaveTextContent('Looking for installed tools…');

    await settle();
    expect(announcer()).toHaveTextContent('3 terminals and 3 editors found.');
    expect(document.querySelector('[data-outcome-note="general.rescan-tools"]')).toHaveTextContent(
      '',
    );
  });

  it('7b: a rescan NEVER blanks a picker that already knows its value', async () => {
    seamUrl('slowrescan');
    renderHarness({ terminalTool: '', editorTool: 'vscode' });
    await settle();
    expect(editorInput()).toHaveValue('Visual Studio Code');

    fireEvent.click(rescan());
    await settle(1000);
    // Mid-rescan: the previous scan is still in hand, so the label stands and
    // the placeholder — which belongs to the COLD window only — never appears.
    expect(editorInput()).toHaveValue('Visual Studio Code');
    expect(editorInput()).not.toHaveAttribute('placeholder');
    expect(note('general-editor-tool-note')).toHaveTextContent('Looking for installed editors…');

    await settle(1500);
    expect(editorInput()).toHaveValue('Visual Studio Code');
  });

  it('site D, cold: SCAN_ERR is the row’s only sentence, and the pickers hold', async () => {
    seamUrl('scanerr');
    renderHarness({ terminalTool: '', editorTool: 'vscode' });
    await settle();

    expect(document.querySelector('[data-outcome-note="general.rescan-tools"]')).toHaveTextContent(
      'Couldn’t check for installed tools. Your current choices still work — try Rescan again.',
    );
    // §16.4 R5: the ONE state with no state note. `SCAN_NONE` here would be a lie.
    expect(note('general-rescan-tools-note')).toHaveTextContent('');
    // The picker has no list and no label map, so it must not claim to be
    // looking either — and must never render the stored value as a raw id.
    expect(editorInput()).toHaveValue('');
    expect(editorInput()).not.toHaveAttribute('placeholder');
    expect(note('general-editor-tool-note')).toHaveTextContent('');
    // …but the TERMINAL row is at `''`, where `Auto-detect` is a synchronous
    // option and `NOTE_AUTO` is true whether or not a scan ever landed. A row
    // that can still say something true must say it.
    expect(input('settings-terminal-tool')).toHaveValue('Auto-detect');
    expect(note('general-terminal-tool-note')).toHaveTextContent(
      '“Open in terminal” uses the first terminal Bonsai finds.',
    );
    expect(document.querySelectorAll('.toast')).toHaveLength(0);
  });

  it('a second press while scanning no-ops (aria-disabled is not disabled)', async () => {
    seamUrl('slowrescan');
    renderHarness();
    await settle();
    fireEvent.click(rescan());
    fireEvent.click(rescan());
    fireEvent.click(rescan());
    await settle(2500);
    // One landed scan, one announcement — three presses would have queued three.
    expect(announcer()).toHaveTextContent('3 terminals and 3 editors found.');
    expect(rescan()).toHaveAttribute('aria-disabled', 'false');
  });
});

/** The three new modules, read as text by Vite (the `tools.test.tsx` technique —
 *  no `node:fs`, this tsconfig has no node types). Globbed, so a rename fails
 *  loudly here instead of silently checking nothing. */
const SOURCES = import.meta.glob(
  './{SettingsExternalToolsSection.tsx,ToolPickerRow.tsx,useExternalToolScan.ts}',
  { query: '?raw', import: 'default', eager: true },
) as Record<string, string>;

/** Comments are stripped before the check: `useExternalToolScan`'s header NAMES
 *  the three launchers in order to state that it cannot reach them, and a check
 *  that forbade the words would forbid documenting the rule. */
const BLOCK_COMMENT = /\/\*[\s\S]*?\*\//g;
const LINE_COMMENT = /^\s*\/\/.*$/gm;

describe('P112 §16.5 / UA17 — the new components cannot launch anything', () => {
  it('takes no launcher callback and no pushToast, structurally', () => {
    expect(Object.keys(SOURCES).sort()).toEqual([
      './SettingsExternalToolsSection.tsx',
      './ToolPickerRow.tsx',
      './useExternalToolScan.ts',
    ]);
    // The structural half of §16.5, checkable by reading the signatures rather
    // than by reasoning about reachability: the picker CONFIGURES launchers and
    // never invokes one. Any future Test/Preview button flips this, at which
    // point `useExternalTools.ts:22/:28/:34` join P113's toast sweep.
    for (const [file, source] of Object.entries(SOURCES)) {
      const code = source.replace(BLOCK_COMMENT, '').replace(LINE_COMMENT, '');
      // Positive control, so the three `not.toContain`s below cannot pass on
      // empty text: an over-eager comment strip, or a file reduced to a bare
      // re-export, would otherwise satisfy all of them vacuously.
      expect(code, `${file} must still hold code after the comment strip`).toContain(
        'export function',
      );
      expect(code, `${file} must not reach for a toast`).not.toContain('pushToast');
      for (const launcher of ['openInTerminal', 'revealInFileManager', 'openInEditor']) {
        expect(code, `${file} must not invoke ${launcher}`).not.toContain(launcher);
      }
    }
  });
});
