/**
 * P112 §13 / §16.14 — the detected-tool picker's states, copy and a11y wiring.
 *
 * Rendered through the REAL `SettingsPanel`, not a bare section: the increment's
 * riskiest seam is the chain `useUiSettings` → context → `useExternalToolScan` →
 * row, and a hand-wired section would test none of it. The `?tools=` seams are
 * the harness's, so the fixtures here and the browser harness cannot diverge.
 *
 * The Browse and Rescan FLOWS live in `externalToolsBrowse.test.tsx` — this file
 * is the state table.
 */
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { act, cleanup, fireEvent, render, screen } from '@testing-library/react';

import { SettingsPanel, type SettingsPanelProps } from '../SettingsPanel';
import { MINIMAL } from './coverageFixtures';
import { WIN_TERMINALS } from '../../ipc/fixtures/externalTools';
import { resetExternalToolMockForTests } from '../../ipc/mock/handlers/tools';
import { resetExternalToolScanCacheForTests } from './toolScanMemory';

export function seamUrl(seam: string): void {
  window.history.replaceState({}, '', seam === '' ? '/' : `/?tools=${seam}`);
}

export function renderGeneral(
  over: Partial<SettingsPanelProps> = {},
): { props: SettingsPanelProps } {
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
    ...over,
  };
  render(<SettingsPanel {...props} />);
  return { props };
}

/** Drain the mock's `delay()` under fake timers. */
export async function settle(ms = 400): Promise<void> {
  await act(async () => {
    await vi.advanceTimersByTimeAsync(ms);
  });
}

export const note = (id: string): HTMLElement | null => document.getElementById(id);
export const input = (id: string): HTMLInputElement => {
  const el = document.getElementById(id);
  if (!(el instanceof HTMLInputElement)) throw new Error(`no input #${id}`);
  return el;
};
export const row = (id: string): HTMLElement => {
  const el = document.querySelector<HTMLElement>(`[data-setting-id="${id}"]`);
  if (el === null) throw new Error(`no row ${id}`);
  return el;
};

beforeEach(() => {
  vi.useFakeTimers();
  resetExternalToolScanCacheForTests();
  resetExternalToolMockForTests();
  seamUrl('');
});

afterEach(() => {
  cleanup();
  vi.useRealTimers();
  vi.restoreAllMocks();
  seamUrl('');
});

describe('P112 §2 — the group, and General’s ONE live region (UA12)', () => {
  it('renders three rows, the lead and the group note', async () => {
    renderGeneral();
    await settle();
    expect(row('general.terminal-tool')).toBeInTheDocument();
    expect(row('general.editor-tool')).toBeInTheDocument();
    expect(row('general.rescan-tools')).toBeInTheDocument();
    expect(screen.getByRole('heading', { name: 'External tools', level: 4 })).toBeInTheDocument();
    expect(document.body.textContent).toContain('never through a shell');
    expect(document.body.textContent).toContain(
      'Not listed? Use Browse to point Bonsai at the program yourself.',
    );
  });

  it('has exactly ONE live region on the pane, and no note is live', async () => {
    renderGeneral();
    await settle();
    const pane = screen.getByRole('tabpanel');
    // UA12, stated as baseline + delta: General had ZERO before this increment,
    // so the one below is the section's `sr-only role="status"` announcer. (The
    // shell's `.settings-search-status` is outside the tabpanel, which is why
    // this is scoped to the pane and not to the dialog.)
    const live = pane.querySelectorAll('[aria-live],[role="status"],[role="alert"]');
    expect(
      [...live].map((el) => el.className),
      'if this comes back with 2, report what the second element is',
    ).toEqual(['sr-only']);
    // UA11: every outcome note is description-only.
    expect(pane.querySelectorAll('[data-outcome-note][aria-live]')).toHaveLength(0);
    expect(pane.querySelectorAll('[data-outcome-note][role]')).toHaveLength(0);
    expect(pane.querySelectorAll('[data-outcome-note]')).toHaveLength(3);
  });

  it('composes both description ids on the picker AND on Browse, none dangling', async () => {
    renderGeneral();
    await settle();
    const ids = [
      input('settings-terminal-tool'),
      input('settings-editor-tool'),
      ...screen.getAllByRole('button', { name: /^Browse/ }),
      screen.getByRole('button', { name: 'Rescan' }),
    ].map((el) => el.getAttribute('aria-describedby'));
    expect(ids).toEqual([
      'general-terminal-tool-note general-terminal-tool-outcome',
      'general-editor-tool-note general-editor-tool-outcome',
      'general-terminal-tool-note general-terminal-tool-outcome',
      'general-editor-tool-note general-editor-tool-outcome',
      'general-rescan-tools-note general-rescan-tools-outcome',
    ]);
    // P113 AC8's method: a dangling idref is worse than no description, and
    // `settingsRowHelpId(rowId)` WOULD dangle here (these rows carry no help).
    for (const list of ids) {
      for (const id of (list ?? '').split(' ')) {
        expect(document.getElementById(id), `${id} dangles`).not.toBeNull();
      }
    }
  });
});

