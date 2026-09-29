// P125-ui §8 — every dependency-tracker string, verbatim. Components and tests
// both import from here (AC5), so a copy change is one edit in one file.
//
// Markdown line wraps in the contract are joined with single spaces. Straight
// apostrophes, literal em dashes (—) and ellipses (…) as written there.
import type {
  DepCheckFailureKind,
  DepEcosystem,
  DepLocalReason,
  DepPrivateReason,
  DepScope,
  DepSeverity,
  DepUnknownReason,
  DepUpdateKind,
} from '../../ipc/types/depTracker';

function plural(n: number, one: string, many: string): string {
  return n === 1 ? one : many;
}

// ----- §3 toolbar -----------------------------------------------------------
export const TOOLBAR_NAME = 'Dependencies';
export const TOOLBAR_TITLE_OFF = 'Dependencies — off';
export const TOOLBAR_TITLE_CHECKING = 'Dependencies — checking…';
export const TOOLBAR_RUNNING_LABEL_SUFFIX = ', checking again';
export const TOOLBAR_RUNNING_TITLE_SUFFIX = ' — checking again…';

export function toolbarLabel(vulnerable: number): string {
  return vulnerable > 0 ? `Dependencies, ${vulnerable} vulnerable` : TOOLBAR_NAME;
}

export function updatesPhrase(updates: number): string {
  return `${updates} ${plural(updates, 'update', 'updates')}`;
}

export function toolbarTitle(vulnerable: number, updates: number, age: string): string {
  if (vulnerable > 0) {
    return `Dependencies — ${vulnerable} vulnerable, ${updatesPhrase(updates)} · checked ${age}`;
  }
  const u = updates === 0 ? 'No updates' : updatesPhrase(updates);
  return `Dependencies — ${u} · checked ${age}`;
}

// ----- §4.1 header ----------------------------------------------------------
export const DIALOG_TITLE = 'Dependencies';
export const CHECK_NOW = 'Check now';
export const CHECKING = 'Checking…';
export const CLOSE = 'Close';
export const STALE_PILL = 'stale';
export const STALE_PILL_TITLE = 'These results may be out of date.';

export function checkedCaption(age: string): string {
  return `checked ${age}`;
}
export function checkedTitle(localized: string): string {
  return `Checked ${localized}`;
}
export function progressCaption(done: number, total: number): string {
  return `Checking… ${done} of ${total}`;
}

export const LIVE_START = 'Checking dependencies…';
export function liveFinished(vulnerable: number, updates: number): string {
  return `Dependency check finished: ${vulnerable} vulnerable, ${updatesPhrase(updates)}.`;
}

// ----- §4.2 summary / filter bar ---------------------------------------------
export const FILTER_VULNERABLE = 'Vulnerable';
export const FILTER_KIND: Readonly<Record<DepUpdateKind, string>> = {
  major: 'Major',
  minor: 'Minor',
  patch: 'Patch',
};
export const FILTER_ARIA = 'Filter packages by name';
export const FILTER_PLACEHOLDER = 'Filter packages…';
export const NO_MATCH = 'No packages match these filters.';
export const CLEAR_FILTERS = 'Clear filters';

export function showAll(n: number): string {
  return `Show all ${n.toLocaleString('en-US')} packages`;
}

/** §8.4 secondary-line items. */
export const secondary = {
  upToDate: (n: number) => `${n} up to date`,
  private: (n: number) => `${n} private — not checked`,
  local: (n: number) => `${n} local`,
  unknown: (n: number) => `${n} unknown`,
  notTracked: (n: number) => `${n} not tracked`,
  parseErrors: (n: number) => `${n} couldn't be read`,
};

// ----- §4.3 groups / columns --------------------------------------------------
export const COLUMNS = {
  package: 'Package',
  installed: 'Installed',
  latest: 'Latest',
  update: 'Update',
  vulnerability: 'Vulnerability',
  actions: 'Actions',
};

export function groupCount(n: number, vulnerable: number): string {
  const base = `${n} ${plural(n, 'package', 'packages')}`;
  return vulnerable > 0 ? `${base} · ${vulnerable} vulnerable` : base;
}

