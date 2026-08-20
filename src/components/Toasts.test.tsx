/** T3.3a — Toasts stack. Presentational by contract: App owns the id counter,
 *  the 5-toast cap and the 5 s auto-dismiss timers, so timer behavior is tested
 *  at the App/hook level — here we cover rendering order, tone variants, the
 *  alert role for errors, and the dismiss callback. */
import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { Toasts, type Toast } from './Toasts';

const toast = (id: number, tone: Toast['tone'], text: string): Toast => ({
  id,
  tone,
  text,
  sticky: tone === 'error',
});

describe('Toasts', () => {
  it('renders nothing for an empty stack', () => {
    const { container } = render(<Toasts toasts={[]} onDismiss={vi.fn()} />);
    expect(container).toBeEmptyDOMElement();
  });

  it('renders every pushed toast, newest (highest id) on top', () => {
    const { container } = render(
      <Toasts
        toasts={[toast(1, 'info', 'first'), toast(2, 'success', 'second')]}
        onDismiss={vi.fn()}
      />,
    );
    const texts = Array.from(container.querySelectorAll('.toast-text')).map(
      (el) => el.textContent,
    );
    expect(texts).toEqual(['second', 'first']); // reversed: newest first
  });

  it('tone drives the variant class; only errors get role=alert', () => {
    const { container } = render(
      <Toasts
        toasts={[toast(1, 'error', 'boom'), toast(2, 'info', 'fyi'), toast(3, 'warning', 'careful')]}
        onDismiss={vi.fn()}
      />,
    );
    expect(container.querySelector('.toast-error')).toHaveTextContent('boom');
    expect(container.querySelector('.toast-info')).toHaveTextContent('fyi');
    expect(container.querySelector('.toast-warning')).toHaveTextContent('careful');
    const alerts = screen.getAllByRole('alert');
    expect(alerts).toHaveLength(1);
    expect(alerts[0]).toHaveTextContent('boom');
  });

  it('the stack is an aria-live polite region', () => {
    const { container } = render(
      <Toasts toasts={[toast(1, 'info', 'fyi')]} onDismiss={vi.fn()} />,
    );
    expect(container.querySelector('.toast-stack')).toHaveAttribute('aria-live', 'polite');
  });

  it('dismiss button reports exactly the clicked toast id', () => {
    const onDismiss = vi.fn();
    render(
      <Toasts
        toasts={[toast(7, 'info', 'first'), toast(9, 'error', 'second')]}
        onDismiss={onDismiss}
      />,
    );
    // Newest first: buttons[0] belongs to id 9, buttons[1] to id 7.
    const buttons = screen.getAllByRole('button', { name: 'Dismiss' });
    fireEvent.click(buttons[1]);
    expect(onDismiss).toHaveBeenCalledTimes(1);
    expect(onDismiss).toHaveBeenCalledWith(7);
  });

  // P74 §1.2: shape, not colour, carries the tone (WCAG 1.4.1).
  it('gives every tone a distinct aria-hidden glyph', () => {
    const tones = ['error', 'warning', 'success', 'info'] as const;
    const { container } = render(
      <Toasts toasts={tones.map((t, i) => toast(i + 1, t, t))} onDismiss={vi.fn()} />,
    );
    const glyphs = [...container.querySelectorAll('.toast-glyph')];
    expect(glyphs).toHaveLength(4);
    for (const g of glyphs) expect(g).toHaveAttribute('aria-hidden', 'true');
    const texts = glyphs.map((g) => g.textContent);
    expect(new Set(texts).size).toBe(4);
    // The glyph must never leak into the announced text.
    expect(container.querySelector('.toast-error .toast-text')?.textContent).toBe('error');
  });
});
