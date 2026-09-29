// P125-ui §4.2 / §6 / §8 — the pure view derivations.
import { describe, expect, it } from 'vitest';

import { buildDepView } from '../../ipc/fixtures/depTracker';
import { group, row } from '../../ipc/fixtures/depTrackerModel';
import type { DepRow, DepTrackerView } from '../../ipc/types/depTracker';
import {
  ALL_PRIVATE_LEAD,
  NOT_VERIFIED_NO_VERSION,
  NOT_VERIFIED_NUGET,
  NOT_VERIFIED_OTHER,
  TOOLBAR_TITLE_CHECKING,
  TOOLBAR_TITLE_OFF,
  allPrivateHostLead,
  configUnreadableLead,
  joinAnd,
  reportWideLead,
} from './depsCopy';
import {
  NO_FILTER,
  availabilityWords,
  chipCount,
  deriveState,
  filterRows,
  firstSentence,
  groupPrivateLead,
  notVerifiedCause,
  privateReasonText,
  reportWideConfigLead,
  rowKey,
  rowTags,
  secondaryLine,
  someUnchecked,
  toolbarNaming,
  visibleGroups,
} from './depsView';
import type { DepsFilter } from './depsView';

const NOW = 1_800_000_000_000;
const view = (mode: Parameters<typeof buildDepView>[0]): DepTrackerView => buildDepView(mode, 'r', NOW);

describe('deriveState (§6)', () => {
  it('maps every harness mode to exactly one body and at most one notice', () => {
    expect(deriveState(null)).toMatchObject({ body: 'loading', notice: null });
    expect(deriveState(view('off'))).toMatchObject({ body: 'off', notice: null, stalePill: false });
    expect(deriveState(view('loading'))).toMatchObject({ body: 'loading', notice: null, running: true });
    expect(deriveState(view('error'))).toMatchObject({ body: 'failed', notice: null });
    expect(deriveState(view('empty'))).toMatchObject({ body: 'empty', notice: null, upToDate: false });
    expect(deriveState(view('populated'))).toMatchObject({ body: 'list', notice: null, upToDate: false });
    expect(deriveState(view('uptodate'))).toMatchObject({ body: 'list', notice: null, upToDate: true });
    expect(deriveState(view('stale'))).toMatchObject({ body: 'list', notice: { kind: 'stale' }, stalePill: true });
    const offline = deriveState(view('offline'));
    expect(offline.body).toBe('list');
    expect(offline.notice?.kind).toBe('failure');
    expect(offline.stalePill).toBe(true);
  });

  it('a failure outranks the stale notice (only ONE state notice)', () => {
    const v = { ...view('offline'), manifestsChanged: true };
    expect(deriveState(v).notice?.kind).toBe('failure');
  });
});

describe('filterRows / visibleGroups (§4.2)', () => {
  const rows: DepRow[] = [
    row({ name: 'alpha', ecosystem: 'npm', current: '1.0.0', latest: '2.0.0', update: 'major' }),
    row({ name: 'Beta', ecosystem: 'npm', current: '1.0.0', latest: '1.1.0', update: 'minor' }),
    row({ name: 'gamma', ecosystem: 'npm', current: '1.0.0', latest: '1.0.0' }),
    row({ name: 'delta', ecosystem: 'npm', current: '1.0.0', availability: { kind: 'local', reason: 'path' } }),
  ];
  const f = (over: Partial<DepsFilter>): DepsFilter => ({ ...NO_FILTER, ...over });

  it('no filter keeps every row, in order', () => {
    expect(filterRows(rows, NO_FILTER)).toEqual(rows);
  });
  it('kind toggles form a set and hide rows with neither update nor vulns', () => {
    expect(filterRows(rows, f({ kinds: new Set(['major']) })).map((r) => r.name)).toEqual(['alpha']);
    expect(filterRows(rows, f({ kinds: new Set(['major', 'minor']) })).map((r) => r.name)).toEqual([
      'alpha',
      'Beta',
    ]);
  });
  it('vulnerable AND kind narrow together', () => {
    const vulnRows = [
      ...rows,
      row({
        name: 'eps',
        ecosystem: 'npm',
        current: '1.0.0',
        latest: '1.0.1',
        update: 'patch',
        vulns: [{ id: 'X', aliases: [], summary: null, severity: 'high', cvssScore: null, modified: '', url: 'u' }],
      }),
    ];
    expect(filterRows(vulnRows, f({ vulnerable: true })).map((r) => r.name)).toEqual(['eps']);
    expect(filterRows(vulnRows, f({ vulnerable: true, kinds: new Set(['major']) }))).toEqual([]);
  });
  it('the name filter is a case-insensitive substring', () => {
    expect(filterRows(rows, f({ name: 'bet' })).map((r) => r.name)).toEqual(['Beta']);
  });
  it('notTracked and parseError groups survive every filter', () => {
    const groups = view('populated').report?.groups ?? [];
    const out = visibleGroups(groups, f({ name: 'zzz-no-such-package' }));
    expect(out.map((g) => g.group.state.kind).sort()).toEqual(['notTracked', 'parseError']);
  });
});

