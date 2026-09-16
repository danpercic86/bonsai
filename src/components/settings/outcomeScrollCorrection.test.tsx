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
 * `.settings-row` from `SettingsRow.tsx:109` — and the note element itself is
 * the real `SettingsOutcomeNote`, so `[data-outcome-note]` is not a guess.
 */
import { afterEach, describe, expect, it, vi } from 'vitest';
import { render } from '@testing-library/react';

import type { RenderResult } from '@testing-library/react';

import { SettingsOutcomeNote, type SettingsOutcome } from './SettingsOutcomeNote';
import { useOutcomeScrollCorrection } from './useOutcomeScrollCorrection';

/** Row 4 of the Dev page — the slot R1 was found on (§17.1). */
const SLOT = 'dev.delete-logs';

const OUTCOME: SettingsOutcome = { tone: 'success', text: 'Deleted 3 log files.' };

/** A slot the fixture renders NO note for — an outcome can outlive its row
 *  (P113 §7) or its row can be filtered out by the Settings search box. */
const ABSENT = 'dev.no-such-row';

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

/** The note taller than the scrollport (700 > CLIP_HEIGHT), used by the
 *  height-guard case. `nearest` on a box whose bottom is outside and whose
 *  height exceeds the scrollport aligns its TOP, so `settleScrollTop` is
 *  `noteTop`. Every number here is inside the scroll model: the note's bottom
 *  (1800) fits `SCROLL_HEIGHT` and the settle offset (1100) is below the max
 *  scroll offset (1350) — see `assertModelIsPossible`. */
const OVERTALL: Geometry = {
  noteTop: 1100,
  noteHeight: 700,
  initialScrollTop: 1000, // note starts below the fold: 237 → 937 against 137 → 687
  settleScrollTop: 1100,
};

const SCROLL_HEIGHT = 1900;

/** The pane's own numbers bound every geometry: content cannot extend past
 *  `scrollHeight`, and no offset past `scrollHeight - clientHeight` is
 *  reachable (the `scrollTop` setter below clamps there, as a real pane does).
 *  A fixture that violates either models a pane that cannot exist, which is
 *  behaviourally harmless here — the module reads neither number — but invites
 *  a wrong diagnosis later, so it fails loudly instead. */
function assertModelIsPossible(geom: Geometry): void {
  const maxOffset = SCROLL_HEIGHT - CLIP_HEIGHT;
  if (geom.noteTop + geom.noteHeight > SCROLL_HEIGHT) {
    throw new Error(
      `impossible pane: note bottom ${geom.noteTop + geom.noteHeight} > scrollHeight ${SCROLL_HEIGHT}`,
    );
  }
  for (const offset of [geom.initialScrollTop, geom.settleScrollTop]) {
    if (offset > maxOffset) throw new Error(`impossible pane: scrollTop ${offset} > max ${maxOffset}`);
  }
}

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
  /** Move the pane as the USER would, bypassing the setter — so the move is not
   *  recorded in `writes` and is not a module action. */
  setScrollTop: (v: number) => void;
}

function wire(geom: Geometry, opts: { scrollIntoView?: boolean } = {}): Wired {
  assertModelIsPossible(geom);
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
    setScrollTop: (v: number) => {
      offset = v;
    },
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
 * map), wire the geometry, then put focus where the case wants it. Stops short
 * of committing an outcome, for the cases that need the view to drive more than
 * one commit themselves.
 */
function mountIdle(
  geom: Geometry,
  focus: 'own-control' | 'elsewhere',
  opts: { scrollIntoView?: boolean } = {},
): { view: RenderResult; wired: Wired } {
  const view = render(<Fixture notes={EMPTY} />);
  const wired = wire(geom, opts);
  view.getByTestId(focus).focus();
  return { view, wired };
}

/** `mountIdle`, then commit the outcome — the same order the app produces it
 *  in: the operation finishes and `report()` lands on an already-mounted,
 *  already-focused row. */
