/** Audit 2026-08-18 §3.8 — `guardChunks` containment semantics: the first throw
 *  from `apply` reaches `onError` exactly once, poisons the handler, and every
 *  later chunk is dropped without touching `apply` again. */
import { describe, expect, it, vi } from 'vitest';

import { guardChunks } from './chunkGuard';

describe('guardChunks', () => {
  it('passes chunks through while apply does not throw', () => {
    const apply = vi.fn();
    const onError = vi.fn();
    const g = guardChunks<number>(apply, onError);
    g.handle(1);
    g.handle(2);
    expect(apply.mock.calls).toEqual([[1], [2]]);
    expect(onError).not.toHaveBeenCalled();
    expect(g.poisoned).toBe(false);
  });

  it('reports the FIRST throw once, poisons, and drops every later chunk', () => {
    const boom = new Error('streamAssembler: batch startRow 7 != expected 3');
    const apply = vi.fn((n: number) => {
      if (n >= 2) throw boom;
    });
    const onError = vi.fn();
    const g = guardChunks<number>(apply, onError);
    g.handle(1);
    g.handle(2); // throws -> poisoned
    g.handle(3); // dropped
    g.handle(4); // dropped
    expect(g.poisoned).toBe(true);
    expect(onError).toHaveBeenCalledExactlyOnceWith(boom);
    // apply saw 1 and 2 only — nothing after the poison.
    expect(apply.mock.calls).toEqual([[1], [2]]);
  });
});
