// Split out of the former monolithic mock.ts (pure refactor; no behavior change).
import { fixtureOid, randomOid } from './oids';
import type { GraphFixture, RepoKind } from '../mock/repoState';
import type { StashEntry } from '../types';

/** Seed the DEFAULT repo's stash stack so the sidebar list and the graph pills
 *  (fixtures/graph.ts §6.6) tell the same story. Non-default repos get []. */
export function seedStashes(kind: RepoKind, graphFixture: GraphFixture): StashEntry[] {
  if (kind !== 'default' || graphFixture !== 'default') return [];
  const now = Math.floor(Date.now() / 1000);
  return [
    {
      index: 0,
      message: 'WIP on main: polish sidebar',
      oid: randomOid(),
      baseOid: fixtureOid(3), // `core work 4` — carries stash@{0} in the graph
      ts: now - 3600,
    },
    {
      index: 1,
      message: 'WIP on main: extract graph layout helpers',
      oid: randomOid(),
      baseOid: fixtureOid(6), // `core work 2` — carries stash@{1}
      ts: now - 7200,
    },
    {
      // Flagged (message contains 'reserved') so applyStash/popStash exercise the
      // Windows reserved-path recovery flow: first attempt → `reservedPaths`,
      // retry with skipReserved → `appliedSkippingReserved`. See RESERVED_STASH_*.
      index: 2,
      message: 'WIP on main: aspire host scaffolding (reserved-name files)',
      oid: randomOid(),
      baseOid: fixtureOid(6), // `core work 2` — carries stash@{2}
      ts: now - 10800,
    },
  ];
}

/** Mock-only marker: a stash whose message contains this substring is treated as
 *  containing a Windows-reserved path that cannot be checked out (mirrors the
 *  `'conflict'` demo-trigger convention). */
export const RESERVED_STASH_MARKER = 'reserved';
/** The reserved path reported/skipped for a flagged fixture stash. */
export const RESERVED_STASH_PATHS = ['src/Aspire.AppHost/NUL'];

/** True when this stash entry is the seeded reserved-path fixture. */
export function stashHasReserved(entry: StashEntry | undefined): boolean {
  return entry !== undefined && entry.message.includes(RESERVED_STASH_MARKER);
}
