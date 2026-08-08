// Signing + verification mock (P58). UI-plumbing only — the browser mock CANNOT
// actually sign or cryptographically verify; this drives the commit-box
// indicator / "will sign" copy AND the per-row/panel signature badges in the
// harness. A `?sign=ssh` / `?sign=gpg` (alias `openpgp`) query flips signing ON
// with a canned key (mirrors P37's `?remote=` seam); anything else ⇒ OFF (git's
// default). `verifyCommits` maps each oid to a DETERMINISTIC status from its
// first hex nibble so every badge state is exercised — the signer/key strings
// are canned and carry NO real signature. Real signing/verification is exercised
// only against the native backend.
import type { CommitVerification, IpcApi, SigningStatus, VerifyResults, VerifyStatus } from '../../types';
import { delay, query, requireRepo } from '../repoState';
import { resolveLayout } from './layout';

const MAX_VERIFY_BATCH = 512; // mirrors the Rust const (argv sanity)

function seededStatus(): SigningStatus {
  switch (query('sign')) {
    case 'ssh':
      return { enabled: true, format: 'ssh', hasKey: true, key: '/home/dev/.ssh/id_ed25519.pub' };
    case 'gpg':
    case 'openpgp':
      return { enabled: true, format: 'openpgp', hasKey: true, key: 'ABCD1234EF567890' };
    default:
      return { enabled: false, format: null, hasKey: false };
  }
}

// Deterministic verdict from the oid's first hex nibble, spread across every
// badge state so the harness can render each glyph. UI-plumbing only.
function mockStatus(oid: string): VerifyStatus {
  switch (oid[0]?.toLowerCase()) {
    case '0':
    case '1':
    case '8':
    case '9':
    case 'f':
      return 'good';
    case '2':
    case 'd':
      return 'goodUnknown';
    case '3':
    case 'a':
      return 'bad';
    case '4':
    case 'b':
      return 'expired';
    case '5':
      return 'expiredKey';
    case '6':
      return 'revoked';
    case '7':
    case 'c':
      return 'cannotCheck';
    default: // 'e' or anything unexpected ⇒ no signature
      return 'unsigned';
  }
}

export const signingHandlers = {
  async signingStatus(repoId: string): Promise<SigningStatus> {
    await delay(60);
    requireRepo(repoId); // honor the noRepo rejection
    return seededStatus();
  },

  async verifyCommits(repoId: string, oids: string[]): Promise<VerifyResults> {
    await delay(80);
    const state = requireRepo(repoId);
    // Oids present in the current layout are "resolvable"; any others are omitted
    // (mirrors the backend dropping unresolvable oids — the frontend keeps them
    // "unchecked").
    const known = new Set(resolveLayout(state).nodes.map((n) => n.id));
    const verifications: CommitVerification[] = [];
    for (const oid of oids.slice(0, MAX_VERIFY_BATCH)) {
      if (!known.has(oid)) continue;
      const status = mockStatus(oid);
      verifications.push(
        status === 'unsigned'
          ? { oid, status }
          : { oid, status, signer: 'Ada Lovelace <ada@example.com>', key: 'SHA256:mockAAAABBBBCCCCDDDD1111' },
      );
    }
    return { verifications };
  },
} satisfies Partial<IpcApi>;
