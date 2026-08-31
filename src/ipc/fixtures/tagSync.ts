import type { TagSyncReport } from '../types';

// P77 tag-sync fixture. Exercises every SHIPPING status (in-sync · local-only ·
// stale · remote-only). `deleted-on-remote` is reserved and never emitted by the
// v1 backend, so it is absent here too (the mock mirrors backend behaviour).
//
// Tag names align with the sidebar Tags list seeded in `fixtures/branches.ts`
// (v0.1.0 · v0.2.0 · v0.3.0 · v0.9 · v1.0 · v1.1.0 · v2.0.0-rc.1); the report
// additionally advertises remote-only ghost tags absent locally.
//
// The flagship case is `v1.1.0` — an ANNOTATED tag force-moved on the remote
// (local peeled committish ≠ remote peeled committish) → `stale`. This models the
// real v1.1.0 drift bug P77 was built to surface and fix.

const OID = {
  v010: '1111111111111111111111111111111111111111',
  v020: '2222222222222222222222222222222222222222',
  v030: '3333333333333333333333333333333333333333',
  v09: '9090909090909090909090909090909090909090',
  v10: 'a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0',
  // v1.1.0: local kept the OLD target; the remote was force-moved to a new one.
  v110local: 'b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1',
  v110remote: 'c1c1c1c1c1c1c1c1c1c1c1c1c1c1c1c1c1c1c1c1',
  v200rc1: 'd2d2d2d2d2d2d2d2d2d2d2d2d2d2d2d2d2d2d2d2',
  // Remote-only ghosts (no local ref).
  v120: 'e1e1e1e1e1e1e1e1e1e1e1e1e1e1e1e1e1e1e1e1',
  v200: 'f2f2f2f2f2f2f2f2f2f2f2f2f2f2f2f2f2f2f2f2',
  // v1.5.0: FF-able stale — remote strictly descends from local (auto-sync MOVE).
  v150local: 'a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5',
  v150remote: 'b5b5b5b5b5b5b5b5b5b5b5b5b5b5b5b5b5b5b5b5',
} as const;

/**
 * P84: stale tags whose REMOTE committish strictly descends from the local one,
 * i.e. a fast-forward the auto-sync pass should MOVE. Any other stale tag is
 * treated as diverged/local-ahead and reported as `skippedDiverged`. (The real
 * backend derives this from `graph_descendant_of`; the mock reads this flag.)
 */
export const AUTO_SYNC_FF_TAGS: ReadonlySet<string> = new Set(['v1.5.0']);

/**
 * Builds a FRESH tag-sync report for `remote` (deep-cloned per call so the mock's
 * in-memory mutations — force-refresh / delete-remote — never leak between repos).
 */
export function buildTagSyncReport(remote: string): TagSyncReport {
  return {
    remote,
    entries: [
      // Both sides present, equal committish.
      { name: 'v0.1.0', status: 'in-sync', localOid: OID.v010, remoteOid: OID.v010, annotated: false },
      { name: 'v0.2.0', status: 'in-sync', localOid: OID.v020, remoteOid: OID.v020, annotated: true },
      // Present locally, never pushed.
      { name: 'v0.3.0', status: 'local-only', localOid: OID.v030, remoteOid: null, annotated: false },
      { name: 'v0.9', status: 'in-sync', localOid: OID.v09, remoteOid: OID.v09, annotated: false },
      { name: 'v1.0', status: 'in-sync', localOid: OID.v10, remoteOid: OID.v10, annotated: false },
      // Flagship: annotated tag force-moved upstream → local committish is stale.
      { name: 'v1.1.0', status: 'stale', localOid: OID.v110local, remoteOid: OID.v110remote, annotated: true },
      // FF-able stale: remote strictly ahead → auto-sync MOVES the local tag.
      { name: 'v1.5.0', status: 'stale', localOid: OID.v150local, remoteOid: OID.v150remote, annotated: false },
      // Present on remote, absent locally (ghost rows — "fetch this tag").
      { name: 'v1.2.0', status: 'remote-only', localOid: null, remoteOid: OID.v120, annotated: false },
      { name: 'v2.0.0', status: 'remote-only', localOid: null, remoteOid: OID.v200, annotated: true },
      // Another unpushed local.
      { name: 'v2.0.0-rc.1', status: 'local-only', localOid: OID.v200rc1, remoteOid: null, annotated: false },
    ],
  };
}
