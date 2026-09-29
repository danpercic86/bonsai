// P125-ui §2 / §6 — the dependency tracker's PURE view derivations.
//
// `deriveState` applies the architect's §2.3 rules (the shipped hook returns
// only `{view, progress, refresh}` and derives nothing). Everything here is a
// function of the DTO: Rust owns ordering and every counted number, so these
// helpers only pick, filter (never re-order) and word.
import type {
  DepCheckFailure,
  DepEcosystem,
  DepManifestGroup,
  DepPrivateReason,
  DepRow,
  DepSummary,
  DepTrackerView,
  DepUpdateKind,
  DepVulnsUnverifiedReason,
} from '../../ipc/types/depTracker';
import {
  ALL_PRIVATE_LEAD,
  AVAIL_LOCAL,
  AVAIL_NOT_CHECKED,
  AVAIL_PRIVATE,
  AVAIL_UNKNOWN,
  ECOSYSTEM_LABEL,
  HOST_LABEL,
  INCOMPLETE_MALFORMED,
  INCOMPLETE_TOO_LARGE,
  LOCAL_REASON,
  NOT_VERIFIED_MALFORMED,
  NOT_VERIFIED_NO_VERSION,
  NOT_VERIFIED_NUGET,
  NOT_VERIFIED_OTHER,
  NOT_VERIFIED_TOO_LARGE,
  PRIVATE_REASON_NO_HOST,
  PRIVATE_REASON_WITH_HOST,
  PRIVATE_TITLE_CONFIG,
  PRIVATE_TITLE_CUSTOM,
  PRIVATE_TITLE_LOCAL_FEED,
  PRIVATE_TITLE_OTHER,
  SCOPE_TAG,
  STALE_NOTICE,
  TAG_TRANSITIVE,
  TOOLBAR_NAME,
  TOOLBAR_RUNNING_LABEL_SUFFIX,
  TOOLBAR_RUNNING_TITLE_SUFFIX,
  TOOLBAR_TITLE_CHECKING,
  TOOLBAR_TITLE_OFF,
  UNKNOWN_REASON,
  UNKNOWN_TITLE,
  allPrivateHostLead,
  configUnreadableLead,
  failureNotice,
  secondary,
  toolbarLabel,
  toolbarTitle,
} from './depsCopy';
import { formatAge } from '../../lib/formatAge';

// ----- §6 state ---------------------------------------------------------------

export type DepsBody = 'off' | 'loading' | 'failed' | 'empty' | 'list';

export type DepsNoticeState =
  | { kind: 'stale' }
  | { kind: 'failure'; failure: DepCheckFailure; checkedAtMs: number }
  | null;

export interface DepsDerivedState {
  body: DepsBody;
  /** The one state notice (§6) — the link note is separate. */
  notice: DepsNoticeState;
  /** The header `stale` pill: `manifestsChanged || lastError`, with a report. */
  stalePill: boolean;
  /** Report present and nothing to update or fix: the calm line shows. */
  upToDate: boolean;
  running: boolean;
}

/** §6 table. A `null` view (the first fetch in flight) reads as loading. */
export function deriveState(view: DepTrackerView | null): DepsDerivedState {
  if (view === null) {
    return { body: 'loading', notice: null, stalePill: false, upToDate: false, running: false };
  }
  const running = view.running !== null;
  if (!view.enabled) {
    return { body: 'off', notice: null, stalePill: false, upToDate: false, running: false };
  }
  const { report, lastError } = view;
  if (report === null) {
    return {
      body: lastError === null ? 'loading' : 'failed',
      notice: null,
      stalePill: false,
      upToDate: false,
      running,
    };
  }
  const notice: DepsNoticeState =
    lastError !== null
      ? { kind: 'failure', failure: lastError, checkedAtMs: report.checkedAtMs }
      : view.manifestsChanged
        ? { kind: 'stale' }
        : null;
  const s = report.summary;
  const empty = report.groups.length === 0;
  return {
    body: empty ? 'empty' : 'list',
    notice,
    stalePill: view.manifestsChanged || lastError !== null,
    upToDate: !empty && s.major + s.minor + s.patch + s.vulnerable === 0,
    running,
  };
}