describe('P112 §5 — the state table', () => {
  it('state 1: auto-detect with tools found, no ↺', async () => {
    renderGeneral();
    await settle();
    expect(input('settings-terminal-tool')).toHaveValue('Auto-detect');
    expect(note('general-terminal-tool-note')).toHaveTextContent(
      '“Open in terminal” uses the first terminal Bonsai finds.',
    );
    expect(row('general.terminal-tool').querySelector('.settings-reset')).toBeNull();
  });

  it('state 2: nothing detected — the picker stays enabled and the note explains', async () => {
    seamUrl('none');
    renderGeneral();
    await settle();
    expect(note('general-editor-tool-note')).toHaveTextContent(
      'No editors found on this computer. “Open in editor” still tries the usual ones for this system.',
    );
    expect(input('settings-editor-tool')).not.toBeDisabled();
    fireEvent.focus(input('settings-editor-tool'));
    expect(screen.getAllByRole('option')).toHaveLength(1);
    expect(note('general-rescan-tools-note')).toHaveTextContent(
      'No terminals or editors found on this computer.',
    );
  });

  it('state 3: a detected tool shows its path in `.mono`, with ↺', async () => {
    renderGeneral({ terminalTool: 'windows-terminal' });
    await settle();
    expect(input('settings-terminal-tool')).toHaveValue('Windows Terminal');
    const mono = note('general-terminal-tool-note')?.querySelector('.mono');
    // UA14 — byte-identical, PATHEXT casing included. No transform of any kind.
    expect(mono?.textContent).toBe(WIN_TERMINALS[0].detail);
    expect(mono?.getAttribute('title')).toBe(WIN_TERMINALS[0].detail);
    expect(row('general.terminal-tool').querySelector('.settings-reset')).not.toBeNull();
  });

  it('state 3: a built-in tool says so instead of showing `built in` as a path', async () => {
    renderGeneral({ terminalTool: 'powershell' });
    await settle();
    expect(note('general-terminal-tool-note')).toHaveTextContent(
      'Windows PowerShell is built in to this system.',
    );
    expect(note('general-terminal-tool-note')?.querySelector('.mono')).toBeNull();
  });

  it('state 5: a kept-but-absent id keeps the selection and says so in words', async () => {
    renderGeneral({ editorTool: 'zed' });
    await settle();
    // Named from the AMEND-1 label map, never rendered as the raw id.
    expect(input('settings-editor-tool')).toHaveValue('Zed');
    expect(note('general-editor-tool-note')).toHaveTextContent(
      'Zed isn’t installed right now. “Open in editor” falls back to auto-detect; your choice is kept.',
    );
    fireEvent.focus(input('settings-editor-tool'));
    const stale = screen.getByRole('option', { name: /^Zed/ });
    // §4.2: ENABLED — disabling greys the row the user is currently on.
    expect(stale).toHaveAttribute('aria-disabled', 'false');
    expect(stale).toHaveAttribute('aria-selected', 'true');
    // UA6: carried by a WORD, not by colour.
    expect(stale).toHaveTextContent('Not installed');
  });

  it('state 6: a browsed path that is gone explains itself, path first', async () => {
    seamUrl('customgone');
    renderGeneral({ editorTool: 'custom' });
    await settle();
    const text = note('general-editor-tool-note')?.textContent ?? '';
    expect(text.startsWith('Nothing is at C:\\Users\\a.very.long')).toBe(true);
    expect(text).toContain(
      'any more. “Open in editor” falls back to auto-detect; your choice is kept.',
    );
    fireEvent.focus(input('settings-editor-tool'));
    expect(screen.getByRole('option', { name: /editor-cli-launcher/ })).toHaveTextContent(
      'Not installed',
    );
  });

  it('state 7a: a COLD scan shows the placeholder, never an empty setting', async () => {
    seamUrl('slow');
    renderGeneral({ editorTool: 'vscode' });
    // UA2, first half: the strict Combobox derives its text from the options, so
    // a stored id whose label has not loaded renders `''` — which reads as
    // UNSET, the opposite of the truth.
    const el = input('settings-editor-tool');
    expect(el).toHaveValue('');
    expect(el).toHaveAttribute('placeholder', 'Looking for installed tools…');
    expect(note('general-editor-tool-note')).toHaveTextContent('Looking for installed editors…');
    expect(note('general-rescan-tools-note')).toHaveTextContent('Looking for installed tools…');
    // …and never a synthetic "loading" OPTION, which would be selectable and
    // would patch a junk value.
    fireEvent.focus(el);
    expect(screen.getAllByRole('option').map((o) => o.textContent)).toEqual([
      'Auto-detectBonsai picks the first tool it finds',
    ]);

    await settle(1500);
    expect(input('settings-editor-tool')).toHaveValue('Visual Studio Code');
    expect(input('settings-editor-tool')).not.toHaveAttribute('placeholder');
  });

  it('UA14: a pre-ellipsized 512-char detail renders byte-identically', async () => {
    seamUrl('longlabels');
    renderGeneral();
    await settle();
    fireEvent.focus(input('settings-editor-tool'));
    const { LONG_EDITORS } = await import('../../ipc/fixtures/externalTools');
    const truncated = LONG_EDITORS[2].detail;
    const hint = [...document.querySelectorAll('.combobox-option-hint')].find((h) =>
      (h.textContent ?? '').endsWith('…'),
    );
    expect(truncated).toHaveLength(513);
    expect(hint?.textContent).toBe(truncated);
  });
});

