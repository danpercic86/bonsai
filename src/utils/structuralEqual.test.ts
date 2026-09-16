import { describe, expect, it } from 'vitest';
import { rowPropsEqual, structuralEqual } from './structuralEqual';

describe('structuralEqual', () => {
  it('is reference-fast and value-correct on primitives', () => {
    expect(structuralEqual(1, 1)).toBe(true);
    expect(structuralEqual('a', 'a')).toBe(true);
    expect(structuralEqual(null, null)).toBe(true);
    expect(structuralEqual(undefined, undefined)).toBe(true);
    expect(structuralEqual(1, '1')).toBe(false);
    expect(structuralEqual(null, undefined)).toBe(false);
    expect(structuralEqual(null, {})).toBe(false);
    expect(structuralEqual(0, -0)).toBe(false); // Object.is semantics
    expect(structuralEqual(Number.NaN, Number.NaN)).toBe(true); // Object.is semantics
  });

  it('compares fresh-but-identical IPC payloads as equal', () => {
    const a = { name: 'main', isHead: true, upstream: null, ahead: 0, behind: null, tip: 'ab' };
    const b = { name: 'main', isHead: true, upstream: null, ahead: 0, behind: null, tip: 'ab' };
    expect(a).not.toBe(b);
    expect(structuralEqual(a, b)).toBe(true);
  });

  it('sees a single changed leaf through nesting', () => {
    const a = { local: [{ name: 'main', ahead: null }], head: { oid: 'x' } };
    const b = { local: [{ name: 'main', ahead: 1 }], head: { oid: 'x' } };
    expect(structuralEqual(a, b)).toBe(false);
  });

  it('distinguishes arrays from objects and length differences', () => {
    expect(structuralEqual([1, 2], [1, 2])).toBe(true);
    expect(structuralEqual([1, 2], [2, 1])).toBe(false);
    expect(structuralEqual([1, 2], [1, 2, 3])).toBe(false);
    expect(structuralEqual([], {})).toBe(false);
  });

  it('rejects a key-set difference at equal key counts', () => {
    expect(structuralEqual({ a: undefined }, { b: undefined })).toBe(false);
    expect(structuralEqual({ a: 1 }, { a: 1, b: 2 })).toBe(false);
  });
});

describe('rowPropsEqual', () => {
  interface Props {
    branch: { name: string; ahead: number | null };
    busy: boolean;
    onCheckout: () => void;
  }
  const onCheckout = () => {};
  const equal = rowPropsEqual<Props>(['branch']);

  it('treats a fresh-but-identical object prop as unchanged', () => {
    expect(
      equal(
        { branch: { name: 'main', ahead: null }, busy: false, onCheckout },
        { branch: { name: 'main', ahead: null }, busy: false, onCheckout },
      ),
    ).toBe(true);
  });

  it('re-renders when the object prop actually changed', () => {
    expect(
      equal(
        { branch: { name: 'main', ahead: null }, busy: false, onCheckout },
        { branch: { name: 'main', ahead: 1 }, busy: false, onCheckout },
      ),
    ).toBe(false);
  });

  it('still compares non-object props by identity', () => {
    const a = { branch: { name: 'main', ahead: null }, busy: false, onCheckout };
    expect(equal(a, { ...a, busy: true })).toBe(false);
    expect(equal(a, { ...a, onCheckout: () => {} })).toBe(false);
  });

  it('rejects a differing prop-key set', () => {
    const a = { branch: { name: 'main', ahead: null }, busy: false, onCheckout };
    expect(equal(a, { ...a, extra: 1 } as unknown as Props)).toBe(false);
  });
});
