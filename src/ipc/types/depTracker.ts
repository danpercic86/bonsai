// P125 dependency tracker — the exact camelCase mirror of the Rust wire types
// (`docs/contracts/P125-ecosystems.md` E6 ⇐ `P125-dependency-tracker.md`
// §2.1 `bonsai-deps/src/model.rs` + §2.2 `src-tauri/src/deps/mod.rs`).
//
// Hand-written lockstep (P75 codegen is HALTED): keep this file and the Rust
// structs identical. Rust owns all ordering (§5.2) and every counted number
// (`summarize`); the frontend only filters and renders.

export type DepEcosystem = 'npm' | 'cargo' | 'pypi' | 'go' | 'nuget';
export type DepUpdateKind = 'patch' | 'minor' | 'major';
export type DepSeverity = 'critical' | 'high' | 'medium' | 'low' | 'unknown';
export type DepRelation = 'direct' | 'transitive';
export type DepScope = 'normal' | 'dev' | 'build' | 'optional' | 'peer';
export type DepLocalReason = 'path' | 'git' | 'workspace' | 'url';
export type DepPrivateReason =
  | 'customRegistry'
  | 'scopedRegistry'
  | 'goPrivate'
  | 'sourceMapping'
  | 'configUnreadable'
  /** P125h §3: a folder / file / UNC package source; `registryHost` is always null. */
  | 'localFeed';

/** Why a public row is `unknown` (P125h §0). A v1 cached row reads `notFound`. */
export type DepUnknownReason = 'notFound' | 'tooLarge';

/** Why a `checked` / `unknown` row's `vulnsChecked` is false (P125h §0). */
export type DepVulnsUnverifiedReason = 'noVersion' | 'nameCasing' | 'malformedAdvisory' | 'responseTooLarge';

/** `unknown` = deps.dev 404 or an oversize answer; OSV is still queried, so it
 *  MAY carry vulns [UI-e]. `registryHost` is display-only (never a link, never
 *  sent) [UI-d]. */
export type DepAvailability =
  | { kind: 'checked' }
  | { kind: 'unknown'; reason: DepUnknownReason }
  | { kind: 'local'; reason: DepLocalReason }
  | { kind: 'private'; reason: DepPrivateReason; registryHost: string | null };

export interface DepAdvisory {
  id: string;
  aliases: string[];
  /** Plain text — never rendered as markdown/HTML [LOW-3]. */
  summary: string | null;
  severity: DepSeverity;
  cvssScore: number | null;
  modified: string;
  /** `https://osv.dev/vulnerability/{id}`, built by Rust. */
  url: string;
}

export interface DepRow {
  name: string;
  ecosystem: DepEcosystem;
  relation: DepRelation;
  scope: DepScope;
  /** Manifest constraint (null for transitive). */
  declared: string | null;
  /** Resolved version; null ⇒ the UI shows `declared`. */
  current: string | null;
  latest: string | null;
  /** null = up to date or n/a. */
  update: DepUpdateKind | null;
  availability: DepAvailability;
  /** false ⇒ the UI must not claim "no vulnerabilities". */
  vulnsChecked: boolean;
  /** Severity desc, then id asc. */
  vulns: DepAdvisory[];
  /** Non-null only for `checked` | `unknown` rows (§7.6). */
  registryUrl: string | null;
  /** Why `vulnsChecked` is false on a `checked` | `unknown` row; always null on
   *  `local` | `private` rows, and null on v1 cached rows (P125h §0). */
  vulnsUnverified: DepVulnsUnverifiedReason | null;
}

export type DepParseErrorKind =
  | 'tooLarge'
  | 'notAFile'
  | 'unreadable'
  | 'invalidSyntax'
  | 'unsupportedFeature'
  | 'unsupportedVersion'
  | 'limitExceeded'
  | 'internal';

export type DepGroupState =
  | { kind: 'ok' }
  | { kind: 'noLockfile' }
  | {
      kind: 'parseError';
      file: string;
      category: DepParseErrorKind;
      line: number | null;
      col: number | null;
      /** Rendered by Rust from a fixed table (e.g. "invalid syntax at 12:4") [MEDIUM-1]. */
      message: string;
    }
  | { kind: 'notTracked'; label: string };

export interface DepManifestGroup {
  manifestPath: string;
  lockfilePath: string | null;
  /** null ⇔ `notTracked`. */
  ecosystem: DepEcosystem | null;
  state: DepGroupState;
  rows: DepRow[];
  /** Public transitive lock entries queried against OSV [UI-d]. */
  transitiveChecked: number;
}

export interface DepSeverityCounts {
  critical: number;
  high: number;
  medium: number;
  low: number;
  unknown: number;
}

/** Computed ONLY by Rust `summarize`; counts ROWS (groups for the last two). */
export interface DepSummary {
  major: number;
  minor: number;
  patch: number;
  vulnerable: number;
  vulnerableBySeverity: DepSeverityCounts;
  upToDate: number;
  unknown: number;
  local: number;
  private: number;
  notTracked: number;
  parseErrors: number;
}

export interface DepReport {
  repoId: string;
  checkedAtMs: number;
  /** Empty ⇒ "No dependency manifests found". */
  groups: DepManifestGroup[];
  summary: DepSummary;
  /** P125h §4b: a user/system config base folder is on a network share (UNC
   *  text check; a mapped drive letter is not detected). False on v1 reports. */
  networkProfile: boolean;
}

export type DepCheckFailureKind = 'offline' | 'rateLimited' | 'service' | 'timeout' | 'internal';

/** `message` is rendered by Rust from `kind` + `httpStatus` [MEDIUM-1]. */
export interface DepCheckFailure {
  kind: DepCheckFailureKind;
  httpStatus: number | null;
  message: string;
  atMs: number;
}

/** done/total = deps.dev lookups + OSV batches — NOT packages [UI-b]. */
export interface DepRunningCheck {
  checkId: number;
  startedAtMs: number;
  done: number;
  total: number;
}

export interface DepTrackerView {
  repoId: string;
  enabled: boolean;
  /** Last completed report (from the cache), shown while running. */
  report: DepReport | null;
  /** D5 stale. */
  manifestsChanged: boolean;
  running: DepRunningCheck | null;
  /** Latest FAILED attempt; cleared on success. */
  lastError: DepCheckFailure | null;
  nextCheckMs: number | null;
}

export type DepsChangedKind =
  | 'started'
  | 'progress'
  | 'finished'
  | 'failed'
  | 'cancelled'
  | 'stale'
  | 'cleared';

export interface DepsChangedPayload {
  repoId: string;
  checkId: number | null;
  kind: DepsChangedKind;
  done: number | null;
  total: number | null;
}

/** `UiSettings.depTracker`. Enabling it IS the D1 consent. */
export interface DepTrackerSettings {
  enabled: boolean;
  /** Clamped to `DEP_TRACKER_INTERVAL_MIN..=MAX` (1..=168) by Rust. */
  intervalHours: number;
}
