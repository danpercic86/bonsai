// P125 E5 — the `DEP_VIEW_POPULATED` / `DEP_VIEW_UPTODATE` group tables.
//
// Covers every AC13 row kind: patch/minor/major updates, a vulnerable DIRECT
// row (critical) and a vulnerable TRANSITIVE row (high), a `local` path dep, a
// `private` scoped dep with its `registryHost`, an `unknown` row that still
// carries an advisory [UI-e], a `noLockfile` group, a `notTracked` group and a
// `parseError` group — plus the [UI-c] long-content rows: a 70-char scoped npm
// name, a Go pseudo-version, a 4-advisory row (one `unknown` severity with
// `summary: null`, one with 3 aliases), a `dev` + `transitive` row and a deep
// manifest path.
import type { DepManifestGroup } from '../types/depTracker';
import {
  advisory,
  group,
  notTrackedGroup,
  parseMessage,
  row,
  sortGroups,
} from './depTrackerModel';

/** [UI-c] exactly 70 characters. */
export const LONG_NPM_NAME = '@enterprise-platform-foundations/observability-instrumentation-bridges';
/** [UI-c] a Go pseudo-version. */
export const GO_PSEUDO_VERSION = 'v0.0.0-20240101123456-abcdef123456';
/** [UI-c] the deep manifest path. */
export const DEEP_MANIFEST_PATH = 'packages/very/deeply/nested/workspace/member/package.json';

const rootPackageJson = group('package.json', 'pnpm-lock.yaml', 'npm', [
  row({ name: 'react', ecosystem: 'npm', declared: '^18.2.0', current: '18.2.0', latest: '19.1.0', update: 'major' }),
  row({ name: 'vite', ecosystem: 'npm', scope: 'dev', declared: '^6.0.1', current: '6.0.1', latest: '6.3.5', update: 'minor' }),
  row({ name: 'typescript', ecosystem: 'npm', scope: 'dev', declared: '~5.8.2', current: '5.8.2', latest: '5.8.3', update: 'patch' }),
  row({ name: 'zustand', ecosystem: 'npm', declared: '^5.0.3', current: '5.0.3', latest: '5.0.3' }),
  // Vulnerable DIRECT (critical).
  row({
    name: '@babel/traverse',
    ecosystem: 'npm',
    scope: 'dev',
    declared: '7.22.5',
    current: '7.22.5',
    latest: '7.27.0',
    update: 'minor',
    vulns: [
      advisory(
        'GHSA-67hx-6x53-jw92',
        'critical',
        'Babel vulnerable to arbitrary code execution when compiling specifically crafted malicious code',
        9.3,
        ['CVE-2023-45133'],
      ),
    ],
  }),
  // Vulnerable TRANSITIVE (high).
  row({
    name: 'braces',
    ecosystem: 'npm',
    relation: 'transitive',
    current: '3.0.2',
    vulns: [
      advisory('GHSA-grv7-fg5c-xmjg', 'high', 'Uncontrolled resource consumption in braces', 7.5, [
        'CVE-2024-4068',
      ]),
    ],
  }),
  // [UI-c] dev + transitive double tag.
  row({
    name: 'esbuild',
    ecosystem: 'npm',
    relation: 'transitive',
    scope: 'dev',
    current: '0.19.12',
    vulns: [
      advisory(
        'GHSA-67mh-4wv8-2f99',
        'medium',
        'esbuild enables any website to send any requests to the development server and read the response',
        5.3,
      ),
    ],
  }),
  // [UI-c] 4 advisories: one `unknown` + `summary: null`, one with 3 aliases.
  row({
    name: 'axios',
    ecosystem: 'npm',
    declared: '^0.21.1',
    current: '0.21.1',
    latest: '1.9.0',
    update: 'major',
    vulns: [
      advisory('GHSA-jr5f-v2jv-69x6', 'high', 'axios Requests Vulnerable To Possible SSRF and Credential Leakage via Absolute URL', 7.5, [
        'CVE-2025-27152',
        'BIT-axios-2025-27152',
        'SNYK-JS-AXIOS-9292519',
      ]),
      advisory('GHSA-cph5-m8f7-6c5x', 'high', 'axios Inefficient Regular Expression Complexity vulnerability', 7.5, [
        'CVE-2021-3749',
      ]),
      advisory('GHSA-wf5p-g6vw-rhxx', 'medium', 'Axios Cross-Site Request Forgery Vulnerability', 6.5, [
        'CVE-2023-45857',
      ]),
      advisory('GHSA-4w2v-q235-vp99', 'unknown', null, null),
    ],
  }),
  // `private` scoped dep, host shown (never a link).
  row({
    name: '@acme/design-tokens',
    ecosystem: 'npm',
    declared: '^3.4.0',
    current: '3.4.2',
    availability: { kind: 'private', reason: 'scopedRegistry', registryHost: 'npm.acme.example' },
  }),
  // `unknown` (deps.dev 404) that STILL carries an advisory [UI-e].
  row({
    name: 'left-pad-legacy',
    ecosystem: 'npm',
    declared: '1.3.0',
    current: '1.3.0',
    availability: { kind: 'unknown' },
    vulns: [advisory('MAL-2025-4102', 'critical', 'Malicious code in left-pad-legacy (npm)', null)],
  }),
], 412);

