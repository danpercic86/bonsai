/** P70 (UI §10.1/§10.2): the toast coalescing rule. The headline assertion is
 *  the INVARIANT — at most one toast with `key === 'git-not-found'` exists at
 *  any moment, regardless of press count or op mix — because error toasts are
 *  sticky and stacking them is the exact symptom P70 exists to kill. */
import { describe, it, expect } from 'vitest';

import { applyToastPush, TOAST_CAP } from './toastQueue';
import type { Toast } from './Toasts';
import { GIT_NOT_FOUND_TOAST_KEY, gitNotFoundToastText } from '../ipc/gitNotFound';

let nextId = 0;
function toast(text: string, over: Partial<Toast> = {}): Toast {
  return { id: ++nextId, tone: 'error', text, sticky: true, ...over };
}

function keyed(op: 'Fetch' | 'Pull' | 'Push'): Toast {
  return toast(gitNotFoundToastText(op), { key: GIT_NOT_FOUND_TOAST_KEY });
}

function countGitNotFound(list: Toast[]): number {
  return list.filter((t) => t.key === GIT_NOT_FOUND_TOAST_KEY).length;
}

describe('applyToastPush — keyless behaviour is unchanged', () => {
  it('appends and enforces the 5-toast cap by dropping the oldest non-sticky', () => {
    let list: Toast[] = [];
    for (let i = 0; i < 6; i++) {
      list = applyToastPush(list, toast(`t${i}`, { tone: 'info', sticky: false }));
    }
    expect(list).toHaveLength(TOAST_CAP);
    expect(list[0].text).toBe('t1'); // t0 dropped
    expect(list.at(-1)?.text).toBe('t5');
  });

  it('never coalesces two keyless toasts with identical text', () => {
    const list = applyToastPush(applyToastPush([], toast('same')), toast('same'));
    expect(list).toHaveLength(2);
  });
});

describe('applyToastPush — keyed coalescing', () => {
  it('same key + same text is a no-op RETURNING THE SAME ARRAY (no remount)', () => {
    const first = applyToastPush([], keyed('Fetch'));
    const second = applyToastPush(first, keyed('Fetch'));
    // Identity, not just equality: this is what tells App to skip the timer and
    // what stops React remounting (and the SR re-announcing) the toast.
    expect(second).toBe(first);
    expect(second).toHaveLength(1);
  });

  it('same key + different text replaces IN PLACE with a new id', () => {
    const before = applyToastPush(applyToastPush([], toast('unrelated')), keyed('Fetch'));
    const fetchToast = before[1];
    const after = applyToastPush(before, keyed('Pull'));

    expect(after).toHaveLength(2);
    expect(after[0]).toBe(before[0]); // the unrelated toast is untouched
    expect(after[1].text).toBe(gitNotFoundToastText('Pull')); // same slot
    expect(after[1].id).not.toBe(fetchToast.id); // new id
    expect(after[1].key).toBe(GIT_NOT_FOUND_TOAST_KEY);
  });

  it('INVARIANT: at most one git-not-found toast, whatever the press count/mix', () => {
    let list: Toast[] = [];
    const presses: Array<'Fetch' | 'Pull' | 'Push'> = [
      'Fetch',
      'Fetch',
      'Fetch',
      'Pull',
      'Push',
      'Push',
      'Fetch',
    ];
    for (const op of presses) {
      list = applyToastPush(list, keyed(op));
      expect(countGitNotFound(list)).toBe(1);
    }
    // The visible message names the op pressed LAST.
    expect(list[0].text).toBe(gitNotFoundToastText('Fetch'));
    expect(list).toHaveLength(1);
  });

  it('a dismissed keyed toast frees the slot — the user is never silenced', () => {
    const list = applyToastPush([], keyed('Fetch'));
    const dismissed = list.filter((t) => t.id !== list[0].id);
    const again = applyToastPush(dismissed, keyed('Fetch'));
    expect(again).toHaveLength(1);
  });

  it('an unkeyed error never displaces the keyed one', () => {
    let list = applyToastPush([], keyed('Fetch'));
    list = applyToastPush(list, toast('some other failure'));
    expect(countGitNotFound(list)).toBe(1);
    expect(list).toHaveLength(2);
  });
});
