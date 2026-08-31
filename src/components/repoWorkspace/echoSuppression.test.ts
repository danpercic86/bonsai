import { beforeEach, describe, expect, it } from 'vitest';
import {
  ECHO_TAIL_MS,
  __resetEchoSuppression,
  armEcho,
  clearEchoSuppression,
  disarmEcho,
  isEchoSuppressed,
} from './echoSuppression';

// P85 A2 — round-anchored suppression: a span is open (no expiry) while the
// nesting count > 0, then a fixed tail begins at settle (disarm to 0).

describe('echoSuppression', () => {
  beforeEach(() => {
    __resetEchoSuppression();
  });

  it('is not suppressed before any arm', () => {
    expect(isEchoSuppressed('r1', 1000)).toBe(false);
  });

  it('an OPEN span suppresses with NO expiry (duration-independent)', () => {
    armEcho('r1');
    expect(isEchoSuppressed('r1', 0)).toBe(true);
    // Even an arbitrarily late echo — a slow round on a large repo — stays
    // suppressed while the span is open. This is the P81 miss this fix closes.
    expect(isEchoSuppressed('r1', 10_000_000)).toBe(true);
  });

  it('disarm to 0 starts the settle-relative tail, released at the boundary', () => {
    armEcho('r1');
    disarmEcho('r1', 10_000); // round settled at t=10_000
    expect(isEchoSuppressed('r1', 10_000)).toBe(true);
    expect(isEchoSuppressed('r1', 10_000 + ECHO_TAIL_MS - 1)).toBe(true); // 599
    expect(isEchoSuppressed('r1', 10_000 + ECHO_TAIL_MS)).toBe(false); // 600 (exclusive)
    expect(isEchoSuppressed('r1', 10_000 + ECHO_TAIL_MS + 1)).toBe(false);
  });

  it('nesting: overlapping mutations settle to the tail only after BOTH disarm', () => {
    armEcho('r1'); // count 1
    armEcho('r1'); // count 2
    disarmEcho('r1', 1_000); // count 1 — still an open span, no tail yet
    expect(isEchoSuppressed('r1', 5_000)).toBe(true); // open span, no expiry
    disarmEcho('r1', 2_000); // count 0 — tail begins at 2_000
    expect(isEchoSuppressed('r1', 2_000 + ECHO_TAIL_MS - 1)).toBe(true);
    expect(isEchoSuppressed('r1', 2_000 + ECHO_TAIL_MS)).toBe(false);
  });

  it('re-arming during the tail reopens the span (clears the pending expiry)', () => {
    armEcho('r1');
    disarmEcho('r1', 1_000); // tail until 1_600
    armEcho('r1'); // new mutation before the tail expired → span reopens
    expect(isEchoSuppressed('r1', 5_000)).toBe(true); // no expiry while open
    disarmEcho('r1', 5_000);
    expect(isEchoSuppressed('r1', 5_000 + ECHO_TAIL_MS)).toBe(false);
  });

  it('disarm floors at 0 (a stray disarm cannot go negative)', () => {
    armEcho('r1');
    disarmEcho('r1', 1_000); // → 0, tail
    disarmEcho('r1', 2_000); // stray extra disarm: floors, resets tail from 2_000
    expect(isEchoSuppressed('r1', 2_000 + ECHO_TAIL_MS - 1)).toBe(true);
    expect(isEchoSuppressed('r1', 2_000 + ECHO_TAIL_MS)).toBe(false);
  });

  it('clearEchoSuppression drops both the span and the tail', () => {
    armEcho('r1');
    clearEchoSuppression('r1');
    expect(isEchoSuppressed('r1', 0)).toBe(false);
  });

  it('is isolated per repoId (AC7)', () => {
    armEcho('r1');
    expect(isEchoSuppressed('r1', 1_100)).toBe(true);
    expect(isEchoSuppressed('r2', 1_100)).toBe(false);
  });

  it('__resetEchoSuppression wipes the registry', () => {
    armEcho('r1');
    __resetEchoSuppression();
    expect(isEchoSuppressed('r1', 0)).toBe(false);
  });
});
