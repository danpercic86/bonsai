// P125 E5 — the pure helpers the dependency-tracker fixtures are BUILT with:
// row/advisory builders, the §5.2 ordering mirror and `summarizeFixture`.
//
// In production Rust owns ordering (`check/rows.rs::sort_rows` / `sort_groups`)
// and counting (`model/summary.rs::summarize`); the frontend never re-sorts or
// re-counts. These mirrors exist ONLY so hand-written fixtures come out in the
// order, and with the summary, a real `DepReport` would have — the fixture test
// then re-checks both with an independent recount.
import type {
  DepAdvisory,
  DepAvailability,
  DepEcosystem,
  DepGroupState,
  DepManifestGroup,
  DepParseErrorKind,
  DepRow,
  DepSeverity,
  DepSummary,
  DepUpdateKind,
} from '../types/depTracker';

const SEVERITY_RANK: Record<DepSeverity, number> = {
  critical: 4,
  high: 3,
  medium: 2,
  low: 1,
  unknown: 0,
};

const UPDATE_RANK: Record<DepUpdateKind, number> = { major: 3, minor: 2, patch: 1 };

/** Mirrors `DepRow::max_severity`: the highest-ranked advisory, or null. */
export function maxSeverity(row: DepRow): DepSeverity | null {
  let best: DepSeverity | null = null;
  for (const a of row.vulns) {
    if (best === null || SEVERITY_RANK[a.severity] > SEVERITY_RANK[best]) best = a.severity;
  }
  return best;
}

/** §5.2 rows: max severity desc (none last), update Major > Minor > Patch > none,
 *  Direct before Transitive, then name asc (byte order, like Rust `str::cmp`). */
export function compareRows(a: DepRow, b: DepRow): number {
  const sev = (r: DepRow) => {
    const s = maxSeverity(r);
    return s === null ? -1 : SEVERITY_RANK[s];
  };
  const upd = (r: DepRow) => (r.update === null ? 0 : UPDATE_RANK[r.update]);
  const tr = (r: DepRow) => (r.relation === 'transitive' ? 1 : 0);
  return (
    sev(b) - sev(a) ||
    upd(b) - upd(a) ||
    tr(a) - tr(b) ||
    (a.name < b.name ? -1 : a.name > b.name ? 1 : 0)
  );
}

/** §5.2 groups: root-level first, then `manifestPath` ascending. */
export function compareGroups(a: DepManifestGroup, b: DepManifestGroup): number {
  const nested = (g: DepManifestGroup) => (g.manifestPath.includes('/') ? 1 : 0);
  return (
    nested(a) - nested(b) ||
    (a.manifestPath < b.manifestPath ? -1 : a.manifestPath > b.manifestPath ? 1 : 0)
  );
}

/** Mirrors Rust `summarize` exactly (see `model/summary.rs`). */
export function summarizeFixture(groups: readonly DepManifestGroup[]): DepSummary {
  const s: DepSummary = {
    major: 0,
    minor: 0,
    patch: 0,
    vulnerable: 0,
    vulnerableBySeverity: { critical: 0, high: 0, medium: 0, low: 0, unknown: 0 },
    upToDate: 0,
    unknown: 0,
    local: 0,
    private: 0,
    notTracked: 0,
    parseErrors: 0,
  };
  for (const g of groups) {
    if (g.state.kind === 'notTracked') s.notTracked += 1;
    if (g.state.kind === 'parseError') s.parseErrors += 1;
    for (const row of g.rows) {
      if (row.update !== null) s[row.update] += 1;
      const sev = maxSeverity(row);
      if (sev !== null) {
        s.vulnerable += 1;
        s.vulnerableBySeverity[sev] += 1;
      }
      switch (row.availability.kind) {
        case 'checked':
          if (row.relation === 'direct' && row.update === null) s.upToDate += 1;
          break;
        case 'unknown':
          s.unknown += 1;
          break;
        case 'local':
          s.local += 1;
          break;
        case 'private':
          s.private += 1;
          break;
      }
    }
  }
  return s;
}

/** Mirrors `check/rows.rs::registry_url`'s encoder: ASCII alphanumerics and
 *  `- . _ ~ / @` pass through, every other byte is `%XX` (upper-case hex). */
function encodeName(name: string): string {
  let out = '';
  for (const b of new TextEncoder().encode(name)) {
    const c = String.fromCharCode(b);
    out += /[A-Za-z0-9\-._~/@]/.test(c) ? c : `%${b.toString(16).toUpperCase().padStart(2, '0')}`;
  }
  return out;
}