// ----- §4.4 / §4.5 rows and pills -------------------------------------------
export const TAG_TRANSITIVE = 'transitive';
export const SCOPE_TAG: Readonly<Record<Exclude<DepScope, 'normal'>, string>> = {
  dev: 'dev',
  build: 'build',
  optional: 'optional',
  peer: 'peer',
};
export const SEVERITY_LABEL: Readonly<Record<DepSeverity, string>> = {
  critical: 'critical',
  high: 'high',
  medium: 'medium',
  low: 'low',
  unknown: 'unrated',
};
export const UPDATE_LABEL: Readonly<Record<DepUpdateKind, string>> = {
  major: 'major',
  minor: 'minor',
  patch: 'patch',
};
export const AVAIL_LOCAL = 'local';
export const AVAIL_PRIVATE = 'private — not checked';
export const AVAIL_UNKNOWN = 'unknown';
export const AVAIL_NOT_CHECKED = 'not checked';
export const NOT_VERIFIED = 'not verified';

export function morePill(n: number): string {
  return `+${n}`;
}
export function cvss(score: number): string {
  return `CVSS ${score}`;
}
export const NO_SUMMARY = 'No summary provided.';
export const OPEN_ADVISORY = 'Open advisory';
export function openAdvisory(id: string): string {
  return `Open advisory ${id}`;
}
export function openOnHost(name: string, host: string): string {
  return `Open ${name} on ${host}`;
}
/** Row toggle accessible name (§5.2). */
export function rowToggleName(name: string, severity: string, id: string, more: number): string {
  const base = `${name}, ${severity} vulnerability ${id}`;
  return more > 0 ? `${base}, and ${more} more` : base;
}

// ----- §5.3 context menu -----------------------------------------------------
export function menuOpenOn(host: string): string {
  return `Open on ${host}`;
}
export function menuShowAllAdvisories(n: number): string {
  return `Show all ${n} advisories`;
}
export const MENU_COPY_NAME = 'Copy name';
export const MENU_COPY_NAME_VERSION = 'Copy name and version';

// ----- §5.4 palette ----------------------------------------------------------
export const PALETTE_OPEN = 'Dependencies';
export const PALETTE_CHECK = 'Check dependencies now';

// ----- §8.1 consent ----------------------------------------------------------
export const OFF_HEADING = 'Dependency checks are off';
export const CONSENT_P1 =
  'Bonsai can look up the packages this repository depends on, and tell you which have newer versions and which have known security advisories. The setting applies to every repository you open.';
/** Bold lead + remainder, rendered as `<strong>{lead}</strong> {rest}`. */
export const CONSENT_FACTS: readonly { lead: string; rest: string }[] = [
  {
    lead: 'What is sent:',
    rest: "the name, ecosystem and installed version of each public package listed in the repository's manifests and lockfiles.",
  },
  {
    lead: 'Where it goes:',
    rest: "api.deps.dev, for the latest versions, and api.osv.dev, for known vulnerabilities. Both are public, free services for open-source package data. Like any website, they can see your computer's internet address.",
  },
  {
    lead: 'Never sent:',
    rest: "packages that come from a folder, a Git URL, this workspace, or a private or company registry; the repository's name, location or remote addresses; the contents of any file; and any password, token or key.",
  },
  {
    lead: 'Go modules:',
    rest: "a private module that isn't listed in GOPRIVATE looks public to Bonsai and is sent, the same way the go command itself would send it to proxy.golang.org. Add your private module paths to GOPRIVATE to keep them out.",
  },
  {
    lead: 'npm organisation packages:',
    rest: 'a package your organisation keeps private on the public npm registry (an @org package on npmjs.org) looks the same as a public one, so its name is sent.',
  },
];
export function consentSchedule(intervalHours: number): string {
  const every = intervalHours === 1 ? 'every hour' : `every ${intervalHours} hours`;
  return `Bonsai checks when you open a repository, ${every}, and whenever you choose Check now. It only reads files — it never runs npm, cargo, pip or any other tool from the repository. Turning this off stops all checks and deletes saved results.`;
}
export const TURN_ON = 'Turn on dependency checks';
export const OPEN_SETTINGS = 'Open Settings';
export const CONSENT_TITLE = 'Turn on dependency checks?';
export const CONSENT_CONFIRM = 'Turn on';

// ----- §7 / §8.2 settings ----------------------------------------------------
export const SETTINGS_GROUP_TITLE = 'Dependency checks';
export const SETTINGS_LEAD =
  'Checks send public package names and versions to deps.dev and OSV.dev. Private and local packages are never sent.';
