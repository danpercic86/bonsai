/** P72 (contract §3.3 w–y): the "Create a token" link is IPC-routed, not a bare
 *  `target="_blank"` (which is a silent no-op in the Tauri webview) — while
 *  keeping full link semantics (`href`, `rel`, native keyboard activation) and
 *  deferring to the platform on a modified/auxiliary click. */
import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { ForgeConnect } from './ForgeConnect';
import type { ForgeKind } from '../ipc';

const AZURE_URL =
  'https://learn.microsoft.com/azure/devops/organizations/accounts/use-personal-access-tokens-to-authenticate';

function renderConnect(provider: ForgeKind = 'gitHub') {
  const onOpenUrl = vi.fn();
  const onSubmit = vi.fn();
  render(
    <ForgeConnect
      provider={provider}
      host="github.com"
      owner="octo-org"
      repo="bonsai"
      submitting={false}
      error={null}
      onSubmit={onSubmit}
      onOpenUrl={onOpenUrl}
    />,
  );
  return { onOpenUrl, onSubmit };
}

const link = () => screen.getByRole('link', { name: 'Create a token' });

describe('ForgeConnect — P72 token-page link', () => {
  it('routes a plain click through the callback and prevents navigation', () => {
    const { onOpenUrl } = renderConnect();
    const clickEvent = new window.MouseEvent('click', { bubbles: true, cancelable: true });
    fireEvent(link(), clickEvent);
    expect(onOpenUrl).toHaveBeenCalledTimes(1);
    expect(onOpenUrl).toHaveBeenCalledWith('https://github.com/settings/personal-access-tokens/new');
    expect(clickEvent.defaultPrevented).toBe(true);
  });

  it('passes the per-provider hint URL (Azure DevOps)', () => {
    const { onOpenUrl } = renderConnect('azureDevOps');
    fireEvent.click(link());
    expect(onOpenUrl).toHaveBeenCalledWith(AZURE_URL);
  });

  it('keeps link semantics: real href plus noreferrer AND noopener', () => {
    renderConnect();
    const a = link();
    expect(a).toHaveAttribute('href', 'https://github.com/settings/personal-access-tokens/new');
    const rel = a.getAttribute('rel') ?? '';
    expect(rel).toContain('noreferrer');
    expect(rel).toContain('noopener');
    // Links must NOT be turned into buttons (no role/tabIndex/keydown override).
    expect(a).not.toHaveAttribute('role');
    expect(a).not.toHaveAttribute('tabindex');
  });

  it('names fine-grained permissions in the hint and uses the github_pat_ placeholder', () => {
    renderConnect('gitHub');
    const hint = document.querySelector('.forge-connect-hint');
    expect(hint).toHaveTextContent(/fine-grained token/i);
    expect(hint).toHaveTextContent(/Pull requests/);
    expect(hint).toHaveTextContent(/Contents/);
    expect(screen.getByPlaceholderText('github_pat_…')).toBeInTheDocument();
  });

  it("renders no link for 'unknown' (empty hint URL) and never calls back", () => {
    const { onOpenUrl } = renderConnect('unknown');
    expect(screen.queryByRole('link')).toBeNull();
    expect(onOpenUrl).not.toHaveBeenCalled();
  });

  it('does NOT intercept a modified or auxiliary click (open-in-new-tab still works)', () => {
    const { onOpenUrl } = renderConnect();
    const a = link();
    for (const init of [
      { ctrlKey: true },
      { metaKey: true },
      { shiftKey: true },
      { altKey: true },
      { button: 1 },
    ]) {
      const ev = new window.MouseEvent('click', { bubbles: true, cancelable: true, ...init });
      fireEvent(a, ev);
      expect(onOpenUrl).not.toHaveBeenCalled();
      expect(ev.defaultPrevented).toBe(false);
    }
  });
});
