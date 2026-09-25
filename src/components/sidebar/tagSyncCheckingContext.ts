// P121e — the tag-sync "a check is on the wire" flag, carried by CONTEXT
// instead of by props (the P118 `sidebarBusyContext.ts` pattern).
//
// `useTagSync` flips `checking` twice per check (true before `listTagSync`,
// false when it settles). As a prop of the memoised `TagsSection` that was one
// extra body render per check, and — because `state` used to go to `'checking'`
// on every re-check — the rows lost and regained their badges every 5 min. Now
// `state` keeps the settled verdict during a re-check and only the rollup pill
// renders the flag. Consuming this context re-renders on every flip, so ONLY
// the leaf `TagSyncRollup` may read it — never a section body.
import { createContext, useContext } from 'react';

/** Outside a provider no check is ever in flight. */
export const TagSyncCheckingContext = createContext<boolean>(false);

/** Leaf controls only — consuming re-renders on every flip. */
export function useTagSyncChecking(): boolean {
  return useContext(TagSyncCheckingContext);
}
