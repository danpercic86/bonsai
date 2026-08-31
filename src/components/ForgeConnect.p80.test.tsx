/** P80 §1.6a + §5 — ForgeConnect `add` mode copy and the refreshed GitLab /
 *  Bitbucket connect-hint strings (GitHub unchanged). */
import { describe, it, expect, vi } from 'vitest';
import { render, screen } from '@testing-library/react';

import { CONNECT_HINTS, ForgeConnect } from './ForgeConnect';

describe('ForgeConnect — P80 add mode', () => {
  it('renders the add-another heading, sub-line, submit label and a Cancel', () => {
    render(
      <ForgeConnect
        provider="gitHub"
        host="github.com"
        owner="octo-org"
        repo="bonsai"
        submitting={false}
        error={null}
        mode="add"
        onSubmit={vi.fn()}
        onCancel={vi.fn()}
        onOpenUrl={vi.fn()}
      />,
    );
    expect(screen.getByText('Add another account for github.com')).toBeInTheDocument();
    expect(
      screen.getByText(
        'Paste a token for a different account. This repository will use the new account.',
      ),
    ).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Add account' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Cancel' })).toBeInTheDocument();
  });

  it('shows the busy submit label while submitting', () => {
    render(
      <ForgeConnect
        provider="gitHub"
        host="github.com"
        owner="octo-org"
        repo="bonsai"
        submitting={true}
        error={null}
        mode="add"
        onSubmit={vi.fn()}
        onOpenUrl={vi.fn()}
      />,
    );
    expect(screen.getByRole('button', { name: 'Adding…' })).toBeInTheDocument();
  });
});

describe('CONNECT_HINTS — P80 §5 copy refresh', () => {
  it('GitLab names the "api" scope and rejects read-only scopes', () => {
    expect(CONNECT_HINTS.gitLab.scopes).toBe(
      'Use a personal access token with the "api" scope. Read-only scopes such as "read_api" or "read_repository" are not enough to create merge requests.',
    );
  });

  it('Bitbucket names Pull requests (read and write) and the app-password retirement', () => {
    expect(CONNECT_HINTS.bitbucket.scopes).toBe(
      'Use a repository or workspace access token with Pull requests (read and write). App passwords still work but Atlassian is retiring them during 2026, so prefer an access token.',
    );
  });

  it('GitHub copy is unchanged (fine-grained token guidance)', () => {
    expect(CONNECT_HINTS.gitHub.scopes).toContain('fine-grained token');
  });
});
