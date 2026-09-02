// The four secondary sidebar collections for one open repo — stashes,
// submodules, worktrees and remotes. Each is an identical last-wins fetch over
// its `list_*` command with a non-fatal error policy (a failure keeps the
// last-known list; these are secondary surfaces and must not block a refresh
// round). Extracted verbatim from RepoWorkspace so the container only wires
// them into `runRefreshRound` / the mount load.
import { useCallback, useRef, useState } from 'react';
import { ipc } from '../../ipc';
import type { RemoteInfo, StashEntry, SubmoduleInfo, WorktreeInfo } from '../../ipc';

export interface UseSidebarCollections {
  stashes: StashEntry[];
  submodules: SubmoduleInfo[];
  worktrees: WorktreeInfo[];
  remotes: RemoteInfo[];
  refetchStashes: () => Promise<void>;
  clearStashes: () => void;
  refetchSubmodules: () => Promise<void>;
  clearSubmodules: () => void;
  refetchWorktrees: () => Promise<void>;
  clearWorktrees: () => void;
  refetchRemotes: () => Promise<void>;
  clearRemotes: () => void;
}

export function useSidebarCollections(repoId: string): UseSidebarCollections {
  const [stashes, setStashes] = useState<StashEntry[]>([]);
  const [submodules, setSubmodules] = useState<SubmoduleInfo[]>([]);
  // P27 §6.3: worktrees (main first), refetched alongside submodules.
  const [worktrees, setWorktrees] = useState<WorktreeInfo[]>([]);
  // P22 §7.1: configured remotes (name + fetch URL), refetched alongside branches.
  const [remotes, setRemotes] = useState<RemoteInfo[]>([]);

  const stashesReqId = useRef(0);
  const submodulesReqId = useRef(0);
  const worktreesReqId = useRef(0);
  const remotesReqId = useRef(0);

  const refetchStashes = useCallback(async () => {
    const id = ++stashesReqId.current;
    try {
      const list = await ipc.listStashes(repoId);
      if (id !== stashesReqId.current) return;
      setStashes(list);
    } catch {
      if (id !== stashesReqId.current) return;
      // Non-fatal: stashes are a secondary surface; keep the last-known list.
    }
  }, [repoId]);

  const clearStashes = useCallback(() => {
    stashesReqId.current += 1;
    setStashes([]);
  }, []);

  const refetchSubmodules = useCallback(async () => {
    const id = ++submodulesReqId.current;
    try {
      const list = await ipc.listSubmodules(repoId);
      if (id !== submodulesReqId.current) return;
      setSubmodules(list);
    } catch {
      if (id !== submodulesReqId.current) return;
      // Non-fatal: submodules are a secondary surface; keep the last-known list.
    }
  }, [repoId]);

  const clearSubmodules = useCallback(() => {
    submodulesReqId.current += 1;
    setSubmodules([]);
  }, []);

  const refetchWorktrees = useCallback(async () => {
    const id = ++worktreesReqId.current;
    try {
      const list = await ipc.listWorktrees(repoId);
      if (id !== worktreesReqId.current) return;
      setWorktrees(list);
    } catch {
      if (id !== worktreesReqId.current) return;
      // Non-fatal: worktrees are a secondary surface; keep the last-known list.
    }
  }, [repoId]);

  const clearWorktrees = useCallback(() => {
    worktreesReqId.current += 1;
    setWorktrees([]);
  }, []);

  const refetchRemotes = useCallback(async () => {
    const id = ++remotesReqId.current;
    try {
      const list = await ipc.listRemotes(repoId);
      if (id !== remotesReqId.current) return;
      setRemotes(list);
    } catch {
      if (id !== remotesReqId.current) return;
      // Non-fatal: remotes are a secondary surface; keep the last-known list.
    }
  }, [repoId]);

  const clearRemotes = useCallback(() => {
    remotesReqId.current += 1;
    setRemotes([]);
  }, []);

  return {
    stashes,
    submodules,
    worktrees,
    remotes,
    refetchStashes,
    clearStashes,
    refetchSubmodules,
    clearSubmodules,
    refetchWorktrees,
    clearWorktrees,
    refetchRemotes,
    clearRemotes,
  };
}
