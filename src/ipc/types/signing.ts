// ---- P58: commit signing ---------------------------------------------------

/** `gpg.format` — how commits are signed. Mirrors the Rust `SignFormat`
 *  (lowercase). */
export type SignFormat = 'ssh' | 'openpgp';

/** Effective signing config for the commit-box indicator/toggle (P58a D6).
 *  Mirrors the Rust `SigningStatus` (camelCase). `enabled` = effective
 *  `commit.gpgsign`; `format` is null when `gpg.format` is unset (git default =
 *  openpgp); `hasKey` = `user.signingkey` set + non-empty; `key` (path or id) is
 *  omitted when unset. */
export interface SigningStatus {
  enabled: boolean;
  format: SignFormat | null;
  hasKey: boolean;
  key?: string;
}

/** `git log --format=%G?` verdict for one commit (P58b). Mirrors the Rust
 *  `VerifyStatus` (camelCase). Authoritative for BOTH ssh and openpgp — git owns
 *  the trust check. `unsigned` ⇒ no signature (badge stays blank). */
export type VerifyStatus =
  | 'good'
  | 'goodUnknown'
  | 'bad'
  | 'expired'
  | 'expiredKey'
  | 'revoked'
  | 'cannotCheck'
  | 'unsigned';

/** One commit's verification verdict (P58b). Mirrors the Rust
 *  `CommitVerification` (camelCase). `signer` (%GS) / `key` (%GK) are omitted
 *  when git reported them empty. */
export interface CommitVerification {
  oid: string;
  status: VerifyStatus;
  signer?: string;
  key?: string;
}

/** Result of `verifyCommits` (P58b): one entry per RESOLVABLE requested oid, in
 *  request order. Non-hex / unresolvable oids are omitted (kept "unchecked" by
 *  the frontend). Mirrors the Rust `VerifyResults`. */
export interface VerifyResults {
  verifications: CommitVerification[];
}
