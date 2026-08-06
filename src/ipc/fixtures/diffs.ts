// Canned FileDiff / CommitDiff fixtures for the browser harness (M4 contract §5).
// Static per-path objects: the stateful status mock moves entries between
// sections, and a path's diff is the same wherever it sits. Exactly one path
// (`src/shared/util.rs`) distinguishes staged vs unstaged content.

import type {
  CommitDiff,
  CompareDiff,
  DiffLine,
  FileDiff,
  FileDiffHeader,
  FileStatus,
  GraphLayout,
  Hunk,
  LineKind,
} from '../types';
import { buildMockGraph } from './graph';

const HOUR = 3600;

// ---------- tiny builders ----------

function ctx(oldNo: number, newNo: number, content: string): DiffLine {
  return { kind: 'context', oldNo, newNo, content };
}

function add(newNo: number, content: string, noNewline = false): DiffLine {
  const line: DiffLine = { kind: 'add', oldNo: null, newNo, content };
  if (noNewline) line.noNewline = true;
  return line;
}

function del(oldNo: number, content: string, noNewline = false): DiffLine {
  const line: DiffLine = { kind: 'del', oldNo, newNo: null, content };
  if (noNewline) line.noNewline = true;
  return line;
}

function hunk(
  oldStart: number,
  oldLines: number,
  newStart: number,
  newLines: number,
  lines: DiffLine[],
): Hunk {
  return { oldStart, oldLines, newStart, newLines, lines };
}

function fileDiff(
  path: string,
  status: FileStatus,
  hunks: Hunk[],
  opts: { origPath?: string; binary?: boolean; tooLarge?: boolean } = {},
): FileDiff {
  return {
    path,
    origPath: opts.origPath ?? null,
    status,
    binary: opts.binary ?? false,
    tooLarge: opts.tooLarge ?? false,
    hunks: opts.binary === true || opts.tooLarge === true ? [] : hunks,
  };
}

/** All-Add hunk from plain lines (untracked / added files). */
function allAddHunk(lines: string[], noNewlineLast = false): Hunk {
  return hunk(
    0,
    0,
    1,
    lines.length,
    lines.map((content, i) => add(i + 1, content, noNewlineLast && i === lines.length - 1)),
  );
}

// ---------- workdir diffs (mode A), keyed by exact path ----------

const APP_RS: FileDiff = fileDiff('src/app.rs', 'added', [
  allAddHunk([
    'use crate::config::Config;',
    '',
    'pub struct App {',
    '    config: Config,',
    '}',
    '',
    'impl App {',
    '    pub fn new(config: Config) -> Self {',
    '        Self { config }',
    '    }',
    '}',
    '',
  ]),
]);

const OLD_CONFIG: FileDiff = fileDiff('old-config.toml', 'deleted', [
  hunk(1, 9, 0, 0, [
    del(1, '[core]'),
    del(2, 'theme = "dark"'),
    del(3, 'autosave = true'),
    del(4, ''),
    del(5, '[graph]'),
    del(6, 'lanes = 10'),
    del(7, 'row-height = 28'),
    del(8, ''),
    del(9, '# superseded by bonsai.toml', true),
  ]),
]);

const GETTING_STARTED: FileDiff = fileDiff(
  'docs/getting-started.md',
  'renamed',
  [
    hunk(1, 5, 1, 5, [
      del(1, '# Introduction'),
      add(1, '# Getting started'),
      ctx(2, 2, ''),
      del(3, 'Bonsai is a Git client.'),
      add(3, 'Bonsai is a fast, native-feeling Git client for Windows.'),
      ctx(4, 4, ''),
      ctx(5, 5, 'Open a repository to begin.'),
    ]),
  ],
  { origPath: 'docs/intro.md' },
);

const TODO_TXT: FileDiff = fileDiff('notes/todo.txt', 'untracked', [
  allAddHunk(['- wire up the diff view', '- test the accordion', '- ship it']),
]);

const SCRATCH_RS: FileDiff = fileDiff('scratch.rs', 'untracked', [
  allAddHunk(['fn scratch() {', '    todo!()', '}']),
]);

const LOGO_PNG: FileDiff = fileDiff('assets/logo.png', 'modified', [], { binary: true });

const BIG_CSV: FileDiff = fileDiff('data/big-report.csv', 'modified', [], { tooLarge: true });

