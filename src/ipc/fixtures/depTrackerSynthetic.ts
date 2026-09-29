// P125 E5 [UI-c] — the two generated fixture modes: `?deps=mirror` (the Q2
// corporate-mirror case: every row in two groups is `private`) and
// `?deps=large` (one group of 1,240 rows, to exercise "Show all").
//
// Generated, never literal, and deterministic (index-driven, no randomness) so
// screenshots and tests are stable.
import type { DepAdvisory, DepManifestGroup, DepRow, DepUpdateKind } from '../types/depTracker';
import { advisory, group, row, sortGroups } from './depTrackerModel';

/** [UI-c] the mirror host every `private` row names. */
export const MIRROR_HOST = 'npm.corp.example';
/** [UI-c] the `?deps=large` row count. */
export const LARGE_ROW_COUNT = 1240;

const MIRROR_NAMES = [
  'react',
  'react-dom',
  'lodash',
  'express',
  'dayjs',
  'classnames',
  'uuid',
  'chalk',
  'commander',
  'semver',
];

function mirrorRows(names: readonly string[]): DepRow[] {
  return names.map((name, i) =>
    row({
      name,
      ecosystem: 'npm',
      scope: i % 4 === 3 ? 'dev' : 'normal',
      declared: `^${1 + (i % 5)}.${i % 7}.0`,
      current: `${1 + (i % 5)}.${i % 7}.${i % 3}`,
      availability: { kind: 'private', reason: 'customRegistry', registryHost: MIRROR_HOST },
    }),
  );
}

/** `DEP_VIEW_MIRROR` groups: two groups, every row private. */
export const MIRROR_GROUPS: readonly DepManifestGroup[] = sortGroups([
  group('package.json', 'package-lock.json', 'npm', mirrorRows(MIRROR_NAMES.slice(0, 6)), 0),
  group('apps/web/package.json', 'package-lock.json', 'npm', mirrorRows(MIRROR_NAMES.slice(4)), 0),
]);

const UPDATE_CYCLE: readonly (DepUpdateKind | null)[] = [null, 'patch', null, 'minor', null, null, 'major'];

function largeVulns(i: number): DepAdvisory[] {
  if (i % 97 === 0) return [advisory(`GHSA-mock-${String(i).padStart(4, '0')}-crit`, 'critical', `Prototype pollution in large-fixture package #${i}`, 9.1)];
  if (i % 41 === 0) return [advisory(`GHSA-mock-${String(i).padStart(4, '0')}-high`, 'high', `Regular expression denial of service in package #${i}`, 7.5)];
  if (i % 23 === 0) return [advisory(`GHSA-mock-${String(i).padStart(4, '0')}-low`, 'low', null, 3.1)];
  return [];
}

function largeRow(i: number): DepRow {
  const name = `@large/pkg-${String(i).padStart(4, '0')}`;
  const current = `${1 + (i % 9)}.${i % 13}.${i % 5}`;
  if (i % 50 === 7) {
    return row({ name, ecosystem: 'npm', relation: 'transitive', current, vulns: largeVulns(i) });
  }
  const update = UPDATE_CYCLE[i % UPDATE_CYCLE.length] ?? null;
  const latest =
    update === 'major'
      ? `${2 + (i % 9)}.0.0`
      : update === 'minor'
        ? `${1 + (i % 9)}.${(i % 13) + 1}.0`
        : update === 'patch'
          ? `${1 + (i % 9)}.${i % 13}.${(i % 5) + 1}`
          : current;
  return row({
    name,
    ecosystem: 'npm',
    scope: i % 6 === 0 ? 'dev' : 'normal',
    declared: `^${current}`,
    current,
    latest,
    update,
    vulns: largeVulns(i),
  });
}

/** `DEP_VIEW_LARGE` groups: ONE group of {@link LARGE_ROW_COUNT} rows. The
 *  transitive rows are only the vulnerable ones — Rust never lists a clean
 *  transitive entry — so `i % 50 === 7` rows that draw no advisory get one. */
export const LARGE_GROUPS: readonly DepManifestGroup[] = [
  group(
    'package.json',
    'pnpm-lock.yaml',
    'npm',
    Array.from({ length: LARGE_ROW_COUNT }, (_, i) => {
      const r = largeRow(i + 1);
      if (r.relation === 'transitive' && r.vulns.length === 0) {
        return {
          ...r,
          vulns: [advisory(`GHSA-mock-${String(i + 1).padStart(4, '0')}-med`, 'medium', `Transitive advisory #${i + 1}`, 5.0)],
        };
      }
      return r;
    }),
    2890,
  ),
];
