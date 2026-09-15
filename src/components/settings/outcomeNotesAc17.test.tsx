/**
 * P113 AC17 — **every reported outcome produces a text change in the announcer**,
 * whatever its key and whether or not the previous announcement carried the same
 * string.
 *
 * Its own file because it replaces a WITHDRAWN rule. Phase 2 shipped a
 * key-scoped clear in `begin`, which reintroduced §8.1's defect across keys and
 * without a bound: the last-announced-KEY ref it needed (since deleted) never
 * expires, so an outcome announced an hour ago silenced a different row
 * carrying the same text. `begin` now clears unconditionally. The realistic
 * trigger is the likeliest MCP failure there is — no `claude` on PATH fails BOTH
 * register rows with `Could not register: Claude Code CLI not found: program not
 * found`, byte for byte.
 *
 * Measured in the browser harness (`?mcpFail=register`, `MutationObserver` on
 * the AI-access announcer, both register rows failed):
 *
 *   before  ['', T]                — the second event is SILENT
 *   after   ['', T, '', T]
 *
 * **Assert the SEQUENCE, never the final value.** They are identical in the
 * passing and the failing version, which is exactly how a passing suite missed
 * this twice. `announce`'s final text is `T` either way.
 */
import { describe, expect, it } from 'vitest';
import { fireEvent, render, screen } from '@testing-library/react';

import { SettingsOutcomeNote } from './SettingsOutcomeNote';
import { useOutcomeNotes } from './useOutcomeNotes';

/** The one string both rows raise — the mock's verbatim backend text. */
const T = 'Could not register: Claude Code CLI not found: program not found';

/** Two independent slots sharing ONE announcer: the shape of every real section
 *  (Accounts has one per host, AI access one per scope). */
function TwoRowHarness() {
  const { notes, announce, begin, report } = useOutcomeNotes();
  const row = (key: string) => (
    <>
      <button type="button" onClick={() => begin(key)}>{`begin ${key}`}</button>
      <button type="button" onClick={() => report(key, 'error', T)}>{`report ${key}`}</button>
      <SettingsOutcomeNote slot={key} id={`${key}-out`} outcome={notes.get(key) ?? null} />
    </>
  );
  return (
    <>
      {row('global')}
      {row('repo')}
      <p className="sr-only" role="status" aria-live="polite">
        {announce}
      </p>
    </>
  );
}

/**
 * Every text the ONE live region has held, in order — reconstructed from the
 * mutation RECORDS, not by reading `textContent` in the callback.
 *
 * That distinction is the whole test. A `MutationObserver` callback is a
 * microtask, and under `act()` React commits BOTH the `''` and the text before
 * any microtask runs, so a callback that reads the current value sees only the
 * final one and the `'' →` step vanishes — a reader-visible change measured as
 * absent. (In a real browser the two commits straddle a microtask, which is why
 * the harness measurement does not need this; jsdom + `act` is the stricter
 * environment, so the records are the only sound source here.)
 *
 * React's representation of this region: it swaps the text NODE when the string
 * goes empty and mutates `nodeValue` IN PLACE otherwise. So this reconstruction
 * is sound only for transitions through `''` — which, by construction, is every
 * row here: `begin` clears unconditionally and `report` flushes a `''` first
 * when the text repeats. A `characterData` record means two distinct non-empty
 * strings met, and the observer then CANNOT see the intermediate text (records
 * batch under `act`, so every `target.nodeValue` reads the final value). It is
 * therefore asserted to stay unused rather than reconstructed from: extending
 * this test with two distinct strings needs `characterDataOldValue`. Failing
 * loudly beats a sequence that silently drops a transition.
 */
function watchAnnouncer(): { texts: string[]; stop(): void } {
  const region = document.querySelector<HTMLElement>('[role="status"][aria-live="polite"]');
  expect(region, 'the announcer must be mounted before the first outcome').not.toBeNull();
  if (region === null) throw new Error('unreachable');
  const texts: string[] = [region.textContent ?? ''];
  const push = (next: string): void => {
    if (next !== texts[texts.length - 1]) texts.push(next);
  };
  const observer = new MutationObserver((records) => {
    for (const record of records) {
      expect(
        record.type,
        'every transition here must pass through `""`, which swaps text NODES; a ' +
          '`characterData` record is an unobservable in-place edit (see above)',
      ).toBe('childList');
      if (record.removedNodes.length > 0) push('');
      for (const added of record.addedNodes) push(added.textContent ?? '');
    }
  });
  observer.observe(region, { characterData: true, childList: true, subtree: true });
  return {
    texts,
    stop: () => {
      // Drain anything still queued before the assertion reads `texts`.
      for (const record of observer.takeRecords()) {
        if (record.removedNodes.length > 0) push('');
        for (const added of record.addedNodes) push(added.textContent ?? '');
      }
      observer.disconnect();
    },
  };
}

/** The counts sentence a warm rescan re-announces byte-identically (P112 §16.8). */
const COUNTS = '3 terminals and 4 editors found.';

/** P112 §16.4 R2 — one slot plus `announceOnly`, the picker section's shape: the
 *  Rescan row's counts are a STATE note, so a landed rescan announces without
 *  writing anything into the outcome slot. */
function AnnounceOnlyHarness() {
  const { notes, announce, begin, report, announceOnly } = useOutcomeNotes();
  return (
    <>
      <button type="button" onClick={() => begin('general.rescan-tools')}>
        begin rescan
      </button>
      <button type="button" onClick={() => announceOnly(COUNTS)}>
        rescan landed
      </button>
      <button type="button" onClick={() => report('general.rescan-tools', 'error', T)}>
        rescan failed
      </button>
      <button type="button" onClick={() => announceOnly(T)}>
        announce the failure text
      </button>
      <SettingsOutcomeNote
        slot="general.rescan-tools"
        id="general-rescan-tools-outcome"
        outcome={notes.get('general.rescan-tools') ?? null}
      />
      <p className="sr-only" role="status" aria-live="polite">
        {announce}
      </p>
    </>
  );
}