/** §7.6 registry-page templates (`catalog.rs::registry_page_template`). */
export function registryUrlOf(ecosystem: DepEcosystem, name: string): string {
  const n = encodeName(name);
  switch (ecosystem) {
    case 'npm':
      return `https://www.npmjs.com/package/${n}`;
    case 'cargo':
      return `https://crates.io/crates/${n}`;
    case 'pypi':
      return `https://pypi.org/project/${n}/`;
    case 'go':
      return `https://pkg.go.dev/${n}`;
    case 'nuget':
      return `https://www.nuget.org/packages/${n}`;
  }
}

/** An advisory with the Rust-built OSV URL. */
export function advisory(
  id: string,
  severity: DepSeverity,
  summary: string | null,
  cvssScore: number | null,
  aliases: string[] = [],
): DepAdvisory {
  return {
    id,
    aliases,
    summary,
    severity,
    cvssScore,
    modified: '2025-06-12T08:31:44Z',
    url: `https://osv.dev/vulnerability/${id}`,
  };
}

/** Rust keeps `vulns` severity desc, then id asc. */
function sortVulns(vulns: DepAdvisory[]): DepAdvisory[] {
  return [...vulns].sort(
    (a, b) =>
      SEVERITY_RANK[b.severity] - SEVERITY_RANK[a.severity] ||
      (a.id < b.id ? -1 : a.id > b.id ? 1 : 0),
  );
}

export interface RowSpec {
  name: string;
  ecosystem: DepEcosystem;
  relation?: DepRow['relation'];
  scope?: DepRow['scope'];
  declared?: string | null;
  current?: string | null;
  latest?: string | null;
  update?: DepUpdateKind | null;
  availability?: DepAvailability;
  vulnsChecked?: boolean;
  vulns?: DepAdvisory[];
}

/**
 * One row, with the Rust invariants applied: transitive rows carry no
 * `declared`/`latest`/`update`; `registryUrl` exists only for checked | unknown
 * rows; local/private rows are never vuln-checked.
 */
export function row(spec: RowSpec): DepRow {
  const availability = spec.availability ?? { kind: 'checked' };
  const relation = spec.relation ?? 'direct';
  const linkable = availability.kind === 'checked' || availability.kind === 'unknown';
  const transitive = relation === 'transitive';
  return {
    name: spec.name,
    ecosystem: spec.ecosystem,
    relation,
    scope: spec.scope ?? 'normal',
    declared: transitive ? null : (spec.declared ?? null),
    current: spec.current ?? null,
    latest: transitive ? null : (spec.latest ?? null),
    update: transitive ? null : (spec.update ?? null),
    availability,
    vulnsChecked: linkable ? (spec.vulnsChecked ?? true) : false,
    vulns: sortVulns(spec.vulns ?? []),
    registryUrl: linkable ? registryUrlOf(spec.ecosystem, spec.name) : null,
  };
}

/** A tracked group, rows sorted per §5.2. */
export function group(
  manifestPath: string,
  lockfilePath: string | null,
  ecosystem: DepEcosystem,
  rows: DepRow[],
  transitiveChecked: number,
  state: DepGroupState = { kind: 'ok' },
): DepManifestGroup {
  return {
    manifestPath,
    lockfilePath,
    ecosystem,
    state,
    rows: [...rows].sort(compareRows),
    transitiveChecked,
  };
}

/** A `notTracked` group (`catalog.rs::NOT_TRACKED` label). */
export function notTrackedGroup(manifestPath: string, label: string): DepManifestGroup {
  return {
    manifestPath,
    lockfilePath: null,
    ecosystem: null,
    state: { kind: 'notTracked', label },
    rows: [],
    transitiveChecked: 0,
  };
}

const PARSE_WHAT: Record<DepParseErrorKind, string> = {
  tooLarge: 'file too large',
  notAFile: 'not a regular file',
  unreadable: 'file unreadable',
  invalidSyntax: 'invalid syntax',
  unsupportedFeature: 'unsupported feature',
  unsupportedVersion: 'unsupported lockfile version',
  limitExceeded: 'limit exceeded',
  internal: 'internal parser error',
};

/** Verbatim mirror of `check/rows.rs::parse_message` (the fixed [MEDIUM-1] table). */
export function parseMessage(kind: DepParseErrorKind, line: number | null, col: number | null): string {
  const what = PARSE_WHAT[kind];
  if (line !== null && col !== null) return `${what} at ${line}:${col}`;
  if (line !== null) return `${what} at line ${line}`;
  return what;
}

/** Sorts groups per §5.2 (fixtures are authored in any order). */
export function sortGroups(groups: DepManifestGroup[]): DepManifestGroup[] {
  return [...groups].sort(compareGroups);
}