export const SETTINGS_OFF_NOTE = 'Dependency checks are off. Saved results were deleted.';

// ----- §8.3 state and notice strings ------------------------------------------
export const UP_TO_DATE = 'All checked packages are up to date, with no known vulnerabilities.';
export const UP_TO_DATE_SOME_UNCHECKED = "Some weren't checked — see below.";
export const STALE_NOTICE = 'Dependency files changed since this check. Check now to update the results.';

export function failureNotice(kind: DepCheckFailureKind, age: string): string {
  switch (kind) {
    case 'offline':
      return `Couldn't reach the package services — showing results from ${age}. Bonsai tries again at the next check, or you can check now.`;
    case 'rateLimited':
      return `The package services are busy — showing results from ${age}. Bonsai tries again at the next check, or you can check now.`;
    case 'timeout':
      return `The check took too long and was stopped — showing results from ${age}. Bonsai tries again at the next check, or you can check now.`;
    case 'service':
      return `The package services sent a response Bonsai couldn't use — showing results from ${age}. Try again later.`;
    case 'internal':
      return `Something went wrong while checking — showing results from ${age}. Try again; if it keeps happening, record a session with Dev mode on and export it.`;
  }
}
export function failureDetails(message: string): string {
  return `Details: ${message}`;
}

export const TRY_AGAIN = 'Try again';
export function failureBody(kind: DepCheckFailureKind): { title: string; reason: string } {
  switch (kind) {
    case 'offline':
    case 'timeout':
      return {
        title: "Couldn't reach the package services",
        reason: 'Check your internet connection, then try again.',
      };
    case 'rateLimited':
      return { title: 'The package services are busy', reason: 'Wait a few minutes, then try again.' };
    case 'service':
      return {
        title: "Couldn't check dependencies",
        reason: "deps.dev or OSV.dev sent a response Bonsai couldn't use. Try again later.",
      };
    case 'internal':
      return {
        title: "Couldn't check dependencies",
        reason:
          'Something went wrong inside Bonsai. Try again; if it keeps happening, record a session with Dev mode on and export it.',
      };
  }
}

export const EMPTY_TITLE = 'No dependency manifests found';
export const EMPTY_REASON =
  'Bonsai looks for package.json, Cargo.toml, pyproject.toml, requirements.txt, go.mod and .csproj files, and none are in this repository.';

export const NO_LOCKFILE_LINE =
  "No lockfile, so these are the declared version ranges, not installed versions. Vulnerabilities can't be checked without exact versions.";
export function parseErrorLine(file: string): string {
  return `${file} — Couldn't read this file. It may be malformed; fix it and check again.`;
}
export function notTrackedSuffix(label: string): string {
  return ` — ${label} isn't tracked yet`;
}
export const ALL_PRIVATE_LEAD =
  "Every package here comes from a private or company registry, so Bonsai doesn't send it anywhere. This is expected for repositories that use a company registry or mirror.";
export function allPrivateHostLead(host: string): string {
  return `Every package here comes from ${host}, so Bonsai doesn't send it anywhere. This is expected for repositories that use a company registry or mirror.`;
}
export function configUnreadableLead(ecosystem: string): string {
  return `Bonsai couldn't read or safely follow the ${ecosystem} registry settings, so it treats these packages as private and doesn't send them.`;
}
/** `A and B` / `A, B and C` (§8.3 `{ecosystems}`). */
export function joinAnd(items: readonly string[]): string {
  if (items.length <= 1) return items.join('');
  return `${items.slice(0, -1).join(', ')} and ${items[items.length - 1]}`;
}

/** §8.3 report-wide network-profile lead (rewritten for P125h, ruling R3):
 *  shown only from `report.networkProfile`, never inferred. `ecosystems` are
 *  the §8.4 labels of the `configUnreadable` ecosystems, in report order. */
export function reportWideLead(ecosystems: readonly string[]): string {
  return `At least one of your user settings folders is on a network share, and Bonsai doesn't read registry settings from network locations. That is likely why it treats ${joinAnd(ecosystems)} packages as private and doesn't send them. This is expected if your organisation keeps your profile on a network share.`;
}

export const NOT_VERIFIED_NO_VERSION = "Not verified — the exact installed version isn't known.";
export const NOT_VERIFIED_NUGET =
  "Not verified — OSV.dev matches NuGet names by exact capitalisation, and without a lockfile Bonsai can't confirm it. Add a packages.lock.json to check this package.";
