import { describe, expect, it } from 'vitest';

import { makeMockConfigStore, type MockConfigStore } from '../fixtures/config';
import {
  HOOK_FAIL_SENTINEL,
  MOCK_HOOK_OUTPUT,
  MOCK_PRE_PUSH_OUTPUT,
  hookRejectionFor,
  prePushRejectionFor,
} from './hooksGate';
import type { MockRepoState } from './repoState';

// P59a/a-2: lock the browser-harness hook gate — the seams the orchestrator drives
// (`?hooks=fail`, `?hooks=failpush`, the `#hookfail` sentinel, the "Skip hooks" /
// "Commit anyway" bypass, and the `bonsai.runHooks` Settings toggle). These pure
// helpers mirror the Rust `hooks_enabled` truth table; the Rust CLI oracle proves
// the real `git hook run` path, this proves the mock path stays faithful.

/** Minimal state — the gate helpers only read `config` + the two hooks flags. */
function stateWith(opts: {
  config?: MockConfigStore;
  hooksFail?: boolean;
  hooksFailPush?: boolean;
}): MockRepoState {
  return {
    config: opts.config ?? makeMockConfigStore(),
    hooksFail: opts.hooksFail ?? false,
    hooksFailPush: opts.hooksFailPush ?? false,
  } as unknown as MockRepoState;
}

/** A config store with an explicit `bonsai.runHooks` value at the local level. */
function configWithRunHooks(value: string): MockConfigStore {
  const store = makeMockConfigStore();
  store.local['bonsai.runHooks'] = value;
  return store;
}

describe('hookRejectionFor (commit / amend / merge gate)', () => {
  it('rejects with the mock hook output when the ?hooks=fail flag is set', () => {
    const err = hookRejectionFor(stateWith({ hooksFail: true }), 'feat: add x');
    expect(err).toEqual({ kind: 'hookRejected', message: MOCK_HOOK_OUTPUT });
  });

  it('rejects on the #hookfail message sentinel even without the flag', () => {
    const err = hookRejectionFor(stateWith({ hooksFail: false }), `wip ${HOOK_FAIL_SENTINEL}`);
    expect(err).toEqual({ kind: 'hookRejected', message: MOCK_HOOK_OUTPUT });
  });

  it('returns null for a clean commit (no flag, no sentinel)', () => {
    expect(hookRejectionFor(stateWith({}), 'feat: clean commit')).toBeNull();
  });

  it('skipHooks bypasses a failing hook (Commit anyway / --no-verify)', () => {
    expect(hookRejectionFor(stateWith({ hooksFail: true }), 'msg', true)).toBeNull();
    // The sentinel is also bypassed by an explicit skip.
    expect(hookRejectionFor(stateWith({}), `x ${HOOK_FAIL_SENTINEL}`, true)).toBeNull();
  });

  it('bonsai.runHooks=false disables the gate (Run git hooks toggle off)', () => {
    const state = stateWith({ config: configWithRunHooks('false'), hooksFail: true });
    expect(hookRejectionFor(state, 'msg')).toBeNull();
  });

  it('an explicit bonsai.runHooks=true keeps the gate enabled', () => {
    const state = stateWith({ config: configWithRunHooks('true'), hooksFail: true });
    expect(hookRejectionFor(state, 'msg')).toEqual({
      kind: 'hookRejected',
      message: MOCK_HOOK_OUTPUT,
    });
  });
});

describe('prePushRejectionFor (pre-push gate, P59a-2)', () => {
  it('rejects with the pre-push output when ?hooks=failpush is set', () => {
    const err = prePushRejectionFor(stateWith({ hooksFailPush: true }));
    expect(err).toEqual({ kind: 'hookRejected', message: MOCK_PRE_PUSH_OUTPUT });
  });

  it('returns null when no pre-push hook fails', () => {
    expect(prePushRejectionFor(stateWith({}))).toBeNull();
  });

  it('skipHooks bypasses a failing pre-push hook (Push anyway)', () => {
    expect(prePushRejectionFor(stateWith({ hooksFailPush: true }), true)).toBeNull();
  });

  it('bonsai.runHooks=false disables the pre-push gate too', () => {
    const state = stateWith({ config: configWithRunHooks('false'), hooksFailPush: true });
    expect(prePushRejectionFor(state)).toBeNull();
  });
});

describe('mock hook-output prefixes drive the HookOutputDialog op label', () => {
  // HookOutputDialog derives its primary-action label inline from the message
  // heading (`message.startsWith('pre-push') ? 'push' : 'commit'`) — it is NOT an
  // exported pure helper, so rather than refactor the component we lock the mock
  // outputs that feed it: the commit gate must read "Commit anyway", the push gate
  // "Push anyway".
  it('the commit hook output begins with "pre-commit hook failed:" (→ Commit anyway)', () => {
    expect(MOCK_HOOK_OUTPUT.startsWith('pre-commit hook failed:')).toBe(true);
    expect(MOCK_HOOK_OUTPUT.startsWith('pre-push')).toBe(false);
  });

  it('the pre-push hook output begins with "pre-push hook failed:" (→ Push anyway)', () => {
    expect(MOCK_PRE_PUSH_OUTPUT.startsWith('pre-push hook failed:')).toBe(true);
  });
});
