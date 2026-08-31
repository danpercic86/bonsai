/**
 * P68e §13.1 — the AI activity dock: the shell, the collapsed bar, the log body and the
 * reply form. Proposal discoverability and multi-run behaviour live next door in
 * `AiActivityPanel.runs.test.tsx` (~500-line rule); the fixtures both files share are in
 * `src/test/aiDockKit.tsx`.
 *
 * The test that matters most is the COLLAPSED BAR: the reported failure was "I
 * clicked the AI button and had no feedback, I didn't know if something was happening
 * or if it had finished", so the bar must answer status + subject + elapsed + latest
 * output line without the user opening anything.
 *
 * The second-most important is `does not steal focus`: Claude's question can arrive
 * while the user is mid-sentence in the commit box, and moving that caret is
 * unacceptable (U6).
 */
import { describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen } from '@testing-library/react';

import { AiActivityPanel } from './AiActivityPanel';
import { PARTIAL_NOTE } from './AiActivityLog';
import { line, mount, props, run } from '../test/aiDockKit';
import type { AiActivityRun } from './aiDockFormat';
import type { AiRunStatus } from './repoWorkspace/useAiRuns';

// ---------------------------------------------------------------- 1, 8, 19

describe('the dock shell', () => {
  it('renders NOTHING at all with no runs (U1 — never pay a pixel for unused AI)', () => {
    const { container } = mount({ runs: [] });
    expect(container).toBeEmptyDOMElement();
  });

  it('collapsed: no body, the latest log line IS shown, aria-expanded=false', () => {
    const { container } = mount({
      collapsed: true,
      runs: [run({ log: [line(1, '⚙ Grep(x)'), line(2, '⚙ Read(src/i18n/index.ts)')] })],
    });
    expect(container.querySelector('#ai-dock-body')).toBeNull();
    expect(container.querySelector('.ai-dock-header')).not.toBeNull();
    expect(screen.getByRole('button', { name: 'AI activity' })).toHaveAttribute(
      'aria-expanded',
      'false',
    );
    // THE actual deliverable: "is something happening?" answered without expanding.
    expect(container.querySelector('.ai-dock-activity')?.textContent).toBe(
      '⚙ Read(src/i18n/index.ts)',
    );
    expect(container.querySelector('.ai-dock-elapsed')?.textContent).toBe('1:07');
  });

  it('the activity line is hidden once the run is no longer running', () => {
    const { container } = mount({
      collapsed: true,
      runs: [run({ status: 'ready', log: [line(1, 'done')] })],
    });
    expect(container.querySelector('.ai-dock-activity')).toBeNull();
  });

  it('density rides through to data-density (U12)', () => {
    const { container } = mount({ density: 'compact' });
    expect(container.querySelector('.ai-dock')).toHaveAttribute('data-density', 'compact');
  });
});

// ---------------------------------------------------------------- 2, 3, 4, 5, 6

