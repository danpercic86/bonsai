// @vitest-environment jsdom
/** P125 E4 AC13 / E5 — the mock dependency tracker: every `?deps=` mode serves
 *  the fixture it names, each summary equals an INDEPENDENT recount of its rows
 *  (the Rust `summarize` rules, re-derived here rather than calling
 *  `summarizeFixture`), rows and groups follow the §5.2 order, and
 *  `depsCheckNow` emits started → 3×progress → finished. */
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { mockIpc } from '../../mock';
import {
  DEP_FAILURE_MESSAGES,
  DEP_MOCK_MODES,
  DEP_VIEW_ERROR,
  DEP_VIEW_LARGE,
  DEP_VIEW_LOADING,
  DEP_VIEW_MIRROR,
  DEP_VIEW_OFF,
  DEP_VIEW_OFFLINE,
  DEP_VIEW_POPULATED,
  DEP_VIEW_STALE,
  DEP_VIEW_UPTODATE,
} from '../../fixtures/depTracker';
import { DEEP_MANIFEST_PATH, GO_PSEUDO_VERSION, LONG_NPM_NAME } from '../../fixtures/depTrackerPopulated';
import { LARGE_ROW_COUNT, MIRROR_HOST } from '../../fixtures/depTrackerSynthetic';
import { sanitizeDepTrackerSettings } from '../depTrackerSettings';
import { MOCK_REPO_PATH, repos } from '../repoState';
import { DEPS_DISABLED_MESSAGE, MOCK_CHECK_STEP_MS, MOCK_CHECK_TOTAL, resetMockDepTrackerForTests } from './depTracker';
import type { DepReport, DepRow, DepSeverity, DepSummary, DepsChangedPayload } from '../../types/depTracker';

const RANK: Record<DepSeverity, number> = { critical: 4, high: 3, medium: 2, low: 1, unknown: 0 };

function setMode(mode: string | null): void {
  window.history.replaceState({}, '', mode === null ? '/' : `/?deps=${mode}`);
}

async function openRepo(): Promise<string> {
  const { repoId } = await mockIpc.openRepo(MOCK_REPO_PATH);
  return repoId;
}

/** Independent recount — deliberately written differently from the fixture's
 *  `summarizeFixture` so a shared bug cannot cancel out. */
function recount(report: DepReport): DepSummary {
  const rows: DepRow[] = report.groups.flatMap((g) => g.rows);
  const withVulns = rows.filter((r) => r.vulns.length > 0);
  const top = (r: DepRow): DepSeverity =>
    r.vulns.reduce((a, v) => (RANK[v.severity] > RANK[a] ? v.severity : a), r.vulns[0].severity);
  const bySev = (s: DepSeverity) => withVulns.filter((r) => top(r) === s).length;
  return {
    major: rows.filter((r) => r.update === 'major').length,
    minor: rows.filter((r) => r.update === 'minor').length,
    patch: rows.filter((r) => r.update === 'patch').length,
    vulnerable: withVulns.length,
    vulnerableBySeverity: {
      critical: bySev('critical'),
      high: bySev('high'),
      medium: bySev('medium'),
      low: bySev('low'),
      unknown: bySev('unknown'),
    },
    upToDate: rows.filter(
      (r) => r.availability.kind === 'checked' && r.relation === 'direct' && r.update === null,
    ).length,
    unknown: rows.filter((r) => r.availability.kind === 'unknown').length,
    local: rows.filter((r) => r.availability.kind === 'local').length,
    private: rows.filter((r) => r.availability.kind === 'private').length,
    notTracked: report.groups.filter((g) => g.state.kind === 'notTracked').length,
    parseErrors: report.groups.filter((g) => g.state.kind === 'parseError').length,
  };
}

/** Pairwise §5.2 check, re-stated from the contract text. */
function assertRowOrder(rows: DepRow[]): void {
  const key = (r: DepRow) => [
    -(r.vulns.length === 0 ? -1 : Math.max(...r.vulns.map((v) => RANK[v.severity]))),
    -(r.update === 'major' ? 3 : r.update === 'minor' ? 2 : r.update === 'patch' ? 1 : 0),
    r.relation === 'direct' ? 0 : 1,
  ];
  for (let i = 1; i < rows.length; i += 1) {
    const [a, b] = [key(rows[i - 1]), key(rows[i])];
    const cmp = a[0] - b[0] || a[1] - b[1] || a[2] - b[2];
    expect(cmp < 0 || (cmp === 0 && rows[i - 1].name <= rows[i].name), `${rows[i - 1].name} before ${rows[i].name}`).toBe(true);
  }
}

