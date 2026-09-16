/**
 * P113 §10.3 ruling R1 / AC2b — `useOutcomeScrollCorrection`, the ONE motion
 * this contract allows.
 *
 * Why this suite exists: the module shipped with zero coverage and its whole
 * behaviour rested on a single harness measurement. The contract is explicit
 * about which part is load-bearing (`P113-settings-inline-notes.md:643-645`):
 * "**`Math.ceil` is load-bearing, not defensive** — without it the four-edge
 * test in AC2b cannot be met by following this contract". A test that cannot
 * tell `Math.ceil` from `Math.floor` would not cover that sentence, so the
 * residual case below asserts the exact post-correction `scrollTop` AND the
 * four edges of AC2b.
 *
 * ## The geometry stubs, and why the module is otherwise a no-op here
 *
 * The DOM env for `*.test.tsx` is happy-dom (`vite.config.ts:45`), which has no
 * layout: every `getBoundingClientRect()` is the zero rect and `clientHeight`
 * is 0. With zero geometry `fullyInside` is trivially true, so
 * `useOutcomeScrollCorrection.ts:69` returns before anything happens — that,
 * not the `scrollIntoView` guard at `:67`, is what makes the module inert in
 * every other suite. (`:67` cannot fire here at all: happy-dom implements
 * `Element.prototype.scrollIntoView` natively as a no-op
 * (`happy-dom@20.12.2/lib/nodes/element/Element.js:1029`), and
 * `src/test/setup.ts:29` would stub it if it were absent. The `jsdom` comment at `:65-66` is stale — see the bail-out case
 * below, which pins the guard by removing the method from the note INSTANCE.)
 *
 * So `wire()` installs a real scroll model on the two elements the module
 * reads: a pane whose client box is a fixed viewport rect, a note whose rect
 * moves with `pane.scrollTop`, and a `scrollTop` setter that **snaps to whole
 * pixels** — the device-pixel snapping the contract measured at DPR 1
 * (`:82`: "`scrollTop += 0.171875` reads back as 1269"). Without the snap a
 * sub-pixel write would land and the residual case could not distinguish
 * `Math.ceil` from a bare assignment.
 *
 * The fixture markup is hand-written rather than the real `SettingsRow` (which
 * needs catalog ids and two providers for no test value); the class names are
 * the ones the app renders — `.settings-pane` from `SettingsShell.tsx:206`,
 * `.settings-row` from `SettingsRow.tsx:110` — and the note element itself is
 * the real `SettingsOutcomeNote`, so `[data-outcome-note]` is not a guess.
 */
import { afterEach, describe, expect, it, vi } from 'vitest';
import { render } from '@testing-library/react';

import { SettingsOutcomeNote, type SettingsOutcome } from './SettingsOutcomeNote';
import { useOutcomeScrollCorrection } from './useOutcomeScrollCorrection';

/** Row 4 of the Dev page — the slot R1 was found on (§17.1). */
const SLOT = 'dev.delete-logs';

const OUTCOME: SettingsOutcome = { tone: 'success', text: 'Deleted 3 log files.' };

// ---------------------------------------------------------------------------
// The scroll model. Numbers are the contract's own where it states them.
// ---------------------------------------------------------------------------

/** `.settings-pane`'s client box, in viewport coordinates: 137 → 687 with zero
 *  borders, exactly as measured (`useOutcomeScrollCorrection.ts:78-80`). */
const CLIP_TOP = 137;
const CLIP_HEIGHT = 550;
const CLIP_BOTTOM = CLIP_TOP + CLIP_HEIGHT; // 687
const CLIP_LEFT = 24;
const CLIP_WIDTH = 600;

const NOTE_INDENT = 12;
const NOTE_WIDTH = 400;

/** The residual `scrollIntoView` leaves behind: 0.171875 px = 11/64, exact in
 *  binary, so the edge numbers below are exact and `toBe` is safe. */
const RESIDUAL = 0.171875;

/** Where `block: 'nearest'` settles, per the measurement at `:77-78`. */
const SETTLE = 1269;

interface Geometry {
  /** The note's top in the pane's CONTENT coordinates. */
  noteTop: number;
  noteHeight: number;
  /** `scrollTop` before the outcome commits. */
  initialScrollTop: number;
  /** Where the stubbed `scrollIntoView({block:'nearest'})` leaves the pane. */
  settleScrollTop: number;
}

