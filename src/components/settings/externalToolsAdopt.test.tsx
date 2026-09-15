/**
 * P112 §17.2 and `useExternalToolScan` rule 6 — the owed adopt's LIFETIME and
 * its SEMANTICS. One mechanism, two defects, both invisible to every test that
 * existed before this file:
 *
 *   * **§17.2 promised recovery "from either the button or a remount" and only
 *     the button worked.** `pendingAdopt` was a per-mount ref and the mount
 *     effect skipped `load()` on a warm cache, so switching category away from
 *     General and back dropped the owed adopt AND its note — silently, with the
 *     picked value still only on disk. A remount is the one flow no assertion
 *     reached, which is exactly how the contract could say one thing while the
 *     code did another.
 *   * **An explicit pick did not supersede that kind's owed adopt.** The adopt
 *     then landed the browsed value over the newer pick, which reached disk
 *     anyway — screen and disk disagreeing until the next launch. Two timing
 *     windows: pick-then-Rescan, and a pick made WHILE a Rescan is in flight.
 *
 * Its own file rather than an append to `externalToolsBrowse.test.tsx` (393
 * lines): the harness here differs in the one way both cases need — `onChange`
 * applies the tool keys to state, as `useUiSettings.handleSettingsChange` does
 * (§17.3 item 1) — and a picker whose pick does not stick cannot show a pick
 * being overwritten.
 */
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { act, cleanup, fireEvent, render, screen } from '@testing-library/react';
import { useState } from 'react';

import { SettingsPanel, type SettingsPanelProps } from '../SettingsPanel';
import { MINIMAL } from './coverageFixtures';
import { BROWSE_STALE, EDITOR_OUTCOME_ID, RESCAN_NOTE_ID } from './toolPickerCopy';
import { resetExternalToolMockForTests } from '../../ipc/mock/handlers/tools';
import { resetExternalToolScanCacheForTests } from './toolScanMemory';
import type { ToolSelection } from '../../hooks/useUiSettings';
import type { UiSettingsPatch } from '../../ipc';

function seamUrl(seam: string): void {
  window.history.replaceState({}, '', seam === '' ? '/' : `/?tools=${seam}`);
}

async function settle(ms = 900): Promise<void> {
  await act(async () => {
    await vi.advanceTimersByTimeAsync(ms);
  });
}

/** Drain microtasks WITHOUT advancing a timer: the note restored on remount
 *  arrives in a promise continuation (`report`'s `flushSync` is legal only
 *  there), while the recovery scan is still behind the mock's `delay()`. This is
 *  the instant the "note came back" assertion has to look at. */
async function microtasks(): Promise<void> {
  await act(async () => {});
}

/** Both selections are STATE here, patched by the two channels App gives the
 *  page — and the difference between them is the whole subject of this file:
 *  `onChange` is the debounced WRITE (`useUiSettings` sets the field at once and
 *  the disk lands 300 ms later), `onAdoptToolSelection` is the non-writing adopt
 *  of something read FROM disk (§16.16-5). The mock's stored settings therefore
 *  still say `custom` after a pick, exactly as a real disk does inside the
 *  debounce window. */
interface AdoptLog {
  adopts: ToolSelection[];
  picks: ToolSelection[];
}

function toolKeys(patch: UiSettingsPatch): ToolSelection | null {
  const picked: ToolSelection = {};
  if (patch.terminalTool !== undefined) picked.terminalTool = patch.terminalTool;
  if (patch.editorTool !== undefined) picked.editorTool = patch.editorTool;
  return Object.keys(picked).length === 0 ? null : picked;
}

function Harness({
  log,
  initial,
}: {
  log: AdoptLog;
  initial: { terminalTool: string; editorTool: string };
}) {
  const [tools, setTools] = useState(initial);
  const props: SettingsPanelProps = {
    open: true,
    initialCategory: 'general',
    onClose: vi.fn(),
    requestSeq: 0,
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
    onChange: (patch: UiSettingsPatch) => {
      const picked = toolKeys(patch);
      if (picked === null) return;
      log.picks.push(picked);
      setTools((prev) => ({ ...prev, ...picked }));
    },
    onAdoptToolSelection: (selection: ToolSelection) => {
      log.adopts.push(selection);
      setTools((prev) => ({ ...prev, ...selection }));
    },
  };
  return <SettingsPanel {...props} />;
}