/** `major + minor + patch` — the `U` of §3. */
export function updateCount(s: DepSummary): number {
  return s.major + s.minor + s.patch;
}

/** §8.3: append "Some weren't checked" when anything went unchecked. */
export function someUnchecked(groups: readonly DepManifestGroup[], s: DepSummary): boolean {
  if (s.private + s.local + s.unknown + s.notTracked + s.parseErrors > 0) return true;
  return groups.some(
    (g) => g.state.kind === 'noLockfile' || g.rows.some((r) => !r.vulnsChecked),
  );
}

/** §4.2 secondary line: the non-zero counts, joined by ` · ` (empty ⇒ omit). */
export function secondaryLine(s: DepSummary): string {
  const parts: string[] = [];
  if (s.upToDate > 0) parts.push(secondary.upToDate(s.upToDate));
  if (s.private > 0) parts.push(secondary.private(s.private));
  if (s.local > 0) parts.push(secondary.local(s.local));
  if (s.unknown > 0) parts.push(secondary.unknown(s.unknown));
  if (s.notTracked > 0) parts.push(secondary.notTracked(s.notTracked));
  if (s.parseErrors > 0) parts.push(secondary.parseErrors(s.parseErrors));
  return parts.join(' · ');
}

// ----- §4.2 filters -------------------------------------------------------------

export interface DepsFilter {
  vulnerable: boolean;
  kinds: ReadonlySet<DepUpdateKind>;
  name: string;
}

export const NO_FILTER: DepsFilter = { vulnerable: false, kinds: new Set(), name: '' };

export function isFilterActive(f: DepsFilter): boolean {
  return f.vulnerable || f.kinds.size > 0 || f.name.trim() !== '';
}

/** Order-preserving (§5.2 of the architect contract: never re-sorted). */
export function filterRows(rows: readonly DepRow[], f: DepsFilter): DepRow[] {
  const needle = f.name.trim().toLowerCase();
  const anyToggle = f.vulnerable || f.kinds.size > 0;
  return rows.filter((r) => {
    if (needle !== '' && !r.name.toLowerCase().includes(needle)) return false;
    if (anyToggle && r.update === null && r.vulns.length === 0) return false;
    if (f.vulnerable && r.vulns.length === 0) return false;
    if (f.kinds.size > 0 && (r.update === null || !f.kinds.has(r.update))) return false;
    return true;
  });
}

export interface VisibleGroup {
  group: DepManifestGroup;
  rows: DepRow[];
}

/** Groups with ≥ 1 matching row, plus every `notTracked` / `parseError` group
 *  (always shown, so a filtered report never looks complete when it isn't). */
export function visibleGroups(groups: readonly DepManifestGroup[], f: DepsFilter): VisibleGroup[] {
  const active = isFilterActive(f);
  const out: VisibleGroup[] = [];
  for (const group of groups) {
    const k = group.state.kind;
    if (k === 'notTracked' || k === 'parseError') {
      out.push({ group, rows: [] });
      continue;
    }
    const rows = filterRows(group.rows, f);
    if (active && rows.length === 0) continue;
    out.push({ group, rows });
  }
  return out;
}

/** §4.4 React key, scoped inside the group keyed by `manifestPath`. */
export function rowKey(row: DepRow): string {
  return `${row.relation}\u0000${row.name}\u0000${row.current ?? row.declared ?? ''}`;
}

// ----- §4.4 / §8.4 row words ------------------------------------------------------

export function ecosystemLabel(e: DepEcosystem): string {
  return ECOSYSTEM_LABEL[e];
}

export function hostLabel(e: DepEcosystem): string {
  return HOST_LABEL[e];
}

/** The row tags, at most two (`transitive` and a non-normal scope never
 *  combine in Rust output; the scope tag is shown only on direct rows). */
export function rowTags(row: DepRow): string[] {
  if (row.relation === 'transitive') return [TAG_TRANSITIVE];
  return row.scope === 'normal' ? [] : [SCOPE_TAG[row.scope]];
}