/** The pathological 7-line outcome on row 4: 64 px tall, and `nearest` settles
 *  with its bottom `RESIDUAL` px past the clip. `noteTop` is derived so that
 *  at `settleScrollTop` the note's bottom is exactly `CLIP_BOTTOM + RESIDUAL`. */
const CLIPPED: Geometry = {
  noteTop: SETTLE + CLIP_HEIGHT + RESIDUAL - 64,
  noteHeight: 64,
  initialScrollTop: 1207, // §17.1's measured pre-correction position
  settleScrollTop: SETTLE,
};

const SCROLL_HEIGHT = 1900;

interface Wired {
  pane: HTMLElement;
  note: HTMLElement;
  scrollIntoView: ReturnType<typeof vi.fn>;
  /** Every value the MODULE assigned to `pane.scrollTop`, in order. The
   *  `scrollIntoView` stub writes the backing store directly, so it never
   *  shows up here. */
  writes: number[];
  /** The note's rect at the current scroll offset — what AC2b measures. */
  noteRect: () => DOMRect;
  clip: { top: number; bottom: number; left: number; right: number };
  scrollTop: () => number;
}

function wire(geom: Geometry, opts: { scrollIntoView?: boolean } = {}): Wired {
  const pane = document.querySelector<HTMLElement>('.settings-pane');
  const note = document.querySelector<HTMLElement>(`[data-outcome-note="${SLOT}"]`);
  if (pane === null || note === null) throw new Error('fixture did not render');

  let offset = geom.initialScrollTop;
  const writes: number[] = [];

  for (const [prop, value] of [
    ['clientTop', 0],
    ['clientLeft', 0],
    ['clientHeight', CLIP_HEIGHT],
    // Required: happy-dom's default 0 would put `clip.right` at `clip.left`,
    // and the module's `fullyInside` checks the RIGHT edge too.
    ['clientWidth', CLIP_WIDTH],
    ['scrollHeight', SCROLL_HEIGHT],
  ] as const) {
    Object.defineProperty(pane, prop, { configurable: true, get: () => value });
  }

  Object.defineProperty(pane, 'scrollTop', {
    configurable: true,
    get: () => offset,
    set: (v: number) => {
      writes.push(v);
      // DPR-1 device-pixel snapping (`useOutcomeScrollCorrection.ts:80-83`):
      // a fractional offset is simply not representable.
      offset = Math.max(0, Math.min(SCROLL_HEIGHT - CLIP_HEIGHT, Math.round(v)));
    },
  });

  const noteRect = () =>
    new DOMRect(
      CLIP_LEFT + NOTE_INDENT,
      CLIP_TOP + geom.noteTop - offset,
      NOTE_WIDTH,
      geom.noteHeight,
    );

  pane.getBoundingClientRect = () => new DOMRect(CLIP_LEFT, CLIP_TOP, CLIP_WIDTH, CLIP_HEIGHT);
  note.getBoundingClientRect = noteRect;

  const scrollIntoView = vi.fn((_arg?: ScrollIntoViewOptions) => {
    offset = geom.settleScrollTop; // bypasses the setter: this is not a module write
  });
  // Stubbed on the INSTANCE, so nothing leaks into other suites.
  if (opts.scrollIntoView !== false) {
    Object.defineProperty(note, 'scrollIntoView', { configurable: true, value: scrollIntoView });
  } else {
    // The `:67` bail-out: `typeof el.scrollIntoView !== 'function'`.
    Object.defineProperty(note, 'scrollIntoView', { configurable: true, value: undefined });
  }

  return {
    pane,
    note,
    scrollIntoView,
    writes,
    noteRect,
    clip: {
      top: CLIP_TOP,
      bottom: CLIP_BOTTOM,
      left: CLIP_LEFT,
      right: CLIP_LEFT + CLIP_WIDTH,
    },
    scrollTop: () => offset,
  };
}

// ---------------------------------------------------------------------------
// The fixture: a pane, the owning row with its control and the real note, and
// one control OUTSIDE the row to stand for "the user navigated elsewhere".
// ---------------------------------------------------------------------------

