/**
 * P113 §15 — the acceptance criteria for the Settings outcome-note sweep that a
 * DOM test can own: the idle shape (AC5), one live region per section (AC6), no
 * double-fire (AC7), `aria-describedby` composition (AC8), lifetime (AC12) and
 * — the one that guards a PRE-EXISTING bug — the same outcome twice announcing
 * twice (AC15).
 *
 * AC15 needs a MUTATION spy, not a final-text assertion: the bug is an ABSENT
 * change. A live region only fires on a text change, so setting the announcer to
 * the same string twice is silent the second time; `begin()` clearing it to `''`
 * at operation start is what makes every transition a real `'' → text`.
 */
import { afterEach, describe, expect, it, vi } from 'vitest';
import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';

import { SettingsPanel } from '../SettingsPanel';
import { MINIMAL } from './coverageFixtures';
import { mockIpc } from '../../ipc/mock';
import { SettingsAccountsSection } from './SettingsAccountsSection';
import { SettingsOutcomeNote } from './SettingsOutcomeNote';
import { useOutcomeNotes } from './useOutcomeNotes';
import type { LogSessionInfo } from '../../ipc';

const DEV_ON = {
  enabled: true,
  level: 'debug' as const,
  captureIpc: true,
  captureReact: true,
  captureFrames: false,
  includeRawNames: false,
};

const INFO: LogSessionInfo = {
  sessionId: 's1',
  dir: '/logs',
  files: ['bonsai-a.jsonl'],
  bytes: 500_000,
  records: 1200,
  anomalies: 0,
  dropped: 0,
  redaction: 'strict',
  salt: 'x',
  totalFiles: 5,
  totalBytes: 16_567_501,
  droppedParts: 0,
  writeFailed: false,
  exportFiles: 0,
  exportBytes: 0,
};

function renderDevPane() {
  return render(
    <SettingsPanel
      open
      initialCategory="dev"
      onClose={vi.fn()}
      requestSeq={0}
      onChange={vi.fn()}
      onToggleTheme={vi.fn()}
      onToggleListView={vi.fn()}
      onRequestEnableAi={vi.fn()}
      onSetMcpEnabled={vi.fn()}
      onRequestEnableMcp={vi.fn()}
      onSetMcpAllowWrite={vi.fn()}
      onRequestEnableMcpWrite={vi.fn()}
      onRegisterMcp={vi.fn(async () => {})}
      onShowOnboarding={vi.fn()}
      onOpenRepository={vi.fn()}
      onCheckUpdate={vi.fn()}
      onOpenUpdateDialog={vi.fn()}
      {...MINIMAL}
      dev={DEV_ON}
    />,
  );
}

/** The Dev page's own announcer — the shell's search-status region belongs to
 *  `SettingsSearchBar`, above every category. */
function devAnnouncer(): HTMLElement {
  const el = document.querySelector<HTMLElement>(
    '[role="status"][aria-live="polite"]:not(.settings-search-status)',
  );
  if (el === null) throw new Error('the Dev page must have exactly one announcer');
  return el;
}

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

describe('SettingsOutcomeNote — the idle shape (AC5, AC6)', () => {
  it('is present, childless, and carries no live semantics', () => {
    render(<SettingsOutcomeNote slot="dev.logs" id="x" outcome={null} />);
    const note = document.querySelector<HTMLElement>('[data-outcome-note="dev.logs"]');
    expect(note).not.toBeNull();
    expect(note?.textContent).toBe('');
    // `:empty` is what the CSS keys the chrome collapse off, and it matches only
    // an element with NO child nodes at all. A stray whitespace text node from a
    // reformatted JSX line would break the collapse SILENTLY, so assert the
    // child-node count — jsdom's own `:empty` selector is unreliable (it matches
    // a text-bearing element too), and the rendered collapse is measured in the
    // browser harness instead.
    expect(note?.childNodes).toHaveLength(0);
    expect(note?.tagName).toBe('P');
    // Idle still carries a modifier: a bare `.settings-row-note` keeps its 2px
    // top margin, which would change idle row geometry (AC11).
    expect(note).toHaveClass('settings-row-note', 'settings-row-note--result');
    expect(note).not.toHaveAttribute('aria-live');
    expect(note).not.toHaveAttribute('role');
  });

  it('selects the recipe by tone and nothing else', () => {
    const { rerender } = render(
      <SettingsOutcomeNote slot="s" id="x" outcome={{ tone: 'error', text: 'nope' }} />,
    );
    const note = () => document.querySelector<HTMLElement>('[data-outcome-note="s"]');
    expect(note()).toHaveClass('settings-row-note--warn');
    expect(note()).not.toHaveClass('settings-row-note--result');
    expect(note()?.childNodes).toHaveLength(1);
    // Same slot, other tone — one action, one place (§3.1.2).
    rerender(<SettingsOutcomeNote slot="s" id="x" outcome={{ tone: 'success', text: 'yes' }} />);
    expect(note()).toHaveClass('settings-row-note--result');
    expect(note()).not.toHaveClass('settings-row-note--warn');
  });
});

