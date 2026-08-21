import { beforeEach, describe, expect, it } from 'vitest';
import {
  ECHO_TTL_MS,
  __resetEchoSuppression,
  armEcho,
  clearEchoSuppression,
  isEchoSuppressed,
} from './echoSuppression';

describe('echoSuppression', () => {
  beforeEach(() => {
    __resetEchoSuppression();
  });

  it('is not suppressed before any arm', () => {
    expect(isEchoSuppressed('r1', 1000)).toBe(false);
  });

  it('suppresses within the window and releases at the boundary', () => {
    const t0 = 10_000;
    armEcho('r1', t0);
    expect(isEchoSuppressed('r1', t0)).toBe(true);
    expect(isEchoSuppressed('r1', t0 + ECHO_TTL_MS - 1)).toBe(true); // 599
    expect(isEchoSuppressed('r1', t0 + ECHO_TTL_MS)).toBe(false); // 600 (exclusive)
    expect(isEchoSuppressed('r1', t0 + ECHO_TTL_MS + 1)).toBe(false);
  });

  it('re-arming extends the window', () => {
    armEcho('r1', 1_000);
    armEcho('r1', 1_400); // second mutation before the first window closed
    expect(isEchoSuppressed('r1', 1_999)).toBe(true); // 1400 + 599
    expect(isEchoSuppressed('r1', 2_000)).toBe(false);
  });

  it('clearEchoSuppression drops the window', () => {
    armEcho('r1', 1_000);
    clearEchoSuppression('r1');
    expect(isEchoSuppressed('r1', 1_000)).toBe(false);
  });

  it('is isolated per repoId (AC7)', () => {
    armEcho('r1', 1_000);
    expect(isEchoSuppressed('r1', 1_100)).toBe(true);
    expect(isEchoSuppressed('r2', 1_100)).toBe(false);
  });

  it('__resetEchoSuppression wipes the registry', () => {
    armEcho('r1', 1_000);
    __resetEchoSuppression();
    expect(isEchoSuppressed('r1', 1_000)).toBe(false);
  });
});
