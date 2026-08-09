/** T3.3a — PaneDivider: pointer-drag deltas (normalized per side), the
 *  pointerup commit point, and keyboard nudges. jsdom lacks the pointer-capture
 *  API on elements, so it is stubbed here (the component calls it directly). */
import { describe, it, expect, vi, beforeAll } from 'vitest';
import { render, fireEvent, screen } from '@testing-library/react';
import { PaneDivider } from './PaneDivider';

beforeAll(() => {
  // jsdom has no pointer-capture; make the calls harmless no-ops.
  Element.prototype.setPointerCapture ??= () => {};
  Element.prototype.releasePointerCapture ??= () => {};
  Element.prototype.hasPointerCapture ??= () => true;
});

function renderDivider(side: 'sidebar' | 'right-panel') {
  const onResize = vi.fn();
  const onResizeEnd = vi.fn();
  render(<PaneDivider side={side} onResize={onResize} onResizeEnd={onResizeEnd} />);
  return { divider: screen.getByRole('separator'), onResize, onResizeEnd };
}

/** jsdom's PointerEvent constructor drops MouseEvent init fields (clientX comes
 *  back NaN), so build the pointer events on MouseEvent and patch pointerId. */
function firePointer(
  el: Element,
  type: 'pointerdown' | 'pointermove' | 'pointerup' | 'pointercancel',
  clientX = 0,
) {
  const ev = new MouseEvent(type, { bubbles: true, cancelable: true, clientX });
  Object.defineProperty(ev, 'pointerId', { value: 1 });
  fireEvent(el, ev);
}

describe('PaneDivider', () => {
  it('drag on the sidebar divider reports raw rightward deltas per move', () => {
    const { divider, onResize, onResizeEnd } = renderDivider('sidebar');
    firePointer(divider, 'pointerdown', 100);
    firePointer(divider, 'pointermove', 110);
    expect(onResize).toHaveBeenLastCalledWith(10);
    firePointer(divider, 'pointermove', 104);
    expect(onResize).toHaveBeenLastCalledWith(-6);
    expect(onResizeEnd).not.toHaveBeenCalled(); // commit only on release
    firePointer(divider, 'pointerup', 104);
    expect(onResizeEnd).toHaveBeenCalledTimes(1);
  });

  it('right-panel dragging negates the delta (leftward drag grows the panel)', () => {
    const { divider, onResize } = renderDivider('right-panel');
    firePointer(divider, 'pointerdown', 500);
    firePointer(divider, 'pointermove', 490);
    expect(onResize).toHaveBeenLastCalledWith(10);
  });

  it('moves without a preceding pointerdown are ignored', () => {
    const { divider, onResize, onResizeEnd } = renderDivider('sidebar');
    firePointer(divider, 'pointermove', 200);
    firePointer(divider, 'pointerup');
    expect(onResize).not.toHaveBeenCalled();
    expect(onResizeEnd).not.toHaveBeenCalled();
  });

  it('pointercancel also ends the drag exactly once', () => {
    const { divider, onResizeEnd } = renderDivider('sidebar');
    firePointer(divider, 'pointerdown', 100);
    firePointer(divider, 'pointercancel');
    firePointer(divider, 'pointercancel'); // second is a no-op
    expect(onResizeEnd).toHaveBeenCalledTimes(1);
  });

  it('ArrowLeft/ArrowRight nudge by 8px and commit immediately', () => {
    const { divider, onResize, onResizeEnd } = renderDivider('sidebar');
    fireEvent.keyDown(divider, { key: 'ArrowRight' });
    expect(onResize).toHaveBeenLastCalledWith(8);
    fireEvent.keyDown(divider, { key: 'ArrowLeft' });
    expect(onResize).toHaveBeenLastCalledWith(-8);
    expect(onResizeEnd).toHaveBeenCalledTimes(2);
  });

  it('keyboard nudges are normalized for the right panel too', () => {
    const { divider, onResize } = renderDivider('right-panel');
    fireEvent.keyDown(divider, { key: 'ArrowLeft' });
    expect(onResize).toHaveBeenLastCalledWith(8); // leftward grows the right panel
  });
});
