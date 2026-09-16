// P91 follow-up (render-storm) — structural equality for IPC payloads.
//
// Every `list_*` / `get_*` command returns a FRESH serde-serialised object on
// every call, so a background refresh round that finds nothing new still stores
// a brand-new reference into React state and commits a render of the whole
// sidebar. Comparing the payload structurally lets a no-change round commit
// NOTHING (`setX(prev => equal ? prev : next)` — React bails on an identical
// reference), and lets a row's `React.memo` comparator see "same data, new
// object" for what it is.
//
// SCOPE, deliberately narrow: JSON-shaped values only — `null`, primitives,
// arrays and plain objects, which is exactly what serde writes over the IPC
// wire. It is NOT a general deep-equal: `Date`, `Map`, `Set`, class instances
// and cyclic graphs are out of contract (a cycle would recurse forever). Do not
// point it at React state that can hold a DOM node or a function.
import type { Dispatch, SetStateAction } from 'react';

/** True when `a` and `b` are the same JSON-shaped value. */
export function structuralEqual(a: unknown, b: unknown): boolean {
  if (Object.is(a, b)) return true;
  // Past this point a difference in kind (or a null vs object) is a difference.
  if (typeof a !== 'object' || typeof b !== 'object' || a === null || b === null) return false;
  const aIsArray = Array.isArray(a);
  if (aIsArray !== Array.isArray(b)) return false;
  if (aIsArray) {
    const bArr = b as unknown[];
    const aArr = a as unknown[];
    if (aArr.length !== bArr.length) return false;
    for (let i = 0; i < aArr.length; i += 1) {
      if (!structuralEqual(aArr[i], bArr[i])) return false;
    }
    return true;
  }
  const aObj = a as Record<string, unknown>;
  const bObj = b as Record<string, unknown>;
  const aKeys = Object.keys(aObj);
  if (aKeys.length !== Object.keys(bObj).length) return false;
  for (const key of aKeys) {
    // An explicit `undefined` value and an absent key are NOT interchangeable
    // here — the key-count check above already rejects a shape difference, so
    // this only needs the own-key guard for the `{a: undefined}` vs `{b: 1}`
    // pair (same count, different keys).
    if (!Object.hasOwn(bObj, key)) return false;
    if (!structuralEqual(aObj[key], bObj[key])) return false;
  }
  return true;
}

/**
 * Store `next` in a `useState` setter only when it differs structurally from the
 * current value. Returning `prev` unchanged makes React bail out, so a refresh
 * round that fetched byte-identical data commits nothing. Race-safe: the
 * comparison happens inside the updater, against whatever is current then.
 *
 * Two type params so `T` is inferred from the SETTER (which may be widened with
 * `| null`) and never narrowed by `next`.
 */
export function keepIfUnchanged<T, U extends T>(
  set: Dispatch<SetStateAction<T>>,
  next: U,
): void {
  set((prev) => (structuralEqual(prev, next) ? prev : next));
}

/**
 * `React.memo` comparator for a row whose payload is ONE fresh-every-round IPC
 * object (`BranchRow.branch`, `WorktreeRow.wt`, …): `objectKeys` are compared
 * structurally, everything else by identity. Without this a plain `memo` is
 * useless on those rows — the object reference always differs.
 */
export function rowPropsEqual<P extends object>(
  objectKeys: readonly (keyof P)[],
): (prev: Readonly<P>, next: Readonly<P>) => boolean {
  return (prev, next) => {
    const prevKeys = Object.keys(prev) as (keyof P)[];
    if (prevKeys.length !== Object.keys(next).length) return false;
    for (const key of prevKeys) {
      if (!Object.hasOwn(next, key as string)) return false;
      const equal = objectKeys.includes(key)
        ? structuralEqual(prev[key], next[key])
        : Object.is(prev[key], next[key]);
      if (!equal) return false;
    }
    return true;
  };
}
