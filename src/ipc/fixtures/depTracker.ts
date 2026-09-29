// P125 E5 — the dependency-tracker `DepTrackerView` fixtures, one per
// `?deps=` harness mode. The group tables live in `depTrackerPopulated.ts` /
// `depTrackerSynthetic.ts`; the builders and mirrors in `depTrackerModel.ts`.
//
// Every summary comes from `summarizeFixture` (the Rust `summarize` rules), so
// a fixture can never show a count its rows do not add up to.
import type {
  DepCheckFailure,
  DepCheckFailureKind,
  DepManifestGroup,
  DepReport,
  DepTrackerView,
} from '../types/depTracker';
import { summarizeFixture } from './depTrackerModel';
import { POPULATED_GROUPS, UPTODATE_GROUPS } from './depTrackerPopulated';
import { LARGE_GROUPS, MIRROR_GROUPS } from './depTrackerSynthetic';

export const DEP_MOCK_MODES = [
  'off',
  'loading',
  'populated',
  'uptodate',
  'stale',
  'offline',
  'error',
  'empty',
  'mirror',
  'large',
] as const;
export type DepMockMode = (typeof DEP_MOCK_MODES)[number];

export function isDepMockMode(v: string | null): v is DepMockMode {
  return v !== null && (DEP_MOCK_MODES as readonly string[]).includes(v);
}

/** Placeholder id; the mock handler re-stamps the caller's `repoId`. */
export const FIXTURE_REPO_ID = 'mock-repo';

const MINUTE = 60_000;
const HOUR = 60 * MINUTE;
/** `DepTracker::interval_hours` default. */
const INTERVAL_MS = 12 * HOUR;

/**
 * ASSUMED — `CheckFailure.message` is rendered by a fixed `kind`/`httpStatus`
 * table in `src-tauri/src/deps` (P125 §2.2 [MEDIUM-1]), which lands in P125e and
 * does not exist yet. These are placeholders in that table's spirit (no free
 * text). **P125e obligation:** either emit exactly these strings or update this
 * table to the real ones in the same increment. The UI never shows `message`
 * as primary copy (P125-ui §6: `title` only, as `Details: {message}`).
 */
export const DEP_FAILURE_MESSAGES: Readonly<Record<DepCheckFailureKind, string>> = {
  offline: 'could not reach the package services',
  rateLimited: 'the package services rate-limited the check',
  service: 'the package services returned HTTP 503',
  timeout: 'the check timed out',
  internal: 'internal error',
};

function failure(kind: DepCheckFailureKind, httpStatus: number | null, atMs: number): DepCheckFailure {
  return { kind, httpStatus, message: DEP_FAILURE_MESSAGES[kind], atMs };
}

function report(repoId: string, checkedAtMs: number, groups: readonly DepManifestGroup[]): DepReport {
  return { repoId, checkedAtMs, groups: [...groups], summary: summarizeFixture(groups) };
}

/** An enabled view with a report and nothing running. */
function withReport(
  repoId: string,
  checkedAtMs: number,
  groups: readonly DepManifestGroup[],
): DepTrackerView {
  return {
    repoId,
    enabled: true,
    report: report(repoId, checkedAtMs, groups),
    manifestsChanged: false,
    running: null,
    lastError: null,
    nextCheckMs: checkedAtMs + INTERVAL_MS,
  };
}

/** Builds the view for one harness mode, with every timestamp relative to
 *  `nowMs` so "checked 3 hours ago" stays true whenever it is served. */
export function buildDepView(mode: DepMockMode, repoId: string, nowMs: number): DepTrackerView {
  const recent = nowMs - 25 * MINUTE;
  switch (mode) {
    case 'off':
      // Disabled: nothing read, nothing cached, nothing scheduled.
      return {
        repoId,
        enabled: false,
        report: null,
        manifestsChanged: false,
        running: null,
        lastError: null,
        nextCheckMs: null,
      };
    case 'loading':
      return {
        repoId,
        enabled: true,
        report: null,
        manifestsChanged: false,
        // No progress yet ⇒ the header reads "Checking…" (P125-ui §4.1).
        running: { checkId: 1, startedAtMs: nowMs - 2_000, done: 0, total: 0 },
        lastError: null,
        nextCheckMs: null,
      };
    case 'populated':
      return withReport(repoId, recent, POPULATED_GROUPS);
    case 'uptodate':
      return withReport(repoId, recent, UPTODATE_GROUPS);
    case 'stale':
      return { ...withReport(repoId, nowMs - 2 * HOUR, POPULATED_GROUPS), manifestsChanged: true };
    case 'offline': {
      const checkedAt = nowMs - 3 * HOUR;
      return {
        ...withReport(repoId, checkedAt, POPULATED_GROUPS),
        lastError: failure('offline', null, nowMs - 5 * MINUTE),
        nextCheckMs: nowMs + INTERVAL_MS - 5 * MINUTE,
      };
    }
    case 'error':
      return {
        repoId,
        enabled: true,
        report: null,
        manifestsChanged: false,
        running: null,
        lastError: failure('service', 503, nowMs - 2 * MINUTE),
        nextCheckMs: nowMs + INTERVAL_MS - 2 * MINUTE,
      };
    case 'empty':
      return withReport(repoId, recent, []);
    case 'mirror':
      return withReport(repoId, recent, MIRROR_GROUPS);
    case 'large':
      return withReport(repoId, recent, LARGE_GROUPS);
  }
}

// One export per mode (E5). Built once at import time against the load clock;
// the mock handler calls `buildDepView` per request so ages stay truthful.
const LOAD_NOW = Date.now();
export const DEP_VIEW_OFF = buildDepView('off', FIXTURE_REPO_ID, LOAD_NOW);
export const DEP_VIEW_LOADING = buildDepView('loading', FIXTURE_REPO_ID, LOAD_NOW);
export const DEP_VIEW_POPULATED = buildDepView('populated', FIXTURE_REPO_ID, LOAD_NOW);
export const DEP_VIEW_UPTODATE = buildDepView('uptodate', FIXTURE_REPO_ID, LOAD_NOW);
export const DEP_VIEW_STALE = buildDepView('stale', FIXTURE_REPO_ID, LOAD_NOW);
export const DEP_VIEW_OFFLINE = buildDepView('offline', FIXTURE_REPO_ID, LOAD_NOW);
export const DEP_VIEW_ERROR = buildDepView('error', FIXTURE_REPO_ID, LOAD_NOW);
export const DEP_VIEW_EMPTY = buildDepView('empty', FIXTURE_REPO_ID, LOAD_NOW);
export const DEP_VIEW_MIRROR = buildDepView('mirror', FIXTURE_REPO_ID, LOAD_NOW);
export const DEP_VIEW_LARGE = buildDepView('large', FIXTURE_REPO_ID, LOAD_NOW);
