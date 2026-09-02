// P89: canned PR base…head (three-dot) diff fixtures for the browser harness
// (served by src/ipc/mock/handlers/forge.ts). Zero network. Shapes mirror the
// Rust `PrDiffStats` (+ reused `FileDiffHeader` / `FileDiff`). The set covers a
// realistic changed-files mix (added / modified / deleted / binary) so the
// PrDetailView files list + per-file diff viewer render offline.
import { annotateIntraline } from '../mock/intralineMock';
import { lineDiff } from './diffs';
import type { AppError, FileDiff, FileDiffHeader, FileStatus, PrDiffStats } from '../types';

// P93 fixtures: a ~180-char deep path and a long rename pair exercise the row's
// single-line ellipsis in both densities; `PR_FAIL_PATH` is the error sentinel
// (any path containing `fail` rejects) so the overlay's `.diff-slot-error`
// banner is reachable in the harness.
const LONG_PATH =
  'src/features/pull-requests/detail/changed-files/very/deeply/nested/directory/structure/' +
  'that/keeps/going/for/a/while/components/PrChangedFileRowContainerImplementation.tsx';
const RENAME_OLD_PATH =
  'src/legacy/pull-requests/detail/changed-files/PrChangedFilesLegacySectionImplementation.tsx';
const RENAME_NEW_PATH =
  'src/features/pull-requests/detail/changed-files/PrChangedFilesSectionImplementation.tsx';
/** Sentinel: `mockPrFileDiff` rejects for this file (harness error state). */
export const PR_FAIL_PATH = 'src/ci/fail-on-purpose.rs';

// Deterministic 40-hex oids for the PR endpoints (base…head three-dot).
const MERGE_BASE_OID = 'aa11bb22cc33dd44ee55ff6677889900aabbccdd';
const BASE_OID = 'bb22cc33dd44ee55ff6677889900aabbccddeeff';
const HEAD_OID = 'cc33dd44ee55ff6677889900aabbccddeeff0011';

// P93 AC9: the two flagship fixtures are deliberately LONG with TWO changed
// regions separated by more than 2×3 unchanged lines, so `Diff` mode yields two
// small hunks while `File` mode (fullContext) yields the whole body. Without that
// gap `asFullContext`-style merging is a no-op and the toolbar toggle is
// unobservable in the harness.
const SERVER_RS_OLD = [
  'use std::net::SocketAddr;',
  '',
  'pub struct Server {',
  '    port: u16,',
  '}',
  '',
  'impl Server {',
  '    pub fn new(port: u16) -> Self {',
  '        Self { port }',
  '    }',
  '',
  '    pub fn addr(&self) -> SocketAddr {',
  '        SocketAddr::from(([127, 0, 0, 1], self.port))',
  '    }',
  '',
  '    pub fn port(&self) -> u16 {',
  '        self.port',
  '    }',
  '',
  '    pub fn describe(&self) -> String {',
  '        format!("server on {}", self.port)',
  '    }',
  '}',
  '',
  '#[cfg(test)]',
  'mod tests {',
  '    use super::*;',
  '',
  '    #[test]',
  '    fn builds() {',
  '        let s = Server::new(8080);',
  '        assert_eq!(s.port(), 8080);',
  '    }',
  '}',
];
const SERVER_RS_NEW = [
  'use std::net::SocketAddr;',
  '',
  'pub struct Server {',
  '    port: u16,',
  '    host: String,',
  '}',
  '',
  'impl Server {',
  '    pub fn new(host: String, port: u16) -> Self {',
  '        Self { host, port }',
  '    }',
  '',
  '    pub fn addr(&self) -> SocketAddr {',
  '        SocketAddr::from(([127, 0, 0, 1], self.port))',
  '    }',
  '',
  '    pub fn port(&self) -> u16 {',
  '        self.port',
  '    }',
  '',
  '    pub fn describe(&self) -> String {',
  '        format!("server on {}:{}", self.host, self.port)',
  '    }',
  '}',
  '',
  '#[cfg(test)]',
  'mod tests {',
  '    use super::*;',
  '',
  '    #[test]',
  '    fn builds() {',
  '        let s = Server::new("localhost".into(), 8080);',
  '        assert_eq!(s.port(), 8080);',
  '    }',
  '}',
];