describe('the header row', () => {
  const cases: [AiRunStatus, boolean, string, string][] = [
    ['running', false, 'Running', 'running'],
    ['running', true, 'Stopping…', 'stopping'],
    ['awaitingInput', false, 'Needs you', 'awaiting'],
    ['ready', false, 'Ready', 'ready'],
    ['failed', false, 'Failed', 'failed'],
    ['cancelled', false, 'Cancelled', 'cancelled'],
  ];

  it('shows the §2 pill word and data-status for every state', () => {
    for (const [status, cancelling, label, data] of cases) {
      const { container, unmount } = mount({
        runs: [run({ status, cancelRequested: cancelling, error: 'boom' })],
      });
      const pill = container.querySelector('.ai-dock-status');
      expect(pill?.textContent).toContain(label);
      expect(pill).toHaveAttribute('data-status', data);
      unmount();
    }
  });

  it('Cancel exists only while live, fires once, and disables on cancelRequested', () => {
    const { p, unmount } = mount();
    const cancel = screen.getByRole('button', { name: 'Cancel the AI run' });
    fireEvent.click(cancel);
    expect(p.onCancel).toHaveBeenCalledTimes(1);
    expect(p.onCancel).toHaveBeenCalledWith(run().key);
    unmount();

    // §6: the disabled `Stopping…` state lands BEFORE any IPC resolves.
    mount({ runs: [run({ cancelRequested: true })] });
    expect(screen.getByRole('button', { name: 'Stopping the AI run' })).toBeDisabled();
  });

  it('no Cancel on a terminal run; ✕ dismiss instead', () => {
    const { p } = mount({ runs: [run({ status: 'ready' })] });
    expect(screen.queryByRole('button', { name: /cancel/i })).toBeNull();
    fireEvent.click(screen.getByRole('button', { name: 'Dismiss this run' }));
    expect(p.onDismiss).toHaveBeenCalledWith(run().key);
  });

  it('no ✕ while the run is live', () => {
    mount();
    expect(screen.queryByRole('button', { name: 'Dismiss this run' })).toBeNull();
  });

  it('a terminal elapsed value does not move across re-renders, and retitles to "Took"', () => {
    const frozen = run({ status: 'cancelled', elapsedMs: 134_000 });
    const { container, rerender } = mount({ runs: [frozen] });
    const read = () => container.querySelector('.ai-dock-elapsed');
    expect(read()?.textContent).toBe('2:14');
    expect(read()).toHaveAttribute('title', 'Took 2:14');
    rerender(<AiActivityPanel {...props({ runs: [frozen] })} />);
    expect(read()?.textContent).toBe('2:14');
  });

  it('cost: $— with the U13 title until a turn ends, then the real number', () => {
    const { container, unmount } = mount();
    const cost = container.querySelector('.ai-dock-cost');
    expect(cost?.textContent).toBe('$—');
    expect(cost).toHaveAttribute('title', 'Cost appears when Claude finishes a turn');
    unmount();
    const second = mount({ runs: [run({ costUsd: 0.0238 })] });
    expect(second.container.querySelector('.ai-dock-cost')?.textContent).toBe('$0.0238');
  });

  /**
   * §12-B1. The user accepted "no default spend cap" BECAUSE spend is visible, and
   * `costUsd` is `$—` until the first turn boundary — on a long single-turn run that is
   * minutes of nothing moving. The CLI's thinking-token heartbeat is the only live
   * signal that exists before then, so it is rendered beside the cost. Its three honest
   * limits are asserted here, and the fact that it is NEVER priced.
   */
  it('shows the live thinking-token estimate beside the cost, and never prices it', () => {
    const { container, unmount } = mount({ runs: [run({ thinkingTokens: 1_450 })] });
    const chip = container.querySelector('.ai-dock-thinking');
    expect(chip?.textContent).toBe(`~${(1450).toLocaleString()} tok`);
    // "estimate, not a price" is stated, and no dollar figure is derived from it.
    expect(chip?.getAttribute('title')).toMatch(/estimate, not a price/);
    expect(chip?.textContent).not.toMatch(/\$/);
    // The cost column stays honest at the same time (U13).
    expect(container.querySelector('.ai-dock-cost')?.textContent).toBe('$—');
    unmount();

    // Absent on a run that never reported one — no `~0 tok`, no placeholder.
    const none = mount({ runs: [run({ thinkingTokens: null })] });
    expect(none.container.querySelector('.ai-dock-thinking')).toBeNull();
    none.unmount();

    // Several runs: summed ACROSS runs (separate processes), like cost.
    const many = mount({
      collapsed: true,
      runs: [
        run({ thinkingTokens: 150 }),
        run({ key: 'conflict:b.json', label: 'b.json', thinkingTokens: 600 }),
      ],
    });
    expect(many.container.querySelector('.ai-dock-thinking')?.textContent).toBe('~750 tok');
  });

  it('the turn counter appears from turn 2 only', () => {
    const { container, unmount } = mount({ runs: [run({ turn: 1 })] });
    expect(container.querySelector('.ai-dock-turn')).toBeNull();
    unmount();
    const second = mount({ runs: [run({ turn: 2 })] });
    expect(second.container.querySelector('.ai-dock-turn')?.textContent).toBe('turn 2');
  });

  it('the progress sweep exists only while something is live', () => {
    const { container, unmount } = mount();
    expect(container.querySelector('.ai-dock-progress')).not.toBeNull();
    unmount();
    const done = mount({ runs: [run({ status: 'failed' })] });
    expect(done.container.querySelector('.ai-dock-progress')).toBeNull();
  });
});

// ---------------------------------------------------------------- 7, 20

