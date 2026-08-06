// Split out of the former monolithic mock.ts (pure refactor; no behavior change).
import type { BlameLine, FileHistoryEntry } from '../types';

// P23d §10.2: blame/file-history fixtures. The oids mirror fixtures/graph.ts
// `oid(row)` (row hex, 2 digits, repeated 20×) so reveal-in-graph resolves to a
// real node in the default mock layout. Authors mirror that fixture's `author`
// (even rows Ada, odd rows Grace). The keyed paths are REAL status rows
// (`src/main.rs` shows in both Staged + Changes; `README.md` in Changes) so the
// row-action buttons produce populated views; every other path → git error / [].
export const BLAME_FIXTURE_PATHS = new Set(['src/main.rs', 'README.md']);

function mockNodeOid(row: number): string {
  return row.toString(16).padStart(2, '0').repeat(20);
}

const BLAME_NOW = Math.floor(Date.now() / 1000);

export const MOCK_BLAME: BlameLine[] = (() => {
  // (row, author, email, summary, lineText) per source line, grouped by commit
  // so consecutive same-oid lines collapse in the gutter (GitHub-blame look).
  const rows: Array<[number, string, string, string, string]> = [
    [1, 'Grace Hopper', 'grace@example.com', 'feat: polish', "import { render } from './render';"],
    [1, 'Grace Hopper', 'grace@example.com', 'feat: polish', ''],
    [5, 'Grace Hopper', 'grace@example.com', 'core work 3', 'export function main() {'],
    [5, 'Grace Hopper', 'grace@example.com', 'core work 3', '  const app = createApp();'],
    [0, 'Ada Lovelace', 'ada@example.com', 'Merge feat and exp', '  app.mount("#root");'],
    [0, 'Ada Lovelace', 'ada@example.com', 'Merge feat and exp', '  return app;'],
    [5, 'Grace Hopper', 'grace@example.com', 'core work 3', '}'],
  ];
  return rows.map(([row, name, email, summary, text], i) => ({
    oid: mockNodeOid(row),
    authorName: name,
    authorEmail: email,
    authorTs: BLAME_NOW - row * 3600,
    summary,
    origLineNo: i + 1,
    finalLineNo: i + 1,
    lineText: text,
  }));
})();

export const MOCK_FILE_HISTORY: FileHistoryEntry[] = [
  { row: 0, name: 'Ada Lovelace', email: 'ada@example.com', summary: 'Merge feat and exp' },
  { row: 1, name: 'Grace Hopper', email: 'grace@example.com', summary: 'feat: polish' },
  { row: 5, name: 'Grace Hopper', email: 'grace@example.com', summary: 'core work 3' },
  { row: 8, name: 'Ada Lovelace', email: 'ada@example.com', summary: 'chore: history 19' },
].map(({ row, name, email, summary }) => ({
  oid: mockNodeOid(row),
  summary,
  authorName: name,
  authorEmail: email,
  authorTs: BLAME_NOW - row * 3600,
}));
