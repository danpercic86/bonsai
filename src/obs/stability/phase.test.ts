import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { markBoot, markInteractive, onPhaseChange, phaseAt, resetPhaseForTests } from './phase';

describe('phase transition log (T-F2, A6)', () => {
  let now = 0;
  beforeEach(() => {
    resetPhaseForTests();
    now = 0;
    vi.spyOn(performance, 'now').mockImplementation(() => now);
  });
  afterEach(() => vi.restoreAllMocks());

  it('boot → interactive → markBoot → interactive, labelled by timestamp', () => {
    expect(phaseAt(5)).toBe('boot');
    now = 100;
    markInteractive();
    now = 300;
    markBoot();
    now = 500;
    markInteractive();
    expect(phaseAt(50)).toBe('boot');
    expect(phaseAt(100)).toBe('interactive');
    expect(phaseAt(299)).toBe('interactive');
    expect(phaseAt(300)).toBe('boot');
    expect(phaseAt(499)).toBe('boot');
    expect(phaseAt(600)).toBe('interactive');
  });

  it('an entry stamped before markInteractive stays boot when read after it', () => {
    now = 1000;
    markInteractive();
    // Delivered late (the observer is async), but its startTime was 900.
    expect(phaseAt(900)).toBe('boot');
  });

  it('markInteractive is a no-op when already interactive; listeners hear each transition once', () => {
    const seen: string[] = [];
    const off = onPhaseChange((p) => seen.push(p));
    now = 10;
    markInteractive();
    now = 20;
    markInteractive();
    markBoot();
    markBoot();
    off();
    markInteractive();
    expect(seen).toEqual(['interactive', 'boot']);
    expect(phaseAt(15)).toBe('interactive');
  });
});