/** The one path where staged vs unstaged content differs (contract §5). */
const UTIL_RS_STAGED: FileDiff = fileDiff('src/shared/util.rs', 'modified', [
  hunk(3, 3, 3, 3, [
    ctx(3, 3, 'pub fn short_oid(oid: &str) -> &str {'),
    del(4, '    &oid[..8]'),
    add(4, '    &oid[..7]'),
    ctx(5, 5, '}'),
  ]),
]);

const UTIL_RS_UNSTAGED: FileDiff = fileDiff('src/shared/util.rs', 'modified', [
  hunk(8, 3, 8, 4, [
    ctx(8, 8, 'pub fn folder_name(path: &str) -> &str {'),
    del(9, '    path.rsplit(\'/\').next().unwrap()'),
    add(9, '    path.rsplit([\'/\', \'\\\\\'])'),
    add(10, '        .next()'),
    ctx(10, 11, '}'),
  ]),
]);

const README_MD: FileDiff = fileDiff('README.md', 'modified', [
  hunk(1, 4, 1, 4, [
    ctx(1, 1, '# Bonsai'),
    ctx(2, 2, ''),
    del(3, 'A Git client.'),
    add(3, 'A tidy Git client.'),
    ctx(4, 4, ''),
  ]),
]);

// P4e Step 2 visibility: diffs across extensions so syntax highlighting is
// exercised in the mock harness (.ts / .json / .css alongside the existing
// .rs / .md / .toml). Fixture DATA only — no shape change.
const IPC_TS: FileDiff = fileDiff('src/ipc/tabs.ts', 'modified', [
  hunk(1, 6, 1, 7, [
    ctx(1, 1, "import type { SessionState } from './types';"),
    ctx(2, 2, ''),
    del(3, 'export function activeTab(s: SessionState): string | null {'),
    add(3, 'export function activeTab(s: SessionState): string | null {'),
    add(4, '  // fall back to the first open repo when none is active'),
    ctx(4, 5, '  return s.activeRepo ?? s.openRepos[0] ?? null;'),
    ctx(5, 6, '}'),
  ]),
]);

const TSCONFIG_JSON: FileDiff = fileDiff('tsconfig.json', 'modified', [
  hunk(2, 5, 2, 5, [
    ctx(2, 2, '  "compilerOptions": {'),
    ctx(3, 3, '    "target": "ES2022",'),
    del(4, '    "strict": false,'),
    add(4, '    "strict": true,'),
    ctx(5, 5, '    "jsx": "react-jsx"'),
    ctx(6, 6, '  }'),
  ]),
]);

const THEME_CSS: FileDiff = fileDiff('src/styles/theme.css', 'modified', [
  hunk(1, 5, 1, 6, [
    ctx(1, 1, '.lang-chip {'),
    del(2, '  color: #888;'),
    add(2, '  color: var(--text-1);'),
    add(3, '  /* accent driven by data-lang */'),
    ctx(3, 4, '  border-radius: 4px;'),
    ctx(4, 5, '}'),
  ]),
]);

const WORKDIR_DIFFS: Record<string, FileDiff> = {
  'src/app.rs': APP_RS,
  'old-config.toml': OLD_CONFIG,
  'docs/getting-started.md': GETTING_STARTED,
  'notes/todo.txt': TODO_TXT,
  'scratch.rs': SCRATCH_RS,
  'assets/logo.png': LOGO_PNG,
  'data/big-report.csv': BIG_CSV,
  'README.md': README_MD,
  'src/ipc/tabs.ts': IPC_TS,
  'tsconfig.json': TSCONFIG_JSON,
  'src/styles/theme.css': THEME_CSS,
};

/** Generic 1-hunk modified diff for any path without a canned fixture. */
function genericModified(path: string, origPath: string | null): FileDiff {
  return {
    ...fileDiff(path, 'modified', [
      hunk(1, 3, 1, 3, [
        ctx(1, 1, `// ${path}`),
        del(2, 'let old = 1;'),
        add(2, 'let new = 2;'),
        ctx(3, 3, ''),
      ]),
    ]),
    origPath,
  };
}

export function mockWorkdirDiff(
  path: string,
  origPath: string | null,
  staged: boolean,
): FileDiff {
  if (path === 'src/shared/util.rs') return staged ? UTIL_RS_STAGED : UTIL_RS_UNSTAGED;
  return WORKDIR_DIFFS[path] ?? genericModified(path, origPath);
}

// ---------- commit diffs (mode B), routed by row index ----------