describe('row words (§4.4 / §8.4)', () => {
  it('rowKey is relation + name + version', () => {
    expect(rowKey(row({ name: 'a', ecosystem: 'npm', current: '1' }))).toBe('direct\u0000a\u00001');
  });
  it('tags: transitive alone, else the non-normal scope', () => {
    expect(rowTags(row({ name: 'a', ecosystem: 'npm', relation: 'transitive', scope: 'dev' }))).toEqual([
      'transitive',
    ]);
    expect(rowTags(row({ name: 'a', ecosystem: 'npm', scope: 'peer' }))).toEqual(['peer']);
    expect(rowTags(row({ name: 'a', ecosystem: 'npm' }))).toEqual([]);
  });
  it('private reason text per (reason, registryHost)', () => {
    expect(privateReasonText('customRegistry', 'h')).toEqual({ prefix: 'from', host: 'h' });
    expect(privateReasonText('customRegistry', null)).toEqual({ prefix: 'custom or local feed', host: null });
    expect(privateReasonText('scopedRegistry', 'h')).toEqual({ prefix: 'scoped to', host: 'h' });
    expect(privateReasonText('scopedRegistry', null).prefix).toBe('scoped registry');
    expect(privateReasonText('sourceMapping', 'h')).toEqual({ prefix: 'mapped to', host: 'h' });
    expect(privateReasonText('sourceMapping', null).prefix).toBe('private package source');
    expect(privateReasonText('goPrivate', null).prefix).toBe('listed in GOPRIVATE');
    expect(privateReasonText('configUnreadable', null).prefix).toBe("settings couldn't be read");
  });
  it('not-verified cause is derived from current + ecosystem', () => {
    expect(notVerifiedCause(row({ name: 'a', ecosystem: 'pypi' }))).toBe(NOT_VERIFIED_NO_VERSION);
    expect(notVerifiedCause(row({ name: 'a', ecosystem: 'nuget', current: '1.0.0' }))).toBe(NOT_VERIFIED_NUGET);
    expect(notVerifiedCause(row({ name: 'a', ecosystem: 'npm', current: '1.0.0' }))).toBe(NOT_VERIFIED_OTHER);
  });
  it('under the report-wide lead a config-unreadable row is just `not checked`', () => {
    const r = row({
      name: 'a',
      ecosystem: 'npm',
      availability: { kind: 'private', reason: 'configUnreadable', registryHost: null },
    });
    expect(availabilityWords(r, true)).toEqual({ pill: 'not checked', reason: null, title: null });
    expect(availabilityWords(r, false)?.pill).toBe('private — not checked');
  });
});