const README_OLD = [
  '# Bonsai',
  '',
  'A local Git client.',
  '',
  '## Status',
  '',
  'Early days. Everything below is subject to change.',
  '',
  '## Building',
  '',
  '```sh',
  'pnpm install',
  'pnpm tauri dev',
  '```',
  '',
  '## Layout',
  '',
  '- `src-tauri/` — the Rust backend (git2, the graph layout).',
  '- `src/` — the React frontend (canvas graph, panels).',
  '',
  '## License',
  '',
  'MIT.',
];
const README_NEW = [
  '# Bonsai',
  '',
  'A fast, native-feeling local Git client.',
  '',
  '## Status',
  '',
  'Early days. Everything below is subject to change.',
  '',
  '## Building',
  '',
  '```sh',
  'pnpm install',
  'pnpm tauri dev',
  '```',
  '',
  '## Layout',
  '',
  '- `src-tauri/` — the Rust backend (git2, the graph layout).',
  '- `src/` — the React frontend (canvas graph, panels).',
  '',
  '## Pull requests',
  '',
  'Browse pull requests and diff their changed files locally, in the center',
  'overlay, without leaving the app.',
  '',
  '## License',
  '',
  'MIT.',
];

/** Per-file SOURCE PAIRS (old ⇢ new whole-file bodies), keyed by path. Kept as
 *  sources rather than pre-baked hunks so `mockPrFileDiff` can honour
 *  `fullContext` for real: `Diff` mode renders 3-line-context hunks, `File` mode
 *  renders the entire body (P93 AC9). */
const PR_FILE_SOURCES: Record<
  string,
  { old: string[]; new: string[]; status: FileStatus; origPath?: string }
> = {
  'src/server.rs': { old: SERVER_RS_OLD, new: SERVER_RS_NEW, status: 'modified' },
  'README.md': { old: README_OLD, new: README_NEW, status: 'modified' },
  'src/pr/view.rs': {
    old: [],
    new: ['pub struct PrView;', '', 'impl PrView {', '    pub fn render(&self) {}', '}'],
    status: 'added',
  },
  'src/legacy.rs': {
    old: ['pub fn legacy() {', '    // removed in this PR', '}'],
    new: [],
    status: 'deleted',
  },
  [LONG_PATH]: {
    old: ['export const Row = () => null;'],
    new: ['export const Row = () => <li />;', '', '// P93: long-path fixture.'],
    status: 'modified',
  },
  [RENAME_NEW_PATH]: {
    old: ['export function LegacySection() {}'],
    new: ['export function Section() {}'],
    status: 'renamed',
    origPath: RENAME_OLD_PATH,
  },
};

/** The one binary changed file — no text diff to build from sources. */
const PR_BINARY_DIFF: FileDiff = {
  path: 'assets/preview.png',
  origPath: null,
  status: 'modified',
  binary: true,
  tooLarge: false,
  hunks: [],
};

const PR_DIFF_HEADERS: FileDiffHeader[] = [
  { path: 'README.md', origPath: null, status: 'modified', additions: 6, deletions: 1, binary: false },
  { path: 'assets/preview.png', origPath: null, status: 'modified', additions: 0, deletions: 0, binary: true },
  { path: 'src/legacy.rs', origPath: null, status: 'deleted', additions: 0, deletions: 3, binary: false },
  { path: 'src/pr/view.rs', origPath: null, status: 'added', additions: 5, deletions: 0, binary: false },
  { path: 'src/server.rs', origPath: null, status: 'modified', additions: 5, deletions: 4, binary: false },
  { path: PR_FAIL_PATH, origPath: null, status: 'modified', additions: 1, deletions: 1, binary: false },
  { path: LONG_PATH, origPath: null, status: 'modified', additions: 3, deletions: 1, binary: false },
  {
    path: RENAME_NEW_PATH,
    origPath: RENAME_OLD_PATH,
    status: 'renamed',
    additions: 1,
    deletions: 1,
    binary: false,
  },
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
 *  fall back to a small generic modified diff so the viewer never errors.
 *
 *  P93: honours `fullContext` (File view = one whole-file hunk) and `intraline`
 *  ("Highlight changes") exactly like the mock workdir/commit diff handlers, so
 *  the overlay's toolbar toggles are observably different in the harness. A path
 *  containing `fail` REJECTS (harness error state). */
export function mockPrFileDiff(
  path: string,
  origPath: string | null,
  fullContext: boolean,
  intraline: boolean,
): FileDiff {
  if (path.includes('fail')) {
    const err: AppError = {
      kind: 'forgeApi',
      message: `mock: could not compute the diff for ${path}`,
    };
    throw err;
  }
  if (path === PR_BINARY_DIFF.path) return structuredClone(PR_BINARY_DIFF);
  const src = PR_FILE_SOURCES[path];
  const built =
    src === undefined
      ? lineDiff(
          ['line one', 'line two', 'line three'],
          ['line one', 'line two changed', 'line three'],
          path,
          'modified',
          fullContext,
        )
      : lineDiff(src.old, src.new, path, src.status, fullContext);
  const cloned = structuredClone({
    ...built,
    origPath: src?.origPath ?? origPath,
  } satisfies FileDiff);
  return intraline ? annotateIntraline(cloned) : cloned;
}
