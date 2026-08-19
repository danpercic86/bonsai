/** P73 §9: SubmoduleRow — badge copy/title per status, the busy pill, and the
 *  context-menu hand-off. Presentational: props in, callbacks out. */
import { describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen } from '@testing-library/react';

import { SubmoduleRow } from './SubmoduleRow';
import { SUBMODULE_BADGE } from './submoduleBadges';
import type { SubmoduleInfo, SubmoduleStatus } from '../../ipc';

function sub(over: Partial<SubmoduleInfo> = {}): SubmoduleInfo {
  return {
    name: 'vendor/libcore',
    path: 'vendor/libcore',
    absPath: '/mock/repo/vendor/libcore',
    url: 'https://example.com/libcore.git',
    headOid: null,
    indexOid: null,
    wtOid: null,
    status: 'uninitialized',
    ...over,
  };
}

describe('SubmoduleRow badge', () => {
  it('uninitialized reads "not checked out" with the remedy in the title', () => {
    render(<SubmoduleRow sub={sub()} submoduleBusy={null} onContextMenu={vi.fn()} />);
    const label = screen.getByText('not checked out');
    expect(label.closest('span[title]')).toHaveAttribute(
      'title',
      'No files on disk yet. Right-click the row → Initialize and check out.',
    );
    expect(screen.getByText('vendor/libcore')).toHaveAttribute('title', 'vendor/libcore');
    expect(document.querySelector('li[aria-busy]')).toBeNull();
  });

  it('every status has a label, a title that is not the label, and a glyph only for verdicts', () => {
    const statuses: SubmoduleStatus[] = [
      'uninitialized',
      'upToDate',
      'outOfSync',
      'modifiedWorkdir',
    ];
    for (const status of statuses) {
      const badge = SUBMODULE_BADGE[status];
      const { unmount } = render(
        <SubmoduleRow sub={sub({ status })} submoduleBusy={null} onContextMenu={vi.fn()} />,
      );
      // What the user sees: a non-empty label whose tooltip adds information.
      const label = screen.getByText(badge.label);
      expect(badge.label.length).toBeGreaterThan(0);
      const pill = label.closest('span[title]');
      expect(pill).not.toBeNull();
      expect(pill).toHaveAttribute('title', badge.title);
      expect(pill?.getAttribute('title')).not.toBe(badge.label);
      // ui-reference §11: only a verdict pill renders a decorative glyph, and it
      // is always aria-hidden (the label carries the meaning).
      const glyph = pill?.querySelector('[aria-hidden="true"]') ?? null;
      expect(glyph === null).toBe(status === 'uninitialized');
      unmount();
    }
    render(
      <SubmoduleRow
        sub={sub({ status: 'outOfSync' })}
        submoduleBusy={null}
        onContextMenu={vi.fn()}
      />,
    );
    expect(screen.getByText('⚠')).toHaveAttribute('aria-hidden', 'true');
    expect(screen.getByText('out of sync')).toBeInTheDocument();
  });
});

describe('SubmoduleRow busy state', () => {
  it('shows the participle pill + aria-busy for THIS row only', () => {
    const busyOn = { name: 'vendor/libcore', label: 'checking out…' };
    render(
      <>
        <SubmoduleRow sub={sub()} submoduleBusy={busyOn} onContextMenu={vi.fn()} />
        <SubmoduleRow
          sub={sub({ name: 'docs/spec', path: 'docs/spec', status: 'outOfSync' })}
          submoduleBusy={busyOn}
          onContextMenu={vi.fn()}
        />
      </>,
    );
    // Assert the rendered affordance (visible participle + aria-busy on the row),
    // not a class name — consistent with the other cases.
    const busy = screen.getByText('checking out…');
    expect(busy).toBeVisible();
    expect(busy.closest('li')).toHaveAttribute('aria-busy', 'true');
    // The other row is untouched and not marked busy.
    expect(screen.getByText('out of sync').closest('li')).not.toHaveAttribute('aria-busy');
    expect(screen.queryByText('not checked out')).toBeNull();
  });

  it('still opens the context menu with the submodule NAME while busy', () => {
    const onContextMenu = vi.fn();
    render(
      <SubmoduleRow
        sub={sub({ name: 'the-name', path: 'other/path' })}
        submoduleBusy={{ name: 'the-name', label: 'checking out…' }}
        onContextMenu={onContextMenu}
      />,
    );
    fireEvent.contextMenu(screen.getByText('the-name').closest('li')!, { clientX: 4, clientY: 5 });
    expect(onContextMenu).toHaveBeenCalledWith('the-name', 4, 5);
  });
});