const deepWorkspace = group(DEEP_MANIFEST_PATH, 'pnpm-lock.yaml', 'npm', [
  // [UI-c] the 70-char scoped name.
  row({ name: LONG_NPM_NAME, ecosystem: 'npm', declared: '^2.14.0', current: '2.14.0', latest: '2.18.1', update: 'minor' }),
  row({ name: 'zod', ecosystem: 'npm', declared: '^3.24.2', current: '3.24.2', latest: '3.24.2' }),
  row({ name: '@bonsai/shared', ecosystem: 'npm', declared: 'workspace:*', current: '0.4.0', availability: { kind: 'local', reason: 'workspace' } }),
], 0);

const cargo = group('src-tauri/Cargo.toml', 'src-tauri/Cargo.lock', 'cargo', [
  row({ name: 'tauri', ecosystem: 'cargo', declared: '2.5', current: '2.5.1', latest: '2.6.2', update: 'minor' }),
  row({ name: 'git2', ecosystem: 'cargo', declared: '0.19', current: '0.19.0', latest: '0.20.2', update: 'minor' }),
  row({ name: 'notify', ecosystem: 'cargo', declared: '6.1', current: '6.1.1', latest: '8.0.0', update: 'major' }),
  row({ name: 'serde', ecosystem: 'cargo', declared: '1', current: '1.0.219', latest: '1.0.219' }),
  row({ name: 'serde_json', ecosystem: 'cargo', declared: '1', current: '1.0.139', latest: '1.0.140', update: 'patch' }),
  row({ name: 'bonsai-core', ecosystem: 'cargo', declared: null, current: '0.1.0', availability: { kind: 'local', reason: 'path' } }),
], 486);

const goModule = group('cmd/indexer/go.mod', 'cmd/indexer/go.sum', 'go', [
  // [UI-c] the Go pseudo-version.
  row({ name: 'github.com/bonsai-dev/fsnotify-shim', ecosystem: 'go', declared: GO_PSEUDO_VERSION, current: GO_PSEUDO_VERSION, latest: GO_PSEUDO_VERSION }),
  row({
    name: 'golang.org/x/net',
    ecosystem: 'go',
    declared: 'v0.33.0',
    current: 'v0.33.0',
    latest: 'v0.38.0',
    update: 'minor',
    vulns: [
      advisory('GO-2025-3503', 'medium', 'HTTP Proxy bypass using IPv6 Zone IDs in golang.org/x/net', 4.4, [
        'CVE-2025-22870',
        'GHSA-qxp5-gwg8-xv66',
      ]),
    ],
  }),
  row({
    name: 'github.com/acme/internal-sdk',
    ecosystem: 'go',
    declared: 'v1.12.0',
    current: 'v1.12.0',
    availability: { kind: 'private', reason: 'goPrivate', registryHost: null },
  }),
], 37);

const requirements = group('tools/requirements.txt', null, 'pypi', [
  // No lockfile ⇒ declared ranges only, never vuln-checked.
  row({ name: 'requests', ecosystem: 'pypi', declared: '>=2.31', latest: '2.32.3', vulnsChecked: false }),
  row({ name: 'pyyaml', ecosystem: 'pypi', declared: '~=6.0', latest: '6.0.2', vulnsChecked: false }),
], 0, { kind: 'noLockfile' });

const gradle = notTrackedGroup('android/build.gradle', 'Gradle');

const brokenGo: DepManifestGroup = {
  manifestPath: 'broken/go.mod',
  lockfilePath: null,
  ecosystem: 'go',
  state: {
    kind: 'parseError',
    file: 'broken/go.mod',
    category: 'invalidSyntax',
    line: 3,
    col: 1,
    message: parseMessage('invalidSyntax', 3, 1),
  },
  rows: [],
  transitiveChecked: 0,
};

/** The `DEP_VIEW_POPULATED` report groups, §5.2-ordered. */
export const POPULATED_GROUPS: readonly DepManifestGroup[] = sortGroups([
  rootPackageJson,
  deepWorkspace,
  cargo,
  goModule,
  requirements,
  gradle,
  brokenGo,
]);

/** The `DEP_VIEW_UPTODATE` groups: nothing to update and no advisories. A
 *  local row is kept so the "Some weren't checked" suffix path is reachable. */
export const UPTODATE_GROUPS: readonly DepManifestGroup[] = sortGroups([
  group('package.json', 'pnpm-lock.yaml', 'npm', [
    row({ name: 'react', ecosystem: 'npm', declared: '^19.1.0', current: '19.1.0', latest: '19.1.0' }),
    row({ name: 'vite', ecosystem: 'npm', scope: 'dev', declared: '^6.3.5', current: '6.3.5', latest: '6.3.5' }),
    row({ name: 'zod', ecosystem: 'npm', declared: '^3.24.2', current: '3.24.2', latest: '3.24.2' }),
  ], 398),
  group('src-tauri/Cargo.toml', 'src-tauri/Cargo.lock', 'cargo', [
    row({ name: 'serde', ecosystem: 'cargo', declared: '1', current: '1.0.219', latest: '1.0.219' }),
    row({ name: 'tauri', ecosystem: 'cargo', declared: '2.6', current: '2.6.2', latest: '2.6.2' }),
    row({ name: 'bonsai-core', ecosystem: 'cargo', declared: null, current: '0.1.0', availability: { kind: 'local', reason: 'path' } }),
  ], 471),
]);