beforeEach(() => {
  repos.clear();
  resetMockDepTrackerForTests();
  window.localStorage.clear();
  setMode(null);
});
afterEach(() => {
  vi.useRealTimers();
  resetMockDepTrackerForTests();
  setMode(null);
});

describe('?deps= modes (AC13)', () => {
  it('serves a view for every mode, stamped with the caller repoId', async () => {
    const repoId = await openRepo();
    for (const mode of DEP_MOCK_MODES) {
      setMode(mode);
      const v = await mockIpc.getDepTracker(repoId);
      expect(v.repoId, mode).toBe(repoId);
      if (v.report !== null) expect(v.report.repoId, mode).toBe(repoId);
      expect(v.enabled, mode).toBe(mode !== 'off');
    }
  });

  it('with no ?deps the mode follows the mock depTracker setting', async () => {
    const repoId = await openRepo();
    expect((await mockIpc.getDepTracker(repoId)).enabled).toBe(false);
    await mockIpc.setUiSettings({ depTracker: { enabled: true, intervalHours: 12 } });
    const v = await mockIpc.getDepTracker(repoId);
    expect(v.enabled).toBe(true);
    expect(v.report?.groups.length).toBe(DEP_VIEW_POPULATED.report?.groups.length);
  });

  it('each mode has the E5 shape', () => {
    expect(DEP_VIEW_OFF).toMatchObject({ enabled: false, report: null, running: null, lastError: null, nextCheckMs: null });
    expect(DEP_VIEW_LOADING.report).toBeNull();
    expect(DEP_VIEW_LOADING.lastError).toBeNull();
    expect(DEP_VIEW_LOADING.running).not.toBeNull();
    expect(DEP_VIEW_STALE.manifestsChanged).toBe(true);
    expect(DEP_VIEW_OFFLINE.lastError?.kind).toBe('offline');
    expect(DEP_VIEW_OFFLINE.lastError?.message).toBe(DEP_FAILURE_MESSAGES.offline);
    const age = (DEP_VIEW_OFFLINE.lastError?.atMs ?? 0) - (DEP_VIEW_OFFLINE.report?.checkedAtMs ?? 0);
    expect(age).toBeGreaterThan(2 * 3_600_000);
    expect(DEP_VIEW_ERROR.report).toBeNull();
    expect(DEP_VIEW_ERROR.lastError).toMatchObject({ kind: 'service', httpStatus: 503 });
    const upd = DEP_VIEW_UPTODATE.report?.summary;
    expect((upd?.major ?? 1) + (upd?.minor ?? 1) + (upd?.patch ?? 1) + (upd?.vulnerable ?? 1)).toBe(0);
  });

  it('every report summary equals an independent recount, and rows/groups are §5.2-ordered', async () => {
    const repoId = await openRepo();
    for (const mode of DEP_MOCK_MODES) {
      setMode(mode);
      const { report } = await mockIpc.getDepTracker(repoId);
      if (report === null) continue;
      expect(report.summary, mode).toEqual(recount(report));
      const paths = report.groups.map((g) => g.manifestPath);
      const expected = [...paths].sort((a, b) => Number(a.includes('/')) - Number(b.includes('/')) || (a < b ? -1 : a > b ? 1 : 0));
      expect(paths, mode).toEqual(expected);
      for (const g of report.groups) assertRowOrder(g.rows);
    }
  });
});

