/**
 * P68g §6.2 — the AI consent copy, which is the only place the user is told what
 * leaves this machine. Security audit M2: the previous wording claimed the payload
 * was "the contents of conflicted files" and that "no files are changed without your
 * review", and BOTH halves were false. These assertions are therefore verbatim: a
 * paraphrase here is a factual regression, not a style change.
 */
import { describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen } from '@testing-library/react';
import { AiConsentDialog } from './AiConsentDialog';

function mount(open = true) {
  const onConfirm = vi.fn();
  const onCancel = vi.fn();
  const view = render(
    <AiConsentDialog open={open} onConfirm={onConfirm} onCancel={onCancel} />,
  );
  return { ...view, onConfirm, onCancel };
}

const BLOCKS = [
  "Bonsai resolves conflicts with the Claude Code CLI installed on this machine, under your Claude subscription. Nothing is sent to Bonsai's own servers.",
  'Claude receives the conflicting versions of the files you choose — and it can read other files in this repository while it works, which is what lets it match your surrounding code. Whatever it reads is sent to Anthropic with the request.',
  'Its tools are read-only: it cannot write files, stage anything, or run commands, and reads outside this repository folder are refused. Refused reads appear in the AI activity dock.',
  "Bonsai changes your files only when you apply a result. The exception is “Resolve automatically” under Settings → AI assistance, which writes and stages Claude's results with no review step.",
];

/** Normalised body text: the apostrophes/quotes as rendered, whitespace collapsed. */
function bodyText(): string {
  const body = document.querySelector('.dialog-body');
  return (body?.textContent ?? '').replace(/\s+/g, ' ').trim();
}

describe('AiConsentDialog', () => {
  it('renders nothing while closed', () => {
    mount(false);
    expect(screen.queryByRole('dialog')).toBeNull();
  });

  it('states the four facts, in order, verbatim', () => {
    mount();
    const text = bodyText();
    let cursor = -1;
    for (const block of BLOCKS) {
      const at = text.indexOf(block);
      expect(at, `missing or reworded: ${block.slice(0, 48)}…`).toBeGreaterThan(cursor);
      cursor = at;
    }
  });

  it('the confirm button is primary, not danger — this opt-in destroys nothing', () => {
    const { onConfirm } = mount();
    const enable = screen.getByRole('button', { name: 'Enable' });
    expect(enable).toHaveClass('btn-primary');
    expect(enable).not.toHaveClass('btn-danger');
    fireEvent.click(enable);
    expect(onConfirm).toHaveBeenCalledTimes(1);
  });

  it('initial focus is Cancel, and Esc cancels', () => {
    const { onCancel } = mount();
    expect(screen.getByRole('button', { name: 'Cancel' })).toHaveFocus();
    fireEvent.keyDown(window, { key: 'Escape' });
    expect(onCancel).toHaveBeenCalledTimes(1);
  });

  it('the dialog is titled as a question and widened for the longer body', () => {
    mount();
    const card = screen.getByRole('dialog', { name: 'Enable AI features?' });
    expect(card).toHaveClass('dialog-card', 'ai-consent-card');
    // The old body repeated the title as its last line; that line is gone.
    expect(bodyText().endsWith('Enable AI features?')).toBe(false);
  });
});

/** Every frontend source file, read as text by Vite (no node:fs — this tsconfig has
 *  no node types, and the harness must stay browser-shaped). */
const SOURCES = import.meta.glob('../../**/*.{ts,tsx}', {
  query: '?raw',
  import: 'default',
  eager: true,
}) as Record<string, string>;

/** The two retired claims, spelled once so this file is the only place they exist. */
const CLAIMS = ['no files are changed without your review', 'contents of conflicted files'];

// Audit M2's regression guard: neither may come back anywhere, including in a comment
// or a fixture.
describe('the retired false claims', () => {
  it('appear nowhere in src/', () => {
    const offenders: string[] = [];
    for (const [path, text] of Object.entries(SOURCES)) {
      if (path.endsWith('AiConsentDialog.test.tsx')) continue;
      for (const claim of CLAIMS) {
        if (text.includes(claim)) offenders.push(`${path}: ${claim}`);
      }
    }
    expect(offenders).toEqual([]);
  });
});