function header(
  path: string,
  status: FileStatus,
  additions: number,
  deletions: number,
  opts: { origPath?: string; binary?: boolean } = {},
): FileDiffHeader {
  return {
    path,
    origPath: opts.origPath ?? null,
    status,
    additions,
    deletions,
    binary: opts.binary ?? false,
  };
}

/** Full oids of the 30-row fixture; index-safe fallback for out-of-range rows. */
function fixtureOid(row: number): string | null {
  const nodes = buildMockGraph().nodes;
  return nodes[row]?.id ?? null;
}

const FAKE_PARENT_OID = 'f0e1d2c3b4a5968778695a4b3c2d1e0ff0e1d2c3';

export function mockCommitDiff(index: number, oid: string): CommitDiff {
  const base = Math.floor(Date.now() / 1000) - HOUR;
  const ts = base - index * HOUR;

  if (index === 0) {
    return {
      details: {
        oid,
        summary: 'Merge feat and exp',
        message:
          'Merge feat and exp\n\nBrings the polish work and the experiment together\nahead of the v1.0 tag.',
        authorName: 'Ada Lovelace',
        authorEmail: 'ada@example.com',
        authorTs: ts,
        committerTs: ts,
        parents: [fixtureOid(3), fixtureOid(1), fixtureOid(2)].filter(
          (p): p is string => p !== null,
        ),
      },
      files: [
        header('assets/icon.png', 'modified', 0, 0, { binary: true }),
        header('src/core/engine.rs', 'modified', 12, 4),
        header('src/core/pipeline.rs', 'added', 30, 0),
      ],
    };
  }
  if (index === 1) {
    return {
      details: {
        oid,
        summary: 'feat: polish',
        message:
          'feat: polish\n\nTighten spacing on the sidebar and align the ref pills.\nAlso bumps the default lane palette for better contrast.',
        authorName: 'Grace Hopper',
        authorEmail: 'grace@example.com',
        authorTs: ts,
        committerTs: ts,
        parents: [fixtureOid(4) ?? FAKE_PARENT_OID],
      },
      files: [
        header('docs/guide.md', 'renamed', 5, 2, { origPath: 'docs/manual.md' }),
        header('src/ui/theme.rs', 'modified', 7, 3),
      ],
    };
  }
  if (index === 7) {
    return {
      details: {
        oid,
        summary: 'core work 1',
        message: 'core work 1\n\nFirst pass over the core module.',
        authorName: 'Grace Hopper',
        authorEmail: 'grace@example.com',
        authorTs: ts,
        committerTs: ts,
        parents: [fixtureOid(8) ?? FAKE_PARENT_OID],
      },
      files: [header('src/core/init.rs', 'modified', 3, 1)],
    };
  }

  // Generic: reuse the fixture node's metadata when the row exists in the
  // 30-row layout; otherwise (e.g. 20k fixture) plain constants.
  const nodes = buildMockGraph().nodes;
  const node = nodes[index];
  const parentIdx = node?.parents[0];
  return {
    details: {
      oid,
      summary: node?.summary ?? 'mock commit',
      message: node?.summary ?? 'mock commit',
      authorName: node?.author ?? 'Ada Lovelace',
      authorEmail: 'dev@example.com',
      authorTs: node?.ts ?? ts,
      committerTs: node?.ts ?? ts,
      parents:
        node !== undefined && node.parents.length === 0
          ? []
          : [(parentIdx !== undefined ? nodes[parentIdx]?.id : null) ?? FAKE_PARENT_OID],
    },
    files: [header('src/lib.rs', 'modified', 3, 1)],
  };
}

// ---------- compare diffs (HEAD → selected), routed by row index ----------

/**
 * Tree-vs-tree comparison HEAD(old) → the right-clicked commit(new). Reuses the
 * commit-diff file list for a believable header set; empty when comparing HEAD
 * to itself (drives the "No differences" state).
 */
export function mockCompareDiff(
  fromOid: string,
  toOid: string,
  toIndex: number,
  layout: GraphLayout,
): CompareDiff {
  const fromSummary = layout.nodes.find((n) => n.id === fromOid)?.summary ?? 'HEAD';
  const toSummary = layout.nodes[toIndex]?.summary ?? 'commit';
  const from = { oid: fromOid, summary: fromSummary };
  const to = { oid: toOid, summary: toSummary };
  if (fromOid === toOid) {
    return { from, to, files: [] };
  }
  return { from, to, files: mockCommitDiff(toIndex, toOid).files };
}

