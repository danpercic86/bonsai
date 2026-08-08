/** P58c: shared classification of a commit's signature {@link VerifyStatus} into
 *  the badge palette bucket (OQ7) plus a human label for the commit-details
 *  signature line. Single source of truth for BOTH the canvas badge draw
 *  ({@link ../graph/drawRowText}) and the React panel line
 *  ({@link ../components/CommitPanel}) so the two never drift. Pure, no canvas,
 *  no React. */

import type { VerifyStatus } from '../ipc';

/** Which badge-color bucket a status paints in (OQ7):
 *  - `good`    → green filled check (`good`)
 *  - `warn`    → red/amber warning triangle (`bad`/`expired`/`expiredKey`/`revoked`)
 *  - `unknown` → neutral hollow glyph (`goodUnknown`/`cannotCheck`)
 *  `unsigned` classifies to `null` — NOTHING is drawn/shown. */
export type VerifyBadgeKind = 'good' | 'warn' | 'unknown';

export function verifyBadgeKind(status: VerifyStatus): VerifyBadgeKind | null {
  switch (status) {
    case 'good':
      return 'good';
    case 'goodUnknown':
    case 'cannotCheck':
      return 'unknown';
    case 'bad':
    case 'expired':
    case 'expiredKey':
    case 'revoked':
      return 'warn';
    case 'unsigned':
      return null;
  }
}

/** Human-readable status text for the commit-details signature line. */
export function verifyStatusLabel(status: VerifyStatus): string {
  switch (status) {
    case 'good':
      return 'Good signature';
    case 'goodUnknown':
      return 'Signed, unverified signer';
    case 'bad':
      return 'BAD signature';
    case 'expired':
      return 'Expired signature';
    case 'expiredKey':
      return 'Signed with an expired key';
    case 'revoked':
      return 'Signed with a revoked key';
    case 'cannotCheck':
      return 'Cannot verify signature';
    case 'unsigned':
      return 'Unsigned';
  }
}
