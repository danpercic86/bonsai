/** P72 (contract §3.3 z): the "Open in browser" link is IPC-routed via the
 *  injected callback (a bare `target="_blank"` is a silent no-op in the Tauri
 *  webview), keeps link semantics, and exposes an accessible name WITHOUT the
 *  decorative ↗ glyph. */
import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { PrDetailView } from './PrDetailView';
import { FORGE_PR_DETAIL } from '../ipc/fixtures/forge';

function renderDetail() {
  const onOpenUrl = vi.fn();
  const onBack = vi.fn();
  render(<PrDetailView detail={FORGE_PR_DETAIL} onBack={onBack} onOpenUrl={onOpenUrl} />);
  return { onOpenUrl, onBack };
}

const url = FORGE_PR_DETAIL.summary.url;
// The ↗ is aria-hidden, so the accessible name is the plain phrase.
const link = () => screen.getByRole('link', { name: 'Open in browser' });

describe('PrDetailView — P72 open-in-browser link', () => {
  it('routes a plain click through onOpenUrl and prevents navigation', () => {
    const { onOpenUrl } = renderDetail();
    const ev = new window.MouseEvent('click', { bubbles: true, cancelable: true });
    fireEvent(link(), ev);
    expect(onOpenUrl).toHaveBeenCalledTimes(1);
    expect(onOpenUrl).toHaveBeenCalledWith(url);
    expect(ev.defaultPrevented).toBe(true);
  });

  it('keeps the href, rel and a hidden glyph', () => {
    renderDetail();
    const a = link();
    expect(a).toHaveAttribute('href', url);
    const rel = a.getAttribute('rel') ?? '';
    expect(rel).toContain('noreferrer');
    expect(rel).toContain('noopener');
    expect(a.textContent).toContain('↗'); // visual result unchanged
    expect(a.querySelector('[aria-hidden="true"]')?.textContent).toBe('↗');
  });

  it('does NOT intercept a ctrl/middle click', () => {
    const { onOpenUrl } = renderDetail();
    for (const init of [{ ctrlKey: true }, { metaKey: true }, { button: 1 }]) {
      const ev = new window.MouseEvent('click', { bubbles: true, cancelable: true, ...init });
      fireEvent(link(), ev);
      expect(onOpenUrl).not.toHaveBeenCalled();
      expect(ev.defaultPrevented).toBe(false);
    }
  });
});
