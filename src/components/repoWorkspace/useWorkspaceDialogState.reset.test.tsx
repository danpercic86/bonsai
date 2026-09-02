/** P38 final batch — `resetArmedDialogs`, the ONE teardown field that disarms
 *  every dialog this hook owns.
 *
 *  Why this suite exists at all. The teardown's required-field deps bag buys
 *  CALL-SITE completeness only: the container cannot enumerate the ~39 members it
 *  does not own, so a 40th flag added to this hook would silently escape the
 *  repo-went-unusable teardown exactly the way the reflog overlay, the commit
 *  search bar, the replay overlay and the composer each did. So completeness here
 *  is enforced MECHANICALLY: the suite reads the hook's OWN returned keys, arms
 *  every one of them, and asserts `resetArmedDialogs` puts each back — a new flag
 *  that is not reset fails here, without anyone remembering to add a case.
 *
 *  The container-side wiring (that `runRefreshRound` actually calls the teardown,
 *  with an armed destructive confirm observably gone) is pinned in
 *  `unusableRepoTeardown.wiring.test.tsx`. */
import { describe, expect, it, vi } from 'vitest';
import { act, renderHook } from '@testing-library/react';

import { COMMIT_PUSH_CANCELED } from '../commitPushSignal';
import { useWorkspaceDialogState } from './useWorkspaceDialogState';
import type { CommitPushResolver, UseWorkspaceDialogState } from './useWorkspaceDialogState';

/** Members excluded from the flag enumeration even where a `set*` partner exists:
 *  the derived disjunction, the reset itself, and the paired promise resolver (a
 *  ref, asserted separately below). */
const NON_FLAGS = new Set(['anyDialogArmed', 'resetArmedDialogs', 'commitPushResolver']);

/** The hook's returned object, keyed dynamically. The enumeration is the point of
 *  this suite, so it reads the real object rather than a hand-maintained list. */
type Bag = Record<string, unknown>;

function bagOf(hook: UseWorkspaceDialogState): Bag {
  return hook as unknown as Bag;
}

function setterName(key: string): string {
  return `set${key[0]?.toUpperCase() ?? ''}${key.slice(1)}`;
}

/** A key is a flag iff its `set*` PARTNER is also a key of the returned object.
 *  Deriving it by PAIRING rather than by excluding a `set` prefix matters: a
 *  future `settingsOpen` + `setSettingsOpen` would BOTH be filtered out by a
 *  prefix test, the count would still read 39, and the new flag would escape the
 *  reset silently — exactly the failure this suite exists to catch. */
function flagKeys(hook: UseWorkspaceDialogState): string[] {
  const keys = new Set(Object.keys(hook));
  return [...keys].filter((k) => !NON_FLAGS.has(k) && keys.has(setterName(k)));
}

/** A value guaranteed to differ from the initial one — and never a function, so
 *  no setter mistakes it for a functional state update. */
function armedValue(initial: unknown, key: string): unknown {
  return typeof initial === 'boolean' ? !initial : { armed: key };
}

function resolverSpy() {
  const resolve = vi.fn(() => {});
  const reject = vi.fn((_e: unknown) => {});
  const resolver: CommitPushResolver = { resolve, reject, sign: null, skipHooks: false };
  return { resolver, resolve, reject };
}

describe('useWorkspaceDialogState — the armed-flag surface', () => {
  it('returns 39 armed flags, each initially falsy and each with a setter', () => {
    const { result } = renderHook(() => useWorkspaceDialogState());
    const keys = flagKeys(result.current);
    // Pinned so ADDING a flag is a deliberate act: bump this number AND add the
    // setter to `resetArmedDialogs` (the reset case below fails otherwise). It
    // also keeps the reset case below from going vacuously green if the filter
    // above ever stops matching anything.
    expect(keys, keys.join(', ')).toHaveLength(39);
    // Belt to the pairing filter's braces: a flag exposed WITHOUT a `set*`
    // partner would be skipped by `flagKeys` and leave the 39 intact. Pinning
    // the TOTAL key count trips on any new member of any shape.
    expect(Object.keys(result.current)).toHaveLength(81);
    const bag = bagOf(result.current);
    for (const key of keys) {
      // Every flag disarms to a falsy initial value (false / null) — that is what
      // makes "restore the initial value" a well-defined reset.
      expect(bag[key], `${key} must start disarmed`).toBeFalsy();
      expect(bag[setterName(key)], `${key} must expose ${setterName(key)}`).toBeTypeOf('function');
    }
    expect(result.current.anyDialogArmed).toBe(false);
  });
});

describe('useWorkspaceDialogState — resetArmedDialogs', () => {
  it('restores EVERY enumerated flag to its initial value', () => {
    const { result } = renderHook(() => useWorkspaceDialogState());
    const keys = flagKeys(result.current);
    const initial = new Map(keys.map((k) => [k, bagOf(result.current)[k]]));

    // Arm all of them through their own setters.
    act(() => {
      const bag = bagOf(result.current);
      for (const key of keys) {
        (bag[setterName(key)] as (v: unknown) => void)(armedValue(initial.get(key), key));
      }
    });
    // PRE-CONDITION: the arming actually took, so the post-teardown assertions
    // below cannot pass by never having changed anything.
    for (const key of keys) {
      expect(bagOf(result.current)[key], `${key} must be armed first`).not.toEqual(
        initial.get(key),
      );
    }
    expect(result.current.anyDialogArmed).toBe(true);

    act(() => result.current.resetArmedDialogs());

    // POST-TEARDOWN: every flag back to its initial value. A flag missing from
    // `resetArmedDialogs` fails HERE, naming itself.
    for (const key of keys) {
      expect(bagOf(result.current)[key], `${key} must be disarmed by the teardown`).toEqual(
        initial.get(key),
      );
    }
    expect(result.current.anyDialogArmed).toBe(false);
  });

  it('settles the parked Commit & Push promise as a cancel, not a success', () => {
    // The one paired resolver in this hook. Dropping `pendingCommitPush` without
    // settling it leaves CommitBox awaiting a promise nothing can resolve any
    // more (only that dialog's buttons settle it) — a hung submit button.
    const { result } = renderHook(() => useWorkspaceDialogState());
    const { resolver, resolve, reject } = resolverSpy();
    act(() => {
      result.current.commitPushResolver.current = resolver;
      result.current.setPendingCommitPush('feat: parked message');
    });

    act(() => result.current.resetArmedDialogs());

    expect(result.current.pendingCommitPush).toBeNull();
    // REJECT with the cancel sentinel, exactly as `handleCancelCommitPush` does:
    // nothing was committed, so CommitBox must keep the typed message and show no
    // error banner. Resolving would read as "commit succeeded" and clear the draft.
    expect(reject).toHaveBeenCalledWith(COMMIT_PUSH_CANCELED);
    expect(resolve).not.toHaveBeenCalled();
    // Detached, so a later Confirm/Cancel cannot double-settle it.
    expect(result.current.commitPushResolver.current).toBeNull();
  });

  it('is a no-op on the resolver when no Commit & Push is parked', () => {
    const { result } = renderHook(() => useWorkspaceDialogState());
    expect(() => act(() => result.current.resetArmedDialogs())).not.toThrow();
    expect(result.current.commitPushResolver.current).toBeNull();
  });
});
