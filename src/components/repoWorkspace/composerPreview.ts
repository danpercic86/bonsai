/** P54c: commit-composer row "Preview" — resolve the changed file's section
 *  from the latest status snapshot (unstaged → untracked → staged) and fetch
 *  that file's workdir diff via the EXISTING IPC (no new path). Moved verbatim
 *  out of RepoWorkspace.tsx (spec-005 size offset — no behavior change). */
import { ipc } from '../../ipc';
import type { FileDiff, StatusEntry, StatusSnapshot } from '../../ipc';

export function composerPreviewFileDiff(
  status: StatusSnapshot | null,
  repoId: string,
  path: string,
): Promise<FileDiff> {
  let entry: StatusEntry | undefined;
  let staged = false;
  if (status !== null) {
    entry = status.unstaged.find((e) => e.path === path);
    if (entry === undefined) entry = status.untracked.find((e) => e.path === path);
    if (entry === undefined) {
      entry = status.staged.find((e) => e.path === path);
      staged = entry !== undefined;
    }
  }
  if (entry === undefined) {
    return Promise.reject(new Error(`No working-tree diff available for ${path}`));
  }
  return ipc.getWorkdirFileDiff(repoId, entry.path, entry.origPath, staged, false, false);
}