describe('leads (§4.3 / §8.3)', () => {
  const priv = (host: string | null, reason: 'customRegistry' | 'configUnreadable' = 'customRegistry') =>
    row({ name: `p${host ?? ''}${reason}`, ecosystem: 'npm', availability: { kind: 'private', reason, registryHost: host } });

  it('the all-private group lead picks the host variant only when every host agrees', () => {
    expect(groupPrivateLead(group('a', null, 'npm', [priv('h'), priv('h')], 0), false)).toBe(allPrivateHostLead('h'));
    expect(groupPrivateLead(group('a', null, 'npm', [priv('h'), priv(null)], 0), false)).toBe(ALL_PRIVATE_LEAD);
    expect(groupPrivateLead(group('a', null, 'npm', [priv(null, 'configUnreadable')], 0), false)).toBe(
      configUnreadableLead('npm'),
    );
    expect(groupPrivateLead(group('a', null, 'npm', [priv(null, 'configUnreadable')], 0), true)).toBeNull();
    const mixed = [priv('h'), row({ name: 'pub', ecosystem: 'npm', current: '1' })];
    expect(groupPrivateLead(group('a', null, 'npm', mixed, 0), false)).toBeNull();
  });

  it('the report-wide lead is inferred: ≥ 2 unreadable ecosystems, none checked', () => {
    expect(reportWideConfigLead(view('mirror').report?.groups ?? [])).toEqual(['npm', 'nuget']);
    // populated has ONE config-unreadable ecosystem (cargo), which also has checked rows.
    expect(reportWideConfigLead(view('populated').report?.groups ?? [])).toBeNull();
  });

  it('the report-wide copy joins 2 and 3 ecosystems, and names causes rather than one', () => {
    expect(joinAnd(['npm', 'NuGet'])).toBe('npm and NuGet');
    expect(joinAnd(['npm', 'Cargo', 'PyPI'])).toBe('npm, Cargo and PyPI');
    expect(reportWideLead(['npm', 'Cargo', 'PyPI'])).toBe(
      "Bonsai couldn't read or safely follow the registry settings for npm, Cargo and PyPI, so it treats those packages as private and doesn't send them. A malformed settings file, or a user settings folder on a network share, can cause this. Fix it, then check again.",
    );
  });

  it('skips the config-unreadable group lead when the group has no ecosystem', () => {
    const g = { ...group('a', null, 'npm', [priv(null, 'configUnreadable')], 0), ecosystem: null };
    expect(groupPrivateLead(g, false)).toBeNull();
  });
});

describe('summary lines (§4.2 / §8.3)', () => {
  it('secondary line lists the non-zero counts', () => {
    const s = view('uptodate').report?.summary;
    if (s === undefined) throw new Error('fixture');
    expect(secondaryLine(s)).toBe(`${s.upToDate} up to date · ${s.local} local`);
  });
  it('up-to-date appends the unchecked suffix when anything went unchecked', () => {
    const r = view('uptodate').report;
    if (r === null) throw new Error('fixture');
    expect(someUnchecked(r.groups, r.summary)).toBe(true);
  });
  it('firstSentence cuts at the first full stop', () => {
    expect(firstSentence('One — two. Three.')).toBe('One — two.');
  });
});

describe('toolbarNaming (§3 table)', () => {
  it('off / checking / counts / running', () => {
    expect(toolbarNaming(view('off'), NOW)).toMatchObject({ label: 'Dependencies', title: TOOLBAR_TITLE_OFF, chip: null });
    expect(toolbarNaming(view('loading'), NOW)).toMatchObject({ label: 'Dependencies', title: TOOLBAR_TITLE_CHECKING });
    const pop = view('populated');
    const n = pop.report?.summary.vulnerable ?? 0;
    const named = toolbarNaming(pop, NOW);
    expect(named.label).toBe(`Dependencies, ${n} vulnerable`);
    expect(named.title).toMatch(new RegExp(`^Dependencies — ${n} vulnerable, \\d+ updates · checked 25 minutes ago$`));
    expect(named.chip).toBe(n);
    const running = toolbarNaming({ ...pop, running: { checkId: 2, startedAtMs: NOW, done: 0, total: 0 } }, NOW);
    expect(running.label).toBe(`Dependencies, ${n} vulnerable, checking again`);
    expect(running.title.endsWith(' — checking again…')).toBe(true);
    expect(running.busy).toBe(true);
    const upd = toolbarNaming(view('uptodate'), NOW);
    expect(upd).toMatchObject({ label: 'Dependencies', title: 'Dependencies — No updates · checked 25 minutes ago', chip: null });
  });
  it('caps the chip at 99+', () => {
    expect(chipCount(99)).toBe('99');
    expect(chipCount(100)).toBe('99+');
  });
});