function renderHarness(initial = { terminalTool: '', editorTool: '' }): AdoptLog {
  const log: AdoptLog = { adopts: [], picks: [] };
  render(<Harness log={log} initial={initial} />);
  return log;
}

const announcer = (): HTMLElement => {
  const el = screen.getByRole('tabpanel').querySelector<HTMLElement>('.sr-only[role="status"]');
  if (el === null) throw new Error('no announcer');
  return el;
};
const editorInput = (): HTMLInputElement => {
  const el = document.getElementById('settings-editor-tool');
  if (!(el instanceof HTMLInputElement)) throw new Error('no editor picker');
  return el;
};
const editorOutcome = (): HTMLElement => {
  const el = document.getElementById(EDITOR_OUTCOME_ID);
  if (el === null) throw new Error('no editor outcome note');
  return el;
};
const rescanNote = (): HTMLElement => {
  const el = document.getElementById(RESCAN_NOTE_ID);
  if (el === null) throw new Error('no rescan note');
  return el;
};
const browseEditor = (): HTMLElement =>
  screen.getByRole('button', { name: 'Browse for an editor program' });
const rescan = (): HTMLElement => screen.getByRole('button', { name: 'Rescan' });
/** The editor row's ↺. Named from the CATALOG label, like every other row's
 *  — `Reset ${label} to default` (`SettingsRow.tsx`). It renders only while the
 *  value differs from the default, which is why the owed-adopt state is where it
 *  is visible: the browsed value is still on disk, so the row shows `vscode`. */
const resetEditor = (): HTMLElement =>
  screen.getByRole('button', { name: 'Reset Editor to default' });
const tab = (name: string): HTMLElement => screen.getByRole('tab', { name });

/** Arm the owed adopt the way a user does: browse, and let the follow-up read
 *  fail (`?tools=browseadopterr`). The pick IS on disk; the UI is not showing it. */
async function browseIntoStaleAdopt(): Promise<AdoptLog> {
  seamUrl('browseadopterr');
  const log = renderHarness({ terminalTool: '', editorTool: 'vscode' });
  await settle();
  fireEvent.click(browseEditor());
  await settle();
  expect(editorOutcome()).toHaveTextContent(BROWSE_STALE);
  expect(editorInput()).toHaveValue('Visual Studio Code');
  expect(log.adopts).toEqual([]);
  return log;
}

/** The picker offers its list on focus; the option's value is committed on
 *  `mouseDown` (`externalToolsPicker.test.tsx`'s technique). */
