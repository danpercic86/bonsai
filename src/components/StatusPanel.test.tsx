/** T3.5 — StatusPanel: staged/changes/conflict list rendering, stage/unstage/
 *  discard callback wiring (incl. rename both-path expansion), conflict-row
 *  actions + AI gating, error-banner dismiss lifecycle, and empty/loading
 *  states. Pure presentational component — no IPC. */
import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent, within } from '@testing-library/react';
import { StatusPanel } from './StatusPanel';
import type { StatusPanelProps } from './StatusPanel';
import type { StatusEntry, StatusSnapshot } from '../ipc';

function entry(path: string, status: StatusEntry['status'] = 'modified', origPath: string | null = null): StatusEntry {
  return { path, origPath, status };
}

function snap(over: Partial<StatusSnapshot> = {}): StatusSnapshot {
  return { staged: [], unstaged: [], untracked: [], conflicted: [], ...over };
}

function renderPanel(over: Partial<StatusPanelProps> = {}) {
  const props: StatusPanelProps = {
    snapshot: snap({
      staged: [entry('src/a.ts', 'added')],
      unstaged: [entry('src/b.ts')],
      untracked: [entry('notes.md', 'untracked')],
    }),
    loading: false,
    error: null,
    busy: false,
    diffSlot: null,
    listView: 'flat',
    conflicts: [],
    aiEligible: false,
    aiRows: {},
    aiAtCapacity: false,
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
    ...over,
  };
  return { ...render(<StatusPanel {...props} />), props };
}

