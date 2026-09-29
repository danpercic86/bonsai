// P125h §7 / P125-ui §9 — the two P125h harness modes:
//
// - `?deps=unverified` (`DEP_VIEW_UNVERIFIED`): one public row per
//   `DepVulnsUnverifiedReason`, a partially malformed direct row (one readable
//   advisory + `malformedAdvisory`), a `responseTooLarge` row that carries its
//   placeholder advisory (`unrated`, `summary: null`) exactly as Rust emits it,
//   an all-malformed TRANSITIVE row (R2: shown as `not verified`, `vulns: []`),
//   one `unknown` / `tooLarge` row and one `private` / `localFeed` row.
// - `?deps=netprofile` (`DEP_VIEW_NETWORK_PROFILE`, `networkProfile: true`):
//   npm + PyPI + NuGet groups where every row is `private` / `configUnreadable`,
//   so the ONE report-wide lead shows (R3: from the flag only, never inferred).
//
// `DEP_VIEW_POPULATED` is untouched, so its counts and tests stay the same.
import type { DepManifestGroup, DepRow } from '../types/depTracker';
import { advisory, group, row, sortGroups } from './depTrackerModel';

/** The `?deps=unverified` group tables, §5.2-ordered. */
export const UNVERIFIED_GROUPS: readonly DepManifestGroup[] = sortGroups([
  group('package.json', 'package-lock.json', 'npm', [
    // `noVersion`: not in the lockfile, so only the declared range is known.
    row({ name: 'left-pad', ecosystem: 'npm', declared: '^1.3.0', latest: '1.3.0' }),
    // `malformedAdvisory` with no readable advisory: `not verified`, no chevron.
    row({
      name: 'node-forge',
      ecosystem: 'npm',
      declared: '^1.3.1',
      current: '1.3.1',
      latest: '1.3.1',
      vulnsUnverified: 'malformedAdvisory',
    }),
    // Partially malformed: one advisory survived, others were dropped ⇒ the
    // combined cell (severity · id · `not verified`) + the incomplete-list note.
    row({
      name: 'ws',
      ecosystem: 'npm',
      declared: '^8.17.0',
      current: '8.17.0',
      latest: '8.18.2',
      update: 'minor',
      vulnsUnverified: 'malformedAdvisory',
      vulns: [
        advisory('GHSA-3h5v-q93c-6h6q', 'high', 'ws affected by a DoS when handling a request with many HTTP headers', 7.5, [
          'CVE-2024-37890',
        ]),
      ],
    }),
    // `responseTooLarge`: Rust keeps a placeholder advisory (id only) for the
    // oversize record, so this row is always the combined case.
    row({
      name: 'lodash',
      ecosystem: 'npm',
      declared: '^4.17.20',
      current: '4.17.20',
      latest: '4.17.21',
      update: 'patch',
      vulnsUnverified: 'responseTooLarge',
      vulns: [advisory('GHSA-35jh-r3h4-6jhm', 'unknown', null, null)],
    }),
    // R2: an all-malformed transitive package is SHOWN as `not verified`.
    row({
      name: 'minimist',
      ecosystem: 'npm',
      relation: 'transitive',
      current: '1.2.5',
      vulnsUnverified: 'malformedAdvisory',
    }),
    // `unknown` / `tooLarge`: deps.dev's record was over the cap; OSV still ran.
    row({
      name: 'aws-sdk',
      ecosystem: 'npm',
      declared: '^2.1692.0',
      current: '2.1692.0',
      availability: { kind: 'unknown', reason: 'tooLarge' },
    }),
  ], 214),
  group('src/Api/Api.csproj', null, 'nuget', [
    // `nameCasing`: a resolved version, but no lockfile to confirm the casing.
    row({
      name: 'Newtonsoft.Json',
      ecosystem: 'nuget',
      declared: '13.0.3',
      current: '13.0.3',
      latest: '13.0.3',
      vulnsUnverified: 'nameCasing',
    }),
    // `private` / `localFeed`: a folder package source.
    row({
      name: 'Corp.Internal.Utils',
      ecosystem: 'nuget',
      declared: '2.4.0',
      current: '2.4.0',
      availability: { kind: 'private', reason: 'localFeed', registryHost: null },
    }),
  ], 0),
]);

function unreadable(ecosystem: 'npm' | 'pypi' | 'nuget', names: readonly string[]): DepRow[] {
  return names.map((name, i) =>
    row({
      name,
      ecosystem,
      declared: `${1 + i}.0.0`,
      current: `${1 + i}.0.2`,
      availability: { kind: 'private', reason: 'configUnreadable', registryHost: null },
    }),
  );
}

/** The `?deps=netprofile` groups. Report order (root first, then path asc) is
 *  npm, PyPI, NuGet — the `.csproj` is nested so the lead reads
 *  `npm, PyPI and NuGet`. */
export const NETWORK_PROFILE_GROUPS: readonly DepManifestGroup[] = sortGroups([
  group('package.json', 'pnpm-lock.yaml', 'npm', unreadable('npm', ['react', 'lodash', 'express']), 0),
  group('pyproject.toml', 'uv.lock', 'pypi', unreadable('pypi', ['requests', 'pydantic']), 0),
  group('src/App/App.csproj', 'src/App/packages.lock.json', 'nuget', unreadable('nuget', ['Serilog', 'Dapper']), 0),
]);
