import { describe, expect, it } from 'vitest';

import { verifyBadgeKind, verifyStatusLabel } from './verifyBadge';
import type { VerifyBadgeKind } from './verifyBadge';
import type { VerifyStatus } from '../ipc';

// P58c: verifyBadge is the single source of truth shared by the canvas badge
// draw (drawRowText) and the React panel line (CommitPanel), so these two pure
// mappings must stay locked to the OQ7 palette + labels. Every VerifyStatus
// variant is listed explicitly (mirrors src/ipc/types.ts) so that adding a new
// status without extending verifyBadge.ts's switches shows up as a gap here.
const ALL_STATUSES: readonly VerifyStatus[] = [
  'good',
  'goodUnknown',
  'bad',
  'expired',
  'expiredKey',
  'revoked',
  'cannotCheck',
  'unsigned',
];

describe('verifyBadgeKind (OQ7 palette buckets)', () => {
  const cases: readonly [VerifyStatus, VerifyBadgeKind | null][] = [
    ['good', 'good'], // green filled check
    ['goodUnknown', 'unknown'], // signed, signer not established
    ['cannotCheck', 'unknown'], // can't verify -> neutral, not a warning
    ['bad', 'warn'], // red/amber warning triangle
    ['expired', 'warn'],
    ['expiredKey', 'warn'],
    ['revoked', 'warn'],
    ['unsigned', null], // NOTHING drawn / no panel line
  ];
  for (const [status, kind] of cases) {
    it(`maps '${status}' -> ${kind === null ? 'null (blank)' : kind}`, () => {
      expect(verifyBadgeKind(status)).toBe(kind);
    });
  }

  it('classifies only unsigned to null (all others draw a badge)', () => {
    for (const status of ALL_STATUSES) {
      const kind = verifyBadgeKind(status);
      if (status === 'unsigned') {
        expect(kind).toBeNull();
      } else {
        expect(kind).not.toBeNull();
        expect(['good', 'warn', 'unknown']).toContain(kind);
      }
    }
  });
});

describe('verifyStatusLabel', () => {
  const cases: readonly [VerifyStatus, string][] = [
    ['good', 'Good signature'],
    ['goodUnknown', 'Signed, unverified signer'],
    ['bad', 'BAD signature'],
    ['expired', 'Expired signature'],
    ['expiredKey', 'Signed with an expired key'],
    ['revoked', 'Signed with a revoked key'],
    ['cannotCheck', 'Cannot verify signature'],
    ['unsigned', 'Unsigned'],
  ];
  for (const [status, label] of cases) {
    it(`labels '${status}' -> "${label}"`, () => {
      expect(verifyStatusLabel(status)).toBe(label);
    });
  }

  it('gives every status a non-empty label', () => {
    for (const status of ALL_STATUSES) {
      expect(verifyStatusLabel(status).length).toBeGreaterThan(0);
    }
  });
});
