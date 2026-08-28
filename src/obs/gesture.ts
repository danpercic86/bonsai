/**
 * P91 §2.4/§2.5 — gesture origination for the instrumented surfaces.
 *
 * `traced(origin, label, fn)` wraps an event handler so that invoking it mints a
 * trace for the synchronous extent of the handler's prefix (via `withTrace`,
 * which also emits the single `gesture` record). Because the trace is cleared on
 * the first `await`, any handler that arms echo suppression / triggers a refresh
 * must **capture `currentTrace()?.trace` at its synchronous entry** and thread it
 * by value into `refreshAll` — the ambient is never read after an `await`
 * (unbound continuations log `trace: undefined`, never a stale one — §2.5).
 *
 * Labels are stable `<area>.<noun>.<verb>` strings, never free text, so they are
 * greppable record keys. Centralised here for the clusters wired in increment 4.
 */
import { withTrace } from './trace';
import type { TraceOrigin } from './types';

/** Stable gesture labels for the six-surface mutation/refresh flows (§12). */
export const GESTURES = {
  branchCheckout: 'sidebar.branch.checkout',
  branchCreate: 'sidebar.branch.create',
  branchCreateHere: 'graph.branch.createHere',
  branchDelete: 'sidebar.branch.delete',
  branchRename: 'sidebar.branch.rename',
  checkoutCommit: 'graph.commit.checkout',
  checkoutRemote: 'sidebar.remoteBranch.checkout',
  deleteRemoteTracking: 'sidebar.remoteBranch.delete',
  fetch: 'toolbar.remote.fetch',
  pull: 'toolbar.remote.pull',
  push: 'toolbar.remote.push',
  commitSubmit: 'commit.commit.submit',
  stashPush: 'sidebar.stash.push',
  stashApply: 'sidebar.stash.apply',
  stashPop: 'sidebar.stash.pop',
  stashDrop: 'sidebar.stash.drop',
  tagCreate: 'sidebar.tag.create',
  tagDelete: 'sidebar.tag.delete',
  tagPush: 'sidebar.tag.push',
  tagForceRefresh: 'sidebar.tag.forceRefresh',
  tagFetchRemote: 'sidebar.tag.fetchRemote',
  tagDeleteRemote: 'sidebar.tag.deleteRemote',
  tagForceMoveRemote: 'sidebar.tag.forceMoveRemote',
  remoteAdd: 'sidebar.remote.add',
  remoteRemove: 'sidebar.remote.remove',
  remoteRename: 'sidebar.remote.rename',
  submoduleUpdate: 'sidebar.submodule.update',
  submoduleAdd: 'sidebar.submodule.add',
  submoduleDeinit: 'sidebar.submodule.deinit',
  submoduleRemove: 'sidebar.submodule.remove',
} as const;

/** Wrap an event handler so invoking it originates a trace (and its `gesture`
 *  record). Preserves the handler's arguments and return value. */
export function traced<A extends unknown[], R>(
  origin: TraceOrigin,
  label: string,
  fn: (...args: A) => R,
): (...args: A) => R {
  return (...args: A): R => withTrace(origin, label, () => fn(...args));
}