describe('useOutcomeNotes — UA13, AC17 holds for `announceOnly` too (P112 §16.4 R2)', () => {
  it('two rescans whose counts do not change announce twice', () => {
    render(<AnnounceOnlyHarness />);
    const watch = watchAnnouncer();

    // The §16.8 case: a warm rescan finds the same tools, so the string is
    // byte-identical. `['', T]` — one utterance — is the failure this pins.
    fireEvent.click(screen.getByRole('button', { name: 'begin rescan' }));
    fireEvent.click(screen.getByRole('button', { name: 'rescan landed' }));
    fireEvent.click(screen.getByRole('button', { name: 'begin rescan' }));
    fireEvent.click(screen.getByRole('button', { name: 'rescan landed' }));
    watch.stop();

    expect(watch.texts).toEqual(['', COUNTS, '', COUNTS]);
  });

  it('announces with NO visible line, and does not clear a standing note', () => {
    render(<AnnounceOnlyHarness />);
    const note = document.querySelector('[data-outcome-note="general.rescan-tools"]');

    fireEvent.click(screen.getByRole('button', { name: 'rescan landed' }));
    // The whole point of R2: the visible channel is the row's own state note, so
    // the outcome slot stays EMPTY (and `:empty`-collapsed) after an announcement.
    expect(note).toHaveTextContent('');
    expect(document.querySelector('[role="status"]')).toHaveTextContent(COUNTS);

    // `announceOnly` owns the announcer only — a note some other outcome put in
    // the slot is not its business to erase (`begin` is what clears a slot).
    fireEvent.click(screen.getByRole('button', { name: 'rescan failed' }));
    fireEvent.click(screen.getByRole('button', { name: 'rescan landed' }));
    expect(note).toHaveTextContent(T);
  });

  it('shares the announced-text ref with `report`, so a repeat across the two still fires', () => {
    render(<AnnounceOnlyHarness />);
    const watch = watchAnnouncer();

    // The proof AC17 rests on is about WHO may write the announcer and whether
    // they RECORD what they wrote. An `announceOnly` with its own ref (or none)
    // passes both tests above and fails here: `report` leaves T displayed, and
    // the announce-only repeat of T is then a no-op write — silence.
    //
    // Only strings equal to their predecessor are asserted, deliberately: this
    // file's `watchAnnouncer` cannot observe a transition between two DISTINCT
    // non-empty strings (see its doc comment), so a sequence mixing T and the
    // counts would drop a step in the reconstruction, not in the product.
    fireEvent.click(screen.getByRole('button', { name: 'rescan failed' }));
    fireEvent.click(screen.getByRole('button', { name: 'announce the failure text' }));
    watch.stop();

    expect(watch.texts).toEqual(['', T, '', T]);
  });
});

describe('useOutcomeNotes — AC17, the announcer transition is guaranteed per REPORT', () => {
  it('two DIFFERENT keys carrying identical text announce twice', () => {
    render(<TwoRowHarness />);
    const watch = watchAnnouncer();

    // Row 1 fails, then row 2 fails with the same sentence. `begin repo` blanks
    // the announcer unconditionally (§8.2), so the middle `''` lands here in its
    // own commit and `report`'s flush is a no-op; the no-`begin` row below is
    // where that flush does the work. Under the withdrawn key-scoped clear the
    // middle `''` was missing and the second outcome was silent.
    fireEvent.click(screen.getByRole('button', { name: 'begin global' }));
    fireEvent.click(screen.getByRole('button', { name: 'report global' }));
    fireEvent.click(screen.getByRole('button', { name: 'begin repo' }));
    fireEvent.click(screen.getByRole('button', { name: 'report repo' }));
    watch.stop();

    expect(watch.texts).toEqual(['', T, '', T]);
    // Both notes stand: the fix touches the announcer, not the slots.
    expect(document.querySelector('[data-outcome-note="global"]')).toHaveTextContent(T);
    expect(document.querySelector('[data-outcome-note="repo"]')).toHaveTextContent(T);
  });

  it('the SAME key twice still announces twice (AC15 unchanged)', () => {
    render(<TwoRowHarness />);
    const watch = watchAnnouncer();

    fireEvent.click(screen.getByRole('button', { name: 'begin global' }));
    fireEvent.click(screen.getByRole('button', { name: 'report global' }));
    fireEvent.click(screen.getByRole('button', { name: 'begin global' }));
    fireEvent.click(screen.getByRole('button', { name: 'report global' }));
    watch.stop();

    expect(watch.texts).toEqual(['', T, '', T]);
  });

  it('repeats with NO `begin` at all still announce every time (rows 9/10)', () => {
    render(<TwoRowHarness />);
    const watch = watchAnnouncer();

    // The shape the deleted `reannounce`/`flushSync` workaround existed for:
    // `SettingsAccountAddForm` exposes only `onSuccess`, so there is no earlier
    // commit to call `begin` in.
    fireEvent.click(screen.getByRole('button', { name: 'report global' }));
    fireEvent.click(screen.getByRole('button', { name: 'report global' }));
    fireEvent.click(screen.getByRole('button', { name: 'report global' }));
    watch.stop();

    expect(watch.texts).toEqual(['', T, '', T, '', T]);
  });
});