// ---------- per-file commit hunks, keyed by path ----------

const ENGINE_RS: FileDiff = fileDiff('src/core/engine.rs', 'modified', [
  hunk(10, 8, 10, 9, [
    ctx(10, 10, 'impl Engine {'),
    ctx(11, 11, '    pub fn tick(&mut self) {'),
    del(12, '        self.frame += 1;'),
    add(12, '        self.frame = self.frame.wrapping_add(1);'),
    add(13, '        self.stats.record(self.frame);'),
    ctx(13, 14, '        self.render();'),
    ctx(14, 15, '    }'),
    ctx(15, 16, ''),
    del(16, '    fn render(&self) {'),
    add(17, '    fn render(&mut self) {'),
    ctx(17, 18, '        // draw the current frame'),
  ]),
]);

const PIPELINE_RS: FileDiff = fileDiff('src/core/pipeline.rs', 'added', [
  allAddHunk(
    Array.from({ length: 30 }, (_, i) => {
      const n = i + 1;
      if (n === 1) return 'pub struct Pipeline {';
      if (n === 2) return '    stages: Vec<Stage>,';
      if (n === 3) return '}';
      return `// stage wiring line ${n}`;
    }),
  ),
]);

const ICON_PNG: FileDiff = fileDiff('assets/icon.png', 'modified', [], { binary: true });

const GUIDE_MD: FileDiff = fileDiff(
  'docs/guide.md',
  'renamed',
  [
    hunk(1, 4, 1, 5, [
      del(1, '# Manual'),
      add(1, '# Guide'),
      ctx(2, 2, ''),
      del(3, 'Read this first.'),
      add(3, 'Start here.'),
      add(4, 'Then read the FAQ.'),
      ctx(4, 5, ''),
    ]),
  ],
  { origPath: 'docs/manual.md' },
);

const THEME_RS: FileDiff = fileDiff('src/ui/theme.rs', 'modified', [
  hunk(5, 5, 5, 6, [
    ctx(5, 5, 'pub const ACCENT: &str = "#4f8cff";'),
    del(6, 'pub const BG: &str = "#101216";'),
    add(6, 'pub const BG: &str = "#16181d";'),
    add(7, 'pub const BG_PANEL: &str = "#1d2026";'),
    ctx(7, 8, ''),
    ctx(8, 9, 'pub fn theme() -> Theme {'),
    ctx(9, 10, '    Theme::dark()'),
  ]),
]);

const INIT_RS: FileDiff = fileDiff('src/core/init.rs', 'modified', [
  hunk(1, 4, 1, 4, [
    ctx(1, 1, 'pub fn init() -> Core {'),
    del(2, '    Core::new()'),
    add(2, '    Core::with_defaults()'),
    ctx(3, 3, '}'),
    ctx(4, 4, ''),
  ]),
]);

const COMMIT_FILE_DIFFS: Record<string, FileDiff> = {
  'src/core/engine.rs': ENGINE_RS,
  'src/core/pipeline.rs': PIPELINE_RS,
  'assets/icon.png': ICON_PNG,
  'docs/guide.md': GUIDE_MD,
  'src/ui/theme.rs': THEME_RS,
  'src/core/init.rs': INIT_RS,
};

export function mockCommitFileDiff(
  _oid: string,
  path: string,
  origPath: string | null,
): FileDiff {
  return COMMIT_FILE_DIFFS[path] ?? genericModified(path, origPath);
}

// ---------- P17: live three-way model for src/main.rs ----------
//
// Exactly one fixture file (`src/main.rs`) gets a three-way line-array model
// (head / index / workdir). Its diffs and partial mutations are computed from
// the arrays with the SAME reconstruction rule as the Rust backend (§2.4),
// operating on string[] (line terminators are irrelevant here). Every other
// file stays static and rejects partial staging.

/** head / index / workdir line arrays for the one live model file. */
export interface ThreeWay {
  head: string[];
  index: string[];
  workdir: string[];
}

/**
 * Seed for src/main.rs. `index != head` (a staged change: an added import line)
 * AND `workdir != index` (an unstaged change: `Arc::new(...)` wrap) so the file
 * appears in BOTH the staged and unstaged sections from first paint. Returns
 * fresh arrays — callers own the copy.
 */
