/** T5b §5.2 — malformed-DTO tolerance for the most-rendered DTO consumers.
 *  Hostile-but-typed payloads (negative/NaN lanes, out-of-range parents,
 *  inverted edges, duplicate/empty/huge paths, out-of-bounds intraline spans,
 *  null-ish commit details, XSS-shaped ref names) must produce an error/empty/
 *  degraded render — NEVER an uncaught throw escaping the real ErrorBoundary,
 *  never a white screen. Seeded deterministic loops, no fast-check. */
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import type { MockInstance } from 'vitest';

import { ErrorBoundary } from '../ErrorBoundary';
import { GraphCanvas } from '../../graph/GraphCanvas';
import { effectiveMetrics } from '../../graph/metrics';
import type { GraphDisplayOptions } from '../../graph/rightColumns';
import { buildEdgeIndex, edgesInRange } from '../../graph/edgeIndex';
import {
  backingStoreSize,
  clampTooltipPos,
  scrollRowIntoView,
  spacerHeight,
  visibleRowRange,
} from '../../graph/viewport';
import { graphAreaRight, laneX, rowAtPoint, rowY, summaryStartX } from '../../graph/geometry';
import { StatusPanel } from '../StatusPanel';
import type { StatusPanelProps } from '../StatusPanel';
import { DiffView } from '../DiffView';
import { CommitPanel } from '../CommitPanel';
import { Sidebar } from '../Sidebar';
import type { SidebarProps } from '../Sidebar';
import type {
  BranchInfo,
  CommitDiff,
  FileDiff,
  FileStatus,
  GraphLayout,
  GraphNode,
  Hunk,
  StatusEntry,
  StatusSnapshot,
} from '../../ipc';

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