describe('DEP_VIEW_POPULATED content (AC13 + [UI-c] / [UI-e])', () => {
  const report = DEP_VIEW_POPULATED.report as DepReport;
  const rows = report.groups.flatMap((g) => g.rows);

  it('covers every group state', () => {
    const byPath = new Map(report.groups.map((g) => [g.manifestPath, g]));
    expect(byPath.get('package.json')?.lockfilePath).toBe('pnpm-lock.yaml');
    expect(byPath.get('src-tauri/Cargo.toml')?.lockfilePath).toBe('src-tauri/Cargo.lock');
    expect(byPath.get('tools/requirements.txt')?.state).toEqual({ kind: 'noLockfile' });
    expect(byPath.get('android/build.gradle')?.state).toEqual({ kind: 'notTracked', label: 'Gradle' });
    expect(byPath.get('android/build.gradle')?.ecosystem).toBeNull();
    expect(byPath.get('broken/go.mod')?.state).toEqual({
      kind: 'parseError',
      file: 'broken/go.mod',
      category: 'invalidSyntax',
      line: 3,
      col: 1,
      message: 'invalid syntax at 3:1',
    });
    expect(byPath.has(DEEP_MANIFEST_PATH)).toBe(true);
    for (const g of report.groups) expect(g.transitiveChecked).toBeGreaterThanOrEqual(0);
    expect(report.groups.some((g) => g.transitiveChecked > 0)).toBe(true);
  });

  it('covers every row kind', () => {
    expect(rows.some((r) => r.update === 'patch')).toBe(true);
    expect(rows.some((r) => r.update === 'minor')).toBe(true);
    expect(rows.some((r) => r.update === 'major')).toBe(true);
    expect(rows.some((r) => r.relation === 'direct' && r.vulns.some((v) => v.severity === 'critical'))).toBe(true);
    expect(rows.some((r) => r.relation === 'transitive' && r.vulns.some((v) => v.severity === 'high'))).toBe(true);
    expect(rows.some((r) => r.availability.kind === 'local' && r.availability.reason === 'path')).toBe(true);
    expect(
      rows.some((r) => r.availability.kind === 'private' && r.availability.reason === 'scopedRegistry' && r.availability.registryHost !== null),
    ).toBe(true);
    expect(rows.some((r) => r.availability.kind === 'unknown' && r.vulns.length === 1)).toBe(true);
  });

  it('carries the long-content rows', () => {
    expect(LONG_NPM_NAME).toHaveLength(70);
    expect(rows.some((r) => r.name === LONG_NPM_NAME)).toBe(true);
    expect(rows.some((r) => r.current === GO_PSEUDO_VERSION)).toBe(true);
    const four = rows.find((r) => r.vulns.length === 4);
    expect(four?.vulns.some((v) => v.severity === 'unknown' && v.summary === null)).toBe(true);
    expect(four?.vulns.some((v) => v.aliases.length === 3)).toBe(true);
    expect(rows.some((r) => r.scope === 'dev' && r.relation === 'transitive')).toBe(true);
  });

  it('keeps the Rust row invariants (links, vulnsChecked, transitive fields)', () => {
    for (const r of rows) {
      const linkable = r.availability.kind === 'checked' || r.availability.kind === 'unknown';
      expect(r.registryUrl !== null, r.name).toBe(linkable);
      if (!linkable) expect(r.vulnsChecked, r.name).toBe(false);
      if (r.relation === 'transitive') {
        expect([r.declared, r.latest, r.update], r.name).toEqual([null, null, null]);
      }
      for (const v of r.vulns) expect(v.url).toBe(`https://osv.dev/vulnerability/${v.id}`);
      for (let i = 1; i < r.vulns.length; i += 1) {
        expect(RANK[r.vulns[i - 1].severity]).toBeGreaterThanOrEqual(RANK[r.vulns[i].severity]);
      }
    }
    expect(rows.find((r) => r.name === '@babel/traverse')?.registryUrl).toBe(
      'https://www.npmjs.com/package/@babel/traverse',
    );
  });
});

describe('mirror and large modes ([UI-c])', () => {
  it('mirror: every row in two groups is private customRegistry on the mirror host', () => {
    const groups = DEP_VIEW_MIRROR.report?.groups ?? [];
    expect(groups).toHaveLength(2);
    for (const r of groups.flatMap((g) => g.rows)) {
      expect(r.availability).toEqual({ kind: 'private', reason: 'customRegistry', registryHost: MIRROR_HOST });
      expect(r.registryUrl).toBeNull();
    }
  });

  it('large: one group of 1,240 rows', () => {
    const groups = DEP_VIEW_LARGE.report?.groups ?? [];
    expect(groups).toHaveLength(1);
    expect(groups[0].rows).toHaveLength(LARGE_ROW_COUNT);
    expect(LARGE_ROW_COUNT).toBe(1240);
    // Rust only lists a transitive entry when it has advisories.
    for (const r of groups[0].rows) if (r.relation === 'transitive') expect(r.vulns.length).toBeGreaterThan(0);
  });
});

