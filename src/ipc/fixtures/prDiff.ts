// P89: canned PR base…head (three-dot) diff fixtures for the browser harness
// (served by src/ipc/mock/handlers/forge.ts). Zero network. Shapes mirror the
// Rust `PrDiffStats` (+ reused `FileDiffHeader` / `FileDiff`). The set covers a
// realistic changed-files mix (added / modified / deleted / binary) so the
// PrDetailView files list + per-file diff viewer render offline.
import { lineDiff } from './diffs';
import type { FileDiff, FileDiffHeader, PrDiffStats } from '../types';

// Deterministic 40-hex oids for the PR endpoints (base…head three-dot).
const MERGE_BASE_OID = 'aa11bb22cc33dd44ee55ff6677889900aabbccdd';
const BASE_OID = 'bb22cc33dd44ee55ff6677889900aabbccddeeff';
const HEAD_OID = 'cc33dd44ee55ff6677889900aabbccddeeff0011';

const HEADER_RS_OLD = [
  'pub struct Server {',
  '    port: u16,',
  '}',
  '',
  'impl Server {',
  '    pub fn new(port: u16) -> Self {',
  '        Self { port }',
  '    }',
  '}',
];
const HEADER_RS_NEW = [
  'pub struct Server {',
  '    port: u16,',
  '    host: String,',
  '}',
  '',
  'impl Server {',
  '    pub fn new(host: String, port: u16) -> Self {',
  '        Self { host, port }',
  '    }',
  '}',
];

const README_OLD = [
  '# Bonsai',
  '',
  'A local Git client.',
];
const README_NEW = [
  '# Bonsai',
  '',
  'A fast, native-feeling local Git client.',
  '',
  '## Pull requests',
  '',
  'Browse and diff PRs locally.',
];

// Per-file FileDiff hunks, keyed by path — reused for forgePrFileDiff.
const PR_FILE_DIFFS: Record<string, FileDiff> = {
  'src/server.rs': lineDiff(HEADER_RS_OLD, HEADER_RS_NEW, 'src/server.rs', 'modified', false),
  'README.md': lineDiff(README_OLD, README_NEW, 'README.md', 'modified', false),
  'src/pr/view.rs': lineDiff(
    [],
    [
      'pub struct PrView;',
      '',
      'impl PrView {',
      '    pub fn render(&self) {}',
      '}',
    ],
    'src/pr/view.rs',
    'added',
    false,
  ),
  'src/legacy.rs': lineDiff(
    ['pub fn legacy() {', '    // removed in this PR', '}'],
    [],
    'src/legacy.rs',
    'deleted',
    false,
  ),
  'assets/preview.png': {
    path: 'assets/preview.png',
    origPath: null,
    status: 'modified',
    binary: true,
    tooLarge: false,
    hunks: [],
  },
};

const PR_DIFF_HEADERS: FileDiffHeader[] = [
  { path: 'README.md', origPath: null, status: 'modified', additions: 4, deletions: 1, binary: false },
  { path: 'assets/preview.png', origPath: null, status: 'modified', additions: 0, deletions: 0, binary: true },
  { path: 'src/legacy.rs', origPath: null, status: 'deleted', additions: 0, deletions: 3, binary: false },
  { path: 'src/pr/view.rs', origPath: null, status: 'added', additions: 5, deletions: 0, binary: false },
  { path: 'src/server.rs', origPath: null, status: 'modified', additions: 3, deletions: 1, binary: false },
];

/** The canned local base…head diff stats for a PR (any number). */
export const PR_DIFF_STATS: PrDiffStats = {
  additions: PR_DIFF_HEADERS.reduce((n, h) => n + h.additions, 0),
  deletions: PR_DIFF_HEADERS.reduce((n, h) => n + h.deletions, 0),
  changedFiles: PR_DIFF_HEADERS.length,
  mergeBaseOid: MERGE_BASE_OID,
  baseOid: BASE_OID,
  headOid: HEAD_OID,
  files: PR_DIFF_HEADERS,
};

/** An empty diff (base === head) for the `empty` state. */
export const PR_DIFF_STATS_EMPTY: PrDiffStats = {
  additions: 0,
  deletions: 0,
  changedFiles: 0,
  mergeBaseOid: MERGE_BASE_OID,
  baseOid: HEAD_OID,
  headOid: HEAD_OID,
  files: [],
};

/** Hunks for ONE file of the PR diff, mirroring `pr_file_diff`. Unknown paths
 *  fall back to a small generic modified diff so the viewer never errors. */
export function mockPrFileDiff(path: string, origPath: string | null): FileDiff {
  const known = PR_FILE_DIFFS[path];
  if (known) return known;
  const fallback = lineDiff(
    ['line one', 'line two', 'line three'],
    ['line one', 'line two changed', 'line three'],
    path,
    'modified',
    false,
  );
  return { ...fallback, origPath };
}
