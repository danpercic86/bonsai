/** P113 §7 — the MCP outcome notes do not outlive the surface that shows them.
 *  Moved out of `SettingsPanel.test.tsx` (526 lines) unchanged; the shared
 *  harness is `src/test/settingsPanelKit.tsx`. */
import { describe, it, expect, vi } from 'vitest';
import { screen, fireEvent } from '@testing-library/react';
import { useEffect } from 'react';

import { renderPanel } from '../../test/settingsPanelKit';

/** A PASSIVE effect that logs when it ran, and nothing else. Rendered as the
 *  panel's preceding sibling by the ordering case below. */
function PassiveMarker({ order }: { order: string[] }) {
  useEffect(() => {
    order.push('marker');
  }, [order]);
  return null;
}

describe('P113 §7 — the MCP outcome notes do not outlive the surface that shows them', () => {
  it('resets them when the AI page unmounts — what closing Settings does', () => {
    const onResetMcpOutcomes = vi.fn();
    const { unmount } = renderPanel({ initialCategory: 'ai', onResetMcpOutcomes });
    // The note map is owned by `useMcpControls`, which App mounts for the app's
    // whole lifetime (§17.3), so the page that shows the notes is the only thing
    // that can say when they die. The mount pass comes first — it covers a
    // `report` that landed after the section was already gone.
    const afterMount = onResetMcpOutcomes.mock.calls.length;
    expect(afterMount).toBeGreaterThanOrEqual(1);

    // `SettingsPanel` returns null when closed, so tearing the tree down is
    // exactly the teardown a close performs.
    unmount();
    expect(onResetMcpOutcomes.mock.calls.length).toBeGreaterThan(afterMount);
  });

  it('resets them on leaving the category, without closing Settings', () => {
    const onResetMcpOutcomes = vi.fn();
    renderPanel({ initialCategory: 'ai', onResetMcpOutcomes });
    const afterMount = onResetMcpOutcomes.mock.calls.length;

    // §7 names both events; the shell renders ONE category at a time, so a rail
    // click unmounts the AI page while Settings stays open.
    fireEvent.click(screen.getByRole('tab', { name: 'General' }));
    expect(onResetMcpOutcomes.mock.calls.length).toBeGreaterThan(afterMount);
  });

  /**
   * The timing half of §7's mount clear (`AiCategory.tsx:71`): it is a
   * `useLayoutEffect`, deliberately, so the clear lands BEFORE paint. A passive
   * mount effect would still clear the notes — the two cases above pass either
   * way — but a remount after a late `report` would commit one painted frame
   * with the note visible and a freshly inserted `role="status"` region that
   * already carries text, which some assistive tech voices.
   *
   * "Before paint" is not directly observable in a test env with no painting,
   * but its React-level equivalent is: every layout effect in a commit runs
   * before ANY passive effect in that same commit. So a passive effect rendered
   * AHEAD of the panel in tree order is the discriminator — passive effects run
   * in tree order, so it precedes the panel's own passive effects and follows
   * the panel's layout effects.
   *
   * The sibling must come FIRST. Rendered after the panel it would log after
   * the panel's subtree either way, and this case would pass with `:71` swapped
   * back to `useEffect` — i.e. it would be no coverage at all.
   */
  it('clears on mount in the LAYOUT phase — before any passive effect of the same commit', () => {
    const order: string[] = [];
    const onResetMcpOutcomes = vi.fn(() => {
      order.push('reset');
    });
    renderPanel({ initialCategory: 'ai', onResetMcpOutcomes }, <PassiveMarker order={order} />);

    // The marker really ran, so the ordering below is a comparison and not a
    // vacuous "reset was first in a one-element list".
    expect(order).toContain('marker');
    // Not `toEqual(['reset', 'marker'])`: the mount clear may legitimately fire
    // more than once (StrictMode, a re-render), and this case rules on order.
    expect(order[0]).toBe('reset');
  });

  it('does not reset while the user is somewhere else in Settings', () => {
    // Scoped to the AI PAGE, not to the panel: opening Settings on General must
    // not touch the notes, and this case is what fails if the effect is ever
    // hoisted to the shell or the adapter (neither of which unmounts on a
    // category change, so hoisting it would also break the case above).
    const onResetMcpOutcomes = vi.fn();
    renderPanel({ initialCategory: 'general', onResetMcpOutcomes });
    expect(onResetMcpOutcomes).not.toHaveBeenCalled();
  });
});