/** Private reason text: `host` set ⇒ render `{prefix} ` + mono host. */
export function privateReasonText(
  reason: DepPrivateReason,
  registryHost: string | null,
): { prefix: string; host: string | null } {
  const withHost = PRIVATE_REASON_WITH_HOST[reason];
  if (withHost !== undefined && registryHost !== null) return { prefix: withHost, host: registryHost };
  return { prefix: PRIVATE_REASON_NO_HOST[reason], host: null };
}

export function privateReasonTitle(reason: DepPrivateReason): string {
  if (reason === 'customRegistry') return PRIVATE_TITLE_CUSTOM;
  if (reason === 'configUnreadable') return PRIVATE_TITLE_CONFIG;
  if (reason === 'localFeed') return PRIVATE_TITLE_LOCAL_FEED;
  return PRIVATE_TITLE_OTHER;
}

export interface AvailabilityWords {
  pill: string;
  /** Visible 12px reason; `null` ⇒ none. */
  reason: { prefix: string; host: string | null } | null;
  title: string | null;
}

/** `null` for `checked` rows (they have no availability cell). */
export function availabilityWords(row: DepRow, reportWide: boolean): AvailabilityWords | null {
  const a = row.availability;
  switch (a.kind) {
    case 'checked':
      return null;
    case 'unknown':
      return {
        pill: AVAIL_UNKNOWN,
        reason: { prefix: UNKNOWN_REASON[a.reason], host: null },
        title: UNKNOWN_TITLE[a.reason],
      };
    case 'local':
      return { pill: AVAIL_LOCAL, reason: { prefix: LOCAL_REASON[a.reason], host: null }, title: null };
    case 'private':
      // §8.3: under the report-wide lead a config-unreadable row is shortened
      // to `not checked` with no reason text — no per-row noise.
      if (reportWide && a.reason === 'configUnreadable') {
        return { pill: AVAIL_NOT_CHECKED, reason: null, title: null };
      }
      return {
        pill: AVAIL_PRIVATE,
        reason: privateReasonText(a.reason, a.registryHost),
        title: privateReasonTitle(a.reason),
      };
  }
}

const NOT_VERIFIED_TITLE: Readonly<Record<DepVulnsUnverifiedReason, string>> = {
  noVersion: NOT_VERIFIED_NO_VERSION,
  nameCasing: NOT_VERIFIED_NUGET,
  malformedAdvisory: NOT_VERIFIED_MALFORMED,
  responseTooLarge: NOT_VERIFIED_TOO_LARGE,
};

/** §8.3 not-verified `title`: a lookup on the wire field (P125h §4c); `null`
 *  (a v1 cached row) reads the generic text. */
export function notVerifiedCause(row: DepRow): string {
  return row.vulnsUnverified === null ? NOT_VERIFIED_OTHER : NOT_VERIFIED_TITLE[row.vulnsUnverified];
}

/** §4.4 / §8.3 incomplete-list note under a row's advisories, or null. Only
 *  the two reasons that can coexist with advisories have one. */
export function incompleteListNote(row: DepRow): string | null {
  if (row.vulnsUnverified === 'malformedAdvisory') return INCOMPLETE_MALFORMED;
  if (row.vulnsUnverified === 'responseTooLarge') return INCOMPLETE_TOO_LARGE;
  return null;
}

// ----- §4.3 / §8.3 leads -------------------------------------------------------------

/**
 * §8.3 report-wide network-profile lead (P125h, ruling R3): the ecosystems
 * with `configUnreadable` rows, in report order, iff `networkProfile` is set
 * AND at least one such row exists — else null. Flag only: no inference from
 * how many ecosystems are affected (a v1 report reads `networkProfile: false`,
 * so the per-group leads apply).
 */
export function reportWideConfigLead(
  groups: readonly DepManifestGroup[],
  networkProfile: boolean,
): DepEcosystem[] | null {
  if (!networkProfile) return null;
  const unreadable = new Set<DepEcosystem>();
  for (const g of groups) {
    for (const r of g.rows) {
      if (r.availability.kind === 'private' && r.availability.reason === 'configUnreadable') {
        unreadable.add(r.ecosystem);
      }
    }
  }
  return unreadable.size === 0 ? null : [...unreadable];
}

