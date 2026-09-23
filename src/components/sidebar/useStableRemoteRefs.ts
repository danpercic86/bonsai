import { useRef } from 'react';
import type { RemoteBranchInfo } from '../../ipc';
import { structuralEqual } from '../../utils/structuralEqual';

/** One stable empty array for the no-snapshot case, so an absent snapshot does
 *  not mint a fresh `[]` identity on every render. */
const NO_REMOTE_BRANCHES: readonly RemoteBranchInfo[] = Object.freeze([]);

/**
 * P118b — hand back the remote-tracking refs with an identity that only changes
 * when they change STRUCTURALLY.
 *
 * Why it is needed: `refetchBranches` stores the branches snapshot through
 * `keepIfUnchanged` at WHOLE-SNAPSHOT granularity (`RepoWorkspace.tsx:797`), so
 * any LOCAL-branch change — an ahead count moving after a commit — replaces the
 * snapshot and hands `data.remote` a FRESH array identity even though `origin/*`
 * is byte-identical. That identity alone re-rendered the whole Remotes section,
 * its tree memo and its filter memos, for zero visual difference — measured as
 * one real render per mutation round by the pre-P118b assertion at
 * `Sidebar.busyChurn.test.tsx:241`, which now asserts zero.
 *
 * Why the during-render ref write is safe: this is a PURE IDENTITY CACHE. The
 * only effect of keeping the older reference is keeping a structurally
 * identical value, so a render React throws away — StrictMode's double invoke,
 * or a torn-down concurrent attempt — cannot lose information or make a later
 * render observe anything different. (Same during-render ref idiom as
 * `busyRef` in Sidebar.tsx and `repoWorkspace/useSidebarCallbacks.ts`.)
 */
export function useStableRemoteRefs(
  refs: readonly RemoteBranchInfo[] | undefined,
): readonly RemoteBranchInfo[] {
  const cached = useRef<readonly RemoteBranchInfo[]>(NO_REMOTE_BRANCHES);
  const next = refs ?? NO_REMOTE_BRANCHES;
  if (!structuralEqual(cached.current, next)) cached.current = next;
  return cached.current;
}
