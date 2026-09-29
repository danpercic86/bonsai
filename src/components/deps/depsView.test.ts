// P125-ui §4.2 / §6 / §8 — the pure view derivations.
import { describe, expect, it } from 'vitest';

import { buildDepView } from '../../ipc/fixtures/depTracker';
import { group, row } from '../../ipc/fixtures/depTrackerModel';
import type { DepRow, DepTrackerView } from '../../ipc/types/depTracker';
import {
  ALL_PRIVATE_LEAD,
  INCOMPLETE_MALFORMED,
  INCOMPLETE_TOO_LARGE,
  NOT_VERIFIED_MALFORMED,
  NOT_VERIFIED_NO_VERSION,
  NOT_VERIFIED_NUGET,
  NOT_VERIFIED_OTHER,
  NOT_VERIFIED_TOO_LARGE,
  PRIVATE_TITLE_LOCAL_FEED,
  TOOLBAR_TITLE_CHECKING,
  TOOLBAR_TITLE_OFF,
  UNKNOWN_TITLE,
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
  incompleteListNote,
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
    // `row()` forces transitive rows to `normal` (P125h §6), so build the
    // off-contract shape by hand: rowTags must still never double-tag it.
    const devTransitive: DepRow = { ...row({ name: 'a', ecosystem: 'npm', relation: 'transitive' }), scope: 'dev' };
    expect(rowTags(devTransitive)).toEqual(['transitive']);
    expect(row({ name: 'a', ecosystem: 'npm', relation: 'transitive', scope: 'dev' }).scope).toBe('normal');
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
    // P125h: `localFeed` has no with-host entry — a host never renders.
    expect(privateReasonText('localFeed', null)).toEqual({ prefix: 'folder feed', host: null });
    expect(privateReasonText('localFeed', 'h')).toEqual({ prefix: 'folder feed', host: null });
  });
  it('localFeed words: pill, `folder feed`, its own title', () => {
    const r = row({
      name: 'a',
      ecosystem: 'nuget',
      current: '1.0.0',
      availability: { kind: 'private', reason: 'localFeed', registryHost: null },
    });
    expect(availabilityWords(r, false)).toEqual({
      pill: 'private — not checked',
      reason: { prefix: 'folder feed', host: null },
      title: PRIVATE_TITLE_LOCAL_FEED,
    });
  });
  it('unknown words by reason: notFound / tooLarge', () => {
    const unk = (reason: 'notFound' | 'tooLarge') =>
      availabilityWords(row({ name: 'a', ecosystem: 'npm', current: '1', availability: { kind: 'unknown', reason } }), false);
    expect(unk('notFound')).toEqual({
      pill: 'unknown',
      reason: { prefix: 'not found on deps.dev', host: null },
      title: UNKNOWN_TITLE.notFound,
    });
    expect(unk('tooLarge')).toEqual({
      pill: 'unknown',
      reason: { prefix: 'too large to look up', host: null },
      title: UNKNOWN_TITLE.tooLarge,
    });
  });
  it('not-verified cause is a lookup on `vulnsUnverified` (null ⇒ the old-cache text)', () => {
    const r = (vulnsUnverified: DepRow['vulnsUnverified']): DepRow => ({
      ...row({ name: 'a', ecosystem: 'npm', current: '1.0.0' }),
      vulnsChecked: false,
      vulnsUnverified,
    });
    expect(notVerifiedCause(r('noVersion'))).toBe(NOT_VERIFIED_NO_VERSION);
    expect(notVerifiedCause(r('nameCasing'))).toBe(NOT_VERIFIED_NUGET);
    expect(notVerifiedCause(r('malformedAdvisory'))).toBe(NOT_VERIFIED_MALFORMED);
    expect(notVerifiedCause(r('responseTooLarge'))).toBe(NOT_VERIFIED_TOO_LARGE);
    expect(notVerifiedCause(r(null))).toBe(NOT_VERIFIED_OTHER);
    // No longer derived from the ecosystem: a NuGet row with a malformed answer says so.
    expect(notVerifiedCause({ ...r('malformedAdvisory'), ecosystem: 'nuget' })).toBe(NOT_VERIFIED_MALFORMED);
    // `row()` applies `noVersion` itself when there is no resolved version.
    expect(notVerifiedCause(row({ name: 'a', ecosystem: 'pypi' }))).toBe(NOT_VERIFIED_NO_VERSION);
  });
  it('incomplete-list note only for the two reasons that coexist with advisories', () => {
    const r = (vulnsUnverified: DepRow['vulnsUnverified']): DepRow => ({
      ...row({ name: 'a', ecosystem: 'npm', current: '1.0.0' }),
      vulnsUnverified,
    });
    expect(incompleteListNote(r('malformedAdvisory'))).toBe(INCOMPLETE_MALFORMED);
    expect(incompleteListNote(r('responseTooLarge'))).toBe(INCOMPLETE_TOO_LARGE);
    expect(incompleteListNote(r('noVersion'))).toBeNull();
    expect(incompleteListNote(r('nameCasing'))).toBeNull();
    expect(incompleteListNote(r(null))).toBeNull();
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

  it('the report-wide lead comes from `networkProfile` only (R3: no inference)', () => {
    const mirror = view('mirror').report;
    if (mirror === null) throw new Error('fixture');
    // Two unreadable ecosystems, nothing checked — but no flag ⇒ no lead.
    expect(mirror.networkProfile).toBe(false);
    expect(reportWideConfigLead(mirror.groups, mirror.networkProfile)).toBeNull();
    // The same groups WITH the flag ⇒ the lead, in report order.
    expect(reportWideConfigLead(mirror.groups, true)).toEqual(['npm', 'nuget']);
    const net = view('netprofile').report;
    if (net === null) throw new Error('fixture');
    expect(reportWideConfigLead(net.groups, net.networkProfile)).toEqual(['npm', 'pypi', 'nuget']);
    // The flag with zero configUnreadable rows ⇒ nothing to explain.
    expect(reportWideConfigLead(view('uptodate').report?.groups ?? [], true)).toBeNull();
    // One unreadable ecosystem that also has checked rows still gets it when flagged.
    expect(reportWideConfigLead(view('populated').report?.groups ?? [], true)).toEqual(['cargo']);
  });

  it('the report-wide copy joins 2 and 3 ecosystems', () => {
    expect(joinAnd(['npm', 'NuGet'])).toBe('npm and NuGet');
    expect(joinAnd(['npm', 'Cargo', 'PyPI'])).toBe('npm, Cargo and PyPI');
    expect(reportWideLead(['npm', 'PyPI', 'NuGet'])).toBe(
      "At least one of your user settings folders is on a network share, and Bonsai doesn't read registry settings from network locations. That is likely why it treats npm, PyPI and NuGet packages as private and doesn't send them. This is expected if your organisation keeps your profile on a network share.",
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