function Fixture({ notes }: { notes: ReadonlyMap<string, SettingsOutcome> }) {
  useOutcomeScrollCorrection(notes);
  return (
    <div className="settings-pane">
      <div className="settings-row" data-setting-id={SLOT}>
        <div className="settings-row-control">
          <button type="button" data-testid="own-control">
            Delete all…
          </button>
        </div>
        <div className="settings-row-help-slot">
          <SettingsOutcomeNote slot={SLOT} id="dev-delete-logs-note" outcome={notes.get(SLOT) ?? null} />
        </div>
      </div>
      <button type="button" data-testid="elsewhere">
        Elsewhere
      </button>
    </div>
  );
}

const EMPTY: ReadonlyMap<string, SettingsOutcome> = new Map();

/**
 * Mount idle (the mount effect is a no-op: `previous` starts AT the mounted
 * map), wire the geometry, put focus where the case wants it, then commit the
 * outcome — the same order the app produces it in.
 */
function mountThenReport(
  geom: Geometry,
  focus: 'own-control' | 'elsewhere',
  opts: { scrollIntoView?: boolean } = {},
): Wired {
  const view = render(<Fixture notes={EMPTY} />);
  const wired = wire(geom, opts);
  view.getByTestId(focus).focus();
  view.rerender(<Fixture notes={new Map([[SLOT, OUTCOME]])} />);
  return wired;
}

afterEach(() => {
  vi.restoreAllMocks();
});

describe('useOutcomeScrollCorrection — condition 1: measured, not assumed (§10.3)', () => {
  it('does not scroll at all when the note is already fully inside the scrollport', () => {
    // Note 100 px below the top of the scrollport, 64 px tall: all four edges
    // inside, so the correction must not run — not the `scrollIntoView`, not
    // the ceil step.
    const inside: Geometry = { ...CLIPPED, noteTop: CLIPPED.initialScrollTop + 100 };
    const w = mountThenReport(inside, 'own-control');

    const rect = w.noteRect();
    expect(rect.top).toBeGreaterThanOrEqual(w.clip.top);
    expect(rect.bottom).toBeLessThanOrEqual(w.clip.bottom);

    expect(w.scrollIntoView).not.toHaveBeenCalled();
    expect(w.writes).toEqual([]);
    expect(w.scrollTop()).toBe(inside.initialScrollTop);
  });
});

describe('useOutcomeScrollCorrection — condition 3: focus inside the owning row (§10.3)', () => {
  it('does not scroll a user who navigated out of the row during the operation', () => {
    // Same clipped geometry as the residual case, so the ONLY reason nothing
    // happens is the focus gate.
    // Stated from the fixture's numbers, not from the live rect: if the gate
    // fails the correction moves the note, and a live read would then fail here
    // instead of on the assertion that names the gate.
    expect(CLIP_TOP + CLIPPED.noteTop - CLIPPED.initialScrollTop + CLIPPED.noteHeight).toBeGreaterThan(
      CLIP_BOTTOM,
    );

    const w = mountThenReport(CLIPPED, 'elsewhere');

    expect(document.activeElement).toBe(document.querySelector('[data-testid="elsewhere"]'));
    expect(w.scrollIntoView).not.toHaveBeenCalled();
    expect(w.writes).toEqual([]);
    expect(w.scrollTop()).toBe(CLIPPED.initialScrollTop);
  });
});

