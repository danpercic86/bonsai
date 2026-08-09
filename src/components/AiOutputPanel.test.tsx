/** T3.7 — AiOutputPanel: read-only prose card for AI explain/review output.
 *  Presentational only (no IPC). Covers: loading skeleton, error banner,
 *  prose render, copy action (clipboard + "Copied" flip), cost badge,
 *  editable textarea mode + onEdit, close wiring. */
import { describe, it, expect, vi, afterEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { AiOutputPanel } from './AiOutputPanel';

function clipboardStub(impl?: () => Promise<void>) {
  const writeText = vi.fn(impl ?? (async () => {}));
  Object.defineProperty(navigator, 'clipboard', {
    value: { writeText },
    configurable: true,
  });
  return writeText;
}

describe('AiOutputPanel', () => {
  afterEach(() => {
    // Remove the stubbed clipboard so it can't leak between tests.
    delete (navigator as unknown as { clipboard?: unknown }).clipboard;
  });

  it('renders the title and prose text when loaded', () => {
    render(
      <AiOutputPanel
        title="Explain commit abc1234"
        text="This commit adds a feature."
        loading={false}
        error={null}
        onClose={vi.fn()}
      />,
    );
    expect(screen.getByRole('region', { name: 'Explain commit abc1234' })).toBeInTheDocument();
    expect(screen.getByText('This commit adds a feature.')).toBeInTheDocument();
  });

  it('shows a skeleton (no text, no Copy) while loading', () => {
    render(
      <AiOutputPanel title="Working" text={null} loading error={null} onClose={vi.fn()} />,
    );
    expect(screen.queryByRole('button', { name: 'Copy to clipboard' })).not.toBeInTheDocument();
    expect(document.querySelector('.skeleton-group')).toBeInTheDocument();
  });

  it('renders a dismissible error banner and no prose', () => {
    const onClose = vi.fn();
    render(
      <AiOutputPanel
        title="Explain"
        text={null}
        loading={false}
        error="model unavailable"
        onClose={onClose}
      />,
    );
    expect(screen.getByRole('alert')).toHaveTextContent('model unavailable');
    fireEvent.click(screen.getByRole('button', { name: 'Dismiss error' }));
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it('shows the cost badge only when costUsd is present', () => {
    const { rerender } = render(
      <AiOutputPanel title="X" text="ok" loading={false} error={null} onClose={vi.fn()} />,
    );
    expect(document.querySelector('.ai-output-cost')).not.toBeInTheDocument();
    rerender(
      <AiOutputPanel
        title="X"
        text="ok"
        loading={false}
        error={null}
        costUsd={0.0123}
        onClose={vi.fn()}
      />,
    );
    expect(screen.getByText('$0.0123')).toBeInTheDocument();
  });

  it('copy writes the text to the clipboard and flips the label to Copied', async () => {
    const writeText = clipboardStub();
    render(
      <AiOutputPanel title="X" text="copy me" loading={false} error={null} onClose={vi.fn()} />,
    );
    fireEvent.click(screen.getByRole('button', { name: 'Copy to clipboard' }));
    expect(writeText).toHaveBeenCalledWith('copy me');
    await waitFor(() =>
      expect(screen.getByRole('button', { name: 'Copy to clipboard' })).toHaveTextContent(
        'Copied',
      ),
    );
  });

  it('close button fires onClose', () => {
    const onClose = vi.fn();
    render(
      <AiOutputPanel title="X" text="hi" loading={false} error={null} onClose={onClose} />,
    );
    fireEvent.click(screen.getByRole('button', { name: 'Close AI output' }));
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it('editable mode renders a textarea seeded from text and reports edits via onEdit', () => {
    const onEdit = vi.fn();
    render(
      <AiOutputPanel
        title="Changelog"
        text="draft body"
        loading={false}
        error={null}
        editable
        onEdit={onEdit}
        onClose={vi.fn()}
      />,
    );
    const ta = screen.getByRole('textbox', { name: 'Changelog (editable)' }) as HTMLTextAreaElement;
    expect(ta.value).toBe('draft body');
    fireEvent.change(ta, { target: { value: 'draft body!' } });
    expect(onEdit).toHaveBeenLastCalledWith('draft body!');
  });

  it('editable copy uses the local draft, not the original text', () => {
    const writeText = clipboardStub();
    render(
      <AiOutputPanel
        title="Changelog"
        text="orig"
        loading={false}
        error={null}
        editable
        onEdit={vi.fn()}
        onClose={vi.fn()}
      />,
    );
    const ta = screen.getByRole('textbox', { name: 'Changelog (editable)' });
    fireEvent.change(ta, { target: { value: 'edited' } });
    fireEvent.click(screen.getByRole('button', { name: 'Copy to clipboard' }));
    expect(writeText).toHaveBeenCalledWith('edited');
  });

  it('re-seeds the draft when the underlying text changes (a new generation)', () => {
    const { rerender } = render(
      <AiOutputPanel
        title="Changelog"
        text="first"
        loading={false}
        error={null}
        editable
        onEdit={vi.fn()}
        onClose={vi.fn()}
      />,
    );
    rerender(
      <AiOutputPanel
        title="Changelog"
        text="second"
        loading={false}
        error={null}
        editable
        onEdit={vi.fn()}
        onClose={vi.fn()}
      />,
    );
    expect(
      (screen.getByRole('textbox', { name: 'Changelog (editable)' }) as HTMLTextAreaElement).value,
    ).toBe('second');
  });
});