describe('the reply form (§4)', () => {
  function asking(over: Partial<AiActivityRun> = {}) {
    return run({ status: 'awaitingInput', question: 'Einträge or Eintraege?', ...over });
  }

  it('renders only for awaitingInput', () => {
    const { container, unmount } = mount();
    expect(container.querySelector('.ai-dock-ask')).toBeNull();
    unmount();
    const ask = mount({ runs: [asking()] });
    expect(ask.container.querySelector('.ai-dock-ask')).not.toBeNull();
    expect(screen.getByText('Einträge or Eintraege?')).toBeInTheDocument();
  });

  it('submits on click, on Enter, and on Ctrl+Enter; Shift+Enter does not', () => {
    // Every send locks the box until the run leaves `awaitingInput` (§4.3), so the
    // store's acknowledgement is simulated between idioms by handing down the NEXT
    // question — which is also what the real store does (status → running, new ask).
    const onReply = vi.fn();
    const ask = (question: string) => props({ runs: [asking({ question })], onReply });
    const { rerender } = render(<AiActivityPanel {...ask('q1')} />);
    const box = () => screen.getByRole('textbox', { name: 'Your answer to Claude' });

    fireEvent.change(box(), { target: { value: 'Einträge' } });
    fireEvent.click(screen.getByRole('button', { name: 'Send' }));
    expect(onReply).toHaveBeenLastCalledWith(asking().key, 'Einträge');

    rerender(<AiActivityPanel {...ask('q2')} />);
    fireEvent.change(box(), { target: { value: 'plain enter' } });
    fireEvent.keyDown(box(), { key: 'Enter' });
    expect(onReply).toHaveBeenLastCalledWith(asking().key, 'plain enter');

    rerender(<AiActivityPanel {...ask('q3')} />);
    fireEvent.change(box(), { target: { value: 'ctrl enter' } });
    fireEvent.keyDown(box(), { key: 'Enter', ctrlKey: true });
    expect(onReply).toHaveBeenLastCalledWith(asking().key, 'ctrl enter');
    expect(onReply).toHaveBeenCalledTimes(3);

    rerender(<AiActivityPanel {...ask('q4')} />);
    fireEvent.change(box(), { target: { value: 'newline' } });
    fireEvent.keyDown(box(), { key: 'Enter', shiftKey: true });
    expect(onReply).toHaveBeenCalledTimes(3);
  });

  /** §4.3 is a LOCKED behaviour: on send the textarea and Send lock and the label reads
   *  `Sending…` until the status leaves `awaitingInput`. It used to be hard-coded
   *  `sending={false}`, so the locked half never happened and could not be tested. */
  it('locks the reply box and says Sending… until the run leaves awaitingInput', () => {
    const { p, rerender } = mount({ runs: [asking()] });
    const box = screen.getByRole('textbox', { name: 'Your answer to Claude' });
    fireEvent.change(box, { target: { value: 'Einträge' } });
    fireEvent.click(screen.getByRole('button', { name: 'Send' }));

    expect(p.onReply).toHaveBeenCalledTimes(1);
    expect(box).toBeDisabled();
    const sending = screen.getByRole('button', { name: 'Sending…' });
    expect(sending).toBeDisabled();
    // A second Enter while locked cannot double-send.
    fireEvent.keyDown(box, { key: 'Enter' });
    expect(p.onReply).toHaveBeenCalledTimes(1);

    // A NEW question on the same run always arrives unlocked.
    rerender(<AiActivityPanel {...props({ runs: [asking({ question: 'and the article?' })] })} />);
    expect(screen.getByRole('textbox', { name: 'Your answer to Claude' })).not.toBeDisabled();
    expect(screen.getByRole('button', { name: 'Send' })).toBeInTheDocument();
  });

  it('an all-whitespace draft cannot submit, and Send stays disabled', () => {
    const { p } = mount({ runs: [asking()] });
    const box = screen.getByRole('textbox', { name: 'Your answer to Claude' });
    fireEvent.change(box, { target: { value: '   \n  ' } });
    expect(screen.getByRole('button', { name: 'Send' })).toBeDisabled();
    fireEvent.keyDown(box, { key: 'Enter' });
    expect(p.onReply).not.toHaveBeenCalled();
  });

  /**
   * U6, THE delicate case. The reported scenario is a user typing a commit message
   * when Claude's question lands 40 s into a run; stealing that caret loses their
   * words. Both branches are asserted because only asserting the happy one would
   * make the guard trivially satisfiable.
   */
  it('does NOT move focus when the user is typing elsewhere, but DOES when idle', () => {
    const other = document.createElement('input');
    document.body.append(other);
    other.focus();
    expect(document.activeElement).toBe(other);

    const busy = mount({ runs: [asking()] });
    expect(document.activeElement).toBe(other);
    busy.unmount();
    other.remove();

    document.body.focus();
    mount({ runs: [asking({ key: 'conflict:other.json' })] });
    expect(document.activeElement).toBe(
      screen.getByRole('textbox', { name: 'Your answer to Claude' }),
    );
  });
});

// ---------------------------------------------------------------- 9, 10, 11, 12, 15

