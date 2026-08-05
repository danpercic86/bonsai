// P46 WS3: pick the next changed file to open in the diff overlay after staging.
// Pure + unit-testable; no React/IPC deps so the auto-advance logic can be
// tested in isolation from RepoWorkspace.

/** One entry in the visible "Changes" list, tagged with its origin section.
 *  Ordering must match StatusPanel: [...unstaged, ...untracked]. */
export interface WorkdirChange {
  section: 'unstaged' | 'untracked';
  path: string;
  origPath: string | null;
}

/**
 * Returns the next changed file to open after staging `stagedPaths`, given the
 * pre-stage visible order (unstaged then untracked). `openPath` is the file
 * currently shown in the overlay (being staged).
 *
 * Finds `openPath`'s index in `changes`, then scans FORWARD for the first entry
 * whose `path` is not in `stagedPaths`. Does not wrap. Returns `null` when no
 * such entry remains (last file, or "stage all" staged everything).
 */
export function nextFileAfter(
  changes: WorkdirChange[],
  openPath: string,
  stagedPaths: string[],
): WorkdirChange | null {
  const start = changes.findIndex((c) => c.path === openPath);
  if (start === -1) return null;
  const staged = new Set(stagedPaths);
  for (let i = start + 1; i < changes.length; i += 1) {
    if (!staged.has(changes[i].path)) return changes[i];
  }
  return null;
}
