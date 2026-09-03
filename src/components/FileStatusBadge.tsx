/** P109 §5: the single owner of the file-status glyph and its accessible name.
 *
 *  Before this file the letter table was duplicated in six components and had
 *  already drifted: `untracked` rendered `A` in three of them and `U` in the
 *  other three, so the SAME file read as two different states depending on which
 *  panel you were looking at. The fix is not a seventh table — it is that the
 *  letter and the word are now physically inseparable, so adding a `FileStatus`
 *  variant is a one-file change.
 *
 *  P109 D-A: `untracked` is `U` everywhere. It was already half the app, the
 *  design system's own family name is "A/M/D/U/R", and porcelain's `A` means
 *  "added to the index" — which is precisely Bonsai's `added`.
 *
 *  P109 D-B: the glyph carries an image role so the label REPLACES the character
 *  in the accessibility tree instead of prefixing it. A hidden sibling span would
 *  make a 200-row list read "Untracked U" on every row and would change
 *  `textContent`; a bare label on a generic element is invalid ARIA (P95 §1.2). */
import type { JSX } from 'react';
import type { FileStatus } from '../ipc';

/** `unknown` is not a `FileStatus`: it is the composer's fallback for a path that
 *  is absent from the working-dir snapshot (`ComposerGroupCard`). */
export type BadgeStatus = FileStatus | 'unknown';

const LETTERS: Record<BadgeStatus, string> = {
  added: 'A',
  modified: 'M',
  deleted: 'D',
  renamed: 'R',
  typechange: 'T',
  untracked: 'U',
  conflicted: 'C',
  unknown: '?',
};

/** P109 §6: the badge names a STATE, so it takes the state vocabulary already in
 *  use at `DiffOverlay`, `WorktreeCopyCandidates` and `RepoHealthPanel`. "New
 *  file" is the CONSEQUENCE vocabulary and stays in destructive copy.
 *  `Status unknown` is inverted on purpose — "Unknown notes/todo.txt" would read
 *  as "we don't know this file" rather than "we don't know its status". */
const LABELS: Record<BadgeStatus, string> = {
  added: 'Added',
  modified: 'Modified',
  deleted: 'Deleted',
  renamed: 'Renamed',
  typechange: 'Type changed',
  conflicted: 'Conflicted',
  untracked: 'Untracked',
  unknown: 'Status unknown',
};

export function FileStatusBadge({ status }: { status: BadgeStatus }): JSX.Element {
  return (
    <span className="file-badge mono" role="img" aria-label={LABELS[status]}>
      {LETTERS[status]}
    </span>
  );
}