describe('the log body (§3)', () => {
  it('renders the sticky trim note with a localised count', () => {
    const { container } = mount({ runs: [run({ logDropped: 1204, log: [line(1, 'x')] })] });
    const note = container.querySelector('.ai-log-dropped');
    expect(note?.textContent).toBe(`↑ ${(1204).toLocaleString()} earlier lines trimmed`);
  });

  it('chips a line of EXACTLY 2000 chars, and not one of 1999', () => {
    const cut = mount({ runs: [run({ log: [line(1, `${'x'.repeat(1999)}…`)] })] });
    expect(cut.container.querySelectorAll('.ai-log-trunc')).toHaveLength(1);
    cut.unmount();
    const whole = mount({ runs: [run({ log: [line(1, 'y'.repeat(1999))] })] });
    expect(whole.container.querySelectorAll('.ai-log-trunc')).toHaveLength(0);
  });

  it('carries the store-assigned kind onto data-kind', () => {
    const { container } = mount({
      runs: [
        run({
          log: [
            line(1, '⚙ Read(x)'),
            line(2, 'stderr: boom'),
            line(3, '» answered (12 bytes)'),
            line(4, 'Hello'),
          ],
        }),
      ],
    });
    expect(
      [...container.querySelectorAll('.ai-log-line')].map((el) => el.getAttribute('data-kind')),
    ).toEqual(['tool', 'stderr', 'meta', 'text']);
  });

  it('never announces the log (U4), but the dock has one polite status region', () => {
    const { container } = mount({ runs: [run({ log: [line(1, 'x')] })] });
    const log = container.querySelector('.ai-log');
    expect(log?.getAttribute('aria-live')).toBeNull();
    expect(log?.getAttribute('role')).toBeNull();
    const announce = container.querySelector('.ai-dock-announce');
    expect(announce).toHaveAttribute('role', 'status');
    expect(announce).toHaveAttribute('aria-live', 'polite');
  });

  it('says so when live output is switched off, instead of looking broken', () => {
    mount({ streamLogEnabled: false });
    expect(
      screen.getByText(
        'Live output is off — turn on "Stream AI output" in Settings to see it here.',
      ),
    ).toBeInTheDocument();
  });

  it('distinguishes "starting" from "captured nothing"', () => {
    const starting = mount();
    expect(screen.getByText('Starting Claude…')).toBeInTheDocument();
    starting.unmount();
    mount({ runs: [run({ status: 'cancelled' })] });
    expect(screen.getByText('No output was captured.')).toBeInTheDocument();
  });

  /** U7 — the fragment is quarantined: no Copy, no Apply, no editable field. */
  it('partial output is closed by default and offers no way to use it', () => {
    const { container } = mount({
      runs: [run({ status: 'cancelled', partialText: 'half a merged body' })],
    });
    const toggle = screen.getByRole('button', { name: /Unfinished output/ });
    expect(toggle).toHaveAttribute('aria-expanded', 'false');
    expect(screen.getByText(PARTIAL_NOTE)).toBeInTheDocument();
    expect(
      PARTIAL_NOTE.endsWith('Bonsai will not apply it.'),
      'the fixed sentence must promise Bonsai will not apply it',
    ).toBe(true);
    expect(container.querySelector('.ai-dock-partial-body')).toBeNull();
    for (const button of screen.getAllByRole('button')) {
      expect(button.getAttribute('aria-label') ?? button.textContent ?? '').not.toMatch(
        /copy|apply|stage/i,
      );
    }
    fireEvent.click(toggle);
    expect(container.querySelector('.ai-dock-partial-body')?.textContent).toBe(
      'half a merged body',
    );
  });
});

// ---------------------------------------------------------------- §8

describe('resize (§8)', () => {
  it('exposes a horizontal separator with the persisted height, and commits ONCE per drag', () => {
    const { container, p } = mount();
    const grip = container.querySelector('.pane-divider-ai-dock');
    expect(grip).toHaveAttribute('aria-orientation', 'horizontal');
    expect(grip).toHaveAttribute('aria-label', 'Resize AI activity dock');
    expect(grip).toHaveAttribute('aria-valuenow', '180');

    // ArrowUp grows the dock and commits immediately (one write per keypress).
    fireEvent.keyDown(grip!, { key: 'ArrowUp' });
    expect(p.onResizeHeight).toHaveBeenCalledWith(188);

    fireEvent.doubleClick(grip!);
    expect(p.onResizeHeight).toHaveBeenLastCalledWith(180);
  });

  it('is not rendered while collapsed — there is nothing to resize', () => {
    const { container } = mount({ collapsed: true });
    expect(container.querySelector('.pane-divider-ai-dock')).toBeNull();
  });
});