export const NOT_VERIFIED_MALFORMED =
  "Not verified — OSV.dev sent advisories for this package that Bonsai couldn't read, so it can't confirm the package is clean.";
export const NOT_VERIFIED_TOO_LARGE =
  "Not verified — an advisory for this package was too large for Bonsai to read, so it can't confirm the list is complete.";
/** `vulnsUnverified: null` while `!vulnsChecked` (v1 cached rows only). */
export const NOT_VERIFIED_OTHER = "Not verified — Bonsai couldn't confirm this package against OSV.dev.";
/** §4.4 combined cell `title`: `{id} · {not-verified title}`. */
export function combinedVulnTitle(id: string, cause: string): string {
  return `${id} · ${cause}`;
}
/** §8.3 incomplete-list notes (details row; only rows that also have advisories). */
export const INCOMPLETE_MALFORMED =
  "Some advisories OSV.dev sent for this package couldn't be read, so this list may be incomplete.";
export const INCOMPLETE_TOO_LARGE =
  'Some advisories were too large for Bonsai to read, so they show only their ID. Open them on OSV.dev for the details.';

export function linkFailure(host: string): string {
  return `Couldn't open ${host} in your browser.`;
}
/** §8.3 copy note — same slot as the link note. */
export const COPY_FAILED = "Couldn't copy to the clipboard.";
export const FOOTER_SETTINGS = 'Dependency check settings';
export function nextCheckIn(inWords: string): string {
  return `Next check in ${inWords}`;
}

// ----- §8.4 labels -----------------------------------------------------------
export const ECOSYSTEM_LABEL: Readonly<Record<DepEcosystem, string>> = {
  npm: 'npm',
  cargo: 'Cargo',
  pypi: 'PyPI',
  go: 'Go',
  nuget: 'NuGet',
};
export const HOST_LABEL: Readonly<Record<DepEcosystem, string>> = {
  npm: 'npmjs.com',
  cargo: 'crates.io',
  pypi: 'PyPI',
  go: 'pkg.go.dev',
  nuget: 'NuGet.org',
};
export const ADVISORY_HOST = 'OSV.dev';
export const LOCAL_REASON: Readonly<Record<DepLocalReason, string>> = {
  path: 'folder on disk',
  git: 'Git dependency',
  workspace: 'workspace package',
  url: 'direct URL',
};
/** §8.4 private reasons: `[prefix, host]` when a host is present, else the null text. */
export const PRIVATE_REASON_WITH_HOST: Readonly<Partial<Record<DepPrivateReason, string>>> = {
  customRegistry: 'from',
  scopedRegistry: 'scoped to',
  sourceMapping: 'mapped to',
};
export const PRIVATE_REASON_NO_HOST: Readonly<Record<DepPrivateReason, string>> = {
  customRegistry: 'custom or local feed',
  scopedRegistry: 'scoped registry',
  sourceMapping: 'private package source',
  goPrivate: 'listed in GOPRIVATE',
  configUnreadable: "settings couldn't be read",
  localFeed: 'folder feed',
};
export const PRIVATE_TITLE_CUSTOM =
  "Resolved from a registry or feed that isn't the public one, so Bonsai doesn't send it.";
export const PRIVATE_TITLE_CONFIG =
  "Bonsai couldn't read or safely follow this ecosystem's registry settings (for example a linked file outside your home folder), so it treats every package as private.";
export const PRIVATE_TITLE_LOCAL_FEED =
  "The NuGet settings point only to package feeds in folders (on this computer or a network share), so Bonsai doesn't send these packages.";
export const PRIVATE_TITLE_OTHER = "Bonsai doesn't send packages from private sources.";
/** §8.4 unknown reasons, by `availability.reason` (P125h). */
export const UNKNOWN_REASON: Readonly<Record<DepUnknownReason, string>> = {
  notFound: 'not found on deps.dev',
  tooLarge: 'too large to look up',
};
export const UNKNOWN_TITLE: Readonly<Record<DepUnknownReason, string>> = {
  notFound:
    "deps.dev has no record of this package. It may be misspelled, unpublished, or from a registry Bonsai can't see.",
  tooLarge:
    "deps.dev's record for this package was larger than Bonsai accepts, so the latest version isn't known. Vulnerabilities are checked separately.",
};
