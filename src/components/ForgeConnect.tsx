import { useState, type MouseEvent } from 'react';
import type { ForgeKind } from '../ipc';

// P62c: paste-a-PAT affordance shown when the host has no stored token. The
// token is submitted to the container (forgeSetToken → OS keychain); it is
// NEVER prefilled, autofilled, echoed, or persisted in the DOM beyond this
// controlled input. Password type keeps it masked on screen.
//
// P64c (contract §3e): the copy is per-provider — a short line naming the token
// type/scopes each forge needs plus a "create a token" help link. Presentational
// only; still a password input, keychain note, no prefill.

export interface ForgeConnectProps {
  provider: ForgeKind;
  host: string;
  owner: string;
  repo: string;
  submitting: boolean;
  error: string | null;
  onSubmit(token: string): void;
  /** P72: route "Create a token" through the openUrl IPC — a bare
   *  `target="_blank"` is a silent no-op in the native webview. REQUIRED (not
   *  optional) so a future call site cannot regress to a dead link. This
   *  component stays presentational: no ipc, no error handling. */
  onOpenUrl(url: string): void;
}

/** A modified/auxiliary click (ctrl/cmd/shift/alt, middle button) must reach the
 *  platform untouched, so open-in-new-tab keeps working in the browser harness;
 *  in the Tauri webview it is a no-op. `true` ⇒ do NOT intercept. */
function isPlatformClick(e: MouseEvent): boolean {
  return e.metaKey || e.ctrlKey || e.shiftKey || e.altKey || e.button !== 0;
}

/** Per-provider connect guidance. A full `Record<ForgeKind, …>` so adding a new
 *  `ForgeKind` (e.g. Azure DevOps in P64d) is a compile error until its hint is
 *  supplied. `unknown` never reaches this panel (the container shows the
 *  unsupported state instead) but carries a sensible generic fallback. */
interface ConnectHint {
  /** One line naming the required token type/scopes. */
  scopes: string;
  /** "Create a token" destination; empty ⇒ no link rendered. */
  url: string;
  /** Masked-input placeholder hinting the token's shape. */
  placeholder: string;
}

const CONNECT_HINTS: Record<ForgeKind, ConnectHint> = {
  gitHub: {
    scopes:
      'Use a fine-grained token with Pull requests (read/write) and Contents (read) permissions — Metadata is added automatically — or a classic token with the "repo" scope.',
    url: 'https://github.com/settings/personal-access-tokens/new',
    placeholder: 'github_pat_…',
  },
  gitLab: {
    scopes: 'Use a personal access token with the "api" scope.',
    url: 'https://gitlab.com/-/user_settings/personal_access_tokens',
    placeholder: 'glpat-…',
  },
  bitbucket: {
    scopes:
      'Use a repository or workspace access token (or an app password) with pull-request read and write.',
    url: 'https://support.atlassian.com/bitbucket-cloud/docs/create-a-repository-access-token/',
    placeholder: 'access token',
  },
  azureDevOps: {
    scopes: 'Use an Azure DevOps personal access token with Code (Read & Write).',
    url: 'https://learn.microsoft.com/azure/devops/organizations/accounts/use-personal-access-tokens-to-authenticate',
    placeholder: 'Azure DevOps PAT',
  },
  unknown: {
    scopes: 'Use a personal access token with read and write access to pull requests.',
    url: '',
    placeholder: 'token',
  },
};

export function ForgeConnect({
  provider,
  host,
  owner,
  repo,
  submitting,
  error,
  onSubmit,
  onOpenUrl,
}: ForgeConnectProps) {
  const [token, setToken] = useState('');
  const canSubmit = !submitting && token.trim() !== '';
  const hint = CONNECT_HINTS[provider] ?? CONNECT_HINTS.unknown;

  return (
    <form
      className="forge-connect"
      autoComplete="off"
      onSubmit={(e) => {
        e.preventDefault();
        if (canSubmit) onSubmit(token);
      }}
    >
      <h3 className="forge-connect-heading">{`Connect to ${host}`}</h3>
      <p className="forge-connect-sub">
        {`Paste a personal access token to view and open pull requests for `}
        <span className="mono">{`${owner}/${repo}`}</span>
        {'.'}
      </p>
      <p className="forge-connect-hint">
        {hint.scopes}
        {hint.url !== '' && (
          <>
            {' '}
            <a
              className="forge-connect-link"
              href={hint.url}
              target="_blank"
              rel="noreferrer noopener"
              onClick={(e) => {
                if (isPlatformClick(e)) return; // let ctrl/middle-click through
                e.preventDefault();
                onOpenUrl(hint.url);
              }}
            >
              Create a token
            </a>
          </>
        )}
      </p>
      <label className="pr-field">
        <span className="pr-field-label">Personal access token</span>
        <input
          className="pr-input"
          type="password"
          autoComplete="off"
          autoCorrect="off"
          spellCheck={false}
          placeholder={hint.placeholder}
          value={token}
          disabled={submitting}
          onChange={(e) => setToken(e.target.value)}
        />
      </label>
      <p className="forge-connect-note">
        Stored in your OS keychain, never in a settings file. It is sent only as an
        authorization header and is never logged.
      </p>

      {error !== null && (
        <div className="error-banner error-banner-dismissible pr-error" role="alert">
          <span className="error-banner-text">{error}</span>
        </div>
      )}

      <button type="submit" className="btn-primary forge-connect-submit" disabled={!canSubmit}>
        {submitting ? 'Connecting…' : 'Connect'}
      </button>
    </form>
  );
}
