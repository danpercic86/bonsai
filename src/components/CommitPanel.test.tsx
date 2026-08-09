/** T3.5 — CommitPanel (mode B, commit selected): metadata rendering, parent
 *  links vs truncated parents, merge note, signature line, body collapse,
 *  Explain gating, and the file-list handoff to DiffFileTree. Presentational —
 *  App owns all fetching. */
import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { CommitPanel } from './CommitPanel';
import type { CommitPanelProps } from './CommitPanel';
import type { CommitDiff, GraphNode } from '../ipc';

const OID = 'a1b2c3d4'.padEnd(40, '0');
const PARENT_A = 'b1'.padEnd(40, '1');
const PARENT_B = 'c2'.padEnd(40, '2');

const NODE: GraphNode = {
  id: OID,
  lane: 0,
  parents: [3], // only the FIRST parent survived the layout truncation
  summary: 'feat: add graph',
  author: 'Ada',
  ts: 1_700_000_000,
  committerTs: 1_700_000_000,
};

function diff(over: Partial<CommitDiff['details']> = {}): CommitDiff {
  return {
    details: {
      oid: OID,
      summary: 'feat: add graph',
      message: 'feat: add graph\n\nBody line 1\nBody line 2',
      authorName: 'Ada Lovelace',
      authorEmail: 'ada@example.com',
      authorTs: 1_700_000_000,
      committerTs: 1_700_000_000,
      parents: [PARENT_A, PARENT_B],
      ...over,
    },
    files: [
      { path: 'src/a.ts', origPath: null, status: 'modified', additions: 3, deletions: 1, binary: false },
      { path: 'src/b.ts', origPath: null, status: 'added', additions: 9, deletions: 0, binary: false },
    ],
  };
}

function renderPanel(over: Partial<CommitPanelProps> = {}) {
  const props: CommitPanelProps = {
    node: NODE,
    data: diff(),
    loading: false,
    error: null,
    listView: 'flat',
    scope: { kind: 'root' },
    onSelectScope: vi.fn(),
    onSelectParent: vi.fn(),
    onClose: vi.fn(),
    aiEligible: false,
    onExplain: vi.fn(),
    signature: null,
    ...over,
  };
  return { ...render(<CommitPanel {...props} />), props };
}

describe('CommitPanel', () => {
  it('renders summary, short oid, author, and the file list with count', () => {
    renderPanel();
    expect(screen.getByText('feat: add graph')).toBeInTheDocument();
    expect(screen.getByText('a1b2c3d')).toBeInTheDocument();
    expect(screen.getByText(/Ada Lovelace/)).toBeInTheDocument();
    expect(screen.getByText('<ada@example.com>')).toBeInTheDocument();
    expect(screen.getByText('Changes (2)')).toBeInTheDocument();
    expect(screen.getByText(/src\/a\.ts/)).toBeInTheDocument();
  });

  it('while loading with no data yet: node summary/oid render with skeletons', () => {
    const { container } = renderPanel({ data: null, loading: true });
    expect(screen.getByText('feat: add graph')).toBeInTheDocument();
    expect(screen.getByText('a1b2c3d')).toBeInTheDocument();
    expect(container.querySelector('.skeleton-group')).toBeInTheDocument();
    expect(screen.queryByText(/Changes \(/)).not.toBeInTheDocument();
  });

  it('parents: in-layout parent is a clickable link; truncated parent is plain text', () => {
    const { props } = renderPanel();
    const link = screen.getByRole('button', { name: PARENT_A.slice(0, 7) });
    fireEvent.click(link);
    expect(props.onSelectParent).toHaveBeenCalledWith(0);
    // Second parent has no layout row (node.parents has 1 entry) -> plain span.
    const plain = screen.getByText(PARENT_B.slice(0, 7));
    expect(plain.tagName).not.toBe('BUTTON');
    // Merge commit -> first-parent note.
    expect(screen.getByText('Showing changes vs first parent')).toBeInTheDocument();
  });

  it('single-parent commit shows no merge note', () => {
    renderPanel({
      node: { ...NODE, parents: [1] },
      data: diff({ parents: [PARENT_A] }),
    });
    expect(screen.queryByText(/vs first parent/)).not.toBeInTheDocument();
  });

  it('message body renders below the header and collapses past 8 lines', () => {
    renderPanel();
    expect(screen.getByText(/Body line 1/).textContent).toBe('Body line 1\nBody line 2');
    const long = Array.from({ length: 12 }, (_, i) => `line ${i + 1}`).join('\n');
    const second = renderPanel({ data: diff({ message: `summary\n\n${long}` }) });
    const pre = second.container.querySelector('.commit-msg-text')!;
    expect(pre.textContent).toContain('line 8');
    expect(pre.textContent).not.toContain('line 9');
    fireEvent.click(screen.getByRole('button', { name: 'Show more' }));
    expect(screen.getByText(/line 12/)).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Show less' })).toBeInTheDocument();
  });

  it('signature line: good/warn kinds render label + signer/key; unsigned renders nothing', () => {
    renderPanel({
      signature: { oid: OID, status: 'good', signer: 'Ada <ada@example.com>', key: 'SHA256:abc' },
    });
    expect(screen.getByText('Good signature')).toBeInTheDocument();
    expect(screen.getByText('Ada <ada@example.com>')).toBeInTheDocument();
    expect(screen.getByText('SHA256:abc')).toBeInTheDocument();
    const bad = renderPanel({ signature: { oid: OID, status: 'bad' } });
    expect(bad.container.querySelector('.commit-signature-warn')).toBeInTheDocument();
    const unsigned = renderPanel({ signature: { oid: OID, status: 'unsigned' } });
    expect(unsigned.container.querySelector('.commit-signature')).not.toBeInTheDocument();
  });

  it('long keys are shortened with an ellipsis but keep the full key as title', () => {
    const key = 'SHA256:' + 'x'.repeat(40);
    renderPanel({ signature: { oid: OID, status: 'good', key } });
    const el = screen.getByTitle(key);
    expect(el.textContent!.endsWith('…')).toBe(true);
    expect(el.textContent!.length).toBe(26); // 25 chars + ellipsis
  });

  it('✨ Explain renders only when aiEligible and fires onExplain', () => {
    renderPanel();
    expect(screen.queryByRole('button', { name: '✨ Explain' })).not.toBeInTheDocument();
    const { props } = renderPanel({ aiEligible: true });
    fireEvent.click(screen.getByRole('button', { name: '✨ Explain' }));
    expect(props.onExplain).toHaveBeenCalledTimes(1);
  });

  it('close button and error banner wiring', () => {
    const { props } = renderPanel({ error: 'diff failed' });
    expect(screen.getByRole('alert')).toHaveTextContent('diff failed');
    fireEvent.click(screen.getByRole('button', { name: 'Close commit details' }));
    expect(props.onClose).toHaveBeenCalledTimes(1);
  });

  it('file rows drive onSelectScope through the shared DiffFileTree', () => {
    const { props } = renderPanel();
    fireEvent.click(screen.getByRole('button', { name: /src\/b\.ts/ }));
    expect(props.onSelectScope).toHaveBeenCalledWith({ kind: 'file', path: 'src/b.ts' });
  });
});
