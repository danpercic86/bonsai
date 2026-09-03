/** P109 AC5/AC6 — the badge's accessible name folds into its wrapper's name.
 *
 *  The badge label is placed on the LEAF, not the row, so accessible-name-from-
 *  content substitutes the word for the glyph in whatever wraps it — the row
 *  keeps its path, rename arrow and ±counts for free. These assertions read the
 *  COMPUTED accessible name (not textContent), which is the only way to observe
 *  the substitution; the visible letter is asserted separately in
 *  `FileStatusBadge.test.tsx`.
 *
 *  Covers the four interactive wrappers named in the contract: S1 status row,
 *  S3 conflict row, S4 diff tree row, S6 PR changed-file row.
 *
 *  WHITESPACE: `dom-accessibility-api` concatenates adjacent inline name parts
 *  with no separator, so jsdom computes `Untrackednotes/todo.txt` where a real
 *  browser computes `Untracked notes/todo.txt`. This is pre-existing and
 *  unrelated to P109 -- the same markup computed `Unotes/todo.txt` before the
 *  badge was named. The patterns below use `\s*` so they hold in BOTH
 *  environments rather than pinning the jsdom quirk. */
import { describe, it, expect, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import { DiffFileTree } from './DiffFileTree';
import { PrFileRow } from './prPanel/PrFileRow';
import { StatusPanel } from './StatusPanel';
import type { StatusPanelProps } from './StatusPanel';
import type { FileDiffHeader, StatusEntry, StatusSnapshot } from '../ipc';

function entry(path: string, status: StatusEntry['status']): StatusEntry {
  return { path, origPath: null, status };
}

function snap(over: Partial<StatusSnapshot> = {}): StatusSnapshot {
  return { staged: [], unstaged: [], untracked: [], conflicted: [], ...over };
}

function renderPanel(over: Partial<StatusPanelProps> = {}) {
  const props: StatusPanelProps = {
    snapshot: snap({
      staged: [entry('src/app.rs', 'added')],
      untracked: [entry('notes/todo.txt', 'untracked')],
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
  return render(<StatusPanel {...props} />);
}

function header(path: string, status: FileDiffHeader['status']): FileDiffHeader {
  return { path, origPath: null, status, additions: 4, deletions: 0, binary: false };
}

describe('status badge accessible names fold into the row name', () => {
  it('S1: an untracked status row reads "Untracked {path}", never a bare letter', () => {
    renderPanel();
    expect(
      screen.getByRole('button', { name: /^Untracked\s*notes\/todo\.txt$/, expanded: false }),
    ).toBeInTheDocument();
    // The old mapping would have left a bare 'A'; the rejected hidden-sibling
    // mechanism would have produced "Untracked U …". Neither may reappear.
    expect(screen.queryByRole('button', { name: /^A\s*notes\/todo\.txt/ })).toBeNull();
    expect(screen.queryByRole('button', { name: /Untracked\s*U/ })).toBeNull();
  });

  it('S1/AC6: added and untracked rows are distinguishable by name, not only by hue', () => {
    renderPanel();
    const added = screen.getByRole('button', {
      name: /^Added\s*src\/app\.rs$/,
      expanded: false,
    });
    const untracked = screen.getByRole('button', {
      name: /^Untracked\s*notes\/todo\.txt$/,
      expanded: false,
    });
    expect(added).not.toBe(untracked);
  });

  it('S3: a conflict row reads "Conflicted {path} {kind}"', () => {
    renderPanel({
      snapshot: snap({ conflicted: [entry('src/auth.ts', 'conflicted')] }),
      conflicts: [
        { path: 'src/auth.ts', kind: 'bothModified', hasBase: true, hasOurs: true, hasTheirs: true },
      ],
    });
    expect(
      screen.getByRole('button', { name: /^Conflicted\s*src\/auth\.ts\s*both modified$/ }),
    ).toBeInTheDocument();
  });

  it('S4: a diff-tree row reads "{Status} {path} …"', () => {
    render(
      <DiffFileTree
        files={[header('src/app.rs', 'modified'), header('notes/todo.txt', 'untracked')]}
        listView="flat"
        scope={{ kind: 'root' }}
        onSelect={vi.fn()}
      />,
    );
    expect(
      screen.getByRole('button', { name: /^Modified\s*src\/app\.rs/ }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole('button', { name: /^Untracked\s*notes\/todo\.txt/ }),
    ).toBeInTheDocument();
  });

  it('S6: a PR changed-file row reads "{Status} {path} …"', () => {
    render(
      <PrFileRow header={header('notes/todo.txt', 'untracked')} active={false} onOpen={vi.fn()} />,
    );
    expect(
      screen.getByRole('button', { name: /^Untracked\s*notes\/todo\.txt/ }),
    ).toBeInTheDocument();
  });
});