function pickSublime(): void {
  fireEvent.focus(editorInput());
  fireEvent.mouseDown(screen.getByRole('option', { name: /^Sublime Text/ }));
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

describe('P112 rule 6 — an explicit pick supersedes that kind’s owed adopt', () => {
  it('the pick survives the Rescan the note asked for, and retracts that note', async () => {
    const log = await browseIntoStaleAdopt();

    pickSublime();
    expect(log.picks).toEqual([{ editorTool: 'sublime' }]);
    expect(editorInput()).toHaveValue('Sublime Text');
    // `BROWSE_STALE` names a Rescan that can no longer change this row, so the
    // pick that superseded it retracts it — rule 4's reason, in reverse.
    expect(editorOutcome()).toHaveTextContent('');

    fireEvent.click(rescan());
    await settle();

    // Before rule 6: the owed adopt read `editorTool: 'custom'` off the disk —
    // which still says `custom`, because the pick's own write is inside its
    // 300 ms debounce — and put the browsed tool back over the newer pick,
    // while that pick reached disk anyway. Screen and disk then disagreed until
    // the next launch.
    expect(editorInput()).toHaveValue('Sublime Text');
    expect(log.adopts).toEqual([]);
  });

  it('a pick made WHILE a rescan is in flight is not overwritten at landing', async () => {
    const log = await browseIntoStaleAdopt();

    // The second window, and it is not the same one: `load` reads the owed adopt
    // when it is DISPATCHED and applies it when it LANDS, up to 2.1 s later.
    fireEvent.click(rescan());
    pickSublime();
    expect(editorInput()).toHaveValue('Sublime Text');

    await settle();

    expect(editorInput()).toHaveValue('Sublime Text');
    expect(log.adopts).toEqual([]);
    // The scan itself still landed: a refused adopt is not a refused scan.
    expect(rescanNote()).toHaveTextContent('3 terminals and 4 editors found.');
  });
});

describe('P112 §17.2 — the owed adopt recovers from a REMOUNT', () => {
  it('brings its note back, completes the adopt, and lands it once', async () => {
    const log = await browseIntoStaleAdopt();

    fireEvent.click(tab('Appearance'));
    expect(document.getElementById('settings-editor-tool')).toBeNull();
    fireEvent.click(tab('General'));
    await microtasks();

    // The note comes back WITH the owed adopt. Restoring the value silently
    // would leave a stale selection on screen with nothing saying why; dropping
    // both — which is what a per-mount ref did — was worse still.
    expect(editorOutcome()).toHaveTextContent(BROWSE_STALE);
    expect(announcer()).toHaveTextContent('Your choice was saved');
    expect(editorInput()).toHaveValue('Visual Studio Code');

    await settle();

    // …and the remount COMPLETES it, which is the half of §17.2's sentence that
    // was never implemented. One field, the owed kind's.
    expect(log.adopts).toEqual([{ editorTool: 'custom' }]);
    expect(editorInput()).toHaveValue('editor-cli-launcher');
    // Rule 4: the recovery retracts its own message — and says what it did, or a
    // screen-reader user is left with an instruction to press Rescan followed by
    // a value that changed unheard.
    expect(editorOutcome()).toHaveTextContent('');
    expect(announcer()).toHaveTextContent('Editor set to editor-cli-launcher.');

    // Once per owed PICK, not once per mount: the second visit neither scans nor
    // re-raises the note it already retracted.
    fireEvent.click(tab('Appearance'));
    fireEvent.click(tab('General'));
    await microtasks();
    expect(editorOutcome()).toHaveTextContent('');
    expect(rescanNote()).not.toHaveTextContent('Looking for installed tools');
    expect(announcer()).toHaveTextContent('');

    await settle();
    expect(log.adopts).toHaveLength(1);
    expect(editorInput()).toHaveValue('editor-cli-launcher');
  });
});

describe('P112 rule 6 — an explicit RESET supersedes it too (the ↺)', () => {
  it('the reset survives the Rescan the note asked for, and retracts that note', async () => {
    const log = await browseIntoStaleAdopt();

    // The ↺ is an explicit selection of the DEFAULT, not a different kind of
    // write: "reset this row" says what the user wants just as plainly as
    // picking an option does, so rule 6 has to hold for it. Before this went
    // through `changeTool` it did not — `resetRow` patched the key through the
    // generic catalog path, which cannot see the owed adopt.
    fireEvent.click(resetEditor());
    expect(log.picks).toEqual([{ editorTool: '' }]);
    expect(editorInput()).toHaveValue('Auto-detect');
    // Rule 4 in reverse, exactly as for a pick: `BROWSE_STALE` names a Rescan
    // that can no longer change this row, so the act that superseded it retracts
    // it. The standing note used to instruct the failing action.
    expect(editorOutcome()).toHaveTextContent('');

    fireEvent.click(rescan());
    await settle();

    // Before the fix the owed adopt read `editorTool: 'custom'` off the disk and
    // put the browsed tool back OVER the reset, while the reset still reached
    // disk — the pick case's divergence, one control over and a shorter path.
    expect(editorInput()).toHaveValue('Auto-detect');
    expect(log.adopts).toEqual([]);
    // A refused adopt is still not a refused scan.
    expect(rescanNote()).toHaveTextContent('3 terminals and 4 editors found.');
  });
});