describe('useOutcomeNotes — begin / report (AC7, AC15)', () => {
  /** A minimal consumer: the announcer plus one note, which is exactly the shape
   *  both real sections use. */
  function Harness() {
    const { notes, announce, begin, report } = useOutcomeNotes();
    return (
      <>
        <button type="button" onClick={() => begin('a')}>
          begin
        </button>
        <button type="button" onClick={() => report('a', 'error', 'boom')}>
          report
        </button>
        <p className="sr-only" role="status" aria-live="polite">
          {announce}
        </p>
        <SettingsOutcomeNote slot="a" id="a-out" outcome={notes.get('a') ?? null} />
      </>
    );
  }

  it('reports the note and the announcement from ONE call, then clears both', () => {
    render(<Harness />);
    const note = () => document.querySelector<HTMLElement>('[data-outcome-note="a"]');
    const live = () => document.querySelector<HTMLElement>('[role="status"]');

    fireEvent.click(screen.getByRole('button', { name: 'report' }));
    expect(note()?.textContent).toBe('boom');
    expect(live()?.textContent).toBe('boom');

    fireEvent.click(screen.getByRole('button', { name: 'begin' }));
    expect(note()?.textContent).toBe('');
    expect(live()?.textContent).toBe('');
  });

  /** AC15 — the regression guard. Asserting the final text proves nothing here:
   *  the defect is that the second identical outcome produces NO mutation. */
  it('mutates the announcer on BOTH of two identical outcomes', async () => {
    render(<Harness />);
    const live = document.querySelector<HTMLElement>('[role="status"]');
    expect(live).not.toBeNull();
    if (live === null) throw new Error('unreachable');

    let mutations = 0;
    const observer = new MutationObserver((records) => {
      mutations += records.length;
    });
    observer.observe(live, { childList: true, characterData: true, subtree: true });

    const begin = screen.getByRole('button', { name: 'begin' });
    const report = screen.getByRole('button', { name: 'report' });
    // Two full operations: begin → report, begin → report, same string both
    // times. Each `begin`/`report` pair is its own commit, exactly as the real
    // callers' `await` separates them.
    fireEvent.click(begin);
    fireEvent.click(report);
    fireEvent.click(begin);
    fireEvent.click(report);
    await waitFor(() => expect(mutations).toBeGreaterThanOrEqual(3));
    observer.disconnect();
    // '' → boom → '' → boom is three text changes; without `begin`'s reset the
    // second `report` would be a no-op and this would stall at 1.
    expect(live.textContent).toBe('boom');
  });
});

describe('DevCategory — live-region count and lifetime (AC6, AC12, AC13)', () => {
  it('keeps exactly ONE live region in the pane, in every state', async () => {
    vi.spyOn(mockIpc, 'logSessionInfo').mockResolvedValue(INFO);
    renderDevPane();
    await screen.findByRole('button', { name: 'Delete all…' });

    const pane = document.querySelector('.settings-pane');
    expect(pane).not.toBeNull();
    expect(pane?.querySelectorAll('[aria-live],[role="status"],[role="alert"]')).toHaveLength(1);
    // Both outcome slots are mounted, and neither is live.
    const slots = document.querySelectorAll('[data-outcome-note]');
    expect(slots).toHaveLength(2);
    for (const slot of slots) {
      expect(slot).not.toHaveAttribute('aria-live');
      expect(slot).not.toHaveAttribute('role');
    }
  });

  /** AC13 — the reveal failure was TOAST-ONLY, so a screen-reader user heard
   *  nothing at all. Inline-only would have made it visible-only. */
  it('announces the reveal failure with the same string it renders inline', async () => {
    vi.spyOn(mockIpc, 'logSessionInfo').mockResolvedValue(INFO);
    vi.spyOn(mockIpc, 'logRevealDir').mockRejectedValue({
      kind: 'externalToolFailed',
      message: 'could not launch file manager (explorer): not found',
    });
    renderDevPane();

    const reveal = await screen.findByRole('button', { name: 'Show in folder' });
    await waitFor(() => expect(reveal).toHaveAttribute('aria-disabled', 'false'));
    await act(async () => {
      fireEvent.click(reveal);
    });

    const expected = "Couldn't open the logs folder. It may have been moved or deleted.";
    const note = document.querySelector<HTMLElement>('[data-outcome-note="dev.logs"]');
    expect(note?.textContent).toBe(expected);
    expect(note).toHaveClass('settings-row-note--warn');
    expect(devAnnouncer().textContent).toBe(expected);
    // The mapped sentence is the whole message: raw OS text is never leaked.
    expect(note?.textContent).not.toContain('explorer');
  });

  /** AC12 — cleared when the OPERATION begins, not when the dialog opens: so
   *  cancelling the confirm leaves the previous outcome intact, and the slot is
   *  empty for the in-flight window. */
  it('clears the delete slot at operation start and leaves it alone on cancel', async () => {
    vi.spyOn(mockIpc, 'logSessionInfo').mockResolvedValue(INFO);
    let release: (() => void) | null = null;
    vi.spyOn(mockIpc, 'logsDeleteAll').mockImplementation(
      () =>
        new Promise((_resolve, reject) => {
          release = () => reject(new Error('logs dir unreadable'));
        }),
    );
    renderDevPane();
    const note = () => document.querySelector<HTMLElement>('[data-outcome-note="dev.delete-logs"]');

    const deleteBtn = await screen.findByRole('button', { name: 'Delete all…' });
    await waitFor(() => expect(deleteBtn).toHaveAttribute('aria-disabled', 'false'));

    // Round 1: run it through to a failure.
    fireEvent.click(deleteBtn);
    fireEvent.click(await screen.findByRole('button', { name: 'Delete all' }));
    expect(note()?.textContent).toBe('');
    await act(async () => {
      release?.();
    });
    await waitFor(() => expect(note()?.textContent).not.toBe(''));
    const first = note()?.textContent ?? '';
    expect(first).not.toBe('');

    // Opening the dialog and CANCELLING must not touch the slot.
    fireEvent.click(deleteBtn);
    // `findBy*` must run OUTSIDE `act` — inside it, the poll cannot advance.
    const cancel = await screen.findByRole('button', { name: 'Cancel' });
    await act(async () => {
      fireEvent.click(cancel);
    });
    expect(note()?.textContent).toBe(first);

    // Pressing through to the operation clears it BEFORE the new result lands.
    fireEvent.click(deleteBtn);
    fireEvent.click(await screen.findByRole('button', { name: 'Delete all' }));
    expect(note()?.textContent).toBe('');
    await act(async () => {
      release?.();
    });
    await waitFor(() => expect(note()?.textContent).toBe(first));
  });
});

