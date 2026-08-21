/** Audit §2.2 regression — an out-of-range `selectedIndex` must NOT crash the
 *  right panel (and with it the whole workspace).
 *
 *  A streaming graph refetch publishes its first partial batch (512 rows) BEFORE
 *  the progressive selection remap runs, so a selection at row >= 512 briefly
 *  indexes past the end of `graph.nodes`; a rebased/GC'd commit never comes back
 *  at all. `CommitPanel`'s `node` prop is non-optional, so TypeScript cannot see
 *  the `undefined` and the panel dereferences `node.summary` → TypeError →
 *  ErrorBoundary tears down the workspace.
 *
 *  These render inside the real ErrorBoundary: a boundary fallback means the
 *  crash reproduced. Expected behaviour is a graceful fall back to the status
 *  panel until the row arrives (or the stream ends and the selection clears). */
import { describe, it, expect, vi } from 'vitest';
import { render, screen } from '@testing-library/react';

import { ErrorBoundary } from '../ErrorBoundary';
import { WorkspaceRightPanel } from '../WorkspaceRightPanel';
import type { WorkspaceRightPanelProps } from '../WorkspaceRightPanel';
import type { GraphLayout, GraphNode, HeadInfo, StatusSnapshot } from '../../ipc';

const HEAD: HeadInfo = { branchName: 'main', oid: 'a'.repeat(40), detached: false, unborn: false };

function snapshot(): StatusSnapshot {
  return {
    staged: [{ path: 'a.ts', origPath: null, status: 'modified' }],
    unstaged: [],
    untracked: [],
    conflicted: [],
  };
}

function node(i: number): GraphNode {
  return {
    id: String(i).padStart(40, '0'),
    lane: 0,
    parents: i === 0 ? [] : [i - 1],
    summary: `commit ${i}`,
    author: 'Test User',
    ts: 1_700_000_000 + i,
    committerTs: 1_700_000_000 + i,
  };
}

/** A partial layout as published by the first streamed batch: `rows` rows only. */
function partialLayout(rows: number): GraphLayout {
  return {
    nodes: Array.from({ length: rows }, (_, i) => node(i)),
    edges: [],
    laneCount: 1,
    headIndex: 0,
    truncated: false,
  };
}

function renderPanel(over: Partial<WorkspaceRightPanelProps>) {
  const props: WorkspaceRightPanelProps = {
    rightPanelWidth: 360,
    repoId: 'r1',
    rightPaneTab: 'work',
    onSelectRightPaneTab: vi.fn(),
    prDefaultHead: 'main',
    prDefaultBase: 'main',
    prBaseOptions: [],
    prCompareOptions: [],
    prNav: null,
    opState: { kind: 'none' },
    conflicts: [],
    mutating: false,
    onCommitMerge: vi.fn(),
    onRebaseContinue: vi.fn(),
    onRebaseSkip: vi.fn(),
    onCherrypickContinue: vi.fn(),
    onRevertContinue: vi.fn(),
    onAbort: vi.fn(),
    onBisectMark: vi.fn(),
    onBisectSkip: vi.fn(),
    bisectSummaries: {},
    compare: null,
    compareData: null,
    compareLoading: false,
    compareError: null,
    headBranch: null,
    listView: 'flat',
    panelDensity: 'cozy',
    scope: { kind: 'root' },
    setScope: vi.fn(),
    clearCompare: vi.fn(),
    selectedIndex: null,
    graph: null,
    commitDiff: null,
    commitDiffLoading: false,
    commitDiffError: null,
    setCommitBrowserOpen: vi.fn(),
    onSelectParent: vi.fn(),
    setSelectedIndex: vi.fn(),
    aiEligible: false,
    runAnalyze: vi.fn(),
    status: snapshot(),
    statusLoading: false,
    statusError: null,
    diffSlot: null,
    aiRows: {},
    aiAtCapacity: false,
    aiPanelLoading: false,
    onStage: vi.fn(),
    onUnstage: vi.fn(),
    onDiscard: vi.fn(),
    onDiscardForce: vi.fn(),
    onToggleDiff: vi.fn(),
    onResolveConflict: vi.fn(),
    onToggleConflictView: vi.fn(),
    onAiResolve: vi.fn(),
    onAiReview: vi.fn(),
    onBlame: vi.fn(),
    onFileHistory: vi.fn(),
    onCreateStash: vi.fn(),
    head: HEAD,
    amend: false,
    onToggleAmend: vi.fn(),
    amendMessage: null,
    commitBoxRef: { current: null },
    onCommitAmend: vi.fn(async () => {}),
    onCommitMergeSubmit: vi.fn(async () => {}),
    onCommit: vi.fn(async () => {}),
    onCommitAndPush: vi.fn(async () => {}),
    onGenerate: vi.fn(async () => ''),
    workingDirty: true,
    onCompose: vi.fn(),
    onOpenIdentitySettings: vi.fn(),
    signingStatus: null,
    commitSignature: null,
    ...over,
  };
  return render(
    <ErrorBoundary label="right panel">
      <WorkspaceRightPanel {...props} />
    </ErrorBoundary>,
  );
}

/** True when the ErrorBoundary fallback replaced the subtree (= it crashed). */
function crashed(): boolean {
  return screen.queryByRole('button', { name: /Try again/i }) !== null;
}

describe('WorkspaceRightPanel out-of-range selectedIndex (audit §2.2)', () => {
  it('mid-stream selection past the last streamed row falls back to the status panel', () => {
    // Selection at row 600 while only the first 512-row batch has landed.
    renderPanel({ graph: partialLayout(512), selectedIndex: 600 });

    expect(crashed()).toBe(false);
    // Fell back to the working-dir status + commit box, not the commit panel.
    expect(screen.getByPlaceholderText('Commit message')).toBeInTheDocument();
    expect(screen.queryByText(/commit 600/)).not.toBeInTheDocument();
  });

  it('an empty layout with a stale selection does not crash', () => {
    renderPanel({ graph: partialLayout(0), selectedIndex: 0 });

    expect(crashed()).toBe(false);
    expect(screen.getByPlaceholderText('Commit message')).toBeInTheDocument();
  });

  it('an in-range selection still renders the commit panel', () => {
    renderPanel({ graph: partialLayout(512), selectedIndex: 3 });

    expect(crashed()).toBe(false);
    expect(screen.getByText('commit 3')).toBeInTheDocument();
  });
});
