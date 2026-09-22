// P118 (render-storm from refresh-round fan-out) — the sidebar's "an action is
// in flight" flag, carried by CONTEXT instead of by props.
//
// WHY THIS EXISTS. `actionsDisabled = busy || opActive` used to be a prop of
// `BranchesSection`, `RemotesSection` and every `BranchRow`. A mutation flips it
// twice — `setMutating(true)` before the git call, `setMutating(false)` in the
// `finally` AFTER `await refreshAll(...)` — and both flips land inside the same
// 500 ms render-tally window. That is TWO renders of every memoised section and
// EVERY row per mutation, measured in the 2026-09-22 Dev session as
// `BranchRow: 100 renders vs 25 instances` (4x: 2 real renders, doubled by
// StrictMode) and the matching `4 renders` tallies on both sections — i.e. the
// `render-storm` rule (`renders > 3 * instances`) firing 24 times.
//
// The flag splits cleanly in two, and the split is the fix:
//
//   * BEHAVIOURAL readers (a row's checkout gesture) never look different when
//     busy — they only need the value AT EVENT TIME. They read
//     `SidebarBusyRefContext`, whose value is a `useRef` box with a STABLE
//     identity, so the flip re-renders nothing at all. Same deps-by-ref idiom as
//     `repoWorkspace/useSidebarCallbacks.ts` and `contextMenuOpeners.ts`.
//
//   * VISUAL readers (the header `+` buttons, the create-branch input) really do
//     render differently, so they read `SidebarBusyContext`, whose value IS the
//     boolean. Consuming it re-renders on the flip — which is why only tiny leaf
//     components (`SidebarActionButton`, `BranchCreateRow`) may consume it, never
//     a section body. A section that consumed it would be back where we started.
import { createContext, useContext } from 'react';

/** Read-only view of the `useRef` box the Sidebar owns. */
export interface BusyRef {
  readonly current: boolean;
}

/** Outside a provider nothing is ever busy (shared rows also render in the
 *  status-panel file tree, which has no sidebar actions). */
const NEVER_BUSY: BusyRef = { current: false };

/** Event-time busy flag. STABLE identity — consuming it never re-renders. */
export const SidebarBusyRefContext = createContext<BusyRef>(NEVER_BUSY);

/** Rendered busy flag. Consuming it re-renders on every flip — leaf controls only. */
export const SidebarBusyContext = createContext<boolean>(false);

/** For handlers: `busyRef.current` read inside the event, never during render. */
export function useSidebarBusyRef(): BusyRef {
  return useContext(SidebarBusyRefContext);
}

/** For controls whose MARKUP changes while an action is in flight. */
export function useSidebarBusy(): boolean {
  return useContext(SidebarBusyContext);
}