describe('aria-describedby composition — Accounts (AC8)', () => {
  it('composes the host outcome id onto the radiogroup and the add-another button', async () => {
    vi.spyOn(mockIpc, 'forgeListAccounts').mockResolvedValue([
      {
        accountId: 'gitHub:github.com:octocat',
        host: 'github.com',
        kind: 'gitHub',
        login: 'octocat',
        avatarUrl: null,
        connected: true,
        isHostDefault: false,
      },
      {
        accountId: 'gitHub:github.com:danpercic86',
        host: 'github.com',
        kind: 'gitHub',
        login: 'danpercic86',
        avatarUrl: null,
        connected: true,
        isHostDefault: false,
      },
    ]);
    render(<SettingsAccountsSection />);
    await screen.findByText('danpercic86');

    const note = document.querySelector('[data-outcome-note="github.com"]');
    expect(note?.id).toBeTruthy();
    const outcomeId = note?.id ?? '';

    // The OD-4 nudge is showing here (2 connected, no default), so the
    // radiogroup must carry BOTH ids — nudge first, outcome second (§9).
    const group = screen.getByRole('radiogroup', { name: 'Default account for github.com' });
    const ids = (group.getAttribute('aria-describedby') ?? '').split(' ').filter((x) => x !== '');
    expect(ids).toHaveLength(2);
    expect(ids[1]).toBe(outcomeId);
    for (const id of ids) expect(document.getElementById(id)).not.toBeNull();

    const addAnother = screen.getByRole('button', { name: 'Add another account to github.com' });
    expect(addAnother.getAttribute('aria-describedby')).toBe(outcomeId);

    // The section slot's own control composes the catalog help id first.
    const addBtn = screen.getByRole('button', { name: 'Add a token for a host' });
    expect((addBtn.getAttribute('aria-describedby') ?? '').split(' ')).toEqual([
      'accounts.add-help',
      'accounts-add-outcome',
    ]);
    expect(document.getElementById('accounts.add-help')).not.toBeNull();
    expect(document.getElementById('accounts-add-outcome')).not.toBeNull();
  });
});

describe('aria-describedby composition (AC8)', () => {
  it('composes both ids on every Dev control, and every id resolves', async () => {
    vi.spyOn(mockIpc, 'logSessionInfo').mockResolvedValue(INFO);
    renderDevPane();
    await screen.findByRole('button', { name: 'Delete all…' });

    const expectations: [string, string[]][] = [
      ['Show in folder', ['dev-logs-note', 'dev-logs-outcome']],
      ['Export session…', ['dev-logs-note', 'dev-logs-outcome']],
      ['Delete all…', ['dev-delete-logs-hint', 'dev-delete-logs-outcome']],
    ];
    for (const [name, ids] of expectations) {
      const btn = screen.getByRole('button', { name });
      expect(
        (btn.getAttribute('aria-describedby') ?? '').split(' ').filter((s) => s !== ''),
        name,
      ).toEqual(ids);
      for (const id of ids) expect(document.getElementById(id), `${name} → #${id}`).not.toBeNull();
    }
  });
});
