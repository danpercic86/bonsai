// Signing status mock (P58a). UI-plumbing only — the browser mock CANNOT
// actually sign; this drives the commit-box indicator / "will sign" copy in the
// harness. A `?sign=ssh` / `?sign=gpg` (alias `openpgp`) query flips signing ON
// with a canned key (mirrors P37's `?remote=` seam); anything else ⇒ OFF (git's
// default). Real signing is exercised only against the native backend.
import type { IpcApi, SigningStatus } from '../../types';
import { delay, query, requireRepo } from '../repoState';

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

export const signingHandlers = {
  async signingStatus(repoId: string): Promise<SigningStatus> {
    await delay(60);
    requireRepo(repoId); // honor the noRepo rejection
    return seededStatus();
  },
} satisfies Partial<IpcApi>;
