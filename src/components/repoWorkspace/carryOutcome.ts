import type { ApplyStashOutcome } from '../../ipc';

/** How the auto-stash carry went, rendered as a toast level + a sentence that
 *  follows the headline ("Switched to main; …"). `null` == a clean carry (or
 *  nothing to carry), so the caller shows its plain success headline.
 *
 *  Every non-clean branch is user-visible and names where the work is. The
 *  backend NEVER drops the stash in any of these cases, so the sentence can
 *  promise recovery unconditionally — that promise is what makes a branch
 *  switch safe by construction. */
export type CarryNotice = {
  level: 'info' | 'warning' | 'error';
  text: string;
};

/** Up to 5 paths, then "+N more" — enough to recognise the files without
 *  turning the toast into a file listing. */
function nameFiles(paths: string[]): string {
  const shown = paths.slice(0, 5).join(', ');
  return paths.length > 5 ? `${shown} +${paths.length - 5} more` : shown;
}

export function describeCarry(apply: ApplyStashOutcome | null | undefined): CarryNotice | null {
  if (!apply) return null;
  switch (apply.kind) {
    case 'applied':
      return null;
    case 'conflicts':
      return {
        level: 'warning',
        text: `your changes were carried over with ${apply.paths.length} conflict(s) and kept safe at stash@{0} — resolve them in the status panel`,
      };
    case 'appliedPartially':
      return {
        level: 'warning',
        text: `${apply.unrestored.length} new file(s) could not be restored here (${nameFiles(apply.unrestored)}) — nothing was lost, they are kept at stash@{0}`,
      };
    case 'reservedPaths':
      return {
        level: 'warning',
        text: `your changes could not be restored because the stash holds path(s) Windows cannot write (${nameFiles(apply.paths)}) — they are kept at stash@{0}`,
      };
    case 'appliedSkippingReserved':
      return {
        level: 'warning',
        text: `${apply.skipped.length} file(s) Windows cannot restore were skipped (${nameFiles(apply.skipped)}) — they are kept at stash@{0}`,
      };
    case 'notApplied':
      return {
        level: 'error',
        text: `your changes could not be re-applied (${apply.message}) — nothing was lost, they are safe at stash@{0}`,
      };
  }
}