describe('StatusPanel', () => {
  it('renders Staged and merged Changes (unstaged + untracked) with counts', () => {
    renderPanel();
    expect(screen.getByText('Staged (1)')).toBeInTheDocument();
    expect(screen.getByText('Changes (2)')).toBeInTheDocument();
    expect(screen.getByText('a.ts')).toBeInTheDocument();
    expect(screen.getByText('b.ts')).toBeInTheDocument();
    expect(screen.getByText('notes.md')).toBeInTheDocument();
  });

  it('empty snapshot shows "No changes"; null snapshot + loading shows skeletons', () => {
    renderPanel({ snapshot: snap() });
    expect(screen.getByText('No changes')).toBeInTheDocument();
    const { container } = renderPanel({ snapshot: null, loading: true });
    expect(container.querySelector('.skeleton-group')).toBeInTheDocument();
  });

  it('row + / − buttons call onStage / onUnstage with that row path', () => {
    const { props } = renderPanel();
    fireEvent.click(screen.getByRole('button', { name: 'Stage src/b.ts' }));
    expect(props.onStage).toHaveBeenCalledWith(['src/b.ts']);
    fireEvent.click(screen.getByRole('button', { name: 'Unstage src/a.ts' }));
    expect(props.onUnstage).toHaveBeenCalledWith(['src/a.ts']);
  });

  it('rename rows expand to BOTH sides of the rename', () => {
    const { props } = renderPanel({
      snapshot: snap({ staged: [entry('new.ts', 'renamed', 'old.ts')] }),
    });
    fireEvent.click(screen.getByRole('button', { name: 'Unstage new.ts' }));
    expect(props.onUnstage).toHaveBeenCalledWith(['old.ts', 'new.ts']);
  });

  it('"Stage all" / "Unstage all" pass every section path (renames expanded)', () => {
    const { props } = renderPanel({
      snapshot: snap({
        staged: [entry('s1.ts'), entry('s2.ts', 'renamed', 's2-old.ts')],
        unstaged: [entry('u1.ts')],
        untracked: [entry('u2.ts', 'untracked')],
      }),
    });
    fireEvent.click(screen.getByRole('button', { name: 'Stage all' }));
    expect(props.onStage).toHaveBeenCalledWith(['u1.ts', 'u2.ts']);
    fireEvent.click(screen.getByRole('button', { name: 'Unstage all' }));
    expect(props.onUnstage).toHaveBeenCalledWith(['s1.ts', 's2-old.ts', 's2.ts']);
  });

  it('discard: offered on tracked (unstaged) rows only; Discard all covers both', () => {
    const { props } = renderPanel();
    // tracked unstaged row has a discard control
    fireEvent.click(screen.getByRole('button', { name: 'Discard changes to src/b.ts' }));
    expect(props.onDiscard).toHaveBeenCalledWith(['src/b.ts']);
    // untracked row has none
    expect(
      screen.queryByRole('button', { name: 'Discard changes to notes.md' }),
    ).not.toBeInTheDocument();
    // staged section rows have none either
    expect(
      screen.queryByRole('button', { name: 'Discard changes to src/a.ts' }),
    ).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'Discard all' }));
    expect(props.onDiscardForce).toHaveBeenCalledWith(['src/b.ts', 'notes.md']);
  });

  it('delete: offered on new (untracked) rows only, wired to force-discard', () => {
    const { props } = renderPanel();
    // untracked row deletes just itself (nothing to revert to)
    fireEvent.click(screen.getByRole('button', { name: 'Delete notes.md' }));
    expect(props.onDiscardForce).toHaveBeenCalledWith(['notes.md']);
    // tracked unstaged + staged rows get discard/nothing, never delete
    expect(screen.queryByRole('button', { name: 'Delete src/b.ts' })).not.toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'Delete src/a.ts' })).not.toBeInTheDocument();
  });

  it('row actions put the destructive control before stage/unstage', () => {
    renderPanel();
    const names = (path: string) => {
      const row = screen.getByRole('button', { name: `Stage ${path}` }).closest('li');
      return [...(row as HTMLElement).querySelectorAll('.row-action')].map((b) =>
        b.getAttribute('aria-label'),
      );
    };
    // tracked: … discard, then stage
    expect(names('src/b.ts')).toEqual([
      'Show history of src/b.ts',
      'Blame src/b.ts',
      'Discard changes to src/b.ts',
      'Stage src/b.ts',
    ]);
    // untracked: delete, then stage
    expect(names('notes.md')).toEqual(['Delete notes.md', 'Stage notes.md']);
  });

  it('tree view: folder actions use the same order as the file rows', () => {
    renderPanel({ listView: 'tree' });
    const changes = document.querySelector('.status-section--changes') as HTMLElement;
    const dirActions = [...changes.querySelectorAll('.tree-dir-actions .row-action')].map((b) =>
      b.getAttribute('aria-label'),
    );
    expect(dirActions).toEqual([
      'Discard all changes in this folder',
      'Stage all files in this folder',
    ]);
  });

  it('busy disables the action buttons', () => {
    renderPanel({ busy: true });
    expect(screen.getByRole('button', { name: 'Stage src/b.ts' })).toBeDisabled();
    expect(screen.getByRole('button', { name: 'Stage all' })).toBeDisabled();
    expect(screen.getByRole('button', { name: 'Discard all' })).toBeDisabled();
    expect(screen.getByRole('button', { name: 'Delete notes.md' })).toBeDisabled();
  });

  it('blame/history controls appear on tracked rows only', () => {
    const { props } = renderPanel();
    fireEvent.click(screen.getByRole('button', { name: 'Blame src/b.ts' }));
    expect(props.onBlame).toHaveBeenCalledWith('src/b.ts');
    fireEvent.click(screen.getByRole('button', { name: 'Show history of src/a.ts' }));
    expect(props.onFileHistory).toHaveBeenCalledWith('src/a.ts');
    expect(screen.queryByRole('button', { name: 'Blame notes.md' })).not.toBeInTheDocument();
  });

  it('clicking a row toggles its diff with the resolved ORIGIN section', () => {
    const { props } = renderPanel();
    fireEvent.click(screen.getByText('notes.md').closest('button')!);
    expect(props.onToggleDiff).toHaveBeenCalledWith(
      'untracked',
      expect.objectContaining({ path: 'notes.md' }),
    );
    fireEvent.click(screen.getByText('b.ts').closest('button')!);
    expect(props.onToggleDiff).toHaveBeenCalledWith(
      'unstaged',
      expect.objectContaining({ path: 'src/b.ts' }),
    );
  });

  it('diffSlot key marks the matching row expanded', () => {
    renderPanel({
      diffSlot: { key: 'unstaged:src/b.ts', state: 'ready', diff: null, error: null },
    });
    expect(screen.getByText('b.ts').closest('li')).toHaveClass('file-row-expanded');
    expect(screen.getByText('a.ts').closest('li')).not.toHaveClass('file-row-expanded');
  });

  it('conflict rows: kind badge, ours/theirs/resolved wiring, danger styling', () => {
    const { props, container } = renderPanel({
      snapshot: snap({ conflicted: [entry('clash.ts', 'conflicted')] }),
      conflicts: [
        { path: 'clash.ts', kind: 'bothModified', hasBase: true, hasOurs: true, hasTheirs: true },
      ],
    });
    expect(screen.getByText('Conflicts (1)')).toBeInTheDocument();
    expect(screen.getByText('both modified')).toBeInTheDocument();
    expect(container.querySelector('.file-status-conflicted')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'Take our version of clash.ts' }));
    expect(props.onResolveConflict).toHaveBeenCalledWith('clash.ts', 'ours');
    fireEvent.click(screen.getByRole('button', { name: 'Take their version of clash.ts' }));
    expect(props.onResolveConflict).toHaveBeenCalledWith('clash.ts', 'theirs');
    fireEvent.click(screen.getByRole('button', { name: 'Mark clash.ts resolved' }));
    expect(props.onResolveConflict).toHaveBeenCalledWith('clash.ts', 'markResolved');
  });

  it('AI resolve: shown only for text-mergeable kinds; disabled unless eligible', () => {
    const conflicted = [entry('a.ts', 'conflicted'), entry('gone.ts', 'conflicted')];
    const conflicts = [
      { path: 'a.ts', kind: 'bothModified', hasBase: true, hasOurs: true, hasTheirs: true },
      { path: 'gone.ts', kind: 'deletedByUs', hasBase: true, hasOurs: false, hasTheirs: true },
    ] as StatusPanelProps['conflicts'];
    const { props } = renderPanel({ snapshot: snap({ conflicted }), conflicts, aiEligible: true });
    expect(screen.queryByRole('button', { name: 'Resolve gone.ts with AI' })).not.toBeInTheDocument();
    const btn = screen.getByRole('button', { name: 'Resolve a.ts with AI' });
    expect(btn).toBeEnabled();
    fireEvent.click(btn);
    expect(props.onAiResolve).toHaveBeenCalledWith('a.ts');
    // Not eligible -> button rendered but disabled.
    renderPanel({
      snapshot: snap({ conflicted: [entry('b.ts', 'conflicted')] }),
      conflicts: [
        { path: 'b.ts', kind: 'bothAdded', hasBase: false, hasOurs: true, hasTheirs: true },
      ],
      aiEligible: false,
    });
    expect(screen.getByRole('button', { name: 'Resolve b.ts with AI' })).toBeDisabled();
  });

  /**
   * P68d — THE ITEM-5 REGRESSION GUARD (part a), inverted from the test that used to
   * live here.
   *
   * Until P68d this test asserted `expect(y.ts button).toBeDisabled()`, i.e. it
   * ENCODED the reported bug: one `aiResolvingPath` scalar plus
   * `aiDisabled={aiResolvingPath !== null}` froze every conflict row during any
   * single run. A run on x.ts must now leave y.ts fully clickable.
   */
  it('a run on one path shows its elapsed timer and does NOT disable other rows', () => {
    const conflicted = [entry('x.ts', 'conflicted'), entry('y.ts', 'conflicted')];
    const conflicts = [
      { path: 'x.ts', kind: 'bothModified', hasBase: true, hasOurs: true, hasTheirs: true },
      { path: 'y.ts', kind: 'bothModified', hasBase: true, hasOurs: true, hasTheirs: true },
    ] as StatusPanelProps['conflicts'];
    renderPanel({
      snapshot: snap({ conflicted }),
      conflicts,
      aiEligible: true,
      aiRows: {
        'x.ts': { status: 'running' as const, elapsedSecs: 4, key: 'conflict:x.ts', error: null },
      },
    });
    const xBtn = screen.getByRole('button', { name: 'Resolve x.ts with AI' });
    expect(xBtn).toHaveTextContent('…4s');
    expect(screen.getByRole('button', { name: 'Resolve y.ts with AI' })).toBeEnabled();
  });

  it('the concurrency cap — and only the cap — disables an idle row', () => {
    const conflicted = [entry('x.ts', 'conflicted')];
    const conflicts = [
      { path: 'x.ts', kind: 'bothModified', hasBase: true, hasOurs: true, hasTheirs: true },
    ] as StatusPanelProps['conflicts'];
    renderPanel({
      snapshot: snap({ conflicted }),
      conflicts,
      aiEligible: true,
      aiRows: {},
      aiAtCapacity: true,
    });
    expect(screen.getByRole('button', { name: 'Resolve x.ts with AI' })).toBeDisabled();
  });

  it('a ready proposal offers ✓ review and calls onAiReview, never a new run', () => {
    const conflicted = [entry('x.ts', 'conflicted')];
    const conflicts = [
      { path: 'x.ts', kind: 'bothModified', hasBase: true, hasOurs: true, hasTheirs: true },
    ] as StatusPanelProps['conflicts'];
    const { props } = renderPanel({
      snapshot: snap({ conflicted }),
      conflicts,
      aiEligible: true,
      // At capacity on purpose: a finished proposal must stay re-openable even when
      // no new run could start — re-opening costs nothing.
      aiAtCapacity: true,
      aiRows: {
        'x.ts': { status: 'ready' as const, elapsedSecs: 9, key: 'conflict:x.ts', error: null },
      },
    });
    const btn = screen.getByRole('button', { name: 'Resolve x.ts with AI' });
    expect(btn).toHaveTextContent('✓ review');
    expect(btn).toBeEnabled();
    fireEvent.click(btn);
    expect(props.onAiReview).toHaveBeenCalledWith('x.ts');
    expect(props.onAiResolve).not.toHaveBeenCalled();
  });

  it('failed shows ⚠ with the error in the title and retries on click', () => {
    const conflicted = [entry('x.ts', 'conflicted')];
    const conflicts = [
      { path: 'x.ts', kind: 'bothModified', hasBase: true, hasOurs: true, hasTheirs: true },
    ] as StatusPanelProps['conflicts'];
    const { props } = renderPanel({
      snapshot: snap({ conflicted }),
      conflicts,
      aiEligible: true,
      aiRows: {
        'x.ts': {
          status: 'failed' as const,
          elapsedSecs: 3,
          key: 'conflict:x.ts',
          error: 'Claude exited without a result',
        },
      },
    });
    const btn = screen.getByRole('button', { name: 'Resolve x.ts with AI' });
    expect(btn).toHaveTextContent('⚠');
    expect(btn.getAttribute('title')).toContain('Claude exited without a result');
    fireEvent.click(btn);
    expect(props.onAiResolve).toHaveBeenCalledWith('x.ts');
  });

  it('awaitingInput shows ? and reveals the dock when a reveal handler exists', () => {
    const conflicted = [entry('x.ts', 'conflicted')];
    const conflicts = [
      { path: 'x.ts', kind: 'bothModified', hasBase: true, hasOurs: true, hasTheirs: true },
    ] as StatusPanelProps['conflicts'];
    const onAiReveal = vi.fn();
    const { props } = renderPanel({
      snapshot: snap({ conflicted }),
      conflicts,
      aiEligible: true,
      onAiReveal,
      aiRows: {
        'x.ts': {
          status: 'awaitingInput' as const,
          elapsedSecs: 12,
          key: 'conflict:x.ts',
          error: null,
        },
      },
    });
    const btn = screen.getByRole('button', { name: 'Resolve x.ts with AI' });
    expect(btn).toHaveTextContent('?');
    fireEvent.click(btn);
    expect(onAiReveal).toHaveBeenCalledWith('x.ts');
    expect(props.onAiResolve).not.toHaveBeenCalled();
  });

  it('AI affordances stay hidden for kinds the editor cannot merge (aiShown gate)', () => {
    const conflicted = [entry('gone.md', 'conflicted')];
    const conflicts = [
      { path: 'gone.md', kind: 'deletedByThem', hasBase: true, hasOurs: true, hasTheirs: false },
    ] as StatusPanelProps['conflicts'];
    renderPanel({
      snapshot: snap({ conflicted }),
      conflicts,
      aiEligible: true,
      aiRows: {
        'gone.md': {
          status: 'ready' as const,
          elapsedSecs: 1,
          key: 'conflict:gone.md',
          error: null,
        },
      },
    });
    expect(screen.queryByRole('button', { name: 'Resolve gone.md with AI' })).toBeNull();
  });

  it('P80 E1: no ✨ Review buttons in the section headers (moved to the commit ⋯ menu)', () => {
    renderPanel({ aiEligible: true });
    expect(screen.queryByRole('button', { name: '✨ Review' })).toBeNull();
    expect(screen.queryByRole('button', { name: '✨ Reviewing…' })).toBeNull();
  });

  it('error banner: dismiss hides it; a NEW error id re-surfaces the banner', () => {
    const { rerender } = render(
      <StatusPanelHarness error={{ id: 1, message: 'stage failed' }} />,
    );
    const banner = screen.getByRole('alert');
    expect(banner).toHaveTextContent('stage failed');
    fireEvent.click(within(banner).getByRole('button', { name: 'Dismiss error' }));
    expect(screen.queryByRole('alert')).not.toBeInTheDocument();
    // Same id stays dismissed; a new id (same text) re-surfaces.
    rerender(<StatusPanelHarness error={{ id: 1, message: 'stage failed' }} />);
    expect(screen.queryByRole('alert')).not.toBeInTheDocument();
    rerender(<StatusPanelHarness error={{ id: 2, message: 'stage failed' }} />);
    expect(screen.getByRole('alert')).toBeInTheDocument();
  });
});

/** Minimal stable-prop harness for the error-id lifecycle test. */
function StatusPanelHarness({ error }: { error: { id: number; message: string } | null }) {
  const noop = () => {};
  return (
    <StatusPanel
      snapshot={snap({ staged: [entry('src/a.ts', 'added')] })}
      loading={false}
      error={error}
      busy={false}
      diffSlot={null}
      listView="flat"
      conflicts={[]}
      aiEligible={false}
      aiRows={{}}
      aiAtCapacity={false}
      onStage={noop}
      onUnstage={noop}
      onDiscard={noop}
      onDiscardForce={noop}
      onToggleDiff={noop}
      onResolveConflict={noop}
      onToggleConflictView={noop}
      onAiResolve={noop}
      onAiReview={noop}
      onBlame={noop}
      onFileHistory={noop}
    />
  );
}