function mountThenReport(
  geom: Geometry,
  focus: 'own-control' | 'elsewhere',
  opts: { scrollIntoView?: boolean } = {},
): Wired {
  const { view, wired } = mountIdle(geom, focus, opts);
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

  /**
   * The doc comment's own claim (`:102-105`): keyed on object IDENTITY, not
   * text, because `report()` mints a fresh object every time. The mirror of
   * that claim is that a commit which does NOT change the object must not
   * scroll — a re-render for an unrelated reason is not an outcome.
   *
   * THREE renders, not two, and the pane is scrolled back between them. Both
   * details are what make this a test of the identity skip rather than of
   * nothing:
   *
   *  - Mounting with the outcome ALREADY in the map (the earlier shape of this
   *    case) leaves `before` at `useRef(notes)`'s initial value, which equals
   *    the mounted map either way — so it passed with `previous.current = notes`
   *    (`useOutcomeScrollCorrection.ts:116`) deleted. Mounting EMPTY and
   *    reporting first means the ref assignment is the ONLY thing that can
   *    carry A's note into the next commit's `before`.
   *  - After the correction the note is fully inside the clip, so a re-detected
   *    change would bail at condition 1 (`:69`) before `scrollIntoView` and
   *    still look like a skip. `setScrollTop` puts the pane back where the user
   *    left it, so a re-detection is observable as a real second yank — which
   *    is the app defect `:116` prevents: without it `previous.current` stays at
   *    its initial value forever and every later unrelated re-render still
   *    carrying the first outcome object re-scrolls the pane.
   */
  it('ignores a re-render that carries the same outcome object', () => {
    const a = new Map([[SLOT, OUTCOME]]);
    const { view, wired: w } = mountIdle(CLIPPED, 'own-control');

    // 1. The outcome arrives: the correction runs, exactly once.
    view.rerender(<Fixture notes={a} />);
    expect(w.scrollIntoView).toHaveBeenCalledTimes(1);
    expect(w.writes).toEqual([1270]);

    // 2. The user scrolls back to where they were, then something unrelated
    //    re-renders: a NEW map (so the effect's dep changes and it really
    //    re-runs) holding the SAME outcome object.
    w.setScrollTop(CLIPPED.initialScrollTop);
    view.rerender(<Fixture notes={new Map(a)} />);

    expect(w.scrollIntoView).toHaveBeenCalledTimes(1);
    expect(w.writes).toEqual([1270]);
    // The observable that matters: the user's scroll position is untouched.
    expect(w.scrollTop()).toBe(CLIPPED.initialScrollTop);
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
    const w = mountThenReport(OVERTALL, 'own-control');

    // The scroll DID happen — the limitation is the missing ceil step, not a
    // missing correction.
    expect(w.scrollIntoView).toHaveBeenCalledTimes(1);
    expect(w.scrollTop()).toBe(OVERTALL.settleScrollTop);
    expect(w.writes).toEqual([]);

    // And so it is still clipped: top flush with the scrollport, bottom 150 px
    // past it. AC2b cannot hold here; this assertion says so out loud.
    const rect = w.noteRect();
    expect(rect.top).toBe(CLIP_TOP);
    expect(rect.bottom).toBe(CLIP_BOTTOM + (OVERTALL.noteHeight - CLIP_HEIGHT));
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

  /** The `:59` bail. A slot can hold an outcome with no note rendered for it:
   *  the four MCP slots live in a map that outlives its surface (P113 §7), and
   *  a slot whose row is filtered out by the Settings search box has no
   *  element. Without the null check this throws on `el.closest`. */
  it('no-ops, without throwing, for a slot with no note element in the DOM', () => {
    const { view, wired: w } = mountIdle(CLIPPED, 'own-control');
    view.rerender(<Fixture notes={new Map([[ABSENT, OUTCOME]])} />);

    expect(document.querySelector(`[data-outcome-note="${ABSENT}"]`)).toBeNull();
    expect(w.scrollIntoView).not.toHaveBeenCalled();
    expect(w.writes).toEqual([]);
    expect(w.scrollTop()).toBe(CLIPPED.initialScrollTop);
  });

  /**
   * The `return` at `:122`, PINNED AS WRITTEN — it is not a `continue`. The
   * module corrects the FIRST changed slot and stops, resting on the stated
   * invariant that at most one slot changes per commit (`:119-120`: every Dev
   * action is `anyBusy`-gated and a General scan or Browse reports into one
   * slot only).
   *
   * Observable here: `ABSENT` is inserted first, so it is the first changed
   * slot; the loop returns on it and never reaches `SLOT`, whose note is
   * genuinely clipped and would otherwise be corrected. If the invariant ever
   * stops holding, this case is where it surfaces — as a deliberate record of
   * the trade, not as a passing test.
   */
  it('stops at the first changed slot, leaving a second changed slot untouched', () => {
    const { view, wired: w } = mountIdle(CLIPPED, 'own-control');
    // Checked BEFORE the commit: SLOT's note is genuinely clipped, so the only
    // reason it goes uncorrected is the loop stopping. Read after the commit
    // this would instead fail first on the very correction under test.
    expect(w.noteRect().bottom).toBeGreaterThan(w.clip.bottom);

    view.rerender(
      <Fixture
        notes={
          new Map([
            [ABSENT, OUTCOME],
            [SLOT, OUTCOME],
          ])
        }
      />,
    );

    expect(w.scrollIntoView).not.toHaveBeenCalled();
    expect(w.writes).toEqual([]);
    expect(w.scrollTop()).toBe(CLIPPED.initialScrollTop);
  });
});