function mulberry32(seed: number): () => number {
  let a = seed >>> 0;
  return () => {
    a = (a + 0x6d2b79f5) >>> 0;
    let t = a;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

/** console.error is spied per test: React's boundary logs + key warnings are
 *  the ONLY tolerated output; anything else (an uncaught throw surfacing via
 *  jsdom, a stray app error) fails the test. */
const ALLOWED_CONSOLE = [
  /ErrorBoundary/,
  /The above error occurred/,
  /React will try to recreate/i,
  /Consider adding an error boundary/i,
  /unique "key"/,
  // React 19 duplicate-key wording. Expected degraded signal for hostile
  // duplicate-path/name DTOs (rows are keyed by path/name); render survives.
  /Encountered two children with the same key/,
  /^Warning:/,
  /Error: Uncaught \[/, // jsdom's echo of an error CAUGHT by the boundary re-throw protocol
];

let consoleSpy: MockInstance;
beforeEach(() => {
  consoleSpy = vi.spyOn(console, 'error').mockImplementation(() => {});
});
afterEach(() => {
  consoleSpy.mockRestore();
});

function assertConsoleClean(): void {
  for (const call of consoleSpy.mock.calls) {
    const first = typeof call[0] === 'string' ? call[0] : String(call[0]);
    expect(
      ALLOWED_CONSOLE.some((re) => re.test(first)),
      `unexpected console.error: ${first.slice(0, 200)}`,
    ).toBe(true);
  }
}

const noop = () => {};

// ---------------------------------------------------------------------------
// GraphLayout — hostile shapes
// ---------------------------------------------------------------------------

function gnode(over: Partial<GraphNode> = {}): GraphNode {
  return {
    id: 'a'.repeat(40),
    lane: 0,
    parents: [],
    summary: 'summary',
    author: 'Author',
    ts: 1_700_000_000,
    committerTs: 1_700_000_000,
    ...over,
  };
}

function layout(over: Partial<GraphLayout> = {}): GraphLayout {
  return { nodes: [], edges: [], laneCount: 1, headIndex: null, truncated: false, ...over };
}

const HOSTILE_LAYOUTS: [string, GraphLayout][] = [
  ['negative lane', layout({ nodes: [gnode({ lane: -1 }), gnode({ id: 'b'.repeat(40), lane: -7 })] })],
  ['NaN lane', layout({ nodes: [gnode({ lane: NaN })] })],
  ['huge lane 1e9', layout({ nodes: [gnode({ lane: 1e9 })], laneCount: 3 })],
  ['parents out of range', layout({ nodes: [gnode({ parents: [99_999] })] })],
  [
    'edge from > to',
    layout({
      nodes: [gnode(), gnode({ id: 'b'.repeat(40) }), gnode({ id: 'c'.repeat(40) })],
      edges: [{ from: 2, to: 0, lane: 0 }],
    }),
  ],
  ['laneCount 0 with nodes', layout({ nodes: [gnode(), gnode({ id: 'b'.repeat(40) })], laneCount: 0 })],
  ['0 rows but edges + headIndex', layout({ edges: [{ from: 0, to: 5, lane: 2 }], headIndex: 5 })],
];

const METRICS = effectiveMetrics({ avatarRadius: 10, rowHeight: 32, laneWidth: 16, compact: false });

function disp(): GraphDisplayOptions {
  return {
    showSha: false,
    showAuthor: false,
    showDate: false,
    dateBasis: 'author',
    showAheadBehind: false,
    branchStats: new Map(),
    showSignatureBadge: false,
    showPrBadge: false,
    showCiStatus: false,
    prByBranch: new Map(),
    ciBySha: new Map(),
  };
}

describe('GraphLayout: hostile shapes against the pure geometry modules', () => {
  it.each(HOSTILE_LAYOUTS)('%s: edge index + range queries never throw', (_name, l) => {
    let ix!: ReturnType<typeof buildEdgeIndex>;
    expect(() => {
      ix = buildEdgeIndex(l);
    }).not.toThrow();
    expect(() => edgesInRange(l, ix, 0, 100)).not.toThrow();
    expect(() => edgesInRange(l, ix, -50, -10)).not.toThrow();
    const out = edgesInRange(l, ix, 0, 1_000_000);
    for (const e of out) expect(l.edges).toContain(e);
  });

  it('seeded loop: viewport/geometry math stays well-formed for hostile numbers', () => {
    const rand = mulberry32(0x9a9a);
    const hostile = [-1, 0, 1, 7, NaN, 1e9, -1e6, 2 ** 31, 0.5];
    const pickN = () => hostile[Math.floor(rand() * hostile.length)];
    for (let i = 0; i < 100; i++) {
      const nodeCount = pickN();
      const scrollTop = pickN();
      // NaN inputs propagate NaN (documented here as the observed degradation:
      // scrollTop/nodeCount come from the DOM scroller + layout length and are
      // never NaN in practice) — well-formedness is pinned for FINITE inputs.
      const r = visibleRowRange(scrollTop, pickN() > 0 ? 1 : 0, 32, 600, nodeCount, 4);
      if (Number.isFinite(scrollTop)) {
        expect(r.firstRow).toBeGreaterThanOrEqual(0);
        if (Number.isFinite(nodeCount)) expect(r.lastRow).toBeLessThanOrEqual(Math.max(nodeCount - 1, -1));
      }
      expect(() => scrollRowIntoView(pickN(), 0, 32, pickN(), 600)).not.toThrow();
      expect(() =>
        clampTooltipPos({ left: pickN(), top: pickN(), width: 10, height: 10 }, 100, 40, 800, 600),
      ).not.toThrow();
      const bs = backingStoreSize(pickN(), pickN(), 1.5);
      if (!Number.isNaN(bs.width)) expect(bs.width).toBeGreaterThanOrEqual(1);
      expect(() => spacerHeight(pickN(), 0, 32)).not.toThrow();
      // Geometry: finite lanes give finite pixels; hostile lanes never throw.
      const lane = pickN();
      expect(() => laneX(lane, METRICS)).not.toThrow();
      if (Number.isFinite(lane)) expect(Number.isFinite(laneX(lane, METRICS))).toBe(true);
      expect(() => graphAreaRight(lane, METRICS)).not.toThrow();
      expect(() => summaryStartX(lane, METRICS)).not.toThrow();
      expect(() => rowY(lane, 0, METRICS)).not.toThrow();
      expect(() => rowAtPoint(pickN(), pickN(), METRICS)).not.toThrow();
    }
  });

  it.each(HOSTILE_LAYOUTS)(
    '%s: GraphCanvas mounts inside the real ErrorBoundary without escaping it',
    (_name, l) => {
      const { container, unmount } = render(
        <ErrorBoundary label="graph">
          <GraphCanvas
            layout={l}
            selectedIndex={null}
            onSelect={noop}
            wip={null}
            themeVersion={0}
            metrics={METRICS}
            metricsVersion={0}
            display={disp()}
          />
        </ErrorBoundary>,
      );
      // Degraded-render contract: either the canvas mounted (graceful clamp)
      // or the boundary fallback replaced it — never a white screen / uncaught.
      const canvas = container.querySelector('canvas');
      const fallback = container.querySelector('.error-boundary');
      expect(canvas !== null || fallback !== null).toBe(true);
      assertConsoleClean();
      unmount();
    },
  );
});

// ---------------------------------------------------------------------------
// StatusSnapshot — StatusPanel
// ---------------------------------------------------------------------------

function sentry(path: string, status: StatusEntry['status'] = 'modified'): StatusEntry {
  return { path, origPath: null, status };
}

function statusProps(snapshot: StatusSnapshot, over: Partial<StatusPanelProps> = {}): StatusPanelProps {
  return {
    snapshot,
    loading: false,
    error: null,
    busy: false,
    diffSlot: null,
    listView: 'flat',
    conflicts: [],
    aiEligible: false,
    aiRows: {},
    aiAtCapacity: false,
    aiAnalyzing: false,
    onStage: noop,
    onUnstage: noop,
    onDiscard: noop,
    onDiscardForce: noop,
    onReviewStaged: noop,
    onReviewWorktree: noop,
    onToggleDiff: noop,
    onResolveConflict: noop,
    onToggleConflictView: noop,
    onAiResolve: noop,
    onAiReview: noop,
    onBlame: noop,
    onFileHistory: noop,
    ...over,
  };
}

describe('StatusSnapshot: hostile entries in StatusPanel', () => {
  const hostileSnapshot: StatusSnapshot = {
    staged: [sentry('dup.ts'), sentry('dup.ts'), sentry('')],
    unstaged: [sentry('dup.ts'), sentry('x/'.repeat(2_500) + 'leaf.ts')],
    untracked: [sentry('weird.ts', 'not-a-real-status' as FileStatus)],
    conflicted: [],
  };

  it('duplicate paths, empty path, 5000-char path, unknown status: renders sections, no crash', () => {
    const { container } = render(
      <ErrorBoundary label="status">
        <StatusPanel {...statusProps(hostileSnapshot)} />
      </ErrorBoundary>,
    );
    expect(container.querySelector('.error-boundary')).toBeNull();
    expect(screen.getByText('Staged (3)')).toBeInTheDocument();
    expect(screen.getByText('Changes (3)')).toBeInTheDocument();
    assertConsoleClean();
  });

  it('tree view over garbage paths (empty string, trailing slashes) does not throw', () => {
    const { container } = render(
      <ErrorBoundary label="status">
        <StatusPanel {...statusProps(hostileSnapshot, { listView: 'tree' })} />
      </ErrorBoundary>,
    );
    expect(container.querySelector('.error-boundary')).toBeNull();
    assertConsoleClean();
  });
});

// ---------------------------------------------------------------------------
// FileDiff / hunks — DiffView
// ---------------------------------------------------------------------------

function fileDiff(hunks: Hunk[]): FileDiff {
  return { path: 'notes.txt', origPath: null, status: 'modified', binary: false, tooLarge: false, hunks };
}

describe('FileDiff: hostile hunks in DiffView', () => {
  it('out-of-bounds / overlapping / negative / NaN spans render the full content verbatim', () => {
    const hunk: Hunk = {
      oldStart: 1,
      oldLines: 2,
      newStart: 1,
      newLines: 2,
      lines: [
        { kind: 'del', oldNo: 1, newNo: null, content: 'delta line', spans: [[-5, 999]] },
        { kind: 'add', oldNo: null, newNo: 1, content: 'added line', spans: [[3, 4], [2, 5], [NaN, NaN], [7, -2]] },
      ],
    };
    const { container } = render(
      <ErrorBoundary label="diff">
        <DiffView diff={fileDiff([hunk])} />
      </ErrorBoundary>,
    );
    expect(container.querySelector('.error-boundary')).toBeNull();
    const contents = Array.from(container.querySelectorAll('.diff-content')).map(
      (el) => el.textContent ?? '',
    );
    expect(contents).toEqual(['delta line', 'added line']);
    assertConsoleClean();
  });

  it('hunk with 0 lines + hunks out of order + negative line numbers: no crash', () => {
    const empty: Hunk = { oldStart: 50, oldLines: 0, newStart: 50, newLines: 0, lines: [] };
    const late: Hunk = {
      oldStart: 90,
      oldLines: 1,
      newStart: 90,
      newLines: 1,
      lines: [{ kind: 'context', oldNo: 90, newNo: 90, content: 'late' }],
    };
    const early: Hunk = {
      oldStart: -3,
      oldLines: NaN,
      newStart: 1,
      newLines: 1,
      lines: [{ kind: 'add', oldNo: null, newNo: -1, content: 'early' }],
    };
    const { container } = render(
      <ErrorBoundary label="diff">
        <DiffView diff={fileDiff([late, empty, early])} />
      </ErrorBoundary>,
    );
    expect(container.querySelector('.error-boundary')).toBeNull();
    expect(screen.getByText('late')).toBeInTheDocument();
    expect(screen.getByText('early')).toBeInTheDocument();
    assertConsoleClean();
  });
});

// ---------------------------------------------------------------------------
// CommitDetails — CommitPanel
// ---------------------------------------------------------------------------

describe('CommitDetails: null-ish / hostile fields in CommitPanel', () => {
  function renderPanel(data: CommitDiff | null, node: GraphNode) {
    return render(
      <ErrorBoundary label="commit">
        <CommitPanel
          node={node}
          data={data}
          loading={false}
          error={null}
          listView="flat"
          scope={{ kind: 'root' }}
          onSelectScope={noop}
          onSelectParent={noop}
          onClose={noop}
          aiEligible={false}
          onExplain={noop}
          signature={null}
        />
      </ErrorBoundary>,
    );
  }

  it('10k-char summary, NUL + RTL-override message, null author, NaN timestamps: degraded render', () => {
    const summary = 'S'.repeat(10_000);
    const data: CommitDiff = {
      details: {
        oid: 'd'.repeat(40),
        summary,
        message: `${summary}\n\nbody   with NUL and ‮rtl override‬ end`,
        authorName: null as unknown as string,
        authorEmail: null as unknown as string,
        authorTs: NaN,
        committerTs: NaN,
        parents: ['e'.repeat(40), 'f'.repeat(40), '1'.repeat(40)],
      },
      files: [],
    };
    const node = gnode({ id: 'd'.repeat(40), parents: [99_999] });
    const { container } = renderPanel(data, node);
    expect(container.querySelector('.error-boundary')).toBeNull();
    expect(screen.getByTestId('commit-details')).toBeInTheDocument();
    // The hostile summary renders as TEXT (truncated visually by CSS, not JS).
    expect(container.querySelector('.commit-summary')?.textContent).toBe(summary);
    assertConsoleClean();
  });

  it('parents beyond the node layout render as plain text, not jump buttons', () => {
    const data: CommitDiff = {
      details: {
        oid: 'd'.repeat(40),
        summary: 's',
        message: 's',
        authorName: 'A',
        authorEmail: 'a@x',
        authorTs: 1,
        committerTs: 1,
        parents: ['e'.repeat(40), 'f'.repeat(40)],
      },
      files: [],
    };
    // node.parents has only ONE entry -> the second parent has no layout row.
    const { container } = renderPanel(data, gnode({ id: 'd'.repeat(40), parents: [0] }));
    expect(container.querySelectorAll('.commit-parent-link')).toHaveLength(1);
    expect(container.querySelectorAll('.commit-parent-plain')).toHaveLength(1);
    assertConsoleClean();
  });
});

// ---------------------------------------------------------------------------
// Refs / branch list — Sidebar
// ---------------------------------------------------------------------------

describe('branch list: 1000 branches, XSS-shaped + duplicate names in Sidebar', () => {
  const XSS = '<script>alert(1)</script>';

  function branch(name: string, over: Partial<BranchInfo> = {}): BranchInfo {
    return { name, isHead: false, upstream: null, ahead: null, behind: null, tip: 'a'.repeat(40), ...over };
  }

  function sidebarProps(local: BranchInfo[]): SidebarProps {
    return {
      data: {
        local,
        remote: [],
        tags: [],
        head: { branchName: local[0]?.name ?? null, oid: 'c'.repeat(40), detached: false, unborn: false },
      },
      loading: false,
      error: null,
      onDismissError: noop,
      busy: false,
      opActive: false,
      currentBranch: local[0]?.name ?? null,
      onCheckout: noop,
      onContextMenu: noop,
      onCreateBranch: async () => {},
      width: 240,
      listView: 'flat',
      stashes: [],
      onCreateStash: noop,
      onStashContextMenu: noop,
      submodules: [],
      onSubmoduleContextMenu: noop,
      submoduleBusy: null,
      onNewSubmodule: noop,
      worktrees: [],
      onWorktreeContextMenu: noop,
      onNewWorktree: noop,
      onTagContextMenu: noop,
      tagSyncReport: null,
      tagSyncState: 'idle',
      tagSyncRemote: null,
      tagSyncCheckedAt: null,
      onTagsExpand: noop,
      remotes: [],
      onRemoteContextMenu: noop,
      onAddRemote: noop,
    };
  }

  it('renders without crashing; script-tag name stays inert text; duplicates tolerated', () => {
    const local = [
      branch('main', { isHead: true }),
      branch(XSS),
      branch('dup'),
      branch('dup'),
      ...Array.from({ length: 1_000 }, (_, i) => branch(`feature/branch-${i}`)),
    ];
    const { container } = render(
      <ErrorBoundary label="sidebar">
        <Sidebar {...sidebarProps(local)} />
      </ErrorBoundary>,
    );
    expect(container.querySelector('.error-boundary')).toBeNull();
    // XSS check: the name must appear as TEXT and never become a live element.
    expect(container.querySelector('script')).toBeNull();
    expect(screen.getByText(XSS)).toBeInTheDocument();
    expect(screen.getAllByText('dup').length).toBeGreaterThanOrEqual(2);
    expect(screen.getByText('feature/branch-999')).toBeInTheDocument();
    assertConsoleClean();
  });
});