export function initialMainRs(): ThreeWay {
  const head = [
    'use std::path::PathBuf;',
    '',
    'fn main() {',
    '    let config = load_config();',
    '    run(config);',
    '}',
    '',
    'fn load_config() -> Config {',
    '    let path = PathBuf::from("bonsai.toml");',
    '    Config::from_file(&path)',
    '}',
  ];
  // Staged change: `use std::sync::Arc;` inserted after line 1.
  const index = [
    'use std::path::PathBuf;',
    'use std::sync::Arc;',
    '',
    'fn main() {',
    '    let config = load_config();',
    '    run(config);',
    '}',
    '',
    'fn load_config() -> Config {',
    '    let path = PathBuf::from("bonsai.toml");',
    '    Config::from_file(&path)',
    '}',
  ];
  // Unstaged change on top of the index: wrap the config in `Arc::new(...)`.
  const workdir = [
    'use std::path::PathBuf;',
    'use std::sync::Arc;',
    '',
    'fn main() {',
    '    let config = Arc::new(load_config());',
    '    run(config);',
    '}',
    '',
    'fn load_config() -> Config {',
    '    let path = PathBuf::from("bonsai.toml");',
    '    Config::from_file(&path)',
    '}',
  ];
  return { head, index, workdir };
}

interface DiffOp {
  kind: LineKind;
  oldNo: number | null;
  newNo: number | null;
  content: string;
}

/** Longest-common-subsequence edit script over two small line arrays. */
function diffOps(oldLines: string[], newLines: string[]): DiffOp[] {
  const n = oldLines.length;
  const m = newLines.length;
  // dp[i][j] = LCS length of oldLines[i..] vs newLines[j..].
  const dp: number[][] = Array.from({ length: n + 1 }, () => new Array<number>(m + 1).fill(0));
  for (let i = n - 1; i >= 0; i--) {
    for (let j = m - 1; j >= 0; j--) {
      dp[i][j] =
        oldLines[i] === newLines[j]
          ? dp[i + 1][j + 1] + 1
          : Math.max(dp[i + 1][j], dp[i][j + 1]);
    }
  }
  const ops: DiffOp[] = [];
  let i = 0;
  let j = 0;
  while (i < n && j < m) {
    if (oldLines[i] === newLines[j]) {
      ops.push({ kind: 'context', oldNo: i + 1, newNo: j + 1, content: oldLines[i] });
      i++;
      j++;
    } else if (dp[i + 1][j] >= dp[i][j + 1]) {
      ops.push({ kind: 'del', oldNo: i + 1, newNo: null, content: oldLines[i] });
      i++;
    } else {
      ops.push({ kind: 'add', oldNo: null, newNo: j + 1, content: newLines[j] });
      j++;
    }
  }
  while (i < n) {
    ops.push({ kind: 'del', oldNo: i + 1, newNo: null, content: oldLines[i] });
    i++;
  }
  while (j < m) {
    ops.push({ kind: 'add', oldNo: null, newNo: j + 1, content: newLines[j] });
    j++;
  }
  return ops;
}

function opToLine(op: DiffOp): DiffLine {
  return { kind: op.kind, oldNo: op.oldNo, newNo: op.newNo, content: op.content };
}

/** Group ops into hunks with `context` surrounding lines (Infinity => one hunk). */
function opsToHunks(ops: DiffOp[], context: number): Hunk[] {
  const changed: number[] = [];
  ops.forEach((op, idx) => {
    if (op.kind !== 'context') changed.push(idx);
  });
  if (changed.length === 0) return [];
  const ranges: Array<[number, number]> = [];
  for (const idx of changed) {
    const lo = Math.max(0, idx - context);
    const hi = Math.min(ops.length - 1, idx + context);
    const last = ranges[ranges.length - 1];
    if (last !== undefined && lo <= last[1] + 1) {
      last[1] = Math.max(last[1], hi);
    } else {
      ranges.push([lo, hi]);
    }
  }
  return ranges.map(([lo, hi]) => {
    const slice = ops.slice(lo, hi + 1);
    const lines = slice.map(opToLine);
    const oldNos = slice.filter((o) => o.oldNo !== null).map((o) => o.oldNo as number);
    const newNos = slice.filter((o) => o.newNo !== null).map((o) => o.newNo as number);
    const oldStart = oldNos.length > 0 ? oldNos[0] : 0;
    const newStart = newNos.length > 0 ? newNos[0] : 0;
    return hunk(oldStart, oldNos.length, newStart, newNos.length, lines);
  });
}