describe('depsCheckNow and events', () => {
  it('rejects with the backend text when the tracker is disabled', async () => {
    const repoId = await openRepo();
    setMode('off');
    await expect(mockIpc.depsCheckNow(repoId)).rejects.toEqual({ kind: 'other', message: DEPS_DISABLED_MESSAGE });
    expect(DEPS_DISABLED_MESSAGE).toBe('dependency tracker is disabled');
  });

  it('rejects noRepo for an unknown repo (both commands)', async () => {
    setMode('populated');
    await expect(mockIpc.getDepTracker('nope')).rejects.toMatchObject({ kind: 'noRepo' });
    await expect(mockIpc.depsCheckNow('nope')).rejects.toMatchObject({ kind: 'noRepo' });
  });

  it('emits started → 3×progress → finished, joins a running check, then serves a fresh report', async () => {
    const repoId = await openRepo();
    setMode('stale');
    const seen: DepsChangedPayload[] = [];
    const off = await mockIpc.onDepsChanged((p) => seen.push(p));
    vi.useFakeTimers();
    const pending = mockIpc.depsCheckNow(repoId);
    await vi.advanceTimersByTimeAsync(60);
    const checkId = await pending;
    expect(checkId).toBeGreaterThanOrEqual(1);
    const again = mockIpc.depsCheckNow(repoId);
    await vi.advanceTimersByTimeAsync(60);
    expect(await again).toBe(checkId);

    const during = mockIpc.getDepTracker(repoId);
    await vi.advanceTimersByTimeAsync(100);
    const mid = await during;
    expect(mid.running?.checkId).toBe(checkId);
    expect(mid.report).not.toBeNull(); // the previous report stays visible (AC10)

    await vi.advanceTimersByTimeAsync(MOCK_CHECK_STEP_MS * 5);
    off();
    expect(seen.map((p) => p.kind)).toEqual(['started', 'progress', 'progress', 'progress', 'finished']);
    expect(seen.every((p) => p.repoId === repoId && p.checkId === checkId)).toBe(true);
    expect(seen.filter((p) => p.kind === 'progress').map((p) => [p.done, p.total])).toEqual([
      [19, MOCK_CHECK_TOTAL],
      [38, MOCK_CHECK_TOTAL],
      [MOCK_CHECK_TOTAL, MOCK_CHECK_TOTAL],
    ]);

    const after = mockIpc.getDepTracker(repoId);
    await vi.advanceTimersByTimeAsync(100);
    const v = await after;
    expect(v.running).toBeNull();
    expect(v.manifestsChanged).toBe(false);
    expect(v.lastError).toBeNull();
  });

  it('disabling in settings emits cleared; enabling starts a check for the active repo', async () => {
    const repoId = await openRepo();
    await mockIpc.setActiveRepo(repoId);
    const seen: DepsChangedPayload[] = [];
    const off = await mockIpc.onDepsChanged((p) => seen.push(p));
    await mockIpc.setUiSettings({ depTracker: { enabled: true, intervalHours: 12 } });
    expect(seen.map((p) => p.kind)).toEqual(['started']);
    await mockIpc.setUiSettings({ depTracker: { enabled: false, intervalHours: 12 } });
    off();
    expect(seen.map((p) => [p.kind, p.repoId])).toEqual([
      ['started', repoId],
      ['cleared', repoId],
    ]);
  });

  it('clamps the interval on write (1..=168)', async () => {
    expect((await mockIpc.setUiSettings({ depTracker: { enabled: false, intervalHours: 999 } })).depTracker.intervalHours).toBe(168);
    expect((await mockIpc.setUiSettings({ depTracker: { enabled: false, intervalHours: 0 } })).depTracker.intervalHours).toBe(1);
    expect((await mockIpc.getUiSettings()).depTracker).toEqual({ enabled: false, intervalHours: 1 });
  });

  it('a pre-P125 or garbled persisted blob reads back OFF / 12 h, never partial', () => {
    expect(sanitizeDepTrackerSettings(undefined)).toEqual({ enabled: false, intervalHours: 12 });
    expect(sanitizeDepTrackerSettings('x')).toEqual({ enabled: false, intervalHours: 12 });
    expect(sanitizeDepTrackerSettings({ enabled: 'yes', intervalHours: 500 })).toEqual({ enabled: false, intervalHours: 168 });
    expect(sanitizeDepTrackerSettings({ enabled: true, intervalHours: Number.NaN })).toEqual({ enabled: true, intervalHours: 12 });
  });
});