describe('useOutcomeScrollCorrection — the sub-pixel residual (AC2b, §10.3 condition 2)', () => {
  it('ceils the residual away so all four edges land inside the clip', () => {
    const w = mountThenReport(CLIPPED, 'own-control');

    // Condition 2: instant, `nearest`, never `smooth`.
    expect(w.scrollIntoView).toHaveBeenCalledTimes(1);
    expect(w.scrollIntoView).toHaveBeenCalledWith({ block: 'nearest', behavior: 'auto' });

    // `scrollIntoView` alone settles 0.171875 px short — the measurement the
    // contract reproduced at two viewports (§10.3 cond. 2, `:636-640`).
    expect(CLIP_TOP + CLIPPED.noteTop - SETTLE + CLIPPED.noteHeight).toBe(CLIP_BOTTOM + RESIDUAL);

    // The ceil step, exactly once, to the next WHOLE pixel.
    //   `Math.ceil(1269 + 0.171875)` = 1270.
    //   `Math.floor` would write 1269  → nothing moves, note still clipped.
    //   a bare `pane.scrollTop + overflow` would write 1269.171875 → snapped
    //   back to 1269 at DPR 1, so also nothing moves.
    expect(w.writes).toEqual([1270]);
    expect(w.scrollTop()).toBe(1270);
    expect(Number.isInteger(w.scrollTop())).toBe(true);

    // AC2b, the four-edge test, on the final rect. Report the numbers, as the
    // criterion asks: bottom 686.171875 against a clip bottom of 687.
    const rect = w.noteRect();
    expect(rect.bottom).toBe(686.171875);
    expect(rect.top).toBeGreaterThanOrEqual(w.clip.top);
    expect(rect.left).toBeGreaterThanOrEqual(w.clip.left);
    expect(rect.bottom).toBeLessThanOrEqual(w.clip.bottom);
    expect(rect.right).toBeLessThanOrEqual(w.clip.right);
  });

  /** The doc comment's own claim (`:102-105`): keyed on object IDENTITY, not
   *  text, because `report()` mints a fresh object every time. The mirror of
   *  that claim is that a commit which does NOT change the object must not
   *  scroll — a re-render for an unrelated reason is not an outcome. */
  it('ignores a re-render that carries the same outcome object', () => {
    const notes = new Map([[SLOT, OUTCOME]]);
    const view = render(<Fixture notes={notes} />);
    const w = wire(CLIPPED, {});
    view.getByTestId('own-control').focus();
    // A NEW map (so the effect's dep changes and it really re-runs) holding the
    // SAME outcome object.
    view.rerender(<Fixture notes={new Map(notes)} />);

    expect(w.scrollIntoView).not.toHaveBeenCalled();
    expect(w.writes).toEqual([]);
  });
});

describe('useOutcomeScrollCorrection — the guards, pinned as written', () => {
  /**
   * KNOWN LIMITATION, PINNED AS INTENDED — do not "fix" this.
   *
   * The contract prescribes `while (deficit > 0) pane.scrollTop += Math.ceil(deficit)`
   * (`P113-settings-inline-notes.md:641-643`). The shipped code is one pass
   * plus a height guard (`useOutcomeScrollCorrection.ts:90-96`), and both
   * choices are deliberate: one `Math.ceil` of the total deficit is enough, and
   * the guard is what stops the loop chasing a note TALLER than the scrollport
   * forever by scrolling its head out of sight.
   *
   * The consequence, recorded rather than hidden: for such a note there is NO
   * ceil correction, so AC2b (all four edges inside the clip) is unachievable
   * for it by construction. `nearest` leaves it top-aligned, which is the
   * better of the two reachable states — the head of the message is readable.
   */
  it('leaves a note taller than the scrollport where `nearest` put it, uncorrected', () => {
    const tall: Geometry = {
      noteTop: 1600,
      noteHeight: 700, // > CLIP_HEIGHT (550)
      initialScrollTop: 1207,
      settleScrollTop: 1600, // `nearest` on an over-tall, bottom-outside box: align top
    };
    const w = mountThenReport(tall, 'own-control');

    // The scroll DID happen — the limitation is the missing ceil step, not a
    // missing correction.
    expect(w.scrollIntoView).toHaveBeenCalledTimes(1);
    expect(w.scrollTop()).toBe(tall.settleScrollTop);
    expect(w.writes).toEqual([]);

    // And so it is still clipped: top flush with the scrollport, bottom 150 px
    // past it. AC2b cannot hold here; this assertion says so out loud.
    const rect = w.noteRect();
    expect(rect.top).toBe(CLIP_TOP);
    expect(rect.bottom).toBe(CLIP_BOTTOM + (tall.noteHeight - CLIP_HEIGHT));
  });

  /**
   * The `:67` bail-out, pinned on the NOTE INSTANCE rather than by breaking the
   * prototype for the whole file. Its comment claims jsdom needs it; this env
   * is happy-dom, which implements the method, so the guard is unreachable in
   * this repo's suites — but it
   * is still the module's contract for a DOM without `scrollIntoView`, and
   * without it the call below would THROW rather than no-op.
   */
  it('no-ops, without throwing, where the element has no `scrollIntoView`', () => {
    const w = mountThenReport(CLIPPED, 'own-control', { scrollIntoView: false });

    expect(w.noteRect().bottom).toBeGreaterThan(w.clip.bottom); // clipped, and left that way
    expect(w.writes).toEqual([]);
    expect(w.scrollTop()).toBe(CLIPPED.initialScrollTop);
  });
});