/**
 * Minimal LCS line diff -> FileDiff. `fullContext` true -> one whole-file hunk
 * (all lines as context); false -> 3 context lines around each change (possibly
 * several hunks). `fullContext` never changes the add/del line numbering.
 */
export function lineDiff(
  oldLines: string[],
  newLines: string[],
  path: string,
  status: FileStatus,
  fullContext: boolean,
): FileDiff {
  const ops = diffOps(oldLines, newLines);
  const hunks = opsToHunks(ops, fullContext ? Number.POSITIVE_INFINITY : 3);
  return fileDiff(path, status, hunks);
}

/**
 * SAME reconstruction rule as the Rust backend (§2.4), on string[]. Returns the
 * new `index` array given the freshly recomputed hunks + the selected add/del
 * sets. `stage`: base = OLD (index) toward workdir. `unstage`: base = NEW
 * (index) toward HEAD.
 */
export function reconstructLines(
  dir: 'stage' | 'unstage',
  hunks: Hunk[],
  oldLines: string[],
  newLines: string[],
  selAdd: Set<number>,
  selDel: Set<number>,
): string[] {
  const result: string[] = [];
  if (dir === 'stage') {
    let cursor = 1; // next OLD line not yet emitted
    for (const h of hunks) {
      while (cursor < h.oldStart) {
        result.push(oldLines[cursor - 1]);
        cursor++;
      }
      for (const line of h.lines) {
        if (line.kind === 'context') {
          result.push(oldLines[(line.oldNo as number) - 1]);
          cursor = (line.oldNo as number) + 1;
        } else if (line.kind === 'del') {
          if (!selDel.has(line.oldNo as number)) result.push(oldLines[(line.oldNo as number) - 1]);
          cursor = (line.oldNo as number) + 1;
        } else {
          // add
          if (selAdd.has(line.newNo as number)) result.push(newLines[(line.newNo as number) - 1]);
        }
      }
    }
    while (cursor <= oldLines.length) {
      result.push(oldLines[cursor - 1]);
      cursor++;
    }
  } else {
    let cursor = 1; // next NEW (index) line not yet emitted
    for (const h of hunks) {
      while (cursor < h.newStart) {
        result.push(newLines[cursor - 1]);
        cursor++;
      }
      for (const line of h.lines) {
        if (line.kind === 'context') {
          result.push(newLines[(line.newNo as number) - 1]);
          cursor = (line.newNo as number) + 1;
        } else if (line.kind === 'add') {
          if (!selAdd.has(line.newNo as number)) result.push(newLines[(line.newNo as number) - 1]);
          cursor = (line.newNo as number) + 1;
        } else {
          // del: restore the HEAD line when selected
          if (selDel.has(line.oldNo as number)) result.push(oldLines[(line.oldNo as number) - 1]);
        }
      }
    }
    while (cursor <= newLines.length) {
      result.push(newLines[cursor - 1]);
      cursor++;
    }
  }
  return result;
}

/**
 * Collapse a static fixture to a single whole-file hunk for File View. Only the
 * safe cases are merged: a diff that is already one hunk, or several PERFECTLY
 * CONTIGUOUS hunks (no unknown gap between them). When a gap cannot be filled
 * (the static fixture omits the intervening lines) the diff is returned
 * unchanged — the harness only exercises File View interactivity on the live
 * `src/main.rs` model, so exactness for static fixtures is not required.
 */
export function asFullContext(fd: FileDiff): FileDiff {
  if (fd.binary || fd.tooLarge || fd.hunks.length <= 1) return fd;
  for (let i = 1; i < fd.hunks.length; i++) {
    const prev = fd.hunks[i - 1];
    const cur = fd.hunks[i];
    if (
      cur.oldStart !== prev.oldStart + prev.oldLines ||
      cur.newStart !== prev.newStart + prev.newLines
    ) {
      return fd; // unfillable gap
    }
  }
  const first = fd.hunks[0];
  const last = fd.hunks[fd.hunks.length - 1];
  const lines = fd.hunks.flatMap((h) => h.lines);
  return {
    ...fd,
    hunks: [
      hunk(
        first.oldStart,
        last.oldStart + last.oldLines - first.oldStart,
        first.newStart,
        last.newStart + last.newLines - first.newStart,
        lines,
      ),
    ],
  };
}