/** §4.3 all-private/local group lead, or null. Needs ≥ 1 private row: an
 *  all-local group comes from no registry, so the lead would be untrue. */
export function groupPrivateLead(group: DepManifestGroup, reportWide: boolean): string | null {
  const rows = group.rows;
  if (rows.length === 0) return null;
  const allOffline = rows.every(
    (r) => r.availability.kind === 'private' || r.availability.kind === 'local',
  );
  if (!allOffline) return null;
  const privates = rows.flatMap((r) => (r.availability.kind === 'private' ? [r.availability] : []));
  if (privates.length === 0) return null;
  if (privates.every((p) => p.reason === 'configUnreadable')) {
    // No ecosystem ⇒ no honest sentence to say (never "the  registry settings").
    if (reportWide || group.ecosystem === null) return null;
    return configUnreadableLead(ecosystemLabel(group.ecosystem));
  }
  const host = privates[0].registryHost;
  if (host !== null && privates.every((p) => p.registryHost === host)) return allPrivateHostLead(host);
  return ALL_PRIVATE_LEAD;
}

/** Group header `title`: `manifestPath · ecosystem · lockfilePath`. */
export function groupTitle(group: DepManifestGroup): string {
  const parts = [group.manifestPath];
  if (group.ecosystem !== null) parts.push(ecosystemLabel(group.ecosystem));
  if (group.lockfilePath !== null) parts.push(group.lockfilePath);
  return parts.join(' · ');
}

/** Rows in the group that carry at least one advisory. */
export function vulnerableRows(group: DepManifestGroup): number {
  return group.rows.reduce((n, r) => n + (r.vulns.length > 0 ? 1 : 0), 0);
}

/** §6 / §8.3: the state notice's text (`{age}` from `report.checkedAtMs`). */
export function noticeText(notice: NonNullable<DepsNoticeState>, nowMs: number): string {
  return notice.kind === 'stale'
    ? STALE_NOTICE
    : failureNotice(notice.failure.kind, formatAge(notice.checkedAtMs, nowMs));
}

/** First sentence of a notice, for the live region (§4.1 failure). */
export function firstSentence(text: string): string {
  const i = text.indexOf('. ');
  return i === -1 ? text : text.slice(0, i + 1);
}

/** Per-group row cap before "Show all" (§4.3). */
export const GROUP_ROW_CAP = 200;

// ----- §3 toolbar -------------------------------------------------------------------

export interface ToolbarNaming {
  label: string;
  title: string;
  /** Vulnerable count for the chip; `null` ⇒ no chip. */
  chip: number | null;
  /** Re-check cue: a check runs AND a chip is shown (§3). */
  busy: boolean;
}

/** §3 table. A `null` view (first fetch in flight) and a no-report failure
 *  have no row in the table: both get the bare name, no invented copy. */
export function toolbarNaming(view: DepTrackerView | null, nowMs: number): ToolbarNaming {
  const bare: ToolbarNaming = { label: TOOLBAR_NAME, title: TOOLBAR_NAME, chip: null, busy: false };
  if (view === null) return bare;
  if (!view.enabled) return { ...bare, title: TOOLBAR_TITLE_OFF };
  const running = view.running !== null;
  const report = view.report;
  if (report === null) {
    return running || view.lastError === null ? { ...bare, title: TOOLBAR_TITLE_CHECKING } : bare;
  }
  const n = report.summary.vulnerable;
  const u = updateCount(report.summary);
  let label = toolbarLabel(n);
  let title = toolbarTitle(n, u, formatAge(report.checkedAtMs, nowMs));
  if (running) {
    label += TOOLBAR_RUNNING_LABEL_SUFFIX;
    title += TOOLBAR_RUNNING_TITLE_SUFFIX;
  }
  return { label, title, chip: n > 0 ? n : null, busy: running && n > 0 };
}

/** Chip text: capped at `99+`. */
export function chipCount(n: number): string {
  return n > 99 ? '99+' : String(n);
}
