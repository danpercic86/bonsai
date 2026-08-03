import type { RepoInfo, WorktreeInfo } from '../ipc';

export function shortOid(oid: string): string {
  return oid.slice(0, 7);
}

/** P23d: how many file-history entries to request (backend caps at MAX_HISTORY). */
export const MAX_HISTORY_UI = 200;

/** P30 D11: compact "Xm" / "Xh" label for the auto-fetch status readout. */
export function minutesLabel(deltaMs: number): string {
  const m = Math.round(Math.max(0, deltaMs) / 60_000);
  if (m < 1) return '<1m';
  if (m < 60) return `${m}m`;
  return `${Math.round(m / 60)}h`;
}

export function isUsableRepo(info: RepoInfo): boolean {
  return info.isRepo && !info.bare;
}

/** P27 §6.5: display-only preview base for the derived worktree path —
 *  `<mainParent>/.worktrees` (the backend derives the authoritative path). */
export function worktreeContainerPreview(worktrees: WorktreeInfo[], repoId: string): string {
  const main = worktrees.find((w) => w.isMain);
  const base = (main?.absPath ?? repoId).replace(/\\/g, '/');
  const cut = base.lastIndexOf('/');
  const parent = cut > 0 ? base.slice(0, cut) : base;
  return `${parent}/.worktrees`;
}