describe('P112 §4 / UA3 — the picker patches one key and nothing else', () => {
  it('picking an option patches exactly that key', async () => {
    const { props } = renderGeneral();
    await settle();
    fireEvent.focus(input('settings-editor-tool'));
    fireEvent.mouseDown(screen.getByRole('option', { name: /^Sublime Text/ }));
    expect(props.onChange).toHaveBeenCalledTimes(1);
    expect(props.onChange).toHaveBeenCalledWith({ editorTool: 'sublime' });
  });

  it('re-picking the CURRENT value patches nothing', async () => {
    const { props } = renderGeneral({ editorTool: 'sublime' });
    await settle();
    fireEvent.focus(input('settings-editor-tool'));
    fireEvent.mouseDown(screen.getByRole('option', { name: /^Sublime Text/ }));
    expect(props.onChange).not.toHaveBeenCalled();
  });

  it('the picker is STRICT: typing can never commit a value', async () => {
    const { props } = renderGeneral();
    await settle();
    const el = input('settings-editor-tool');
    fireEvent.focus(el);
    fireEvent.change(el, { target: { value: 'C:\\evil.exe' } });
    fireEvent.keyDown(el, { key: 'Enter' });
    fireEvent.blur(el);
    // The security property: the backend coerces an unknown id to `''`, so a
    // control that accepted free text would silently discard what was typed.
    expect(props.onChange).not.toHaveBeenCalled();
    expect(el).toHaveValue('Auto-detect');
  });
});

describe('P112 §6 / UA7 — the two Browse buttons', () => {
  it('have distinct accessible names, each containing the visible label', async () => {
    renderGeneral();
    await settle();
    const names = screen
      .getAllByRole('button', { name: /^Browse/ })
      .map((b) => b.getAttribute('aria-label'));
    expect(names).toEqual([
      'Browse for a terminal program',
      'Browse for an editor program',
    ]);
    for (const b of screen.getAllByRole('button', { name: /^Browse/ })) {
      expect(b).toHaveTextContent('Browse…');
      expect(b).not.toBeDisabled();
    }
  });
});
